#!/usr/bin/env python3
"""Distill US schools and colleges into `schools.bin`.

Inputs (git-ignored, in `datasets/education/` at the repository root):
- `ccd/ccd_sch_029_2425_w_1a_073025.csv`: the Common Core of Data school
  directory 2024–25 (grades offered, type, status);
- `ccd/derived_membership_total_2425.csv`: each public school's students;
- `edge/EDGE_GEOCODE_PUBLICSCH_2425.TXT`: public schools' locations (NCES
  EDGE);
- `pss/pss2324_pu.csv`: the Private School Survey 2023–24 (name, city,
  location, grades, students);
- `ipeds/HD2023.zip`: IPEDS institutional characteristics (level, control,
  degrees offered, location);
- `ipeds/EFFY2024.csv`: IPEDS 12-month enrollment by level.

All are US government works (public domain).

Run from the repository root (Python 3, standard library only):
    python3 internot_society/data/distill_schools.py
It writes `worlds/us/data/schools.bin`.

Kept: open regular public schools (CCD type 1) and private schools that
offer some of grades K–12 and have students; degree-granting colleges
active in 2023 with undergraduates or graduate students.

Binary format (little-endian; `v` = unsigned LEB128):
    b"INTSCHOL", u8 version (1)
    schools:  v n; n x (u8 kind: 0 public, 1 private; u8 lowest grade, u8
              highest grade (0 kindergarten, 1–12); f32 latitude, f32
              longitude; v students; u8 state FIPS; u8 len, UTF-8 name;
              u8 len, UTF-8 city)
    colleges: v m; m x (u8 level: 1 four-year, 2 two-year; u8 control: 1
              public, 2 private nonprofit, 3 for-profit; u8 graduate
              programs (0/1); f32 latitude, f32 longitude; v undergraduates;
              v graduate students; u8 state FIPS; u8 len, UTF-8 name; u8 len,
              UTF-8 city)
"""

import csv
import io
import os
import struct
import sys
import zipfile

ROOT = os.path.join(os.path.dirname(__file__), "..", "..")
EDU = os.path.join(ROOT, "datasets", "education")
OUT = os.path.join(ROOT, "worlds", "us", "data", "schools.bin")

STATE_FIPS = {
    "AL": 1, "AK": 2, "AZ": 4, "AR": 5, "CA": 6, "CO": 8, "CT": 9, "DE": 10, "DC": 11, "FL": 12, "GA": 13, "HI": 15, "ID": 16, "IL": 17, "IN": 18,
    "IA": 19, "KS": 20, "KY": 21, "LA": 22, "ME": 23, "MD": 24, "MA": 25, "MI": 26, "MN": 27, "MS": 28, "MO": 29, "MT": 30, "NE": 31, "NV": 32,
    "NH": 33, "NJ": 34, "NM": 35, "NY": 36, "NC": 37, "ND": 38, "OH": 39, "OK": 40, "OR": 41, "PA": 42, "RI": 44, "SC": 45, "SD": 46, "TN": 47,
    "TX": 48, "UT": 49, "VT": 50, "VA": 51, "WA": 53, "WV": 54, "WI": 55, "WY": 56,
}


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


def title(s):
    """Upper-case survey names to title case, keeping short words and initials."""
    small = {"of", "the", "and", "for", "at", "in", "on", "de", "la", "del"}
    words = s.strip().split()
    out = []
    for i, w in enumerate(words):
        lw = w.lower()
        if i > 0 and lw in small:
            out.append(lw)
        elif len(w) <= 3 and "." in w:
            out.append(w.upper() if len(w) <= 2 else w.capitalize())
        else:
            out.append("-".join(p.capitalize() for p in lw.split("-")))
    return " ".join(out)


def ccd_grade(g):
    g = g.strip()
    if g in ("KG", "PK"):
        return 0
    return int(g) if g.isdigit() and 1 <= int(g) <= 12 else None


def pss_grade(c):
    c = int(c)
    if 3 <= c <= 5:
        return 0
    if 6 <= c <= 17:
        return c - 5
    return None


def main():
    # Public schools.
    loc = {}
    with open(os.path.join(EDU, "edge", "EDGE_GEOCODE_PUBLICSCH_2425.TXT"), encoding="latin-1") as f:
        for line in f:
            p = line.rstrip("\n").split("|")
            if len(p) > 13 and p[0].isdigit():
                loc[p[0]] = (float(p[12]), float(p[13]), p[5])
    students = {}
    with open(os.path.join(EDU, "ccd", "derived_membership_total_2425.csv"), encoding="utf-8-sig") as f:
        for r in csv.DictReader(f):
            if r["STUDENT_COUNT"].strip().lstrip("-").isdigit():
                students[r["NCESSCH"]] = int(r["STUDENT_COUNT"])
    schools = []
    with open(os.path.join(EDU, "ccd", "ccd_sch_029_2425_w_1a_073025.csv"), encoding="latin-1") as f:
        for r in csv.DictReader(f):
            sid = r["NCESSCH"]
            if r["SCH_TYPE"] != "1" or r["SY_STATUS"] not in ("1", "3", "4", "5", "8") or sid not in loc:
                continue
            lo, hi = ccd_grade(r["GSLO"]), ccd_grade(r["GSHI"])
            st = STATE_FIPS.get(r["ST"])
            n = students.get(sid, 0)
            if lo is None or hi is None or hi < lo or st is None or n <= 0:
                continue
            lat, lon, city = loc[sid]
            name = r["SCH_NAME"].strip()
            # Some states report names in capitals.
            if name.isupper():
                name = title(name)
            schools.append((0, lo, hi, lat, lon, n, st, name, city.strip().title()))
    public = len(schools)
    # Private schools.
    with open(os.path.join(EDU, "pss", "pss2324_pu.csv"), encoding="latin-1") as f:
        for r in csv.DictReader(f):
            try:
                lo, hi = pss_grade(r["LOGR2024"]), pss_grade(r["HIGR2024"])
                lat, lon, n = float(r["LATITUDE24"]), float(r["LONGITUDE24"]), int(float(r["NUMSTUDS"]))
            except ValueError:
                continue
            st = STATE_FIPS.get(r["PSTABB"])
            if lo is None or hi is None or hi < lo or st is None or n <= 0:
                continue
            schools.append((1, lo, hi, lat, lon, n, st, title(r["PINST"]), title(r["PCITY"])))
    # Colleges.
    enroll = {}
    with open(os.path.join(EDU, "ipeds", "EFFY2024.csv"), encoding="utf-8-sig") as f:
        for r in csv.DictReader(f):
            if r["EFFYLEV"] in ("2", "4") and r["EFYTOTLT"].strip().isdigit():
                e = enroll.setdefault(r["UNITID"], [0, 0])
                e[0 if r["EFFYLEV"] == "2" else 1] = int(r["EFYTOTLT"])
    colleges = []
    z = zipfile.ZipFile(os.path.join(EDU, "ipeds", "HD2023.zip"))
    for r in csv.DictReader(io.TextIOWrapper(z.open("HD2023.csv"), encoding="utf-8-sig", errors="replace")):
        if r["CYACTIVE"] != "1" or r["DEGGRANT"] != "1" or r["ICLEVEL"] not in ("1", "2"):
            continue
        st = STATE_FIPS.get(r["STABBR"])
        ug, gr = enroll.get(r["UNITID"], [0, 0])
        try:
            lat, lon = float(r["LATITUDE"]), float(r["LONGITUD"])
        except ValueError:
            continue
        if st is None or ug + gr <= 0:
            continue
        control = int(r["CONTROL"]) if r["CONTROL"] in ("1", "2", "3") else 2
        name = r["INSTNM"].strip()
        colleges.append((int(r["ICLEVEL"]), control, 1 if r["GROFFER"] == "1" and gr > 0 else 0, lat, lon, ug, gr, st, title(name) if name.isupper() else name, r["CITY"].strip()))
    out = bytearray(b"INTSCHOL")
    out.append(1)
    out += leb(len(schools))
    for kind, lo, hi, lat, lon, n, st, name, city in schools:
        out += bytes([kind, lo, hi]) + struct.pack("<ff", lat, lon) + leb(n) + bytes([st]) + text(name) + text(city)
    out += leb(len(colleges))
    for lvl, control, grad, lat, lon, ug, gr, st, name, city in colleges:
        out += bytes([lvl, control, grad]) + struct.pack("<ff", lat, lon) + leb(ug) + leb(gr) + bytes([st]) + text(name) + text(city)
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    with open(OUT, "wb") as f:
        f.write(out)
    print(
        f"{public} public and {len(schools) - public} private schools ({sum(s[5] for s in schools):,} students); "
        f"{len(colleges)} colleges ({sum(c[5] for c in colleges):,} undergraduates, {sum(c[6] for c in colleges):,} graduate); "
        f"{len(out) / 1e6:.1f} MB",
        file=sys.stderr,
    )


if __name__ == "__main__":
    main()
