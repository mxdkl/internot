//! Traces where a few never-partnered adults and their parents lived over
//! time, to see where distance between them comes from.
//! Run: cargo run --release -p internot_society --example residence_trace
use internot_society::residence::{Places, Pos, Residence, AREA, TRACT, ZONE};
use internot_society::{Params, PersonId, World};
use procedural_core::geo::haversine_miles;
use procedural_core::stream::{year_start, DAY};

fn main() {
    let w = World::build(Params::prototype(), 42);
    let r = Residence::build(&w, Places::from_params(&w.ledger().params).unwrap());
    let pl = r.places();
    let miles = |a: &Pos, b: &Pos| haversine_miles(pl.tract_at(a[TRACT]), pl.tract_at(b[TRACT]));
    let t = year_start(2013) + 180 * DAY;
    let mut x = 0x00c0_ffee_u64;
    let mut shown = 0;
    while shown < 8 {
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
        shown += 1;
        println!(
            "\nperson {id} born {}, spell0 {:?}, household {:?}",
            w.birth_year(id),
            r.spell0_start(&w, id).map(procedural_core::stream::year_of),
            w.household(id, t)
        );
        for year in (w.birth_year(id)..=2013).step_by(3) {
            let at = year_start(year) + 100 * DAY;
            let (Some(a), Some(b)) = (r.address_of(&w, id, at), r.address_of(&w, m, at)) else {
                continue;
            };
            println!("  {year}: self area {} zone {} tract {} | mother area {} zone {} | {:.0} mi | hh {:?}",
                a[AREA], a[ZONE], a[TRACT], b[AREA], b[ZONE], miles(&a, &b), w.household(id, at).map(|h| format!("{h:?}").chars().take(30).collect::<String>()));
        }
    }
}
