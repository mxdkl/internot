//! Names (N1) on the tiny world: every person, exhaustively. Names are pure
//! functions; surnames change only at weddings and separations; children's
//! surnames come from their parents; marriages are facts of the union.

use std::sync::OnceLock;

use internot_society::world::{year_start, DAY};
use internot_society::{Middle, Params, PersonId, Sex, Surname, World};

fn world() -> &'static World {
    static W: OnceLock<World> = OnceLock::new();
    W.get_or_init(|| World::build(Params::tiny(), 7))
}

fn everyone(w: &World) -> impl Iterator<Item = PersonId> + '_ {
    (0..w.population()).map(|x| x as PersonId)
}

#[test]
fn names_are_pure_functions() {
    let (a, b) = (world(), World::build(Params::tiny(), 7));
    let t = year_start(1985);
    for x in everyone(a).step_by(7) {
        assert_eq!(a.first_name(x), b.first_name(x));
        assert_eq!(a.middle_name_of(x), b.middle_name_of(x));
        assert_eq!(a.surname(x, t), b.surname(x, t));
    }
}

#[test]
fn everyone_has_a_name_and_middle_names_differ() {
    let w = world();
    let mut with_middle = 0;
    for x in everyone(w) {
        assert!(!w.first_name(x).is_empty(), "{x} has no first name");
        if let Some(m) = w.middle_name_of(x) {
            with_middle += 1;
            if let Middle::Given(id) = m {
                assert_ne!(id, w.first_name_id(x), "{x}: middle name = first name");
            }
        }
        assert!(!w.surname_text(w.birth_surname(x)).is_empty());
    }
    let share = with_middle as f64 / w.population() as f64;
    assert!(
        (0.6..=0.99).contains(&share),
        "middle-name share {share:.3}"
    );
}

#[test]
fn marriages_are_facts_of_the_union() {
    let w = world();
    let (mut unions, mut married) = (0u64, 0u64);
    for x in everyone(w) {
        for u in w.unions(x).into_iter().flatten() {
            unions += 1;
            let m = w.marriage_date(x, &u);
            // The same from the partner's side.
            let other = w
                .unions(u.partner)
                .into_iter()
                .flatten()
                .find(|v| v.partner == x && v.start == u.start)
                .expect("partners share the union");
            assert_eq!(
                m,
                w.marriage_date(u.partner, &other),
                "{x} and {}",
                u.partner
            );
            if let Some(m) = m {
                married += 1;
                assert!(u.start <= m && m < u.end, "a wedding outside its union");
            }
        }
    }
    let share = married as f64 / unions as f64;
    assert!((0.5..=0.99).contains(&share), "married share {share:.3}");
}

/// Every date at which `x`'s surname may change: weddings and separations.
fn events(w: &World, x: PersonId) -> Vec<i64> {
    let mut out = Vec::new();
    for u in w.unions(x).into_iter().flatten() {
        if let Some(m) = w.marriage_date(x, &u) {
            out.push(m);
            if let Some(s) = u.separation {
                out.push(s);
            }
        }
    }
    out.sort_unstable();
    out
}

#[test]
fn surnames_change_only_at_weddings_and_separations() {
    let w = world();
    let mut changed = 0u64;
    for x in everyone(w).step_by(3) {
        let birth = w.birth(x);
        let ev = events(w, x);
        assert_eq!(w.surname(x, birth), w.birth_surname(x), "{x} at birth");
        // Between consecutive events the surname is constant.
        let mut bounds = vec![birth];
        bounds.extend(ev.iter().copied());
        bounds.push(w.death(x));
        for win in bounds.windows(2) {
            let (a, b) = (win[0], win[1]);
            if b <= a {
                continue;
            }
            let s = w.surname(x, a);
            for t in [a + (b - a) / 3, a + 2 * (b - a) / 3, b - 1] {
                assert_eq!(w.surname(x, t), s, "{x} changed between events");
            }
        }
        changed += (w.surname(x, w.death(x) - 1) != w.birth_surname(x)) as u64;
    }
    assert!(changed > 100, "almost nobody's surname changes ({changed})");
}

#[test]
fn a_wedding_change_takes_the_partners_surname_as_it_was() {
    let w = world();
    let (mut takes, mut checked) = (0u64, 0u64);
    for x in everyone(w) {
        for u in w.unions(x).into_iter().flatten() {
            let Some(m) = w.marriage_date(x, &u) else {
                continue;
            };
            let (before, after) = (w.surname(x, m - 1), w.surname(x, m));
            if before == after {
                continue;
            }
            checked += 1;
            let partner = w.surname(u.partner, m - 1);
            let took = after == partner;
            let hyphenated = after
                == Surname {
                    first: before.first,
                    second: Some(partner.first),
                    hyphen: true,
                };
            assert!(
                took || hyphenated,
                "{x}: {after:?} from neither {partner:?} nor {before:?}"
            );
            takes += took as u64;
        }
    }
    assert!(checked > 100 && takes * 2 > checked, "{takes} of {checked}");
}

#[test]
fn children_take_a_parents_surname() {
    let w = world();
    let (mut kids, mut fathers) = (0u64, 0u64);
    for x in everyone(w) {
        let Some(m) = w.mother(x) else { continue };
        let birth = w.birth(x);
        let s = w.birth_surname(x);
        let mother_lines = [w.surname(m, birth).first, w.birth_surname(m).first];
        match w.father(x) {
            Some(f) => {
                kids += 1;
                let father_line = w.surname(f, birth).first;
                assert!(
                    s.first == father_line || mother_lines.contains(&s.first),
                    "{x}: surname from neither parent"
                );
                if let Some(second) = s.second {
                    assert_eq!(second, w.birth_surname(m).first, "{x}: second part");
                }
                fathers += (s.first == father_line) as u64;
            }
            None => assert_eq!(s, w.surname(m, birth), "{x}: no father, the mother's"),
        }
    }
    let share = fathers as f64 / kids as f64;
    assert!(share > 0.85, "father's surname share {share:.3}");
}

#[test]
fn first_names_follow_sex() {
    // Names come from the data for the person's sex. The data itself has a
    // few cross-sex records (SSA 1950: 180 girls named John, 119 boys named
    // Mary, out of about 1.7M each), so this is a rate, not a rule.
    let w = world();
    let t = year_start(1980) + 100 * DAY;
    let (mut women, mut men) = (0u64, 0u64);
    let (mut johns, mut marys, mut female_johns, mut male_marys) = (0u64, 0u64, 0u64, 0u64);
    for x in everyone(w).filter(|&x| w.alive_at(x, t)) {
        let female = w.sex(x) == Sex::Female;
        women += female as u64;
        men += !female as u64;
        match w.first_name(x) {
            "John" => {
                johns += 1;
                female_johns += female as u64;
            }
            "Mary" => {
                marys += 1;
                male_marys += !female as u64;
            }
            _ => {}
        }
    }
    assert!(johns > 100 && marys > 100, "{johns} {marys}");
    assert!(
        female_johns * 100 < johns,
        "{female_johns} of {johns} Johns are women"
    );
    assert!(
        male_marys * 100 < marys,
        "{male_marys} of {marys} Marys are men"
    );
    assert!(women > 0 && men > 0);
}
