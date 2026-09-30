//! Round-trip tests for `Session::into_trajectory` + `replay`.
//!
//! Invariant: a session's reads after replay match the original session's
//! reads exactly (for every (space, id, attr, t) the original could
//! answer).

use procedural_core::bits::BitLayout;
use procedural_core::space::Space;
use procedural_core::world::World;
use procedural_overlay::{replay, OverlayError, Session, Trajectory, TrajectoryEvent};

fn build_world() -> World<u64> {
    let layout = BitLayout::<u64>::new(vec![("user_id", 32), ("entropy", 28)]).unwrap();
    let mut people = Space::<u64>::new("people", layout);
    people
        .indexable_attribute::<u64, _>("user_id", "user_id", |b| b)
        .unwrap();
    people
        .attribute::<f64, _>("score", |id: u64| (id & 0xFFFF) as f64 * 0.5)
        .unwrap();
    people
        .attribute::<String, _>("nickname", |id: u64| format!("user_{}", id & 0xFFFF))
        .unwrap();
    let mut world: World<u64> = World::new();
    world.register(people).unwrap();
    world
}

#[test]
fn empty_session_round_trips() {
    let world = build_world();
    let s = Session::with_session_id(&world, 42);
    let t = s.into_trajectory();
    assert_eq!(t.session_id, 42);
    assert!(t.events.is_empty());
    let s2 = replay(&world, t).unwrap();
    assert_eq!(s2.session_id(), 42);
    assert_eq!(s2.override_count(), 0);
    assert_eq!(s2.allocated_count(), 0);
}

#[test]
fn writes_round_trip_exactly() {
    let world = build_world();
    let mut s = Session::with_session_id(&world, 7);
    s.write_attribute::<f64>("people", 1u64, "score", 100.0)
        .unwrap();
    s.write_attribute::<f64>("people", 2u64, "score", 200.0)
        .unwrap();
    s.write_attribute::<String>("people", 1u64, "nickname", "alice".into())
        .unwrap();

    let t = s.into_trajectory();
    assert_eq!(t.session_id, 7);
    assert_eq!(t.events.len(), 3);

    let s2 = replay(&world, t).unwrap();
    assert_eq!(s2.session_id(), 7);
    assert_eq!(s2.override_count(), 3);

    let v: f64 = s2.attribute_value("people", 1u64, "score", None).unwrap();
    assert_eq!(v, 100.0);
    let v: f64 = s2.attribute_value("people", 2u64, "score", None).unwrap();
    assert_eq!(v, 200.0);
    let n: String = s2
        .attribute_value("people", 1u64, "nickname", None)
        .unwrap();
    assert_eq!(n, "alice");
    // Unwritten id falls through to procedural in the replayed session too.
    let v: f64 = s2.attribute_value("people", 3u64, "score", None).unwrap();
    assert_eq!(v, 1.5);
}

#[test]
fn allocations_round_trip_and_subsequent_allocate_doesnt_collide() {
    let world = build_world();
    let mut s = Session::with_session_id(&world, 99);
    let id_a = s.allocate_entity("people");
    let id_b = s.allocate_entity("people");
    let id_c = s.allocate_entity("people");
    assert_ne!(id_a, id_b);
    assert_ne!(id_b, id_c);

    let t = s.into_trajectory();
    let mut s2 = replay(&world, t).unwrap();
    assert_eq!(s2.allocated_count(), 3);
    assert!(s2.was_allocated("people", id_a));
    assert!(s2.was_allocated("people", id_b));
    assert!(s2.was_allocated("people", id_c));

    // Allocator counter must have been bumped past the highest observed
    // id; new allocations must not collide with the originals.
    let id_d = s2.allocate_entity("people");
    let id_e = s2.allocate_entity("people");
    for prior in [id_a, id_b, id_c] {
        assert_ne!(id_d, prior);
        assert_ne!(id_e, prior);
    }
    assert_ne!(id_d, id_e);
}

#[test]
fn writes_on_allocated_ids_round_trip() {
    let world = build_world();
    let mut s = Session::with_session_id(&world, 5);
    let new_id = s.allocate_entity("people");
    s.write_attribute::<f64>("people", new_id, "score", 7.0)
        .unwrap();
    s.write_attribute::<String>("people", new_id, "nickname", "ghost".into())
        .unwrap();

    let t = s.into_trajectory();
    assert_eq!(t.events.len(), 3); // 1 alloc + 2 writes

    let s2 = replay(&world, t).unwrap();
    let v: f64 = s2
        .attribute_value("people", new_id, "score", None)
        .unwrap();
    assert_eq!(v, 7.0);
    let n: String = s2
        .attribute_value("people", new_id, "nickname", None)
        .unwrap();
    assert_eq!(n, "ghost");
    assert!(s2.was_allocated("people", new_id));
}

#[test]
fn replay_against_world_missing_space_errors() {
    // Build a session with a write, then try to replay against a world
    // that doesn't have the space. Must return UnknownSpace, not panic.
    let world1 = build_world();
    let mut s = Session::with_session_id(&world1, 1);
    s.write_attribute::<f64>("people", 1u64, "score", 1.0)
        .unwrap();
    let trajectory = s.into_trajectory();

    let world2: World<u64> = World::new(); // no spaces
    let err = replay(&world2, trajectory).unwrap_err();
    assert!(matches!(err, OverlayError::UnknownSpace(ref n) if n == "people"));
}

#[test]
fn replay_against_world_missing_attribute_errors() {
    let world1 = build_world();
    let mut s = Session::with_session_id(&world1, 1);
    s.write_attribute::<f64>("people", 1u64, "score", 1.0)
        .unwrap();
    let trajectory = s.into_trajectory();

    // Different world: same space name but no `score` attribute.
    let layout = BitLayout::<u64>::new(vec![("user_id", 32), ("entropy", 28)]).unwrap();
    let people_no_score = Space::<u64>::new("people", layout);
    let mut world2: World<u64> = World::new();
    world2.register(people_no_score).unwrap();

    let err = replay(&world2, trajectory).unwrap_err();
    assert!(matches!(
        err,
        OverlayError::UnknownAttribute { ref attr, .. } if attr == "score"
    ));
}

#[test]
fn into_trajectory_then_replay_preserves_session_id_for_id_allocation() {
    // The session_id round-trip is what makes allocated ids reproducible.
    let world = build_world();
    let mut s = Session::with_session_id(&world, 1234);
    let id_orig = s.allocate_entity("people");
    let t = s.into_trajectory();
    let mut s2 = replay(&world, t).unwrap();
    let id_new = s2.allocate_entity("people");
    // The replayed session should produce the next-counter id, NOT
    // collide with the observed one.
    assert_ne!(id_new, id_orig);
    // Both must carry session_id 1234 in their bits.
    assert!(Session::<u64>::is_session_allocated(id_orig));
    assert!(Session::<u64>::is_session_allocated(id_new));
}

#[test]
fn trajectory_event_debug_doesnt_panic() {
    let e: TrajectoryEvent<u64> = TrajectoryEvent::Allocated {
        space: "s".into(),
        id: 0u64,
    };
    let _ = format!("{:?}", e);
    let t: Trajectory<u64> = Trajectory {
        session_id: 0,
        events: vec![e],
    };
    let _ = format!("{:?}", t);
}

#[test]
fn round_trip_with_u128_word() {
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

    let mut s = Session::with_session_id(&world, 17);
    let id = s.allocate_entity("people");
    s.write_attribute::<u64>("people", id, "squared_low", 999)
        .unwrap();
    s.write_attribute::<u64>("people", 5u128, "squared_low", 12345)
        .unwrap();

    let t = s.into_trajectory();
    let s2 = replay(&world, t).unwrap();
    let v: u64 = s2
        .attribute_value("people", id, "squared_low", None)
        .unwrap();
    assert_eq!(v, 999);
    let v: u64 = s2
        .attribute_value("people", 5u128, "squared_low", None)
        .unwrap();
    assert_eq!(v, 12345);
}
