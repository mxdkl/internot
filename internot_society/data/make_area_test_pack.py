#!/usr/bin/env python3
"""Write the `us-areas-tiny` pack: the area mode (B at area level, spec
2026-10-01-ledger-areas.md) at test scale, on four neighbouring areas.

The exhaustive kinship suite needs blocks about as thick as `us-tiny`'s
(300 founder births a year over 2 regions × 5 heritages), so the test world
keeps a few areas of similar size instead of all 62 (with 62, blocks hold a
handful of people and pairwise kin repair cannot always avoid siblings).

Reads `worlds/us/data/places.bin` (format in distill_places.py), keeps the
areas named in AREAS with their zones, counties, clusters and tracts
(renumbered, in the same order), and writes into `worlds/us-areas-tiny/`:
- `data/places.bin`, the subset (it replaces the parent's file of the same
  path);
- `world.ron`, extending `us-tiny`: one region per kept area, with
  founder_weight the area's share of the kept population in 1900 (where the
  test world starts) and immigrant_weight the mean of its shares in 1900 and
  2020, the parent's two regions emptied, and twice `us-tiny`'s founder
  births for twice the regions;
- `places.ron` (area mode) and `unions.ron` (markets by area, as in
  `us-areas`).

Run from the repository root:
    python3 internot_society/data/make_area_test_pack.py
"""

import os
import struct

ROOT = os.path.join(os.path.dirname(__file__), "..", "..")
PLACES = os.path.join(ROOT, "worlds", "us", "data", "places.bin")
OUT = os.path.join(ROOT, "worlds", "us-areas-tiny")
AREAS = ["PA 1", "PA 2", "OH 1", "OH 2"]
FOUNDER_BIRTHS = 600.0


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


def parse(b):
    assert b[:8] == b"INTPLACE" and b[8] == 2, "needs places.bin version 2"
    pos = [9]

    def rleb():
        n, shift = 0, 0
        while True:
            x = b[pos[0]]
            pos[0] += 1
            n |= (x & 0x7F) << shift
            shift += 7
            if x < 0x80:
                return n

    def rtext():
        n = b[pos[0]]
        s = b[pos[0] + 1 : pos[0] + 1 + n].decode("utf-8")
        pos[0] += 1 + n
        return s

    first, decades = struct.unpack_from("<HB", b, pos[0])
    pos[0] += 3
    areas = [rtext() for _ in range(rleb())]
    zones = [(rleb(), rleb(), rtext()) for _ in range(rleb())]
    counties = [(rleb(), rleb(), rtext()) for _ in range(rleb())]
    clusters = [rleb() for _ in range(rleb())]
    tracts = []
    for _ in range(rleb()):
        geoid = rleb()
        lat, lon = struct.unpack_from("<ff", b, pos[0])
        pos[0] += 8
        li = rleb()
        tracts.append((geoid, lat, lon, li, [rleb() for _ in range(decades)]))
    return first, decades, areas, zones, counties, clusters, tracts


def main():
    first, decades, areas, zones, counties, clusters, tracts = parse(open(PLACES, "rb").read())
    # In file order, so the subset stays in tree order.
    keep = sorted(areas.index(a) for a in AREAS)
    area_new = {a: i for i, a in enumerate(keep)}
    zone_new, zone_rows = {}, []
    for zi, (code, a, name) in enumerate(zones):
        if a in area_new:
            zone_new[zi] = len(zone_rows)
            zone_rows.append((code, area_new[a], name))
    county_new, county_rows = {}, []
    for ci, (fips, zi, name) in enumerate(counties):
        if zi in zone_new:
            county_new[ci] = len(county_rows)
            county_rows.append((fips, zone_new[zi], name))
    cluster_new, cluster_rows = {}, []
    for li, ci in enumerate(clusters):
        if ci in county_new:
            cluster_new[li] = len(cluster_rows)
            cluster_rows.append(county_new[ci])
    tract_rows = [(g, la, lo, cluster_new[li], w) for g, la, lo, li, w in tracts if li in cluster_new]

    out = bytearray(b"INTPLACE")
    out.append(2)
    out += struct.pack("<HB", first, decades)
    out += leb(len(keep))
    for a in keep:
        out += text(areas[a])
    out += leb(len(zone_rows))
    for code, a, name in zone_rows:
        out += leb(code) + leb(a) + text(name)
    out += leb(len(county_rows))
    for fips, zi, name in county_rows:
        out += leb(fips) + leb(zi) + text(name)
    out += leb(len(cluster_rows))
    for ci in cluster_rows:
        out += leb(ci)
    out += leb(len(tract_rows))
    for geoid, lat, lon, li, w in tract_rows:
        out += leb(geoid) + struct.pack("<ff", lat, lon) + leb(li)
        for x in w:
            out += leb(x)
    os.makedirs(os.path.join(OUT, "data"), exist_ok=True)
    with open(os.path.join(OUT, "data", "places.bin"), "wb") as f:
        f.write(out)

    # Area weights by decade, from the kept tracts.
    weight = [[0] * decades for _ in keep]
    for _, _, _, li, w in tract_rows:
        a = zone_rows[county_rows[cluster_rows[li]][1]][1]
        for d in range(decades):
            weight[a][d] += w[d]
    share = lambda a, year: weight[a][(year - first) // 10] / sum(w[(year - first) // 10] for w in weight)
    lines = [
        "// The area mode (B at area level, spec 2026-10-01-ledger-areas.md) at",
        "// test scale: us-tiny on four neighbouring areas, for the exhaustive",
        "// kinship suite (TEST_PACK=us-areas-tiny). Generated by",
        "// internot_society/data/make_area_test_pack.py, with data/places.bin the",
        "// subset of the us place tree; founder_weight is the area's share of the",
        "// kept population in 1900, immigrant_weight the mean of its shares in",
        "// 1900 and 2020.",
        "(",
        '    name: "United States, area lineages (tiny test world)",',
        '    description: "us-tiny on four areas in area mode: 1900–1990, boosted same-sex and cross-heritage unions.",',
        '    extends: Some("us-tiny"),',
        "    founders: (",
        f"        births: {FOUNDER_BIRTHS},",
        "        growth: 0.01,",
        "    ),",
        "    regions: [",
        "        // The parent pack's two regions, emptied (lists merge by id).",
        '        (id: "east", founder_weight: 0.0, immigrant_weight: 0.0),',
        '        (id: "west", founder_weight: 0.0, immigrant_weight: 0.0),',
    ]
    for i, a in enumerate(keep):
        f = share(i, 1900)
        im = 0.5 * (share(i, 1900) + share(i, 2020))
        lines.append(f'        (id: "{areas[a]}", name: "{areas[a]}", founder_weight: {f:.6f}, immigrant_weight: {im:.6f}),')
    lines += ["    ],", ")", ""]
    with open(os.path.join(OUT, "world.ron"), "w") as f:
        f.write("\n".join(lines))
    with open(os.path.join(OUT, "places.ron"), "w") as f:
        f.write(
            "// Each area is its own lineage region (the world region named after it);\n"
            "// data/places.bin holds only the test world's four areas.\n"
            "(\n    by_area: true,\n)\n"
        )
    with open(os.path.join(OUT, "unions.ron"), "w") as f:
        f.write(
            "// Markets by area, as in us-areas: partners meet where they live, with a\n"
            "// small national share.\n"
            "(\n"
            "    national_market_share: [(1840, 0.06), (1950, 0.08), (2020, 0.10)],\n"
            "    local_open_markets: true,\n"
            ")\n"
        )
    print(f"{len(keep)} areas, {len(tract_rows)} tracts written to {OUT}")


if __name__ == "__main__":
    main()
