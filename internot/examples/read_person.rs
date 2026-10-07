//! Print `read_person` for a person, as an MCP client sees it:
//! `cargo run --release -p internot --example read_person -- [ID] [AT]`
//! (default: someone born in 1960, read at 2023-01-01).
use chrono::{TimeZone, Utc};
use internot::Universe;
use internot_society::mono::Pid;

fn main() {
    let u = Universe::with_now(Utc.with_ymd_and_hms(2023, 1, 1, 12, 0, 0).unwrap());
    let w = &u.society.world;
    let args: Vec<String> = std::env::args().collect();
    let id = args.get(1).and_then(|s| s.parse().ok()).unwrap_or_else(|| w.id(Pid { cell: 0, y: 1960, i: 31_337 }));
    let mut params = serde_json::json!({ "person_id": id });
    if let Some(at) = args.get(2) {
        params["at"] = at.clone().into();
    }
    let reg = internot::registry();
    let view = reg.get("read_person").expect("registered");
    let out = view.execute(&u, params).expect("a person");
    println!("{}", serde_json::to_string_pretty(&out).unwrap());
}
