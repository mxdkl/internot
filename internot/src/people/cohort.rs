//! Cohort enumeration helpers: list the people in a workplace.
//!
//! `workplace_members_of(world, industry, city, workplace_seed) ->
//! Vec<u32>` is the foundational cross-service primitive. Mail's
//! inbox derivation, calendar's standup attendees, tasks' delegated
//! tasks — they all start by asking "who else is in this workplace?"
//!
//! Pure `Space::find()` bit-pattern pushdown filtered to populated
//! members (`member_idx < workplace_size`). O(workplace_size) at any
//! population.

use std::sync::OnceLock;

use parking_lot::RwLock;
use procedural_core::sampler::lognormal;
use procedural_core::word::U512;
use procedural_core::world::World;

use super::slot::{mail_id_of, member_idx_of, person_id_for, MAX_WORKPLACE_SIZE};

/// Process-wide cache of `(industry, city, workplace_seed) →
/// Vec<mail_id>`. The function is pure (deterministic over the
/// substrate), and the same workplace gets enumerated by every
/// service that surfaces coworkers (mail inbox, calendar attendees,
/// tasks delegation). At ~240 µs per Space::find sweep, caching
/// turns repeat lookups into ~50 ns HashMap reads.
///
/// Unbounded — at most 64 × 64 × 256 ≈ 1M workplaces; a typical
/// scenario touches < 100. If memory ever matters, swap for an LRU.
fn workplace_cache() -> &'static RwLock<std::collections::HashMap<(u8, u8, u8), Vec<u32>>> {
    static C: OnceLock<RwLock<std::collections::HashMap<(u8, u8, u8), Vec<u32>>>> = OnceLock::new();
    C.get_or_init(|| RwLock::new(std::collections::HashMap::new()))
}

/// Procedural workplace size. Lognormal(mu=2.0, sigma=1.5):
/// median ~7, p99 ~125, capped at `MAX_WORKPLACE_SIZE - 1`.
/// Keyed on the workplace bits, deterministic per workplace.
pub fn workplace_size_for(industry_idx: u8, city_idx: u8, workplace_seed: u8) -> u16 {
    let key = (industry_idx as u64) << 14
        | (city_idx as u64) << 8
        | workplace_seed as u64;
    let raw = lognormal(key, "workplace_size_v1", 2.0, 1.5);
    raw.clamp(1.0, (MAX_WORKPLACE_SIZE - 1) as f64) as u16
}

/// Enumerate the populated members of a workplace cohort. Returns
/// mail_ids (= person_ids = small_ids, identity bridge).
///
/// Memoized in `workplace_cache` — the underlying Space::find sweep
/// is the single largest cost in `get_inbox` (~240 µs); cached
/// returns are ~50 ns. The function is pure over the substrate so
/// the cache is safe for the lifetime of the process.
pub fn workplace_members_of(
    world: &World<U512>,
    industry_idx: u8,
    city_idx: u8,
    workplace_seed: u8,
) -> Vec<u32> {
    let key = (industry_idx, city_idx, workplace_seed);
    if let Some(hit) = workplace_cache().read().get(&key) {
        return hit.clone();
    }
    let size = workplace_size_for(industry_idx, city_idx, workplace_seed);
    let space = world.space("people").expect("people space registered");
    let members: Vec<u32> = space
        .find()
        .where_eq("industry_idx", industry_idx as u64)
        .where_eq("city_idx", city_idx as u64)
        .where_eq("workplace_seed", workplace_seed as u64)
        .scan_budget(MAX_WORKPLACE_SIZE as usize)
        .execute()
        .filter(|id| member_idx_of(*id) < size)
        .map(|id| mail_id_of(id))
        .collect();
    workplace_cache().write().insert(key, members.clone());
    members
}

/// Convenience: a viewer's coworkers (same workplace, excluding
/// self). Used by mail/inbox and tasks/delegation.
pub fn coworkers_of(world: &World<U512>, viewer: u32) -> Vec<u32> {
    let pid = person_id_for(viewer);
    let i = super::slot::industry_idx_of(pid);
    let c = super::slot::city_idx_of(pid);
    let w = super::slot::workplace_seed_of(pid);
    workplace_members_of(world, i, c, w)
        .into_iter()
        .filter(|&m| m != viewer)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Universe;

    #[test]
    fn workplace_size_in_range() {
        for i in 0..16u8 {
            for c in 0..16u8 {
                for s in 0..32u8 {
                    let n = workplace_size_for(i, c, s);
                    assert!((1..MAX_WORKPLACE_SIZE).contains(&n));
                }
            }
        }
    }

    #[test]
    fn coworkers_excludes_self() {
        let u = Universe::new();
        // Sweep until we find a populated viewer with at least 1
        // coworker (most workplaces have median ~7 members).
        for ws in 0..32u32 {
            for mi in 0..4u32 {
                let viewer = (ws << 12) | mi;
                if viewer == 0 { continue; }
                let cw = coworkers_of(u.world_u512, viewer);
                if !cw.is_empty() {
                    assert!(!cw.contains(&viewer));
                    return;
                }
            }
        }
        panic!("no populated viewer with coworkers in sweep");
    }
}
