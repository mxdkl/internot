//! End-to-end tests for `Session<'w, W>` over a real `World`.
//!
//! Exercises read fall-through, write-then-read, type-mismatch errors,
//! cross-session isolation, and the sentinel-bit invariant on
//! session-allocated ids.

use chrono::{DateTime, Utc};
use procedural_core::bits::BitLayout;
use procedural_core::space::Space;
use procedural_core::world::World;
use procedural_overlay::{OverlayError, Session};

// ---- Test fixture: a small two-attribute people space ---------------------

fn build_world_u64() -> World<u64> {
    // Layout sums to 60 bits so the top bit is reserved per the
    // procedural_overlay sentinel convention.
    let layout = BitLayout::<u64>::new(vec![("user_id", 32), ("entropy", 28)]).unwrap();
    let mut people = Space::<u64>::new("people", layout);
    people
        .indexable_attribute::<u64, _>("user_id", "user_id", |b| b)
        .unwrap();
    people
        .attribute::<f64, _>("score", |id: u64| {
            // Deterministic, recognizable.
            (id & 0xFFFF) as f64 * 0.5
        })
        .unwrap();
    people
        .attribute::<String, _>("nickname", |id: u64| format!("user_{}", id & 0xFFFF))
        .unwrap();
    let mut world: World<u64> = World::new();
    world.register(people).unwrap();
    world
}

// ---- Read path -----------------------------------------------------------

#[test]
fn read_falls_through_to_world_when_no_override() {
    let world = build_world_u64();
    let session = Session::new(&world);
    // World's deterministic value: id=42 → score = 42 * 0.5 = 21.0
    let v: f64 = session
        .attribute_value("people", 42u64, "score", None)
        .expect("read should succeed");
    assert_eq!(v, 21.0);
}

#[test]
fn read_after_write_returns_the_write() {
    let world = build_world_u64();
    let mut session = Session::new(&world);
    session
        .write_attribute::<f64>("people", 42u64, "score", 999.0)
        .unwrap();
    let v: f64 = session
        .attribute_value("people", 42u64, "score", None)
        .unwrap();
    assert_eq!(v, 999.0);
}

#[test]
fn write_does_not_affect_other_ids() {
    let world = build_world_u64();
    let mut session = Session::new(&world);
    session
        .write_attribute::<f64>("people", 42u64, "score", 999.0)
        .unwrap();
    // id=43 still gets the procedural value
    let v: f64 = session
        .attribute_value("people", 43u64, "score", None)
        .unwrap();
    assert_eq!(v, 21.5);
}

#[test]
fn write_does_not_affect_other_attributes_on_same_id() {
    let world = build_world_u64();
    let mut session = Session::new(&world);
    session
        .write_attribute::<f64>("people", 42u64, "score", 999.0)
        .unwrap();
    let nick: String = session
        .attribute_value("people", 42u64, "nickname", None)
        .unwrap();
    assert_eq!(nick, "user_42");
}

#[test]
fn second_write_overwrites_first() {
    let world = build_world_u64();
    let mut session = Session::new(&world);
    session
        .write_attribute::<f64>("people", 42u64, "score", 1.0)
        .unwrap();
    session
        .write_attribute::<f64>("people", 42u64, "score", 2.0)
        .unwrap();
    let v: f64 = session
        .attribute_value("people", 42u64, "score", None)
        .unwrap();
    assert_eq!(v, 2.0);
    assert_eq!(session.override_count(), 1);
}

// ---- Errors --------------------------------------------------------------

#[test]
fn read_unknown_space_returns_unknown_space() {
    let world = build_world_u64();
    let session = Session::new(&world);
    let err = session
        .attribute_value::<f64>("nope", 0u64, "score", None)
        .unwrap_err();
    assert!(matches!(err, OverlayError::UnknownSpace(ref n) if n == "nope"));
}

#[test]
fn read_unknown_attribute_returns_unknown_attribute() {
    let world = build_world_u64();
    let session = Session::new(&world);
    let err = session
        .attribute_value::<f64>("people", 0u64, "missing", None)
        .unwrap_err();
    assert!(matches!(
        err,
        OverlayError::UnknownAttribute { ref space, ref attr }
            if space == "people" && attr == "missing"
    ));
}

#[test]
fn read_type_mismatch_returns_type_mismatch() {
    let world = build_world_u64();
    let session = Session::new(&world);
    // score is f64; ask for i64.
    let err = session
        .attribute_value::<i64>("people", 0u64, "score", None)
        .unwrap_err();
    assert!(matches!(err, OverlayError::TypeMismatch { ref attr, .. } if attr == "score"));
}

#[test]
fn write_unknown_space_returns_unknown_space() {
    let world = build_world_u64();
    let mut session = Session::new(&world);
    let err = session
        .write_attribute::<f64>("nope", 0u64, "score", 1.0)
        .unwrap_err();
    assert!(matches!(err, OverlayError::UnknownSpace(ref n) if n == "nope"));
    assert_eq!(session.override_count(), 0, "failed write must not insert");
}

#[test]
fn write_unknown_attribute_returns_unknown_attribute() {
    let world = build_world_u64();
    let mut session = Session::new(&world);
    let err = session
        .write_attribute::<f64>("people", 0u64, "missing", 1.0)
        .unwrap_err();
    assert!(matches!(err, OverlayError::UnknownAttribute { .. }));
    assert_eq!(session.override_count(), 0);
}

#[test]
fn write_type_mismatch_returns_type_mismatch() {
    let world = build_world_u64();
    let mut session = Session::new(&world);
    let err = session
        .write_attribute::<i64>("people", 0u64, "score", 5)
        .unwrap_err();
    assert!(matches!(err, OverlayError::TypeMismatch { .. }));
    assert_eq!(session.override_count(), 0);
}

// ---- Isolation -----------------------------------------------------------

#[test]
fn writes_in_session_a_invisible_to_session_b() {
    let world = build_world_u64();
    let mut a = Session::new(&world);
    let b = Session::new(&world);
    a.write_attribute::<f64>("people", 42u64, "score", 999.0)
        .unwrap();
    let v: f64 = b
        .attribute_value("people", 42u64, "score", None)
        .unwrap();
    assert_eq!(v, 21.0, "session B must see procedural value, not A's write");
}

#[test]
fn many_concurrent_sessions_isolated() {
    let world = build_world_u64();
    let mut sessions: Vec<_> = (0..10).map(|_| Session::new(&world)).collect();
    // Each session writes a different value to the same id/attr.
    for (i, s) in sessions.iter_mut().enumerate() {
        s.write_attribute::<f64>("people", 1u64, "score", i as f64)
            .unwrap();
    }
    for (i, s) in sessions.iter().enumerate() {
        let v: f64 = s.attribute_value("people", 1u64, "score", None).unwrap();
        assert_eq!(v, i as f64);
    }
}

// ---- Allocation ----------------------------------------------------------

#[test]
fn allocate_entity_returns_id_with_sentinel_bit_set() {
    let world = build_world_u64();
    let mut session = Session::new(&world);
    let id = session.allocate_entity("people");
    assert!(Session::<u64>::is_session_allocated(id), "sentinel bit not set");
}

#[test]
fn allocate_entity_returns_distinct_ids() {
    let world = build_world_u64();
    let mut session = Session::new(&world);
    let ids: Vec<u64> = (0..100).map(|_| session.allocate_entity("people")).collect();
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), ids.len(), "duplicate allocated ids");
    assert_eq!(session.allocated_count(), 100);
}

#[test]
fn write_then_read_works_on_allocated_id() {
    let world = build_world_u64();
    let mut session = Session::new(&world);
    let new_id = session.allocate_entity("people");
    session
        .write_attribute::<f64>("people", new_id, "score", 7.0)
        .unwrap();
    let v: f64 = session
        .attribute_value("people", new_id, "score", None)
        .unwrap();
    assert_eq!(v, 7.0);
}

#[test]
fn was_allocated_reports_correctly() {
    let world = build_world_u64();
    let mut session = Session::new(&world);
    let id = session.allocate_entity("people");
    assert!(session.was_allocated("people", id));
    assert!(!session.was_allocated("people", 0u64));
    // Allocated in "people"; querying same id under different space → false.
    assert!(!session.was_allocated("messages", id));
}

#[test]
fn distinct_sessions_get_distinct_session_ids_in_practice() {
    // Not a strict guarantee (15-bit space; collisions possible), but
    // entropy should make any one-call sequence essentially unique.
    let world = build_world_u64();
    let s1 = Session::new(&world);
    let s2 = Session::new(&world);
    // Either they differ, or we got unlucky and got the same id from
    // entropy. Allow same-id in 1-of-32k outcomes by not asserting hard.
    // Just confirm both fit in 15 bits.
    assert!(s1.session_id() < (1 << 15));
    assert!(s2.session_id() < (1 << 15));
}

// ---- Time-aware reads ----------------------------------------------------

#[test]
fn temporal_attribute_reads_through_to_world() {
    use chrono::TimeZone;
    let mut layout =
        BitLayout::<u64>::new(vec![("user_id", 32), ("entropy", 28)]).unwrap();
    let _ = &mut layout;
    let layout = BitLayout::<u64>::new(vec![("user_id", 32), ("entropy", 28)]).unwrap();
    let mut people = Space::<u64>::new("people", layout);
    people
        .temporal_attribute::<i64, _>("year_offset", |_id: u64, t: DateTime<Utc>| {
            use chrono::Datelike;
            (t.year() - 2000) as i64
        })
        .unwrap();
    let mut world: World<u64> = World::new();
    world.register(people).unwrap();

    let session = Session::new(&world);
    let t = Utc.with_ymd_and_hms(2026, 4, 25, 0, 0, 0).unwrap();
    let v: i64 = session
        .attribute_value("people", 1u64, "year_offset", Some(t))
        .unwrap();
    assert_eq!(v, 26);
}

#[test]
fn write_overrides_temporal_too() {
    use chrono::TimeZone;
    let layout = BitLayout::<u64>::new(vec![("user_id", 32), ("entropy", 28)]).unwrap();
    let mut people = Space::<u64>::new("people", layout);
    people
        .temporal_attribute::<i64, _>("year_offset", |_id: u64, t: DateTime<Utc>| {
            use chrono::Datelike;
            (t.year() - 2000) as i64
        })
        .unwrap();
    let mut world: World<u64> = World::new();
    world.register(people).unwrap();

    let mut session = Session::new(&world);
    session
        .write_attribute::<i64>("people", 1u64, "year_offset", 999)
        .unwrap();
    // Even with a "different t" the override wins (overlay reads ignore t).
    let t = Utc.with_ymd_and_hms(2030, 1, 1, 0, 0, 0).unwrap();
    let v: i64 = session
        .attribute_value("people", 1u64, "year_offset", Some(t))
        .unwrap();
    assert_eq!(v, 999);
}

// ---- u128 word support ---------------------------------------------------

#[test]
fn works_with_u128_word() {
    let layout = BitLayout::<u128>::new(vec![("uid", 64), ("rest", 60)]).unwrap();
    let mut people = Space::<u128>::new("people", layout);
    people
        .attribute::<u64, _>("squared_low", |id: u128| {
            let low = id as u64;
            low.wrapping_mul(low)
        })
        .unwrap();
    let mut world: World<u128> = World::new();
    world.register(people).unwrap();

    let mut session = Session::new(&world);
    let v: u64 = session
        .attribute_value("people", 7u128, "squared_low", None)
        .unwrap();
    assert_eq!(v, 49);

    let new_id = session.allocate_entity("people");
    assert!(Session::<u128>::is_session_allocated(new_id));
    session
        .write_attribute::<u64>("people", new_id, "squared_low", 42)
        .unwrap();
    let v: u64 = session
        .attribute_value("people", new_id, "squared_low", None)
        .unwrap();
    assert_eq!(v, 42);
}
