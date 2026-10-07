//! The directory service through the registry, as a transport calls it
//! (JSON in, JSON out), on the `us-tiny` world in 1985: records agree with
//! each other both ways (partners, unions, parents and children) and with
//! the world. Spec: `docs/superpowers/specs/2026-10-01-directory.md`.

use std::sync::OnceLock;

use chrono::{TimeZone, Utc};
use internot::{registry, Universe, ViewError, ViewRegistry};
use internot_society::mono::Pid;
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

/// Every 400th person ever born, as external ids.
fn sample() -> Vec<u64> {
    let (u, _) = setup();
    let w = &u.society.world;
    let all: Vec<Pid> = (0..w.cells())
        .flat_map(|cell| (w.first_year()..=w.last_year()).map(move |y| (cell, y)))
        .flat_map(|(cell, y)| (0..w.cohort_n(cell, y)).map(move |i| Pid { cell, y, i }))
        .collect();
    all.iter().step_by((all.len() / 400).max(1)).map(|&x| w.id(x)).collect()
}

#[test]
fn the_registry_serves_the_directory() {
    let (_, reg) = setup();
    for name in ["read_person", "_get_trace"] {
        assert!(reg.get(name).is_some(), "{name} registered");
    }
    for old in ["read_household", "find_people", "list_people_in_workplace"] {
        assert!(reg.get(old).is_none(), "{old} is gone");
    }
}

#[test]
fn people_records_agree_both_ways() {
    let (mut alive, mut partnered, mut unions) = (0, 0, 0);
    for id in sample() {
        let p = call("read_person", json!({ "person_id": id })).expect("a person");
        assert_eq!(p["person_id"].as_u64(), Some(id));
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
        // A union reads the same from both partners.
        for u in p["unions"].as_array().unwrap() {
            unions += 1;
            let q = u["partner"]["person_id"].as_u64().unwrap();
            let r = call("read_person", json!({ "person_id": q })).unwrap();
            let back = r["unions"].as_array().unwrap().iter().find(|v| v["partner"]["person_id"].as_u64() == Some(id));
            let back = back.unwrap_or_else(|| panic!("{q} lists the union with {id}"));
            assert_eq!(back["started"], u["started"]);
            assert_eq!(back["ended"], u["ended"]);
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
    }
    println!("{alive} alive, {partnered} partnered, {unions} unions");
    assert!(alive > 50 && partnered > 20 && unions > 100);
}

#[test]
fn time_travel_and_errors() {
    let (u, _) = setup();
    let w = &u.society.world;
    // Someone born in 1940 and alive in 1951: before birth, then a child.
    let x = (0..w.cohort_n(0, 1940))
        .map(|i| Pid { cell: 0, y: 1940, i })
        .find(|&x| w.death(x) > procedural_core::stream::year_start(1951))
        .expect("someone born in 1940 alive in 1951");
    let before = call("read_person", json!({ "person_id": w.id(x), "at": "1930-01-01" })).unwrap();
    assert_eq!(before["status"], "not yet born");
    assert!(before["age"].is_null() && before["unions"].as_array().unwrap().is_empty());
    let child = call("read_person", json!({ "person_id": w.id(x), "at": "1950-06-01" })).unwrap();
    assert!(matches!(child["age"].as_u64(), Some(9 | 10)), "{child}");
    // Errors: an unknown id, an unreadable time.
    let past = Pid { cell: 0, y: 1940, i: w.cohort_n(0, 1940) };
    assert!(matches!(call("read_person", json!({ "person_id": w.id(past) })), Err(ViewError::NotFound(_))));
    assert!(matches!(
        call("read_person", json!({ "person_id": w.id(x), "at": "last tuesday" })),
        Err(ViewError::InvalidParams(_))
    ));
}

#[test]
fn work_records_agree_with_the_career() {
    let (mut employed, mut jobs) = (0, 0);
    for id in sample() {
        let p = call("read_person", json!({ "person_id": id })).unwrap();
        let w = &p["work"];
        let as_of = p["as_of"].as_str().unwrap()[..10].to_string();
        if p["status"] != "alive" || p["age"].as_u64().unwrap_or(0) < 14 {
            assert!(w["status"].is_null() || p["status"] == "alive", "{id}: work status while {}", p["status"]);
        }
        if w["status"] == "employed" {
            employed += 1;
            let j = &w["job"];
            assert!(j["to"].is_null() && j["from"].as_str().unwrap() <= as_of.as_str(), "{id}: current job {j}");
            assert!(!j["title"].as_str().unwrap().is_empty() && !j["employer"].as_str().unwrap().is_empty());
        } else {
            assert!(w["job"].is_null(), "{id}: a job while {}", w["status"]);
        }
        // Earlier jobs, most recent first, each ended before the next began.
        let hist = w["history"].as_array().unwrap();
        jobs += hist.len();
        for pair in hist.windows(2) {
            let (later, earlier) = (&pair[0], &pair[1]);
            assert!(earlier["to"].as_str().unwrap() <= later["from"].as_str().unwrap(), "{id}: jobs overlap: {earlier} then {later}");
        }
        for j in hist {
            assert!(j["to"].as_str().unwrap() <= as_of.as_str(), "{id}: an earlier job ends after now: {j}");
            assert!(j["annual_pay"].as_u64().unwrap() > 0, "{id}: unpaid {j}");
        }
    }
    println!("{employed} employed, {jobs} earlier jobs");
    assert!(employed > 30 && jobs > 300);
}
