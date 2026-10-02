//! Where the distance between never-partnered adults and their mothers comes
//! from: at leaving home (formation), then the child's and the mother's
//! moves since. Run: cargo run --release -p internot_society --example residence_drift [pack]
use internot_society::residence::{Places, Pos, Residence, TRACT};
use internot_society::{Params, PersonId, World};
use procedural_core::geo::haversine_miles;
use procedural_core::stream::{year_start, DAY};

fn main() {
    let params = match std::env::args().nth(1) {
        Some(pack) => Params::load("worlds".as_ref(), &pack).expect("a pack"),
        None => Params::prototype(),
    };
    let w = World::build(params, 42);
    let r = Residence::build(&w, Places::from_params(&w.ledger().params).unwrap());
    let pl = r.places();
    let miles = |a: &Pos, b: &Pos| haversine_miles(pl.tract_at(a[TRACT]), pl.tract_at(b[TRACT]));
    let t = year_start(2013) + 180 * DAY;
    let mut x = 0x00c0_ffee_u64;
    let (mut n, mut at_leave, mut child_moved, mut mother_moved, mut now) =
        (0usize, vec![], vec![], vec![], vec![]);
    while n < 3000 {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        let id = (x % w.population()) as PersonId;
        if !w.present_at(id, t) || w.is_immigrant(id) || (t - w.birth(id)) < 25 * 365 * DAY {
            continue;
        }
        if w.unions(id).iter().flatten().any(|u| u.start <= t) {
            continue;
        }
        let Some(m) = w.mother(id).filter(|&m| w.present_at(m, t)) else {
            continue;
        };
        let Some(s0) = r.spell0_start(&w, id) else {
            continue;
        };
        if s0 > t {
            continue;
        }
        let (Some(c0), Some(m0), Some(c1), Some(m1)) = (
            r.address_of(&w, id, s0 + DAY),
            r.address_of(&w, m, s0 + DAY),
            r.address_of(&w, id, t),
            r.address_of(&w, m, t),
        ) else {
            continue;
        };
        n += 1;
        at_leave.push(miles(&c0, &m0));
        child_moved.push(miles(&c0, &c1));
        mother_moved.push(miles(&m0, &m1));
        now.push(miles(&c1, &m1));
    }
    let show = |label: &str, v: &mut Vec<f64>| {
        v.sort_by(|a, b| a.total_cmp(b));
        let q = |f: f64| v[((v.len() - 1) as f64 * f) as usize];
        let under = |d: f64| 100.0 * v.iter().filter(|&&x| x < d).count() as f64 / v.len() as f64;
        println!("{label:<28} p25 {:6.1} p50 {:6.1} p75 {:6.1} p90 {:7.1} mi; under 1 mi {:4.1}%, 30 mi {:4.1}%", q(0.25), q(0.5), q(0.75), q(0.9), under(1.0), under(30.0));
    };
    println!("{n} never-partnered adults 25+ in 2013 with a living mother");
    show("child–mother at leaving", &mut at_leave);
    show("child's move since", &mut child_moved);
    show("mother's move since", &mut mother_moved);
    show("child–mother now", &mut now);
}
