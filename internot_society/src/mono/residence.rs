//! Residence (L4, design A, forward) on the monotone world: where every
//! household lives at `t`, as a pure function of `(seed, id, t)`. Spec
//! `docs/superpowers/specs/2026-10-01-residence.md` (§2); ported from the
//! ledger world's residence (archived in
//! `.scratch/archive/2026-10-03-old-worlds/`). Rates and shares: the pack's
//! `residence.ron`; places: `places.ron` and `data/places.bin`.
//!
//! - **Places** are a fixed tree: areas ⊃ commuting zones ⊃ counties (or
//!   1M parts) ⊃ clusters of neighbouring tracts ⊃ tracts. Areas, zones and
//!   counties weigh by decade (1840–2100); clusters and tracts by their
//!   static share of the county (the data scales 2020 tract shares by
//!   county populations), so the tree holds about 2 MB.
//! - **Units** have addresses: a first union, or a single adult's spell (0
//!   from leaving home to the union, 1 after it). Each has a position over
//!   its whole span; households live at their anchor unit's.
//! - **Position at `t`** is nested regeneration: a move at level k redraws
//!   levels ≥ k, so each level is the draw at its last regeneration
//!   (`stream::nested_regen_anchored`), with no replay. Moves within an area
//!   draw by gravity around the unit's anchor; long moves (to another area)
//!   are the driver's Poisson stream, landing in the lineage region of the
//!   area they leave with the pack's share, else anywhere, by weight.
//! - **A new unit starts near its source** (leaving home: the household
//!   left; a union: one partner's household; after a separation: the
//!   union's home, kept or left nearby). Units with no source (founders',
//!   who have no parents in the world) are seeded by weight when they
//!   start.
//!
//! Forward only for now: rosters (who lives in a place) need areas as
//! cells of the kinship world, so partners meet where they live (the
//! design-A finding, spec §7–§8). Until then partners come from anywhere in
//! the country and one of them moves at the union.

use std::collections::HashMap;
use std::sync::Mutex;

use procedural_core::curve::piecewise_power;
use procedural_core::geo::{haversine_miles, LatLon};
use procedural_core::key::Key;
use procedural_core::sample::frailty;
use procedural_core::stream::{nested_regen_anchored, year_of, year_start, Event, PoissonTree, Regen, DAY};

use super::{Household, Mono, Pid, Union};
use crate::params::{Params, Residence as Rules};

const T_SEED: u64 = 60;
const T_FORM: u64 = 61;
const T_SOURCE: u64 = 62;
const T_KEEPER: u64 = 63;
const T_SPELL: u64 = 64;
const T_UNION: u64 = 65;
const T_LOCAL: u64 = 66;
const T_LONG: u64 = 67;
const T_FRAILTY: u64 = 68;
const T_STAYS: u64 = 69;
const T_DEST: u64 = 70;
const T_ADDRESS: u64 = 71;

/// Levels of the place tree.
pub const LEVELS: usize = 5;
pub const AREA: usize = 0;
pub const ZONE: usize = 1;
pub const COUNTY: usize = 2;
pub const CLUSTER: usize = 3;
pub const TRACT: usize = 4;

/// Mean Gregorian year, in seconds.
const YEAR_S: i64 = 31_556_952;

/// Leaves of the move trees: 30 days, 2^11 of them (about 168 years,
/// longer than any unit).
const LEAF: i64 = 30 * DAY;
const TREE_LEVELS: u8 = 11;

/// A position: the node at each level, area first.
pub type Pos = [u32; LEVELS];

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
    /// Tract centres; every coarser node's population-weighted centre (in
    /// 2020).
    tract_at: Vec<[f32; 2]>,
    centre: [Vec<LatLon>; LEVELS - 1],
    first_decade: i32,
    decades: usize,
    /// Areas, zones and counties: weight by decade, `w[k][d · n_k + i]`,
    /// and (zones, counties) the cumulative among siblings.
    w: [Vec<f32>; 3],
    cum: [Vec<f32>; 3],
    /// Clusters (in their county) and tracts (in their cluster): the
    /// cumulative static share among siblings, and the share itself.
    scum: [Vec<f32>; 2],
    share: [Vec<f32>; 2],
    area_name: Strings,
    zone_name: Strings,
    county_name: Strings,
    county_fips: Vec<u32>,
    tract_geoid: Vec<u64>,
    /// Postal places (GeoNames place names), ZIP codes (code, state FIPS,
    /// place), and each tract's ZIPs by land area: `tract_zip_at[t]..
    /// tract_zip_at[t + 1]` in `tract_zips` as (ZIP, cumulative share of
    /// 65535).
    city: Strings,
    zips: Vec<(u32, u8, u32)>,
    tract_zip_at: Vec<u32>,
    tract_zips: Vec<(u32, u16)>,
    /// Street names and their cumulative TIGER feature counts.
    streets: Strings,
    street_cum: Vec<u64>,
}

/// Many short strings in one buffer (one allocation instead of one each).
#[derive(Default)]
pub(super) struct Strings {
    buf: String,
    end: Vec<u32>,
}

impl Strings {
    pub(super) fn push(&mut self, s: &str) {
        self.buf.push_str(s);
        self.end.push(self.buf.len() as u32);
    }

    pub(super) fn get(&self, i: usize) -> &str {
        let start = if i == 0 { 0 } else { self.end[i - 1] as usize };
        &self.buf[start..self.end[i] as usize]
    }

    pub(super) fn len(&self) -> usize {
        self.end.len()
    }

    pub(super) fn heap_bytes(&self) -> usize {
        self.buf.capacity() + self.end.capacity() * 4
    }
}

impl FromIterator<String> for Strings {
    fn from_iter<I: IntoIterator<Item = String>>(it: I) -> Self {
        let mut s = Strings::default();
        for x in it {
            s.push(&x);
        }
        s.buf.shrink_to_fit();
        s.end.shrink_to_fit();
        s
    }
}

/// State FIPS code to postal abbreviation.
pub fn state_abbr(fips: u32) -> &'static str {
    const S: [(u32, &str); 51] = [
        (1, "AL"), (2, "AK"), (4, "AZ"), (5, "AR"), (6, "CA"), (8, "CO"), (9, "CT"), (10, "DE"), (11, "DC"), (12, "FL"), (13, "GA"), (15, "HI"), (16, "ID"), (17, "IL"), (18, "IN"), (19, "IA"), (20, "KS"),
        (21, "KY"), (22, "LA"), (23, "ME"), (24, "MD"), (25, "MA"), (26, "MI"), (27, "MN"), (28, "MS"), (29, "MO"), (30, "MT"), (31, "NE"), (32, "NV"), (33, "NH"), (34, "NJ"), (35, "NM"), (36, "NY"), (37, "NC"),
        (38, "ND"), (39, "OH"), (40, "OK"), (41, "OR"), (42, "PA"), (44, "RI"), (45, "SC"), (46, "SD"), (47, "TN"), (48, "TX"), (49, "UT"), (50, "VT"), (51, "VA"), (53, "WA"), (54, "WV"), (55, "WI"), (56, "WY"),
    ];
    S.iter().find(|s| s.0 == fips).map_or("", |s| s.1)
}

struct Reader<'a> {
    b: &'a [u8],
    i: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], String> {
        let s = self.b.get(self.i..self.i + n).ok_or("places data is truncated")?;
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
            if shift > 63 {
                return Err("places data: bad varint".into());
            }
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
        if i < len { Ok(i as u32) } else { Err(format!("places data: {what} index {i} out of range")) }
    }
}

impl Places {
    /// Parses `places.bin` (format in `internot_society/data/distill_places.py`),
    /// each area in the lineage region of its most populous county.
    pub fn from_data(b: &[u8], region_of_state: impl Fn(u8) -> u16) -> Result<Self, String> {
        let mut r = Reader { b, i: 0 };
        if r.take(8)? != b"INTPLACE" {
            return Err("not a places file".into());
        }
        let version = r.take(1)?[0];
        if !(2..=3).contains(&version) {
            return Err(format!("places file version {version}, expected 2 or 3"));
        }
        let head = r.take(3)?;
        let first_decade = u16::from_le_bytes([head[0], head[1]]) as i32;
        let decades = (head[2] as usize).max(1);
        let na = r.leb()? as usize;
        let area_name: Vec<String> = (0..na).map(|_| r.text()).collect::<Result<_, _>>()?;
        let nz = r.leb()? as usize;
        let (mut zone_area, mut zone_name) = (Vec::with_capacity(nz), Vec::with_capacity(nz));
        for _ in 0..nz {
            let _code = r.leb()?;
            zone_area.push(r.index(na, "area")?);
            zone_name.push(r.text()?);
        }
        let nc = r.leb()? as usize;
        let (mut county_fips, mut county_zone, mut county_name) = (Vec::new(), Vec::new(), Vec::new());
        for _ in 0..nc {
            county_fips.push(r.leb()? as u32);
            county_zone.push(r.index(nz, "zone")?);
            county_name.push(r.text()?);
        }
        let nl = r.leb()? as usize;
        let cluster_county: Vec<u32> = (0..nl).map(|_| r.index(nc, "county")).collect::<Result<_, _>>()?;
        let nt = r.leb()? as usize;
        let d2020 = (((2020 - first_decade) / 10).max(0) as usize).min(decades - 1);
        let mut tract_cluster = Vec::with_capacity(nt);
        let mut tract_at = Vec::with_capacity(nt);
        let mut tract_geoid = Vec::with_capacity(nt);
        let mut tract_w2020 = Vec::with_capacity(nt);
        let mut county_w = vec![0.0f64; nc * decades];
        for _ in 0..nt {
            let geoid = r.leb()?;
            let (lat, lon) = (r.f32()?, r.f32()?);
            let cl = r.index(nl, "cluster")?;
            let county = cluster_county[cl as usize] as usize;
            let mut w2020 = 0.0;
            for d in 0..decades {
                let w = r.leb()? as f64;
                county_w[d * nc + county] += w;
                if d == d2020 {
                    w2020 = w;
                }
            }
            tract_cluster.push(cl);
            tract_at.push([lat, lon]);
            tract_geoid.push(geoid);
            tract_w2020.push(w2020);
        }
        // Postal places and ZIPs (version 3).
        let (mut city, mut zips, mut tract_zip_at, mut tract_zips) = (Strings::default(), Vec::new(), vec![0u32], Vec::new());
        if version >= 3 {
            let nm = r.leb()? as usize;
            city = (0..nm).map(|_| r.text()).collect::<Result<Strings, _>>()?;
            let nzip = r.leb()? as usize;
            for _ in 0..nzip {
                let code = r.leb()? as u32;
                let st = r.take(1)?[0];
                zips.push((code, st, r.index(nm, "postal place")?));
            }
            for _ in 0..nt {
                let k = r.take(1)?[0] as usize;
                let mut acc = 0u32;
                for _ in 0..k {
                    let z = r.index(nzip, "zip")?;
                    let sh = r.take(2)?;
                    acc += u16::from_le_bytes([sh[0], sh[1]]) as u32;
                    tract_zips.push((z, acc.min(65535) as u16));
                }
                tract_zip_at.push(tract_zips.len() as u32);
            }
        } else {
            tract_zip_at = vec![0; nt + 1];
        }
        let (mut streets, mut street_cum) = (Strings::default(), Vec::new());
        if version >= 3 {
            let ns = r.leb()? as usize;
            let mut acc = 0u64;
            for _ in 0..ns {
                streets.push(&r.text()?);
                acc += r.leb()?;
                street_cum.push(acc);
            }
        }
        // Tree order: every level's children contiguous and in parent order.
        let parent: [Vec<u32>; LEVELS] = [Vec::new(), zone_area.clone(), county_zone.clone(), cluster_county.clone(), tract_cluster.clone()];
        let counts = [na, nz, nc, nl, nt];
        let mut first: [Vec<u32>; LEVELS - 1] = Default::default();
        for k in 0..LEVELS - 1 {
            let mut f = vec![0u32; counts[k] + 1];
            for &p in &parent[k + 1] {
                f[p as usize + 1] += 1;
            }
            for i in 0..counts[k] {
                f[i + 1] += f[i];
            }
            for (c, &p) in parent[k + 1].iter().enumerate() {
                if !(f[p as usize]..f[p as usize + 1]).contains(&(c as u32)) {
                    return Err("places data: not in tree order".into());
                }
            }
            first[k] = f;
        }
        // Decade weights up to counties.
        let mut zone_w = vec![0.0f64; nz * decades];
        let mut area_w = vec![0.0f64; na * decades];
        for d in 0..decades {
            for c in 0..nc {
                zone_w[d * nz + county_zone[c] as usize] += county_w[d * nc + c];
            }
            for z in 0..nz {
                area_w[d * na + zone_area[z] as usize] += zone_w[d * nz + z];
            }
        }
        let wk = [area_w, zone_w, county_w];
        let mut cum: [Vec<f32>; 3] = Default::default();
        for k in 1..3 {
            let n = counts[k];
            let mut c = vec![0.0f32; n * decades];
            for d in 0..decades {
                for pnode in 0..counts[k - 1] {
                    let mut acc = 0.0f64;
                    for i in first[k - 1][pnode] as usize..first[k - 1][pnode + 1] as usize {
                        acc += wk[k][d * n + i];
                        c[d * n + i] = acc as f32;
                    }
                }
            }
            cum[k] = c;
        }
        // Static shares below counties, from 2020.
        let mut cluster_w = vec![0.0f64; nl];
        for (t, &cl) in tract_cluster.iter().enumerate() {
            cluster_w[cl as usize] += tract_w2020[t];
        }
        let static_cum = |w: &[f64], k: usize| -> (Vec<f32>, Vec<f32>) {
            let (mut c, mut s) = (vec![0.0f32; w.len()], vec![0.0f32; w.len()]);
            for pnode in 0..counts[k - 1] {
                let r = first[k - 1][pnode] as usize..first[k - 1][pnode + 1] as usize;
                let total: f64 = r.clone().map(|i| w[i]).sum();
                let mut acc = 0.0;
                for i in r {
                    let sh = if total > 0.0 { w[i] / total } else { 0.0 };
                    acc += sh;
                    c[i] = acc as f32;
                    s[i] = sh as f32;
                }
            }
            (c, s)
        };
        let (cl_cum, cl_share) = static_cum(&cluster_w, CLUSTER);
        let (tr_cum, tr_share) = static_cum(&tract_w2020, TRACT);
        // Centres of coarser nodes, weighted by 2020 population.
        let mut centre: [Vec<LatLon>; LEVELS - 1] = Default::default();
        for k in 0..LEVELS - 1 {
            let mut acc = vec![(0.0f64, 0.0f64, 0.0f64); counts[k]];
            for t in 0..nt {
                let mut node = tract_cluster[t];
                for level in (k + 1..CLUSTER + 1).rev() {
                    node = parent[level][node as usize];
                }
                let node = if k == CLUSTER { tract_cluster[t] } else { node };
                let a = &mut acc[node as usize];
                let w = tract_w2020[t].max(1.0);
                a.0 += w * tract_at[t][0] as f64;
                a.1 += w * tract_at[t][1] as f64;
                a.2 += w;
            }
            centre[k] = acc.iter().map(|a| LatLon::new(a.0 / a.2.max(1e-9), a.1 / a.2.max(1e-9))).collect();
        }
        // Each area takes the region of its most populous county.
        let mut best = vec![(f64::MIN, 0u32); na];
        for (c, &z) in county_zone.iter().enumerate() {
            let a = zone_area[z as usize] as usize;
            if wk[2][d2020 * nc + c] > best[a].0 {
                best[a] = (wk[2][d2020 * nc + c], county_fips[c]);
            }
        }
        let area_region = best.iter().map(|&(_, fips)| region_of_state((fips / 1000) as u8)).collect();
        Ok(Places {
            first,
            parent,
            area_region,
            tract_at,
            centre,
            first_decade,
            decades,
            w: wk.map(|v| v.into_iter().map(|x| x as f32).collect()),
            cum,
            scum: [cl_cum, tr_cum],
            share: [cl_share, tr_share],
            area_name: area_name.into_iter().collect(),
            zone_name: zone_name.into_iter().collect(),
            county_name: county_name.into_iter().collect(),
            county_fips,
            tract_geoid,
            city,
            zips,
            tract_zip_at,
            tract_zips,
            streets,
            street_cum,
        })
    }

    /// A street name drawn by how many streets carry it, with `u ∈ [0, 1)`.
    pub fn street(&self, u: f64) -> Option<&str> {
        let total = *self.street_cum.last()?;
        let v = (u * total as f64) as u64;
        Some(self.streets.get(self.street_cum.partition_point(|&c| c <= v).min(self.streets.len() - 1)))
    }

    /// A ZIP of tract `t` drawn by land area with `u ∈ [0, 1)`: (code, state
    /// FIPS, postal place), if the tract has one.
    pub fn zip_in(&self, t: u32, u: f64) -> Option<(u32, u8, &str)> {
        let r = self.tract_zip_at[t as usize] as usize..self.tract_zip_at[t as usize + 1] as usize;
        let row = &self.tract_zips[r];
        let v = (u * 65535.0) as u16;
        let &(z, _) = row.iter().find(|e| v < e.1).or(row.last())?;
        let (code, st, c) = self.zips[z as usize];
        Some((code, st, self.city.get(c as usize)))
    }

    /// Heap bytes.
    pub fn heap_bytes(&self) -> usize {
        let v4 = |v: &[Vec<u32>]| v.iter().map(|x| x.capacity() * 4).sum::<usize>();
        let f4 = |v: &[Vec<f32>]| v.iter().map(|x| x.capacity() * 4).sum::<usize>();
        v4(&self.first)
            + v4(&self.parent)
            + f4(&self.w)
            + f4(&self.cum)
            + f4(&self.scum)
            + f4(&self.share)
            + self.tract_at.capacity() * 8
            + self.centre.iter().map(|c| c.capacity() * 16).sum::<usize>()
            + self.tract_geoid.capacity() * 8
            + self.county_fips.capacity() * 4
            + self.city.heap_bytes()
            + self.zips.capacity() * 12
            + self.tract_zip_at.capacity() * 4
            + self.tract_zips.capacity() * 8
            + self.streets.heap_bytes()
            + self.street_cum.capacity() * 8
            + self.area_name.heap_bytes()
            + self.zone_name.heap_bytes()
            + self.county_name.heap_bytes()
    }

    /// Number of nodes at level `k`.
    pub fn count(&self, k: usize) -> usize {
        if k == AREA { self.area_region.len() } else { self.parent[k].len() }
    }

    /// The parent of node `i` at level `k ≥ 1`.
    pub fn parent(&self, k: usize, i: u32) -> u32 {
        self.parent[k][i as usize]
    }

    /// The children of node `i` at level `k < TRACT`.
    pub fn children(&self, k: usize, i: u32) -> std::ops::Range<u32> {
        self.first[k][i as usize]..self.first[k][i as usize + 1]
    }

    /// Lineage region of area `a`.
    pub fn region(&self, a: u32) -> u16 {
        self.area_region[a as usize]
    }

    /// Population-weighted centre of node `i` at level `k`.
    pub fn centre(&self, k: usize, i: u32) -> LatLon {
        if k == TRACT {
            let [lat, lon] = self.tract_at[i as usize];
            LatLon::new(lat as f64, lon as f64)
        } else {
            self.centre[k][i as usize]
        }
    }

    pub fn area_name(&self, a: u32) -> &str {
        self.area_name.get(a as usize)
    }

    pub fn zone_name(&self, z: u32) -> &str {
        self.zone_name.get(z as usize)
    }

    /// A county's (or county part's) name and FIPS code.
    pub fn county(&self, c: u32) -> (&str, u32) {
        (self.county_name.get(c as usize), self.county_fips[c as usize])
    }

    /// A tract's 2020 GEOID.
    pub fn tract_geoid(&self, t: u32) -> u64 {
        self.tract_geoid[t as usize]
    }

    fn decade(&self, year: i32) -> usize {
        ((year - self.first_decade).div_euclid(10)).clamp(0, self.decades as i32 - 1) as usize
    }

    /// Weight of node `i` at level `k ≤ COUNTY` in `year`; below counties,
    /// its static share of its parent.
    pub fn weight(&self, k: usize, i: u32, year: i32) -> f64 {
        if k <= COUNTY {
            self.w[k][self.decade(year) * self.count(k) + i as usize] as f64
        } else {
            self.share[k - CLUSTER][i as usize] as f64
        }
    }

    /// A child of node `parent` (level `k − 1`) drawn by weight in `year`;
    /// uniformly if every child weighs nothing.
    pub fn draw_child(&self, k: usize, parent: u32, year: i32, key: Key) -> u32 {
        let r = self.children(k - 1, parent);
        let slice = if k <= COUNTY {
            let base = self.decade(year) * self.count(k);
            &self.cum[k][base + r.start as usize..base + r.end as usize]
        } else {
            &self.scum[k - CLUSTER][r.start as usize..r.end as usize]
        };
        let total = *slice.last().expect("every node has children") as f64;
        if total <= 0.0 {
            return r.start + key.below((r.end - r.start) as u64) as u32;
        }
        let u = (key.unit() * total) as f32;
        r.start + slice.partition_point(|&c| c <= u).min(slice.len() - 1) as u32
    }

    /// An area drawn by weight in `year`, among region `region`'s areas
    /// (or all).
    pub fn draw_area(&self, year: i32, region: Option<u16>, key: Key) -> u32 {
        let na = self.count(AREA);
        let base = self.decade(year) * na;
        let ok = |a: usize| region.is_none_or(|r| self.area_region[a] == r);
        let total: f64 = (0..na).filter(|&a| ok(a)).map(|a| self.w[AREA][base + a] as f64).sum();
        if total <= 0.0 {
            return key.below(na as u64) as u32;
        }
        let mut u = key.unit() * total;
        for a in (0..na).filter(|&a| ok(a)) {
            u -= self.w[AREA][base + a] as f64;
            if u < 0.0 {
                return a as u32;
            }
        }
        (0..na).rfind(|&a| ok(a)).unwrap_or(0) as u32
    }
}

/// Destinations of moves at one level, by gravity around an anchor: for
/// each parent node, the kernel between its children (by the distance
/// between their centres), times the children's weight.
struct LevelGravity {
    level: usize,
    /// Per parent node: where its block starts in `kernel`.
    start: Vec<usize>,
    /// Per parent, `n × n` kernel values between its children (row: the
    /// anchor), row-major.
    kernel: Vec<f32>,
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
                    kernel.push(piecewise_power(miles, g.flat_miles, &g.segments, g.beyond) as f32);
                }
            }
        }
        Self { level, start, kernel }
    }

    /// A child of `parent` drawn around `anchor` in `year`: weight times the
    /// kernel from the anchor.
    fn draw(&self, places: &Places, parent: u32, anchor: u32, year: i32, key: Key) -> u32 {
        let kids = places.children(self.level - 1, parent);
        let n = (kids.end - kids.start) as usize;
        let row = self.start[parent as usize] + (anchor - kids.start) as usize * n;
        let mut cum = [0.0f64; 256];
        let mut heap = if n > cum.len() { vec![0.0; n] } else { Vec::new() };
        let c: &mut [f64] = if n > 256 { &mut heap } else { &mut cum[..n] };
        let mut total = 0.0;
        for (i, j) in kids.clone().enumerate() {
            total += places.weight(self.level, j, year) * self.kernel[row + i] as f64;
            c[i] = total;
        }
        if total <= 0.0 {
            return kids.start + key.below(n as u64) as u32;
        }
        let u = key.unit() * total;
        kids.start + c.partition_point(|&x| x <= u).min(n - 1) as u32
    }
}

// --- units --------------------------------------------------------------------------

/// Something with an address: a first union (named by its wife), or a
/// person's single spell (0 before their union, 1 after it).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Unit {
    Union(Pid),
    Spell(Pid, u8),
}

/// Where a unit's first position comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// Placed by weight in its first year (founders' units).
    Seed,
    /// Near `unit`'s position at the start: keeping its dwelling, or drawn
    /// afresh from `level` down.
    From { unit: Unit, keep: bool, level: u8 },
}

/// A dwelling: the unit that moved in and when. A unit keeping its
/// source's home keeps its dwelling, so an address is stable for a stay.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Dwelling {
    pub unit: Unit,
    pub since: i64,
}

/// A home at some time: where in the place tree, the dwelling, and its
/// mail address.
#[derive(Clone, Debug)]
pub struct Home {
    pub pos: Pos,
    pub dwelling: Dwelling,
    /// House number and street, postal place, state (postal code) and ZIP.
    pub street: String,
    pub city: String,
    pub state: &'static str,
    pub zip: Option<u32>,
}

impl Home {
    /// One line: "1204 Oak St, Springfield, IL 62704".
    pub fn line(&self) -> String {
        match self.zip {
            Some(z) => format!("{}, {}, {} {z:05}", self.street, self.city, self.state),
            None => format!("{}, {}, {}", self.street, self.city, self.state),
        }
    }
}

/// A unit's facts.
#[derive(Clone, Copy, Debug)]
pub struct Info {
    /// In-world span `[start, end)`.
    pub start: i64,
    pub end: i64,
    /// Who drives its moves: the single adult, or the couple's wife.
    pub driver: Pid,
    pub source: Source,
    key: Key,
}

/// A memo split over locked shards, bounded: a shard that reaches its share
/// of the cap is emptied (the values are pure, so dropping them only costs
/// recomputing).
struct Memo<K, V> {
    shards: Vec<Mutex<HashMap<K, V>>>,
    cap: usize,
}

impl<K: std::hash::Hash + Eq, V: Clone> Memo<K, V> {
    const SHARDS: usize = 64;

    fn new(cap: usize) -> Self {
        Self { shards: (0..Self::SHARDS).map(|_| Mutex::new(HashMap::new())).collect(), cap: (cap / Self::SHARDS).max(1) }
    }

    fn shard(&self, k: &K) -> &Mutex<HashMap<K, V>> {
        use std::hash::BuildHasher;
        let h = std::hash::BuildHasherDefault::<std::collections::hash_map::DefaultHasher>::default().hash_one(k);
        &self.shards[h as usize % Self::SHARDS]
    }

    fn get(&self, k: &K) -> Option<V> {
        self.shard(k).lock().unwrap().get(k).cloned()
    }

    fn insert(&self, k: K, v: V) {
        let mut shard = self.shard(&k).lock().unwrap();
        if shard.len() >= self.cap {
            *shard = HashMap::new();
        }
        shard.insert(k, v);
    }

    fn len(&self) -> usize {
        self.shards.iter().map(|s| s.lock().unwrap().len()).sum()
    }
}

/// Residence for one world: the place tree, gravity kernels, move-rate
/// tables and bounded memos. Built on first use.
pub(super) struct Residence {
    pub(super) places: Places,
    gravity: [Option<LevelGravity>; LEVELS],
    rules: Rules,
    y0: i32,
    /// Per (driver birth year − `first_birth_year`, year − y0): cumulative
    /// local and long move rates (per year), `years + 1` per row.
    local_cum: Vec<f64>,
    long_cum: Vec<f64>,
    first_birth_year: i32,
    years: usize,
    initial: Memo<Unit, (Info, Pos)>,
    spell0: Memo<Pid, Option<i64>>,
}

/// Units whose first position is remembered (pure values: dropping one
/// costs a recomputation).
const MEMO_CAP: usize = 1 << 15;

impl Residence {
    pub(super) fn build(p: &Params, b_min: i32) -> Self {
        let places = Places::from_data(&p.place_data, |state| p.region_of_state(state)).expect("the pack's places data is valid");
        let rules = p.residence.clone();
        let gravity: [Option<LevelGravity>; LEVELS] = std::array::from_fn(|k| match k {
            ZONE => Some(LevelGravity::new(&places, k, &rules.gravity)),
            COUNTY | CLUSTER => Some(LevelGravity::new(&places, k, &rules.local_gravity)),
            _ => None,
        });
        let (y0, y1) = (p.y0, p.y1);
        let years = (y1 - y0 + 1) as usize;
        let births = (y1 - b_min + 1) as usize;
        // Rates by the driver's age in whole years that year.
        let table = |rate: &dyn Fn(i32, i32) -> f64| -> Vec<f64> {
            let mut cum = vec![0.0f64; births * (years + 1)];
            for b in 0..births {
                let by = b_min + b as i32;
                let row = b * (years + 1);
                for yi in 0..years {
                    let y = y0 + yi as i32;
                    let age = y - by;
                    cum[row + yi + 1] = cum[row + yi] + if age < 0 { 0.0 } else { rate(age, y) };
                }
            }
            cum
        };
        let local_cum = table(&|age, y| rules.local.by_age.at(age) * rules.local.era.at(y));
        let long_cum = table(&|age, y| rules.long.by_age.at(age) * rules.long.era.at(y));
        Residence {
            places,
            gravity,
            rules,
            y0,
            local_cum,
            long_cum,
            first_birth_year: b_min,
            years,
            initial: Memo::new(MEMO_CAP),
            spell0: Memo::new(MEMO_CAP),
        }
    }

    /// Heap bytes by part: places, gravity kernels, rate tables, memos.
    pub(super) fn heap_parts(&self) -> [(&'static str, usize); 4] {
        [
            ("residence places", self.places.heap_bytes()),
            ("residence gravity kernels", self.gravity.iter().flatten().map(|g| g.kernel.capacity() * 4 + g.start.capacity() * 8).sum::<usize>()),
            ("residence move rates", (self.local_cum.capacity() + self.long_cum.capacity()) * 8),
            (
                "residence memos",
                self.initial.len() * (std::mem::size_of::<(Unit, (Info, Pos))>() + 16) + self.spell0.len() * (std::mem::size_of::<(Pid, Option<i64>)>() + 16),
            ),
        ]
    }


}

impl<'a> Mono<'a> {
    pub(super) fn residence(&self) -> &Residence {
        self.residence.get_or_init(|| Residence::build(self.p, self.b_min))
    }

    /// The place tree (built on first use).
    pub fn places(&self) -> &Places {
        &self.residence().places
    }

    /// When `x` enters the world: birth (founders live from birth too, with
    /// no parents in the world; their first units are seeded).
    fn entry(&self, x: Pid) -> i64 {
        self.birth(x)
    }

    /// The first time `x` is independent (`dependent_of` is `None`), at or
    /// after entry: checked at every time that can end dependence.
    pub fn spell0_start(&self, x: Pid) -> Option<i64> {
        let r = self.residence();
        if let Some(v) = r.spell0.get(&x) {
            return v;
        }
        let v = self.spell0_raw(x);
        r.spell0.insert(x, v);
        v
    }

    fn spell0_raw(&self, x: Pid) -> Option<i64> {
        let entry = self.entry(x);
        let death = self.death(x);
        let birth = self.birth(x);
        let adult = birth + 18 * YEAR_S;
        let union = self.union_of(x);
        let in_union = |t: i64| union.is_some_and(|u| u.start <= t && t < u.end);
        let parents: Vec<Pid> = self.parents(x).map_or(vec![], |(m, _, u)| std::iter::once(m).chain(u.map(|u| u.husband)).collect());
        // The usual end of dependence: the first time at or after leaving
        // home that `x` is an adult or in a union. Before it, `x` can only be
        // independent with no custodial parent, so if a parent is present
        // from entry through it, it is the answer (one check, not a scan).
        if let Some(l) = self.leave_time(x) {
            let usual = if l >= adult || in_union(l) {
                l
            } else {
                union.map(|u| u.start).filter(|&s| s >= l).map_or(adult, |s| s.min(adult))
            }
            .max(entry);
            let parent_through = parents.iter().any(|&p| self.present_at(p, entry) && self.death(p) > usual);
            if usual < death && parent_through && self.dependent_of(x, usual).is_none() {
                return Some(usual);
            }
        }
        let mut cands = vec![entry, adult];
        if let Some(l) = self.leave_time(x) {
            cands.push(l);
        }
        if let Some(u) = union {
            cands.push(u.start);
        }
        for &p in &parents {
            cands.push(self.death(p));
            if let Some((m, _, u)) = self.parents(p) {
                cands.push(self.death(m));
                if let Some(u) = u {
                    cands.push(self.death(u.husband));
                }
            }
        }
        for &s in self.siblings(x).iter() {
            cands.push(self.death(s));
        }
        cands.retain(|&t| t >= entry && t < death);
        cands.sort_unstable();
        cands.dedup();
        cands.into_iter().find(|&t| self.dependent_of(x, t).is_none())
    }

    /// The unit whose household `q` lives in at `t` (before kin hosting):
    /// `q`'s union, else `q`'s current spell.
    fn unit_of(&self, q: Pid, t: i64) -> Unit {
        match self.union_during(q, t) {
            Some(u) => Unit::Union(u.wife),
            None => Unit::Spell(q, self.union_of(q).is_some_and(|u| u.end <= t) as u8),
        }
    }

    /// A unit's facts, or `None` if it never exists (an empty spell).
    pub fn unit_info(&self, u: Unit) -> Option<Info> {
        self.unit_initial(u).map(|v| v.0)
    }

    fn compute_info(&self, u: Unit) -> Option<Info> {
        let rules = &self.residence().rules;
        match u {
            Unit::Union(w) => {
                let un: Union = self.union_of(w)?;
                let entry = self.entry(un.wife).max(self.entry(un.husband));
                let begin = un.start.max(entry);
                if begin >= un.end {
                    return None;
                }
                let key = self.pkey(w, T_UNION);
                // A union that began before its people entered the world
                // (founders' unions) is seeded.
                let source = if un.start <= entry {
                    Source::Seed
                } else {
                    let (p, q) = if key.with(T_SOURCE).unit() < rules.union_source_woman { (un.wife, un.husband) } else { (un.husband, un.wife) };
                    let before = un.start - 1;
                    let present = |z: Pid| self.entry(z) <= before && self.present_at(z, before);
                    match [p, q].into_iter().find(|&z| present(z)) {
                        Some(z) => Source::From {
                            unit: self.unit_of(self.chain_end(z, before), before),
                            keep: key.with(T_STAYS).unit() < rules.union_keep,
                            level: rules.formation.union.level(key.with(T_FORM).unit()),
                        },
                        None => Source::Seed,
                    }
                };
                Some(Info { start: begin, end: un.end, driver: un.wife, source, key })
            }
            Unit::Spell(x, s) => {
                let death = self.death(x);
                let union = self.union_of(x);
                let key = self.pkey(x, T_SPELL).with(s as u64);
                let (start, end, source) = if s == 0 {
                    let end = union.map_or(death, |u| u.start).min(death);
                    let b = self.spell0_start(x)?;
                    let level = rules.formation.leave_home.level(key.with(T_FORM).unit());
                    let before = b - 1;
                    let q = self.chain_end(x, before);
                    let source = if b > self.entry(x) && q != x {
                        Source::From { unit: self.unit_of(q, before), keep: false, level }
                    } else {
                        // Independent on entry: near the mother if she is
                        // here, else a seed.
                        match self.mother(x).filter(|&m| self.present_at(m, b)) {
                            Some(m) => Source::From { unit: self.unit_of(self.chain_end(m, b), b), keep: false, level },
                            None => Source::Seed,
                        }
                    };
                    (b, end, source)
                } else {
                    let ended = union?;
                    let begin = ended.end.max(self.entry(x));
                    let partner = ended.partner(x);
                    let source = if ended.end <= self.entry(x) {
                        Source::Seed
                    } else {
                        let partner_alive = self.death(partner) > ended.end;
                        // The woman, the man, or neither keeps the home (a
                        // widowed survivor always does).
                        let keep = !partner_alive || {
                            let r = self.pkey(ended.wife, T_KEEPER).unit();
                            let keeper = if r < rules.stays.woman {
                                Some(ended.wife)
                            } else if r < rules.stays.woman + rules.stays.man {
                                Some(ended.husband)
                            } else {
                                None
                            };
                            keeper == Some(x)
                        };
                        Source::From { unit: Unit::Union(ended.wife), keep, level: rules.formation.separation.level(key.with(T_FORM).unit()) }
                    };
                    (begin, death, source)
                };
                (start < end).then_some(Info { start, end, driver: x, source, key })
            }
        }
    }

    /// The unit's facts and first position, memoized.
    fn unit_initial(&self, u: Unit) -> Option<(Info, Pos)> {
        let r = self.residence();
        if let Some(v) = r.initial.get(&u) {
            return Some(v);
        }
        let info = self.compute_info(u)?;
        let year = year_of(info.start);
        let places = &r.places;
        // A new home redraws from level `from` down: the first level around
        // the source's node there (by gravity), the finer ones by weight.
        let draw_from = |mut pos: Pos, from: usize, key: Key, near: bool| {
            for k in from.max(1)..LEVELS {
                let kk = key.with(k as u64);
                pos[k] = match &r.gravity[k] {
                    Some(g) if near && k == from => g.draw(places, pos[k - 1], pos[k], year, kk),
                    _ => places.draw_child(k, pos[k - 1], year, kk),
                };
            }
            pos
        };
        let pos = match info.source {
            Source::Seed => {
                let mut p = [0; LEVELS];
                p[AREA] = places.draw_area(year, None, info.key.with(T_SEED));
                draw_from(p, ZONE, info.key.with(T_SEED), false)
            }
            Source::From { unit, keep, level } => {
                let src = self.unit_pos(unit, info.start).unwrap_or_else(|| panic!("source {unit:?} of {u:?} ({info:?}) does not exist"));
                if keep || level as usize >= LEVELS { src } else { draw_from(src, level as usize, info.key.with(T_FORM), true) }
            }
        };
        r.initial.insert(u, (info, pos));
        Some((info, pos))
    }

    /// The move tree of `u` at level `k` (0 long moves; 1–4 local moves to
    /// another zone, county, cluster or tract).
    fn move_tree(&self, info: &Info, k: usize) -> PoissonTree {
        let r = self.residence();
        let (table, share) = if k == AREA {
            (&r.long_cum, 1.0)
        } else {
            (&r.local_cum, r.rules.local.levels.as_array()[k - 1] * frailty(info.key.with(T_FRAILTY), r.rules.local.frailty_variance))
        };
        let by = info.driver.y;
        // The tree owns a copy of the driver's row (one per call).
        let row_at = (by - r.first_birth_year).max(0) as usize * (r.years + 1);
        let row: Vec<f64> = table[row_at..row_at + r.years + 1].to_vec();
        let (y0, years) = (r.y0, r.years);
        let f = move |t: i64| -> f64 {
            let y = year_of(t);
            let yi = (y - y0).clamp(0, years as i32) as usize;
            let a = row[yi];
            if yi >= years || y < y0 {
                return a;
            }
            let (s, e) = (year_start(y), year_start(y + 1));
            let frac = ((t - s) as f64 / (e - s) as f64).clamp(0.0, 1.0);
            a + frac * (row[yi + 1] - a)
        };
        let tag = if k == AREA { T_LONG } else { T_LOCAL };
        PoissonTree::new(info.key.with2(tag, k as u64), info.start, LEAF, TREE_LEVELS, move |t0, t1| share * (f(t1) - f(t0)))
    }

    /// Where unit `u` is at `t` (events strictly before `t` count), or
    /// `None` if `u` never exists. `t` is clamped to `u`'s span.
    pub fn unit_pos(&self, u: Unit, t: i64) -> Option<Pos> {
        self.unit_pos_since(u, t).map(|p| p.0)
    }

    /// The dwelling unit `u` lives in at `t` and its position.
    pub fn unit_dwelling(&self, u: Unit, t: i64) -> Option<(Pos, Dwelling)> {
        let (pos, moved) = self.unit_pos_since(u, t)?;
        if let Some(since) = moved {
            return Some((pos, Dwelling { unit: u, since }));
        }
        // Not moved since it formed: its first dwelling, the source's if
        // it kept that home.
        let (info, _) = self.unit_initial(u)?;
        let d = match info.source {
            Source::From { unit, keep, level } if keep || level as usize >= LEVELS => self.unit_dwelling(unit, info.start).map_or(Dwelling { unit: u, since: info.start }, |s| s.1),
            _ => Dwelling { unit: u, since: info.start },
        };
        Some((pos, d))
    }

    /// [`Self::unit_pos`], and the time of the unit's last move before `t`
    /// (`None`: none since it formed).
    fn unit_pos_since(&self, u: Unit, t: i64) -> Option<(Pos, Option<i64>)> {
        let (info, init) = self.unit_initial(u)?;
        let t = t.clamp(info.start, info.end);
        let r = self.residence();
        let trees: Vec<PoissonTree> = (0..LEVELS).map(|k| self.move_tree(&info, k)).collect();
        let last: Vec<Option<Event>> = (0..LEVELS).map(|k| trees[k].last_before(t).filter(|e: &Event| e.t > info.start)).collect();
        let moved = last.iter().flatten().map(|e| e.t).max();
        let mut out = Vec::with_capacity(LEVELS);
        // Moves at each level draw around the level's anchor: its node at the
        // last coarser regeneration (where the unit formed, or where its last
        // long move landed), so families stay near where they settled.
        nested_regen_anchored(
            LEVELS,
            t,
            |k, _| last[k],
            |k, _| init[k],
            |k, regen: Regen, key, parent: Option<&u32>| self.draw_level(r, k, year_of(regen.event.t), key, parent.copied(), None),
            |k, regen: Regen, key, parent: Option<&u32>, anchor: &u32| self.draw_level(r, k, year_of(regen.event.t), key, parent.copied(), Some(*anchor)),
            &mut out,
        );
        Some((std::array::from_fn(|k| out[k]), moved))
    }

    /// The draw at level `k` in `year`: a long move's destination at the
    /// area level (the region of the area it leaves with the pack's share,
    /// else anywhere, by weight); below, around `anchor` by gravity for a
    /// level's own move, else by weight under the parent.
    #[allow(clippy::too_many_arguments)]
    fn draw_level(&self, r: &Residence, k: usize, year: i32, key: Key, parent: Option<u32>, anchor: Option<u32>) -> u32 {
        if k == AREA {
            let from = anchor.expect("an area move has an anchor");
            let region = (key.with(T_DEST).unit() < r.rules.long.own_region).then(|| r.places.region(from));
            return r.places.draw_area(year, region, key);
        }
        let parent = parent.expect("levels below the area have a parent");
        match (&r.gravity[k], anchor) {
            (Some(g), Some(a)) => g.draw(&r.places, parent, a, year, key),
            _ => r.places.draw_child(k, parent, year, key),
        }
    }

    // --- addresses -----------------------------------------------------------------------

    /// The unit household `h` lives at.
    pub fn anchor_unit(&self, h: Household) -> Unit {
        match h {
            Household::Union { wife, .. } => Unit::Union(wife),
            Household::Solo { person, spell } => Unit::Spell(person, spell),
        }
    }

    /// Where household `h` lives at `t`.
    pub fn address(&self, h: Household, t: i64) -> Pos {
        self.unit_pos(self.anchor_unit(h), t).expect("a household's anchor exists")
    }

    /// Where `x` lives at `t`, if present.
    pub fn address_of(&self, x: Pid, t: i64) -> Option<Pos> {
        self.household(x, t).map(|h| self.address(h, t))
    }

    /// `x`'s home at `t`, if present: position, dwelling and mail address
    /// (a house number and street keyed by the dwelling, a ZIP of the tract
    /// by land area, its postal place).
    pub fn home(&self, x: Pid, t: i64) -> Option<Home> {
        let h = self.household(x, t)?;
        let (pos, dwelling) = self.unit_dwelling(self.anchor_unit(h), t)?;
        let places = self.places();
        let key = match dwelling.unit {
            Unit::Union(w) => self.pkey(w, T_ADDRESS).with(1),
            Unit::Spell(p, s) => self.pkey(p, T_ADDRESS).with2(2, s as u64),
        }
        .with(dwelling.since as u64);
        // House numbers: mostly low, a long tail to five digits.
        let number = (procedural_core::dmath::pow(10.0, 0.3 + 3.6 * key.with(1).unit()) as u64).clamp(1, 19_999);
        let street = format!("{number} {}", places.street(key.with(2).unit()).unwrap_or("Main St"));
        let (county, fips) = places.county(pos[COUNTY]);
        let (zip, city, state) = match places.zip_in(pos[TRACT], key.with(3).unit()) {
            Some((code, st, place)) => (Some(code), place.to_string(), state_abbr(st as u32)),
            None => (None, county.to_string(), state_abbr(fips / 1000)),
        };
        Some(Home { pos, dwelling, street, city, state, zip })
    }

    /// Where `x` was born: their mother's address at the birth (a founder's:
    /// their first unit's seed area).
    pub fn birth_place(&self, x: Pid) -> Option<Pos> {
        let (m, birth, _) = self.parents(x)?;
        self.address_of(m, birth)
    }
}
