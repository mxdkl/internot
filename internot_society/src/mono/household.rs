//! Households (L3) on the monotone world: who lives with whom at `t`, as
//! a pure function of `(seed, id, t)` over its kin lookups. Ported from the
//! ledger world's households (archived in
//! `.scratch/archive/2026-10-03-old-worlds/`); rules and sources: the pack's
//! `households.ron`.
//!
//! A person's household follows three rules, in order:
//! 1. a dependent (a child who hasn't left home, or an orphaned minor)
//!    lives in the household of the parent or guardian responsible for them;
//! 2. a unit (a single adult, or a couple deciding together) may live with
//!    kin: a parent, a sibling or an adult child who is an anchor (lives on
//!    their own and doesn't seek kin);
//! 3. partners in a union live in the union's household; a single adult
//!    lives alone.
//!
//! Rule 1 moves to an older person responsible for a dependent; rule 2 is a
//! single step to an anchor, who never seeks kin, so every chain ends.
//! [`Mono::members`] enumerates a household from its seeds through the same
//! rules and confirms each candidate with them, so the two always agree.
//!
//! Debt: no roommate groups yet (the pack's `roommates` section): their
//! frames would pair people from the whole country until areas exist, so
//! those adults live alone.

use procedural_core::stream::year_of;

use super::{Mono, Pid, Union, YEAR};
use crate::params::{Households, Sex};

const T_LEAVE: u64 = 40;
const T_CUSTODY: u64 = 41;
const T_KIN: u64 = 42;

/// Age of majority: minors without a living parent need a guardian.
const ADULT_AGE: f64 = 18.0;

/// A household at some time. Members are found with [`Mono::members`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Household {
    /// The home of a first union, named by its wife.
    Union { wife: Pid, husband: Pid, start: i64 },
    /// A single adult's own home: `spell` 0 before their union, 1 after it.
    Solo { person: Pid, spell: u8 },
}

/// Most members one household can list.
pub const MAX_MEMBERS: usize = 64;

/// A household's members, stored inline (no allocation), without
/// duplicates. Reads as a slice of people.
#[derive(Clone, Copy)]
pub struct Members {
    ids: [Pid; MAX_MEMBERS],
    len: u8,
}

impl Members {
    const fn new() -> Self {
        Self { ids: [Pid { cell: 0, y: 0, i: 0 }; MAX_MEMBERS], len: 0 }
    }

    /// Adds `x` unless it is already listed.
    fn push(&mut self, x: Pid) {
        if self.as_slice().contains(&x) {
            return;
        }
        assert!((self.len as usize) < MAX_MEMBERS, "household over capacity");
        self.ids[self.len as usize] = x;
        self.len += 1;
    }

    /// The members, as a slice.
    pub fn as_slice(&self) -> &[Pid] {
        &self.ids[..self.len as usize]
    }
}

impl std::ops::Deref for Members {
    type Target = [Pid];

    fn deref(&self) -> &[Pid] {
        self.as_slice()
    }
}

impl std::fmt::Debug for Members {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_slice().fmt(f)
    }
}

impl<'a> Mono<'a> {
    fn hh(&self) -> &'a Households {
        let p: &'a crate::Params = self.p;
        &p.households
    }

    // --- basic predicates ---------------------------------------------------

    /// True if `x` is in the world at `t`: born and alive.
    pub fn present_at(&self, x: Pid, t: i64) -> bool {
        self.alive_at(x, t)
    }

    /// Age in years at `t`.
    pub fn age_at(&self, x: Pid, t: i64) -> f64 {
        (t - self.birth(x)) as f64 / YEAR
    }

    /// `x`'s union if it is in force at `t`.
    pub fn union_during(&self, x: Pid, t: i64) -> Option<Union> {
        self.union_of(x).filter(|u| u.start <= t && t < u.end)
    }

    /// `x`'s single spell at `t`: 0 before their union (or with none), 1
    /// after it ends.
    fn spell_at(&self, x: Pid, t: i64) -> u8 {
        self.union_of(x).is_some_and(|u| u.end <= t) as u8
    }

    // --- rule 1: living at home -----------------------------------------------

    /// When `x` leaves home on their own (by the independence curve) or by
    /// starting their union, whichever comes first (`None`: neither ever
    /// happens).
    pub fn leave_time(&self, x: Pid) -> Option<i64> {
        let u = self.pkey(x, T_LEAVE).unit();
        let independence = self.hh().independence_age(self.sex(x), x.y, u).map(|a| self.birth(x) + (a * YEAR) as i64);
        let union = self.union_of(x).map(|u| u.start);
        [independence, union].into_iter().flatten().min()
    }

    /// The parent `x` lives with at `t` if `x` is still at home: the mother,
    /// or the father if the parents' union ended in separation and its
    /// custody draw went to him; the other parent if that one is dead.
    fn custodial(&self, x: Pid, t: i64) -> Option<Pid> {
        let (mother, union) = match self.parents(x) {
            Some((m, _, u)) => (Some(m), u),
            None => (None, None),
        };
        let father = union.map(|u| u.husband);
        let (mut primary, mut other) = (mother, father);
        if let Some(u) = union {
            let separated = u.how == 0 && u.end <= t;
            if separated && self.pkey(u.wife, T_CUSTODY).unit() < self.hh().custody_father {
                (primary, other) = (father, mother);
            }
        }
        let here = |p: &Pid| self.present_at(*p, t);
        primary.filter(here).or(other.filter(here))
    }

    /// The person `x` lives with as a dependent at `t`: a parent if `x`
    /// hasn't left home, or a guardian if `x` is a minor with no living
    /// parent. `None` if `x` is independent.
    pub fn dependent_of(&self, x: Pid, t: i64) -> Option<Pid> {
        if self.leave_time(x).is_some_and(|l| l <= t) && (self.age_at(x, t) >= ADULT_AGE || self.union_during(x, t).is_some()) {
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
    fn guardian(&self, x: Pid, t: i64) -> Option<Pid> {
        let (m, f) = match self.parents(x) {
            Some((m, _, u)) => (Some(m), u.map(|u| u.husband)),
            None => (None, None),
        };
        let gp = |p: Option<Pid>| match p.and_then(|p| self.parents(p)) {
            Some((m, _, u)) => [Some(m), u.map(|u| u.husband)],
            None => [None, None],
        };
        let [mm, mf] = gp(m);
        let [fm, ff] = gp(f);
        if let Some(g) = [mm, mf, fm, ff].into_iter().flatten().find(|&g| self.present_at(g, t)) {
            return Some(g);
        }
        self.siblings(x)
            .iter()
            .copied()
            .filter(|&s| self.present_at(s, t) && self.age_at(s, t) >= ADULT_AGE && self.dependent_of(s, t).is_none())
            .min_by_key(|&s| (self.birth(s), s))
    }

    // --- rule 2: living with kin ------------------------------------------------

    /// The unit an independent `x` belongs to at `t`: `x` alone, or the
    /// couple, as `(decider, partner)`. A couple decides together: the wife.
    pub fn unit(&self, x: Pid, t: i64) -> (Pid, Option<Pid>) {
        match self.union_during(x, t) {
            None => (x, None),
            Some(u) => (u.wife, Some(u.husband)),
        }
    }

    /// The unit's age: the older partner's, for a couple.
    fn unit_age(&self, (decider, other): (Pid, Option<Pid>), t: i64) -> f64 {
        let age = self.age_at(decider, t);
        other.map_or(age, |o| age.max(self.age_at(o, t)))
    }

    /// True if the unit seeks to live with kin at `t`: the decider's keyed
    /// uniform under the unit's propensity. It depends on nobody else's
    /// decision, which keeps hosts and guests apart.
    fn seeks_kin(&self, unit: (Pid, Option<Pid>), t: i64) -> bool {
        let age = self.unit_age(unit, t);
        let (year, cohort) = (year_of(t), unit.0.y);
        let q = match unit.1 {
            None => self.hh().kin_single(age, year, cohort),
            Some(_) => self.hh().kin_couple(age, year, cohort),
        };
        self.pkey(unit.0, T_KIN).unit() < q
    }

    /// True if `h` can host kin at `t`: present, an adult on their own (not
    /// a dependent) and not seeking kin. Anchors never seek kin, so a
    /// guest's host is never a guest.
    fn is_anchor(&self, h: Pid, t: i64) -> bool {
        self.present_at(h, t) && self.age_at(h, t) >= ADULT_AGE && self.dependent_of(h, t).is_none() && !self.seeks_kin(self.unit(h, t), t)
    }

    /// Children of `p`, daughters first, then elder before younger.
    fn children_by_preference(&self, p: Pid) -> Vec<Pid> {
        let mut kids: Vec<(bool, i64, Pid)> = self.children(p).iter().map(|&c| (self.sex(c) != Sex::Female, self.birth(c), c)).collect();
        kids.sort_unstable();
        kids.into_iter().map(|(_, _, c)| c).collect()
    }

    /// Siblings of `p`, eldest first.
    fn siblings_by_age(&self, p: Pid) -> Vec<Pid> {
        let mut sibs: Vec<(i64, Pid)> = self.siblings(p).iter().map(|&s| (self.birth(s), s)).collect();
        sibs.sort_unstable();
        sibs.into_iter().map(|(_, s)| s).collect()
    }

    /// `p`'s mother and father, where known.
    fn parents_of(&self, p: Pid) -> impl Iterator<Item = Pid> {
        let (m, f) = match self.parents(p) {
            Some((m, _, u)) => (Some(m), u.map(|u| u.husband)),
            None => (None, None),
        };
        [m, f].into_iter().flatten()
    }

    /// The kin an independent `x` lives with at `t`, if `x`'s unit seeks kin
    /// and one of its candidates is an anchor. Candidates, in order:
    /// - single, under 35: mother, father, then siblings;
    /// - single, 35 to 59: siblings, mother, father, then children;
    /// - single, 60+: children (daughters first), then siblings;
    /// - couple, under 45: the wife's parents, then the husband's;
    /// - couple, 45+: the wife's children, then the husband's.
    pub fn kin_host(&self, x: Pid, t: i64) -> Option<Pid> {
        let unit = self.unit(x, t);
        if !self.seeks_kin(unit, t) {
            return None;
        }
        let age = self.unit_age(unit, t);
        let mut candidates: Vec<Pid> = Vec::new();
        match unit {
            (d, None) if age < 35.0 => {
                candidates.extend(self.parents_of(d));
                candidates.extend(self.siblings_by_age(d));
            }
            (d, None) if age < 60.0 => {
                candidates.extend(self.siblings_by_age(d));
                candidates.extend(self.parents_of(d));
                candidates.extend(self.children_by_preference(d));
            }
            (d, None) => {
                candidates.extend(self.children_by_preference(d));
                candidates.extend(self.siblings_by_age(d));
            }
            (d, Some(o)) if age < 45.0 => {
                candidates.extend(self.parents_of(d));
                candidates.extend(self.parents_of(o));
            }
            (d, Some(o)) => {
                candidates.extend(self.children_by_preference(d));
                candidates.extend(self.children_by_preference(o));
            }
        }
        candidates.into_iter().find(|&h| h != unit.0 && Some(h) != unit.1 && self.is_anchor(h, t))
    }

    // --- households -------------------------------------------------------------

    /// The person at the end of `x`'s dependent chain at `t` (the one whose
    /// household `x` lives in, before any kin hosting), or `x` if
    /// independent.
    pub fn chain_end(&self, x: Pid, t: i64) -> Pid {
        let mut p = x;
        while let Some(q) = self.dependent_of(p, t) {
            p = q;
        }
        p
    }

    /// The household `x` lives in at `t`, or `None` if `x` isn't in the
    /// world then (not yet born, or dead).
    pub fn household(&self, x: Pid, t: i64) -> Option<Household> {
        if !self.present_at(x, t) {
            return None;
        }
        let mut p = self.chain_end(x, t);
        // Hosts are anchors (never guests), so this is one step.
        if let Some(host) = self.kin_host(p, t) {
            p = host;
        }
        Some(self.own_household(p, t))
    }

    /// The household an independent anchor `p` heads at `t`: their union's,
    /// or their own.
    fn own_household(&self, p: Pid, t: i64) -> Household {
        match self.union_during(p, t) {
            Some(u) => Household::Union { wife: u.wife, husband: u.husband, start: u.start },
            None => Household::Solo { person: p, spell: self.spell_at(p, t) },
        }
    }

    /// Everyone who lives in `h` at `t`: its seeds (the partners, or the
    /// single adult), then everyone whose household resolves through a
    /// member: children at home with them, orphans in their care, and kin
    /// they host (with a guest's partner), recursively. Each candidate is
    /// confirmed by the same rules [`Self::household`] follows.
    pub fn members(&self, h: Household, t: i64) -> Members {
        let mut out = Members::new();
        match h {
            Household::Union { wife, husband, .. } => {
                out.push(wife);
                out.push(husband);
            }
            Household::Solo { person, .. } => out.push(person),
        }
        let mut i = 0;
        while i < out.len() {
            self.add_dependents(out[i], t, &mut out);
            i += 1;
        }
        out
    }

    /// Adds the people whose household resolves through `s` at `t`.
    fn add_dependents(&self, s: Pid, t: i64, out: &mut Members) {
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
                if self.present_at(sib, t) && self.age_at(sib, t) < ADULT_AGE && self.dependent_of(sib, t) == Some(s) {
                    out.push(sib);
                }
            }
        }
        // Guests: kin of `s` whose unit lives with `s`, and the partner of a
        // guest couple.
        let kin: Vec<Pid> = self.parents_of(s).chain(children.iter().copied()).chain(siblings.iter().copied()).collect();
        for k in kin {
            if self.present_at(k, t) && self.dependent_of(k, t).is_none() && self.kin_host(k, t) == Some(s) {
                out.push(k);
                if let Some(u) = self.union_during(k, t) {
                    out.push(u.partner(k));
                }
            }
        }
    }
}
