#!/usr/bin/env python3
"""Distill the public-domain name datasets into `names.bin`.

Inputs (git-ignored, in `datasets/names/` at the repository root):
- `ssa_names.zip`: SSA baby names, births by first name, sex and year,
  1880 onward (names given to at least 5 babies of a sex in a year).
  https://www.ssa.gov/oact/babynames/limits.html ("National data").
- `Names2020_FirstNames_RaceHispanic.xlsx`, `Names2020_FirstNames_Sex.xlsx`,
  `Names2020_LastNames_RaceHispanic.xlsx`: Census 2020 frequently occurring
  first names and surnames, counts by race and Hispanic origin and by sex.
  https://www2.census.gov/topics/genealogy/2020surnames/

Both sources are US government works (public domain).

Run from the repository root (needs Python 3 with openpyxl):
    python3 internot_society/data/distill_names.py
It writes `worlds/us/data/names.bin`, the `us` pack's name data.

What the distillate keeps:
- every first name in either source, with its display form;
- per Census first name, its counts in the six race/Hispanic columns and by
  sex, plus the "ALL OTHER NAMES" tail;
- per SSA (name, sex), its yearly births;
- every Census surname with its six counts, its display form and the tail.

Cleaning (each rule exists because a sample showed the problem):
- SSA records for unnamed infants ("Unknown", "Infant", "Notnamed", ...)
  are dropped.
- Census-only first names that are response artifacts ("REF", "MISS", "HIM",
  ...) or surnames typed into the first-name field (a listed surname at
  least 50 times as common as the first name, e.g. "GONZALEZ") are dropped.
- Census-only Hispanic compound names lost their spaces
  ("MARIADELPILAR"); they are split back into known names and particles
  ("Maria del Pilar").
- Census surnames lost their punctuation and inner capitals ("OBRIEN",
  "MCDONALD", "DELACRUZ"). Display forms restore the common patterns:
  Mc and Mac prefixes, a short list of Irish O' surnames, Spanish particles,
  Dutch "Van" and "St." with a saint's name. Everything else is title case.

Binary format (little-endian; `v` = unsigned LEB128, `z` = zigzag LEB128):
    b"INTNAMES", u8 version (2)
    columns:     v n; n x (u8 len, ASCII column name)
    first names: v n; n x (u8 len, ASCII display);
                 n x (u8 flags [bit 0: in Census]; if in Census:
                      6 v group counts, 2 v sex counts [male, female]);
                 6 v group counts of the Census tail
    SSA:         u16 first year, u16 last year; v series;
                 per series (sorted by name, then sex): v name delta,
                 u8 sex (0 female, 1 male), v start offset, v length,
                 length x z count deltas (a zero count is an absent year)
    surnames:    v n; n x (u8 len, ASCII display, 6 v group counts);
                 6 v group counts of the Census tail
Columns, in order (named in the file): "white", "black", "aian", "asian",
"multi", "hispanic": non-Hispanic White, Black, AIAN, Asian and Pacific
Islander, two or more races, and Hispanic (any race). A world pack maps its
heritage groups to columns by name (`names.ron`).
"""

import io
import os
import re
import struct
import sys
import zipfile

import openpyxl

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
SRC = os.path.join(ROOT, "datasets", "names")
OUT = os.path.join(ROOT, "worlds", "us", "data", "names.bin")

WHITE, BLACK, AIAN, ASIAN, MULTI, HISP = range(6)

SSA_PLACEHOLDERS = {
    "UNKNOWN", "INFANT", "NOTNAMED", "UNNAMED", "NONAME", "NAME",
    "BABYBOY", "BABYGIRL", "MALE", "FEMALE", "BOY", "GIRL",
}
CENSUS_ARTIFACTS = {"REF", "MISS", "MEN", "HIM", "DAU", "FAM", "NON", "INF"}

# Irish O' surnames among the Census candidates (White-dominant, the rest a
# listed surname). Curated: "OBERG", "OSTER", "OLSON" are not O' names.
IRISH_O = set("""
BANION BANNON BARR BEIRNE BOYLE BRIAN BRIANT BRIEN BRYAN BRYANT BYRNE
CALLAGHAN CARROLL CONNELL CONNOR DANIEL DAY DEA DELL DOHERTY DONNELL
DONOGHUE DONOHUE DONOVAN DOWD DRISCOLL DWYER FALLON FARRELL FLAHERTY FLYNN
GARA GORMAN GRADY HAGAN HAIR HALLORAN HANLON HARA HARE HARRA HEARN HERN
KANE KEEFE KEEFFE KELLEY KELLY LAUGHLIN LEARY LOUGHLIN MAHONEY MAHONY
MALLEY MARA MEARA MELIA NEAL NEALE NEIL NEILL QUIN QUINN REAR REGAN
REILLY RILEY RIORDAN ROURKE SHAUGHNESSY SHEA SHIELDS SULLIVAN TOOLE
""".split())

# St. surnames: "St." and a saint's name.
SAINTS = set("""
AMAND AMOUR ANDRE ARNAUD AUBIN CHARLES CLAIR CLAIRE CROIX CYR DENIS
GELAIS GEORGE GERMAIN HILAIRE JACQUES JAMES JEAN JOHN JULIEN LAURENT
LEGER LOUIS MARIE MARTIN MICHEL ONGE PAUL PETER PIERRE ROMAIN THOMAS
VINCENT YVES
""".split())

SPANISH_PARTICLES = [("DELOS", "De Los"), ("DELAS", "De Las"), ("DELA", "De La"),
                     ("DEL", "Del"), ("DE", "De")]


def read_census(name, columns):
    wb = openpyxl.load_workbook(os.path.join(SRC, name), read_only=True)
    out = {}
    for i, row in enumerate(wb.worksheets[0].iter_rows(values_only=True)):
        if i < 3 or row[0] is None:
            continue
        out[str(row[0]).upper()] = [int(row[c]) for c in columns]
    return out


def read_ssa():
    """{(NAME, sex): {year: births}} and {NAME: display form}."""
    series, display = {}, {}
    with zipfile.ZipFile(os.path.join(SRC, "ssa_names.zip")) as z:
        for info in z.infolist():
            m = re.fullmatch(r"yob(\d{4})\.txt", info.filename)
            if not m:
                continue
            year = int(m.group(1))
            for line in io.TextIOWrapper(z.open(info), encoding="ascii"):
                name, sex, count = line.strip().split(",")
                key = name.upper()
                if key in SSA_PLACEHOLDERS:
                    continue
                display.setdefault(key, name)
                s = series.setdefault((key, 0 if sex == "F" else 1), {})
                s[year] = s.get(year, 0) + int(count)
    return series, display


def segment(name, tokens):
    """Split a run-together Spanish compound ("MARIADELPILAR") into its
    parts, or None. Compounds open with a common head (Maria, Jose, Juan,
    ...); the rest is one known name after a particle, or a common name
    (5,000+ SSA births) on its own. Single names that merely contain
    shorter names ("ALBANIA", "RUBIELA", "ANACLETA") stay whole."""
    heads = {"MARIA": "Maria", "MA": "Maria", "JOSE": "Jose", "JUAN": "Juan",
             "ANA": "Ana", "LUZ": "Luz", "ROSA": "Rosa", "MANUEL": "Manuel",
             "LUIS": "Luis"}
    particles = [("DELOS", "de los"), ("DELAS", "de las"), ("DELA", "de la"),
                 ("DEL", "del"), ("DE", "de")]
    for head, shown in heads.items():
        if not name.startswith(head):
            continue
        rest = name[len(head):]
        if tokens.get(rest, 0) >= 5000:
            return f"{shown} {rest.title()}"
        for p, word in particles:
            if rest.startswith(p) and rest[len(p):] in tokens:
                return f"{shown} {word} {rest[len(p):].title()}"
    return None


def surname_display(name, counts, listed):
    total = counts[0]

    def share(*gs):
        return sum(counts[1 + g] for g in gs) / total if total else 0.0

    if name.startswith("MC") and len(name) >= 5:
        return "Mc" + name[2:].title()
    if (name.startswith("MAC") and len(name) >= 7 and name[3:] in listed and share(WHITE) > 0.6
            and listed[name[3:]][1 + ASIAN] < 0.5 * listed[name[3:]][0]):
        return "Mac" + name[3:].title()
    if name.startswith("O") and name[1:] in IRISH_O:
        return "O'" + name[1:].title()
    if share(HISP, ASIAN) > 0.5:
        # The longest particle whose remainder is a Hispanic surname, unless
        # a shorter one leaves a far more common one: DELATORRE is De La
        # Torre, DELEON is De Leon (not Del Eon); DESAI (Indian) stays whole.
        best = None
        for p, shown in SPANISH_PARTICLES:
            rest = name[len(p):]
            if name.startswith(p) and len(rest) >= 3 and rest in listed:
                c = listed[rest]
                if c[1 + HISP] >= 0.3 * c[0] and (best is None or c[0] > 10 * best[0]):
                    best = (c[0], shown + " " + rest.title())
        if best:
            return best[1]
    if name.startswith("VAN") and len(name) >= 7 and name[3:] in listed and share(WHITE) > 0.7:
        return "Van " + name[3:].title()
    if name.startswith("ST") and name[2:] in SAINTS:
        return "St. " + name[2:].title()
    return name.title()


def v(x):
    assert x >= 0
    out = bytearray()
    while True:
        b = x & 0x7F
        x >>= 7
        if x:
            out.append(b | 0x80)
        else:
            out.append(b)
            return bytes(out)


def z(x):
    return v((x << 1) if x >= 0 else ((-x << 1) - 1))


def ascii_name(s):
    b = s.encode("ascii")
    assert 0 < len(b) < 256, s
    return bytes([len(b)]) + b


def main():
    print("reading", SRC, file=sys.stderr)
    first_race = read_census("Names2020_FirstNames_RaceHispanic.xlsx", [2, 5, 6, 7, 8, 9, 10])
    first_sex = read_census("Names2020_FirstNames_Sex.xlsx", [2, 5, 6])
    last = read_census("Names2020_LastNames_RaceHispanic.xlsx", [2, 5, 6, 7, 8, 9, 10])
    series, ssa_display = read_ssa()

    first_tail = first_race.pop("ALL OTHER NAMES")[1:]
    first_sex.pop("ALL OTHER NAMES")
    last_tail = last.pop("ALL OTHER NAMES")[1:]
    listed = last

    ssa_total = {}
    for (name, _), s in series.items():
        ssa_total[name] = ssa_total.get(name, 0) + sum(s.values())
    tokens = {n: c for n, c in ssa_total.items() if c >= 1000 and len(n) >= 3}

    # First names: every SSA name, plus clean Census-only names.
    display = dict(ssa_display)
    dropped = []
    for name, counts in first_race.items():
        if name in display:
            continue
        if name in CENSUS_ARTIFACTS:
            dropped.append(name)
            continue
        if name in last and last[name][0] >= 50 * counts[0]:
            dropped.append(name)
            continue
        shown = None
        if counts[0] and counts[1 + HISP] / counts[0] > 0.5 and len(name) >= 7:
            shown = segment(name, tokens)
        display[name] = shown or name.title()
    print("dropped Census-only first names:", len(dropped), dropped[:20], file=sys.stderr)

    def weight(n):
        c = first_race.get(n)
        return ssa_total.get(n, 0) + (c[0] if c else 0)

    names = sorted(display, key=lambda n: (-weight(n), n))
    index = {n: i for i, n in enumerate(names)}

    out = bytearray(b"INTNAMES") + bytes([2])
    columns = ["white", "black", "aian", "asian", "multi", "hispanic"]
    out += v(len(columns))
    for c in columns:
        out += ascii_name(c)
    out += v(len(names))
    for n in names:
        out += ascii_name(display[n])
    for n in names:
        c = first_race.get(n)
        if c is None or n not in first_sex:
            out.append(0)
            continue
        out.append(1)
        for x in c[1:]:
            out += v(x)
        male, female = first_sex[n][1], first_sex[n][2]
        out += v(male) + v(female)
    for x in first_tail:
        out += v(x)

    years = sorted({y for s in series.values() for y in s})
    y0, y1 = years[0], years[-1]
    out += struct.pack("<HH", y0, y1)
    keys = sorted(series, key=lambda k: (index[k[0]], k[1]))
    out += v(len(keys))
    prev = 0
    for name, sex in keys:
        s = series[(name, sex)]
        start, end = min(s), max(s)
        out += v(index[name] - prev) + bytes([sex]) + v(start - y0) + v(end - start + 1)
        prev = index[name]
        last_count = 0
        for y in range(start, end + 1):
            c = s.get(y, 0)
            out += z(c - last_count)
            last_count = c

    surnames = sorted(last, key=lambda n: (-last[n][0], n))
    out += v(len(surnames))
    for n in surnames:
        out += ascii_name(surname_display(n, last[n], listed))
        for x in last[n][1:]:
            out += v(x)
    for x in last_tail:
        out += v(x)

    with open(OUT, "wb") as f:
        f.write(out)
    print(f"wrote {OUT}: {len(out)} bytes; {len(names)} first names, "
          f"{len(keys)} SSA series {y0}-{y1}, {len(surnames)} surnames", file=sys.stderr)


if __name__ == "__main__":
    main()
