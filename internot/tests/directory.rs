//! The directory service through the registry, as a transport calls it
//! (JSON in, JSON out), on the `us-tiny` world in 1985: records agree with
//! each other both ways (partners, parents and children, household members)
//! and with the world. Spec: `docs/superpowers/specs/2026-10-01-directory.md`.

use std::sync::OnceLock;

use chrono::{TimeZone, Utc};
use internot::{registry, Universe, ViewError, ViewRegistry};
use serde_json::{json, Value};

fn setup() -> &'static (Universe, ViewRegistry) {
    static U: OnceLock<(Universe, ViewRegistry)> = OnceLock::new();
    U.get_or_init(|| {
        let now = Utc.with_ymd_and_hms(1985, 6, 30, 9, 0, 0).single().unwrap();
        (Universe::for_pack("us-tiny", now), registry())
    })
}

fn call(view: &str, params: Value) -> Result<Value, ViewError> {
    let (u, reg) = setup();
    reg.get(view).expect("registered").execute(u, params)
}

fn ids(v: &Value, field: &str) -> Vec<u64> {
    v[field]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["person_id"].as_u64().unwrap())
        .collect()
}

#[test]
fn the_registry_serves_the_directory_and_not_people() {
    let (_, reg) = setup();
    for name in ["read_person", "read_household", "_get_trace"] {
        assert!(reg.get(name).is_some(), "{name} registered");
    }
    for old in ["find_people", "list_people_in_workplace", "read_career_history"] {
        assert!(reg.get(old).is_none(), "{old} is gone");
    }
}

#[test]
fn people_records_agree_both_ways() {
    let (u, _) = setup();
    let n = u.society.world.population();
    let (mut alive, mut partnered, mut households) = (0, 0, 0);
    for id in (0..n).step_by((n / 400) as usize) {
        let p = call("read_person", json!({ "person_id": id })).expect("a person");
        assert_eq!(p["person_id"].as_u64(), Some(id));
        assert!(!p["full_name"].as_str().unwrap().is_empty());
        if p["status"] == "not yet born" {
            continue;
        }
        // Parents list the person among their children, and the other way.
        for parent in ids(&p, "parents") {
            let q = call("read_person", json!({ "person_id": parent })).unwrap();
            assert!(ids(&q, "children").contains(&id), "{parent} lists child {id}");
        }
        for child in ids(&p, "children") {
            let c = call("read_person", json!({ "person_id": child })).unwrap();
            assert!(ids(&c, "parents").contains(&id), "{child} lists parent {id}");
        }
        if p["status"] != "alive" {
            continue;
        }
        alive += 1;
        // A partner's record has the person as partner.
        if let Some(q) = p["partner"]["person_id"].as_u64() {
            partnered += 1;
            let r = call("read_person", json!({ "person_id": q })).unwrap();
            assert_eq!(r["partner"]["person_id"].as_u64(), Some(id), "{q}'s partner");
        }
        // Every household member's household is the same, with the same
        // address, and the person in it.
        if let Some(h) = p["household"].as_object() {
            households += 1;
            let members: Vec<u64> = h["members"]
                .as_array()
                .unwrap()
                .iter()
                .map(|m| m["person"]["person_id"].as_u64().unwrap())
                .collect();
            assert!(members.contains(&id), "{id} lives in their household");
            for &m in &members {
                let hm = call("read_household", json!({ "person_id": m })).unwrap();
                let theirs: Vec<u64> = hm["members"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|m| m["person"]["person_id"].as_u64().unwrap())
                    .collect();
                assert_eq!(theirs.len(), members.len(), "{m} shares {id}'s household");
                assert_eq!(hm["address"], h["address"], "{m}'s address");
            }
            let me = h["members"]
                .as_array()
                .unwrap()
                .iter()
                .find(|m| m["person"]["person_id"].as_u64() == Some(id))
                .unwrap();
            assert_eq!(me["relation"], "self");
        }
    }
    println!("{alive} alive, {partnered} partnered, {households} with households");
    assert!(alive > 100 && partnered > 30 && households > 100);
}

#[test]
fn time_travel_and_errors() {
    let (u, _) = setup();
    // Someone born in the world: before birth, then alive, then perhaps dead.
    let w = &u.society.world;
    let id = (0..w.population() as u32)
        .find(|&x| !w.is_founder(x) && w.birth_year(x) == 1940)
        .expect("someone born in 1940");
    let before = call("read_person", json!({ "person_id": id, "at": "1930-01-01" })).unwrap();
    assert_eq!(before["status"], "not yet born");
    assert!(before["household"].is_null() && before["age"].is_null());
    let child = call("read_person", json!({ "person_id": id, "at": "1950-06-01" })).unwrap();
    assert!(matches!(child["age"].as_u64(), Some(9 | 10)));
    // Errors: an unknown id, an unreadable time.
    let n = w.population();
    assert!(matches!(
        call("read_person", json!({ "person_id": n })),
        Err(ViewError::NotFound(_))
    ));
    assert!(matches!(
        call("read_person", json!({ "person_id": 0, "at": "last tuesday" })),
        Err(ViewError::InvalidParams(_))
    ));
}
