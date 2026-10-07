#!/usr/bin/env python3
"""Distill the ACS 2023 1-year PUMS into `work.bin`: occupations, their
wages and industries, and who works in them.

Inputs (git-ignored, at the repository root):
- `datasets/acs/pums2023_1yr/csv_pus.zip`: the person records
  (`psam_pusa.csv`, `psam_pusb.csv`);
- `datasets/acs/pums2023_1yr/PUMS_Data_Dictionary_2023.csv`: SOC and NAICS
  labels;
- `datasets/census_io/Census-2022-Occupation-Index_Final.xlsx`: the Census
  alphabetical index of occupations (about 34,500 job titles, each with its
  Census occupation code and the industries it is restricted to);
- `datasets/census_io/Census-2022-Industry-Index_Final.xlsx`: the Census
  alphabetical index of industries (about 24,000 lines of business, each
  with its Census industry code);
- `datasets/census_io/pub-io-write-ins-acs2019.xlsx`: a public-use sample of
  10,449 ACS 2019 occupation write-ins with their codes (how often a title
  is used).
All are Census Bureau works (public domain), from
`https://www2.census.gov/programs-surveys/demo/guidance/industry-occupation/`.

Run from the repository root (Python 3, standard library only; a few
minutes):
    python3 internot_society/data/distill_work.py
It writes `worlds/us/data/work.bin`.

What it computes (person weights PWGTP):
- **occupations** (SOC 2018 detailed, as PUMS codes them), each with: the
  median and log standard deviation of wages of full-time year-round workers
  25–64 (35+ hours a week, 50+ weeks; wage and salary income), the
  self-employed and government shares of its workers, and its top industries
  (NAICS recodes) with shares;
- **who holds them:** for each sex, age band (16–24, 25–64, 65+) and
  education level (less than HS, HS, some college, associate, BA, master's,
  professional, doctorate), the occupation shares of the employed;
- **labour-force status** for each sex, five-year age band (16–19, 20–24,
  …, 75+) and education level: shares employed and unemployed (the rest are
  out of the labour force);
- **job titles** of each occupation: the index's titles, cleaned (coding
  notes, abbreviations, inverted forms such as "Manager general" and
  titles naming girls or boys dropped; title case), each with the industries
  it is restricted to (mapped from Census industry codes to the PUMS NAICS
  recodes through the person records), weighted by how often the ACS
  write-ins use them, plus the occupation's own name made singular
  ("Correctional Officers And Jailers" gives "Correctional Officer" and
  "Jailer"), weighted to a third of draws;
- **lines of business** of each industry: the index's descriptions, cleaned
  ("Pizza parlor", "Coffee tables" for furniture makers), each marked as a
  place ("Pizza parlor", a name can be "{surname}'s Pizza Parlor"), a
  product (manufacturing) or an activity.

Binary format (little-endian; `v` = unsigned LEB128):
    b"INTWORK1", u8 version (2)
    industries:  v m; m x (u8 len, NAICS code; u8 len, title; v l; l x
                 (u8 len, line of business; u8 kind: 0 activity, 1 place,
                 2 product))
    occupations: v n; n x (u8 len, SOC code; u8 len, title; f32 median
                 wage; f32 log sd; f32 self-employed share; f32 government
                 share; u8 k; k x (v industry, u16 share of 65535 of all the
                 occupation's workers): its top 8 industries; v t; t x
                 (u8 len, job title; u16 weight; v r; r x v industry: the
                 industries it is restricted to, none for any))
    holders:     2 x 3 x 8 tables (sex, age band, level): v c; c x (v
                 occupation, u32 weight)
    status:      2 x 13 x 8 (sex, age band, level): f32 employed, f32
                 unemployed
"""

import csv
import io
import math
import os
import re
import struct
import sys
import xml.etree.ElementTree as ET
import zipfile
from collections import defaultdict

ROOT = os.path.join(os.path.dirname(__file__), "..", "..")
PUMS = os.path.join(ROOT, "datasets", "acs", "pums2023_1yr")
INDEX = os.path.join(ROOT, "datasets", "census_io")
OUT = os.path.join(ROOT, "worlds", "us", "data", "work.bin")


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


def level(schl):
    s = int(schl)
    if s <= 15:
        return 0
    if s <= 17:
        return 1
    if s <= 19:
        return 2
    return {20: 3, 21: 4, 22: 5, 23: 6, 24: 7}[s]


def xlsx_rows(path):
    """The rows of an .xlsx file's first sheet, as lists of strings (the
    standard library only)."""
    return xlsx_rows_sheet(path, None)


def xlsx_rows_sheet(path, sheet):
    """The rows of sheet `sheet` (by name; None: the first) of an .xlsx
    file."""
    ns = {"m": "http://schemas.openxmlformats.org/spreadsheetml/2006/main"}
    rel = "{http://schemas.openxmlformats.org/officeDocument/2006/relationships}id"
    z = zipfile.ZipFile(path)
    wb = ET.fromstring(z.read("xl/workbook.xml"))
    rels = {r.get("Id"): r.get("Target") for r in ET.fromstring(z.read("xl/_rels/workbook.xml.rels"))}
    target = None
    for sh in wb.find("m:sheets", ns):
        if sheet is None or sh.get("name") == sheet:
            target = rels[sh.get(rel)].lstrip("/")
            break
    target = target if target.startswith("xl/") else "xl/" + target
    shared = []
    if "xl/sharedStrings.xml" in z.namelist():
        for si in ET.fromstring(z.read("xl/sharedStrings.xml")).findall("m:si", ns):
            shared.append("".join(t.text or "" for t in si.iter("{%s}t" % ns["m"])))
    rows = []
    for r in ET.fromstring(z.read(target)).iter("{%s}row" % ns["m"]):
        row = {}
        for c in r.findall("m:c", ns):
            col = 0
            for ch in re.match(r"[A-Z]+", c.get("r")).group():
                col = col * 26 + ord(ch) - 64
            v = c.find("m:v", ns)
            if c.get("t") == "s" and v is not None:
                row[col - 1] = shared[int(v.text)]
            elif c.get("t") == "inlineStr":
                row[col - 1] = "".join(x.text or "" for x in c.iter("{%s}t" % ns["m"]))
            else:
                row[col - 1] = v.text if v is not None else ""
        rows.append([row.get(i, "") for i in range(max(row) + 1)] if row else [])
    return rows


def labels(var):
    out = {}
    with open(os.path.join(PUMS, "PUMS_Data_Dictionary_2023.csv"), encoding="utf-8") as f:
        for r in csv.reader(f):
            if len(r) >= 7 and r[0] == "VAL" and r[1] == var:
                out[r[4]] = r[6]
    return out


def clean_title(t):
    # "MGR-Marketing Managers" -> "Marketing Managers"; "CON-Construction
    # (The Cleaning Of …)" -> "Construction"
    if "-" in t[:5]:
        t = t.split("-", 1)[1]
    return t.split(" (")[0].strip()


SMALL = {"of", "and", "in", "for", "the", "a", "an", "to", "on", "at", "or", "with", "by", "except"}
HEADS = {
    "manager", "engineer", "clerk", "agent", "operator", "worker", "assistant", "helper", "inspector", "technician",
    "supervisor", "director", "officer", "analyst", "specialist", "consultant", "attendant", "representative",
    "teacher", "nurse", "driver", "mechanic", "repairer", "installer", "laborer", "tender", "aide", "programmer",
    "designer", "developer", "administrator", "coordinator", "examiner", "investigator", "planner", "salesperson",
    "dealer", "buyer", "cook", "cutter", "machinist", "setter", "fitter", "maker", "painter", "cleaner", "foreman",
    "sales", "repair", "guard", "monitor", "superintendent", "adjuster", "chief", "keeper", "service", "officer",
}
# Titles ending in "-man" kept (common today); others ("Cokeman", "Mop Man")
# are dropped with "mother", "father" and other dated forms.
MAN_OK = {"foreman", "fisherman", "lineman", "fireman", "salesman", "doorman", "repairman", "handyman", "cameraman", "craftsman", "draftsman", "policeman", "deliveryman", "chairman", "councilman", "alderman", "congressman", "patrolman", "longshoreman", "journeyman"}


def title_word(w, first):
    """Title case one word, keeping acronyms and mixed case ("CCU", "3D")."""
    if any(c.isupper() for c in w[1:]) or any(c.isdigit() for c in w):
        return w
    if not first and w.lower() in SMALL:
        return w.lower()
    return "-".join(p[:1].upper() + p[1:] for p in w.split("-"))


def title_case(t):
    return " ".join(title_word(w, i == 0) for i, w in enumerate(t.split()))


def singular(w):
    lw = w.lower()
    if lw.endswith("ies") and len(w) > 4:
        return w[:-3] + "y"
    if lw.endswith("sses") or lw.endswith("ches") or lw.endswith("shes") or lw.endswith("xes"):
        return w[:-2]
    if lw.endswith("s") and not lw.endswith("ss") and not lw.endswith("us") and len(w) > 3:
        return w[:-1]
    return w


def plural(w):
    lw = w.lower()
    return lw.endswith("s") and not lw.endswith("ss") and not lw.endswith("us") and len(w) > 3


def own_titles(label):
    """Singular titles from an occupation's plural name ("Hosts And
    Hostesses, Restaurant, …" gives "Host" and "Hostess"); none for
    catch-alls."""
    t = label
    if ", All Other" in t or "--" in t:
        return []
    for pre in ("Miscellaneous ", "Other "):
        if t.startswith(pre):
            t = t[len(pre):]
    # A list ("Securities, Commodities, And Financial Services Sales
    # Agents"): its last item names the workers. The first item of a list
    # doesn't end with workers ("Securities", "Electric Motor"); a name
    # followed by qualifiers does ("Hosts And Hostesses, Restaurant, …").
    first = t.split(", ")[0].split(" Of ")[0].split()[-1]
    listed = not plural(first) or singular(first).lower().endswith(("ty", "ing", "ion", "ics", "ure", "ment", "ness"))
    if ", And " in t and listed:
        t = t.rsplit(", And ", 1)[1]
        for pre in ("Related ", "Other "):
            if t.startswith(pre):
                t = t[len(pre):]
    t = t.split(", ")[0]
    tail = ""
    if " Of " in t:
        t, tail = t.split(" Of ", 1)
        tail = " of " + tail
    parts = t.split(" And ")
    if all(plural(p.split()[-1]) for p in parts if p):
        out = [" ".join(p.split()[:-1] + [singular(p.split()[-1])]) + tail for p in parts]
    elif plural(t.split()[-1]):
        out = [" ".join(t.split()[:-1] + [singular(t.split()[-1])]) + tail]
    else:
        return []
    return [title_case(o) for o in out]


def clean_index_title(t, attested=False):
    """A display title from an index entry, or None (`attested`: the title
    is a frequent ACS write-in, so abbreviations such as "RN" stay)."""
    t = " ".join(t.split())
    lt = t.lower()
    if not t or "\\" in t or "(" in t or " exc" in lt or "self employed" in lt or lt.startswith("u s ") or " etc" in lt:
        return None
    words = t.split()
    if any(w.lower() in ("girl", "girls", "boy", "boys", "ns", "nec", "mother", "father", "workman", "gang") for w in words):
        return None
    last = words[-1].lower()
    if (last == "man" or (last.endswith("man") and last not in MAN_OK)) and not attested:
        return None
    if len(words) == 1 and t.isupper() and not attested:
        return None
    if len(words) >= 2 and words[0].lower() in HEADS and words[-1].lower() not in HEADS and singular(words[-1]).lower() not in HEADS:
        return None
    # A lone abstract noun is not a title ("Security", "Maintenance").
    if len(words) == 1 and last not in ("attorney", "secretary", "deputy", "notary", "nanny", "jockey", "missionary", "emissary") and last.endswith(("y", "ion", "ment", "ness", "ance", "ence", "ics", "ure", "ism", "ship", "age")):
        return None
    # Activities, not titles ("Farming", "Child Care", "Border Patrol").
    if last.endswith("ing") or last in ("work", "care", "patrol", "help", "labor", "service", "services"):
        return None
    return title_case(t)


def drop_inverted(ts):
    """Drop titles that are another title of the occupation plus a trailing
    qualifier ("Flight Attendant Ramp" next to "Flight Attendant")."""
    lower = {t.lower() for t in ts}
    out = {}
    for t, res in ts.items():
        w = t.split()
        if len(w) > 1 and " ".join(w[:-1]).lower() in lower and w[-1].lower() not in HEADS and singular(w[-1]).lower() not in HEADS:
            continue
        out[t] = res
    return out


PLACES = {
    "shop", "store", "stand", "parlor", "restaurant", "bar", "cafe", "cafeteria", "diner", "grill", "lounge", "inn",
    "motel", "hotel", "garage", "salon", "studio", "clinic", "office", "agency", "center", "school", "academy",
    "hospital", "bank", "company", "firm", "farm", "ranch", "orchard", "mill", "plant", "yard", "station", "market",
    "bakery", "pharmacy", "nursery", "dairy", "laundry", "laboratory", "theater", "club", "camp", "library", "museum",
    "gallery", "kennel", "hatchery", "winery", "brewery", "distillery", "cannery", "foundry", "quarry", "dealer",
    "dealership", "contractor", "luncheonette", "pizzeria", "tavern", "pub", "saloon", "deli",
    "delicatessen", "supermarket", "boutique", "emporium", "outlet", "warehouse", "terminal", "depot", "home",
    "practice", "lab", "spa", "gym", "rink", "arena", "stadium", "marina", "lodge", "resort", "kitchen",
    "caterer", "florist", "jeweler", "tailor", "cleaner", "barbershop", "laundromat", "mortuary", "chapel",
    "bistro", "plantation", "bog", "house", "shoppe", "eatery", "mart", "stall", "truck", "cart", "sanatorium",
}


def clean_line(t):
    """A line of business and its kind (0 activity, 1 place, 2 product)."""
    t = " ".join(t.split())
    lt = t.lower()
    kind = 0
    for suf in (" (ret)", " (whsl)", " (const)", " (mfg)"):
        if lt.endswith(suf):
            if suf == " (mfg)":
                kind = 2
            t = t[: -len(suf)]
            lt = t.lower()
    if not t or "\\" in t or "(" in t or " exc" in lt or "self employed" in lt or lt.startswith("u s ") or lt.startswith("state ") or lt.startswith("city ") or " etc" in lt:
        return None
    words = t.split()
    if any(w in ("PR", "GOV", "OWN", "ns", "nec") for w in words) or len(words) > 6 or "full service" in lt:
        return None
    # "Repair auto body" -> "Auto body repair"
    if len(words) > 1 and words[0].lower() in ("repair", "installation", "rental", "manufacture", "sale", "sales") and "of" not in [w.lower() for w in words]:
        words = words[1:] + [words[0].lower()]
    # "Trucking Co" -> "Trucking"
    if len(words) > 1 and words[-1].lower() in ("co", "company", "cos"):
        words = words[:-1]
    # Inverted forms ("Strollers baby", "Laundries hospital"): a plural
    # first word and a singular last one.
    if len(words) > 1 and plural(words[0]) and not plural(words[-1]):
        return None
    if len(words) == 1 and kind != 2 and singular(words[0]).lower() not in PLACES:
        return None
    if kind == 0 and singular(words[-1]).lower() in PLACES:
        kind = 1
        words[-1] = singular(words[-1])
    return title_case(" ".join(words)), kind


def restriction(r, ind_naics, naics_index):
    """The NAICS recode indexes a title is restricted to (none: any)."""
    r = r.strip()
    if not r or "exc" in r.lower() or "any not listed" in r.lower():
        return []
    out = set()
    for m in re.finditer(r"(\d{4})(?:\s*-\s*(\d{4}))?", r):
        a = int(m.group(1))
        b = int(m.group(2) or a)
        for code, naics in ind_naics.items():
            if a <= code <= b and naics in naics_index:
                out.add(naics_index[naics])
    return sorted(out)


def weighted_median_sd(pairs):
    """Median of log wages and their standard deviation, weighted."""
    if not pairs:
        return 0.0, 0.0
    pairs.sort()
    total = sum(w for _, w in pairs)
    acc, med = 0, pairs[-1][0]
    for v, w in pairs:
        acc += w
        if acc * 2 >= total:
            med = v
            break
    mean = sum(v * w for v, w in pairs) / total
    var = sum(w * (v - mean) ** 2 for v, w in pairs) / total
    return med, math.sqrt(var)


def main():
    soc_title = {k: clean_title(v) for k, v in labels("SOCP").items() if k.strip("b")}
    naics_title = {k: clean_title(v) for k, v in labels("NAICSP").items() if k.strip("b")}
    holders = defaultdict(lambda: defaultdict(int))  # (sex, band, level) -> soc -> weight
    wages = defaultdict(list)  # soc -> [(log wage, weight)]
    industry = defaultdict(lambda: defaultdict(int))  # soc -> naics -> weight
    cow = defaultdict(lambda: [0, 0, 0])  # soc -> [all, self-employed, government]
    status = defaultdict(lambda: [0, 0, 0])  # (sex, band5, level) -> [employed, unemployed, all]
    occ_soc = defaultdict(lambda: defaultdict(int))  # Census occupation code -> soc -> weight
    ind_naics_w = defaultdict(lambda: defaultdict(int))  # Census industry code -> naics -> weight
    z = zipfile.ZipFile(os.path.join(PUMS, "csv_pus.zip"))
    rows = 0
    for name in z.namelist():
        if not name.endswith(".csv"):
            continue
        for r in csv.DictReader(io.TextIOWrapper(z.open(name), encoding="latin-1")):
            rows += 1
            age = int(r["AGEP"])
            if age < 16 or not r["SCHL"]:
                continue
            w = int(r["PWGTP"])
            sex = 0 if r["SEX"] == "2" else 1  # 0 female, 1 male
            lv = level(r["SCHL"])
            esr = r["ESR"]
            employed = esr in ("1", "2", "4", "5")
            b5 = 0 if age < 20 else min(1 + (age - 20) // 5, 12)
            st = status[(sex, b5, lv)]
            st[2] += w
            if employed:
                st[0] += w
            elif esr == "3":
                st[1] += w
            soc = r["SOCP"].strip()
            if not employed or not soc or soc not in soc_title:
                continue
            if r["OCCP"].strip():
                occ_soc[int(r["OCCP"])][soc] += w
            if r["INDP"].strip() and r["NAICSP"].strip():
                ind_naics_w[int(r["INDP"])][r["NAICSP"].strip()] += w
            band = 0 if age < 25 else (1 if age < 65 else 2)
            holders[(sex, band, lv)][soc] += w
            naics = r["NAICSP"].strip()
            if naics in naics_title:
                industry[soc][naics] += w
            c = cow[soc]
            c[0] += w
            if r["COW"] in ("6", "7"):
                c[1] += w
            elif r["COW"] in ("3", "4", "5"):
                c[2] += w
            if 25 <= age <= 64 and r["WKHP"] and int(r["WKHP"]) >= 35 and r["WKWN"] and int(r["WKWN"]) >= 50:
                wage = int(r["WAGP"] or 0)
                if wage > 0:
                    wages[soc].append((math.log(wage), w))
            if rows % 1_000_000 == 0:
                print(f"  {rows:,} rows", file=sys.stderr)
    socs = sorted(s for s in cow if cow[s][0] > 0)
    soc_index = {s: i for i, s in enumerate(socs)}
    naics_list = sorted({n for s in socs for n in industry[s]})
    naics_index = {n: i for i, n in enumerate(naics_list)}
    n_titles = 0
    # The indexes: titles by occupation, lines by industry.
    occ_to_soc = {o: max(v.items(), key=lambda x: x[1])[0] for o, v in occ_soc.items()}
    ind_naics = {i: max(v.items(), key=lambda x: x[1])[0] for i, v in ind_naics_w.items()}
    # How often each index title is written in: the ACS 2019 public-use
    # sample of write-ins (10,449 records; exact matches, ignoring case).
    index = xlsx_rows(os.path.join(INDEX, "Census-2022-Occupation-Index_Final.xlsx"))[6:]
    known = {(int(r[2]), " ".join(r[0].lower().split())) for r in index if len(r) >= 3 and r[2].strip().isdigit()}
    written = defaultdict(int)  # (soc, title lower) -> count
    for r in xlsx_rows_sheet(os.path.join(INDEX, "pub-io-write-ins-acs2019.xlsx"), "PublicUse")[3:]:
        if len(r) >= 3 and r[1].strip().isdigit():
            k = (int(r[1]), " ".join(r[2].lower().split()))
            if k in known and int(r[1]) in occ_to_soc:
                written[(occ_to_soc[int(r[1])], k[1])] += 1
    titles = defaultdict(dict)  # soc -> title -> restriction
    for r in index:
        if len(r) < 3 or not r[2].strip().isdigit():
            continue
        soc = occ_to_soc.get(int(r[2]))
        if soc is None:
            continue
        t = clean_index_title(r[0], written.get((soc, " ".join(r[0].lower().split())), 0) >= 2)
        if t is None:
            continue
        res = restriction(r[1] if len(r) > 1 else "", ind_naics, naics_index)
        # A title listed several times: the union of its restrictions (any
        # if one is unrestricted).
        old = titles[soc].get(t)
        titles[soc][t] = res if old is None else ([] if not old or not res else sorted(set(old) | set(res)))
    lines = defaultdict(dict)  # naics -> line -> kind
    for r in xlsx_rows(os.path.join(INDEX, "Census-2022-Industry-Index_Final.xlsx"))[6:]:
        if len(r) < 2 or not r[1].strip().isdigit():
            continue
        naics = ind_naics.get(int(r[1]))
        c = clean_line(r[0])
        if naics is None or c is None:
            continue
        lines[naics].setdefault(c[0], c[1])
    out = bytearray(b"INTWORK1")
    out.append(2)
    out += leb(len(naics_list))
    n_lines = 0
    for n in naics_list:
        out += text(n) + text(title_case(naics_title[n].replace(" And ", " and ")))
        ls = sorted(lines[n].items())
        n_lines += len(ls)
        out += leb(len(ls))
        for line, kind in ls:
            out += text(line)
            out.append(kind)
    out += leb(len(socs))
    for s in socs:
        med, sd = weighted_median_sd(wages[s])
        c = cow[s]
        # The top industries, as shares of all the occupation's workers
        # (the rest unlisted: a draw among the listed is proportional).
        inds = sorted(industry[s].items(), key=lambda x: -x[1])[:8]
        tot = sum(industry[s].values()) or 1
        out += text(s) + text(soc_title[s]) + struct.pack("<ffff", math.exp(med) if med else 0.0, sd, c[1] / c[0], c[2] / c[0])
        out.append(len(inds))
        for n, v in inds:
            out += leb(naics_index[n]) + struct.pack("<H", round(65535 * v / tot))
        # Titles: the index's (variety, weight 1 each), plus the written-in
        # ones by frequency (twice the variety's mass), plus the
        # occupation's own name made singular (a third of the total).
        own = own_titles(soc_title[s])
        ts = sorted(drop_inverted(titles[s]).items())
        v = len(ts)
        m = sum(written.get((s, t.lower()), 0) for t, _ in ts)
        per = 2 * v / m if m else 0.0
        weight = {t: 1.0 + per * written.get((s, t.lower()), 0) for t, _ in ts}
        mass = sum(weight.values()) or 2.0
        for t in own:
            weight[t] = weight.get(t, 0.0) + mass / 2 / len(own)
        entries = [(t, weight[t], []) for t in own if t not in titles[s]] + [(t, weight[t], res) for t, res in ts]
        if not entries:
            entries = [(title_case(soc_title[s]), 1.0, [])]
        top = max(e[1] for e in entries)
        n_titles += len(entries)
        out += leb(len(entries))
        for t, wt, res in entries:
            out += text(t)
            out += struct.pack("<H", max(1, round(wt * min(1.0, 65535 / top))))
            out += leb(len(res))
            for i in res:
                out += leb(i)
    for sex in range(2):
        for band in range(3):
            for lv in range(8):
                t = holders[(sex, band, lv)]
                items = sorted((soc_index[s], v) for s, v in t.items() if v > 0)
                out += leb(len(items))
                for i, v in items:
                    out += leb(i) + struct.pack("<I", v)
    for sex in range(2):
        for b5 in range(13):
            for lv in range(8):
                e, u, n = status[(sex, b5, lv)]
                out += struct.pack("<ff", e / n if n else 0.0, u / n if n else 0.0)
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    with open(OUT, "wb") as f:
        f.write(out)
    print(f"{rows:,} person records; {len(socs)} occupations ({n_titles} titles), {len(naics_list)} industries ({n_lines} lines); {len(out) / 1e3:.0f} KB", file=sys.stderr)


if __name__ == "__main__":
    main()
