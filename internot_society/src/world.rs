//! People and kinship as pure functions of `(seed, id, t)`.
//!
//! An id is a rank in a birth block (block = birth year × lineage group, a
//! group being a region and a heritage; a child is born into its mother's
//! group). A block's raw ids are its
//! entry cohorts in order: natives, then immigrants by arrival year (R1b-2).
//! Keyed permutations order them:
//! - each cohort's **life line**: `[women | men]`, each sex as `[sub-cells
//!   by union year | never partnered]`; a block's union cell for a year is
//!   the concatenation of its cohorts' sub-cells;
//! - the natives' **parent line**: births grouped by mother block, then by
//!   mother event (union cell, plan leaf, birth index, then non-union
//!   births by cohort).
//!
//! Each union cell carries two independent partitions (R1 decision D-R1.3):
//! partner (for couples) and fertility plan (for births), each through its
//! own keyed permutation. Couples are slot `j` of slice `(i, j_block)` on
//! the women's side and slot `j` of slice `(j_block, i)` on the men's, so
//! both partners compute the same union. Birth events enumerate identically
//! from the mother's side and the child block's side.
//!
//! Deaths are keyed draws conditioned on the survival a person's own cells
//! require (D-R1.2); the never-partnered draw from the block's residual
//! mortality so block totals stay right.

use procedural_core::key::{label, Key};
use procedural_core::perm::{Bijection, CompactParts, CompactPerm};
use rayon::prelude::*;

use crate::ledger::{cutoff, plan_key, Block, CellKind, Ledger, MotherShare, MIN_UNION_AGE};
use crate::params::{
    Heritage, Params, Sex, DISSOLUTION_BANDS, MAX_AGE, MAX_BIRTH_AGE, MAX_REMARRIAGE_AGE,
    MIN_BIRTH_AGE,
};
use crate::plan::{
    arrival_births, arrival_plans, leaf_births, nonunion_plans, NonUnionLeaf, MAX_PARITY,
};

/// A person's id: a dense rank over everyone ever born in the world.
pub type PersonId = u32;

const TAG_LIFE: u64 = label("world/life");
const TAG_PARENT: u64 = label("world/parent");
const TAG_PARTNER: u64 = label("world/partner");
const TAG_PLAN: u64 = label("world/plan");
const TAG_NONUNION: u64 = label("world/nonunion");
const TAG_BIRTHDAY: u64 = label("person/birthday");
const TAG_DEATH: u64 = label("person/death");
const TAG_COUPLE: u64 = label("couple");
const TAG_ARRIVAL: u64 = label("person/arrival");
const TAG_REMARRY: u64 = label("world/remarry");

/// A union as seen by one partner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Union {
    /// The other partner.
    pub partner: PersonId,
    /// Union start, seconds since 1800.
    pub start: i64,
    /// Planned separation, seconds since 1800 (`None`: lasts until a death).
    pub separation: Option<i64>,
    /// Actual end: separation or the first death.
    pub end: i64,
    /// Key shared by both partners.
    pub key: Key,
}

/// Most children one person can have in R1: a full union plan plus two
/// non-union births.
pub const MAX_KIN: usize = 4 * MAX_PARITY;

/// Most unions one person's lookups can return. A technical bound, not a
/// rule (R1d): anyone can re-partner after any separation, and
/// [`World::unions`] panics if a chain ever runs past this.
pub const MAX_UNIONS: usize = 16;

/// A short list of kin stored inline, so kin lookups never allocate. Reads
/// as a slice of ids.
#[derive(Clone, Copy)]
pub struct KinList {
    ids: [PersonId; MAX_KIN],
    len: u8,
}

impl KinList {
    const fn new() -> Self {
        Self {
            ids: [0; MAX_KIN],
            len: 0,
        }
    }

    fn push(&mut self, id: PersonId) {
        assert!((self.len as usize) < MAX_KIN, "kin list over capacity");
        self.ids[self.len as usize] = id;
        self.len += 1;
    }

    /// The ids, as a slice.
    pub fn as_slice(&self) -> &[PersonId] {
        &self.ids[..self.len as usize]
    }
}

impl std::ops::Deref for KinList {
    type Target = [PersonId];
    fn deref(&self) -> &[PersonId] {
        self.as_slice()
    }
}

impl PartialEq for KinList {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl Eq for KinList {}

impl std::fmt::Debug for KinList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.as_slice()).finish()
    }
}

impl IntoIterator for KinList {
    type Item = PersonId;
    type IntoIter = std::iter::Take<std::array::IntoIter<PersonId, MAX_KIN>>;
    fn into_iter(self) -> Self::IntoIter {
        self.ids.into_iter().take(self.len as usize)
    }
}

impl<'a> IntoIterator for &'a KinList {
    type Item = &'a PersonId;
    type IntoIter = std::slice::Iter<'a, PersonId>;
    fn into_iter(self) -> Self::IntoIter {
        self.as_slice().iter()
    }
}

/// A plan leaf packed into one word: `start << 32 | mask >> 1`, where
/// `start` is the leaf's first position in plan order. Bit `o` of `mask` is
/// set if the leaf's women give birth `o` years after the union year;
/// `1 <= o <= 29`, because unions start at [`MIN_UNION_AGE`] or later and
/// births stop at [`MAX_BIRTH_AGE`] (and before the couple's separation). A
/// leaf's count is the next leaf's start minus its own (see [`Plans`]).
#[derive(Clone, Copy, Debug)]
struct Leaf(u64);

impl Leaf {
    fn pack(start: u64, mask: u32) -> Self {
        assert!(start < 1 << 32, "plan cell too large");
        assert!(
            mask & 1 == 0 && mask < 1 << 30,
            "birth offsets out of range"
        );
        Leaf(start << 32 | (mask >> 1) as u64)
    }

    fn start(self) -> u64 {
        self.0 >> 32
    }

    fn mask(self) -> u32 {
        (self.0 as u32 & 0x1FFF_FFFF) << 1
    }

    /// True if the leaf's women give birth `o` years after the union year.
    fn births_at(self, o: i32) -> bool {
        (0..32).contains(&o) && self.mask() >> o & 1 == 1
    }

    /// Offset of the last planned birth, if any.
    fn last_offset(self) -> Option<i32> {
        let m = self.mask();
        (m != 0).then(|| 31 - m.leading_zeros() as i32)
    }
}

/// The outcome of kin repair over one group of a woman's cell: its women
/// and their default men (group order), the woman at group index `a` taking
/// man `perm[a]`, and the asker's index `at`.
struct Repair {
    women: [PersonId; 3],
    men: [PersonId; 3],
    /// Where each default man sits in his own union cell.
    slots: [ManSlot; 3],
    /// Where each woman sits in her union cell.
    wslots: [ManSlot; 3],
    perm: [u8; 3],
    at: usize,
}

impl Repair {
    /// Group index of the woman who takes man `at`.
    fn taker(&self) -> usize {
        self.perm
            .iter()
            .position(|&x| x as usize == self.at)
            .expect("a permutation")
    }
}

/// A right-role member's union cell (block, sex, index) and position in
/// its partner order ("man" for short: in same-sex cells it can be a woman).
#[derive(Clone, Copy, Debug)]
struct ManSlot {
    block: u32,
    sex: Sex,
    ci: usize,
    pos: u64,
}

impl Default for ManSlot {
    fn default() -> Self {
        Self {
            block: 0,
            sex: Sex::Male,
            ci: 0,
            pos: 0,
        }
    }
}

/// Permutations tried by kin repair, by group size, identity first.
const PERMS: [&[&[u8]]; 4] = [
    &[],
    &[&[0]],
    &[&[0, 1], &[1, 0]],
    &[
        &[0, 1, 2],
        &[0, 2, 1],
        &[1, 0, 2],
        &[1, 2, 0],
        &[2, 0, 1],
        &[2, 1, 0],
    ],
];

/// The repair group of position `q` in a cell of `n` couples, as `(first
/// position, size)`: pairs `(2k, 2k + 1)`, the last three positions of an
/// odd cell, or the lone position of a one-couple cell.
fn repair_group(q: u64, n: u64) -> (u64, u64) {
    if n <= 1 {
        (q, 1)
    } else if n % 2 == 1 && q >= n - 3 {
        (n - 3, 3)
    } else {
        (q & !1, 2)
    }
}

/// A couple's facts that need no death lookups.
struct Couple {
    partner: PersonId,
    key: Key,
    start: i64,
    separation: Option<i64>,
    /// Where the partner sits in this union's cell.
    seat: Seat,
}

/// A birth event on a mother's side: `(year, kind, offset within the event
/// cell)`.
/// For union births `births_of` also precomputes the event's start among
/// the mother block's births that year (`NO_START` otherwise).
type BirthEvent = (i32, EventKind, u64, u64);

/// A birth event whose start `child_of_event` computes itself.
const NO_START: u64 = u64::MAX;

/// A woman's births, in year order, stored inline.
struct Births {
    items: [BirthEvent; MAX_KIN],
    len: usize,
}

impl Births {
    const EMPTY: BirthEvent = (
        0,
        EventKind::NonUnion {
            cohort: 0,
            leaf: 0,
            k: 0,
        },
        0,
        NO_START,
    );

    fn new() -> Self {
        Self {
            items: [Self::EMPTY; MAX_KIN],
            len: 0,
        }
    }

    fn push(&mut self, e: BirthEvent) {
        assert!(self.len < MAX_KIN, "births over capacity");
        self.items[self.len] = e;
        self.len += 1;
    }

    /// Stable insertion sort by year (at most [`MAX_KIN`] items).
    fn sort_by_year(&mut self) {
        for i in 1..self.len {
            let mut j = i;
            while j > 0 && self.items[j - 1].0 > self.items[j].0 {
                self.items.swap(j - 1, j);
                j -= 1;
            }
        }
    }

    fn iter(&self) -> impl Iterator<Item = BirthEvent> + '_ {
        self.items[..self.len].iter().copied()
    }
}

/// A union cell's slice for one partner block: the block, and where its
/// couples start in the cell's partner order. A cell's slices end with a
/// sentinel (block `u32::MAX`, start the cell's total), so one array answers
/// both searches, by position and by block, and gives each slice's length
/// from its neighbour: a lookup touches one array, not two.
#[derive(Clone, Copy, Debug)]
struct Slice {
    block: u32,
    start: u32,
}

/// One of a cohort's sub-cells on its sex line: where it starts on the
/// line, its union cell, its start in that cell's index space, and the
/// cell's year (which fills padding, and spares a death lookup the cell).
/// A line's sub-cells end with a sentinel whose `start` is the line's union
/// total.
#[derive(Clone, Copy, Debug)]
struct Sub {
    start: u32,
    in_cell: u32,
    ci: u16,
    year: i16,
    /// The cell separates and feeds a divorced source (R1c): its members
    /// may hold a second union.
    divorces: bool,
}

/// A cell without plans (men's and same-sex cells).
const NO_PLAN: u16 = u16::MAX;

/// A union cell, in one cache line: R1c splits cells by dissolution
/// class, so a block has many small cells. Its arrays live in the block's
/// arenas; [`CellView`] pairs the two.
#[derive(Clone, Copy, Debug)]
#[repr(align(64))]
struct CellLayout {
    /// Partner-order permutation of the cell (keyed by block, year, sex,
    /// kind and class), without its size (`total`).
    perm: CompactParts,
    size: u32,
    /// The cell's slices (partner-block order, see [`Slice`]) in the block's
    /// slice arena: `n_slices` of them, then a sentinel.
    slices: u32,
    n_slices: u16,
    /// Index of the cell's fertility plans in [`BlockLayout::plans`] (women's
    /// opposite-sex cells only; [`NO_PLAN`] for the rest).
    plan: u16,
    /// `(cohort, in-cell start, line start)` parts in the block's part
    /// arena: where each entry cohort's members begin in the cell's index
    /// space and on the cohort's sex line, in cohort order. The first is
    /// also inline (`first_*`): usually the natives, most of the cell.
    parts: u32,
    n_parts: u16,
    first_cohort: u16,
    first_line: u32,
    first_end: u32,
    /// Right-role cells: partner-order positions of the couples whose woman
    /// is alone in her cell ("free" couples: only the men's side can repair
    /// them; see [`World::repair_men`]), ascending, in the block's free
    /// arena.
    free: u32,
    n_free: u16,
    yr: i16,
    /// Which of the year's union cells (in-world, same-sex left or right,
    /// arrival), and its members' sex.
    kind: CellKind,
    sex: Sex,
    /// Dissolution class (R1c): the couples separate `class` calendar
    /// years after the cell's year (0: never). Kin repair, the permutations
    /// and the plans all work within a class-cell.
    class: u8,
}

/// A cell's fertility plans, in the block's arenas. A block keeps them in
/// one array, in the order of its women's cells.
#[derive(Clone, Copy, Debug)]
struct PlanHead {
    /// Plan-order permutation of the cell, without its size.
    perm: CompactParts,
    total: u32,
    /// The leaves in the block's leaf arena: canonical order, then a
    /// sentinel whose start is the cell's total, so finding a position's
    /// leaf and reading the leaf touch one array. In an arrival cell a
    /// leaf's mask holds its in-world births, counted from the arrival year.
    leaves: u32,
    n_leaves: u16,
    /// The cell (women's cell index) and its birth-table column: the cells
    /// of one `(year, kind)`, every class, share a column.
    cell: u16,
    column: u16,
    /// Arrival cells: per leaf, `(d, kids)` in the block's arrival arena
    /// (years of union before arrival, and a mask of the arrival ages of
    /// children born abroad); `u32::MAX` for the rest.
    arrivals: u32,
}

/// A union cell with its block's arenas.
#[derive(Clone, Copy)]
struct CellView<'a> {
    c: &'a CellLayout,
    l: &'a BlockLayout,
}

impl std::ops::Deref for CellView<'_> {
    type Target = CellLayout;
    fn deref(&self) -> &CellLayout {
        self.c
    }
}

impl<'a> CellView<'a> {
    /// Slices, then the sentinel.
    fn slices(&self) -> &'a [Slice] {
        let a = self.c.slices as usize;
        &self.l.slice_arena[a..a + self.c.n_slices as usize + 1]
    }

    fn parts(&self) -> &'a [(u16, u32, u32)] {
        let a = self.c.parts as usize;
        &self.l.part_arena[a..a + self.c.n_parts as usize]
    }

    fn free(&self) -> &'a [u32] {
        let a = self.c.free as usize;
        &self.l.free_arena[a..a + self.c.n_free as usize]
    }

    fn perm(&self) -> CompactPerm {
        CompactPerm::from_parts(self.c.size as u64, self.c.perm)
    }

    /// The slice holding partner-order position `q`: its partner block, and
    /// `q`'s slot within it.
    fn slice_at(&self, q: u64) -> (u32, u64) {
        let slices = self.slices();
        let i = slices.partition_point(|s| s.start as u64 <= q) - 1;
        let s = slices[i];
        (s.block, q - s.start as u64)
    }

    /// The start and length of partner block `b`'s slice, if the cell has
    /// one.
    fn slice_of(&self, b: u32) -> Option<(u64, u64)> {
        let slices = self.slices();
        let real = &slices[..slices.len() - 1];
        let i = real.binary_search_by_key(&b, |s| s.block).ok()?;
        let start = slices[i].start;
        Some((start as u64, (slices[i + 1].start - start) as u64))
    }
}

/// A cell's plans with its block's arenas.
#[derive(Clone, Copy)]
struct PlanView<'a> {
    p: &'a PlanHead,
    l: &'a BlockLayout,
}

impl std::ops::Deref for PlanView<'_> {
    type Target = PlanHead;
    fn deref(&self) -> &PlanHead {
        self.p
    }
}

impl<'a> PlanView<'a> {
    /// Leaves, then the sentinel.
    fn leaves(&self) -> &'a [Leaf] {
        let a = self.p.leaves as usize;
        &self.l.leaf_arena[a..a + self.p.n_leaves as usize + 1]
    }

    /// Arrival data per leaf (empty unless an arrival cell).
    fn arrivals(&self) -> &'a [(u8, u32)] {
        if self.p.arrivals == u32::MAX {
            return &[];
        }
        let a = self.p.arrivals as usize;
        &self.l.arrival_arena[a..a + self.p.n_leaves as usize]
    }

    fn perm(&self) -> CompactPerm {
        CompactPerm::from_parts(self.p.total as u64, self.p.perm)
    }

    /// The leaf holding plan-order position `pos`.
    fn leaf_at(&self, pos: u64) -> usize {
        self.leaves().partition_point(|l| l.start() <= pos) - 1
    }

    /// `(leaf index, leaf, count)` for every leaf, in order.
    fn counted(&self) -> impl Iterator<Item = (usize, Leaf, u64)> + 'a {
        self.leaves()
            .windows(2)
            .enumerate()
            .map(|(li, w)| (li, w[0], w[1].start() - w[0].start()))
    }
}

#[derive(Clone, Debug)]
struct BlockLayout {
    f_cells: Vec<CellLayout>,
    m_cells: Vec<CellLayout>,
    /// Cell keys (see [`cell_key`]), per sex, packed for cache-friendly
    /// searches (cells are ordered by `(year, kind, class)`).
    f_keys: Coarse<i32>,
    m_keys: Coarse<i32>,
    /// Entry cohorts (natives first) and their raw starts, plus the total.
    cohorts: Vec<CohortLayout>,
    cohort_starts: Vec<u64>,
    /// Natives: the first raw ids, and the parent line's length.
    natives: u64,
    /// Parent line: the start of each mother-age slot, plus the total.
    /// Every mother is in the child's region and aged [`MIN_BIRTH_AGE`] to
    /// [`MAX_BIRTH_AGE`] at the birth, so her block follows from her age:
    /// slot `k` holds mothers aged `MAX_BIRTH_AGE - k`, i.e. mother blocks
    /// in ascending order, as the ledger lists them. Inline, so finding a
    /// mother's range is one index, not a search.
    parent_starts: [u64; PARENT_SLOTS + 1],
    /// Births to the block's women by mother's age (rows, from
    /// [`MIN_BIRTH_AGE`]) and union cell group (columns: a `(year, kind)`,
    /// every class): row `a` holds the cumulative count of union births at
    /// age `a` over columns `0..col`, plus the row total in the last column.
    /// A birth's place in the parent line is a row lookup plus a scan of
    /// one column's leaves (its class-cells' leaves are contiguous).
    birth_rows: Vec<u32>,
    /// Every [`COARSE`]th entry of each birth row (padded with `u32::MAX`),
    /// for the column search.
    birth_coarse: Vec<u32>,
    /// Each column's first plan, then the end.
    columns: Vec<u32>,
    /// Plans of the women's cells with plans, in cell order.
    plans: Vec<PlanHead>,
    /// Arenas of the cells' and plans' arrays.
    slice_arena: Vec<Slice>,
    part_arena: Vec<(u16, u32, u32)>,
    free_arena: Vec<u32>,
    leaf_arena: Vec<Leaf>,
    arrival_arena: Vec<(u8, u32)>,
    /// In-world non-union births `(count, cohort, leaf, k)` grouped by
    /// mother's age, in canonical (cohort, leaf, k) order;
    /// `nu_age_starts[a - MIN_BIRTH_AGE]` indexes the first of age `a`.
    nu_events: Vec<(u64, u16, u16, u8)>,
    nu_age_starts: Vec<u32>,
    /// The natives' parent-line permutation, built once.
    parent_perm: CompactPerm,
    /// Divorced sources per sex (R1c), their parts, and each cell's source
    /// (`u32::MAX`: none), per sex.
    sources: [Vec<SourceLayout>; 2],
    source_arena: Vec<SourcePart>,
    source_of: [Vec<u32>; 2],
}

/// One entry cohort's life line within its block.
#[derive(Clone, Debug)]
struct CohortLayout {
    /// First raw id of the cohort in its block.
    raw_start: u64,
    females: u64,
    /// Arrival year (`None`: natives).
    arrival: Option<i32>,
    /// Life-line permutation over the cohort's raw ids.
    life_perm: CompactPerm,
    /// Per sex (`Sex as usize`): the cohort's sub-cells in year order, then
    /// a sentinel (see [`Sub`]).
    subs: [Vec<Sub>; 2],
    /// The parts' starts on each line, coarse-indexed for the search.
    sub_starts: [Coarse<u32>; 2],
    /// The women's non-union plan partition and its permutation.
    nonunion: Vec<NonUnionLeaf>,
    nu_starts: Vec<u64>,
    nonunion_perm: CompactPerm,
    /// Women in the leading leaf without non-union births (most of them),
    /// inline, so their lookups skip the leaf arrays.
    nu_none: u64,
    /// Children who arrived with their parents: their parent line.
    parents: Option<Box<ParentLine>>,
}

/// Arenas of a block's cell and plan arrays, filled while building its
/// layout.
#[derive(Default)]
struct Arenas {
    slices: Vec<Slice>,
    parts: Vec<(u16, u32, u32)>,
    free: Vec<u32>,
    leaves: Vec<Leaf>,
    arrivals: Vec<(u8, u32)>,
    plans: Vec<PlanHead>,
}

/// A parent line: mother-age slots (see `BlockLayout::parent_starts`) and
/// the permutation from raw ids to positions.
#[derive(Clone, Debug)]
struct ParentLine {
    starts: [u64; PARENT_SLOTS + 1],
    perm: CompactPerm,
}

/// Sort key of a union cell: year, then kind, then dissolution class.
fn cell_key(year: i32, kind: CellKind, class: u8) -> i32 {
    ((8 * year + kind as i32) << 6) | class as i32
}

impl CellLayout {
    /// Members.
    fn total(&self) -> u64 {
        self.size as u64
    }

    /// The union year (the arrival year, for couples who arrived together).
    fn year(&self) -> i32 {
        self.yr as i32
    }

    /// True if this cell's members take the women's role in coupling: the
    /// women of opposite-sex cells, and the left side of same-sex cells.
    fn left(&self) -> bool {
        match self.kind {
            CellKind::SameLeft => true,
            CellKind::SameRight => false,
            _ => self.sex == Sex::Female,
        }
    }

    /// Sex and kind of the partners' cells.
    fn partner_cell(&self) -> (Sex, CellKind) {
        partner_cell(self.sex, self.kind)
    }
}

/// A right-role cell's free couple: `(year, kind, class, left block)` (see
/// `World::free_marks`).
type FreeMark = (i32, CellKind, u8, u32);

/// Sex and kind of the partners' cells for a cell of `sex` and `kind`.
fn partner_cell(sex: Sex, kind: CellKind) -> (Sex, CellKind) {
    if kind.same_sex() {
        (sex, kind.partner())
    } else {
        (opposite(sex), kind.partner())
    }
}

/// Entries in a survival or residual table: ages `0..=MAX_AGE`, plus one.
const TABLE: usize = MAX_AGE as usize + 2;

/// Mother-age slots on a parent line (see `BlockLayout::parent_starts`).
const PARENT_SLOTS: usize = (MAX_BIRTH_AGE - MIN_BIRTH_AGE + 1) as usize;

/// One kind of birth event on a mother block's side.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum EventKind {
    Union {
        cell: u16,
        leaf: u16,
        k: u8,
    },
    NonUnion {
        cohort: u16,
        leaf: u16,
        k: u8,
    },
    /// A child born abroad to an arriving couple: the mother's arrival cell
    /// and plan leaf (at most one such child per arrival age).
    Arrival {
        cell: u16,
        leaf: u16,
    },
}

/// Where a person sits on their parent line: `(mother block, position
/// within that block's range, arrival year)`, the arrival year being `Some`
/// for a child who arrived with the parents (see [`World::parent_line_pos`]).
type ParentPos = (u32, u64, Option<i32>);

/// A person's seat in a union cell: the block, sex, cell and in-cell
/// index (R1c: a person can hold two, a first and a second union).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Seat {
    block: u32,
    sex: Sex,
    ci: usize,
    i: u64,
}

/// A divorced source's layout (R1c): its first-union class-cell, the
/// remarriage order of its members (a keyed permutation of the cell's index
/// space), and its parts in that order, in the block's source arena.
#[derive(Clone, Copy, Debug)]
struct SourceLayout {
    perm: CompactParts,
    members: u32,
    cell: u32,
    parts: u32,
    n_parts: u16,
}

/// One remarriage part of a source: the second-union cell, the part's start
/// in the source's remarriage order, its size, and its start in the cell.
#[derive(Clone, Copy, Debug)]
struct SourcePart {
    cell: u32,
    start: u32,
    count: u32,
    at: u32,
}

/// A person's position on their cohort's life line.
#[derive(Clone, Copy, Debug)]
struct LifePos {
    block: u32,
    cohort: u16,
    sex: Sex,
    /// Offset within the cohort's sex line.
    offset: u64,
}

/// The world: the ledger plus per-block structure derived from it (life-line
/// layouts, birth tables, life tables), all computed once at build
/// time. Queries are pure lookups: there are no caches, so there is no
/// cold/warm difference and nothing that could change an answer.
pub struct World {
    ledger: Ledger,
    key: Key,
    /// Id → block index: bucket `id >> bucket_shift` spans blocks
    /// `bucket_block[i]..=bucket_block[i + 1]`, so most ids resolve with
    /// no search (see [`Self::decode`]).
    bucket_shift: u32,
    bucket_block: Vec<u32>,
    layouts: Vec<BlockLayout>,
    /// Survival tables `l[0..=MAX_AGE + 1]`, one row per (block, sex),
    /// flat: row `2 * block + sex`.
    lifetables: Vec<f64>,
    /// The natives' cumulative residual death tables, same rows.
    residuals: Vec<f64>,
    /// Coarse indexes of both, [`COARSE_LEN`] entries per row.
    lifetable_coarse: Vec<f64>,
    residual_coarse: Vec<f64>,
    /// Name sampling tables, built lazily (N1).
    name_tables: crate::names::NameTables,
}

impl World {
    /// Build a world from parameters and a seed.
    pub fn build(params: Params, seed: u64) -> Self {
        let ledger = Ledger::build(params, seed);
        let n = ledger.blocks.len() as u32;
        let (bucket_shift, bucket_block) = Self::build_buckets(&ledger.base);
        let mut w = Self {
            ledger,
            key: Key::from_seed(seed),
            bucket_shift,
            bucket_block,
            layouts: Vec::new(),
            lifetables: Vec::new(),
            residuals: Vec::new(),
            lifetable_coarse: Vec::new(),
            residual_coarse: Vec::new(),
            name_tables: crate::names::NameTables::default(),
        };
        w.name_tables = crate::names::NameTables::new(&w.ledger.params.name_data);
        // Each block's structures are a pure function of the ledger, so
        // they are built on every core.
        let marks = Self::free_marks(&w.ledger);
        w.layouts = (0..n)
            .into_par_iter()
            .map(|b| w.build_layout(b, &marks[b as usize]))
            .collect();
        let tables: Vec<[Vec<f64>; 2]> = (0..n)
            .into_par_iter()
            .map(|b| w.build_lifetable(b))
            .collect();
        w.lifetables = tables.into_iter().flatten().flatten().collect();
        let residuals: Vec<[Vec<f64>; 2]> = (0..n)
            .into_par_iter()
            .map(|b| w.build_residual(b))
            .collect();
        w.residuals = residuals.into_iter().flatten().flatten().collect();
        assert_eq!(
            w.lifetables.len(),
            2 * n as usize * TABLE,
            "life table rows"
        );
        assert_eq!(w.residuals.len(), 2 * n as usize * TABLE, "residual rows");
        w.lifetable_coarse = w.lifetables.chunks(TABLE).flat_map(coarse_index).collect();
        w.residual_coarse = w.residuals.chunks(TABLE).flat_map(coarse_index).collect();
        w
    }

    /// The world's root key (for views keyed on people, such as
    /// households).
    /// The world's name sampling tables (N1).
    pub(crate) fn name_tables(&self) -> &crate::names::NameTables {
        &self.name_tables
    }

    pub(crate) fn key(&self) -> Key {
        self.key
    }

    /// The ledger this world reads.
    pub fn ledger(&self) -> &Ledger {
        &self.ledger
    }

    /// Everyone ever born in the world.
    pub fn population(&self) -> u64 {
        self.ledger.population()
    }

    // --- ids ------------------------------------------------------------------

    /// Buckets of `2^shift` ids, about four per block, each mapped to the
    /// first block it overlaps; one extra entry closes the last bucket.
    fn build_buckets(base: &[u64]) -> (u32, Vec<u32>) {
        let blocks = base.len() - 1;
        let pop = base[blocks].max(1);
        let target = (4 * blocks as u64).next_power_of_two().max(1);
        let mut shift = 0;
        while (pop - 1) >> shift >= target {
            shift += 1;
        }
        let buckets = (((pop - 1) >> shift) + 1) as usize;
        let block_of = |id: u64| (base.partition_point(|&s| s <= id) - 1).min(blocks - 1) as u32;
        let mut table: Vec<u32> = (0..buckets)
            .map(|i| block_of((i as u64) << shift))
            .collect();
        table.push(block_of(pop - 1));
        (shift, table)
    }

    /// `(block, raw)` for an id.
    pub(crate) fn decode(&self, id: PersonId) -> (u32, u64) {
        let id = id as u64;
        assert!(id < self.population(), "person id {id} out of range");
        let i = (id >> self.bucket_shift) as usize;
        let (lo, hi) = (
            self.bucket_block[i] as usize,
            self.bucket_block[i + 1] as usize,
        );
        let b = if lo == hi {
            lo
        } else {
            lo + self.ledger.base[lo + 1..=hi].partition_point(|&s| s <= id)
        };
        (b as u32, id - self.ledger.base[b])
    }

    pub(crate) fn id_of(&self, block: u32, raw: u64) -> PersonId {
        (self.ledger.base[block as usize] + raw) as PersonId
    }

    fn block(&self, b: u32) -> &Block {
        &self.ledger.blocks[b as usize]
    }

    // --- layouts ----------------------------------------------------------------

    fn layout(&self, b: u32) -> &BlockLayout {
        &self.layouts[b as usize]
    }

    /// For each block and sex, the right-role cells' couples whose left
    /// partner is alone in her cell, as `(year, kind, class, left block)`,
    /// sorted: one pass over every left-role cell with one member, so the
    /// layouts need no search into other blocks.
    fn free_marks(ledger: &Ledger) -> Vec<[Vec<FreeMark>; 2]> {
        let mut marks: Vec<[Vec<FreeMark>; 2]> =
            vec![[Vec::new(), Vec::new()]; ledger.blocks.len()];
        for (wb, block) in ledger.blocks.iter().enumerate() {
            for (sex, cells) in [(Sex::Female, &block.union_f), (Sex::Male, &block.union_m)] {
                for c in cells {
                    let left = match c.kind {
                        CellKind::SameLeft => true,
                        CellKind::SameRight => false,
                        _ => sex == Sex::Female,
                    };
                    if !left || c.total != 1 {
                        continue;
                    }
                    let (rs, rk) = partner_cell(sex, c.kind);
                    let rb = c.partners[0].0;
                    marks[rb as usize][rs as usize].push((c.year, rk, c.class, wb as u32));
                }
            }
        }
        for m in marks.iter_mut().flatten() {
            m.sort_unstable();
        }
        marks
    }

    fn build_layout(&self, b: u32, free_marks: &[Vec<FreeMark>; 2]) -> BlockLayout {
        let block = self.block(b);
        let tables = &self.ledger.plan_tables;
        let mut ar = Arenas::default();
        let cells = |src: &[crate::ledger::UnionCell], sex: Sex, ar: &mut Arenas| {
            let mut out = Vec::with_capacity(src.len());
            for (ci, c) in src.iter().enumerate() {
                assert!(c.total < 1 << 32, "union cell too large");
                let total = c.total as u32;
                let slices = ar.slices.len() as u32;
                let mut acc = 0u32;
                for &(block, n) in &c.partners {
                    ar.slices.push(Slice { block, start: acc });
                    acc += n as u32;
                }
                ar.slices.push(Slice {
                    block: u32::MAX,
                    start: acc,
                });
                let with_plans = sex == Sex::Female && !c.kind.same_sex();
                let age = c.year - block.year;
                let s = if sex == Sex::Female { 0 } else { 1 };
                // Each kind of cell keys its permutations apart.
                let tag = |t: u64| t ^ (c.kind as u64).wrapping_mul(TAG_ARRIVAL);
                let plan = if !with_plans {
                    NO_PLAN
                } else {
                    let pk = plan_key(self.ledger.plan_root, b, c.year, c.kind, c.class);
                    let leaves = ar.leaves.len() as u32;
                    let mut at = 0u64;
                    let arrivals = if c.kind == CellKind::Arrival {
                        let off = ar.arrivals.len() as u32;
                        let dens = self.ledger.arrival_density(c.year);
                        let p = &self.ledger.params;
                        let mca = p.immigration.min_couple_age;
                        let h = block.heritage;
                        for leaf in arrival_plans(c.total, c.year, age, pk, dens, tables, h, mca) {
                            let (in_world, kids) = arrival_births(&leaf, age, c.class, p);
                            ar.leaves.push(Leaf::pack(at, in_world));
                            ar.arrivals.push((leaf.d, kids));
                            at += leaf.plan.count;
                        }
                        off
                    } else {
                        let mut plans = Vec::new();
                        tables.at(c.year, block.heritage).plans_into(
                            c.total,
                            c.kind.second(),
                            pk,
                            &mut plans,
                        );
                        for leaf in &plans {
                            let (offs, n) = leaf_births(
                                leaf,
                                age,
                                cutoff(c.class),
                                &self.ledger.params.fertility,
                            );
                            let mask = offs[..n].iter().fold(0u32, |m, &o| m | 1 << o);
                            ar.leaves.push(Leaf::pack(at, mask));
                            at += leaf.count;
                        }
                        u32::MAX
                    };
                    assert_eq!(at, c.total, "plans cover the cell");
                    let n_leaves = ar.leaves.len() as u32 - leaves;
                    ar.leaves.push(Leaf::pack(at, 0));
                    ar.plans.push(PlanHead {
                        perm: CompactPerm::new(
                            c.total,
                            self.key.with3(
                                tag(TAG_PLAN),
                                b as u64,
                                (c.year as i64 as u64) << 8 | c.class as u64,
                            ),
                        )
                        .parts(),
                        total,
                        leaves,
                        n_leaves: u16::try_from(n_leaves).expect("leaves per cell fit u16"),
                        cell: ci as u16,
                        column: 0,
                        arrivals,
                    });
                    u16::try_from(ar.plans.len() - 1).expect("fewer than 65535 plan cells")
                };
                // Right-role cells list their free couples: those whose
                // left partner is alone in her cell.
                let right = match c.kind {
                    CellKind::SameLeft => false,
                    CellKind::SameRight => true,
                    _ => sex == Sex::Male,
                };
                let free = ar.free.len() as u32;
                if right {
                    let marks = &free_marks[s as usize];
                    let key = (c.year, c.kind, c.class);
                    let from = marks.partition_point(|m| (m.0, m.1, m.2) < key);
                    let to = from + marks[from..].partition_point(|m| (m.0, m.1, m.2) == key);
                    let alone = &marks[from..to];
                    for (&(wb, n), sl) in c.partners.iter().zip(&ar.slices[slices as usize..]) {
                        if alone.iter().any(|m| m.3 == wb) {
                            debug_assert_eq!(n, 1, "a lone partner's slice holds one couple");
                            ar.free.push(sl.start);
                        }
                    }
                }
                // Parts: entry cohorts for first unions, divorced sources for
                // second unions (R1c). Line starts are filled in once the
                // cohorts and sources are laid out.
                let parts = ar.parts.len() as u32;
                let mut acc = 0u32;
                let members: Vec<(u16, u64)> = if c.kind.second() {
                    c.sources.iter().map(|&(s, n)| (s as u16, n)).collect()
                } else {
                    c.cohorts.clone()
                };
                for &(co, n) in &members {
                    ar.parts.push((co, acc, 0));
                    acc += n as u32;
                }
                out.push(CellLayout {
                    perm: CompactPerm::new(
                        c.total,
                        self.key.with3(
                            tag(TAG_PARTNER),
                            b as u64,
                            (((c.year as i64 as u64) << 1) | s) << 8 | c.class as u64,
                        ),
                    )
                    .parts(),
                    size: total,
                    slices,
                    n_slices: u16::try_from(c.partners.len()).expect("slices per cell fit u16"),
                    plan,
                    parts,
                    n_parts: members.len() as u16,
                    first_cohort: 0,
                    first_line: 0,
                    first_end: 0,
                    free,
                    n_free: (ar.free.len() as u32 - free) as u16,
                    yr: i16::try_from(c.year).expect("union years fit i16"),
                    kind: c.kind,
                    sex,
                    class: c.class,
                });
            }
            out
        };
        let mut f_cells = cells(&block.union_f, Sex::Female, &mut ar);
        let mut m_cells = cells(&block.union_m, Sex::Male, &mut ar);
        // Entry cohorts: raw ranges, life lines and non-union plans. Each
        // cohort's sex line is its sub-cells in year order, then the rest.
        let n_co = block.cohorts.len();
        let mut subs: Vec<[Vec<Sub>; 2]> = vec![[Vec::new(), Vec::new()]; n_co];
        let mut lines: Vec<[u32; 2]> = vec![[0, 0]; n_co];
        for (si, cells) in [&mut f_cells, &mut m_cells].into_iter().enumerate() {
            for (ci, cell) in cells.iter_mut().enumerate() {
                if cell.kind.second() {
                    continue;
                }
                let total = cell.size;
                let parts =
                    &mut ar.parts[cell.parts as usize..(cell.parts + cell.n_parts as u32) as usize];
                for k in 0..parts.len() {
                    let (co, start, _) = parts[k];
                    let end = parts.get(k + 1).map_or(total, |x| x.1);
                    let line = lines[co as usize][si];
                    parts[k].2 = line;
                    subs[co as usize][si].push(Sub {
                        start: line,
                        in_cell: start,
                        ci: ci as u16,
                        year: cell.yr,
                        divorces: cell.class > 0 && cell.kind.feeds_divorced(),
                    });
                    lines[co as usize][si] = line + (end - start);
                }
                (cell.first_cohort, cell.first_line) = (parts[0].0, parts[0].2);
                cell.first_end = parts.get(1).map_or(total, |x| x.1);
            }
        }
        // Divorced sources (R1c): each first-union class-cell that separates,
        // its members' remarriage order, and its parts; then the second-union
        // cells' parts point back into them.
        let mut sources: [Vec<SourceLayout>; 2] = [Vec::new(), Vec::new()];
        let mut source_arena: Vec<SourcePart> = Vec::new();
        let mut source_of = [vec![u32::MAX; f_cells.len()], vec![u32::MAX; m_cells.len()]];
        for (si, (divs, cells)) in [(&block.div_f, &f_cells), (&block.div_m, &m_cells)]
            .into_iter()
            .enumerate()
        {
            let find = |year: i32, kind: CellKind, class: u8| -> usize {
                cells
                    .binary_search_by_key(&(year, kind, class), |c| (c.year(), c.kind, c.class))
                    .expect("a source's cells are recorded")
            };
            for (k, d) in divs.iter().enumerate() {
                let ci = find(d.year, d.kind, d.class);
                source_of[si][ci] = k as u32;
                let parts = source_arena.len() as u32;
                let mut start = 0u32;
                for &(y2, k2, c2, n) in &d.parts {
                    source_arena.push(SourcePart {
                        cell: find(y2, k2, c2) as u32,
                        start,
                        count: n as u32,
                        at: 0,
                    });
                    start += n as u32;
                }
                sources[si].push(SourceLayout {
                    perm: CompactPerm::new(
                        d.members,
                        self.key
                            .with3(TAG_REMARRY, b as u64, (si as u64) << 32 | k as u64),
                    )
                    .parts(),
                    members: d.members as u32,
                    cell: ci as u32,
                    parts,
                    n_parts: d.parts.len() as u16,
                });
            }
        }
        for (si, cells) in [&mut f_cells, &mut m_cells].into_iter().enumerate() {
            for (ci, cell) in cells.iter_mut().enumerate() {
                if !cell.kind.second() {
                    continue;
                }
                let total = cell.size;
                let parts =
                    &mut ar.parts[cell.parts as usize..(cell.parts + cell.n_parts as u32) as usize];
                for k in 0..parts.len() {
                    let (src, at, _) = parts[k];
                    let sl = sources[si][src as usize];
                    let sp = source_arena
                        [sl.parts as usize..sl.parts as usize + sl.n_parts as usize]
                        .iter_mut()
                        .find(|p| p.cell == ci as u32)
                        .expect("a second-union cell's source records it");
                    sp.at = at;
                    parts[k].2 = sp.start;
                }
                (cell.first_cohort, cell.first_line) = (parts[0].0, parts[0].2);
                cell.first_end = parts.get(1).map_or(total, |x| x.1);
            }
        }
        for (subs, lines) in subs.iter_mut().zip(&lines) {
            for (line, &total) in subs.iter_mut().zip(lines) {
                line.push(Sub {
                    start: total,
                    in_cell: 0,
                    ci: u16::MAX,
                    year: 0,
                    divorces: false,
                });
                line.shrink_to_fit();
            }
        }
        let mut cohorts = Vec::with_capacity(n_co);
        let mut cohort_starts = Vec::with_capacity(n_co + 1);
        let mut raw = 0u64;
        for (c, (co, subs)) in block.cohorts.iter().zip(subs).enumerate() {
            let p = &self.ledger.params;
            let factor = p.heritage.fertility_factor(block.heritage, block.year + 25);
            let nonunion = nonunion_plans(co.females, block.year, &p.fertility, factor);
            let mut nu_starts = Vec::with_capacity(nonunion.len() + 1);
            let mut acc = 0;
            for leaf in &nonunion {
                nu_starts.push(acc);
                acc += leaf.count;
            }
            nu_starts.push(acc);
            // Natives keep the block-level keys; arrival cohorts add theirs.
            let key = |tag: u64| {
                if c == 0 {
                    self.key.with2(tag, b as u64)
                } else {
                    self.key.with3(tag, b as u64, c as u64)
                }
            };
            cohort_starts.push(raw);
            cohorts.push(CohortLayout {
                raw_start: raw,
                females: co.females,
                arrival: co.arrival,
                life_perm: CompactPerm::new(co.size, key(TAG_LIFE)),
                sub_starts: [0, 1].map(|x| Coarse::new(subs[x].iter().map(|s| s.start).collect())),
                subs,
                nu_none: nonunion
                    .first()
                    .filter(|l| l.births == 0)
                    .map_or(0, |l| l.count),
                nonunion,
                nu_starts,
                nonunion_perm: CompactPerm::new(co.females, key(TAG_NONUNION)),
                parents: (!co.mothers.is_empty()).then(|| {
                    Box::new(ParentLine {
                        starts: self.parent_starts(block, &co.mothers),
                        perm: CompactPerm::new(co.size, key(TAG_PARENT)),
                    })
                }),
            });
            raw += co.size;
        }
        cohort_starts.push(raw);
        let parent_starts = self.parent_starts(block, &block.mothers);
        // Birth rows: cumulative union births per mother's age over the
        // columns, one per plan cell. (Grouping a year's class-cells into one
        // column made every birth lookup scan all their leaves: mother and
        // siblings ran 11% and 25% slower. A coarse index keeps the wider
        // rows' search short instead.)
        let mut columns: Vec<u32> = (0..=ar.plans.len() as u32).collect();
        for (pi, plan) in ar.plans.iter_mut().enumerate() {
            plan.column = pi as u16;
        }
        columns.shrink_to_fit();
        let n_cols = columns.len() - 1;
        let stride = n_cols + 1;
        let ages = (MAX_BIRTH_AGE - MIN_BIRTH_AGE + 1) as usize;
        // Each column's births by mother's age, from its leaves' offsets;
        // then each age's row is the running sum over the columns.
        let mut births = vec![0u64; ages * stride];
        for col in 0..n_cols {
            for plan in &ar.plans[columns[col] as usize..columns[col + 1] as usize] {
                let first_ai = f_cells[plan.cell as usize].year() - block.year - MIN_BIRTH_AGE;
                let leaves = &ar.leaves
                    [plan.leaves as usize..plan.leaves as usize + plan.n_leaves as usize + 1];
                for w in leaves.windows(2) {
                    let count = w[1].start() - w[0].start();
                    let mut mask = w[0].mask();
                    while mask != 0 {
                        let ai = first_ai + mask.trailing_zeros() as i32;
                        mask &= mask - 1;
                        if (0..ages as i32).contains(&ai) {
                            births[ai as usize * stride + col] += count;
                        }
                    }
                }
            }
        }
        let mut birth_rows = vec![0u32; ages * stride];
        for (row, counts) in birth_rows
            .chunks_exact_mut(stride)
            .zip(births.chunks_exact(stride))
        {
            let mut acc = 0u64;
            for (r, &c) in row.iter_mut().zip(&counts[..n_cols]) {
                *r = acc as u32;
                acc += c;
            }
            assert!(acc < u32::MAX as u64, "birth row overflow");
            row[n_cols] = acc as u32;
        }
        let birth_coarse: Vec<u32> = birth_rows
            .chunks(stride)
            .flat_map(|row| {
                let mut c: Vec<u32> = row.iter().step_by(COARSE).copied().collect();
                c.resize(stride.div_ceil(COARSE), u32::MAX);
                c
            })
            .collect();
        // In-world non-union births grouped by age, in (cohort, leaf, k)
        // order within an age. Births before a cohort's first in-world year
        // (abroad, or before the world starts) are not people.
        let mut nu_events: Vec<(u8, u64, u16, u16, u8)> = Vec::new();
        for (c, co) in cohorts.iter().enumerate() {
            let first = self.first_birth_year(b, co);
            for (li, leaf) in co.nonunion.iter().enumerate() {
                for k in 0..leaf.births as usize {
                    if block.year + leaf.ages[k] as i32 >= first {
                        nu_events.push((leaf.ages[k], leaf.count, c as u16, li as u16, k as u8));
                    }
                }
            }
        }
        nu_events.sort_unstable_by_key(|&(age, _, c, li, k)| (age, c, li, k));
        let nu_age_starts: Vec<u32> = (MIN_BIRTH_AGE..=MAX_BIRTH_AGE + 1)
            .map(|a| nu_events.partition_point(|e| (e.0 as i32) < a) as u32)
            .collect();
        let nu_events = nu_events
            .into_iter()
            .map(|(_, count, c, li, k)| (count, c, li, k))
            .collect();
        let natives = block.cohorts.first().map_or(0, |c| c.size);
        let Arenas {
            slices: slice_arena,
            parts: part_arena,
            free: free_arena,
            leaves: leaf_arena,
            arrivals: arrival_arena,
            plans,
        } = ar;
        BlockLayout {
            birth_rows,
            birth_coarse,
            columns,
            plans,
            slice_arena,
            part_arena,
            free_arena,
            leaf_arena,
            arrival_arena,
            nu_events,
            nu_age_starts,
            f_keys: Coarse::new(
                f_cells
                    .iter()
                    .map(|c| cell_key(c.year(), c.kind, c.class))
                    .collect(),
            ),
            m_keys: Coarse::new(
                m_cells
                    .iter()
                    .map(|c| cell_key(c.year(), c.kind, c.class))
                    .collect(),
            ),
            f_cells,
            m_cells,
            cohorts,
            cohort_starts,
            natives,
            parent_starts,
            parent_perm: CompactPerm::new(natives, self.key.with2(TAG_PARENT, b as u64)),
            sources,
            source_arena,
            source_of,
        }
    }

    /// Starts of the mother-age slots of a parent line whose mothers are
    /// `mothers` (in mother-block order). Every mother is in the child's
    /// lineage group and aged [`MIN_BIRTH_AGE`] to [`MAX_BIRTH_AGE`] at the birth.
    fn parent_starts(&self, block: &Block, mothers: &[MotherShare]) -> [u64; PARENT_SLOTS + 1] {
        let mut slot_births = [0u64; PARENT_SLOTS];
        for m in mothers {
            let mother = &self.ledger.blocks[m.mother as usize];
            assert_eq!(
                mother.group, block.group,
                "a child is in its mother's lineage group"
            );
            let age = block.year - mother.year;
            assert!(
                (MIN_BIRTH_AGE..=MAX_BIRTH_AGE).contains(&age),
                "mother aged {age}"
            );
            slot_births[(MAX_BIRTH_AGE - age) as usize] += m.union_births + m.nonunion_births;
        }
        let mut starts = [0u64; PARENT_SLOTS + 1];
        for k in 0..PARENT_SLOTS {
            starts[k + 1] = starts[k] + slot_births[k];
        }
        starts
    }

    /// First calendar year a cohort's women can give birth in-world: after
    /// arrival, after the world starts (founders), or after their birth.
    fn first_birth_year(&self, b: u32, co: &CohortLayout) -> i32 {
        match co.arrival {
            Some(t) => t + 1,
            None if self.block(b).founder => self.ledger.params.y0 + 1,
            None => self.block(b).year + 1,
        }
    }

    fn parent_perm(&self, b: u32) -> &CompactPerm {
        &self.layout(b).parent_perm
    }

    /// Number of natives in a block (the parent line's length).
    fn natives(layout: &BlockLayout) -> u64 {
        layout.natives
    }

    fn plans<'a>(layout: &'a BlockLayout, cell: &CellLayout) -> PlanView<'a> {
        PlanView {
            p: layout
                .plans
                .get(cell.plan as usize)
                .expect("women's opposite-sex cells carry plans"),
            l: layout,
        }
    }

    /// Plan `plan` of a block, if any.
    fn plan(layout: &BlockLayout, plan: u16) -> Option<PlanView<'_>> {
        Some(PlanView {
            p: layout.plans.get(plan as usize)?,
            l: layout,
        })
    }

    fn life_pos(&self, id: PersonId) -> LifePos {
        let (block, raw) = self.decode(id);
        let layout = self.layout(block);
        // Natives first: most ids need no cohort search.
        let cohort = if raw < Self::natives(layout) {
            0
        } else {
            layout.cohort_starts.partition_point(|&s| s <= raw) - 1
        };
        let co = &layout.cohorts[cohort];
        let rank = co.life_perm.fwd(raw - co.raw_start);
        let (sex, offset) = if rank < co.females {
            (Sex::Female, rank)
        } else {
            (Sex::Male, rank - co.females)
        };
        LifePos {
            block,
            cohort: cohort as u16,
            sex,
            offset,
        }
    }

    /// Raw id of the person at `offset` on a cohort's sex line.
    fn raw_of(&self, block: u32, cohort: u16, sex: Sex, offset: u64) -> u64 {
        let co = &self.layout(block).cohorts[cohort as usize];
        let rank = match sex {
            Sex::Female => offset,
            Sex::Male => co.females + offset,
        };
        co.raw_start + co.life_perm.inv(rank)
    }

    /// A partnered person's sub-cell and in-cell index.
    fn sub_of(layout: &BlockLayout, p: &LifePos) -> Option<(Sub, u64)> {
        let co = &layout.cohorts[p.cohort as usize];
        let starts = &co.sub_starts[p.sex as usize];
        if p.offset >= *starts.values.last()? as u64 {
            return None;
        }
        let s = co.subs[p.sex as usize][starts.count_le(p.offset as u32) - 1];
        Some((s, s.in_cell as u64 + (p.offset - s.start as u64)))
    }

    /// The seat, in the source's own union cell, of the member at place
    /// `pos` of source `src`'s remarriage order.
    fn seat_of_source(&self, block: u32, sex: Sex, src: usize, pos: u64) -> Seat {
        let sl = self.layout(block).sources[sex as usize][src];
        let i = CompactPerm::from_parts(sl.members as u64, sl.perm).inv(pos);
        Seat {
            block,
            sex,
            ci: sl.cell as usize,
            i,
        }
    }

    /// The seat of a person's next union, after the union at `seat`
    /// separates, if they re-partner (R1c, any order R1d).
    fn next_seat(&self, first: Seat) -> Option<Seat> {
        let layout = self.layout(first.block);
        let src = layout.source_of[first.sex as usize][first.ci];
        if src == u32::MAX {
            return None;
        }
        let sl = layout.sources[first.sex as usize][src as usize];
        let pos = CompactPerm::from_parts(sl.members as u64, sl.perm).fwd(first.i);
        let parts =
            &layout.source_arena[sl.parts as usize..sl.parts as usize + sl.n_parts as usize];
        let k = parts.partition_point(|p| p.start as u64 <= pos);
        let part = parts.get(k.checked_sub(1)?)?;
        (pos < (part.start + part.count) as u64).then(|| Seat {
            block: first.block,
            sex: first.sex,
            ci: part.cell as usize,
            i: part.at as u64 + (pos - part.start as u64),
        })
    }

    /// The person at in-cell index `i` of union cell `ci`.
    fn person_in_cell(&self, block: u32, sex: Sex, ci: usize, i: u64) -> PersonId {
        let layout = self.layout(block);
        let cell = Self::cell(layout, sex, ci);
        let (cohort, start, line) = if i < cell.first_end as u64 {
            (cell.first_cohort, 0, cell.first_line)
        } else {
            let parts = cell.parts();
            parts[parts.partition_point(|&(_, s, _)| s as u64 <= i) - 1]
        };
        let offset = line as u64 + (i - start as u64);
        if cell.kind.second() {
            // A re-partnering cell's members come from divorced sources:
            // `cohort` is the source, `offset` a place in its remarriage
            // order, which maps back to the member's seat in the previous
            // union (itself a re-partnering cell for a third union, and so
            // on).
            let prev = self.seat_of_source(block, sex, cohort as usize, offset);
            return self.person_in_cell(prev.block, sex, prev.ci, prev.i);
        }
        self.id_of(block, self.raw_of(block, cohort, sex, offset))
    }

    /// Index of the union cell of `year`, `kind` and `class`, if the block
    /// has one.
    fn cell_of(
        layout: &BlockLayout,
        sex: Sex,
        year: i32,
        kind: CellKind,
        class: u8,
    ) -> Option<usize> {
        Self::keys(layout, sex).find(cell_key(year, kind, class))
    }

    /// Indices of the union cells of `year` and `kind`, every class.
    fn cells_of(
        layout: &BlockLayout,
        sex: Sex,
        year: i32,
        kind: CellKind,
    ) -> std::ops::Range<usize> {
        let keys = Self::keys(layout, sex);
        let lo = cell_key(year, kind, 0);
        keys.count_lt(lo)..keys.count_lt(lo + 64)
    }

    fn keys(layout: &BlockLayout, sex: Sex) -> &Coarse<i32> {
        match sex {
            Sex::Female => &layout.f_keys,
            Sex::Male => &layout.m_keys,
        }
    }

    /// Union cell `ci` of `sex`, with its block's arenas.
    fn cell(layout: &BlockLayout, sex: Sex, ci: usize) -> CellView<'_> {
        CellView {
            c: &Self::cells(layout, sex)[ci],
            l: layout,
        }
    }

    fn cells(layout: &BlockLayout, sex: Sex) -> &[CellLayout] {
        match sex {
            Sex::Female => &layout.f_cells,
            Sex::Male => &layout.m_cells,
        }
    }

    // --- basic facts -----------------------------------------------------------

    /// Sex.
    pub fn sex(&self, id: PersonId) -> Sex {
        self.life_pos(id).sex
    }

    /// Birth year.
    pub fn birth_year(&self, id: PersonId) -> i32 {
        self.block(self.decode(id).0).year
    }

    /// Birth time, seconds since 1800 (a keyed day and second in the year).
    pub fn birth(&self, id: PersonId) -> i64 {
        let year = self.birth_year(id);
        let k = self.key.with2(TAG_BIRTHDAY, id as u64);
        year_start(year) + k.below(365) as i64 * DAY + k.with(1).below(DAY as u64) as i64
    }

    /// Lineage region (index into [`Params::regions`]): the mother's
    /// region, or the founder's or immigrant's.
    pub fn region(&self, id: PersonId) -> u16 {
        self.block(self.decode(id).0).region
    }

    /// Heritage group (N1): the mother's, or the founder's or immigrant's.
    pub fn heritage(&self, id: PersonId) -> Heritage {
        self.block(self.decode(id).0).heritage
    }

    /// Lineage group (region × heritage): the mother's, or the founder's or
    /// immigrant's. Block `(year, group)` holds the person.
    pub fn lineage_group(&self, id: PersonId) -> u16 {
        self.block(self.decode(id).0).group
    }

    /// True if born before the world starts and not an immigrant (no
    /// in-world parents).
    pub fn is_founder(&self, id: PersonId) -> bool {
        let (b, raw) = self.decode(id);
        self.block(b).founder && raw < Self::natives(self.layout(b))
    }

    /// True if born abroad (arrived as an immigrant).
    pub fn is_immigrant(&self, id: PersonId) -> bool {
        let (b, raw) = self.decode(id);
        raw >= Self::natives(self.layout(b))
    }

    /// Arrival time of an immigrant, seconds since 1800. A family arrives
    /// together: a child who arrived with the parents shares the mother's
    /// date, and an arriving couple the wife's. The date is keyed on the
    /// family and falls after the birth of any child born abroad in the
    /// arrival year. `None` for people born in-world and founders.
    pub fn arrival(&self, id: PersonId) -> Option<i64> {
        let p = self.life_pos(id);
        let layout = self.layout(p.block);
        let co = &layout.cohorts[p.cohort as usize];
        let t = co.arrival?;
        let woman = if co.parents.is_some() {
            Some(self.mother(id).expect("a child arrives with the parents"))
        } else {
            match self.first_seat(&p) {
                Some(seat) if Self::cells(layout, p.sex)[seat.ci].kind == CellKind::Arrival => {
                    match p.sex {
                        Sex::Female => Some(id),
                        Sex::Male => self.repaired_partner(seat, id).map(|x| x.0),
                    }
                }
                _ => None,
            }
        };
        let family = woman.unwrap_or(id);
        let mut lo = year_start(t);
        if let Some(w) = woman {
            let wp = self.life_pos(w);
            for (year, kind, off, start) in self.births_of(&wp).iter() {
                if year == t && matches!(kind, EventKind::Arrival { .. }) {
                    let child = self
                        .child_of_event(wp.block, year, kind, off, start)
                        .expect("an arriving child");
                    lo = lo.max(self.birth(child) + 1);
                }
            }
        }
        let span = (year_start(t + 1) - lo).max(1);
        let k = self.key.with2(TAG_ARRIVAL, family as u64);
        Some(lo + k.below(span as u64) as i64)
    }

    // --- partners ----------------------------------------------------------------

    /// The partner at `slot` of the slice `(my block, partner block)` in the
    /// union cell of `block`, `sex` and `(year, kind)`, without kin repair.
    fn slot_person(
        &self,
        block: u32,
        sex: Sex,
        (year, kind, class): (i32, CellKind, u8),
        partner_block: u32,
        slot: u64,
    ) -> Option<PersonId> {
        let layout = self.layout(block);
        let ci = Self::cell_of(layout, sex, year, kind, class)?;
        let cell = Self::cell(layout, sex, ci);
        let (start, len) = cell.slice_of(partner_block)?;
        if slot >= len {
            return None;
        }
        Some(self.person_in_cell(block, sex, ci, cell.perm().inv(start + slot)))
    }

    /// First partner without kin repair (used inside the repair predicate).
    fn partner_unrepaired(&self, seat: Seat) -> Option<PersonId> {
        let cell = Self::cell(self.layout(seat.block), seat.sex, seat.ci);
        let (pb, slot) = cell.slice_at(cell.perm().fwd(seat.i));
        let (psex, pkind) = cell.partner_cell();
        self.slot_person(pb, psex, (cell.year(), pkind, cell.class), seat.block, slot)
    }

    /// The default (unrepaired) fathers a child can have: for a union birth
    /// the mother's default partner in that union; for a non-union birth
    /// her default partner in any of her unions (R1c; conservative: the kin
    /// predicate treats any of them as the father).
    fn default_fathers(&self, c: PersonId, pc: ParentPos) -> [Option<PersonId>; MAX_UNIONS] {
        let (m, _, seat) = self.mother_event_at(c, pc);
        match seat {
            Some(s) => {
                let mut out = [None; MAX_UNIONS];
                out[0] = self.partner_unrepaired(s);
                out
            }
            None => self
                .seats(&self.life_pos(m))
                .map(|s| s.and_then(|s| self.partner_unrepaired(s))),
        }
    }

    /// For a union birth, from its birth-table column: whether the father
    /// re-partnered for that union (his second or a later one; R1c, R1d).
    /// `None` for other births.
    fn father_in_second(&self, (mb, _, _): ParentPos, column: Option<usize>) -> Option<bool> {
        let layout = self.layout(mb);
        let plan = &layout.plans[column?];
        Some(layout.f_cells[plan.cell as usize].kind.partner().second())
    }

    /// Where a person with an in-world mother sits on their parent line:
    /// `(mother block, position within that mother block's range, arrival
    /// year)`, the arrival year being `Some` for a child who arrived with
    /// the parents. `None` for founders and adult immigrants.
    fn parent_line_pos(&self, id: PersonId) -> Option<ParentPos> {
        let (b, raw) = self.decode(id);
        let layout = self.layout(b);
        let (starts, q, arrival) = if raw < Self::natives(layout) {
            if self.block(b).founder {
                return None;
            }
            (&layout.parent_starts, self.parent_perm(b).fwd(raw), None)
        } else {
            let c = layout.cohort_starts.partition_point(|&s| s <= raw) - 1;
            let co = &layout.cohorts[c];
            let pl = co.parents.as_ref()?;
            (&pl.starts, pl.perm.fwd(raw - co.raw_start), co.arrival)
        };
        let k = starts.partition_point(|&s| s <= q) - 1;
        let block = self.block(b);
        let age = MAX_BIRTH_AGE - k as i32;
        let mb = self
            .ledger
            .block_of(block.year - age, block.group)
            .expect("the mother's block");
        Some((mb, q - starts[k], arrival))
    }

    /// The arrival cohort of a block for arrival year `t`, if any (cohorts
    /// after the natives are in arrival-year order).
    fn cohort_by_arrival(layout: &BlockLayout, t: i32) -> Option<usize> {
        layout.cohorts[1..]
            .binary_search_by_key(&Some(t), |c| c.arrival)
            .ok()
            .map(|i| i + 1)
    }

    /// True if two people may not partner: siblings (shared mother), or
    /// parent and child either way. Fathers are looked up without repair, so
    /// the predicate never recurses (R1 plan D-R1.7). Symmetric in `a` and
    /// `b`, and correct for any sexes (same-sex couples use it too).
    ///
    /// Cheap necessary conditions run first: siblings share a mother block
    /// (few couples do), a mother is at least [`MIN_BIRTH_AGE`] birth years
    /// older than her child, and a father at least [`MIN_UNION_AGE`] (he
    /// partners at that age at the earliest and the child is born during the
    /// union). Almost no couple meets an age condition, so the full lookups
    /// rarely run.
    fn related(&self, a: PersonId, b: PersonId) -> bool {
        let (ya, yb) = (self.birth_year(a), self.birth_year(b));
        let (pa, pb) = (self.parent_line_pos(a), self.parent_line_pos(b));
        if let (Some(pa), Some(pb)) = (pa, pb) {
            if pa.0 == pb.0 && self.same_mother(a, pa, b, pb) {
                return true;
            }
        }
        // Parent `p` of child `c`: `c` has an in-world mother and was born
        // well after `p`.
        let gap = MIN_BIRTH_AGE.min(MIN_UNION_AGE);
        for (p, c, age, pc) in [(a, b, yb - ya, pb), (b, a, ya - yb, pa)] {
            let Some(pc) = pc else { continue };
            if age < gap {
                continue;
            }
            let lp = self.life_pos(p);
            let parent = match lp.sex {
                // A mother is in her child's mother block.
                Sex::Female => {
                    age >= MIN_BIRTH_AGE && lp.block == pc.0 && self.mother_event_at(c, pc).0 == p
                }
                // A father is the mother's default partner in one of his
                // unions, so that union's cell has a slice of her block.
                Sex::Male => {
                    age >= MIN_UNION_AGE
                        && self.seats(&lp).into_iter().flatten().any(|s| {
                            let cell = Self::cell(self.layout(s.block), s.sex, s.ci);
                            !cell.kind.same_sex() && cell.slice_of(pc.0).is_some()
                        })
                        && self.default_fathers(c, pc).contains(&Some(p))
                }
            };
            if parent {
                return true;
            }
        }
        // Paternal half-siblings (R1c, R1d): the same father with two
        // mothers. A man has one first union, so two union births can share
        // a father across mothers only if at least one comes from a union in
        // which he re-partnered (his second, third, ...).
        if let (Some(pa), Some(pb)) = (pa, pb) {
            let (fa, fb) = (
                self.father_in_second(pa, self.birth_cell(a, pa)),
                self.father_in_second(pb, self.birth_cell(b, pb)),
            );
            if matches!((fa, fb), (Some(x), Some(y)) if x || y) {
                let (da, db) = (
                    self.default_fathers(a, pa)[0],
                    self.default_fathers(b, pb)[0],
                );
                if da.is_some() && da == db {
                    return true;
                }
            }
        }
        false
    }

    /// True if two children whose mothers share a block have the same
    /// mother. Each woman has one union cell (R1), so union births and
    /// children born abroad whose mothers' cells differ have different
    /// mothers; only same-cell pairs and non-union births resolve their
    /// mothers. (Re-partnering, R1c, gives some women a second cell, and
    /// must revisit this.)
    fn same_mother(&self, a: PersonId, pa: ParentPos, b: PersonId, pb: ParentPos) -> bool {
        if let (Some(ca), Some(cb)) = (self.birth_cell(a, pa), self.birth_cell(b, pb)) {
            // A woman's first union is in one cell; a second union (R1c)
            // puts her in another, so only two first-union cells rule out a
            // shared mother.
            let layout = self.layout(pa.0);
            let second = |col: usize| {
                layout.f_cells[layout.plans[col].cell as usize]
                    .kind
                    .second()
            };
            if ca != cb && !second(ca) && !second(cb) {
                return false;
            }
        }
        self.mother_event_at(a, pa).0 == self.mother_event_at(b, pb).0
    }

    /// The mother's union cell group (her cells' birth-table column: one
    /// `(year, kind)`, every class) for a union birth, from the child's
    /// parent-line position; `None` for a non-union birth or a child born
    /// abroad. One birth-row search: no leaf scan and no permutation.
    fn birth_cell(&self, id: PersonId, (mb, e, arrival): ParentPos) -> Option<usize> {
        let layout = self.layout(mb);
        if arrival.is_some() {
            return None;
        }
        let row = Self::birth_row(layout, self.birth_year(id) - self.block(mb).year)?;
        if e >= *row.last()? as u64 {
            return None;
        }
        Some(Self::column_of(
            layout,
            self.birth_year(id) - self.block(mb).year,
            e,
        ))
    }

    /// Default couple at partner-order position `q` of a left-role union
    /// cell ("woman" and "man" name the roles; same-sex cells use them too),
    /// without repair. Ids the caller already knows are not recomputed.
    fn default_couple(
        &self,
        w_block: u32,
        (ci, cell): (usize, CellView<'_>),
        q: u64,
        woman: Option<PersonId>,
        man: Option<(PersonId, ManSlot)>,
    ) -> (PersonId, PersonId, ManSlot) {
        let woman =
            woman.unwrap_or_else(|| self.person_in_cell(w_block, cell.sex, ci, cell.perm().inv(q)));
        if let Some((man, ms)) = man {
            return (woman, man, ms);
        }
        // The man sits at the same slot of the mirror slice in his cell.
        let (mb, slot) = cell.slice_at(q);
        let (msex, mkind) = cell.partner_cell();
        let ml = self.layout(mb);
        let mci = Self::cell_of(ml, msex, cell.year(), mkind, cell.class)
            .expect("the ledger records every slice on both sides");
        let mc = Self::cell(ml, msex, mci);
        let (start, _) = mc
            .slice_of(w_block)
            .expect("the ledger records every slice on both sides");
        let ms = ManSlot {
            block: mb,
            sex: msex,
            ci: mci,
            pos: start + slot,
        };
        let man = self.person_in_cell(mb, msex, mci, mc.perm().inv(ms.pos));
        (woman, man, ms)
    }

    /// Kin repair for position `q` of a woman's union cell (R1 plan D-R1.7).
    ///
    /// Positions are repaired in small groups ([`repair_group`]): pairs
    /// `(2k, 2k + 1)`, and in a cell with an odd count the last three
    /// positions together, so no position is left alone unless the cell has
    /// a single couple. Within a group the default men are permuted among
    /// the women to minimise the number of related couples; ties keep the
    /// earliest permutation in a fixed order, identity first, so nothing
    /// moves unless it must. The women are in the same block and year, so a
    /// permutation never changes any (woman's block, man's block, year)
    /// count: ledger closure holds exactly. Whoever asks, the same predicate
    /// runs over the same people, so the choice is reciprocal.
    ///
    /// `woman` and `man` are the default couple at `q` when the caller
    /// already knows one of them.
    fn repair(
        &self,
        w_block: u32,
        (ci, cell): (usize, CellView<'_>),
        q: u64,
        woman: Option<PersonId>,
        man: Option<(PersonId, ManSlot)>,
    ) -> Repair {
        let (start, len) = repair_group(q, cell.total());
        let at = (q - start) as usize;
        let mut women = [0 as PersonId; 3];
        let mut men = [0 as PersonId; 3];
        let mut slots = [ManSlot::default(); 3];
        let mut wslots = [ManSlot::default(); 3];
        for g in 0..len as usize {
            let known = if g == at { (woman, man) } else { (None, None) };
            (women[g], men[g], slots[g]) =
                self.default_couple(w_block, (ci, cell), start + g as u64, known.0, known.1);
            wslots[g] = ManSlot {
                block: w_block,
                sex: cell.sex,
                ci,
                pos: start + g as u64,
            };
        }
        let perm = self.least_related(&women, &men, len as usize);
        Repair {
            women,
            men,
            slots,
            wslots,
            perm,
            at,
        }
    }

    /// The permutation of `k` men among `k` women (woman `a` takes man
    /// `perm[a]`) with the fewest related couples; ties keep the earliest in
    /// [`PERMS`], identity first.
    fn least_related(&self, women: &[PersonId; 3], men: &[PersonId; 3], k: usize) -> [u8; 3] {
        let mut perm = [0u8, 1, 2];
        // Common case: the default couples are all unrelated.
        if (0..k).all(|g| !self.related(women[g], men[g])) {
            return perm;
        }
        let mut rel = [[false; 3]; 3];
        for (a, row) in rel.iter_mut().enumerate().take(k) {
            for (b, r) in row.iter_mut().enumerate().take(k) {
                *r = self.related(women[a], men[b]);
            }
        }
        let mut best = usize::MAX;
        for cand in PERMS[k] {
            let cost = (0..k).filter(|&a| rel[a][cand[a] as usize]).count();
            if cost < best {
                best = cost;
                perm[..k].copy_from_slice(cand);
            }
        }
        perm
    }

    /// Men's-side kin repair, stage two: for the group of man-cell
    /// position `ms` ([`repair_group`] over the man's cell), if the group
    /// holds a free couple (a woman alone in her cell, whom the women's side
    /// could not repair). The group's women after the women's side are
    /// permuted among its men by [`Self::least_related`]. The men share a
    /// block and year, so counts are unchanged; groups without a free couple
    /// are left alone, so almost every query stops at one range check.
    /// `man` is at `ms`, and `woman` is his after the women's side.
    fn repair_men(
        &self,
        ms: ManSlot,
        man: PersonId,
        (woman, wslot): (PersonId, ManSlot),
    ) -> Option<Repair> {
        let mc = Self::cell(self.layout(ms.block), ms.sex, ms.ci);
        let (start, len) = repair_group(ms.pos, mc.total());
        let free = mc.free();
        let first_free = free.partition_point(|&f| (f as u64) < start);
        if free
            .get(first_free)
            .map_or(true, |&f| f as u64 >= start + len)
        {
            return None;
        }
        let at = (ms.pos - start) as usize;
        let mut women = [0 as PersonId; 3];
        let mut men = [0 as PersonId; 3];
        let mut slots = [ManSlot::default(); 3];
        let mut wslots = [ManSlot::default(); 3];
        for g in 0..len as usize {
            let pos = start + g as u64;
            slots[g] = ManSlot { pos, ..ms };
            if g == at {
                (women[g], men[g], wslots[g]) = (woman, man, wslot);
            } else {
                men[g] = self.person_in_cell(ms.block, ms.sex, ms.ci, mc.perm().inv(pos));
                (women[g], wslots[g]) = self.women_side_woman(slots[g], men[g]);
            }
        }
        let perm = self.least_related(&women, &men, len as usize);
        Some(Repair {
            women,
            men,
            slots,
            wslots,
            perm,
            at,
        })
    }

    /// The woman the man at `ms` has after the women's-side repair, and
    /// where she sits.
    fn women_side_woman(&self, ms: ManSlot, man: PersonId) -> (PersonId, ManSlot) {
        let mc = Self::cell(self.layout(ms.block), ms.sex, ms.ci);
        let (wb, slot) = mc.slice_at(ms.pos);
        let (wsex, wkind) = mc.partner_cell();
        let wl = self.layout(wb);
        let wci = Self::cell_of(wl, wsex, mc.year(), wkind, mc.class).expect("the woman's cell");
        let wc = Self::cell(wl, wsex, wci);
        let (start, _) = wc
            .slice_of(ms.block)
            .expect("the ledger records every slice on both sides");
        let q = start + slot;
        let r = self.repair(wb, (wci, wc), q, None, Some((man, ms)));
        let t = r.taker();
        (r.women[t], r.wslots[t])
    }

    /// The woman's union cell and her partner-order position, for any member
    /// of a couple: a left-role member directly, a right-role member through
    /// the mirror slice. For a right-role member, also their own slot.
    #[allow(clippy::type_complexity)]
    fn couple_position(
        &self,
        seat: Seat,
    ) -> Option<(u32, (usize, CellView<'_>), u64, Option<ManSlot>)> {
        let p = &seat;
        let layout = self.layout(p.block);
        let ci = seat.ci;
        let cell = Self::cell(layout, p.sex, ci);
        let pos = cell.perm().fwd(seat.i);
        if cell.left() {
            return Some((p.block, (ci, cell), pos, None));
        }
        let (wb, slot) = cell.slice_at(pos);
        let (wsex, wkind) = cell.partner_cell();
        let wl = self.layout(wb);
        let wci = Self::cell_of(wl, wsex, cell.year(), wkind, cell.class)?;
        let wc = Self::cell(wl, wsex, wci);
        let (start, _) = wc.slice_of(p.block)?;
        let ms = ManSlot {
            block: p.block,
            sex: p.sex,
            ci,
            pos,
        };
        Some((wb, (wci, wc), start + slot, Some(ms)))
    }

    /// The repaired partner of a person, if partnered.
    ///
    /// Two stages: the women's side ([`Self::repair`]) within the woman's
    /// cell, then the men's side ([`Self::repair_men`]) within the man's
    /// cell, which acts only on groups holding a free couple. Roles, not
    /// sexes: same-sex couples go through the same stages.
    fn repaired_partner(&self, seat: Seat, id: PersonId) -> Option<(PersonId, Seat)> {
        let (wb, cell, q, ms) = self.couple_position(seat)?;
        Some(match ms {
            None => {
                let r = self.repair(wb, cell, q, Some(id), None);
                let g = r.perm[r.at] as usize;
                match self.repair_men(r.slots[g], r.men[g], (id, r.wslots[r.at])) {
                    None => (r.men[g], self.seat_at(r.slots[g])),
                    Some(f) => {
                        let m = f.perm[f.at] as usize;
                        (f.men[m], self.seat_at(f.slots[m]))
                    }
                }
            }
            Some(ms) => {
                let r = self.repair(wb, cell, q, None, Some((id, ms)));
                let t = r.taker();
                match self.repair_men(r.slots[r.at], id, (r.women[t], r.wslots[t])) {
                    None => (r.women[t], self.seat_at(r.wslots[t])),
                    Some(f) => {
                        let t = f.taker();
                        (f.women[t], self.seat_at(f.wslots[t]))
                    }
                }
            }
        })
    }

    /// The seat at partner-order position `ms.pos` of a cell.
    fn seat_at(&self, ms: ManSlot) -> Seat {
        let cell = Self::cell(self.layout(ms.block), ms.sex, ms.ci);
        Seat {
            block: ms.block,
            sex: ms.sex,
            ci: ms.ci,
            i: cell.perm().inv(ms.pos),
        }
    }

    /// A person's first-union seat, from their cohort line.
    fn first_seat(&self, p: &LifePos) -> Option<Seat> {
        let (sub, i) = Self::sub_of(self.layout(p.block), p)?;
        Some(Seat {
            block: p.block,
            sex: p.sex,
            ci: sub.ci as usize,
            i,
        })
    }

    /// A person's union seats, in order: each after the previous one
    /// separates (R1c; any number, R1d).
    fn seats(&self, p: &LifePos) -> [Option<Seat>; MAX_UNIONS] {
        let mut out = [None; MAX_UNIONS];
        let mut seat = self.first_seat(p);
        for slot in out.iter_mut() {
            *slot = seat;
            seat = seat.and_then(|s| self.next_seat(s));
        }
        assert!(seat.is_none(), "more than {MAX_UNIONS} unions");
        out
    }

    /// The union cell of a partnered person.
    fn union_cell(&self, p: &LifePos) -> Option<CellView<'_>> {
        let seat = self.first_seat(p)?;
        Some(Self::cell(self.layout(p.block), p.sex, seat.ci))
    }

    /// The couple `id` belongs to, without the death lookups its end needs.
    fn couple(&self, seat: Seat, id: PersonId) -> Option<Couple> {
        let (partner, pseat) = self.repaired_partner(seat, id)?;
        let cell = Self::cell(self.layout(seat.block), seat.sex, seat.ci);
        // The left-role member (the woman, in opposite-sex couples) keys
        // the couple and holds its plan.
        let (left, lseat) = if cell.left() {
            (id, seat)
        } else {
            (partner, pseat)
        };
        let key = self.key.with2(TAG_COUPLE, left as u64);
        if cell.kind.same_sex() {
            // No plan, no births (R1): the start and the dissolution are
            // keyed draws, as no ledger count depends on them.
            let year = cell.year();
            let start = year_start(year) + key.below(360) as i64 * DAY;
            let pmf = self.ledger.params.dissolution.pmf(year);
            let u = key.with(3).unit() * pmf.iter().sum::<f64>();
            let mut acc = 0.0;
            let band = pmf
                .iter()
                .position(|&w| {
                    acc += w;
                    u < acc
                })
                .unwrap_or(0);
            let separation = DISSOLUTION_BANDS[band].map(|(lo, hi)| {
                let years = lo as i32 + key.with(1).below((hi - lo) as u64) as i32;
                year_start(year + years) + key.with(2).below(360) as i64 * DAY
            });
            return Some(Couple {
                partner,
                key,
                start,
                separation,
                seat: pseat,
            });
        }
        let (_, _, union_year) = self
            .plan_leaf_at(lseat)
            .expect("a partnered woman has a plan");
        let start = year_start(union_year) + key.below(360) as i64 * DAY;
        // The cell's dissolution class, which both partners' cells share:
        // separation in the class's year after the cell's year (the arrival
        // year, for couples who arrived together). Every planned birth falls
        // in an earlier year.
        let separation = (cell.class > 0).then(|| {
            year_start(cell.year() + cell.class as i32) + key.with(2).below(360) as i64 * DAY
        });
        Some(Couple {
            partner,
            key,
            start,
            separation,
            seat: pseat,
        })
    }

    /// True if the couple of `id` is together at `t`. Deaths are looked up
    /// only when the dates alone don't decide.
    fn couple_active(&self, id: PersonId, c: &Couple, t: i64) -> bool {
        c.start <= t
            && c.separation.map_or(true, |s| t < s)
            && t < self.death(id)
            && t < self.death(c.partner)
    }

    /// The union of `id` at `seat`.
    fn union_at(&self, seat: Seat, id: PersonId) -> Option<Union> {
        let c = self.couple(seat, id)?;
        let first_death = self.death(id).min(self.death(c.partner));
        Some(Union {
            partner: c.partner,
            start: c.start,
            separation: c.separation,
            end: c.separation.map_or(first_death, |s| s.min(first_death)),
            key: c.key,
        })
    }

    /// First union of `id`, with kin repair applied (see
    /// [`Self::repair`]).
    pub fn union(&self, id: PersonId) -> Option<Union> {
        self.union_at(self.first_seat(&self.life_pos(id))?, id)
    }

    /// Unions of `id`: the first, and the second if the first ended in
    /// divorce and they re-partnered (R1c). A second union starts after the
    /// first's separation year.
    pub fn unions(&self, id: PersonId) -> [Option<Union>; MAX_UNIONS] {
        self.seats(&self.life_pos(id))
            .map(|s| s.and_then(|s| self.union_at(s, id)))
    }

    /// The year and kind of a person's first union cell: formed in-world,
    /// arrived together, same-sex (left or right side of its market), or
    /// with a divorced partner.
    pub fn union_class(&self, id: PersonId) -> Option<(i32, CellKind)> {
        let cell = self.union_cell(&self.life_pos(id))?;
        Some((cell.year(), cell.kind))
    }

    /// The year and kind of each of a person's union cells: first, then
    /// second (R1c).
    pub fn union_cells(&self, id: PersonId) -> [Option<(i32, CellKind)>; MAX_UNIONS] {
        let p = self.life_pos(id);
        self.seats(&p).map(|s| {
            s.map(|s| {
                let cell = &Self::cells(self.layout(s.block), s.sex)[s.ci];
                (cell.year(), cell.kind)
            })
        })
    }

    /// Partner at time `t`, if a union is active then.
    pub fn partner_at(&self, id: PersonId, t: i64) -> Option<PersonId> {
        self.seats(&self.life_pos(id))
            .into_iter()
            .flatten()
            .filter_map(|s| self.couple(s, id))
            .find(|c| self.couple_active(id, c, t))
            .map(|c| c.partner)
    }

    // --- plans -------------------------------------------------------------------

    /// A woman's union-plan leaf, her cell's year (which the leaf's birth
    /// offsets count from) and the union's start year (earlier for couples
    /// who arrived together).
    fn plan_leaf_at(&self, seat: Seat) -> Option<(Leaf, i32, i32)> {
        if seat.sex != Sex::Female {
            return None;
        }
        let layout = self.layout(seat.block);
        let cell = &layout.f_cells[seat.ci];
        // Same-sex cells carry no plans.
        let pl = Self::plan(layout, cell.plan)?;
        let li = pl.leaf_at(pl.perm().fwd(seat.i));
        let year = cell.year();
        let union_year = match pl.arrivals().get(li) {
            Some(&(d, _)) => year - d as i32,
            None => year,
        };
        Some((pl.leaves()[li], year, union_year))
    }

    /// Calendar year of the last planned in-world birth of a woman's union
    /// at `seat`, if any.
    fn plan_end_at(&self, seat: Seat) -> Option<i32> {
        let (leaf, year, _) = self.plan_leaf_at(seat)?;
        leaf.last_offset().map(|o| year + o)
    }

    /// A woman's non-union births, if any: `None` (without reading the leaf
    /// arrays) for the many in the leading leaf without births.
    fn nonunion_births(&self, p: &LifePos) -> Option<NonUnionLeaf> {
        let co = &self.layout(p.block).cohorts[p.cohort as usize];
        if co.nonunion_perm.fwd(p.offset) < co.nu_none {
            return None;
        }
        let leaf = co.nonunion[self.nonunion_slot(p).0];
        (leaf.births > 0).then_some(leaf)
    }

    /// A woman's non-union leaf index and her offset within it.
    fn nonunion_slot(&self, p: &LifePos) -> (usize, u64) {
        let co = &self.layout(p.block).cohorts[p.cohort as usize];
        let pos = co.nonunion_perm.fwd(p.offset);
        if pos < co.nu_none {
            return (0, pos);
        }
        let li = co.nu_starts.partition_point(|&s| s <= pos) - 1;
        (li, pos - co.nu_starts[li])
    }

    // --- parents and children -----------------------------------------------------

    /// Row `a` (mother's age) of a block's birth table, if in range.
    fn birth_row(layout: &BlockLayout, age: i32) -> Option<&[u32]> {
        if !(MIN_BIRTH_AGE..=MAX_BIRTH_AGE).contains(&age) {
            return None;
        }
        let stride = layout.columns.len();
        let ai = (age - MIN_BIRTH_AGE) as usize;
        Some(&layout.birth_rows[ai * stride..(ai + 1) * stride])
    }

    /// The column of a birth-row entry `e` (below the row total): the last
    /// column starting at or before it, found through the row's coarse
    /// index.
    fn column_of(layout: &BlockLayout, age: i32, e: u64) -> usize {
        let stride = layout.columns.len();
        let cst = stride.div_ceil(COARSE);
        let ai = (age - MIN_BIRTH_AGE) as usize;
        let row = &layout.birth_rows[ai * stride..(ai + 1) * stride];
        let coarse = &layout.birth_coarse[ai * cst..(ai + 1) * cst];
        let e = e as u32;
        let c = coarse.partition_point(|&v| v <= e);
        let lo = (c - 1) * COARSE;
        let hi = (lo + COARSE).min(stride);
        lo + row[lo..hi].partition_point(|&v| v <= e) - 1
    }

    /// Non-union births of a block at mother's age `age`.
    fn nu_events_at(layout: &BlockLayout, age: i32) -> &[(u64, u16, u16, u8)] {
        if !(MIN_BIRTH_AGE..=MAX_BIRTH_AGE).contains(&age) {
            return &[];
        }
        let ai = (age - MIN_BIRTH_AGE) as usize;
        let (lo, hi) = (layout.nu_age_starts[ai], layout.nu_age_starts[ai + 1]);
        &layout.nu_events[lo as usize..hi as usize]
    }

    /// The birth event holding position `e` of mother block `mb`'s births
    /// in `year`, and the offset within it. Canonical order: union births by
    /// cell, then plan leaf (each leaf's women give at most one birth a
    /// year), then non-union births by leaf and birth index.
    fn locate_birth(&self, mb: u32, year: i32, e: u64) -> (EventKind, u64) {
        let layout = self.layout(mb);
        let age = year - self.block(mb).year;
        let row = Self::birth_row(layout, age).expect("a birth year within childbearing ages");
        let union_total = *row.last().unwrap() as u64;
        if e < union_total {
            let col = Self::column_of(layout, age, e);
            let mut r = e - row[col] as u64;
            // The column's class-cells in order, each one's leaves in order.
            for pi in layout.columns[col]..layout.columns[col + 1] {
                let pl = Self::plan(layout, pi as u16).expect("a column's plans");
                let o = year - layout.f_cells[pl.cell as usize].year();
                for (li, leaf, count) in pl.counted() {
                    if leaf.births_at(o) {
                        if r < count {
                            let k = (leaf.mask() & ((1u32 << o) - 1)).count_ones() as u8;
                            let kind = EventKind::Union {
                                cell: pl.cell,
                                leaf: li as u16,
                                k,
                            };
                            return (kind, r);
                        }
                        r -= count;
                    }
                }
            }
            unreachable!("birth rows agree with the leaves");
        }
        let mut r = e - union_total;
        for &(count, cohort, leaf, k) in Self::nu_events_at(layout, age) {
            if r < count {
                return (EventKind::NonUnion { cohort, leaf, k }, r);
            }
            r -= count;
        }
        unreachable!("the ledger counts every birth");
    }

    /// First position of a birth event among mother block `mb`'s births in
    /// `year` (the inverse of [`Self::locate_birth`]).
    fn event_start(&self, mb: u32, year: i32, kind: EventKind) -> u64 {
        let layout = self.layout(mb);
        let age = year - self.block(mb).year;
        let row = Self::birth_row(layout, age).expect("a birth year within childbearing ages");
        match kind {
            EventKind::Union { cell, leaf, .. } => {
                let plan = layout.f_cells[cell as usize].plan;
                let column = layout.plans[plan as usize].column;
                row[column as usize] as u64 + Self::births_before(layout, plan, leaf as usize, year)
            }
            EventKind::Arrival { .. } => unreachable!("arrival events use their own line"),
            EventKind::NonUnion { cohort, leaf, k } => {
                let before: u64 = Self::nu_events_at(layout, age)
                    .iter()
                    .take_while(|&&(_, c, l, kk)| (c, l, kk) != (cohort, leaf, k))
                    .map(|&(count, _, _, _)| count)
                    .sum();
                *row.last().unwrap() as u64 + before
            }
        }
    }

    /// Births in `year` in plan `plan`'s column before leaf `leaf` of that
    /// plan: from the column's earlier class-cells, then from the plan's
    /// earlier leaves.
    fn births_before(layout: &BlockLayout, plan: u16, leaf: usize, year: i32) -> u64 {
        let count = |pi: u32, upto: usize| -> u64 {
            let p = Self::plan(layout, pi as u16).expect("a column's plans");
            let o = year - layout.f_cells[p.cell as usize].year();
            p.counted()
                .take(upto)
                .filter(|&(_, l, _)| l.births_at(o))
                .map(|(_, _, n)| n)
                .sum()
        };
        let column = layout.plans[plan as usize].column as usize;
        (layout.columns[column]..plan as u32)
            .map(|pi| count(pi, usize::MAX))
            .sum::<u64>()
            + count(plan as u32, leaf)
    }

    /// Mother, if born in-world.
    pub fn mother(&self, id: PersonId) -> Option<PersonId> {
        self.mother_event(id).map(|(m, _)| m)
    }

    /// Mother and the kind of birth event: for people born in-world, and
    /// for children who arrived with their parents.
    fn mother_event(&self, id: PersonId) -> Option<(PersonId, EventKind)> {
        let (m, kind, _) = self.mother_event_at(id, self.parent_line_pos(id)?);
        Some((m, kind))
    }

    /// [`Self::mother_event`] from the child's parent-line position.
    /// Also the mother's seat in the union of a union birth or a child born
    /// abroad.
    fn mother_event_at(
        &self,
        id: PersonId,
        (mb, e, arrival): ParentPos,
    ) -> (PersonId, EventKind, Option<Seat>) {
        let mlayout = self.layout(mb);
        if let Some(t) = arrival {
            // Born abroad: the leaves of the mother's arrival cells (one per
            // class, in class order) with a child of this arrival age, in
            // leaf order.
            let age = t - self.birth_year(id);
            let mut r = e;
            for ci in Self::cells_of(mlayout, Sex::Female, t, CellKind::Arrival) {
                let pl = Self::plans(mlayout, &mlayout.f_cells[ci]);
                for ((li, leaf, count), &(_, kids)) in pl.counted().zip(pl.arrivals().iter()) {
                    if kids >> age & 1 == 1 {
                        if r < count {
                            let pos = leaf.start() + r;
                            let i = pl.perm().inv(pos);
                            let mother = self.person_in_cell(mb, Sex::Female, ci, i);
                            let kind = EventKind::Arrival {
                                cell: ci as u16,
                                leaf: li as u16,
                            };
                            let seat = Seat {
                                block: mb,
                                sex: Sex::Female,
                                ci,
                                i,
                            };
                            return (mother, kind, Some(seat));
                        }
                        r -= count;
                    }
                }
            }
            unreachable!("the ledger counts every child who arrives");
        }
        let (kind, offset_in_cell) = self.locate_birth(mb, self.birth_year(id), e);
        match kind {
            EventKind::Union { cell, leaf, .. } => {
                let pl = Self::plans(mlayout, &mlayout.f_cells[cell as usize]);
                let pos = pl.leaves()[leaf as usize].start() + offset_in_cell;
                let seat = Seat {
                    block: mb,
                    sex: Sex::Female,
                    ci: cell as usize,
                    i: pl.perm().inv(pos),
                };
                (
                    self.person_in_cell(mb, Sex::Female, seat.ci, seat.i),
                    kind,
                    Some(seat),
                )
            }
            EventKind::NonUnion { cohort, leaf, .. } => {
                let co = &mlayout.cohorts[cohort as usize];
                let pos = co.nu_starts[leaf as usize] + offset_in_cell;
                let offset = co.nonunion_perm.inv(pos);
                (
                    self.id_of(mb, self.raw_of(mb, cohort, Sex::Female, offset)),
                    kind,
                    None,
                )
            }
            EventKind::Arrival { .. } => unreachable!("native parent lines hold in-world births"),
        }
    }

    /// Father: the mother's partner at conception, the pack's `gestation_days`
    /// before the birth.
    ///
    /// For a union birth that is her partner if he was still alive then:
    /// plans end before separation, so only his death can end the union
    /// first. A child conceived after his death has no in-world father
    /// (until widowed re-partnering). Children born abroad arrived with
    /// both parents.
    pub fn father(&self, id: PersonId) -> Option<PersonId> {
        let (m, kind, seat) = self.mother_event_at(id, self.parent_line_pos(id)?);
        let conception = self.birth(id) - self.ledger.params.fertility.gestation_days * DAY;
        // A union birth's father is the mother's partner in that union
        // (her first or her second, R1c).
        let partner = || seat.and_then(|s| self.repaired_partner(s, m)).map(|x| x.0);
        match kind {
            EventKind::Arrival { .. } => partner(),
            EventKind::Union { .. } => partner().filter(|&f| conception < self.death(f)),
            // Only a man can be a father (a woman's same-sex partner is not).
            EventKind::NonUnion { .. } => self
                .partner_at(m, conception)
                .filter(|&f| self.sex(f) == Sex::Male),
        }
    }

    /// The child born to a mother's event, as an id.
    fn child_of_event(
        &self,
        mb: u32,
        year: i32,
        kind: EventKind,
        offset_in_cell: u64,
        start: u64,
    ) -> Option<PersonId> {
        let mother = self.block(mb);
        let cb = self.ledger.block_of(year, mother.group)?;
        let age = year - mother.year;
        let k = (MAX_BIRTH_AGE - age) as usize;
        if let EventKind::Arrival { cell, leaf } = kind {
            // A child born abroad: the arrival parent line of the child's
            // block's cohort for the mother's arrival year.
            let ml = self.layout(mb);
            let c = &ml.f_cells[cell as usize];
            let arrival_age = c.year() - year;
            // Children of this arrival age in earlier class-cells of the
            // arrival year, then in earlier leaves of this one.
            let kids_before = |ci: usize, leaves: usize| -> u64 {
                let pl = Self::plans(ml, &ml.f_cells[ci]);
                pl.counted()
                    .take(leaves)
                    .zip(pl.arrivals().iter())
                    .filter(|&(_, &(_, kids))| kids >> arrival_age & 1 == 1)
                    .map(|((_, _, n), _)| n)
                    .sum()
            };
            let before: u64 = Self::cells_of(ml, Sex::Female, c.year(), CellKind::Arrival)
                .take_while(|&ci| ci < cell as usize)
                .map(|ci| kids_before(ci, usize::MAX))
                .sum::<u64>()
                + kids_before(cell as usize, leaf as usize);
            let clayout = self.layout(cb);
            let co = &clayout.cohorts[Self::cohort_by_arrival(clayout, c.year())?];
            let pl = co.parents.as_ref()?;
            let q = pl.starts[k] + before + offset_in_cell;
            return Some(self.id_of(cb, co.raw_start + pl.perm.inv(q)));
        }
        let e = if start == NO_START {
            self.event_start(mb, year, kind)
        } else {
            start
        };
        let q = self.layout(cb).parent_starts[k] + e + offset_in_cell;
        Some(self.id_of(cb, self.parent_perm(cb).inv(q)))
    }

    /// A woman's births, in year order: her unions' planned births (both
    /// unions, R1c) and her non-union births.
    fn births_of(&self, p: &LifePos) -> Births {
        let mut out = Births::new();
        for seat in self.seats(p).into_iter().flatten() {
            self.union_births(seat, &mut out);
        }
        self.nonunion_births_into(p, &mut out);
        out.sort_by_year();
        out
    }

    /// The planned births, and children born abroad, of a woman's union at
    /// `seat`.
    fn union_births(&self, seat: Seat, out: &mut Births) {
        let layout = self.layout(seat.block);
        let block_year = self.block(seat.block).year;
        let cell = &Self::cells(layout, seat.sex)[seat.ci];
        if seat.sex == Sex::Female && cell.plan != NO_PLAN {
            let (ci, i) = (seat.ci, seat.i);
            let cell_year = cell.year();
            let plan = cell.plan;
            let pl = Self::plan(layout, plan).expect("the cell's plans");
            let pos = pl.perm().fwd(i);
            let li = pl.leaf_at(pos);
            let in_leaf = pos - pl.leaves()[li].start();
            // Children born abroad, who arrived with the couple.
            if let Some(&(_, kids)) = pl.arrivals().get(li) {
                for age in (0..32).filter(|a| kids >> a & 1 == 1) {
                    out.push((
                        cell_year - age,
                        EventKind::Arrival {
                            cell: ci as u16,
                            leaf: li as u16,
                        },
                        in_leaf,
                        NO_START,
                    ));
                }
            }
            // One pass over the column's earlier leaves (its earlier
            // class-cells, which share the union year, then this cell's)
            // gives every union birth's start in its year's birth order.
            let own = pl.leaves()[li].mask();
            let mut before = [0u64; 32];
            let mut add = |leaf: Leaf, count: u64| {
                let mut m = leaf.mask() & own;
                while m != 0 {
                    before[m.trailing_zeros() as usize] += count;
                    m &= m - 1;
                }
            };
            for pi in layout.columns[pl.column as usize]..plan as u32 {
                for (_, leaf, count) in Self::plan(layout, pi as u16).unwrap().counted() {
                    add(leaf, count);
                }
            }
            for (_, leaf, count) in pl.counted().take(li) {
                add(leaf, count);
            }
            let mut mask = own;
            let mut k = 0;
            while mask != 0 {
                let o = mask.trailing_zeros() as i32;
                mask &= mask - 1;
                let year = cell_year + o;
                if year <= self.ledger.params.y1 {
                    let row = Self::birth_row(layout, year - block_year)
                        .expect("plan births fall within childbearing ages");
                    out.push((
                        year,
                        EventKind::Union {
                            cell: ci as u16,
                            leaf: li as u16,
                            k,
                        },
                        in_leaf,
                        row[pl.column as usize] as u64 + before[o as usize],
                    ));
                }
                k += 1;
            }
        }
    }

    /// A woman's non-union births.
    fn nonunion_births_into(&self, p: &LifePos, out: &mut Births) {
        let layout = self.layout(p.block);
        let block_year = self.block(p.block).year;
        let (li, off) = self.nonunion_slot(p);
        let co = &layout.cohorts[p.cohort as usize];
        let leaf = co.nonunion[li];
        let first_birth_year = self.first_birth_year(p.block, co);
        for k in 0..leaf.births as usize {
            let year = block_year + leaf.ages[k] as i32;
            if year >= first_birth_year && year <= self.ledger.params.y1 {
                out.push((
                    year,
                    EventKind::NonUnion {
                        cohort: p.cohort,
                        leaf: li as u16,
                        k: k as u8,
                    },
                    off,
                    NO_START,
                ));
            }
        }
    }

    /// A man's children with his partner in the union at `seat` (the rule
    /// of `father`): her planned births in that union conceived while he was
    /// alive, children born abroad, and her non-union births conceived while
    /// their union was active. `death` is his.
    fn children_of_couple(&self, seat: Seat, id: PersonId, death: i64, out: &mut KinList) {
        let Some(couple) = self.couple(seat, id) else {
            return;
        };
        if couple.seat.sex != Sex::Female {
            return;
        }
        let wp = self.life_pos(couple.partner);
        let mut births = Births::new();
        self.union_births(couple.seat, &mut births);
        self.nonunion_births_into(&wp, &mut births);
        births.sort_by_year();
        for (year, kind, off, start) in births.iter() {
            let Some(c) = self.child_of_event(wp.block, year, kind, off, start) else {
                continue;
            };
            let conception = self.birth(c) - self.ledger.params.fertility.gestation_days * DAY;
            let his = match kind {
                EventKind::Arrival { .. } => true,
                EventKind::Union { .. } => conception < death,
                EventKind::NonUnion { .. } => self.couple_active(id, &couple, conception),
            };
            if his {
                out.push(c);
            }
        }
    }

    /// Children, in birth-year order within each union. For a man: children
    /// born to each partner while their union was active.
    pub fn children(&self, id: PersonId) -> KinList {
        let p = self.life_pos(id);
        let mut out = KinList::new();
        match p.sex {
            Sex::Female => {
                for (year, kind, off, start) in self.births_of(&p).iter() {
                    if let Some(c) = self.child_of_event(p.block, year, kind, off, start) {
                        out.push(c);
                    }
                }
            }
            Sex::Male => {
                let death = self.death(id);
                for seat in self.seats(&p).into_iter().flatten() {
                    self.children_of_couple(seat, id, death, &mut out);
                }
            }
        }
        out
    }

    /// Siblings (sharing a mother or a father), excluding `id`, in
    /// birth-year order. In R1 everyone has at most one union and a man's
    /// children are all his partner's, so the mother's children are every
    /// sibling, full or half. The exhaustive test
    /// `siblings_are_the_union_of_both_parents_children` guards this;
    /// re-partnering (R1c) must revisit it.
    pub fn siblings(&self, id: PersonId) -> KinList {
        let mut out = KinList::new();
        let Some(pos) = self.parent_line_pos(id) else {
            return out;
        };
        let (m, _, seat) = self.mother_event_at(id, pos);
        for c in self.children(m) {
            if c != id {
                out.push(c);
            }
        }
        // Paternal half-siblings (R1c): a father with another union has
        // children with another partner. His cell tells cheaply whether he
        // can: he is in his second union, or his first one separates.
        let other_union = match seat {
            Some(s) => {
                let cell = Self::cell(self.layout(s.block), s.sex, s.ci);
                let his = cell.kind.partner();
                his.second() || (his.feeds_divorced() && cell.class > 0)
            }
            None => self.first_seat(&self.life_pos(m)).is_some(),
        };
        if other_union {
            if let Some(f) = self.father(id) {
                let fp = self.life_pos(f);
                let death = self.death(f);
                for s in self.seats(&fp).into_iter().flatten() {
                    let mut kids = KinList::new();
                    self.children_of_couple(s, f, death, &mut kids);
                    for c in kids {
                        if c != id && !out.contains(&c) {
                            out.push(c);
                        }
                    }
                }
            }
        }
        out
    }

    // --- death ---------------------------------------------------------------------

    fn lifetable(&self, b: u32, sex: Sex) -> &[f64] {
        let row = (2 * b as usize + sex as usize) * TABLE;
        &self.lifetables[row..row + TABLE]
    }

    fn lifetable_indexed(&self, b: u32, sex: Sex) -> Indexed<'_> {
        let r = 2 * b as usize + sex as usize;
        Indexed {
            row: &self.lifetables[r * TABLE..(r + 1) * TABLE],
            coarse: &self.lifetable_coarse[r * COARSE_LEN..(r + 1) * COARSE_LEN],
        }
    }

    fn build_lifetable(&self, b: u32) -> [Vec<f64>; 2] {
        let block = self.block(b);
        let year = block.year;
        let p = &self.ledger.params;
        let table = |sex| {
            let mut l = Vec::with_capacity(MAX_AGE as usize + 2);
            let mut s = 1.0;
            for a in 0..=MAX_AGE {
                l.push(s);
                let t = year + a as i32;
                let f = p.heritage.mortality_factor(block.heritage, t);
                s *= 1.0 - p.mortality.death_prob_scaled(sex, a, t, f);
            }
            l.push(0.0);
            l
        };
        [table(Sex::Female), table(Sex::Male)]
    }

    /// Age a cohort's members must reach alive: founders are alive when
    /// the world starts, immigrants through their arrival year.
    fn entry_age(&self, b: u32, cohort: u16) -> u32 {
        let block = self.block(b);
        match self.layout(b).cohorts[cohort as usize].arrival {
            Some(t) => (t - block.year + 1) as u32,
            None if block.founder => (self.ledger.params.y0 - block.year).max(0) as u32,
            None => 0,
        }
    }

    /// Residual death distribution for a block's never-partnered natives, as
    /// a cumulative table by age: the natives' cohort deaths minus the mass
    /// their partnered members carry. (Never-partnered immigrants draw from
    /// the life table conditioned on arrival: mixing them in would dilute
    /// the natives' childhood deaths.)
    fn residual_indexed(&self, b: u32, sex: Sex) -> Indexed<'_> {
        let r = 2 * b as usize + sex as usize;
        Indexed {
            row: &self.residuals[r * TABLE..(r + 1) * TABLE],
            coarse: &self.residual_coarse[r * COARSE_LEN..(r + 1) * COARSE_LEN],
        }
    }

    fn build_residual(&self, b: u32) -> [Vec<f64>; 2] {
        let layout = self.layout(b);
        let block = self.block(b);
        let make = |sex: Sex| {
            let l = self.lifetable(b, sex);
            let cells = Self::cells(layout, sex);
            // The natives' expected deaths by age from their entry age on.
            let natives = &block.cohorts[0];
            let n = match sex {
                Sex::Female => natives.females,
                Sex::Male => natives.size - natives.females,
            };
            let a0 = (self.entry_age(b, 0) as usize).min(MAX_AGE as usize);
            let mut dens = vec![0.0; MAX_AGE as usize + 1];
            if l[a0] > 0.0 {
                for (a, d) in dens.iter_mut().enumerate().skip(a0) {
                    *d = n as f64 * (l[a] - l[a + 1]) / l[a0];
                }
            }
            for c in cells.iter() {
                // The natives' share of the cell (cohort 0 comes first).
                let share = if c.first_cohort == 0 {
                    c.first_end as f64 / c.total() as f64
                } else {
                    0.0
                };
                // Partnered members reach the year after their union (which
                // is after any cohort's entry).
                let req = |extra: i32| ((c.year() - block.year + 1 + extra).max(0)) as usize;
                let mut take = |count: u64, a_req: usize| {
                    if a_req > MAX_AGE as usize || l[a_req] <= 0.0 {
                        return;
                    }
                    let count = count as f64 * share;
                    for (a, d) in dens.iter_mut().enumerate().skip(a_req) {
                        *d -= count * (l[a] - l[a + 1]) / l[a_req];
                    }
                };
                // Women with plans survive them; everyone else partnered
                // reaches the year after the union.
                match Self::plan(layout, c.plan) {
                    Some(pl) => {
                        for (_, leaf, count) in pl.counted() {
                            take(count, req(leaf.last_offset().unwrap_or(0)));
                        }
                    }
                    None => take(c.total(), req(0)),
                }
            }
            let mut total: f64 = dens.iter().map(|d| d.max(0.0)).sum();
            if total <= 0.0 {
                // Degenerate: fall back to the plain cohort table.
                dens = (0..=MAX_AGE as usize)
                    .map(|a| if a < a0 { 0.0 } else { l[a] - l[a + 1] })
                    .collect();
                total = dens.iter().sum();
            }
            // Cumulative: `cum[a]` is the mass below age `a`.
            let mut cum = Vec::with_capacity(dens.len() + 1);
            let mut acc = 0.0;
            cum.push(acc);
            for d in dens {
                acc += d.max(0.0) / total;
                cum.push(acc);
            }
            cum
        };
        [make(Sex::Female), make(Sex::Male)]
    }

    /// Age the person must reach alive (survive through calendar year
    /// `birth year + age - 1`), given what their own cells commit them to:
    /// entry into the world, the union's start and, for a woman, her planned
    /// births. A father need only be alive at conception, which
    /// [`Self::father`] checks on the child's side, so no death depends on
    /// anyone else's.
    fn required_age(&self, p: LifePos) -> u32 {
        let year = self.block(p.block).year;
        let mut req = self.entry_age(p.block, p.cohort) as i32;
        let layout = self.layout(p.block);
        // Each union: its start and, for a woman, its planned births.
        for seat in self.seats(&p).into_iter().flatten() {
            let cell = &Self::cells(layout, p.sex)[seat.ci];
            req = req.max(cell.year() - year + 1);
            if let Some(end) = self.plan_end_at(seat) {
                req = req.max(end - year + 1);
            }
        }
        if p.sex == Sex::Female {
            if let Some(leaf) = self.nonunion_births(&p) {
                req = req.max(leaf.ages[leaf.births as usize - 1] as i32 + 1);
            }
        }
        req.max(0) as u32
    }

    /// Death time, seconds since 1800.
    ///
    /// The age at death is drawn from the block's distribution (cohort life
    /// table if partnered, residual if never partnered) conditioned on
    /// reaching the required age `r` ([`Self::required_age`]): what the
    /// person's own cells commit them to. Beyond entry and union years only
    /// a woman's planned births bind her. A father need only be alive at
    /// conception, which is checked on the child's side ([`Self::father`]),
    /// so a death never looks at anyone else.
    ///
    /// For speed the draw has two stages, which give exactly that
    /// conditional law:
    ///
    /// 1. draw `A0` conditioned on the constraints `r0 <= r` that need no
    ///    plan (entry into the world, the union year);
    /// 2. keep `A0` if `A0 >= r`; otherwise redraw conditioned on `r` with a
    ///    fresh uniform.
    ///
    /// For `a >= r`: `P(a) = f(a)/S(r0) + (1 - S(r)/S(r0)) * f(a)/S(r) =
    /// f(a)/S(r)`. Every planned birth falls by [`MAX_BIRTH_AGE`], so `A0`
    /// past it is kept without computing `r`; for men `r = r0`.
    pub fn death(&self, id: PersonId) -> i64 {
        let p = self.life_pos(id);
        let by = self.block(p.block).year;
        let sub = Self::sub_of(self.layout(p.block), &p).map(|(s, _)| s);
        let union_year = sub.map(|s| s.year as i32);
        let entry = self.entry_age(p.block, p.cohort) as i32;
        // Constraints that need no plan or second union, and a bound on
        // what a woman's births and a second union add to them.
        let own = match union_year {
            Some(year) => entry.max(year - by + 1),
            None => entry,
        };
        let mut bound = match p.sex {
            Sex::Male => own,
            Sex::Female => own.max(MAX_BIRTH_AGE + 1),
        };
        if sub.is_some_and(|s| s.divorces) {
            // A second union starts by the remarriage age limit.
            bound = bound.max(MAX_REMARRIAGE_AGE + 1);
        }
        let max = MAX_AGE as usize;
        let u = self.key.with2(TAG_DEATH, id as u64);
        let draw = |req: i32, key: Key| -> usize {
            let req = (req.max(0) as usize).min(max);
            match union_year {
                None if p.cohort == 0 => {
                    invert_cumulative(self.residual_indexed(p.block, p.sex), req, key.unit())
                }
                _ => invert_survival(
                    self.lifetable_indexed(p.block, p.sex),
                    req,
                    key.unit_open0(),
                ),
            }
        };
        let a0 = draw(own, u);
        let age = if a0 as i32 >= bound {
            a0
        } else {
            let req = self.required_age(p) as i32;
            if a0 as i32 >= req {
                a0
            } else {
                draw(req, u.with(3))
            }
        };
        // Death falls in calendar year `birth year + age`, after the birth
        // itself in the first year. A required age r therefore guarantees
        // survival through all of calendar year `birth year + r - 1`.
        let year = self.block(p.block).year;
        if age == 0 {
            let birth = self.birth(id);
            let span = (year_start(year + 1) - birth - 1).max(1);
            birth + 1 + u.with(1).below(span as u64) as i64
        } else {
            let (lo, hi) = (
                year_start(year + age as i32),
                year_start(year + age as i32 + 1),
            );
            lo + u.with(1).below((hi - lo) as u64) as i64
        }
    }

    /// True if alive at time `t` (seconds since 1800).
    pub fn alive_at(&self, id: PersonId, t: i64) -> bool {
        self.birth(id) <= t && t < self.death(id)
    }
}

/// A table row with a coarse index: `coarse[k] = row[COARSE * k]`, one cache
/// line, so a search reads it and one 16-entry segment instead of a binary
/// search's scattered lines.
#[derive(Clone, Copy)]
struct Indexed<'a> {
    row: &'a [f64],
    coarse: &'a [f64],
}

/// Coarse index spacing and length for a [`TABLE`]-long row.
const COARSE: usize = 16;
const COARSE_LEN: usize = TABLE.div_ceil(COARSE);

/// A sorted array with a coarse index (every [`COARSE`]th value), so a
/// search reads the index (a line or two) and one segment of [`COARSE`]
/// values instead of a binary search's scattered lines: R1c's class-cells
/// made cell keys and cohort lines several times longer.
#[derive(Clone, Debug, Default)]
struct Coarse<T> {
    values: Box<[T]>,
    index: Box<[T]>,
}

impl<T: Copy + Ord> Coarse<T> {
    fn new(values: Vec<T>) -> Self {
        let index = values.iter().step_by(COARSE).copied().collect();
        Self {
            values: values.into_boxed_slice(),
            index,
        }
    }

    /// Number of values at most `x`.
    fn count_le(&self, x: T) -> usize {
        let c = self.index.partition_point(|&v| v <= x);
        if c == 0 {
            return 0;
        }
        let lo = (c - 1) * COARSE;
        let hi = (lo + COARSE).min(self.values.len());
        lo + self.values[lo..hi].partition_point(|&v| v <= x)
    }

    /// Number of values below `x`.
    fn count_lt(&self, x: T) -> usize {
        let c = self.index.partition_point(|&v| v < x);
        if c == 0 {
            return 0;
        }
        let lo = (c - 1) * COARSE;
        let hi = (lo + COARSE).min(self.values.len());
        lo + self.values[lo..hi].partition_point(|&v| v < x)
    }

    /// Index of `x`, if present.
    fn find(&self, x: T) -> Option<usize> {
        let i = self.count_lt(x);
        (self.values.get(i) == Some(&x)).then_some(i)
    }
}

/// Every [`COARSE`]th entry of a row.
fn coarse_index(row: &[f64]) -> impl Iterator<Item = f64> + '_ {
    row.iter().step_by(COARSE).copied()
}

impl Indexed<'_> {
    /// The first index `>= from` where `pred` fails, for `pred` true on a
    /// prefix of the row and false after (monotone). Equal to
    /// `from + row[from..].partition_point(pred)`.
    fn first_fail(&self, from: usize, pred: impl Fn(f64) -> bool) -> usize {
        let k = self.coarse.partition_point(|&c| pred(c));
        let first = if k == 0 {
            0
        } else {
            let lo = COARSE * (k - 1) + 1;
            let hi = (COARSE * k).min(self.row.len());
            lo + self.row[lo..hi].partition_point(|&x| pred(x))
        };
        first.max(from)
    }
}

/// Age at death from a survival table `l` (`l[a]`: share alive at age `a`,
/// non-increasing, `l[MAX_AGE + 1] = 0`) conditioned on reaching `req`, by
/// inversion of `u` in `(0, 1]`: the first age whose survivors fall below
/// `u * l[req]`.
fn invert_survival(l: Indexed, req: usize, u: f64) -> usize {
    let target = u * l.row[req];
    (l.first_fail(req + 1, |s| s >= target) - 1).min(MAX_AGE as usize)
}

/// Age at death from a cumulative table (`cum[a]`: mass below age `a`,
/// non-decreasing) conditioned on reaching `req`, by inversion of `u` in
/// `[0, 1)`: the first age whose cumulative mass passes the target.
fn invert_cumulative(cum: Indexed, req: usize, u: f64) -> usize {
    let max = MAX_AGE as usize;
    let tail = cum.row[max + 1] - cum.row[req];
    if tail <= 0.0 {
        return req;
    }
    let threshold = cum.row[req] + u * tail;
    (cum.first_fail(req + 1, |c| c <= threshold) - 1).min(max)
}

fn opposite(s: Sex) -> Sex {
    match s {
        Sex::Female => Sex::Male,
        Sex::Male => Sex::Female,
    }
}

pub use procedural_core::stream::{year_of, year_start, DAY};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_stage_death_draw_has_the_conditional_law() {
        // The law `death` relies on: draw from "reach r0", keep it if it
        // reaches r, else redraw from "reach r". The result must be the
        // "reach r" law exactly; checked by total variation on 400k draws.
        let w = World::build(Params::tiny(), 3);
        let lt = w.lifetable_indexed(w.ledger.blocks.len() as u32 / 2, Sex::Male);
        let l = lt.row;
        let (r0, r) = (20usize, 45usize);
        let key = Key::from_seed(11);
        let n = 400_000u64;
        let mut hist = vec![0u64; MAX_AGE as usize + 1];
        for i in 0..n {
            let k = key.with(i);
            let a0 = invert_survival(lt, r0, k.unit_open0());
            let a = if a0 >= r {
                a0
            } else {
                invert_survival(lt, r, k.with(3).unit_open0())
            };
            hist[a] += 1;
        }
        let mut tv = 0.0;
        for (a, &h) in hist.iter().enumerate() {
            let exact = if a < r { 0.0 } else { (l[a] - l[a + 1]) / l[r] };
            tv += (h as f64 / n as f64 - exact).abs();
        }
        assert!(tv / 2.0 < 0.01, "total variation {:.4}", tv / 2.0);
    }

    #[test]
    fn coarse_sorted_search_matches_a_plain_search() {
        for n in [0usize, 1, 15, 16, 17, 100, 1000] {
            let values: Vec<i32> = (0..n as i32).map(|i| 3 * i + (i % 3)).collect();
            let c = Coarse::new(values.clone());
            for x in -2..(3 * n as i32 + 8) {
                assert_eq!(
                    c.count_le(x),
                    values.partition_point(|&v| v <= x),
                    "n {n} x {x}"
                );
                assert_eq!(
                    c.count_lt(x),
                    values.partition_point(|&v| v < x),
                    "n {n} x {x}"
                );
                assert_eq!(c.find(x), values.binary_search(&x).ok(), "n {n} x {x}");
            }
        }
    }

    #[test]
    fn coarse_indexed_search_matches_a_plain_search() {
        let w = World::build(Params::tiny(), 3);
        let key = Key::from_seed(5);
        for b in 0..w.layouts.len() as u32 {
            for sex in [Sex::Female, Sex::Male] {
                let tables = [w.lifetable_indexed(b, sex), w.residual_indexed(b, sex)];
                for (k, t) in tables.into_iter().enumerate() {
                    for i in 0..40u64 {
                        let v = key.with3(b as u64, k as u64, i).unit();
                        let from =
                            key.with3(b as u64, k as u64, i + 99).below(TABLE as u64) as usize;
                        // Survival rows fall, cumulative rows rise.
                        let (got, want) = if k == 0 {
                            let pred = |x: f64| x >= v;
                            let want = from + t.row[from..].partition_point(|&x| pred(x));
                            (t.first_fail(from, pred), want)
                        } else {
                            let pred = |x: f64| x <= v;
                            let want = from + t.row[from..].partition_point(|&x| pred(x));
                            (t.first_fail(from, pred), want)
                        };
                        assert_eq!(got, want, "block {b} {sex:?} table {k}");
                    }
                }
            }
        }
    }

    #[test]
    fn bucketed_decode_matches_a_full_search() {
        let w = World::build(Params::tiny(), 3);
        let base = &w.ledger.base;
        for id in 0..w.population() {
            let b = base.partition_point(|&s| s <= id) - 1;
            assert_eq!(
                w.decode(id as PersonId),
                (b as u32, id - base[b]),
                "id {id}"
            );
        }
    }

    #[test]
    fn same_mother_matches_resolving_both_mothers() {
        // The cell shortcut in `same_mother` against the full comparison:
        // every pair of siblings, and each person against up to 64 others
        // whose mothers share a block.
        for seed in [3, 11] {
            let w = World::build(Params::tiny(), seed);
            let mut by_block: std::collections::BTreeMap<u32, Vec<(PersonId, ParentPos)>> =
                Default::default();
            for x in 0..w.population() as PersonId {
                if let Some(px) = w.parent_line_pos(x) {
                    by_block.entry(px.0).or_default().push((x, px));
                }
            }
            let (mut pairs, mut same) = (0u64, 0u64);
            for group in by_block.values() {
                for (i, &(x, px)) in group.iter().enumerate() {
                    for &(y, py) in group.iter().skip(i + 1).take(64) {
                        let expected = w.mother(x) == w.mother(y);
                        assert_eq!(w.same_mother(x, px, y, py), expected, "{x} and {y}");
                        pairs += 1;
                        same += expected as u64;
                    }
                }
            }
            for x in 0..w.population() as PersonId {
                let Some(px) = w.parent_line_pos(x) else {
                    continue;
                };
                // Maternal siblings only: since R1c, `siblings` also lists
                // paternal half-siblings, who have another mother.
                let m = w.mother(x).expect("a mother");
                for s in w.children(m).into_iter().filter(|&s| s != x) {
                    let ps = w.parent_line_pos(s).expect("a sibling has a mother");
                    assert!(w.same_mother(x, px, s, ps), "siblings {x} and {s}");
                }
            }
            assert!(
                pairs > 100_000 && same > 100,
                "{pairs} pairs, {same} siblings"
            );
        }
    }
}

#[cfg(test)]
mod footprint {
    use super::*;

    #[test]
    #[ignore = "diagnostic: prints the world's memory footprint"]
    fn print_footprint() {
        let w = World::build(Params::prototype(), 42);
        let (mut cells, mut partners, mut plans) = (0usize, 0usize, 0usize);
        for l in &w.layouts {
            for c in l.f_cells.iter().chain(&l.m_cells) {
                cells += 1;
                partners += c.n_slices as usize;
            }
            plans += l.plans.iter().map(|p| p.n_leaves as usize).sum::<usize>();
        }
        let cell_bytes = cells * std::mem::size_of::<CellLayout>() + partners * 8 + plans * (8 + 4);
        let tables: usize = w
            .layouts
            .iter()
            .map(|l| l.birth_rows.len() * 4 + l.nu_events.len() * 16)
            .sum();
        println!(
            "blocks {} | cells {cells} (CellLayout {} B, CompactPerm {} B) partners {partners} plans {plans} -> ~{:.1} MB",
            w.layouts.len(),
            std::mem::size_of::<CellLayout>(),
            std::mem::size_of::<CompactPerm>(),
            cell_bytes as f64 / 1e6
        );
        println!("birth tables ~{:.1} MB", tables as f64 / 1e6);
        let mut by_total = [0usize; 6];
        let mut same_sex = 0usize;
        for l in &w.layouts {
            for c in l.f_cells.iter().chain(&l.m_cells) {
                by_total[(c.total() as usize).min(5)] += 1;
                same_sex += c.kind.same_sex() as usize;
            }
        }
        println!("cells by total (0..=4, 5+): {by_total:?}; same-sex cells {same_sex}");
        let cohorts: usize = w.layouts.iter().map(|l| l.cohorts.len()).sum();
        let cohort_bytes: usize = w
            .layouts
            .iter()
            .flat_map(|l| &l.cohorts)
            .map(|c| {
                std::mem::size_of::<CohortLayout>()
                    + (c.subs[0].len() + c.subs[1].len()) * std::mem::size_of::<Sub>()
                    + c.nonunion.len() * std::mem::size_of::<NonUnionLeaf>()
                    + c.nu_starts.len() * 8
            })
            .sum();
        println!(
            "cohorts {cohorts} (CohortLayout {} B) -> ~{:.1} MB; population {}",
            std::mem::size_of::<CohortLayout>(),
            cohort_bytes as f64 / 1e6,
            w.population()
        );
    }
}
