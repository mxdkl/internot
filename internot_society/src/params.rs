//! World definitions for the society layers: the typed sections of a world
//! pack (spec `docs/superpowers/specs/2026-10-01-world-packs.md`).
//!
//! Every number the population model uses comes from a pack (`worlds/us/`
//! and the packs that extend it), next to a comment naming its source. This
//! module defines the sections, checks them, and gives each the methods the
//! world calls (`params.mortality.siler(...)`). Sections no world reads yet
//! (households, residence, places, repartnering, immigration) are kept for
//! the features being rebuilt on the monotone world. Each
//! method keeps a fixed arithmetic, so worlds are bit-identical across
//! machines; all transcendental math goes through `procedural_core::dmath`.
//!
//! What stays in code is mechanism, and the structural bounds below, which
//! size arrays on hot paths.

use std::path::Path;

use internot_def::{
    Bands, BySex, DefError, Dir, Embedded, EmbeddedFile, Pack, Ranges, Series, Steps, VecSeries,
};
use procedural_core::curve::{
    dilated, gaussian_bump, gaussian_kernel, log_logistic_cdf, logistic_floor_quantile,
    logistic_rise, ramp, rogers_castro_labour, symmetrized,
};
use procedural_core::fit::tilt_mean;
use procedural_core::interp::{bracket, lerp, log_lerp, piecewise_linear};
use procedural_core::life::{cure_hazard, Siler};
use procedural_core::pmf::{floored, mix_into, normalized, one_fewer_or_none, spread_evenly};
use serde::Deserialize;

/// The packs compiled into the binary (`worlds/` at build time).
static EMBEDDED: &[EmbeddedFile] = include!(concat!(env!("OUT_DIR"), "/embedded_packs.rs"));

/// Sex. `Female` first: women's rows index union tables.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Sex {
    Female,
    Male,
}

impl Sex {
    pub fn is_female(self) -> bool {
        self == Sex::Female
    }
}

// --- structural bounds ----------------------------------------------------------
//
// These size arrays and bound loops on hot paths, so they are code. A pack is
// checked against them; raising one is a code change (worlds/README.md).

/// Maximum modelled age.
pub const MAX_AGE: u32 = 110;
/// Youngest age at which a union starts, so everyone is at least 15 on the
/// union date. Birth offsets from a union fit 32 bits because of it (a pack's
/// `first_union.min_age` may be higher, not lower).
pub const MIN_UNION_AGE: i32 = 16;
/// Oldest age at arrival.
pub const MAX_ARRIVAL_AGE: i32 = 80;
/// Age past which nobody re-partners: where the hazard is negligible, which
/// keeps the death draw's fast path. Not a rule about people.
pub const MAX_REMARRIAGE_AGE: i32 = 95;
/// The fertile window: births happen at mother's ages `MIN_BIRTH_AGE..=MAX_BIRTH_AGE`
/// (it sizes the parent slots).
pub const MIN_BIRTH_AGE: i32 = 15;
pub const MAX_BIRTH_AGE: i32 = 45;
/// Dissolution bands (years after the union year): `[lo, hi)`; `None` means
/// the union lasts until a partner dies. The pack sets each band's share.
pub const DISSOLUTION_BANDS: [Option<(u32, u32)>; 6] = [
    None,
    Some((1, 4)),
    Some((4, 8)),
    Some((8, 13)),
    Some((13, 21)),
    Some((21, 41)),
];
/// Dissolution classes (R1c): `0` means the union lasts until a partner
/// dies; `k` in `1..=MAX_CLASS` means the couple separates in the `k`-th
/// calendar year after the union year (after arrival, for couples who
/// arrive together).
pub const MAX_CLASS: usize = 40;
/// People per roommate frame (the removed ledger world's households; kept
/// with the pack's `households.ron` for their rebuild).
pub const ROOMMATE_FRAME: usize = 12;

// --- the world ----------------------------------------------------------------------

/// A compiled world definition: everything a world is built from, besides
/// its seed.
#[derive(Clone, Debug)]
pub struct Params {
    /// The pack's name and fingerprint (see [`Pack::fingerprint`]).
    pub pack: String,
    pub fingerprint: u64,
    /// Display name.
    pub name: String,
    /// First simulated year. People alive then are founders.
    pub y0: i32,
    /// Last simulated year (inclusive). Blocks exist for births up to it.
    pub y1: i32,
    /// Oldest founder age at `y0`.
    pub founder_max_age: i32,
    /// Expected births in the year before `y0`. Sets the world's scale.
    pub founder_births: f64,
    /// Growth rate of the stable population founders are drawn from.
    pub founder_growth: f64,
    /// Share of births that are male.
    pub male_share_at_birth: f64,
    /// Lineage regions (at least one). A child belongs to its mother's
    /// region (R1 plan D-R1.1).
    pub regions: Vec<Region>,
    /// Test boosts (1 and 0 in realistic worlds): multiplier on the same-sex
    /// share, floor on every heritage share, multiplier on open-market shares.
    pub same_sex_boost: f64,
    pub heritage_floor: f64,
    pub open_market_boost: f64,
    pub mortality: Mortality,
    pub unions: Unions,
    pub repartnering: Repartnering,
    pub fertility: Fertility,
    pub dissolution: Dissolution,
    pub immigration: Immigration,
    pub heritage: Heritages,
    pub households: Households,
    pub residence: Residence,
    pub places: PlacesSection,
    /// The place tree's data (`places.ron`'s `data` file), parsed by
    /// [`crate::residence::Places::from_params`].
    pub place_data: std::sync::Arc<[u8]>,
    pub names: Names,
    /// The pack's name tables (`names.data`), shared by every world built
    /// from the same bytes.
    pub name_data: std::sync::Arc<crate::names::NameData>,
    pub education: Education,
    /// The schools and colleges data (`education.data`).
    pub school_data: std::sync::Arc<[u8]>,
    pub work: Work,
    /// The occupations data (`work.data`).
    pub work_data: std::sync::Arc<[u8]>,
}

/// One lineage region.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Region {
    pub id: String,
    /// Short name, for reports.
    pub name: String,
    /// Relative share of the founder population.
    pub founder_weight: f64,
    /// Relative share of immigrants arriving from abroad.
    pub immigrant_weight: f64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WorldSection {
    name: String,
    #[allow(dead_code)]
    description: String,
    #[allow(dead_code)]
    extends: Option<String>,
    timeline: Timeline,
    founders: Founders,
    male_share_at_birth: f64,
    regions: Vec<Region>,
    test_boosts: TestBoosts,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Timeline {
    first_year: i32,
    last_year: i32,
    founder_max_age: i32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Founders {
    births: f64,
    growth: f64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TestBoosts {
    same_sex: f64,
    heritage_floor: f64,
    open_market: f64,
}

impl Params {
    /// The `us` pack: the United States, 1840–2100, at prototype scale.
    pub fn prototype() -> Self {
        Self::embedded("us").unwrap_or_else(|e| panic!("{e}"))
    }

    /// The `us-tiny` pack: a small world for exhaustive tests.
    pub fn tiny() -> Self {
        Self::embedded("us-tiny").unwrap_or_else(|e| panic!("{e}"))
    }

    /// A pack compiled into the binary.
    pub fn embedded(name: &str) -> Result<Self, DefError> {
        Self::from_pack(&Pack::load(name, &[&Embedded(EMBEDDED)])?)
    }

    /// Pack `name` from directory `root` (packs are `root/<name>/`). Its
    /// `extends` chain may reach packs in `root` or embedded ones.
    pub fn load(root: &Path, name: &str) -> Result<Self, DefError> {
        Self::from_pack(&Pack::load(
            name,
            &[&Dir(root.to_path_buf()), &Embedded(EMBEDDED)],
        )?)
    }

    /// Read and check every section of a loaded pack.
    pub fn from_pack(pack: &Pack) -> Result<Self, DefError> {
        let w: WorldSection = pack.section("world")?;
        let p = Params {
            pack: pack.name().to_string(),
            fingerprint: pack.fingerprint(),
            name: w.name,
            y0: w.timeline.first_year,
            y1: w.timeline.last_year,
            founder_max_age: w.timeline.founder_max_age,
            founder_births: w.founders.births,
            founder_growth: w.founders.growth,
            male_share_at_birth: w.male_share_at_birth,
            regions: w.regions,
            same_sex_boost: w.test_boosts.same_sex,
            heritage_floor: w.test_boosts.heritage_floor,
            open_market_boost: w.test_boosts.open_market,
            mortality: pack.section("mortality")?,
            unions: pack.section("unions")?,
            repartnering: pack.section("repartnering")?,
            fertility: pack.section("fertility")?,
            dissolution: pack.section("dissolution")?,
            immigration: pack.section("immigration")?,
            heritage: pack.section("heritage")?,
            households: pack.section("households")?,
            residence: pack.section("residence")?,
            places: pack.section("places")?,
            place_data: std::sync::Arc::from(&[][..]),
            names: pack.section("names")?,
            name_data: std::sync::Arc::new(crate::names::NameData::empty()),
            education: pack.section("education")?,
            school_data: std::sync::Arc::from(&[][..]),
            work: pack.section("work")?,
            work_data: std::sync::Arc::from(&[][..]),
        };
        let mut p = p;
        p.place_data = std::sync::Arc::from(pack.data(&p.places.data)?);
        p.school_data = std::sync::Arc::from(pack.data(&p.education.data)?);
        p.work_data = std::sync::Arc::from(pack.data(&p.work.data)?);
        p.name_data = crate::names::NameData::cached(pack.data(&p.names.data)?)
            .map_err(|m| DefError::invalid(pack.name(), &p.names.data, "", m))?;
        p.validate(pack.name())?;
        Ok(p)
    }

    /// Number of lineage regions.
    pub fn region_count(&self) -> usize {
        self.regions.len()
    }

    /// Number of heritage groups.
    pub fn heritage_count(&self) -> usize {
        self.heritage.groups.len()
    }

    /// Number of lineage groups: region × heritage. Group `g` is region
    /// `g / heritage_count()` and heritage `g % heritage_count()`.
    pub fn group_count(&self) -> usize {
        self.regions.len() * self.heritage_count()
    }

    /// `mix` with every share raised to at least [`Params::heritage_floor`],
    /// renormalized.
    pub fn floored(&self, mix: &[f64]) -> Vec<f64> {
        floored(mix, self.heritage_floor)
    }

    fn validate(&self, pack: &str) -> Result<(), DefError> {
        let err = |file: &str, at: &str, msg: String| DefError::invalid(pack, file, at, msg);
        let check = |file: &str, at: &str, r: Result<(), String>| r.map_err(|m| err(file, at, m));
        let share = |file: &str, at: &str, x: f64| {
            check(
                file,
                at,
                if (0.0..=1.0).contains(&x) {
                    Ok(())
                } else {
                    Err(format!("{x} is not a share in [0, 1]"))
                },
            )
        };

        // world.ron
        if self.y0 >= self.y1 || self.founder_max_age < 1 || self.founder_max_age > MAX_AGE as i32 {
            return Err(err(
                "world.ron",
                "timeline",
                "needs first_year < last_year and founder_max_age in 1..=110".into(),
            ));
        }
        if !(self.founder_births > 0.0 && self.founder_growth.is_finite()) {
            return Err(err(
                "world.ron",
                "founders",
                "births must be positive and growth finite".into(),
            ));
        }
        share("world.ron", "male_share_at_birth", self.male_share_at_birth)?;
        if self.regions.is_empty() {
            return Err(err(
                "world.ron",
                "regions",
                "a world needs at least one region".into(),
            ));
        }
        unique_ids(self.regions.iter().map(|r| r.id.as_str()))
            .map_err(|m| err("world.ron", "regions", m))?;
        if self
            .regions
            .iter()
            .any(|r| !(r.founder_weight >= 0.0 && r.immigrant_weight >= 0.0))
        {
            return Err(err(
                "world.ron",
                "regions",
                "weights must be non-negative".into(),
            ));
        }
        if !(self.same_sex_boost >= 0.0 && self.open_market_boost >= 0.0) {
            return Err(err(
                "world.ron",
                "test_boosts",
                "boosts must be non-negative".into(),
            ));
        }
        share(
            "world.ron",
            "test_boosts.heritage_floor",
            self.heritage_floor,
        )?;

        // mortality.ron
        let m = &self.mortality;
        if m.multipliers.is_empty() || m.multipliers.windows(2).any(|w| w[0].year >= w[1].year) {
            return Err(err(
                "mortality.ron",
                "multipliers",
                "needs anchors in increasing year order".into(),
            ));
        }
        if m.multipliers.iter().any(|a| {
            [a.infant, a.background_female, a.background_male, a.old]
                .iter()
                .any(|x| x.is_nan() || *x <= 0.0)
        }) {
            return Err(err(
                "mortality.ron",
                "multipliers",
                "multipliers must be positive (they interpolate geometrically)".into(),
            ));
        }

        // unions.ron
        let u = &self.unions;
        if u.first_union.min_age < MIN_UNION_AGE {
            return Err(err(
                "unions.ron",
                "first_union.min_age",
                format!("must be at least {MIN_UNION_AGE} (the code's bound)"),
            ));
        }
        check(
            "unions.ron",
            "first_union.median_female",
            u.first_union.median_female.validate(),
        )?;
        check(
            "unions.ron",
            "first_union.ever_partnered",
            u.first_union.ever_partnered.validate_within(0.0, 1.0),
        )?;
        if u.first_union
            .median_female
            .0
            .iter()
            .any(|a| a.1 <= u.first_union.origin)
        {
            return Err(err(
                "unions.ron",
                "first_union.median_female",
                "medians must exceed `origin`".into(),
            ));
        }
        check(
            "unions.ron",
            "national_market_share",
            u.national_market_share.validate_within(0.0, 1.0),
        )?;
        check(
            "unions.ron",
            "same_sex_share",
            u.same_sex_share.validate_within(0.0, 0.5),
        )?;
        check("unions.ron", "age_gap", u.age_gap.validate())?;

        let mar = &u.marriage;
        check(
            "unions.ron",
            "marriage.at_start",
            mar.at_start.validate_within(0.0, 1.0),
        )?;
        check(
            "unions.ron",
            "marriage.later",
            mar.later.validate_within(0.0, 1.0),
        )?;
        check(
            "unions.ron",
            "marriage.same_sex_legal",
            mar.same_sex_legal.validate_within(0.0, 1.0),
        )?;
        if mar.delay_years.is_nan() || mar.delay_years <= 0.0 {
            return Err(err(
                "unions.ron",
                "marriage.delay_years",
                "must be positive".into(),
            ));
        }

        // repartnering.ron
        let r = &self.repartnering;
        check("repartnering.ron", "duration", r.duration.validate())?;
        check("repartnering.ron", "age_factor", r.age_factor.validate())?;
        check("repartnering.ron", "era", r.era.validate())?;
        share("repartnering.ron", "cap", r.cap)?;

        // fertility.ron
        let f = &self.fertility;
        check(
            "fertility.ron",
            "union_parity",
            f.union_parity.validate_pmf(9),
        )?;
        share(
            "fertility.ron",
            "second_union_fertile",
            f.second_union_fertile,
        )?;
        for (at, v) in [
            ("first_birth.historical", &f.first_birth.historical),
            ("first_birth.modern", &f.first_birth.modern),
        ] {
            check(
                "fertility.ron",
                at,
                VecSeries(vec![(0, v.clone())]).validate_pmf(4),
            )?;
        }
        check(
            "fertility.ron",
            "first_birth.shift",
            f.first_birth.shift.validate_within(0.0, 1.0),
        )?;
        if f.spacing.is_empty()
            || f.spacing
                .iter()
                .any(|s| s.gaps.0 < 1 || s.gaps.1 < 1 || s.weight.is_nan() || s.weight < 0.0)
        {
            return Err(err(
                "fertility.ron",
                "spacing",
                "needs patterns with gaps of at least a year and non-negative weights".into(),
            ));
        }
        check(
            "fertility.ron",
            "nonunion.one",
            f.nonunion.one.validate_within(0.0, 0.8),
        )?;
        if f.nonunion.second_gaps.is_empty() || f.nonunion.second_gaps.contains(&0) {
            return Err(err(
                "fertility.ron",
                "nonunion.second_gaps",
                "needs gaps of at least a year".into(),
            ));
        }
        if f.nonunion.age_variance.is_nan()
            || f.nonunion.age_variance <= 0.0
            || f.gestation_days < 1
        {
            return Err(err(
                "fertility.ron",
                "nonunion",
                "age_variance and gestation_days must be positive".into(),
            ));
        }

        // dissolution.ron
        let d = &self.dissolution;
        check(
            "dissolution.ron",
            "separation_share",
            d.separation_share.validate_within(0.0, 1.0),
        )?;
        for (at, v) in [
            ("bands.early", &d.bands.early),
            ("bands.late", &d.bands.late),
        ] {
            check(
                "dissolution.ron",
                at,
                VecSeries(vec![(0, v.clone())]).validate_pmf(DISSOLUTION_BANDS.len() - 1),
            )?;
        }
        check(
            "dissolution.ron",
            "bands.shift",
            d.bands.shift.validate_within(0.0, 1.0),
        )?;
        share("dissolution.ron", "max_share", d.max_share)?;

        // immigration.ron
        let im = &self.immigration;
        check(
            "immigration.ron",
            "rate.anchors",
            im.rate.anchors.validate_within(0.0, 0.1),
        )?;
        check(
            "immigration.ron",
            "couple_share",
            im.couple_share.validate_within(0.0, 1.0),
        )?;
        check(
            "immigration.ron",
            "male_share",
            im.male_share.validate_within(0.0, 1.0),
        )?;
        if !(1..MAX_ARRIVAL_AGE).contains(&im.min_age)
            || !(16..MAX_ARRIVAL_AGE).contains(&im.min_couple_age)
            || !(0..=31).contains(&im.max_child_age)
            || im.max_husband_younger < 0
        {
            return Err(err(
                "immigration.ron",
                "ages",
                "ages must lie within 1..80 (child ages within 0..=31)".into(),
            ));
        }

        // heritage.ron
        let h = &self.heritage;
        if h.groups.is_empty() || h.groups.len() > 64 {
            return Err(err(
                "heritage.ron",
                "groups",
                "a world needs 1 to 64 heritage groups".into(),
            ));
        }
        unique_ids(h.groups.iter().map(|g| g.id.as_str()))
            .map_err(|m| err("heritage.ron", "groups", m))?;
        for g in &h.groups {
            let at = |field: &str| format!("groups[{}].{field}", g.id);
            check(
                "heritage.ron",
                &at("founder_share"),
                g.founder_share.validate_within(0.0, f64::MAX),
            )?;
            check(
                "heritage.ron",
                &at("immigrant_share"),
                g.immigrant_share.validate_within(0.0, f64::MAX),
            )?;
            check(
                "heritage.ron",
                &at("open_market.female"),
                g.open_market.female.validate_within(0.0, 1.0),
            )?;
            check(
                "heritage.ron",
                &at("open_market.male"),
                g.open_market.male.validate_within(0.0, 1.0),
            )?;
            if let Some(m) = &g.immigrant_male_share {
                check(
                    "heritage.ron",
                    &at("immigrant_male_share"),
                    m.validate_within(0.0, 1.0),
                )?;
            }
            for (field, series) in [
                ("fertility", &g.fertility),
                ("mortality.infant", &g.mortality.infant),
                ("mortality.adult", &g.mortality.adult),
            ] {
                check(
                    "heritage.ron",
                    &at(field),
                    series.validate_within(0.05, 20.0),
                )?;
            }
        }
        if h.founder_mix(self.y0).iter().any(|x| !x.is_finite()) {
            return Err(err(
                "heritage.ron",
                "groups",
                "founder shares sum to zero".into(),
            ));
        }

        // names.ron
        self.names
            .validate(&self.heritage, &self.name_data)
            .map_err(|(at, m)| err("names.ron", &at, m))?;

        // households.ron
        let hh = &self.households;
        if hh.independence.female.is_empty() || hh.independence.male.is_empty() {
            return Err(err(
                "households.ron",
                "independence",
                "needs anchors for both sexes".into(),
            ));
        }
        share("households.ron", "custody_father", hh.custody_father)?;
        check(
            "households.ron",
            "kin.elder.single",
            hh.kin.elder.single.validate_within(0.0, 1.0),
        )?;
        check(
            "households.ron",
            "kin.elder.couple",
            hh.kin.elder.couple.validate_within(0.0, 1.0),
        )?;
        check(
            "households.ron",
            "roommates.level",
            hh.roommates.level.validate_within(0.0, 1.0),
        )?;
        check(
            "households.ron",
            "roommates.by_age",
            hh.roommates.by_age.validate(),
        )?;
        if hh.roommates.band_years < 1 || hh.roommates.epoch_years < 1 {
            return Err(err(
                "households.ron",
                "roommates",
                "band_years and epoch_years must be at least 1".into(),
            ));
        }

        // residence.ron
        let r = &self.residence;
        let rate = |at: &str, st: &Steps| {
            check(
                "residence.ron",
                at,
                st.validate().and_then(|()| {
                    if st.steps.iter().all(|s| s.1 >= 0.0) && st.above >= 0.0 {
                        Ok(())
                    } else {
                        Err("rates must be non-negative".into())
                    }
                }),
            )
        };
        rate("local.by_age", &r.local.by_age)?;
        rate("long.by_age", &r.long.by_age)?;
        check(
            "residence.ron",
            "local.era",
            r.local.era.validate_within(0.0, 100.0),
        )?;
        check(
            "residence.ron",
            "long.era",
            r.long.era.validate_within(0.0, 100.0),
        )?;
        check(
            "residence.ron",
            "local.levels",
            shares_sum_to_one(&r.local.levels.as_array()),
        )?;
        for (at, l) in [
            ("formation.leave_home", &r.formation.leave_home),
            ("formation.union", &r.formation.union),
            ("formation.separation", &r.formation.separation),
        ] {
            check("residence.ron", at, l.validate())?;
        }
        if r.local.frailty_variance.is_nan() || r.local.frailty_variance < 0.0 {
            return Err(err(
                "residence.ron",
                "local.frailty_variance",
                "must be non-negative".into(),
            ));
        }
        for (at, g) in [("gravity", &r.gravity), ("local_gravity", &r.local_gravity)] {
            if g.flat_miles.is_nan()
                || g.flat_miles <= 0.0
                || g.segments.windows(2).any(|w| w[0].0 >= w[1].0)
                || g.segments.first().is_some_and(|s| s.0 <= g.flat_miles)
            {
                return Err(err(
                    "residence.ron",
                    at,
                    "needs flat_miles > 0 and segment ends increasing past it".into(),
                ));
            }
        }
        share("residence.ron", "long.own_region", r.long.own_region)?;
        share("residence.ron", "union_source_woman", r.union_source_woman)?;
        share("residence.ron", "union_keep", r.union_keep)?;
        share("residence.ron", "stays.woman", r.stays.woman)?;
        share("residence.ron", "stays.man", r.stays.man)?;
        if r.stays.woman + r.stays.man > 1.0 {
            return Err(err(
                "residence.ron",
                "stays",
                "woman + man must be at most 1".into(),
            ));
        }

        // places.ron
        let ids: Vec<&str> = self.regions.iter().map(|r| r.id.as_str()).collect();
        let pl = &self.places;
        for (i, sr) in pl.regions.iter().enumerate() {
            if !ids.contains(&sr.region.as_str()) {
                return Err(err(
                    "places.ron",
                    &format!("regions[{i}]"),
                    format!("`{}` is not a region of world.ron", sr.region),
                ));
            }
        }
        if !pl.by_area && !ids.contains(&pl.others.as_str()) {
            return Err(err(
                "places.ron",
                "others",
                format!("`{}` is not a region of world.ron", pl.others),
            ));
        }
        let mut seen = std::collections::HashSet::new();
        if let Some(s) = pl
            .regions
            .iter()
            .flat_map(|r| &r.states)
            .find(|&&s| !seen.insert(s))
        {
            return Err(err(
                "places.ron",
                "regions",
                format!("state {s} is listed twice"),
            ));
        }
        self.work.validate().map_err(|(at, m)| err("work.ron", &at, m))?;
        self.education.validate(&self.heritage).map_err(|(at, m)| err("education.ron", &at, m))?;
        Ok(())
    }
}

fn unique_ids<'a>(ids: impl Iterator<Item = &'a str>) -> Result<(), String> {
    let mut seen = std::collections::BTreeSet::new();
    for id in ids {
        if !seen.insert(id) {
            return Err(format!("id `{id}` appears twice"));
        }
    }
    Ok(())
}

// --- mortality ---------------------------------------------------------------------

/// `mortality.ron`: a Siler hazard by sex, scaled by era.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mortality {
    pub infant_decay: f64,
    pub base: MortalityBase,
    pub multipliers: Vec<MortalityAnchor>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MortalityBase {
    pub infant: f64,
    pub background: BySex<f64>,
    pub old: BySex<f64>,
    pub old_slope: BySex<f64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MortalityAnchor {
    pub year: i32,
    pub infant: f64,
    pub background_female: f64,
    pub background_male: f64,
    pub old: f64,
}

impl Mortality {
    /// The era multipliers `(infant, background_female, background_male,
    /// old)` at `year`, interpolated log-linearly between anchors (constant
    /// outside them).
    fn multipliers(&self, year: i32) -> [f64; 4] {
        let a = &self.multipliers;
        let v = |m: &MortalityAnchor| [m.infant, m.background_female, m.background_male, m.old];
        let b = bracket(a.len(), |i| a[i].year as f64, year as f64);
        if b.clamped() {
            return v(&a[b.lo]);
        }
        let (m0, m1) = (v(&a[b.lo]), v(&a[b.hi]));
        std::array::from_fn(|k| log_lerp(m0[k], m1[k], b.f))
    }

    /// The Siler hazard for `sex` in `year`.
    pub fn siler(&self, sex: Sex, year: i32) -> Siler {
        let [infant, bf, bm, old] = self.multipliers(year);
        let b = &self.base;
        let (background, old, slope) = match sex {
            Sex::Female => (
                bf * b.background.female,
                old * b.old.female,
                b.old_slope.female,
            ),
            Sex::Male => (bm * b.background.male, old * b.old.male, b.old_slope.male),
        };
        Siler {
            infant: infant * b.infant,
            decay: self.infant_decay,
            background,
            old,
            slope,
        }
    }

    /// Force of mortality at exact `age` in calendar `year`.
    pub fn hazard(&self, sex: Sex, age: f64, year: i32) -> f64 {
        self.siler(sex, year).hazard(age)
    }

    /// [`Self::death_prob`] for a group whose infant term is scaled by
    /// `f.infant` and whose background and old-age terms by `f.adult`
    /// (heritage.ron). Factors of exactly 1 give [`Self::death_prob`]'s bits.
    pub fn death_prob_scaled(&self, sex: Sex, age: u32, year: i32, f: MortalityFactor) -> f64 {
        let s = self.siler(sex, year);
        Siler {
            infant: s.infant * f.infant,
            background: s.background * f.adult,
            old: s.old * f.adult,
            ..s
        }
        .year_death_prob(age)
    }

    /// Probability of dying within age `[a, a + 1)` during calendar year
    /// `year` ([`Siler::year_death_prob`]).
    pub fn death_prob(&self, sex: Sex, age: u32, year: i32) -> f64 {
        self.siler(sex, year).year_death_prob(age)
    }
}

// --- unions ------------------------------------------------------------------------

/// `unions.ron`: first-union timing, market shares and age-gap kernels.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Unions {
    pub first_union: FirstUnion,
    pub national_market_share: Series,
    /// Open (cross-heritage) and same-sex markets run per region too, with
    /// the national share across regions, as the heritage markets do (B at
    /// area level: partners meet where they live). Off: they are national.
    #[serde(default)]
    pub local_open_markets: bool,
    pub same_sex_share: Series,
    pub age_gap: Ranges,
    pub repartnering_gap_factor: f64,
    pub status_affinity: StatusAffinity,
    pub marriage: Marriage,
}

/// Which unions are marriages, and from when (`unions.ron`, N1).
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Marriage {
    /// Share of unions that begin as a marriage, by union year.
    pub at_start: Series,
    /// Of unions that begin as cohabitation, the share that marry if the
    /// couple is still together, by union year.
    pub later: Series,
    /// Mean years from moving in to the wedding (exponential).
    pub delay_years: f64,
    /// Share of same-sex couples who can marry, by year.
    pub same_sex_legal: Series,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FirstUnion {
    pub min_age: i32,
    pub origin: f64,
    pub shape: f64,
    pub median_female: Series,
    pub male_delay: f64,
    pub ever_partnered: Series,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatusAffinity {
    pub neither: f64,
    pub both: f64,
    pub one: f64,
}

impl Unions {
    /// Youngest age at which anyone enters a union.
    pub fn min_age(&self) -> i32 {
        self.first_union.min_age
    }

    /// Annual first-union hazard at `age` for the never-partnered, by year:
    /// log-logistic age at first union among those who ever partner, with no
    /// upper age (statistics, not rules).
    pub fn first_union_hazard(&self, sex: Sex, age: u32, year: i32) -> f64 {
        let fu = &self.first_union;
        if (age as i64) < fu.min_age as i64 {
            return 0.0;
        }
        let median = fu.median_female.at(year) + if sex == Sex::Male { fu.male_delay } else { 0.0 };
        let e = fu.ever_partnered.at(year);
        let cdf = |a: f64| log_logistic_cdf(a, fu.origin, median, fu.shape);
        cure_hazard(e, cdf(age as f64), cdf(age as f64 + 1.0))
    }

    /// Share of each block's desired unions that goes to the national
    /// (cross-region) market, by year.
    pub fn national_market_share(&self, year: i32) -> f64 {
        self.national_market_share.at(year)
    }

    /// Share of each sex's first-union demand that goes to same-sex unions.
    pub fn same_sex_share(&self, year: i32) -> f64 {
        self.same_sex_share.at(year)
    }

    /// Age-gap kernel `g(age_m − age_f)`.
    pub fn age_gap_weight(&self, gap: i32) -> f64 {
        self.age_gap.at(gap)
    }

    /// Age-gap kernel for same-sex couples: the opposite-sex kernel made
    /// symmetric.
    pub fn same_sex_gap_weight(&self, gap: i32) -> f64 {
        symmetrized(|g| self.age_gap_weight(g), gap)
    }

    /// Age-gap kernel for unions with a divorced partner: wider.
    pub fn remarriage_gap_weight(&self, gap: i32) -> f64 {
        dilated(
            |g| self.age_gap_weight(g),
            gap,
            self.repartnering_gap_factor,
        )
    }

    /// Relative affinity of a union by whether each partner is divorced.
    pub fn status_affinity(&self, woman_divorced: bool, man_divorced: bool) -> f64 {
        let a = &self.status_affinity;
        match (woman_divorced, man_divorced) {
            (false, false) => a.neither,
            (true, true) => a.both,
            _ => a.one,
        }
    }
}

// --- re-partnering (R1c) -----------------------------------------------------------

/// `repartnering.ron`: the hazard of a new union after a separation.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Repartnering {
    pub duration: Steps,
    pub age_factor: Steps,
    pub male_factor: f64,
    pub era: Series,
    pub cap: f64,
}

impl Repartnering {
    /// Annual hazard that a separated person starts a new union, by sex,
    /// age, years since the separation and year.
    pub fn hazard(&self, sex: Sex, age: i32, years: i32, year: i32) -> f64 {
        (self.duration(years) * self.base(sex, age, year)).min(self.cap)
    }

    /// The duration factor of [`Self::hazard`].
    pub fn duration(&self, years: i32) -> f64 {
        self.duration.at(years)
    }

    /// The rest of [`Self::hazard`]: age, sex and era factors, the same for
    /// all of a block's separated in a year.
    pub fn base(&self, sex: Sex, age: i32, year: i32) -> f64 {
        if age > MAX_REMARRIAGE_AGE {
            return 0.0;
        }
        let age_factor = self.age_factor.at(age);
        let sex_factor = if sex == Sex::Male {
            self.male_factor
        } else {
            1.0
        };
        age_factor * sex_factor * self.era.at(year)
    }
}

// --- fertility -------------------------------------------------------------------

/// `fertility.ron`: births per union, their timing, and non-union births.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fertility {
    /// Births a year (millions), the world's own series before scaling:
    /// the monotone world's births are this, scaled so the year before the
    /// first one is the founders' births.
    pub births: Series,
    /// Mothers' ages at birth (monotone world).
    pub mother_age: MotherAge,
    /// Births a woman has in her unions, against which the single share of
    /// births is set (monotone world): a non-union birth per woman `e` gives
    /// a single share `e / (e + union_births)`.
    pub union_births: f64,
    pub union_parity: VecSeries,
    pub second_union_fertile: f64,
    pub first_birth: FirstBirth,
    pub spacing: Vec<Spacing>,
    pub gestation_days: i64,
    pub nonunion: NonUnion,
}

/// Mothers' ages at birth: an algebraic sigmoid CDF over the fertile window,
/// centred at `peak` (by year), with `scale`; single mothers' centre is
/// `single_offset` years from married mothers'.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MotherAge {
    pub peak: Series,
    pub single_offset: f64,
    pub scale: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FirstBirth {
    pub historical: Vec<f64>,
    pub modern: Vec<f64>,
    pub shift: Series,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spacing {
    /// Years between births, alternating.
    pub gaps: (u32, u32),
    pub weight: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NonUnion {
    pub one: Series,
    pub two_per_one: f64,
    pub second_gaps: Vec<u8>,
    pub age_peak: i32,
    pub age_variance: f64,
}

impl Fertility {
    /// Parity pmf (0..=8) among women in a union, by union year, for a
    /// group whose fertility is `factor` times the base: the base pmf
    /// exponentially tilted (`p_k θ^k`, normalized) until its mean is
    /// `factor` times the base mean. A factor of exactly 1 is the base.
    pub fn union_parity_pmf(&self, year: i32, factor: f64) -> [f64; 9] {
        let mut pmf = self.union_parity.at::<9>(year);
        tilt_mean(&mut pmf, factor);
        pmf
    }

    /// Parity pmf in a re-partnering union (R1c): with probability
    /// `second_union_fertile`, the union year's schedule moved one birth
    /// down; otherwise no birth. Mother-age truncation at [`MAX_BIRTH_AGE`]
    /// does the rest.
    pub fn second_union_parity_pmf(&self, year: i32, factor: f64) -> [f64; 9] {
        one_fewer_or_none(
            &self.union_parity_pmf(year, factor),
            self.second_union_fertile,
        )
    }

    /// First-birth offsets in years after union start (1..=4), by era.
    pub fn first_birth_offset_pmf(&self, year: i32) -> [f64; 4] {
        let modern = self.first_birth.shift.at(year);
        let (hist, now) = (&self.first_birth.historical, &self.first_birth.modern);
        let mut out = [0.0; 4];
        mix_into(&hist[..4], &now[..4], modern, &mut out);
        out
    }

    /// The `k`-th gap (`k >= 1`) of spacing pattern `pattern`.
    pub fn spacing_gap(&self, pattern: usize, k: u32) -> u32 {
        let g = self.spacing[pattern].gaps;
        if (k - 1) % 2 == 0 {
            g.0
        } else {
            g.1
        }
    }

    /// Weights of the spacing patterns.
    pub fn spacing_weights(&self) -> Vec<f64> {
        self.spacing.iter().map(|s| s.weight).collect()
    }

    /// Share of women with non-union births (0, 1 or 2 births), by year of
    /// the woman's 25th birthday, for a group whose fertility is `factor`
    /// times the base (capped so the shares stay a distribution).
    pub fn nonunion_count_pmf(&self, year: i32, factor: f64) -> [f64; 3] {
        let base = self.nonunion.one.at(year);
        let p1 = if factor == 1.0 {
            base
        } else {
            (base * factor).min(0.9 / (1.0 + self.nonunion.two_per_one))
        };
        let p2 = p1 * self.nonunion.two_per_one;
        [1.0 - p1 - p2, p1, p2]
    }

    /// Age weight (15..=44) of a non-union birth.
    pub fn nonunion_age_weight(&self, age: i32) -> f64 {
        if !(MIN_BIRTH_AGE..MAX_BIRTH_AGE).contains(&age) {
            return 0.0;
        }
        gaussian_kernel(
            (age - self.nonunion.age_peak) as f64,
            self.nonunion.age_variance,
        )
    }
}

// --- dissolution -------------------------------------------------------------------

/// `dissolution.ron`: how unions end.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dissolution {
    pub separation_share: Series,
    pub bands: DissolutionBandShares,
    pub repartnered_factor: f64,
    pub max_share: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DissolutionBandShares {
    pub early: Vec<f64>,
    pub late: Vec<f64>,
    pub shift: Series,
}

impl Dissolution {
    /// Pmf over [`DISSOLUTION_BANDS`] for a union formed in `year`.
    pub fn pmf(&self, year: i32) -> [f64; 6] {
        let d = self.separation_share.at(year);
        let (old, new) = (&self.bands.early, &self.bands.late);
        let f = self.bands.shift.at(year);
        let within: [f64; 5] = std::array::from_fn(|k| lerp(old[k], new[k], f));
        let mut out = [0.0; 6];
        out[0] = 1.0 - d;
        for k in 0..5 {
            out[k + 1] = d * within[k];
        }
        out
    }

    /// Pmf over dissolution classes for a union formed in `year`: each band
    /// of [`Self::pmf`] spread evenly over its years. `remarriage` (either
    /// partner re-partnering) raises the separation share by
    /// `repartnered_factor`, capped at `max_share`.
    pub fn class_pmf(&self, year: i32, remarriage: bool) -> [f64; MAX_CLASS + 1] {
        let bands = self.pmf(year);
        let scale = if remarriage {
            self.repartnered_factor
        } else {
            1.0
        };
        let divorce: f64 = bands[1..].iter().sum::<f64>() * scale;
        let divorce = divorce.min(self.max_share);
        let first_total: f64 = bands[1..].iter().sum();
        let mut out = [0.0; MAX_CLASS + 1];
        out[0] = 1.0 - divorce;
        for (band, &w) in DISSOLUTION_BANDS.iter().zip(&bands) {
            if let Some((lo, hi)) = band {
                let share = if first_total > 0.0 {
                    divorce * w / first_total
                } else {
                    0.0
                };
                spread_evenly(&mut out, *lo as usize, *hi as usize, share);
            }
        }
        out
    }
}

// --- immigration -----------------------------------------------------------------

/// `immigration.ron`: arrivals per year, their ages, couples and sex.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Immigration {
    pub rate: ImmigrationRate,
    pub min_age: i32,
    pub age_profile: AgeProfile,
    pub couple_share: Series,
    pub min_couple_age: i32,
    pub max_husband_younger: i32,
    pub max_child_age: i32,
    pub male_share: Series,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImmigrationRate {
    pub scale: f64,
    pub anchors: Series,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgeProfile {
    pub mu: i32,
    pub alpha: f64,
    pub lambda: f64,
}

impl Immigration {
    /// Net adult immigrants per year as a share of the living population.
    pub fn rate(&self, year: i32) -> f64 {
        self.rate.scale * self.rate.anchors.at(year)
    }

    /// Relative weight of arriving at `age` (Rogers–Castro labour component).
    pub fn age_weight(&self, age: i32) -> f64 {
        if !(self.min_age..=MAX_ARRIVAL_AGE).contains(&age) {
            return 0.0;
        }
        let p = &self.age_profile;
        rogers_castro_labour((age - p.mu) as f64, p.alpha, p.lambda)
    }

    /// Share of adult immigrants who arrive as a couple.
    pub fn couple_share(&self, year: i32) -> f64 {
        self.couple_share.at(year)
    }

    /// Share of immigrants who are men.
    pub fn male_share(&self, year: i32) -> f64 {
        self.male_share.at(year)
    }
}

// --- heritage (N1) -------------------------------------------------------------

/// A heritage group: an index into the pack's `heritage.ron` groups.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Heritage(pub u8);

impl Heritage {
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

impl std::fmt::Debug for Heritage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// `heritage.ron`: heritage groups, their founder and immigrant shares, and
/// their open-market (cross-heritage) shares.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Heritages {
    pub groups: Vec<HeritageGroup>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeritageGroup {
    pub id: String,
    pub name: String,
    pub description: String,
    pub founder_share: Series,
    pub immigrant_share: Series,
    pub open_market: BySex<Series>,
    /// Fertility relative to the base schedules (fertility.ron), by year:
    /// births per union and births outside a union alike.
    pub fertility: Series,
    /// Mortality relative to the base (mortality.ron), by calendar year.
    pub mortality: GroupMortality,
    /// Share of the group's immigrants who are men, by arrival year, where
    /// it differs from immigration.ron's `male_share` (optional).
    #[serde(default)]
    pub immigrant_male_share: Option<Series>,
}

/// A group's mortality factors, by calendar year.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupMortality {
    /// Multiplies the infant term.
    pub infant: Series,
    /// Multiplies the background and old-age terms.
    pub adult: Series,
}

/// Mortality factors in one year: see [`Mortality::death_prob_scaled`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MortalityFactor {
    pub infant: f64,
    pub adult: f64,
}

impl Heritages {
    /// The group with id `id`.
    pub fn find(&self, id: &str) -> Option<Heritage> {
        self.groups
            .iter()
            .position(|g| g.id == id)
            .map(|i| Heritage(i as u8))
    }

    /// Group `h`'s definition.
    pub fn group(&self, h: Heritage) -> &HeritageGroup {
        &self.groups[h.index()]
    }

    /// Heritage mix of the founders, the population alive at `y0`.
    pub fn founder_mix(&self, y0: i32) -> Vec<f64> {
        normalized(self.groups.iter().map(|g| g.founder_share.at(y0)))
    }

    /// Heritage mix of `year`'s immigrants.
    pub fn immigrant_mix(&self, year: i32) -> Vec<f64> {
        normalized(self.groups.iter().map(|g| g.immigrant_share.at(year)))
    }

    /// Share of group `h`'s immigrants arriving in `year` who are men, if the
    /// group sets its own.
    pub fn immigrant_male_share(&self, h: Heritage, year: i32) -> Option<f64> {
        self.groups[h.index()]
            .immigrant_male_share
            .as_ref()
            .map(|s| s.at(year))
    }

    /// Group `h`'s fertility factor in `year`.
    pub fn fertility_factor(&self, h: Heritage, year: i32) -> f64 {
        self.groups[h.index()].fertility.at(year)
    }

    /// Group `h`'s mortality factors in calendar `year`.
    pub fn mortality_factor(&self, h: Heritage, year: i32) -> MortalityFactor {
        let m = &self.groups[h.index()].mortality;
        MortalityFactor {
            infant: m.infant.at(year),
            adult: m.adult.at(year),
        }
    }

    /// Share of a seeker's wants that go to the open (cross-heritage)
    /// market, by year, heritage and sex (N1 plan §3).
    pub fn open_market_share(&self, year: i32, h: Heritage, sex: Sex) -> f64 {
        self.groups[h.index()]
            .open_market
            .get(sex.is_female())
            .at(year)
    }
}

// --- names (N1) ---------------------------------------------------------------

/// `names.ron`: name data and naming practice.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Names {
    /// The name tables, a data file of the pack.
    pub data: String,
    /// Each heritage group's column in the data.
    pub columns: Vec<GroupColumn>,
    pub first_names: FirstNames,
    pub middle_names: MiddleNames,
    pub surnames: SurnameRules,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupColumn {
    pub group: String,
    pub column: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupShare {
    pub group: String,
    pub share: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FirstNames {
    /// Share of each group's immigrants named from the data's foreign names.
    pub foreign: Vec<GroupShare>,
    /// Probability that a child of parents in different groups is named in
    /// the mother's group's tradition.
    pub mixed_mother: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MiddleNames {
    /// Share with a middle name, by birth cohort and sex.
    pub default: BySex<Series>,
    pub by_group: Vec<GroupMiddle>,
    /// Share of middle names that are a family surname, by cohort and sex.
    pub family_surname: BySex<Series>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupMiddle {
    pub group: String,
    pub share: BySex<Series>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SurnameRules {
    /// Unlisted surnames are spread over listed ones with fewer holders.
    pub rare_below: u32,
    pub child: ChildSurname,
    pub double: DoubleSurname,
    pub wedding: Wedding,
    pub separation_reverts: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChildSurname {
    pub married: ParentShares,
    pub cohabiting: ParentShares,
}

/// Shares of a child's surname coming from the father, the mother, or both
/// hyphenated.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParentShares {
    pub father: f64,
    pub mother: f64,
    pub both: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DoubleSurname {
    pub group: String,
    pub share: Series,
    pub hyphenated: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Wedding {
    pub woman_takes: Series,
    pub woman_hyphenates: Series,
    pub older_bride_age: i32,
    pub older_bride_keeps: f64,
    pub man_takes: Series,
    pub same_sex_takes: f64,
    pub same_sex_hyphenates: f64,
    pub keeps_more: Vec<GroupFactor>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupFactor {
    pub group: String,
    pub factor: f64,
}

impl Names {
    /// Every group maps to a column of the data, shares are shares, and
    /// group ids resolve. Errors are `(field path, message)`.
    fn validate(
        &self,
        her: &Heritages,
        data: &crate::names::NameData,
    ) -> Result<(), (String, String)> {
        let e = |at: &str, m: String| Err((at.to_string(), m));
        for g in &her.groups {
            match self.columns.iter().find(|c| c.group == g.id) {
                None => return e("columns", format!("group `{}` has no column", g.id)),
                Some(c) if !data.columns.contains(&c.column) => {
                    return e(
                        "columns",
                        format!(
                            "column `{}` is not in the data ({:?})",
                            c.column, data.columns
                        ),
                    )
                }
                _ => {}
            }
        }
        let known = |id: &str| her.find(id).is_some();
        let ids = self
            .columns
            .iter()
            .map(|c| c.group.as_str())
            .chain(self.first_names.foreign.iter().map(|f| f.group.as_str()))
            .chain(self.middle_names.by_group.iter().map(|g| g.group.as_str()))
            .chain(
                self.surnames
                    .wedding
                    .keeps_more
                    .iter()
                    .map(|g| g.group.as_str()),
            )
            .chain(std::iter::once(self.surnames.double.group.as_str()));
        for id in ids {
            if !known(id) {
                return e("", format!("unknown heritage group `{id}`"));
            }
        }
        let shares = [
            self.first_names.mixed_mother,
            self.surnames.double.hyphenated,
            self.surnames.separation_reverts,
            self.surnames.wedding.same_sex_takes + self.surnames.wedding.same_sex_hyphenates,
        ]
        .into_iter()
        .chain(self.first_names.foreign.iter().map(|f| f.share));
        for x in shares {
            if !(0.0..=1.0).contains(&x) {
                return e("", format!("{x} is not a share in [0, 1]"));
            }
        }
        for (at, p) in [
            ("surnames.child.married", self.surnames.child.married),
            ("surnames.child.cohabiting", self.surnames.child.cohabiting),
        ] {
            if (p.father + p.mother + p.both - 1.0).abs() > 1e-9 {
                return e(at, "father + mother + both must be 1".into());
            }
        }
        let w = &self.surnames.wedding;
        for (at, s) in [
            ("surnames.wedding.woman_takes", &w.woman_takes),
            ("surnames.wedding.woman_hyphenates", &w.woman_hyphenates),
            ("surnames.wedding.man_takes", &w.man_takes),
            ("surnames.double.share", &self.surnames.double.share),
        ] {
            s.validate_within(0.0, 1.0)
                .map_err(|m| (at.to_string(), m))?;
        }
        Ok(())
    }

    /// Group `h`'s column index in `data`.
    pub fn column(&self, her: &Heritages, data: &crate::names::NameData, h: Heritage) -> usize {
        let id = &her.group(h).id;
        let c = &self
            .columns
            .iter()
            .find(|c| &c.group == id)
            .expect("validated")
            .column;
        data.columns.iter().position(|d| d == c).expect("validated")
    }
}

// --- education (L5) -----------------------------------------------------------------

/// Education: final attainment by cohort, sex and group, how it runs in
/// families and couples, and when schooling happens (`education.ron`).
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Education {
    /// Percent shares of [less than HS, HS, some college, BA, graduate] by
    /// birth cohort, per sex.
    pub attainment: BySex<VecSeries>,
    /// Of some college, the share with an associate degree, by cohort.
    pub associate: Series,
    /// Of graduate degrees, [master's, professional, doctorate] by cohort.
    pub graduate: VecSeries,
    /// Each group's latent shift (log-odds of every threshold), by cohort.
    pub group_shift: Vec<GroupShift>,
    /// The latent's variance split.
    pub latent: EducationLatent,
    /// Schools and colleges (`data`), and how people choose them.
    pub data: String,
    pub private_share: Series,
    pub college: CollegeChoice,
    pub timing: EducationTiming,
}

/// How students choose colleges.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CollegeChoice {
    pub in_state: f64,
    pub miles_scale: f64,
    pub decay: f64,
    pub two_year: Series,
    pub graduate_in_state: f64,
    pub graduate_decay: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupShift {
    pub group: String,
    pub shift: Series,
}

/// Weights squared of the latent's keys (they sum to 1).
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EducationLatent {
    pub own: f64,
    pub parents: f64,
    pub grandparents: f64,
    pub union: f64,
}

/// A span of years, drawn uniformly.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Span {
    pub min: f64,
    pub max: f64,
}

impl Span {
    /// The span at `u ∈ [0, 1)`.
    pub fn at(&self, u: f64) -> f64 {
        self.min + u * (self.max - self.min)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EducationTiming {
    pub first_grade_age: Series,
    pub kindergarten: Series,
    pub dropout_age: Series,
    pub dropout_spread: f64,
    pub hs_end_age: i32,
    pub college_on_time: f64,
    pub late_start_years: f64,
    pub some_college_years: Span,
    pub associate_years: Span,
    pub bachelor_years: Span,
    pub graduate_gap_years: f64,
    pub master_years: Span,
    pub professional_years: Span,
    pub doctorate_years: Span,
}

impl Education {
    /// Group `h`'s shift in `cohort` (0 for a group without one).
    pub fn shift(&self, her: &Heritages, h: Heritage, cohort: i32) -> f64 {
        self.group_shift.iter().find(|g| her.find(&g.group) == Some(h)).map_or(0.0, |g| g.shift.at(cohort))
    }

    fn validate(&self, her: &Heritages) -> Result<(), (String, String)> {
        let e = |f: &str, m: String| (f.to_string(), m);
        for (name, v) in [("attainment.female", &self.attainment.female), ("attainment.male", &self.attainment.male)] {
            v.validate(5).map_err(|m| e(name, m))?;
            if let Some(a) = v.0.iter().find(|a| (a.1.iter().sum::<f64>() - 100.0).abs() > 0.5 || a.1.iter().any(|&x| x < 0.0)) {
                return Err(e(name, format!("cohort {}'s shares don't sum to 100", a.0)));
            }
        }
        self.associate.validate_within(0.0, 1.0).map_err(|m| e("associate", m))?;
        self.graduate.validate_pmf(3).map_err(|m| e("graduate", m))?;
        for g in &self.group_shift {
            if her.find(&g.group).is_none() {
                return Err(e("group_shift", format!("no heritage group `{}`", g.group)));
            }
            g.shift.validate().map_err(|m| e("group_shift", m))?;
        }
        let l = self.latent;
        if [l.own, l.parents, l.grandparents, l.union].iter().any(|&x| x < 0.0) || (l.own + l.parents + 2.0 * l.grandparents + l.union - 1.0).abs() > 1e-9 {
            return Err(e("latent", "the weights must be nonnegative with own + parents + 2·grandparents + union = 1".into()));
        }
        Ok(())
    }
}

// --- work (L6) ----------------------------------------------------------------------

/// Careers, occupations, pay and employers (`work.ron`).
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Work {
    pub data: String,
    /// `(age at the job's start, log-mean, log-sd)` of job lengths in years.
    pub job_years: Vec<(i32, f64, f64)>,
    pub direct: f64,
    pub unemployment_weeks: LogNormal,
    pub break_share: f64,
    pub break_years: LogNormal,
    pub homemaker: Series,
    pub homemaker_returns: Series,
    pub return_child_age: f64,
    pub homemaker_single: Series,
    pub students: Students,
    pub disability: Disability,
    pub retirement: BySex<Vec<RetirementCurve>>,
    pub era_factors: Vec<EraFactor>,
    pub switch_occupation: f64,
    pub experience: Experience,
    pub real_wage: Series,
    pub cpi: Series,
    pub size_classes: Vec<SizeClass>,
    pub other_county: f64,
    pub names: EmployerNames,
}

/// How employers are named: patterns with `{s}` (a surname), `{t}` (a
/// second surname), `{p}` (the county's name), `{l}` (the line of business)
/// and `{x}` (a corporate suffix).
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmployerNames {
    /// For a line of business that names a place ("Pizza Parlor"), the
    /// share named after it, and the patterns.
    pub place_share: f64,
    pub place: Vec<String>,
    /// Place nouns of institutions (the last word of a line, lower case)
    /// and their patterns.
    pub civic_nouns: Vec<String>,
    pub civic: Vec<String>,
    /// Size classes from which an establishment takes its sector's `large`
    /// patterns.
    pub large_from: u8,
    pub suffixes: Vec<String>,
    /// By NAICS recode prefix (the longest listed prefix of an industry's
    /// code applies; `""` matches every industry).
    pub sectors: Vec<SectorNames>,
}

/// Name patterns of a sector.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SectorNames {
    pub naics: String,
    pub small: Vec<String>,
    #[serde(default)]
    pub large: Vec<String>,
}

impl EmployerNames {
    /// The patterns for an industry's NAICS recode `code`.
    pub fn sector(&self, code: &str) -> &SectorNames {
        self.sectors.iter().filter(|s| code.starts_with(s.naics.as_str())).max_by_key(|s| s.naics.len()).expect("a sector matches every code")
    }
}

/// A lognormal by its median and log-sd.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogNormal {
    pub median: f64,
    pub sigma: f64,
}

/// A birth cohort's curve of working: `(cohort, [(age, share still
/// working of those working at 50)])`.
pub type RetirementCurve = (i32, Vec<(f64, f64)>);

/// Students' jobs: the share of students working at a time by year, the
/// lengths of their jobs, and their hours as a share of full time.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Students {
    pub share: Series,
    pub job_years: LogNormal,
    pub hours: f64,
}

/// Leaving the labour force for disability or ill health: a yearly hazard
/// by age, its scale by year, the share who return and the spell's length.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Disability {
    pub hazard: Steps,
    pub era: Series,
    pub returns: f64,
    pub years: LogNormal,
}

/// A SOC major group's weight relative to 2023, by year.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EraFactor {
    pub group: u8,
    pub factor: Series,
}

/// Log points of pay per year of experience, up to a peak, from a start.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Experience {
    pub per_year: f64,
    pub peak_years: f64,
    pub start: f64,
}

/// An establishment size class: its share of employment and mean size.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SizeClass {
    pub share: f64,
    pub mean: f64,
}

impl Work {
    /// Job length's (log-mean, log-sd) for a job started at `age`.
    pub fn job_law(&self, age: f64) -> (f64, f64) {
        let a = &self.job_years;
        let f = |i: usize| a[i].0 as f64;
        let mu = procedural_core::interp::piecewise_linear(a.len(), f, |i| a[i].1, age);
        let sd = procedural_core::interp::piecewise_linear(a.len(), f, |i| a[i].2, age);
        (mu, sd)
    }

    /// The age at which someone of `cohort` leaves work for good, at
    /// quantile `u`: each listed cohort's curve inverted at `u`, the ages
    /// interpolated between cohorts.
    pub fn retirement_age(&self, female: bool, cohort: i32, u: f64) -> f64 {
        let rows = self.retirement.get(female);
        let age = |r: &[(f64, f64)]| procedural_core::interp::inverse_decreasing(r.len(), |i| r[i].0, |i| r[i].1, u);
        let b = procedural_core::interp::bracket(rows.len(), |i| rows[i].0 as f64, cohort as f64);
        procedural_core::interp::lerp(age(&rows[b.lo].1), age(&rows[b.hi].1), b.f)
    }

    /// Era factor of SOC major group `group` in `year` (1 if not listed).
    pub fn era_factor(&self, group: u8, year: i32) -> f64 {
        self.era_factors.iter().find(|e| e.group == group).map_or(1.0, |e| e.factor.at(year))
    }

    fn validate(&self) -> Result<(), (String, String)> {
        let e = |f: &str, m: String| (f.to_string(), m);
        if self.job_years.is_empty() || self.job_years.windows(2).any(|w| w[1].0 <= w[0].0) {
            return Err(e("job_years", "ages must increase".into()));
        }
        for (name, v) in [("direct", self.direct), ("break_share", self.break_share), ("switch_occupation", self.switch_occupation), ("other_county", self.other_county)] {
            if !(0.0..=1.0).contains(&v) {
                return Err(e(name, format!("{v} is not a share")));
            }
        }
        self.homemaker.validate_within(0.0, 1.0).map_err(|m| e("homemaker", m))?;
        self.homemaker_returns.validate_within(0.0, 1.0).map_err(|m| e("homemaker_returns", m))?;
        self.homemaker_single.validate_within(0.0, 1.0).map_err(|m| e("homemaker_single", m))?;
        self.students.share.validate_within(0.0, 1.0).map_err(|m| e("students.share", m))?;
        if !(0.0..=1.0).contains(&self.students.hours) || !(0.0..=1.0).contains(&self.disability.returns) {
            return Err(e("students.hours", "hours and returns are shares".into()));
        }
        self.disability.hazard.validate().map_err(|m| e("disability.hazard", m))?;
        self.disability.era.validate().map_err(|m| e("disability.era", m))?;
        for (sex, rows) in [("female", &self.retirement.female), ("male", &self.retirement.male)] {
            let f = format!("retirement.{sex}");
            if rows.is_empty() || rows.windows(2).any(|w| w[1].0 <= w[0].0) {
                return Err(e(&f, "cohorts must increase".into()));
            }
            for (c, r) in rows {
                if r.len() < 2 || r.windows(2).any(|w| w[1].0 <= w[0].0 || w[1].1 > w[0].1) || r[0].1 != 1.0 || r[r.len() - 1].1 != 0.0 {
                    return Err(e(&f, format!("cohort {c}: ages must increase and shares fall from 1 to 0")));
                }
            }
        }
        self.real_wage.validate().map_err(|m| e("real_wage", m))?;
        self.cpi.validate().map_err(|m| e("cpi", m))?;
        if self.size_classes.is_empty() || (self.size_classes.iter().map(|c| c.share).sum::<f64>() - 1.0).abs() > 0.01 {
            return Err(e("size_classes", "shares must sum to 1".into()));
        }
        let n = &self.names;
        if n.place.is_empty() || n.civic.is_empty() || n.suffixes.is_empty() || !(0.0..=1.0).contains(&n.place_share) {
            return Err(e("names", "place patterns and suffixes must not be empty".into()));
        }
        if !n.sectors.iter().any(|s| s.naics.is_empty()) {
            return Err(e("names.sectors", "one sector must have naics \"\" (every industry)".into()));
        }
        if let Some(s) = n.sectors.iter().find(|s| s.small.is_empty()) {
            return Err(e("names.sectors", format!("sector {:?} has no small patterns", s.naics)));
        }
        Ok(())
    }
}

// --- residence (L4) -----------------------------------------------------------------

/// `residence.ron`: how households move (spec `2026-10-01-residence.md`).
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Residence {
    pub local: LocalMoves,
    pub long: LongMoves,
    pub gravity: Gravity,
    pub local_gravity: Gravity,
    pub formation: Formation,
    pub union_source_woman: f64,
    pub union_keep: f64,
    pub stays: Stays,
}

/// Moves of a continuing household within its area, per year.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalMoves {
    pub by_age: Steps,
    pub era: Series,
    pub frailty_variance: f64,
    pub levels: MoveLevels,
}

/// Shares of the level a local move redraws.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MoveLevels {
    pub zone: f64,
    pub county: f64,
    pub cluster: f64,
    pub tract: f64,
}

impl MoveLevels {
    /// The shares as `[zone, county, cluster, tract]`.
    pub fn as_array(&self) -> [f64; 4] {
        [self.zone, self.county, self.cluster, self.tract]
    }
}

/// Moves to another area, per person per year.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LongMoves {
    pub by_age: Steps,
    pub era: Series,
    pub own_region: f64,
}

/// A piecewise power-law distance decay (miles): flat to `flat_miles`,
/// then `(end, exponent)` segments, then `beyond`.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Gravity {
    pub flat_miles: f64,
    pub segments: Vec<(f64, f64)>,
    pub beyond: f64,
}

/// Where new households form, relative to their source, per channel.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Formation {
    pub leave_home: FormLevels,
    pub union: FormLevels,
    pub separation: FormLevels,
}

/// Shares of a new home's place relative to its source: the same tract,
/// or a fresh draw from the county, cluster or tract level (within the
/// source's zone).
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormLevels {
    pub same: f64,
    pub county: f64,
    pub cluster: f64,
    pub tract: f64,
}

impl FormLevels {
    /// The first level redrawn for a uniform `u`, in the residence tree's
    /// numbering (2 county, 3 cluster, 4 tract), or 5 for the same tract.
    pub fn level(&self, u: f64) -> u8 {
        if u < self.county {
            2
        } else if u < self.county + self.cluster {
            3
        } else if u < self.county + self.cluster + self.tract {
            4
        } else {
            5
        }
    }

    fn validate(&self) -> Result<(), String> {
        shares_sum_to_one(&[self.same, self.county, self.cluster, self.tract])
    }
}

/// Who keeps the home after a separation; the rest of the time neither does.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stays {
    pub woman: f64,
    pub man: f64,
}

fn shares_sum_to_one(a: &[f64]) -> Result<(), String> {
    if a.iter().any(|x| !(0.0..=1.0).contains(x)) {
        return Err("shares must lie in [0, 1]".into());
    }
    if (a.iter().sum::<f64>() - 1.0).abs() > 1e-9 {
        return Err("shares must sum to 1".into());
    }
    Ok(())
}

/// `places.ron`: the place tree's data, and which lineage region each
/// state's basins belong to.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlacesSection {
    pub data: String,
    /// Each area is its own lineage region: the world region whose id is
    /// the area's name (B at area level, spec `2026-10-01-ledger-areas.md`).
    #[serde(default)]
    pub by_area: bool,
    #[serde(default)]
    pub regions: Vec<StateRegion>,
    #[serde(default)]
    pub others: String,
}

/// The states of one lineage region (FIPS codes).
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StateRegion {
    pub region: String,
    pub states: Vec<u8>,
}

impl Params {
    /// The lineage region (index into [`Params::regions`]) whose id is
    /// `id`, if any.
    pub fn region_by_id(&self, id: &str) -> Option<u16> {
        self.regions
            .iter()
            .position(|r| r.id == id)
            .map(|i| i as u16)
    }

    /// The lineage region (index into [`Params::regions`]) of state `fips`.
    pub fn region_of_state(&self, fips: u8) -> u16 {
        let id = self
            .places
            .regions
            .iter()
            .find(|r| r.states.contains(&fips))
            .map_or(self.places.others.as_str(), |r| r.region.as_str());
        self.regions
            .iter()
            .position(|r| r.id == id)
            .expect("places.ron regions are checked against world.ron") as u16
    }
}

// --- households (L3) ------------------------------------------------------------

/// `households.ron`: leaving home, kin co-residence, custody, roommates.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Households {
    pub independence: Independence,
    pub custody_father: f64,
    pub kin: Kin,
    pub roommates: Roommates,
}

/// `(birth cohort, m, s, tail)` anchors of an independence curve.
pub type IndependenceAnchor = (i32, f64, f64, f64);

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Independence {
    pub min_age: f64,
    pub female: Vec<IndependenceAnchor>,
    pub male: Vec<IndependenceAnchor>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Kin {
    pub elder: Elder,
    pub boomerang: Boomerang,
    pub other: OtherKin,
    pub young_couple: YoungCouple,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Elder {
    pub single: Series,
    pub couple: Series,
    pub midpoint: f64,
    pub width: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Boomerang {
    pub level: Series,
    pub min_age: f64,
    pub max_age: f64,
    pub peak: f64,
    pub width: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OtherKin {
    pub level: Series,
    pub from: f64,
    pub years: f64,
    pub floor: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct YoungCouple {
    pub level: Series,
    pub from: f64,
    pub years: f64,
    pub max_age: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Roommates {
    pub min_age: f64,
    pub max_age: f64,
    pub band_years: i32,
    pub epoch_years: i32,
    pub level: Series,
    pub by_age: Bands,
}

impl Households {
    /// The independence schedule `(m, s, tail)` of a birth cohort: the share
    /// of people who haven't left home on their own by age `a` is `S(a) =
    /// tail + (1 − tail) / (1 + exp((a − m) / s))`.
    pub fn independence_curve(&self, sex: Sex, cohort: i32) -> (f64, f64, f64) {
        let a = match sex {
            Sex::Female => &self.independence.female,
            Sex::Male => &self.independence.male,
        };
        let at = |k: usize| {
            piecewise_linear(
                a.len(),
                |i| a[i].0 as f64,
                |i| [a[i].1, a[i].2, a[i].3][k],
                cohort as f64,
            )
        };
        (at(0), at(1), at(2))
    }

    /// The age at which a person leaves home on their own, for survival
    /// quantile `u` in `[0, 1)`: the inverse of [`Self::independence_curve`]'s
    /// `S(a) = u`. `None` if they never do (`u` below the tail).
    pub fn independence_age(&self, sex: Sex, cohort: i32, u: f64) -> Option<f64> {
        let (m, s, tail) = self.independence_curve(sex, cohort);
        logistic_floor_quantile(m, s, tail, u).map(|a| a.max(self.independence.min_age))
    }

    /// The probability that an elder unit lives with a child, by the unit's
    /// age, the decider's birth cohort and whether the elder is single.
    pub fn elder_coresidence(&self, age: f64, cohort: i32, single: bool) -> f64 {
        let e = &self.kin.elder;
        let level = if single { &e.single } else { &e.couple };
        logistic_rise(level.at(cohort), age, e.midpoint, e.width)
    }

    /// Propensity of an independent single adult to live with kin.
    pub fn kin_single(&self, age: f64, year: i32, cohort: i32) -> f64 {
        self.boomerang(age, year)
            + self.kin_other(age, year)
            + self.elder_coresidence(age, cohort, true)
    }

    /// Propensity of a couple (deciding together, at the older partner's
    /// age) to live with kin.
    pub fn kin_couple(&self, age: f64, year: i32, cohort: i32) -> f64 {
        let y = &self.kin.young_couple;
        let young = y.level.at(year);
        let young = if age < y.max_age {
            young * (1.0 - ramp(age, y.from, y.years, 0.0, 1.0))
        } else {
            0.0
        };
        young + self.elder_coresidence(age, cohort, false)
    }

    fn boomerang(&self, age: f64, year: i32) -> f64 {
        let b = &self.kin.boomerang;
        if !(b.min_age..b.max_age).contains(&age) {
            return 0.0;
        }
        b.level.at(year) * gaussian_bump(age, b.peak, b.width)
    }

    fn kin_other(&self, age: f64, year: i32) -> f64 {
        let o = &self.kin.other;
        if age < o.from {
            return 0.0;
        }
        o.level.at(year) * ramp(age, o.from, o.years, o.floor, 1.0)
    }

    /// Probability that an eligible adult seeks roommates in an epoch.
    pub fn roommate_uptake(&self, age: f64, year: i32) -> f64 {
        let r = &self.roommates;
        r.level.at(year) * r.by_age.at(age)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use procedural_core::dmath::exp;

    fn us() -> Params {
        Params::prototype()
    }

    fn period_e0(p: &Params, sex: Sex, year: i32) -> f64 {
        let (mut l, mut e) = (1.0, 0.0);
        for a in 0..=MAX_AGE {
            let q = p.mortality.death_prob(sex, a, year);
            e += l * (1.0 - q / 2.0);
            l *= 1.0 - q;
        }
        e
    }

    #[test]
    fn the_embedded_packs_load() {
        let (us, tiny) = (Params::prototype(), Params::tiny());
        assert_eq!((us.y0, us.y1, tiny.y0, tiny.y1), (1840, 2100, 1900, 1990));
        assert_eq!(tiny.regions[0].name, "east", "inherited from us");
        assert_eq!(
            tiny.mortality.multipliers.len(),
            us.mortality.multipliers.len()
        );
        assert_ne!(us.fingerprint, tiny.fingerprint);
        assert_eq!(us.heritage.find("hispanic"), Some(Heritage(4)));
    }

    #[test]
    fn independence_age_inverts_the_survival_curve() {
        let p = us();
        let h = &p.households;
        for sex in [Sex::Female, Sex::Male] {
            for cohort in [1840, 1900, 1955, 2000, 2050] {
                let (m, s, tail) = h.independence_curve(sex, cohort);
                assert!(tail > 0.0 && tail < 0.8 && s > 0.0 && m > h.independence.min_age);
                let surv = |a: f64| tail + (1.0 - tail) / (1.0 + exp((a - m) / s));
                // Quantile u maps to the age where S(a) = u, and larger
                // quantiles leave earlier.
                let mut last = f64::INFINITY;
                for k in 0..100 {
                    let u = (k as f64 + 0.5) / 100.0;
                    match h.independence_age(sex, cohort, u) {
                        None => assert!(u < tail),
                        Some(a) => {
                            assert!(u >= tail && a <= last && a >= h.independence.min_age);
                            if a > h.independence.min_age {
                                assert!((surv(a) - u).abs() < 1e-9, "{cohort} {u}");
                            }
                            last = a;
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn elder_coresidence_never_reverts() {
        let p = us();
        for cohort in [1780, 1850, 1912, 1960, 2000] {
            let mut last = 0.0;
            for age in 50..110 {
                let (s, c) = (
                    p.households.elder_coresidence(age as f64, cohort, true),
                    p.households.elder_coresidence(age as f64, cohort, false),
                );
                assert!(s >= c && s >= last && (0.0..=1.0).contains(&s));
                last = s;
            }
        }
    }

    #[test]
    fn life_expectancy_follows_anchors() {
        let p = us();
        let cases = [(1840, 39.5, 36.8), (1900, 47.5, 44.6), (2024, 81.6, 76.6)];
        for (year, ef, em) in cases {
            let (f, m) = (
                period_e0(&p, Sex::Female, year),
                period_e0(&p, Sex::Male, year),
            );
            assert!((f - ef).abs() < 1.0, "{year} female e0 {f}");
            assert!((m - em).abs() < 1.0, "{year} male e0 {m}");
        }
    }

    #[test]
    fn death_prob_is_the_exact_hazard_integral() {
        let p = us();
        // Compare the closed form with a fine midpoint rule.
        for sex in [Sex::Female, Sex::Male] {
            for year in [1840, 1923, 2024, 2100] {
                for age in [0u32, 1, 30, 70, 100, 110] {
                    let n = 20_000;
                    let h: f64 = (0..n)
                        .map(|k| {
                            p.mortality
                                .hazard(sex, age as f64 + (k as f64 + 0.5) / n as f64, year)
                        })
                        .sum::<f64>()
                        / n as f64;
                    let q = 1.0 - exp(-h);
                    let got = p.mortality.death_prob(sex, age, year);
                    assert!(
                        (got - q).abs() < 1e-7 * q.max(1e-3),
                        "{sex:?} {year} {age}: {got} vs {q}"
                    );
                }
            }
        }
    }

    #[test]
    fn first_union_hazard_hits_median_and_ever_share() {
        let p = us();
        let fu = &p.unions.first_union;
        for year in [1900, 1956, 2024] {
            let mut never = 1.0;
            let mut median = None;
            for age in 0..=65u32 {
                never *= 1.0 - p.unions.first_union_hazard(Sex::Female, age, year);
                if median.is_none() && never <= 1.0 - 0.5 * fu.ever_partnered.at(year) {
                    median = Some(age);
                }
            }
            let target = fu.median_female.at(year);
            let m = median.unwrap() as f64;
            assert!((m - target).abs() <= 1.5, "{year}: median {m} vs {target}");
            assert!(
                ((1.0 - never) - fu.ever_partnered.at(year)).abs() < 0.02,
                "{year}: ever {}",
                1.0 - never
            );
        }
    }

    #[test]
    fn pmfs_are_normalised() {
        let p = us();
        for year in [1840, 1900, 1957, 2024, 2100] {
            let s: f64 = p.fertility.union_parity_pmf(year, 1.0).iter().sum();
            assert!((s - 1.0).abs() < 1e-9, "{year} parity sum {s}");
            let s: f64 = p.dissolution.pmf(year).iter().sum();
            assert!((s - 1.0).abs() < 1e-9);
            for remarriage in [false, true] {
                let classes = p.dissolution.class_pmf(year, remarriage);
                let s: f64 = classes.iter().sum();
                assert!((s - 1.0).abs() < 1e-9, "{year} class sum {s}");
            }
            // The classes keep the bands' divorce share for first unions.
            let classes = p.dissolution.class_pmf(year, false);
            assert!((classes[0] - p.dissolution.pmf(year)[0]).abs() < 1e-12);
            let s: f64 = p.fertility.first_birth_offset_pmf(year).iter().sum();
            assert!((s - 1.0).abs() < 1e-9);
            let s: f64 = p.fertility.nonunion_count_pmf(year, 1.0).iter().sum();
            assert!((s - 1.0).abs() < 1e-9);
        }
        let g: f64 = (-40..=40).map(|gap| p.unions.age_gap_weight(gap)).sum();
        assert!((g - 1.0).abs() < 0.03, "gap kernel mass {g}");
    }

    #[test]
    fn tilting_scales_the_mean_and_keeps_a_distribution() {
        let p = us();
        let mean = |pmf: &[f64; 9]| {
            pmf.iter()
                .enumerate()
                .map(|(k, x)| k as f64 * x)
                .sum::<f64>()
        };
        for year in [1840, 1900, 1957, 2024] {
            let base = p.fertility.union_parity_pmf(year, 1.0);
            assert_eq!(
                base,
                p.fertility.union_parity.at::<9>(year),
                "factor 1 is the base"
            );
            for f in [0.7, 0.9, 1.2, 1.5] {
                let t = p.fertility.union_parity_pmf(year, f);
                assert!((t.iter().sum::<f64>() - 1.0).abs() < 1e-12);
                assert!((mean(&t) / mean(&base) - f).abs() < 1e-9, "{year} {f}");
                // More births mean fewer childless.
                assert_eq!(t[0] < base[0], f > 1.0);
            }
        }
    }

    #[test]
    fn a_directory_pack_extends_an_embedded_one() {
        // The variant from worlds/README.md: only the immigration anchors change.
        let root = std::env::temp_dir().join(format!("internot-packs-{}", std::process::id()));
        let dir = root.join("us-closed");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("world.ron"),
            "(name: \"United States without immigration\", description: \"\", extends: Some(\"us\"))",
        )
        .unwrap();
        std::fs::write(
            dir.join("immigration.ron"),
            "(rate: (anchors: [(1840, 0.0)]))",
        )
        .unwrap();
        let p = Params::load(&root, "us-closed").unwrap();
        let us = us();
        assert_eq!(p.immigration.rate(1900), 0.0);
        assert_eq!(
            p.immigration.rate.scale, us.immigration.rate.scale,
            "inherited"
        );
        assert_eq!(p.y0, us.y0);
        assert_ne!(p.fingerprint, us.fingerprint);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_bad_pack_says_where() {
        static BAD: &[EmbeddedFile] = &[
            EmbeddedFile {
                pack: "bad",
                path: "world.ron",
                bytes: b"(name: \"Bad\", description: \"\", extends: Some(\"us\"), timeline: (first_year: 1900, last_year: 1950, founder_max_age: 80), founders: (births: 300.0, growth: 0.01), male_share_at_birth: 0.5, test_boosts: (same_sex: 1.0, heritage_floor: 0.0, open_market: 1.0))",
            },
            EmbeddedFile {
                pack: "bad",
                path: "unions.ron",
                bytes: b"(national_market_share: [(1900, 1.5)])",
            },
        ];
        let e =
            Params::from_pack(&Pack::load("bad", &[&Embedded(BAD), &Embedded(EMBEDDED)]).unwrap())
                .unwrap_err();
        assert_eq!(e.file.as_deref(), Some("unions.ron"), "{e}");
        assert!(e.message.contains("1.5"), "{e}");
    }
}
