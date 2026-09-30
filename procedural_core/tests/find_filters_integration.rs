//! End-to-end tests for filter_static and scan_budget on Space::find().

use procedural_core::bits::BitLayout;
use procedural_core::search::Termination;
use procedural_core::space::Space;

fn people_space() -> Space<u64> {
    let layout = BitLayout::<u64>::new(vec![("locale", 8), ("age", 7), ("hash", 49)]).unwrap();
    Space::<u64>::new("people", layout)
}

#[test]
fn filter_rejects_candidates_that_dont_match() {
    // Pin all structured fields so the unconstrained 49-bit hash is the only
    // free dimension. Filter selects every other hash value; take(16) finishes
    // after 32 candidates evaluated.
    let space = people_space();
    let ids: Vec<u64> = space
        .find()
        .where_eq("locale", 10)
        .where_eq("age", 40)
        .filter_static("hash_even", |id| {
            space.layout().extract(id, "hash") & 1 == 0
        })
        .execute()
        .take(16)
        .collect();
    assert_eq!(ids.len(), 16);
    for id in &ids {
        assert_eq!(space.layout().extract(*id, "locale"), 10);
        assert_eq!(space.layout().extract(*id, "age"), 40);
        assert_eq!(space.layout().extract(*id, "hash") & 1, 0);
    }
}

#[test]
fn scan_budget_terminates_after_n_candidates() {
    let space = people_space();
    let mut results = space
        .find()
        .where_eq("locale", 10)
        .where_eq("age", 40)
        .scan_budget(100)
        .execute();
    let _ids: Vec<u64> = (&mut results).collect();
    assert_eq!(results.evaluated(), 100);
    assert_eq!(
        results.termination(),
        &Termination::Budgeted { evaluated: 100 }
    );
}

#[test]
fn filter_and_budget_compose() {
    let space = people_space();
    let mut results = space
        .find()
        .where_eq("locale", 10)
        .where_eq("age", 40)
        .filter_static("hash_div_7", |id| {
            space.layout().extract(id, "hash") % 7 == 0
        })
        .scan_budget(50)
        .execute();
    let ids: Vec<u64> = (&mut results).collect();
    // Budget caps total draws at 50; filter selects roughly 1/7.
    assert!(ids.len() <= 50);
    assert_eq!(results.evaluated(), 50);
    assert_eq!(
        results.termination(),
        &Termination::Budgeted { evaluated: 50 }
    );
    for id in &ids {
        assert_eq!(space.layout().extract(*id, "hash") % 7, 0);
    }
}

#[test]
fn unfiltered_exhaustive_narrow_query_reports_exhaustive() {
    // Tiny layout fully pinned → 1 result then Exhaustive.
    let layout = BitLayout::<u64>::new(vec![("a", 4), ("b", 4)]).unwrap();
    let space = Space::<u64>::new("tiny", layout);
    let mut results = space.find().where_eq("a", 3).where_eq("b", 7).execute();
    let _ids: Vec<u64> = (&mut results).collect();
    assert_eq!(results.termination(), &Termination::Exhaustive);
}
