//! Households (L3): who lives with whom at time `t`, as a pure function of
//! `(seed, id, t)` over the world's kin lookups. Plan:
//! `docs/superpowers/plans/2026-09-30-l3-households.md`.
//!
//! A person's household follows four rules, in order:
//! 1. a dependent (a child who hasn't left home, or an orphaned minor)
//!    lives in the household of the parent or guardian responsible for them;
//! 2. a unit (a single adult, or a couple deciding together) may live with
//!    kin: a parent, a sibling or an adult child who is an anchor (lives on
//!    their own and doesn't seek kin);
//! 3. partners in a union live in the union's household;
//! 4. a single adult lives alone or in a roommate group.
//!
//! Rule 1 moves to an older person responsible for a dependent; rule 2 is a
//! single step to an anchor, who never seeks kin, so every chain ends. [`World::members`]
//! enumerates a household from its seeds through the same rules and
//! confirms each candidate with them, so the two always agree.

use procedural_core::key::label;
use procedural_core::partition::{even_parts, locate_in_segments, segment_offset};
use procedural_core::perm::{Bijection, CompactPerm};
use procedural_core::stream::{year_of, year_start, Epochs, DAY};

use crate::params::{Households, Sex, ROOMMATE_FRAME};
use crate::ledger::UPBRINGING_AGE;
use crate::world::{PersonId, Union, World};

const TAG_LEAVE: u64 = label("household/leave");
const TAG_CUSTODY: u64 = label("household/custody");
const TAG_KIN: u64 = label("household/kin");
const TAG_ROOM_PHASE: u64 = label("household/roommate-phase");
const TAG_ROOM_FRAME: u64 = label("household/roommate-frame");
const TAG_ROOM_UPTAKE: u64 = label("household/roommate-uptake");

/// Mean Gregorian year, in seconds (for ages).
const YEAR: f64 = 365.2425 * DAY as f64;

/// Age of majority: minors without a living parent need a guardian.
const ADULT_AGE: f64 = 18.0;

/// A household at some time. Members are found with [`World::members`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Household {
    /// The home of a union: its partners (`a < b`) and its start (two people
    /// can divorce and later remarry each other).
    Union {
        a: PersonId,
        b: PersonId,
        start: i64,
    },
    /// A single adult's own home. `spell` counts the person's unions that
    /// ended before (0 to 2), so each single stretch of life is its own
    /// household.
    Solo { person: PersonId, spell: u8 },
    /// A roommate group: the frame and chunk that formed it (see
    /// [`World::members`] and the plan, §5).
    Roommates {
        region: u16,
        band: u16,
        epoch: i32,
        frame: u32,
        group: u8,
    },
}

/// Most members one household can list.
pub const MAX_MEMBERS: usize = 64;

/// A household's members, stored inline (no allocation), without
/// duplicates. Reads as a slice of ids.
#[derive(Clone, Copy)]
pub struct Members {
    ids: [PersonId; MAX_MEMBERS],
    len: u8,
}

impl Members {
    const fn new() -> Self {
        Self {
            ids: [0; MAX_MEMBERS],
            len: 0,
        }
    }

    /// Adds `id` unless it is already listed.
    fn push(&mut self, id: PersonId) {
        if self.as_slice().contains(&id) {
            return;
        }
        assert!((self.len as usize) < MAX_MEMBERS, "household over capacity");
        self.ids[self.len as usize] = id;
        self.len += 1;
    }

    /// The ids, as a slice.
    pub fn as_slice(&self) -> &[PersonId] {
        &self.ids[..self.len as usize]
    }
}

impl std::ops::Deref for Members {
    type Target = [PersonId];

    fn deref(&self) -> &[PersonId] {
        self.as_slice()
    }
}

impl std::fmt::Debug for Members {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_slice().fmt(f)
    }
}

/// A roommate frame: one lineage region's band of birth years in one epoch.
#[derive(Clone, Copy, Debug)]
struct Frame {
    region: u16,
    band: u16,
    epoch: i32,
    /// When the epoch starts: eligibility is fixed then.
    start: i64,
}

impl World {
    /// The pack's household section.
    fn hh(&self) -> &Households {
        &self.ledger().params.households
    }

    // --- basic predicates ---------------------------------------------------

    /// True if `x` is in the world at `t`: born, alive, and arrived if an
    /// immigrant.
    pub fn present_at(&self, x: PersonId, t: i64) -> bool {
        self.alive_at(x, t) && (!self.is_immigrant(x) || self.arrival(x).is_some_and(|a| a <= t))
    }

    /// Age in years at `t`.
    pub(crate) fn age_at(&self, x: PersonId, t: i64) -> f64 {
        (t - self.birth(x)) as f64 / YEAR
    }

    /// The union `x` is in at `t`, if any.
    pub(crate) fn union_during(&self, x: PersonId, t: i64) -> Option<Union> {
        self.unions(x)
            .into_iter()
            .flatten()
            .find(|u| u.start <= t && t < u.end)
    }

    /// Unions of `x` that ended by `t`.
    pub(crate) fn unions_ended(&self, x: PersonId, t: i64) -> u8 {
        self.unions(x)
            .into_iter()
            .flatten()
            .filter(|u| u.end <= t)
            .count() as u8
    }

    // --- rule 1: living at home -----------------------------------------------

    /// When `x` leaves home: at the independence age, the first union's start
    /// or (area mode) a single long move or the parents' household moving to
    /// another area once `x` is an adult, whichever comes first (`None`:
    /// none ever happens).
    pub(crate) fn leave_time(&self, x: PersonId) -> Option<i64> {
        let u = self.key().with2(TAG_LEAVE, x as u64).unit();
        let independence = self
            .hh()
            .independence_age(self.sex(x), self.birth_year(x), u)
            .map(|a| self.birth(x) + (a * YEAR) as i64);
        let union = self.union(x).map(|u| u.start);
        // Area mode: a single long move to another area is leaving home, and
        // so is the parents' household moving away once `x` is an adult:
        // `x` stays in the area of upbringing, as the ledger counts them.
        let migration = self.migration(x).map(|m| m.1);
        let parents_move = if self.ledger().params.places.by_area {
            [self.mother(x), self.father(x)]
                .into_iter()
                .flatten()
                .flat_map(|p| self.couple_moves(p))
                .flatten()
                .map(|(_, t)| t)
                // The ledger's area of upbringing, by calendar year.
                .filter(|&t| year_of(t) > self.birth_year(x) + UPBRINGING_AGE)
                .min()
        } else {
            None
        };
        [independence, union, migration, parents_move]
            .into_iter()
            .flatten()
            .min()
    }

    /// The parent `x` lives with at `t` if `x` is still at home: the mother,
    /// or the father if the parents' union ended in divorce and its custody
    /// draw went to him; the other parent if that one is dead.
    fn custodial(&self, x: PersonId, t: i64) -> Option<PersonId> {
        let mother = self.mother(x);
        let father = self.father(x);
        let (mut primary, mut other) = (mother, father);
        if let (Some(m), Some(f)) = (mother, father) {
            // The union of conception: the latest with the father that
            // started before the birth.
            let birth = self.birth(x);
            let union = self
                .unions(m)
                .into_iter()
                .flatten()
                .filter(|u| u.partner == f && u.start <= birth)
                .max_by_key(|u| u.start);
            if let Some(u) = union {
                let divorced = u.separation.is_some_and(|s| s == u.end && s <= t);
                if divorced && u.key.with(TAG_CUSTODY).unit() < self.hh().custody_father {
                    (primary, other) = (father, mother);
                }
            }
        }
        let here = |p: &PersonId| self.present_at(*p, t);
        primary.filter(here).or(other.filter(here))
    }

    /// The person `x` lives with as a dependent at `t`: a parent if `x`
    /// hasn't left home, or a guardian if `x` is a minor with no living
    /// parent. A minor whose union has ended goes back to a parent. `None`
    /// if `x` is independent.
    pub fn dependent_of(&self, x: PersonId, t: i64) -> Option<PersonId> {
        if self.leave_time(x).is_some_and(|l| l <= t)
            && (self.age_at(x, t) >= ADULT_AGE || self.union_during(x, t).is_some())
        {
            return None;
        }
        if let Some(p) = self.custodial(x, t) {
            return Some(p);
        }
        // No living parent: adults are on their own; minors need a guardian.
        if self.age_at(x, t) >= ADULT_AGE {
            return None;
        }
        self.guardian(x, t)
    }

    /// The guardian of an orphaned minor: the first living grandparent
    /// (mother's mother, mother's father, father's mother, father's father),
    /// else the eldest living sibling who is an adult and has left home.
    fn guardian(&self, x: PersonId, t: i64) -> Option<PersonId> {
        let (m, f) = (self.mother(x), self.father(x));
        let grandparents = [
            m.and_then(|m| self.mother(m)),
            m.and_then(|m| self.father(m)),
            f.and_then(|f| self.mother(f)),
            f.and_then(|f| self.father(f)),
        ];
        if let Some(g) = grandparents
            .into_iter()
            .flatten()
            .find(|&g| self.present_at(g, t))
        {
            return Some(g);
        }
        self.siblings(x)
            .iter()
            .copied()
            .filter(|&s| {
                self.present_at(s, t)
                    && self.age_at(s, t) >= ADULT_AGE
                    && self.dependent_of(s, t).is_none()
            })
            .min_by_key(|&s| (self.birth(s), s))
    }

    // --- rule 2: living with kin (L3 §13) ---------------------------------------

    /// The unit an independent `x` belongs to at `t`: `x` alone, or the
    /// couple, as `(decider, partner)`. A couple decides together: the
    /// woman (the lower id, for a same-sex couple).
    pub(crate) fn unit(&self, x: PersonId, t: i64) -> (PersonId, Option<PersonId>) {
        match self.union_during(x, t).map(|u| u.partner) {
            None => (x, None),
            Some(p) => {
                let x_first = match (self.sex(x), self.sex(p)) {
                    (Sex::Female, Sex::Male) => true,
                    (Sex::Male, Sex::Female) => false,
                    _ => x < p,
                };
                if x_first {
                    (x, Some(p))
                } else {
                    (p, Some(x))
                }
            }
        }
    }

    /// The unit's age: the older partner's, for a couple.
    fn unit_age(&self, (decider, other): (PersonId, Option<PersonId>), t: i64) -> f64 {
        let age = self.age_at(decider, t);
        other.map_or(age, |o| age.max(self.age_at(o, t)))
    }

    /// True if the unit seeks to live with kin at `t`: the decider's keyed
    /// uniform under the unit's propensity. It depends on nobody else's
    /// decision, which keeps hosts and guests apart.
    fn seeks_kin(&self, unit: (PersonId, Option<PersonId>), t: i64) -> bool {
        let age = self.unit_age(unit, t);
        let (year, cohort) = (year_of(t), self.birth_year(unit.0));
        let q = match unit.1 {
            None => self.hh().kin_single(age, year, cohort),
            Some(_) => self.hh().kin_couple(age, year, cohort),
        };
        self.key().with2(TAG_KIN, unit.0 as u64).unit() < q
    }

    /// True if `h` can host kin at `t`: present, an adult on their own (not
    /// a dependent), not seeking kin, and not in a roommate group's
    /// running. Anchors never seek kin, so a guest's host is never a guest.
    fn is_anchor(&self, h: PersonId, t: i64) -> bool {
        self.present_at(h, t)
            && self.age_at(h, t) >= ADULT_AGE
            && self.dependent_of(h, t).is_none()
            && !self.seeks_kin(self.unit(h, t), t)
            && !self.roommate_eligible(h, &self.frame_of(h, t))
    }

    /// Children of `p`, daughters first, then elder before younger.
    fn children_by_preference(&self, p: PersonId) -> Vec<PersonId> {
        let mut kids: Vec<(bool, i64, PersonId)> = self
            .children(p)
            .iter()
            .map(|&c| (self.sex(c) != Sex::Female, self.birth(c), c))
            .collect();
        kids.sort_unstable();
        kids.into_iter().map(|(_, _, c)| c).collect()
    }

    /// Siblings of `p`, eldest first.
    fn siblings_by_age(&self, p: PersonId) -> Vec<PersonId> {
        let mut sibs: Vec<(i64, PersonId)> = self
            .siblings(p)
            .iter()
            .map(|&s| (self.birth(s), s))
            .collect();
        sibs.sort_unstable();
        sibs.into_iter().map(|(_, s)| s).collect()
    }

    /// The kin an independent `x` lives with at `t`, if `x`'s unit seeks kin
    /// and one of its candidates is an anchor (L3 §13). Candidates, in
    /// order:
    /// - single, under 35: mother, father, then siblings;
    /// - single, 35 to 59: siblings, mother, father, then children;
    /// - single, 60+: children (daughters first), then siblings;
    /// - couple, under 45: the decider's parents, then the partner's;
    /// - couple, 45+: the decider's children, then the partner's.
    pub fn kin_host(&self, x: PersonId, t: i64) -> Option<PersonId> {
        let unit = self.unit(x, t);
        if !self.seeks_kin(unit, t) {
            return None;
        }
        let age = self.unit_age(unit, t);
        let parents = |p: PersonId| [self.mother(p), self.father(p)].into_iter().flatten();
        let mut candidates: Vec<PersonId> = Vec::new();
        match unit {
            (d, None) if age < 35.0 => {
                candidates.extend(parents(d));
                candidates.extend(self.siblings_by_age(d));
            }
            (d, None) if age < 60.0 => {
                candidates.extend(self.siblings_by_age(d));
                candidates.extend(parents(d));
                candidates.extend(self.children_by_preference(d));
            }
            (d, None) => {
                candidates.extend(self.children_by_preference(d));
                candidates.extend(self.siblings_by_age(d));
            }
            (d, Some(o)) if age < 45.0 => {
                candidates.extend(parents(d));
                candidates.extend(parents(o));
            }
            (d, Some(o)) => {
                candidates.extend(self.children_by_preference(d));
                candidates.extend(self.children_by_preference(o));
            }
        }
        // Area mode: only kin the ledger counts in the unit's own area (a
        // guest elsewhere would live away from its market; debt: no moves to
        // kin in another area).
        let areas = self.ledger().params.places.by_area;
        let here = |h: PersonId| !areas || self.area_at(h, t) == self.area_at(unit.0, t);
        candidates
            .into_iter()
            .find(|&h| h != unit.0 && Some(h) != unit.1 && self.is_anchor(h, t) && here(h))
    }

    // --- rule 4: roommates ------------------------------------------------------

    /// The roommate frame of `x`'s band at `t`.
    fn frame_of(&self, x: PersonId, t: i64) -> Frame {
        let first_year = self.ledger().first_year;
        let band = ((self.birth_year(x) - first_year) / self.hh().roommates.band_years) as u16;
        self.frame_at(self.region(x), band, t)
    }

    /// The frame of `(region, band)` in force at `t`: epochs of
    /// the pack's `roommates.epoch_years` with a keyed phase per band, so bands don't
    /// all regroup on the same day.
    fn frame_at(&self, region: u16, band: u16, t: i64) -> Frame {
        let epochs = self.epochs(region, band);
        self.frame_in(region, band, epochs.index(t) as i32)
    }

    /// Frame `epoch` of `(region, band)`.
    fn frame_in(&self, region: u16, band: u16, epoch: i32) -> Frame {
        Frame {
            region,
            band,
            epoch,
            start: self.epochs(region, band).start(epoch as i64),
        }
    }

    /// The band's epochs: the pack's `roommates.epoch_years` long, with a
    /// keyed phase.
    fn epochs(&self, region: u16, band: u16) -> Epochs {
        let span = (self.hh().roommates.epoch_years as f64 * YEAR) as i64;
        Epochs::keyed(
            year_start(self.ledger().first_year),
            span,
            self.key().with3(TAG_ROOM_PHASE, region as u64, band as u64),
        )
    }

    /// The blocks of a band, as `(block, size)` in (birth year, heritage)
    /// order: every heritage of the region, so roommates mix heritages.
    fn band_blocks(&self, region: u16, band: u16) -> impl Iterator<Item = (u32, u64)> + '_ {
        let ledger = self.ledger();
        let groups = ledger.groups as u32;
        let years = ledger.blocks.len() as u32 / groups;
        let band_years = self.hh().roommates.band_years as u32;
        let heritages = ledger.params.heritage_count() as u32;
        let first = band as u32 * band_years;
        let region_base = region as u32 * heritages;
        (first..(first + band_years).min(years)).flat_map(move |y| {
            (0..heritages).map(move |h| {
                let b = y * groups + region_base + h;
                (b, ledger.base[b as usize + 1] - ledger.base[b as usize])
            })
        })
    }

    /// The frame's permutation of its band, and the band's size.
    fn frame_perm(&self, f: &Frame) -> (CompactPerm, u64) {
        let n: u64 = self.band_blocks(f.region, f.band).map(|(_, s)| s).sum();
        let key = self.key().with3(
            TAG_ROOM_FRAME,
            (f.region as u64) << 16 | f.band as u64,
            f.epoch as i64 as u64,
        );
        (CompactPerm::new(n, key), n)
    }

    /// `x`'s index in its band.
    fn band_index(&self, x: PersonId, f: &Frame) -> u64 {
        let (block, raw) = self.decode(x);
        segment_offset(self.band_blocks(f.region, f.band), block)
            .expect("a person is in their own band")
            + raw
    }

    /// The person at index `i` of a band.
    fn band_person(&self, f: &Frame, i: u64) -> PersonId {
        let (b, offset) = locate_in_segments(self.band_blocks(f.region, f.band), i)
            .expect("index within the band");
        self.id_of(b, offset)
    }

    /// True if `x` can stay in a roommate group at `t`: present, single, no
    /// child under 18, not seeking kin, and (area mode) counted by the ledger
    /// in the frame's area, `x`'s block's (a migrant leaves the group).
    fn roommate_ok(&self, x: PersonId, t: i64) -> bool {
        self.present_at(x, t)
            && self.union_during(x, t).is_none()
            && self
                .children(x)
                .iter()
                .all(|&c| self.birth(c) > t || self.age_at(c, t) >= ADULT_AGE)
            && !self.seeks_kin((x, None), t)
            && (!self.ledger().params.places.by_area || self.area_at(x, t) == self.region(x))
    }

    /// True if `x` seeks roommates in frame `f`: at the epoch start, of
    /// roommate age, with the epoch's uptake draw, single with no child
    /// under 18, left home and not seeking kin. Cheapest tests first.
    fn roommate_eligible(&self, x: PersonId, f: &Frame) -> bool {
        let t = f.start;
        if !self.alive_at(x, t) {
            return false;
        }
        let age = self.age_at(x, t);
        let r = &self.hh().roommates;
        if !(r.min_age..r.max_age).contains(&age) {
            return false;
        }
        let u = self
            .key()
            .with3(TAG_ROOM_UPTAKE, x as u64, f.epoch as i64 as u64)
            .unit();
        u < self.hh().roommate_uptake(age, year_of(t))
            && self.roommate_ok(x, t)
            && self.dependent_of(x, t).is_none()
            && !self.seeks_kin((x, None), t)
    }

    /// The people of frame `f`'s slot range `frame` who seek roommates, in
    /// slot order.
    fn frame_eligible(&self, f: &Frame, perm: &CompactPerm, n: u64, frame: u32) -> Members {
        let mut eligible = Members::new();
        let k = ROOMMATE_FRAME as u64;
        for slot in frame as u64 * k..((frame as u64 + 1) * k).min(n) {
            let m = self.band_person(f, perm.inv(slot));
            if self.roommate_eligible(m, f) {
                eligible.push(m);
            }
        }
        eligible
    }

    /// The groups the eligible people of a frame form (plan §5), as
    /// `(group, members)`: pairs and triples in slot order, as few groups as
    /// possible ([`even_parts`]).
    fn chunks(eligible: &Members) -> impl Iterator<Item = (u8, &[PersonId])> {
        let mut at = 0;
        even_parts(eligible.len() as u64, 2, 3)
            .enumerate()
            .map(move |(g, size)| {
                let chunk = &eligible[at..at + size as usize];
                at += size as usize;
                (g as u8, chunk)
            })
    }

    /// The roommate household of an independent single `x` at `t`, if any.
    fn roommates(&self, x: PersonId, t: i64) -> Option<Household> {
        let f = self.frame_of(x, t);
        if !self.roommate_eligible(x, &f) || !self.roommate_ok(x, t) {
            return None;
        }
        let (perm, n) = self.frame_perm(&f);
        let frame = (perm.fwd(self.band_index(x, &f)) / ROOMMATE_FRAME as u64) as u32;
        let eligible = self.frame_eligible(&f, &perm, n, frame);
        let (group, chunk) = Self::chunks(&eligible).find(|(_, c)| c.contains(&x))?;
        let staying = chunk.iter().filter(|&&m| self.roommate_ok(m, t)).count();
        (staying >= 2).then_some(Household::Roommates {
            region: f.region,
            band: f.band,
            epoch: f.epoch,
            frame,
            group,
        })
    }

    // --- for residence (L4) ------------------------------------------------------

    /// The person at the end of `x`'s dependent chain at `t` (the one whose
    /// household `x` lives in, before any kin hosting), or `x` if
    /// independent.
    pub(crate) fn chain_end(&self, x: PersonId, t: i64) -> PersonId {
        let mut p = x;
        while let Some(q) = self.dependent_of(p, t) {
            p = q;
        }
        p
    }

    /// When the roommate epoch of `x`'s band in force at `t` began.
    pub(crate) fn roommate_epoch_start(&self, x: PersonId, t: i64) -> i64 {
        self.frame_of(x, t).start
    }

    /// The length of a roommate epoch, in seconds.
    pub(crate) fn roommate_epoch_span(&self) -> i64 {
        (self.hh().roommates.epoch_years as f64 * YEAR) as i64
    }

    /// The lease holder of roommate household `h` and the start of its
    /// epoch: the first member of the group's chunk (eligibility, and so
    /// the chunk, is fixed at the epoch start). `None` for other households.
    pub(crate) fn roommate_lease(&self, h: Household) -> Option<(PersonId, i64)> {
        let Household::Roommates {
            region,
            band,
            epoch,
            frame,
            group,
        } = h
        else {
            return None;
        };
        let f = self.frame_in(region, band, epoch);
        let (perm, n) = self.frame_perm(&f);
        let eligible = self.frame_eligible(&f, &perm, n, frame);
        let (_, chunk) = Self::chunks(&eligible).find(|&(g, _)| g == group)?;
        Some((chunk[0], f.start))
    }

    /// The roommate household at `t` whose lease holder is `l`, if any:
    /// `l` was eligible at the epoch start, leads its chunk, and at least
    /// two of the chunk are still there at `t` (`l` need not be).
    pub(crate) fn lease_group(&self, l: PersonId, t: i64) -> Option<(Household, i64)> {
        let f = self.frame_of(l, t);
        if !self.roommate_eligible(l, &f) {
            return None;
        }
        let (perm, n) = self.frame_perm(&f);
        let frame = (perm.fwd(self.band_index(l, &f)) / ROOMMATE_FRAME as u64) as u32;
        let eligible = self.frame_eligible(&f, &perm, n, frame);
        let (group, chunk) = Self::chunks(&eligible).find(|(_, c)| c.contains(&l))?;
        if chunk[0] != l {
            return None;
        }
        let staying = chunk.iter().filter(|&&m| self.roommate_ok(m, t)).count();
        (staying >= 2).then_some((
            Household::Roommates {
                region: f.region,
                band: f.band,
                epoch: f.epoch,
                frame,
                group,
            },
            f.start,
        ))
    }

    // --- households -------------------------------------------------------------

    /// The household `x` lives in at `t`, or `None` if `x` isn't in the
    /// world then (not yet born or arrived, or dead).
    pub fn household(&self, x: PersonId, t: i64) -> Option<Household> {
        if !self.present_at(x, t) {
            return None;
        }
        let mut p = x;
        while let Some(q) = self.dependent_of(p, t) {
            p = q;
        }
        // Hosts are anchors (never guests), so this is one step.
        if let Some(host) = self.kin_host(p, t) {
            p = host;
        }
        if let Some(u) = self.union_during(p, t) {
            let (a, b) = (p.min(u.partner), p.max(u.partner));
            return Some(Household::Union {
                a,
                b,
                start: u.start,
            });
        }
        if let Some(h) = self.roommates(p, t) {
            return Some(h);
        }
        Some(Household::Solo {
            person: p,
            spell: self.unions_ended(p, t),
        })
    }

    /// Everyone who lives in `h` at `t`: its seeds (the partners, the single
    /// adult, or the roommates still in the group), then everyone whose
    /// household resolves through a member: children at home with them,
    /// orphans in their care, and elder parents they host (with the
    /// parent's partner), recursively. Each candidate is confirmed by the
    /// same rules [`Self::household`] follows.
    pub fn members(&self, h: Household, t: i64) -> Members {
        let mut out = Members::new();
        match h {
            Household::Union { a, b, .. } => {
                out.push(a);
                out.push(b);
            }
            Household::Solo { person, .. } => out.push(person),
            Household::Roommates {
                region,
                band,
                epoch,
                frame,
                group,
            } => {
                let f = self.frame_in(region, band, epoch);
                let (perm, n) = self.frame_perm(&f);
                let eligible = self.frame_eligible(&f, &perm, n, frame);
                let chunk = Self::chunks(&eligible).find(|&(g, _)| g == group);
                for &m in chunk.map_or(&[][..], |(_, c)| c) {
                    if self.roommate_ok(m, t) {
                        out.push(m);
                    }
                }
            }
        }
        let mut i = 0;
        while i < out.len() {
            self.add_dependents(out[i], t, &mut out);
            i += 1;
        }
        out
    }

    /// Adds the people whose household resolves through `s` at `t`.
    fn add_dependents(&self, s: PersonId, t: i64, out: &mut Members) {
        let children = self.children(s);
        let siblings = self.siblings(s);
        for &c in children.iter() {
            if self.present_at(c, t) {
                if self.dependent_of(c, t) == Some(s) {
                    out.push(c);
                }
            } else if self.birth(c) <= t {
                // A dead child's orphaned children may be in `s`'s care.
                for &g in self.children(c).iter() {
                    if self.present_at(g, t) && self.dependent_of(g, t) == Some(s) {
                        out.push(g);
                    }
                }
            }
        }
        if self.age_at(s, t) >= ADULT_AGE {
            for &sib in siblings.iter() {
                if self.present_at(sib, t)
                    && self.age_at(sib, t) < ADULT_AGE
                    && self.dependent_of(sib, t) == Some(s)
                {
                    out.push(sib);
                }
            }
        }
        // Guests: kin of `s` whose unit lives with `s`, and the partner of a
        // guest couple.
        let kin = [self.mother(s), self.father(s)]
            .into_iter()
            .flatten()
            .chain(children.iter().copied())
            .chain(siblings.iter().copied());
        for k in kin {
            if self.present_at(k, t)
                && self.dependent_of(k, t).is_none()
                && self.kin_host(k, t) == Some(s)
            {
                out.push(k);
                if let Some(u) = self.union_during(k, t) {
                    out.push(u.partner);
                }
            }
        }
    }
}
