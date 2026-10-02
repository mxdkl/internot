#!/usr/bin/env python3
"""Distill the public-domain geography datasets into `places.bin`.

Inputs (git-ignored, in `datasets/geo/` at the repository root):
- `cenpop/CenPop2020_Mean_TR.txt`: 2020 census tracts with population and
  population centre (Census Bureau, centers of population).
- `cenpop/CenPop2010_Mean_CO.txt`: 2010 county populations.
- `popest/co-est00int-tot.csv`, `popest/co-est2025-alldata.csv`: county
  populations for 2000 and 2025 (Census Bureau population estimates).
- `cz/ers_2020-commuting-zones.csv`: county to 2020 commuting zone (USDA ERS,
  Fowler & Jensen delineation). Connecticut's 2020 counties are mapped
  through its planning regions, which all lie in one zone.
- `history/forstall1996/forstall_partIII_counties_1800_1990.csv` and
  `..._partIV_county_dates_fips.csv`: county populations at every census
  1800–1990 (Forstall 1996, Census Bureau), joined to FIPS by name.

All are US government works (public domain).

Run from the repository root (Python 3, standard library only):
    python3 internot_society/data/distill_places.py [area cap]
It writes `worlds/us/data/places.bin`, the `us` pack's place tree.

The tree (spec 2026-10-01-residence, §2.1 and §7):
- **Areas**, the closed units of design A: each state's commuting zones (a
  zone belongs to the state of its most populous county), cut by recursive
  bisection of zone centres when a state has more than `cap` people in 2020
  (default 12,000,000). A zone is never split (research note §1f): a zone
  bigger than `cap` is an area of its own.
- **Zones** are the ERS 2020 commuting zones, whole.
- **Counties** are 2020 counties; a county over 2M people is cut into parts
  of about 1M by recursive bisection of its tracts (the note's "county or
  1M part" level).
- **Clusters** are groups of at most 16 neighbouring tracts within a county
  node, by recursive bisection on the wider of latitude and longitude.
- **Tracts** are 2020 tracts with people in the 50 states and DC
  (zero-population tracts and Puerto Rico dropped).

Weights by decade, 1840 to 2100 (27 decades), are populations:
- a tract's weight in decade d is its county's population then times the
  tract's share of the county in 2020;
- county populations are the census counts 1840–1990 (Forstall), 2000,
  2010, 2020 (tract sums) and the 2025 estimate, held after 2025;
- each state's counties are scaled to the state's census total, which
  spreads the population of counties that no longer exist (or are named
  differently) over the state's counties in proportion; a state with no
  matched county in a decade spreads its total by 2020 shares;
- a county with no population in a decade (not yet organized) weighs 0.

Binary format (little-endian; `v` = unsigned LEB128):
    b"INTPLACE", u8 version (2)
    u16 first decade, u8 decades
    areas:    v n; n x (u8 len, UTF-8 name, e.g. "CA 2")
    zones:    v n; n x (v commuting zone, v area, u8 len, UTF-8 zone name)
    counties: v n; n x (v FIPS, v zone, u8 len, UTF-8 name)
    clusters: v n; n x (v county)
    tracts:   v n; n x (v GEOID, f32 latitude, f32 longitude, v cluster,
                        decades x v weight)
Every level is listed in tree order: a node's children are contiguous.
States are the first two FIPS digits of a county (or of a tract's GEOID);
a world pack maps them to its lineage regions (`places.ron`).
"""

import csv
import os
import struct
import sys
from collections import defaultdict

ROOT = os.path.join(os.path.dirname(__file__), "..", "..")
GEO = os.path.join(ROOT, "datasets", "geo")
OUT = os.path.join(ROOT, "worlds", "us", "data", "places.bin")

FIRST_DECADE = 1840
DECADES = 27  # 1840 ... 2100
CLUSTER_MAX = 16
COUNTY_SPLIT = 2_000_000
COUNTY_PART = 1_000_000
STATE_POSTAL = dict(zip(
    "01 02 04 05 06 08 09 10 11 12 13 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30 31 32 33 34 35 36 37 38 39 40 41 42 44 45 46 47 48 49 50 51 53 54 55 56".split(),
    "AL AK AZ AR CA CO CT DE DC FL GA HI ID IL IN IA KS KY LA ME MD MA MI MN MS MO MT NE NV NH NJ NM NY NC ND OH OK OR PA RI SC SD TN TX UT VT VA WA WV WI WY".split(),
))

# FIPS changes since the 1990 codes Forstall uses.
FIPS_RENAMED = {"12025": "12086", "46113": "46102", "02270": "02158"}


def leb(n):
    out = bytearray()
    while True:
        b = n & 0x7F
        n >>= 7
        if n:
            out.append(b | 0x80)
        else:
            out.append(b)
            return bytes(out)


def text(s):
    b = s.encode("utf-8")[:255]
    return bytes([len(b)]) + b


def load_tracts():
    tracts = []
    path = os.path.join(GEO, "cenpop", "CenPop2020_Mean_TR.txt")
    with open(path, encoding="utf-8-sig") as f:
        for r in csv.DictReader(f):
            pop = int(r["POPULATION"])
            # The 50 states and DC: the world's demography is calibrated to
            # them (Puerto Rico, state 72, is not in the census totals).
            if pop <= 0 or int(r["STATEFP"]) > 56:
                continue
            tracts.append(
                {
                    "county": r["STATEFP"] + r["COUNTYFP"],
                    "geoid": int(r["STATEFP"] + r["COUNTYFP"] + r["TRACTCE"]),
                    "pop": pop,
                    "lat": float(r["LATITUDE"]),
                    "lon": float(r["LONGITUDE"]),
                }
            )
    return tracts


def load_cz():
    cz, names, county_names = {}, {}, {}
    with open(os.path.join(GEO, "cz", "ers_2020-commuting-zones.csv"), encoding="utf-8-sig") as f:
        for r in csv.DictReader(f):
            cz[r["FIPStxt"]] = int(r["CZ2020"])
            names[int(r["CZ2020"])] = r["CZName"]
            county_names[r["FIPStxt"]] = r["CountyName"]
    # Connecticut: every planning region lies in one zone.
    ct = {cz[k] for k in cz if k.startswith("09")}
    assert len(ct) == 1, "Connecticut spans several zones"
    return cz, names, county_names, ct.pop()


def county_series(tract_pop2020):
    """Population by county and census year: {fips: {year: pop}}."""
    pops = defaultdict(dict)
    for fips, p in tract_pop2020.items():
        pops[fips][2020] = p
    with open(os.path.join(GEO, "cenpop", "CenPop2010_Mean_CO.txt"), encoding="latin-1") as f:
        for r in csv.DictReader(f):
            pops[r["STATEFP"] + r["COUNTYFP"]][2010] = int(r["POPULATION"])
    with open(os.path.join(GEO, "popest", "co-est00int-tot.csv"), encoding="latin-1") as f:
        for r in csv.DictReader(f):
            if r["SUMLEV"].lstrip("0") == "50":
                pops["%02d%03d" % (int(r["STATE"]), int(r["COUNTY"]))][2000] = int(r["ESTIMATESBASE2000"])
    with open(os.path.join(GEO, "popest", "co-est2025-alldata.csv"), encoding="latin-1") as f:
        for r in csv.DictReader(f):
            if r["SUMLEV"] == "050":
                pops[r["STATE"] + r["COUNTY"]][2025] = int(r["POPESTIMATE2025"])
    # Forstall 1840-1990, joined to FIPS by (state, county name).
    hist = os.path.join(GEO, "history", "forstall1996")
    fips_of, state = {}, None
    with open(os.path.join(hist, "forstall_partIV_county_dates_fips.csv"), encoding="utf-8-sig") as f:
        for r in csv.DictReader(f):
            if r["fips"].endswith("000"):
                state = r["county"].upper()
                continue
            fips_of[(state, r["county"].strip().lower())] = FIPS_RENAMED.get(r["fips"], r["fips"])
    state_totals = defaultdict(dict)  # state FIPS -> {year: pop}
    state_fips = {}
    matched = unmatched = 0
    with open(os.path.join(hist, "forstall_partIII_counties_1800_1990.csv"), encoding="utf-8-sig") as f:
        rows = list(csv.DictReader(f))
    for r in rows:
        if r["kind"] == "state":
            continue
        k = (r["state"].upper(), r["county"].strip().lower())
        if k in fips_of:
            state_fips[r["state"].upper()] = fips_of[k][:2]
    for r in rows:
        years = {}
        for y in range(1840, 1991, 10):
            v = r[str(y)].replace(",", "").strip()
            if v.isdigit():
                years[y] = int(v)
        if r["kind"] == "state":
            sf = state_fips.get(r["state"].upper())
            if sf:
                state_totals[sf].update(years)
            continue
        f = fips_of.get((r["state"].upper(), r["county"].strip().lower()))
        if f is None:
            unmatched += 1
            continue
        matched += 1
        for y, p in years.items():
            pops[f][y] = pops[f].get(y, 0) + p
    print(f"Forstall counties joined to FIPS: {matched}, not joined: {unmatched}", file=sys.stderr)
    return pops, state_totals


def bisect(items, key_lat, key_lon, cap, weight):
    """Split `items` into groups of total `weight` at most `cap` by
    recursive bisection on the wider coordinate, at the weighted median."""
    total = sum(weight(i) for i in items)
    if total <= cap or len(items) <= 1:
        return [items]
    lats = [key_lat(i) for i in items]
    lons = [key_lon(i) for i in items]
    by = key_lat if max(lats) - min(lats) >= max(lons) - min(lons) else key_lon
    items = sorted(items, key=lambda i: (by(i), key_lat(i), key_lon(i)))
    acc, cut = 0, len(items) // 2
    for k, i in enumerate(items):
        acc += weight(i)
        if acc * 2 >= total:
            cut = max(1, min(len(items) - 1, k + 1))
            break
    return bisect(items[:cut], key_lat, key_lon, cap, weight) + bisect(items[cut:], key_lat, key_lon, cap, weight)


def main():
    cap = int(sys.argv[1]) if len(sys.argv) > 1 else 12_000_000
    tracts = load_tracts()
    cz_of, cz_names, county_names, ct_zone = load_cz()
    by_county = defaultdict(list)
    for t in tracts:
        by_county[t["county"]].append(t)
    county_pop2020 = {c: sum(t["pop"] for t in ts) for c, ts in by_county.items()}
    pops, state_totals = county_series(county_pop2020)

    def zone(c):
        return ct_zone if c.startswith("09") else cz_of[c]

    # County weights by decade, scaled to state totals where Forstall has them.
    years = [FIRST_DECADE + 10 * d for d in range(DECADES)]
    weight = {c: [0.0] * DECADES for c in by_county}
    for d, y in enumerate(years):
        src = min(y, 2025) if y > 2020 else y
        for c in by_county:
            series = pops.get(c, {})
            weight[c][d] = float(series.get(src, series.get(2020, 0) if src == 2025 else 0))
        if y <= 1990:
            by_state = defaultdict(list)
            for c in by_county:
                by_state[c[:2]].append(c)
            for s, cs in by_state.items():
                total = state_totals.get(s, {}).get(y)
                if not total:
                    continue
                have = sum(weight[c][d] for c in cs)
                if have > 0:
                    for c in cs:
                        weight[c][d] *= total / have
                else:
                    share = sum(county_pop2020[c] for c in cs)
                    for c in cs:
                        weight[c][d] = total * county_pop2020[c] / share

    # Zones are kept whole (research note §1f: splitting turns local moves
    # into static long moves). Counties above 2M are cut into parts of about
    # 1M by recursive bisection of their tracts (the note's "county or 1M
    # part" level).
    zones = defaultdict(list)
    for c in by_county:
        zones[zone(c)].append(c)
    county_parts = {}  # county -> [tracts per part]
    for c, ts in by_county.items():
        if county_pop2020[c] > COUNTY_SPLIT:
            county_parts[c] = bisect(ts, lambda t: t["lat"], lambda t: t["lon"], COUNTY_PART, lambda t: t["pop"])
        else:
            county_parts[c] = [ts]
    zone_pop = {z: sum(county_pop2020[c] for c in cs) for z, cs in zones.items()}
    zone_at = {}
    for z, cs in zones.items():
        tot = zone_pop[z]
        zone_at[z] = (
            sum(t["lat"] * t["pop"] for c in cs for t in by_county[c]) / tot,
            sum(t["lon"] * t["pop"] for c in cs for t in by_county[c]) / tot,
        )
    # Areas, the closed units: a state's zones (a zone belongs to the state
    # of its most populous county), cut above `cap` by recursive bisection
    # of zone centres, never splitting a zone.
    zone_state = {z: max(cs, key=lambda c: (county_pop2020[c], c))[:2] for z, cs in zones.items()}
    by_state = defaultdict(list)
    for z in zones:
        by_state[zone_state[z]].append(z)
    areas = []  # (state, [zones])
    for st in sorted(by_state):
        parts = bisect(sorted(by_state[st]), lambda z: zone_at[z][0], lambda z: zone_at[z][1], cap, lambda z: zone_pop[z])
        for p in parts:
            areas.append((st, sorted(p)))

    out = bytearray(b"INTPLACE")
    out.append(2)
    out += struct.pack("<HB", FIRST_DECADE, DECADES)
    out += leb(len(areas))
    per_state = defaultdict(int)
    for st, _ in areas:
        per_state[st] += 1
        out += text(f"{STATE_POSTAL.get(st, st)} {per_state[st]}")
    zone_rows, county_nodes, cluster_nodes, tract_rows = [], [], [], []
    for a, (_, zs) in enumerate(areas):
        for z in zs:
            zi = len(zone_rows)
            zone_rows.append((z, a))
            for c in sorted(zones[z]):
                share = county_pop2020[c]
                for part in county_parts[c]:
                    ci = len(county_nodes)
                    county_nodes.append((int(c), zi, county_names.get(c, c)))
                    for group in bisect(part, lambda t: t["lat"], lambda t: t["lon"], CLUSTER_MAX, lambda t: 1):
                        li = len(cluster_nodes)
                        cluster_nodes.append(ci)
                        for t in sorted(group, key=lambda t: t["geoid"]):
                            w = [round(weight[c][d] * t["pop"] / share) for d in range(DECADES)]
                            tract_rows.append((t["geoid"], t["lat"], t["lon"], li, w))
    out += leb(len(zone_rows))
    for z, a in zone_rows:
        out += leb(z) + leb(a) + text(cz_names.get(z, "Connecticut"))
    out += leb(len(county_nodes))
    for fips, zi, name in county_nodes:
        out += leb(fips) + leb(zi) + text(name)
    out += leb(len(cluster_nodes))
    for ci in cluster_nodes:
        out += leb(ci)
    out += leb(len(tract_rows))
    for geoid, lat, lon, li, w in tract_rows:
        out += leb(geoid) + struct.pack("<ff", lat, lon) + leb(li)
        for x in w:
            out += leb(x)
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    with open(OUT, "wb") as f:
        f.write(out)
    sizes = sorted((sum(zone_pop[z] for z in zs), f"{STATE_POSTAL.get(st, st)}") for st, zs in areas)
    print(
        f"{len(areas)} areas, {len(zone_rows)} zones, {len(county_nodes)} county nodes, "
        f"{len(cluster_nodes)} clusters, {len(tract_rows)} tracts; {len(out) / 1e6:.1f} MB",
        file=sys.stderr,
    )
    print(f"  largest areas (2020): {sizes[-6:]}", file=sys.stderr)
    # Check: census-region shares against CPH-2-1 Table 20 (research note).
    region = {}
    for name, states in (
        ("NE", "09 23 25 33 44 50 34 36 42"),
        ("MW", "17 18 26 39 55 19 20 27 29 31 38 46"),
        ("S", "10 11 12 13 24 37 45 51 54 01 21 28 47 05 22 40 48"),
        ("W", "04 08 16 30 32 35 49 56 02 06 15 41 53"),
    ):
        for st in states.split():
            region[int(st)] = name
    for y in (1840, 1850, 1900, 1950, 2000, 2020):
        d = (y - FIRST_DECADE) // 10
        total = sum(r[4][d] for r in tract_rows)
        by = defaultdict(int)
        for geoid, _, _, _, w in tract_rows:
            by[region[geoid // 10**9]] += w[d]
        shares = " ".join(f"{k} {100 * by[k] / total:.1f}" for k in ("NE", "MW", "S", "W"))
        print(f"  {y}: total {total:,}; {shares}", file=sys.stderr)


if __name__ == "__main__":
    main()
