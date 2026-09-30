#![allow(deprecated)] // exercises graph::stable_roommates_match until it is removed
//! `graph_demo` — exercise every Phase 0 primitive on a tiny synthetic
//! household. Self-contained; no `internot` dependency.
//!
//! Run with:  cargo run --release --example graph_demo -p procedural_core
//!
//! What you'll see:
//!   1. A small "cohort" of 8 people procedurally paired into 4 couples
//!      via stable-roommates matching (symmetric by construction).
//!   2. A married couple's tie strength tracked through marriage,
//!      cohabitation, divorce, and post-divorce decay — same function
//!      composed differently per phase.
//!   3. Communication intensity for the couple at noon Monday vs.
//!      3 AM Sunday, showing diurnal × weekly modulation.
//!   4. Twelve months of procedural mail + chat events between them,
//!      enumerated deterministically.
//!
//! This is the cleanest way to see how the primitives compose. The
//! same machinery will drive `internot::social` in Phase 1.

use chrono::{DateTime, Datelike, TimeZone, Timelike, Utc};

use procedural_core::graph::{
    canonical_pair, comm_intensity, enumerate_events, pair_hash_float, stable_roommates_match,
    tie_strength, CommIntensity, PersonalityProjection, TieStrengthProfile,
};

fn main() {
    println!("\n=== procedural_core::graph demo ===\n");

    // -------------------------------------------------------------
    // 1. Build a tiny cohort and pair it up.
    // -------------------------------------------------------------
    let cohort: Vec<u32> = vec![100, 200, 300, 400, 500, 600, 700, 800];
    let pref = |a: u32, b: u32| pair_hash_float(a, b, "marriage:v1");
    let pairs = stable_roommates_match(&cohort, pref);

    println!("Step 1 — stable-roommates match ({} people)", cohort.len());
    println!("  Cohort: {:?}", cohort);
    println!("  Pairs:");
    for (a, b) in &pairs {
        let p = pair_hash_float(*a, *b, "marriage:v1");
        println!("    ({a}, {b})   mutual-pref hash = {p:.4}");
    }
    println!(
        "  Output is canonical (lo < hi) and symmetric: \
         calling with [800, 700, ...] yields the same pairs.\n"
    );

    // Pick the first couple to follow through life events.
    let (eli, jamie) = pairs[0];

    // -------------------------------------------------------------
    // 2. Tie strength through the relationship's lifecycle.
    // -------------------------------------------------------------
    // Place marriage 5 years into our day axis so "before marriage"
    // can be a real positive u32 day.
    let marriage_day: u32 = 365 * 5;
    let divorce_day: u32 = marriage_day + 365 * 12;
    let profile = TieStrengthProfile {
        base_floor: 0.10,       // residual tie even post-divorce
        cohabit_peak: 1.00,     // partner during marriage
        cohabit_start_day: Some(marriage_day),
        cohabit_end_day: Some(divorce_day),
        decay_tau_days: 182.5,  // 6 months τ — Roberts & Dunbar 2011 post-divorce
    };

    let timeline = [
        ("3 years before marriage",  marriage_day - 365 * 3),
        ("day before marriage",      marriage_day - 1),
        ("marriage day",             marriage_day),
        ("5 years into marriage",    marriage_day + 365 * 5),
        ("last day of marriage",     divorce_day),
        ("1 day post-divorce",       divorce_day + 1),
        ("1 month post-divorce",     divorce_day + 30),
        ("1 year post-divorce",      divorce_day + 365),
        ("5 years post-divorce",     divorce_day + 365 * 5),
    ];

    println!("Step 2 — partner tie strength over lifecycle (couple {eli}↔{jamie})");
    println!("  base_floor=0.10  cohabit_peak=1.00  τ=6 months");
    for (label, t) in timeline {
        let s = tie_strength(&profile, t);
        let bar = "█".repeat((s * 40.0) as usize);
        println!("    {label:24}  (day {t:>5})   strength {s:.3}  {bar}");
    }
    println!();

    // -------------------------------------------------------------
    // 3. Communication intensity — diurnal × weekly × personality.
    // -------------------------------------------------------------
    // Pick a moment 5 years into the marriage, strength near peak.
    let strength = tie_strength(&profile, 365 * 5);

    // Personalities: extraverted, conscientious, mid-chronotype.
    let eli_p = PersonalityProjection {
        extraversion: 0.85,
        conscientiousness: 0.70,
        chronotype: 0.40, // slightly early-bird
    };
    let jamie_p = PersonalityProjection {
        extraversion: 0.30,
        conscientiousness: 0.55,
        chronotype: 0.75, // night-owl
    };

    let monday_noon = utc(2025, 6, 2, 12);     // Mon (2025-06-02)
    let sunday_3am = utc(2025, 6, 1, 3);       // Sun (2025-06-01)
    let saturday_4pm = utc(2025, 6, 7, 16);    // Sat

    println!("Step 3 — comm intensity (events/day) — strength {strength:.3}");
    println!(
        "  Eli (E={:.2}, C={:.2}, chrono={:.2})",
        eli_p.extraversion, eli_p.conscientiousness, eli_p.chronotype
    );
    println!(
        "  Jamie (E={:.2}, C={:.2}, chrono={:.2})",
        jamie_p.extraversion, jamie_p.conscientiousness, jamie_p.chronotype
    );
    println!();
    println!(
        "    {:<22}  {:>6}  {:>6}  {:>6}",
        "moment", "mail", "chat", "cal"
    );
    for (label, t) in [
        ("Mon 12:00 (weekday)", monday_noon),
        ("Sun 03:00 (deep night)", sunday_3am),
        ("Sat 16:00 (weekend pm)", saturday_4pm),
    ] {
        let i = comm_intensity(strength, &eli_p, &jamie_p, t);
        print_intensity_row(label, &i);
    }
    println!();

    // -------------------------------------------------------------
    // 4. Enumerate procedural events for a year of mail + chat.
    // -------------------------------------------------------------
    // Use a coarse intensity function that respects diurnal + weekly
    // for the whole year. Each call to `intensity` here builds a
    // CommIntensity at that moment; the closure picks the right channel.
    let t_start = utc(2025, 1, 1, 0);
    let t_end = utc(2026, 1, 1, 0);

    let mail_lambda = |t: DateTime<Utc>| {
        comm_intensity(strength, &eli_p, &jamie_p, t).mail_per_day
    };
    let chat_lambda = |t: DateTime<Utc>| {
        comm_intensity(strength, &eli_p, &jamie_p, t).chat_per_day
    };

    let mail_events = enumerate_events(eli, jamie, "mail", mail_lambda, t_start, t_end);
    let chat_events = enumerate_events(eli, jamie, "chat", chat_lambda, t_start, t_end);

    println!(
        "Step 4 — one year of procedural events ({} → {})",
        t_start.format("%Y-%m-%d"),
        t_end.format("%Y-%m-%d")
    );
    println!(
        "  mail: {} events ({:.1}/day)",
        mail_events.len(),
        mail_events.len() as f64 / 365.0
    );
    println!(
        "  chat: {} events ({:.1}/day)",
        chat_events.len(),
        chat_events.len() as f64 / 365.0
    );

    println!("\n  First five mail events:");
    for e in mail_events.iter().take(5) {
        let who = if e.initiator == 0 { eli } else { jamie };
        let dow = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"][e.at.weekday().num_days_from_monday() as usize];
        println!(
            "    {} {} {:02}:{:02}   from {}",
            e.at.format("%Y-%m-%d"),
            dow,
            e.at.hour(),
            e.at.minute(),
            who
        );
    }
    println!("\n  First five chat events:");
    for e in chat_events.iter().take(5) {
        let who = if e.initiator == 0 { eli } else { jamie };
        let dow = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"][e.at.weekday().num_days_from_monday() as usize];
        println!(
            "    {} {} {:02}:{:02}   from {}",
            e.at.format("%Y-%m-%d"),
            dow,
            e.at.hour(),
            e.at.minute(),
            who
        );
    }

    // -------------------------------------------------------------
    // 5. Reproducibility check.
    // -------------------------------------------------------------
    let mail_events2 = enumerate_events(eli, jamie, "mail", mail_lambda, t_start, t_end);
    assert_eq!(mail_events, mail_events2, "event stream must be reproducible");
    let pairs2 = stable_roommates_match(&cohort, pref);
    assert_eq!(pairs, pairs2, "matching must be deterministic");
    let pairs_swapped = stable_roommates_match(&cohort.iter().rev().copied().collect::<Vec<_>>(), pref);
    assert_eq!(pairs, pairs_swapped, "matching must be order-invariant");
    let venue = canonical_pair(eli, jamie);
    assert_eq!(venue, canonical_pair(jamie, eli), "canonical_pair must be symmetric");

    println!(
        "\nStep 5 — invariants verified:\n  \
         • event streams reproduce bit-for-bit\n  \
         • matching is deterministic\n  \
         • matching is order-invariant in the cohort\n  \
         • canonical_pair is symmetric in (a, b)\n"
    );
    println!("=== done ===\n");
}

fn utc(y: i32, m: u32, d: u32, h: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(y, m, d, h, 0, 0).unwrap()
}

fn print_intensity_row(label: &str, i: &CommIntensity) {
    println!(
        "    {:<22}  {:>6.3}  {:>6.3}  {:>6.3}",
        label, i.mail_per_day, i.chat_per_day, i.calendar_per_day
    );
}
