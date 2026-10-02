//! Residence (L4, design A): where every household lives at `t`, and every
//! household living in a place at `t`, exactly. Prototype of spec
//! `docs/superpowers/specs/2026-10-01-residence.md`.
//!
//! - **Places** are a fixed tree: areas ⊃ commuting zones ⊃ counties ⊃
//!   clusters ⊃ tracts, with attractiveness by decade. Areas are the closed
//!   units of design A.
//! - **Units** have addresses: a union, or a single adult's spell. Each has
//!   a position over its whole span, latent while nobody lives there, and
//!   households live at their anchor unit's position (§2.2).
//! - **Position at `t`** is nested regeneration: a move at level k redraws
//!   levels ≥ k, so each level is the draw at its last regeneration, one
//!   `last_before` per level and no replay (§2.4). A new unit starts near
//!   its source unit (§2.3).
//! - **Moves between zones of an area** draw by gravity around the unit's
//!   home zone (its first zone, or where its last long move landed), so
//!   they still regenerate; moves within a zone draw by attractiveness.
//! - **Long moves** (to another area) are a counted flow per (birth block,
//!   year): exact and invertible (§2.5). Founders and immigrants are seeded
//!   the same way (§2.6).
//! - **Rosters:** an area's history is the least fixed point from its
//!   static entries through sources (`fixpoint::closure_layers`), cached
//!   per area (§2.7).

use std::collections::HashMap;
use std::hash::{BuildHasher, Hash};
use std::sync::{Arc, Mutex};

use procedural_core::curve::piecewise_power;
use procedural_core::fixpoint::closure_layers;
use procedural_core::geo::{haversine_miles, LatLon};
use procedural_core::key::{label, Key};
use procedural_core::partition::apportion_systematic;
use procedural_core::perm::{Bijection, CompactPerm, SegmentedPerm};
use procedural_core::sample::frailty;
use procedural_core::stream::{
    nested_regen_anchored, year_of, year_start, Event, PoissonTree, Regen, DAY,
};
use rayon::prelude::*;

use crate::household::Household;
use crate::params::{Residence as Rules, Sex};
use crate::world::{PersonId, World};

const TAG_SEED: u64 = label("residence/seed");
const TAG_SEED_FLOW: u64 = label("residence/seed-flow");
const TAG_FORM: u64 = label("residence/form");
const TAG_SOURCE: u64 = label("residence/source");
const TAG_KEEPER: u64 = label("residence/keeper");
const TAG_SPELL: u64 = label("residence/spell");
const TAG_UNION: u64 = label("residence/union");
const TAG_LOCAL: u64 = label("residence/local");
const TAG_LONG_COUNT: u64 = label("residence/long-count");
const TAG_LONG_PERM: u64 = label("residence/long-perm");
const TAG_LONG_DAY: u64 = label("residence/long-day");
const TAG_LONG_FLOW: u64 = label("residence/long-flow");
const TAG_LONG_EVENT: u64 = label("residence/long-event");
const TAG_FRAILTY: u64 = label("residence/frailty");
const TAG_STAYS: u64 = label("residence/stays");
const TAG_AREA_MOVE: u64 = label("residence/area-move");

/// Levels of the place tree: areas (the closed units), commuting zones,
/// counties (or parts of large ones), clusters of neighbouring tracts, and
/// tracts.
pub const LEVELS: usize = 5;
pub const AREA: usize = 0;
pub const ZONE: usize = 1;
pub const COUNTY: usize = 2;
pub const CLUSTER: usize = 3;
pub const TRACT: usize = 4;

/// Mean Gregorian year, in seconds.
const YEAR: i64 = 31_556_952;

/// Leaves of the local-move trees: 30 days, 2^11 of them (about 168 years,
/// longer than any unit).
const LOCAL_LEAF: i64 = 30 * DAY;
const LOCAL_LEVELS: u8 = 11;

// --- places ---------------------------------------------------------------------

/// The place tree. Each level's nodes are numbered so that a node's
/// children are a contiguous range of the next level.
pub struct Places {
    /// `first[k][i]..first[k][i + 1]`: the children of node `i` at level
    /// `k < TRACT`.
    first: [Vec<u32>; LEVELS - 1],
    /// `parent[k][i]`: the parent of node `i` at level `k ≥ 1`.
    parent: [Vec<u32>; LEVELS],
    /// Lineage region of each area (of its most populous county).
    area_region: Vec<u16>,
    /// Per (area, region) with people of that region's states: weights by
    /// decade. Seeds and long moves of a region use these, so a region whose
    /// states share zones with a bigger neighbour (DC in Virginia's area,
    /// say) still has places.
    region_weight: HashMap<(u32, u16), Vec<f64>>,
    /// Tract centres, and every node's population-weighted centre (in 2020
    /// or the last decade before it).
    tract_at: Vec<LatLon>,
    centre: [Vec<LatLon>; LEVELS],
    /// Year of decade 0 and the number of decades with weights.
    first_decade: i32,
    decades: usize,
    /// Node weights per level, per decade: `weight[k][d · n_k + i]`.
    weight: [Vec<f64>; LEVELS],
    /// Per level `k ≥ 1`, per decade: the cumulative weight of node `i`
    /// among its siblings, inclusive.
    cum: [Vec<f64>; LEVELS],
    /// Names and codes, when the tree comes from data (empty if synthetic).
    area_name: Vec<String>,
    zone_name: Vec<String>,
    county_name: Vec<String>,
    county_fips: Vec<u32>,
    tract_geoid: Vec<u64>,
}

impl Places {
    /// Builds the tree from its tracts, listed in tree order: `tracts[i] =
    /// ([area, zone, county, cluster], centre, weights by decade)`, with
    /// consecutive numbering at every level.
    pub fn from_tracts(
        area_region: Vec<u16>,
        tracts: &[([u32; LEVELS - 1], LatLon, Vec<f64>)],
        first_decade: i32,
    ) -> Self {
        let regions: Vec<u16> = tracts
            .iter()
            .map(|t| area_region[t.0[AREA] as usize])
            .collect();
        Self::from_tracts_with_regions(area_region, tracts, &regions, first_decade)
    }

    /// [`Self::from_tracts`], with each tract's own lineage region.
    pub fn from_tracts_with_regions(
        area_region: Vec<u16>,
        tracts: &[([u32; LEVELS - 1], LatLon, Vec<f64>)],
        tract_region: &[u16],
        first_decade: i32,
    ) -> Self {
        let mut region_weight: HashMap<(u32, u16), Vec<f64>> = HashMap::new();
        for ((path, _, w), &r) in tracts.iter().zip(tract_region) {
            let e = region_weight
                .entry((path[AREA], r))
                .or_insert_with(|| vec![0.0; w.len()]);
            for (a, x) in e.iter_mut().zip(w) {
                *a += x.max(0.0);
            }
        }
        let decades = tracts.first().map_or(1, |t| t.2.len());
        let mut counts = [0usize; LEVELS];
        counts[AREA] = area_region.len();
        counts[TRACT] = tracts.len();
        for (path, _, w) in tracts {
            assert_eq!(w.len(), decades, "every tract needs a weight per decade");
            for k in 1..TRACT {
                counts[k] = counts[k].max(path[k] as usize + 1);
            }
        }
        let mut parent: [Vec<u32>; LEVELS] = Default::default();
        for k in 1..LEVELS {
            parent[k] = vec![0; counts[k]];
        }
        for (i, (path, _, _)) in tracts.iter().enumerate() {
            for k in 1..TRACT {
                parent[k][path[k] as usize] = path[k - 1];
            }
            parent[TRACT][i] = path[CLUSTER];
        }
        let mut first: [Vec<u32>; LEVELS - 1] = Default::default();
        for k in 0..LEVELS - 1 {
            let mut f = vec![0u32; counts[k] + 1];
            for &p in &parent[k + 1] {
                f[p as usize + 1] += 1;
            }
            for i in 0..counts[k] {
                f[i + 1] += f[i];
            }
            // Children must be contiguous and in parent order.
            for (c, &p) in parent[k + 1].iter().enumerate() {
                assert!(
                    (f[p as usize]..f[p as usize + 1]).contains(&(c as u32)),
                    "places must be listed in tree order"
                );
            }
            first[k] = f;
        }
        let mut weight: [Vec<f64>; LEVELS] = Default::default();
        weight[TRACT] = vec![0.0; decades * counts[TRACT]];
        for (i, (_, _, w)) in tracts.iter().enumerate() {
            for d in 0..decades {
                weight[TRACT][d * counts[TRACT] + i] = w[d].max(0.0);
            }
        }
        for k in (0..TRACT).rev() {
            let mut wk = vec![0.0; decades * counts[k]];
            for d in 0..decades {
                for c in 0..counts[k + 1] {
                    let p = parent[k + 1][c] as usize;
                    wk[d * counts[k] + p] += weight[k + 1][d * counts[k + 1] + c];
                }
            }
            weight[k] = wk;
        }
        let mut cum: [Vec<f64>; LEVELS] = Default::default();
        for k in 1..LEVELS {
            let n = counts[k];
            let mut c = vec![0.0; decades * n];
            for d in 0..decades {
                for p in 0..counts[k - 1] {
                    let (lo, hi) = (first[k - 1][p] as usize, first[k - 1][p + 1] as usize);
                    let mut acc = 0.0;
                    for i in lo..hi {
                        acc += weight[k][d * n + i];
                        c[d * n + i] = acc;
                    }
                }
            }
            cum[k] = c;
        }
        // Node centres, weighted by population in 2020 (or the last decade
        // before it); an empty node takes its tracts' plain mean.
        let d = (((2020 - first_decade) / 10).max(0) as usize).min(decades - 1);
        let centre: [Vec<LatLon>; LEVELS] = std::array::from_fn(|k| {
            let mut acc = vec![(0.0, 0.0, 0.0, 0.0, 0.0, 0u32); counts[k]];
            for (i, (path, at, w)) in tracts.iter().enumerate() {
                let node = if k == TRACT { i } else { path[k] as usize };
                let a = &mut acc[node];
                a.0 += w[d] * at.lat;
                a.1 += w[d] * at.lon;
                a.2 += w[d];
                a.3 += at.lat;
                a.4 += at.lon;
                a.5 += 1;
            }
            acc.iter()
                .map(|a| {
                    if a.2 > 0.0 {
                        LatLon::new(a.0 / a.2, a.1 / a.2)
                    } else {
                        LatLon::new(a.3 / a.5.max(1) as f64, a.4 / a.5.max(1) as f64)
                    }
                })
                .collect()
        });
        Self {
            first,
            parent,
            area_region,
            region_weight,
            tract_at: tracts.iter().map(|t| t.1).collect(),
            centre,
            first_decade,
            decades,
            weight,
            cum,
            area_name: Vec::new(),
            zone_name: Vec::new(),
            county_name: Vec::new(),
            county_fips: Vec::new(),
            tract_geoid: Vec::new(),
        }
    }

    /// The pack's place tree (`places.ron` and its data file), with each
    /// area in the lineage region of its most populous county (in 2020).
    pub fn from_params(p: &crate::params::Params) -> Result<Self, String> {
        let mut places = Self::from_data(&p.place_data, |state| {
            if p.places.by_area {
                0
            } else {
                p.region_of_state(state)
            }
        })?;
        if p.places.by_area {
            // Each area is its own region: the world region named after it.
            let regions: Vec<u16> = places
                .area_name
                .iter()
                .map(|name| {
                    p.region_by_id(name)
                        .ok_or_else(|| format!("no world region is named after area `{name}`"))
                })
                .collect::<Result<_, _>>()?;
            let mut by_area: HashMap<(u32, u16), Vec<f64>> = HashMap::new();
            for ((a, _), w) in places.region_weight.drain() {
                let e = by_area
                    .entry((a, regions[a as usize]))
                    .or_insert_with(|| vec![0.0; w.len()]);
                for (x, y) in e.iter_mut().zip(&w) {
                    *x += y;
                }
            }
            places.region_weight = by_area;
            places.area_region = regions;
        }
        Ok(places)
    }

    /// Parses `places.bin` (format in `internot_society/data/distill_places.py`).
    pub fn from_data(b: &[u8], region_of_state: impl Fn(u8) -> u16) -> Result<Self, String> {
        struct Reader<'a> {
            b: &'a [u8],
            i: usize,
        }
        impl Reader<'_> {
            fn take(&mut self, n: usize) -> Result<&[u8], String> {
                let s = self
                    .b
                    .get(self.i..self.i + n)
                    .ok_or("places data is truncated")?;
                self.i += n;
                Ok(s)
            }
            fn leb(&mut self) -> Result<u64, String> {
                let (mut n, mut shift) = (0u64, 0);
                loop {
                    let x = self.take(1)?[0];
                    n |= ((x & 0x7f) as u64) << shift;
                    if x < 0x80 {
                        return Ok(n);
                    }
                    shift += 7;
                }
            }
            fn text(&mut self) -> Result<String, String> {
                let n = self.take(1)?[0] as usize;
                Ok(String::from_utf8_lossy(self.take(n)?).into_owned())
            }
            fn f32(&mut self) -> Result<f32, String> {
                Ok(f32::from_le_bytes(self.take(4)?.try_into().unwrap()))
            }
            fn index(&mut self, len: usize, what: &str) -> Result<u32, String> {
                let i = self.leb()? as usize;
                if i < len {
                    Ok(i as u32)
                } else {
                    Err(format!("places data: {what} index {i} out of range"))
                }
            }
        }
        let mut r = Reader { b, i: 0 };
        if r.take(8)? != b"INTPLACE" || r.take(1)?[0] != 2 {
            return Err("not a version-2 places file".into());
        }
        let head = r.take(3)?;
        let first_decade = u16::from_le_bytes([head[0], head[1]]) as i32;
        let decades = head[2] as usize;
        let na = r.leb()? as usize;
        let area_name: Vec<String> = (0..na).map(|_| r.text()).collect::<Result<_, _>>()?;
        let nz = r.leb()? as usize;
        let mut zone_area = Vec::with_capacity(nz);
        let mut zone_name = Vec::with_capacity(nz);
        for _ in 0..nz {
            let _code = r.leb()?;
            zone_area.push(r.index(na, "area")?);
            zone_name.push(r.text()?);
        }
        let nc = r.leb()? as usize;
        let (mut county_fips, mut county_zone, mut county_name) =
            (Vec::new(), Vec::new(), Vec::new());
        for _ in 0..nc {
            county_fips.push(r.leb()? as u32);
            county_zone.push(r.index(nz, "zone")?);
            county_name.push(r.text()?);
        }
        let nl = r.leb()? as usize;
        let cluster_county: Vec<u32> = (0..nl)
            .map(|_| r.index(nc, "county"))
            .collect::<Result<_, _>>()?;
        let nt = r.leb()? as usize;
        let mut tracts = Vec::with_capacity(nt);
        let mut geoids = Vec::with_capacity(nt);
        let mut county_w = vec![0.0f64; nc];
        let d2020 = (((2020 - first_decade) / 10).max(0) as usize).min(decades.max(1) - 1);
        for _ in 0..nt {
            let geoid = r.leb()?;
            let (lat, lon) = (r.f32()? as f64, r.f32()? as f64);
            let cl = r.index(nl, "cluster")?;
            let w: Vec<f64> = (0..decades)
                .map(|_| r.leb().map(|x| x as f64))
                .collect::<Result<_, _>>()?;
            let county = cluster_county[cl as usize];
            county_w[county as usize] += w[d2020];
            let zone = county_zone[county as usize];
            tracts.push((
                [zone_area[zone as usize], zone, county, cl],
                LatLon::new(lat, lon),
                w,
            ));
            geoids.push(geoid);
        }
        // Each area takes the region of its most populous county.
        let mut best = vec![(f64::MIN, 0u32); na];
        for (c, &z) in county_zone.iter().enumerate() {
            let a = zone_area[z as usize] as usize;
            if county_w[c] > best[a].0 {
                best[a] = (county_w[c], county_fips[c]);
            }
        }
        let area_region = best
            .iter()
            .map(|&(_, fips)| region_of_state((fips / 1000) as u8))
            .collect();
        let tract_region: Vec<u16> = geoids
            .iter()
            .map(|&g| region_of_state((g / 1_000_000_000) as u8))
            .collect();
        let mut places =
            Self::from_tracts_with_regions(area_region, &tracts, &tract_region, first_decade);
        places.area_name = area_name;
        places.zone_name = zone_name;
        places.county_name = county_name;
        places.county_fips = county_fips;
        places.tract_geoid = geoids;
        Ok(places)
    }

    /// The area's name (state and part), if the tree comes from data.
    pub fn area_name(&self, a: u32) -> Option<&str> {
        self.area_name.get(a as usize).map(String::as_str)
    }

    /// The zone's name (its commuting zone), if the tree comes from data.
    pub fn zone_name(&self, z: u32) -> Option<&str> {
        self.zone_name.get(z as usize).map(String::as_str)
    }

    /// The county node's name and FIPS code, if the tree comes from data.
    pub fn county(&self, c: u32) -> Option<(&str, u32)> {
        Some((
            self.county_name.get(c as usize)?.as_str(),
            *self.county_fips.get(c as usize)?,
        ))
    }

    /// The tract's census GEOID, if the tree comes from data.
    pub fn tract_geoid(&self, t: u32) -> Option<u64> {
        self.tract_geoid.get(t as usize).copied()
    }

    /// A synthetic country for tests and prototypes: per region, `shape =
    /// [areas, zones per area, counties per zone, clusters per county, tracts
    /// per cluster]` on a grid of points, with keyed weights that grow or
    /// shrink by zone over `decades` decades from `first_decade`.
    pub fn synthetic(
        regions: u16,
        shape: [u32; LEVELS],
        first_decade: i32,
        decades: usize,
        key: Key,
    ) -> Self {
        let mut area_region = Vec::new();
        let mut tracts = Vec::new();
        let mut next = [0u32; LEVELS - 1];
        for r in 0..regions {
            for ai in 0..shape[0] {
                area_region.push(r);
                for zi in 0..shape[1] {
                    // Zones about 60 miles apart; areas and regions side by side.
                    let (zlat, zlon) = (
                        35.0 + zi as f64 * 0.9,
                        -110.0 + r as f64 * 20.0 + ai as f64 * 4.0 + (zi % 3) as f64 * 1.0,
                    );
                    let growth = key.with2(next[ZONE] as u64, 1).unit() * 0.08 - 0.02;
                    for ci in 0..shape[2] {
                        for li in 0..shape[3] {
                            for ti in 0..shape[4] {
                                let k = key.with3(
                                    next[ZONE] as u64,
                                    next[COUNTY] as u64,
                                    (next[CLUSTER] as u64) << 16 | ti as u64,
                                );
                                let at = LatLon::new(
                                    zlat + ci as f64 * 0.15 + li as f64 * 0.04 + ti as f64 * 0.01,
                                    zlon + ci as f64 * 0.15 + k.with(1).unit() * 0.03,
                                );
                                let base = 0.3 + 2.0 * k.with(2).unit();
                                let w = (0..decades)
                                    .scan(base, |acc, _| {
                                        let now = *acc;
                                        *acc *= 1.0 + growth;
                                        Some(now)
                                    })
                                    .collect();
                                tracts.push((
                                    [next[AREA], next[ZONE], next[COUNTY], next[CLUSTER]],
                                    at,
                                    w,
                                ));
                            }
                            next[CLUSTER] += 1;
                        }
                        next[COUNTY] += 1;
                    }
                    next[ZONE] += 1;
                }
                next[AREA] += 1;
            }
        }
        Self::from_tracts(area_region, &tracts, first_decade)
    }

    /// Number of nodes at level `k`.
    pub fn count(&self, k: usize) -> usize {
        if k == AREA {
            self.area_region.len()
        } else {
            self.parent[k].len()
        }
    }

    /// The parent of node `i` at level `k ≥ 1`.
    pub fn parent(&self, k: usize, i: u32) -> u32 {
        self.parent[k][i as usize]
    }

    /// The ancestor at level `up ≤ k` of node `i` at level `k`.
    pub fn ancestor(&self, k: usize, mut i: u32, up: usize) -> u32 {
        for level in (up + 1..=k).rev() {
            i = self.parent[level][i as usize];
        }
        i
    }

    /// The children of node `i` at level `k < TRACT`.
    pub fn children(&self, k: usize, i: u32) -> std::ops::Range<u32> {
        self.first[k][i as usize]..self.first[k][i as usize + 1]
    }

    /// The tracts under node `i` at level `k`.
    pub fn tracts_under(&self, k: usize, i: u32) -> std::ops::Range<u32> {
        let mut r = i..i + 1;
        for level in k..TRACT {
            r = self.first[level][r.start as usize]..self.first[level][r.end as usize];
        }
        r
    }

    /// Lineage region of area `a`.
    pub fn region(&self, a: u32) -> u16 {
        self.area_region[a as usize]
    }

    /// Centre of tract `t`.
    pub fn tract_at(&self, t: u32) -> LatLon {
        self.tract_at[t as usize]
    }

    /// Population-weighted centre of node `i` at level `k`.
    pub fn centre(&self, k: usize, i: u32) -> LatLon {
        self.centre[k][i as usize]
    }

    fn decade(&self, year: i32) -> usize {
        ((year - self.first_decade).div_euclid(10)).clamp(0, self.decades as i32 - 1) as usize
    }

    /// Weight of region `r`'s people in area `a` in `year`.
    pub fn region_weight(&self, a: u32, r: u16, year: i32) -> f64 {
        self.region_weight
            .get(&(a, r))
            .map_or(0.0, |w| w[self.decade(year)])
    }

    /// Weight of node `i` at level `k` in `year`.
    pub fn weight(&self, k: usize, i: u32, year: i32) -> f64 {
        self.weight[k][self.decade(year) * self.count(k) + i as usize]
    }

    /// A child of node `parent` (level `k − 1`) drawn by weight in `year`;
    /// uniformly if every child weighs nothing.
    pub fn draw_child(&self, k: usize, parent: u32, year: i32, key: Key) -> u32 {
        let r = self.children(k - 1, parent);
        let n = self.count(k);
        let base = self.decade(year) * n;
        let slice = &self.cum[k][base + r.start as usize..base + r.end as usize];
        let total = *slice.last().expect("every node has children");
        if total <= 0.0 {
            return r.start + key.below((r.end - r.start) as u64) as u32;
        }
        let u = key.unit() * total;
        let i = slice.partition_point(|&c| c <= u).min(slice.len() - 1);
        r.start + i as u32
    }
}

/// Destinations of moves at one level, by gravity around an anchor: for
/// each parent node, the kernel between its children (by the distance
/// between their centres), times the children's attractiveness by decade.
struct LevelGravity {
    level: usize,
    /// Per parent node: where its block starts in `kernel`.
    start: Vec<usize>,
    /// Per parent, `n × n` kernel values between its children (row: the
    /// anchor), row-major.
    kernel: Vec<f64>,
}

impl LevelGravity {
    fn new(places: &Places, level: usize, g: &crate::params::Gravity) -> Self {
        let parents = places.count(level - 1) as u32;
        let mut start = Vec::with_capacity(parents as usize);
        let mut kernel = Vec::new();
        for p in 0..parents {
            start.push(kernel.len());
            let kids = places.children(level - 1, p);
            for h in kids.clone() {
                for j in kids.clone() {
                    let miles = haversine_miles(places.centre(level, h), places.centre(level, j));
                    kernel.push(piecewise_power(miles, g.flat_miles, &g.segments, g.beyond));
                }
            }
        }
        Self {
            level,
            start,
            kernel,
        }
    }

    /// A child of `parent` drawn around `anchor` in `year`: attractiveness
    /// times the kernel from the anchor.
    fn draw(&self, places: &Places, parent: u32, anchor: u32, year: i32, key: Key) -> u32 {
        let kids = places.children(self.level - 1, parent);
        let n = (kids.end - kids.start) as usize;
        let row = self.start[parent as usize] + (anchor - kids.start) as usize * n;
        let mut total = 0.0;
        let mut cum = [0.0f64; 512];
        let big = n > cum.len();
        let mut heap = if big { vec![0.0; n] } else { Vec::new() };
        let c: &mut [f64] = if big { &mut heap } else { &mut cum[..n] };
        for (i, j) in kids.clone().enumerate() {
            total += places.weight(self.level, j, year) * self.kernel[row + i];
            c[i] = total;
        }
        if total <= 0.0 {
            return kids.start + key.below(n as u64) as u32;
        }
        let u = key.unit() * total;
        kids.start + c.partition_point(|&x| x <= u).min(n - 1) as u32
    }
}

/// A memo split over locked shards, so parallel closures rarely contend.
struct Memo<K, V> {
    shards: Vec<Mutex<HashMap<K, V>>>,
    hasher: std::hash::BuildHasherDefault<std::collections::hash_map::DefaultHasher>,
}

impl<K: Hash + Eq, V: Clone> Memo<K, V> {
    const SHARDS: usize = 64;

    fn new() -> Self {
        Self {
            shards: (0..Self::SHARDS)
                .map(|_| Mutex::new(HashMap::new()))
                .collect(),
            hasher: Default::default(),
        }
    }

    fn shard(&self, k: &K) -> &Mutex<HashMap<K, V>> {
        &self.shards[self.hasher.hash_one(k) as usize % Self::SHARDS]
    }

    fn get(&self, k: &K) -> Option<V> {
        self.shard(k).lock().unwrap().get(k).cloned()
    }

    fn insert(&self, k: K, v: V) {
        self.shard(&k).lock().unwrap().insert(k, v);
    }

    fn clear(&self) {
        for s in &self.shards {
            s.lock().unwrap().clear();
        }
    }
}

/// A position: the node at each level, area first.
pub type Pos = [u32; LEVELS];

/// A `[from, to)` stretch of time at one position.
pub type Stretch = (i64, i64, Pos);

/// A unit's first area and its long moves `(time, destination)`.
type AreaTrack = (Info, u32, Vec<(i64, u32)>);

// --- units --------------------------------------------------------------------------

/// Something with an address: a union, or the `s`-th single stretch of a
/// person's independent life (`s` counts the unions ended before).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Unit {
    Union {
        a: PersonId,
        b: PersonId,
        start: i64,
    },
    Spell {
        x: PersonId,
        s: u8,
    },
}

/// Where a unit's first position comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// Placed by a static seed flow (founders, immigrants).
    Seed,
    /// Near `unit`'s position at the start: keeping its dwelling, or drawn
    /// afresh from `level` down.
    From { unit: Unit, keep: bool, level: u8 },
    /// Area mode: drawn afresh in `area`, the unit's ledger area, when no
    /// source lives there (ledger spec §9).
    Fresh { area: u32 },
    /// Area mode: after a separation of a couple that moved, near where the
    /// union formed (`unit`'s first position), drawn from `level` down: the
    /// divorced are counted in the formation area (v1).
    Back { unit: Unit, level: u8 },
}

/// A unit's facts.
#[derive(Clone, Copy, Debug)]
pub struct Info {
    /// In-world span `[start, end)`.
    pub start: i64,
    pub end: i64,
    /// Who drives its moves: the single adult, or the couple's decider.
    pub driver: PersonId,
    pub source: Source,
    key: Key,
}

/// An area's history: every unit ever in it, with where it was when, and
/// an index of those stretches by tract.
pub struct History {
    pub area: u32,
    pub members: Vec<Member>,
    /// `(tract, from, to, member)` for every stretch, sorted.
    index: Vec<(u32, i64, i64, u32)>,
}

/// A unit in an area's history.
#[derive(Clone, Debug)]
pub struct Member {
    pub unit: Unit,
    pub info: Info,
    /// `[from, to)` stretches in the area, with the position held.
    pub stays: Vec<(i64, i64, Pos)>,
}

/// Counted long moves of one block: counts by year from `first_year`.
struct LongRow {
    first_year: i32,
    counts: Vec<u32>,
}

/// A counted flow: a dense domain of contiguous pieces (by block or
/// cohort), shuffled onto area segments.
struct Flow {
    /// `(block, cohort, first raw id)` of each piece, sorted.
    pieces: Vec<(u32, u32, u64)>,
    /// Where each piece starts in the domain; the last entry is its size.
    starts: Vec<u64>,
    seg: SegmentedPerm,
}

impl Flow {
    fn new(
        pieces: Vec<(u32, u32, u64, u64)>,
        sizes_by_area: impl FnOnce(u64) -> Vec<u64>,
        key: Key,
    ) -> Self {
        let mut starts = Vec::with_capacity(pieces.len() + 1);
        let mut at = 0;
        for p in &pieces {
            starts.push(at);
            at += p.3;
        }
        starts.push(at);
        let sizes = sizes_by_area(at);
        Self {
            pieces: pieces.into_iter().map(|p| (p.0, p.1, p.2)).collect(),
            starts,
            seg: SegmentedPerm::new(&sizes, key),
        }
    }

    /// Domain index of `(block, cohort)` offset `off`.
    fn index(&self, block: u32, cohort: u32, off: u64) -> Option<u64> {
        let i = self
            .pieces
            .binary_search_by(|p| (p.0, p.1).cmp(&(block, cohort)))
            .ok()?;
        Some(self.starts[i] + off)
    }

    /// The piece and offset of domain index `i`.
    fn locate(&self, i: u64) -> ((u32, u32, u64), u64) {
        let k = self.starts.partition_point(|&s| s <= i) - 1;
        (self.pieces[k], i - self.starts[k])
    }
}

/// Residence for one world and one place tree.
pub struct Residence {
    places: Places,
    /// Gravity draws around an anchor, for the zone, county and cluster
    /// levels.
    gravity: [Option<LevelGravity>; LEVELS],
    rules: Rules,
    key: Key,
    y0: i32,
    y1: i32,
    world_start: i64,
    /// Per block: counted long moves by year.
    long: Vec<LongRow>,
    /// Per (region, year − y0): the long-move flow.
    long_flows: Vec<Option<Flow>>,
    /// Per region: the founders' seed flow.
    founder_flows: Vec<Option<Flow>>,
    /// Per (region, year − y0): the arrivals' seed flow.
    arrival_flows: Vec<Option<Flow>>,
    /// Cumulative local-move rate by (birth year − ledger first year, year
    /// − y0), for a unit's driver.
    local_cum: Arc<Vec<f64>>,
    first_birth_year: i32,
    years: usize,
    memo: Memo<Unit, (Info, Pos)>,
    infos: Memo<Unit, Option<Info>>,
    /// Area mode: each unit's moves to another area, from the ledger.
    area_moves: Memo<Unit, Arc<[(i64, u32)]>>,
    /// Area mode (`places.by_area`): areas follow the ledger (spec §9).
    areas_on: bool,
    /// Area mode: the place area of each lineage region (the ledger counts
    /// areas as regions), `u32::MAX` for a region without one.
    region_area: Vec<u32>,
    facts: Memo<PersonId, Arc<Facts>>,
    histories: Mutex<HashMap<u32, Arc<History>>>,
}

/// A person's facts that residence reads often, memoized: unions by start,
/// entry into the world, and first independence.
struct Facts {
    unions: Vec<crate::world::Union>,
    entry: i64,
    spell0: Option<i64>,
}

impl Residence {
    /// Builds the counted flows for world `w` on `places`. The places'
    /// areas must use the world's lineage regions.
    pub fn build(w: &World, places: Places) -> Self {
        let ledger = w.ledger();
        let p = &ledger.params;
        let rules = p.residence.clone();
        let key = w.key().with(label("residence"));
        let (y0, y1) = (p.y0, p.y1);
        let regions = p.region_count();
        assert!(
            places.area_region.iter().all(|&r| (r as usize) < regions),
            "areas must use the world's lineage regions"
        );
        let years = (y1 - y0 + 1) as usize;
        let nb = places.count(AREA);

        // Long moves: counts per (block, year), then a flow per (region, year).
        // Area mode takes every move between areas from the ledger instead.
        let areas_on = p.places.by_area;
        let mut long = Vec::with_capacity(ledger.blocks.len());
        let mut by_year: Vec<Vec<(u32, u32, u64, u64)>> = vec![Vec::new(); regions * years];
        for (bi, block) in ledger.blocks.iter().enumerate() {
            if areas_on {
                long.push(LongRow {
                    first_year: y0,
                    counts: Vec::new(),
                });
                continue;
            }
            let first_year = block.year.max(y0);
            let last = (block.year + crate::params::MAX_AGE as i32).min(y1);
            let mut counts = Vec::new();
            for y in first_year..=last {
                let rate = rules.long.by_age.at(y - block.year) * rules.long.era.at(y);
                let e = rate * block.size as f64;
                let k = key.with3(TAG_LONG_COUNT, bi as u64, y as i64 as u64);
                let c = e.floor() as u64 + (k.unit() < e - e.floor()) as u64;
                let c = c.min(block.size) as u32;
                counts.push(c);
                if c > 0 {
                    by_year[block.region as usize * years + (y - y0) as usize]
                        .push((bi as u32, 0, 0, c as u64));
                }
            }
            long.push(LongRow { first_year, counts });
        }
        let long_flows = by_year
            .into_iter()
            .enumerate()
            .map(|(i, pieces)| {
                if pieces.is_empty() {
                    return None;
                }
                let (r, y) = ((i / years) as u16, y0 + (i % years) as i32);
                let shares = Self::long_shares(&places, &rules, r, y);
                let k = key.with3(TAG_LONG_FLOW, r as u64, y as i64 as u64);
                Some(Flow::new(
                    pieces,
                    |n| {
                        let mut out = vec![0; nb];
                        apportion_systematic(n, &shares, k.with(1), &mut out);
                        out
                    },
                    k,
                ))
            })
            .collect();

        // Seeds: founders per region, arrivals per (region, year).
        let mut founders: Vec<Vec<(u32, u32, u64, u64)>> = vec![Vec::new(); regions];
        let mut arrivals: Vec<Vec<(u32, u32, u64, u64)>> = vec![Vec::new(); regions * years];
        for (bi, block) in ledger.blocks.iter().enumerate() {
            let mut raw = 0u64;
            for (ci, co) in block.cohorts.iter().enumerate() {
                match co.arrival {
                    None if block.founder && co.size > 0 => {
                        founders[block.region as usize].push((bi as u32, ci as u32, raw, co.size))
                    }
                    Some(a) if co.size > 0 && (y0..=y1).contains(&a) => arrivals
                        [block.region as usize * years + (a - y0) as usize]
                        .push((bi as u32, ci as u32, raw, co.size)),
                    _ => {}
                }
                raw += co.size;
            }
        }
        let seed_flow = |pieces: Vec<(u32, u32, u64, u64)>, r: u16, y: i32, k: Key| {
            if pieces.is_empty() {
                return None;
            }
            // Settle by the region's weights that year, or by its latest
            // weights if it had nobody yet (immigrants can arrive before a
            // territory's first census).
            let in_region = |year: i32| -> Vec<f64> {
                (0..nb as u32)
                    .map(|b| places.region_weight(b, r, year))
                    .collect()
            };
            let mut shares = in_region(y);
            if shares.iter().all(|&x| x <= 0.0) {
                shares = in_region(y1);
            }
            if shares.iter().all(|&x| x <= 0.0) {
                // A region with no places at all: anywhere, by weight.
                shares = (0..nb as u32).map(|b| places.weight(AREA, b, y1)).collect();
            }
            Some(Flow::new(
                pieces,
                |n| {
                    let mut out = vec![0; nb];
                    apportion_systematic(n, &shares, k.with(1), &mut out);
                    out
                },
                k,
            ))
        };
        let founder_flows = founders
            .into_iter()
            .enumerate()
            .map(|(r, pieces)| seed_flow(pieces, r as u16, y0, key.with2(TAG_SEED_FLOW, r as u64)))
            .collect();
        let arrival_flows = arrivals
            .into_iter()
            .enumerate()
            .map(|(i, pieces)| {
                let (r, y) = ((i / years) as u16, y0 + (i % years) as i32);
                seed_flow(
                    pieces,
                    r,
                    y,
                    key.with3(TAG_SEED_FLOW, r as u64, y as i64 as u64 + 1),
                )
            })
            .collect();

        // Local moves: cumulative rate by driver birth year and year.
        let first_birth_year = ledger.first_year;
        let births = (y1 - first_birth_year + 1) as usize;
        let mut local_cum = vec![0.0; births * (years + 1)];
        for bi in 0..births {
            let by = first_birth_year + bi as i32;
            let mut acc = 0.0;
            for yi in 0..years {
                local_cum[bi * (years + 1) + yi] = acc;
                let y = y0 + yi as i32;
                acc += rules.local.by_age.at(y - by) * rules.local.era.at(y);
            }
            local_cum[bi * (years + 1) + years] = acc;
        }

        let mut region_area = vec![u32::MAX; regions];
        for a in 0..nb as u32 {
            region_area[places.region(a) as usize] = a;
        }
        let gravity = std::array::from_fn(|k| match k {
            ZONE => Some(LevelGravity::new(&places, ZONE, &rules.gravity)),
            COUNTY | CLUSTER => Some(LevelGravity::new(&places, k, &rules.local_gravity)),
            _ => None,
        });
        Self {
            places,
            gravity,
            rules,
            key,
            y0,
            y1,
            world_start: year_start(y0),
            long,
            long_flows,
            founder_flows,
            arrival_flows,
            local_cum: Arc::new(local_cum),
            first_birth_year,
            years,
            memo: Memo::new(),
            infos: Memo::new(),
            area_moves: Memo::new(),
            areas_on,
            region_area,
            facts: Memo::new(),
            histories: Mutex::new(HashMap::new()),
        }
    }

    /// The place tree.
    pub fn places(&self) -> &Places {
        &self.places
    }

    /// Destination shares of region `r`'s long moves in `year`: the own
    /// region's share spread by area weight, the rest over other regions.
    fn long_shares(places: &Places, rules: &Rules, r: u16, year: i32) -> Vec<f64> {
        let nb = places.count(AREA) as u32;
        let own: Vec<f64> = (0..nb).map(|b| places.region_weight(b, r, year)).collect();
        let all: Vec<f64> = (0..nb).map(|b| places.weight(AREA, b, year)).collect();
        let (own_total, all_total) = (own.iter().sum::<f64>(), all.iter().sum::<f64>());
        let own_share = if own_total > 0.0 {
            rules.long.own_region
        } else {
            0.0
        };
        (0..nb as usize)
            .map(|b| {
                own_share
                    * if own_total > 0.0 {
                        own[b] / own_total
                    } else {
                        0.0
                    }
                    + (1.0 - own_share)
                        * if all_total > 0.0 {
                            all[b] / all_total
                        } else {
                            0.0
                        }
            })
            .collect()
    }

    // --- people and time -------------------------------------------------------------

    /// When `x` enters the world: birth, the world's start for founders, or
    /// arrival.
    fn entry(&self, w: &World, x: PersonId) -> i64 {
        self.facts(w, x).entry
    }

    /// `x`'s memoized facts.
    fn facts(&self, w: &World, x: PersonId) -> Arc<Facts> {
        if let Some(f) = self.facts.get(&x) {
            return f;
        }
        let entry = self.entry_raw(w, x);
        let unions = Self::unions_sorted(w, x);
        let spell0 = self.spell0_raw(w, x, entry, &unions);
        let f = Arc::new(Facts {
            unions,
            entry,
            spell0,
        });
        self.facts.insert(x, Arc::clone(&f));
        f
    }

    fn entry_raw(&self, w: &World, x: PersonId) -> i64 {
        if w.is_immigrant(x) {
            w.arrival(x).expect("immigrants arrive")
        } else {
            w.birth(x).max(self.world_start)
        }
    }

    /// `x`'s unions, by start.
    fn unions_sorted(w: &World, x: PersonId) -> Vec<crate::world::Union> {
        let mut us: Vec<_> = w.unions(x).into_iter().flatten().collect();
        us.sort_by_key(|u| u.start);
        us
    }

    /// The first time `x` is independent (L3's `dependent_of` is `None`),
    /// at or after entry: checked at every time that can end dependence.
    pub fn spell0_start(&self, w: &World, x: PersonId) -> Option<i64> {
        self.facts(w, x).spell0
    }

    fn spell0_raw(
        &self,
        w: &World,
        x: PersonId,
        entry: i64,
        unions: &[crate::world::Union],
    ) -> Option<i64> {
        let death = w.death(x);
        let birth = w.birth(x);
        let adult = birth + 18 * YEAR;
        // The usual end of dependence: the first time at or after leaving
        // home that `x` is an adult or in a union. Before it, `x` can only be
        // independent with no custodial parent, so if a parent is present
        // from entry through it, it is the answer (one check, not a scan).
        if let Some(l) = w.leave_time(x) {
            let in_union = |t: i64| unions.iter().any(|u| u.start <= t && t < u.end);
            let usual = if l >= adult || in_union(l) {
                l
            } else {
                let next = unions.iter().map(|u| u.start).filter(|&s| s >= l).min();
                next.map_or(adult, |s| s.min(adult))
            }
            .max(entry);
            let parent_through = [w.mother(x), w.father(x)]
                .into_iter()
                .flatten()
                .any(|p| w.present_at(p, entry) && w.death(p) > usual);
            if usual < death && parent_through && w.dependent_of(x, usual).is_none() {
                return Some(usual);
            }
        }
        let mut cands = vec![entry, adult];
        if let Some(l) = w.leave_time(x) {
            cands.push(l);
        }
        if let Some(u) = unions.first() {
            cands.push(u.start);
        }
        let parents = [w.mother(x), w.father(x)];
        for p in parents.into_iter().flatten() {
            cands.push(w.death(p));
            for g in [w.mother(p), w.father(p)].into_iter().flatten() {
                cands.push(w.death(g));
            }
        }
        for &s in w.siblings(x).iter() {
            cands.push(w.death(s));
        }
        cands.retain(|&t| t >= entry && t < death);
        cands.sort_unstable();
        cands.dedup();
        cands.into_iter().find(|&t| w.dependent_of(x, t).is_none())
    }

    /// The unit whose household `q` lives in at `t` (before kin hosting):
    /// `q`'s union, else `q`'s current spell.
    fn unit_of(&self, w: &World, q: PersonId, t: i64) -> Unit {
        let f = self.facts(w, q);
        match f.unions.iter().find(|u| u.start <= t && t < u.end) {
            Some(u) => Unit::Union {
                a: q.min(u.partner),
                b: q.max(u.partner),
                start: u.start,
            },
            None => Unit::Spell {
                x: q,
                s: f.unions.iter().filter(|u| u.end <= t).count() as u8,
            },
        }
    }

    /// A couple's decider, as L3's `unit()`: the woman, or the lower id for
    /// a same-sex couple.
    fn decider(w: &World, a: PersonId, b: PersonId) -> PersonId {
        match (w.sex(a), w.sex(b)) {
            (Sex::Female, Sex::Male) => a,
            (Sex::Male, Sex::Female) => b,
            _ => a.min(b),
        }
    }

    /// The unit `x` drives at `t`: the union if `x` is its decider, else
    /// `x`'s spell if one spans `t`.
    pub fn driven_unit(&self, w: &World, x: PersonId, t: i64) -> Option<Unit> {
        let f = self.facts(w, x);
        if let Some(u) = f.unions.iter().find(|u| u.start <= t && t < u.end) {
            return (Self::decider(w, x, u.partner) == x).then_some(Unit::Union {
                a: x.min(u.partner),
                b: x.max(u.partner),
                start: u.start,
            });
        }
        let s = Unit::Spell {
            x,
            s: f.unions.iter().filter(|u| u.end <= t).count() as u8,
        };
        let info = self.info(w, s)?;
        (info.start <= t && t < info.end).then_some(s)
    }

    /// A unit's facts, or `None` if it never exists (an empty spell).
    pub fn info(&self, w: &World, u: Unit) -> Option<Info> {
        if let Some(i) = self.infos.get(&u) {
            return i;
        }
        let i = self.compute_info(w, u);
        self.infos.insert(u, i);
        i
    }

    fn compute_info(&self, w: &World, u: Unit) -> Option<Info> {
        match u {
            Unit::Union { a, b, start } => {
                let fa = self.facts(w, a);
                let un = fa
                    .unions
                    .iter()
                    .find(|v| v.partner == b && v.start == start)?;
                let decider = Self::decider(w, a, b);
                let entry = self.entry(w, a).max(self.entry(w, b));
                let begin = start.max(entry);
                if begin >= un.end {
                    return None;
                }
                let key = un.key.with(TAG_UNION);
                // A union that began by the time its people entered the
                // world (founders' unions, couples arriving together) is
                // seeded.
                let source = if start <= entry {
                    Source::Seed
                } else {
                    let woman_first = match (w.sex(a), w.sex(b)) {
                        (Sex::Female, Sex::Male) => Some(a),
                        (Sex::Male, Sex::Female) => Some(b),
                        _ => None,
                    };
                    let u = key.with(TAG_SOURCE).unit();
                    let (p, q) = match woman_first {
                        Some(f) => {
                            let m = if f == a { b } else { a };
                            if u < self.rules.union_source_woman {
                                (f, m)
                            } else {
                                (m, f)
                            }
                        }
                        None => {
                            if u < 0.5 {
                                (a, b)
                            } else {
                                (b, a)
                            }
                        }
                    };
                    let before = start - 1;
                    let present = |z: PersonId| self.entry(w, z) <= before && w.present_at(z, before);
                    // Area mode: the union forms in its cell area, near a
                    // partner the ledger counts there, else afresh in it.
                    let cell_area = self.areas_on.then(|| {
                        let us = w.unions(a);
                        let k = us
                            .iter()
                            .position(|v| v.is_some_and(|v| v.partner == b && v.start == start))
                            .expect("the union is a's");
                        w.union_areas(a)[k].expect("a union has an area")
                    });
                    // The household `z` lives in (a parent's, for a young
                    // partner still at home) must be in the union's area.
                    let here = |z: PersonId| {
                        cell_area.is_none_or(|ar| w.area_at(w.chain_end(z, before), before) == ar)
                    };
                    let from = [p, q].into_iter().find(|&z| present(z) && here(z));
                    match (from, cell_area) {
                        (Some(z), _) => Source::From {
                            unit: self.unit_of(w, w.chain_end(z, before), before),
                            keep: key.with(TAG_STAYS).unit() < self.rules.union_keep,
                            level: self.rules.formation.union.level(key.with(TAG_FORM).unit()),
                        },
                        (None, Some(area)) if present(p) || present(q) => Source::Fresh {
                            area: self.place_area(area),
                        },
                        (None, _) => Source::Seed,
                    }
                };
                Some(Info {
                    start: begin,
                    end: un.end,
                    driver: decider,
                    source,
                    key,
                })
            }
            Unit::Spell { x, s } => {
                let fx = self.facts(w, x);
                let us = &fx.unions;
                let death = w.death(x);
                let key = self.key.with3(TAG_SPELL, x as u64, s as u64);
                let end = us.get(s as usize).map_or(death, |u| u.start).min(death);
                let (start, source) = if s == 0 {
                    let b = self.spell0_start(w, x)?;
                    let level = self
                        .rules
                        .formation
                        .leave_home
                        .level(key.with(TAG_FORM).unit());
                    let before = b - 1;
                    let q = w.chain_end(x, before);
                    // Area mode: near the household only if the ledger
                    // counts it in `x`'s area (of upbringing, or the
                    // migration's destination), else afresh there.
                    let area = self.areas_on.then(|| w.area_at(x, b));
                    let at = |z: PersonId, t: i64| -> Source {
                        match area {
                            Some(ar) if w.area_at(z, t) != ar => Source::Fresh {
                                area: self.place_area(ar),
                            },
                            _ => Source::From {
                                unit: self.unit_of(w, z, t),
                                keep: false,
                                level,
                            },
                        }
                    };
                    let source = if b > self.entry(w, x) && q != x {
                        at(q, before)
                    } else {
                        // Independent on entry: near the mother if she is
                        // here (a child arriving with the parents), else a
                        // seed.
                        match w.mother(x).filter(|&m| w.present_at(m, b)) {
                            Some(m) => at(w.chain_end(m, b), b),
                            None => Source::Seed,
                        }
                    };
                    (b, source)
                } else {
                    let ended = us.get(s as usize - 1)?;
                    let begin = ended.end.max(self.entry(w, x));
                    let union = Unit::Union {
                        a: x.min(ended.partner),
                        b: x.max(ended.partner),
                        start: ended.start,
                    };
                    // Area mode: a separated couple that moved returns to
                    // where it formed (the divorced pool's area, v1).
                    let moved_back = self.areas_on
                        && ended.separation == Some(ended.end)
                        && ended.end > self.entry(w, x)
                        && {
                            let k = w
                                .unions(x)
                                .iter()
                                .position(|v| {
                                    v.is_some_and(|v| {
                                        v.partner == ended.partner && v.start == ended.start
                                    })
                                })
                                .expect("the union is x's");
                            w.union_areas(x)[k] != Some(w.area_at(x, ended.end - 1))
                        };
                    let source = if ended.end <= self.entry(w, x) {
                        Source::Seed
                    } else if moved_back {
                        Source::Back {
                            unit: union,
                            level: self
                                .rules
                                .formation
                                .separation
                                .level(key.with(TAG_FORM).unit()),
                        }
                    } else {
                        let partner_alive = w.death(ended.partner) > ended.end;
                        // The woman, the man, or neither keeps the home (a
                        // widowed survivor always does).
                        let keep = if !partner_alive {
                            true
                        } else {
                            let u = ended.key.with(TAG_KEEPER).unit();
                            let (wo, ma) = (self.rules.stays.woman, self.rules.stays.man);
                            let mixed = w.sex(x) != w.sex(ended.partner);
                            let (first, second) = match w.sex(x) {
                                _ if !mixed => (x.min(ended.partner), x.max(ended.partner)),
                                Sex::Female => (x, ended.partner),
                                Sex::Male => (ended.partner, x),
                            };
                            let cut1 = if mixed { wo } else { (wo + ma) / 2.0 };
                            let keeper = if u < cut1 {
                                Some(first)
                            } else if u < wo + ma {
                                Some(second)
                            } else {
                                None
                            };
                            keeper == Some(x)
                        };
                        Source::From {
                            unit: union,
                            keep,
                            level: self
                                .rules
                                .formation
                                .separation
                                .level(key.with(TAG_FORM).unit()),
                        }
                    };
                    (begin, source)
                };
                if start >= end {
                    return None;
                }
                Some(Info {
                    start,
                    end,
                    driver: x,
                    source,
                    key,
                })
            }
        }
    }

    // --- long moves and seeds ------------------------------------------------------------

    /// If `x` is a long mover in `year`: the rank among its block's movers.
    fn long_rank(&self, w: &World, x: PersonId, year: i32) -> Option<u64> {
        let (block, raw) = w.decode(x);
        let row = &self.long[block as usize];
        let c = *row
            .counts
            .get(usize::try_from(year - row.first_year).ok()?)? as u64;
        if c == 0 {
            return None;
        }
        let size = w.ledger().blocks[block as usize].size;
        let perm = CompactPerm::new(
            size,
            self.key
                .with3(TAG_LONG_PERM, block as u64, year as i64 as u64),
        );
        let r = perm.fwd(raw);
        (r < c).then_some(r)
    }

    /// When `x`'s long move of `year` happens.
    fn long_time(&self, x: PersonId, year: i32) -> i64 {
        let len = year_start(year + 1) - year_start(year);
        year_start(year)
            + self
                .key
                .with3(TAG_LONG_DAY, x as u64, year as i64 as u64)
                .below(len as u64) as i64
    }

    /// Destination area of `x`'s long move of `year` (rank `rank`).
    fn long_dest(&self, w: &World, x: PersonId, year: i32, rank: u64) -> u32 {
        let (block, _) = w.decode(x);
        let region = w.ledger().blocks[block as usize].region as usize;
        let flow = self.long_flows[region * self.years + (year - self.y0) as usize]
            .as_ref()
            .expect("a mover's year has a flow");
        let i = flow
            .index(block, 0, rank)
            .expect("the block is in its year's flow");
        flow.seg.assign(i).0 as u32
    }

    /// `x`'s long moves with times in `(lo, hi)`, as `(time, year, rank)`, in
    /// time order.
    fn long_moves(&self, w: &World, x: PersonId, lo: i64, hi: i64, out: &mut Vec<(i64, i32, u64)>) {
        out.clear();
        if hi <= lo {
            return;
        }
        for year in year_of(lo).max(self.y0)..=year_of(hi - 1).min(self.y1) {
            if let Some(rank) = self.long_rank(w, x, year) {
                let t = self.long_time(x, year);
                if lo < t && t < hi {
                    out.push((t, year, rank));
                }
            }
        }
    }

    /// The last long move of `x` in `(lo, hi)`.
    fn last_long(&self, w: &World, x: PersonId, lo: i64, hi: i64) -> Option<(i64, i32, u64)> {
        if hi <= lo {
            return None;
        }
        for year in (year_of(lo).max(self.y0)..=year_of(hi - 1).min(self.y1)).rev() {
            if let Some(rank) = self.long_rank(w, x, year) {
                let t = self.long_time(x, year);
                if lo < t && t < hi {
                    return Some((t, year, rank));
                }
            }
        }
        None
    }

    /// The seed area of `x` (a founder or an immigrant).
    fn seed_area(&self, w: &World, x: PersonId) -> u32 {
        let (block, raw) = w.decode(x);
        let b = &w.ledger().blocks[block as usize];
        let mut first = 0u64;
        for (ci, co) in b.cohorts.iter().enumerate() {
            if raw < first + co.size {
                let flow = match co.arrival {
                    None => self.founder_flows[b.region as usize].as_ref(),
                    Some(a) => self.arrival_flows
                        [b.region as usize * self.years + (a - self.y0) as usize]
                        .as_ref(),
                }
                .expect("seeded people have a flow");
                let i = flow
                    .index(block, ci as u32, raw - first)
                    .expect("the cohort is in its flow");
                return flow.seg.assign(i).0 as u32;
            }
            first += co.size;
        }
        unreachable!("raw id within its block")
    }

    /// Area mode: `u`'s moves to another area within its span, `(time,
    /// destination)` in time order, memoized: the times at which its
    /// driver's ledger area ([`World::area_at`]) changes, among the
    /// migration move, the couples' moves and planned separations (ledger
    /// spec §9).
    fn ledger_moves(&self, w: &World, u: Unit, info: &Info) -> Arc<[(i64, u32)]> {
        if let Some(m) = self.area_moves.get(&u) {
            return m;
        }
        let x = info.driver;
        let mut cands: Vec<i64> = Vec::new();
        if let Some((_, t)) = w.migration(x) {
            cands.push(t);
        }
        for (mv, un) in w.couple_moves(x).iter().zip(w.unions(x)) {
            if let Some((_, t)) = mv {
                cands.push(*t);
            }
            if let Some(sep) = un.and_then(|un| un.separation) {
                cands.push(sep);
            }
        }
        cands.retain(|&t| info.start < t && t < info.end);
        cands.sort_unstable();
        cands.dedup();
        let mut out = Vec::new();
        for t in cands {
            let (before, after) = (w.area_at(x, t - 1), w.area_at(x, t));
            if before != after {
                out.push((t, self.place_area(after)));
            }
        }
        let out: Arc<[(i64, u32)]> = out.into();
        self.area_moves.insert(u, Arc::clone(&out));
        out
    }

    /// The place area of a ledger area (a lineage region).
    fn place_area(&self, region: u16) -> u32 {
        let a = self.region_area[region as usize];
        assert!(a != u32::MAX, "region {region} has no area");
        a
    }

    /// The key of a ledger area move at `t`.
    fn area_move_key(info: &Info, t: i64) -> Key {
        info.key.with2(TAG_AREA_MOVE, t as u64)
    }

    // --- positions ---------------------------------------------------------------------

    /// The unit's facts and first position, memoized.
    fn initial(&self, w: &World, u: Unit) -> Option<(Info, Pos)> {
        if let Some(v) = self.memo.get(&u) {
            return Some(v);
        }
        let info = self.info(w, u)?;
        let year = year_of(info.start);
        // A new home redraws from level `from` down: the first level around
        // the source's node there (by gravity, where the level has it), the
        // finer ones by attractiveness.
        let draw_from = |mut pos: Pos, from: usize, key: Key, near: bool| {
            for k in from.max(1)..LEVELS {
                let kk = key.with(k as u64);
                pos[k] = match &self.gravity[k] {
                    Some(g) if near && k == from => {
                        g.draw(&self.places, pos[k - 1], pos[k], year, kk)
                    }
                    _ => self.places.draw_child(k, pos[k - 1], year, kk),
                };
            }
            pos
        };
        let pos = match info.source {
            Source::Seed => {
                let mut p = [0; LEVELS];
                p[AREA] = self.seed_area(w, info.driver);
                draw_from(p, ZONE, info.key.with(TAG_SEED), false)
            }
            Source::From { unit, keep, level } => {
                let Some(src) = self.pos(w, unit, info.start) else {
                    panic!(
                        "source {unit:?} of {u:?} ({info:?}) does not exist; source info {:?}",
                        self.info(w, unit)
                    );
                };
                if keep || level as usize >= LEVELS {
                    src
                } else {
                    draw_from(src, level as usize, info.key.with(TAG_FORM), true)
                }
            }
            Source::Fresh { area } => {
                let mut p = [0; LEVELS];
                p[AREA] = area;
                draw_from(p, ZONE, info.key.with(TAG_SEED), false)
            }
            Source::Back { unit, level } => {
                let (_, first) = self
                    .initial(w, unit)
                    .expect("a separated union existed");
                let level = (level as usize).min(LEVELS - 1);
                draw_from(first, level, info.key.with(TAG_FORM), true)
            }
        };
        self.memo.insert(u, (info, pos));
        Some((info, pos))
    }

    /// The local-move tree of `u` at level `k` (1 county, 2 cluster, 3 tract).
    fn local_tree(&self, w: &World, info: &Info, k: usize) -> PoissonTree {
        let share = self.rules.local.levels.as_array()[k - 1]
            * frailty(
                info.key.with(TAG_FRAILTY),
                self.rules.local.frailty_variance,
            );
        let by = (w.birth_year(info.driver) - self.first_birth_year) as usize;
        let cum = Arc::clone(&self.local_cum);
        let (row, years, y0) = (by * (self.years + 1), self.years, self.y0);
        let f = move |t: i64| -> f64 {
            let y = year_of(t);
            let yi = (y - y0).clamp(0, years as i32) as usize;
            let a = cum[row + yi];
            if yi >= years {
                return a;
            }
            let (s, e) = (year_start(y), year_start(y + 1));
            let frac = ((t - s) as f64 / (e - s) as f64).clamp(0.0, 1.0);
            a + frac * (cum[row + yi + 1] - a)
        };
        PoissonTree::new(
            info.key.with2(TAG_LOCAL, k as u64),
            info.start,
            LOCAL_LEAF,
            LOCAL_LEVELS,
            move |t0, t1| share * (f(t1) - f(t0)),
        )
    }

    /// Where unit `u` is at `t` (events strictly before `t` count), or
    /// `None` if `u` never exists. `t` is clamped to `u`'s span.
    pub fn pos(&self, w: &World, u: Unit, t: i64) -> Option<Pos> {
        let (info, init) = self.initial(w, u)?;
        let t = t.clamp(info.start, info.end);
        let trees: Vec<PoissonTree> = (1..LEVELS).map(|k| self.local_tree(w, &info, k)).collect();
        let moves = if self.areas_on {
            Some(self.ledger_moves(w, u, &info))
        } else {
            None
        };
        let dest_at = |te: i64| -> u32 {
            let m = moves.as_ref().expect("area mode");
            m[m.partition_point(|e| e.0 < te)].1
        };
        let mut out = Vec::with_capacity(LEVELS);
        // Moves at each level draw around the level's anchor: its node at
        // the last coarser regeneration (where the unit formed, or where its
        // last long move landed), so families stay near where they settled.
        nested_regen_anchored(
            LEVELS,
            t,
            |k, t| {
                if k == AREA {
                    if let Some(m) = &moves {
                        let i = m.partition_point(|e| e.0 < t);
                        return (i > 0).then(|| Event {
                            t: m[i - 1].0,
                            key: Self::area_move_key(&info, m[i - 1].0),
                        });
                    }
                    self.last_long(w, info.driver, info.start, t)
                        .map(|(lt, year, _)| Event {
                            t: lt,
                            key: self.long_key(info.driver, year),
                        })
                } else {
                    trees[k - 1].last_before(t).filter(|e| e.t > info.start)
                }
            },
            |k, _| init[k],
            |k, regen: Regen, key, parent: Option<&u32>| {
                if k == AREA && moves.is_some() {
                    return dest_at(regen.event.t);
                }
                self.draw_level(
                    w,
                    info.driver,
                    k,
                    year_of(regen.event.t),
                    key,
                    parent.copied(),
                    None,
                )
            },
            |k, regen: Regen, key, parent: Option<&u32>, anchor: &u32| {
                if k == AREA && moves.is_some() {
                    return dest_at(regen.event.t);
                }
                self.draw_level(
                    w,
                    info.driver,
                    k,
                    year_of(regen.event.t),
                    key,
                    parent.copied(),
                    Some(*anchor),
                )
            },
            &mut out,
        );
        Some(std::array::from_fn(|k| out[k]))
    }

    /// The key of `x`'s long move of `year`.
    fn long_key(&self, x: PersonId, year: i32) -> Key {
        self.key.with3(TAG_LONG_EVENT, x as u64, year as i64 as u64)
    }

    /// The draw at level `k` in `year`: the long move's destination at the
    /// area level; below, around `anchor` by gravity for a level's own move
    /// (where the level has gravity), else by attractiveness under the
    /// parent.
    #[allow(clippy::too_many_arguments)]
    fn draw_level(
        &self,
        w: &World,
        driver: PersonId,
        k: usize,
        year: i32,
        key: Key,
        parent: Option<u32>,
        anchor: Option<u32>,
    ) -> u32 {
        if k == AREA {
            let rank = self
                .long_rank(w, driver, year)
                .expect("a long move has a rank");
            return self.long_dest(w, driver, year, rank);
        }
        let parent = parent.expect("levels below the area have a parent");
        match (&self.gravity[k], anchor) {
            (Some(g), Some(a)) => g.draw(&self.places, parent, a, year, key),
            _ => self.places.draw_child(k, parent, year, key),
        }
    }

    /// `u`'s positions over its span, replayed from its events, as `[from,
    /// to)` stretches. A move at `τ` counts from `τ + 1`, matching
    /// [`Self::pos`] (events strictly before `t`); at equal times the
    /// coarser move comes first, as in `nested_regen`.
    pub fn timeline(&self, w: &World, u: Unit) -> Option<(Info, Vec<Stretch>)> {
        let (info, init) = self.initial(w, u)?;
        let mut events: Vec<(i64, usize, Key)> = Vec::new();
        let moves = if self.areas_on {
            let m = self.ledger_moves(w, u, &info);
            for &(t, _) in m.iter() {
                events.push((t, AREA, Self::area_move_key(&info, t)));
            }
            Some(m)
        } else {
            let mut longs = Vec::new();
            self.long_moves(w, info.driver, info.start, info.end, &mut longs);
            for (t, year, _) in longs {
                events.push((t, AREA, self.long_key(info.driver, year)));
            }
            None
        };
        let mut ev = Vec::new();
        for k in 1..LEVELS {
            ev.clear();
            self.local_tree(w, &info, k)
                .events(info.start + 1, info.end, &mut ev);
            events.extend(ev.iter().map(|e| (e.t, k, e.key)));
        }
        events.sort_by_key(|e| (e.0, e.1));
        let mut pos = init;
        let mut anchor = init;
        let mut from = info.start;
        let mut out = Vec::with_capacity(events.len() + 1);
        for (t, k, key) in events {
            if t + 1 > from {
                out.push((from, t + 1, pos));
            }
            let year = year_of(t);
            for j in k..LEVELS {
                let parent = (j > AREA).then(|| pos[j - 1]);
                let kk = key.with(j as u64);
                let drawn = match &moves {
                    Some(m) if j == AREA => m[m.partition_point(|e| e.0 < t)].1,
                    _ if j == k => {
                        self.draw_level(w, info.driver, j, year, kk, parent, Some(anchor[j]))
                    }
                    _ => self.draw_level(w, info.driver, j, year, kk, parent, None),
                };
                pos[j] = drawn;
                if j != k {
                    anchor[j] = pos[j];
                }
            }
            from = t + 1;
        }
        if info.end > from {
            out.push((from, info.end, pos));
        }
        Some((info, out))
    }

    /// The area of `u` just before `t` from its first area and long moves.
    fn area_at(first: u32, moves: &[(i64, u32)], t: i64) -> u32 {
        let i = moves.partition_point(|m| m.0 < t);
        if i == 0 {
            first
        } else {
            moves[i - 1].1
        }
    }

    /// `u`'s first area and its long moves `(time, destination)`.
    fn area_track(&self, w: &World, u: Unit) -> Option<AreaTrack> {
        let (info, init) = self.initial(w, u)?;
        if self.areas_on {
            let track = self.ledger_moves(w, u, &info).to_vec();
            return Some((info, init[AREA], track));
        }
        let mut moves = Vec::new();
        self.long_moves(w, info.driver, info.start, info.end, &mut moves);
        let track = moves
            .iter()
            .map(|&(t, year, rank)| (t, self.long_dest(w, info.driver, year, rank)))
            .collect();
        Some((info, init[AREA], track))
    }

    // --- addresses -----------------------------------------------------------------------

    /// The unit whose position household `h` lives at, and the time to read
    /// it at (the epoch start for a roommate group).
    pub fn anchor(&self, w: &World, h: Household, t: i64) -> (Unit, i64) {
        match h {
            Household::Union { a, b, start } => (Unit::Union { a, b, start }, t),
            Household::Solo { person, spell } => (
                Unit::Spell {
                    x: person,
                    s: spell,
                },
                t,
            ),
            Household::Roommates { .. } => {
                let (l, t0) = w
                    .roommate_lease(h)
                    .expect("a roommate group has a lease holder");
                (
                    Unit::Spell {
                        x: l,
                        s: w.unions_ended(l, t0),
                    },
                    t0,
                )
            }
        }
    }

    /// Where household `h` lives at `t`.
    pub fn address(&self, w: &World, h: Household, t: i64) -> Pos {
        let (u, at) = self.anchor(w, h, t);
        self.pos(w, u, at).expect("a household's anchor exists")
    }

    /// Where `x` lives at `t`, if present.
    pub fn address_of(&self, w: &World, x: PersonId, t: i64) -> Option<Pos> {
        w.household(x, t).map(|h| self.address(w, h, t))
    }

    // --- rosters -------------------------------------------------------------------------

    /// Units that enter area `area` statically: seeds placed there and
    /// long movers arriving there.
    fn entries(&self, w: &World, area: u32) -> Vec<Unit> {
        let mut out = Vec::new();
        let ledger = w.ledger();
        let person = |block: u32, raw: u64| (ledger.base[block as usize] + raw) as PersonId;
        let seed_flows = self
            .founder_flows
            .iter()
            .chain(self.arrival_flows.iter())
            .flatten();
        for flow in seed_flows {
            for off in 0..flow.seg.segment_len(area as usize) {
                let i = flow.seg.member(area as usize, off);
                let ((block, _, first), within) = flow.locate(i);
                let x = person(block, first + within);
                let at = self.entry(w, x);
                if let Some(u) = self.driven_unit(w, x, at) {
                    if self.info(w, u).is_some_and(|i| i.source == Source::Seed) {
                        out.push(u);
                    }
                }
            }
        }
        if self.areas_on {
            self.ledger_entries(w, area, &mut out);
        }
        for (fi, flow) in self.long_flows.iter().enumerate() {
            let Some(flow) = flow else { continue };
            let year = self.y0 + (fi % self.years) as i32;
            for off in 0..flow.seg.segment_len(area as usize) {
                let i = flow.seg.member(area as usize, off);
                let ((block, _, _), rank) = flow.locate(i);
                let size = ledger.blocks[block as usize].size;
                let perm = CompactPerm::new(
                    size,
                    self.key
                        .with3(TAG_LONG_PERM, block as u64, year as i64 as u64),
                );
                let x = person(block, perm.inv(rank));
                let t = self.long_time(x, year);
                if let Some(u) = self.driven_unit(w, x, t) {
                    if self.info(w, u).is_some_and(|i| i.start < t && t < i.end) {
                        out.push(u);
                    }
                }
            }
        }
        out
    }

    /// Area mode: entries of `area` from the ledger (spec §9), a superset of
    /// the units that enter it other than through a source there (units
    /// never in it drop out of the history):
    /// - the first single spell of everyone of the area's blocks (natives
    ///   and immigrants) and of every migrant into it (fresh starts
    ///   included);
    /// - every union formed in it (fresh formations included);
    /// - every couple moving into it: the union, or the widowed partner's
    ///   spell that makes the move;
    /// - widowed partners of unions formed in it, from the union's planned
    ///   separation (the ledger counts them back here then).
    fn ledger_entries(&self, w: &World, area: u32, out: &mut Vec<Unit>) {
        let ledger = w.ledger();
        let region = self.places.region(area);
        let mut people: Vec<PersonId> = Vec::new();
        for (bi, block) in ledger.blocks.iter().enumerate() {
            let mut raw = 0u64;
            for co in &block.cohorts {
                // Everyone of the area's blocks (natives and immigrants),
                // and migrants into it.
                let ours = block.region == region
                    || co.class.is_some_and(|(dest, _)| dest == region);
                if ours {
                    let base = ledger.base[bi] + raw;
                    people.extend((0..co.size).map(|i| (base + i) as PersonId));
                }
                raw += co.size;
            }
        }
        let spells: Vec<Unit> = people
            .par_iter()
            .map(|&x| Unit::Spell { x, s: 0 })
            .filter(|&u| self.info(w, u).is_some())
            .collect();
        out.extend(spells);
        let formed = w.unions_formed_in(region);
        for &(x, u) in &formed {
            out.push(Unit::Union {
                a: x.min(u.partner),
                b: x.max(u.partner),
                start: u.start,
            });
        }
        // A widowed partner is counted back where the union formed from its
        // planned separation on: that unit returns here then.
        let returns: Vec<Unit> = formed
            .par_iter()
            .filter(|(_, u)| u.separation.is_some_and(|sep| u.end < sep))
            .flat_map_iter(|&(x, u)| {
                let sep = u.separation.expect("a planned separation");
                [x, u.partner]
                    .into_iter()
                    .filter(move |&p| w.death(p) > sep)
                    .filter_map(move |p| self.driven_unit(w, p, sep))
            })
            .collect();
        out.extend(returns);
        // Either partner can make the move (a widowed one alone).
        let movers: Vec<Unit> = w
            .couple_movers_into(region)
            .par_iter()
            .flat_map_iter(|&(x, start)| {
                let partner = w
                    .unions(x)
                    .iter()
                    .flatten()
                    .find(|v| v.start == start)
                    .map(|v| v.partner);
                [Some(x), partner].into_iter().flatten().filter_map(move |p| {
                    let k = w
                        .unions(p)
                        .iter()
                        .position(|v| v.is_some_and(|v| v.start == start))?;
                    let (_, t) = w.couple_moves(p)[k]?;
                    self.driven_unit(w, p, t)
                })
            })
            .collect();
        out.extend(movers);
    }

    /// People whose dependent chain can end at `q` while a unit of `q`'s
    /// lasts (`[from, until)`). A chain steps from a dependent to a
    /// custodial parent, a grandparent or a sibling guardian, so it reaches
    /// `q` from `q`'s children, from deeper descendants only through people
    /// who were themselves still dependents then, from minor siblings in
    /// `q`'s care, and from their children while they are minors.
    ///
    /// The descent through a child `c` is pruned exactly: a grandchild `g`
    /// leaves at 16 or later (the youngest union age; leaving home alone
    /// starts at 18), and `c` was at least 15 at `g`'s birth, so a chain
    /// `g → c → …` needs `c` still dependent at 31, or `c` dead while the
    /// unit lasts (`g` orphaned, in a grandparent's care).
    fn dependents_of(
        &self,
        w: &World,
        q: PersonId,
        from: i64,
        until: i64,
        out: &mut Vec<PersonId>,
    ) {
        const LATE_DEPENDENT: i64 = 31 * YEAR;
        let mut stack = vec![q];
        while let Some(p) = stack.pop() {
            for &c in w.children(p).iter() {
                let born = w.birth(c);
                if born >= until {
                    continue;
                }
                out.push(c);
                let late = born + LATE_DEPENDENT;
                let dependent_late = self.facts(w, c).spell0.map_or(true, |s| s > late);
                if dependent_late || w.death(c) < until {
                    stack.push(c);
                }
            }
        }
        for &s in w.siblings(q).iter() {
            let minor_until = w.birth(s) + 18 * YEAR;
            if w.birth(s) < until && minor_until > from {
                out.push(s);
                for &c in w.children(s).iter() {
                    if w.birth(c) < until.min(minor_until) {
                        out.push(c);
                    }
                }
            }
        }
    }

    /// The units whose source is `v` and that start while `v` is in `area`.
    fn successors(&self, w: &World, v: Unit, area: u32, out: &mut Vec<Unit>) {
        let Some((info, first, moves)) = self.area_track(w, v) else {
            return;
        };
        let (lo, hi) = (info.start, info.end);
        let people: Vec<PersonId> = match v {
            Unit::Union { a, b, .. } => vec![a, b],
            Unit::Spell { x, .. } => vec![x],
        };
        let mut cands: Vec<PersonId> = people.clone();
        for &q in &people {
            self.dependents_of(w, q, lo, hi, &mut cands);
        }
        cands.sort_unstable();
        cands.dedup();
        // A successor starts within `v`'s span (its source is read just
        // before, or at, its start), which the memoized facts check cheaply.
        let within = |t: i64| lo <= t && t <= hi;
        let mut units = Vec::new();
        for &c in &cands {
            if w.birth(c) >= hi {
                continue;
            }
            let fc = self.facts(w, c);
            if fc.spell0.is_some_and(within) {
                units.push(Unit::Spell { x: c, s: 0 });
            }
            for (i, un) in fc.unions.iter().enumerate() {
                if within(un.start) {
                    units.push(Unit::Union {
                        a: c.min(un.partner),
                        b: c.max(un.partner),
                        start: un.start,
                    });
                }
                if people.contains(&c) && within(un.end) {
                    units.push(Unit::Spell {
                        x: c,
                        s: i as u8 + 1,
                    });
                }
            }
        }
        for u in units {
            let Some(ui) = self.info(w, u) else { continue };
            match ui.source {
                Source::From { unit, .. } if unit == v => {
                    if Self::area_at(first, &moves, ui.start) == area {
                        out.push(u);
                    }
                }
                // A return to where the union formed.
                Source::Back { unit, .. } if unit == v && first == area => out.push(u),
                _ => {}
            }
        }
    }

    /// Every unit ever in `area`, with its stretches there, computed once
    /// and cached.
    pub fn history(&self, w: &World, area: u32) -> Arc<History> {
        if let Some(h) = self.histories.lock().unwrap().get(&area) {
            return Arc::clone(h);
        }
        // Layers expand in parallel; each layer's successors keep their
        // order, so the history is the same on any number of threads.
        let units = closure_layers(self.entries(w, area), |layer| {
            layer
                .par_iter()
                .map(|&v| {
                    let mut out = Vec::new();
                    self.successors(w, v, area, &mut out);
                    out
                })
                .collect::<Vec<_>>()
                .into_iter()
                .flatten()
                .collect()
        });
        let members: Vec<Member> = units
            .into_par_iter()
            .filter_map(|u| {
                let (info, line) = self.timeline(w, u)?;
                let stays: Vec<(i64, i64, Pos)> =
                    line.into_iter().filter(|s| s.2[AREA] == area).collect();
                (!stays.is_empty()).then_some(Member {
                    unit: u,
                    info,
                    stays,
                })
            })
            .collect();
        let mut index: Vec<(u32, i64, i64, u32)> = members
            .iter()
            .enumerate()
            .flat_map(|(i, m)| {
                m.stays
                    .iter()
                    .map(move |s| (s.2[TRACT], s.0, s.1, i as u32))
            })
            .collect();
        index.sort_unstable();
        let h = Arc::new(History {
            area,
            members,
            index,
        });
        self.histories.lock().unwrap().insert(area, Arc::clone(&h));
        // The per-unit and per-person caches served the closure; the history
        // keeps what rosters need, so they are dropped to bound memory.
        self.infos.clear();
        self.area_moves.clear();
        self.facts.clear();
        self.memo.clear();
        h
    }

    /// Every household living under place `node` (at level `level`) at
    /// `t`, sorted.
    pub fn roster(&self, w: &World, level: usize, node: u32, t: i64) -> Vec<Household> {
        let area = self.places.ancestor(level, node, AREA);
        let hist = self.history(w, area);
        let tracts = self.places.tracts_under(level, node);
        let lo = hist.index.partition_point(|e| e.0 < tracts.start);
        let hi = hist.index.partition_point(|e| e.0 < tracts.end);
        let span = w.roommate_epoch_span();
        // Only stretches that can hold a household at `t` need the (costly)
        // occupancy checks, which run in parallel.
        let mut out: Vec<Household> = hist.index[lo..hi]
            .iter()
            .filter(|e| e.1 <= t && e.2 > t - span)
            .collect::<Vec<_>>()
            .par_iter()
            .flat_map_iter(|&&(_, from, to, i)| {
                let m = &hist.members[i as usize];
                let mut found: [Option<Household>; 2] = [None, None];
                if from <= t && t < to {
                    let h = match m.unit {
                        Unit::Union { a, b, start } => Household::Union { a, b, start },
                        Unit::Spell { x, s } => Household::Solo {
                            person: x,
                            spell: s,
                        },
                    };
                    if w.household(m.info.driver, t) == Some(h) {
                        found[0] = Some(h);
                    }
                }
                // A roommate group lives where its lease holder's spell was
                // at the epoch start.
                if let Unit::Spell { x, .. } = m.unit {
                    let t0 = w.roommate_epoch_start(x, t);
                    if from <= t0 && t0 < to {
                        found[1] = w.lease_group(x, t).map(|(h, _)| h);
                    }
                }
                found.into_iter().flatten()
            })
            .collect();
        out.sort_unstable();
        out.dedup();
        out
    }
}
