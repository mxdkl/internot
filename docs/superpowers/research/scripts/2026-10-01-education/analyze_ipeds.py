# IPEDS: HD2024 directory, EF2023A fall enrollment, EF2023B age, EF2022C residence
# of first-time students, IC2023 dorm capacity; plus distance from 2020 tract
# population centres to the nearest institution.
import csv, collections, numpy as np
from scipy.spatial import cKDTree
I = "/home/player1/internot/datasets/education/ipeds/"
def R(path):
    for r in csv.DictReader(open(path, encoding="utf-8-sig", errors="replace")):
        yield {k.strip(): (v or "").strip() for k, v in r.items()}
def num(s):
    try: return float(s)
    except: return 0.0
# public K-12 (no PK) from CCD per-grade file, for the private share
g = collections.Counter()
for r in csv.DictReader(open("/home/player1/internot/datasets/education/ccd/derived_membership_by_grade_2425.csv")):
    if r["NCESSCH"][:2] <= "56": g[r["GRADE"]] += max(num(r["STUDENT_COUNT"]), 0)
k12 = sum(v for k, v in g.items() if k not in ("Pre-Kindergarten", "Adult Education", "Not Specified", "Grade 13"))
print("CCD 2024-25 public K-12 + ungraded (no PK), 50 states + DC: %.0f; PK %.0f" % (k12, g["Pre-Kindergarten"]))
print("Private share with PSS 2023-24 weighted 5,096,365: %.1f%%" % (100*5096365/(5096365+k12)))

hd = {r["UNITID"]: r for r in R(I+"HD2024.csv")}
SECT = {"1": "Public 4-year+", "2": "Private nonprofit 4-year+", "3": "Private for-profit 4-year+", "4": "Public 2-year", "5": "Private nonprofit 2-year", "6": "Private for-profit 2-year", "7": "Public <2-year", "8": "Private nonprofit <2-year", "9": "Private for-profit <2-year", "0": "Administrative unit", "99": "Unknown"}
us = {k: r for k, r in hd.items() if 0 < int(r["FIPS"]) <= 56}
print("\nHD2024 rows", len(hd), "; 50 states + DC", len(us))
dg = {k: r for k, r in us.items() if r["DEGGRANT"] == "1" and r["PSET4FLG"] == "1" and r["CYACTIVE"] == "1"}
print("Active, degree-granting, Title IV (the Digest universe):", len(dg), dict(collections.Counter(SECT[r["SECTOR"]] for r in dg.values())))
print("HBCU:", sum(1 for r in dg.values() if r["HBCU"] == "1"), "; tribal:", sum(1 for r in dg.values() if r["TRIBAL"] == "1"), "; land-grant (LANDGRNT=1):", sum(1 for r in us.values() if r["LANDGRNT"] == "1"))
print("With lat/lon:", sum(1 for r in dg.values() if r["LATITUDE"] not in ("", "-2")))

ef = collections.defaultdict(dict)
for r in R(I+"ef2023a.csv"):
    ef[r["UNITID"]][r["EFALEVEL"]] = num(r["EFTOTLT"])
tot = {k: ef[k].get("1", 0) for k in dg}; ug = {k: ef[k].get("2", 0) for k in dg}; ft = {k: ef[k].get("4", 0) for k in dg}
print("\nFall 2023, degree-granting Title IV, 50+DC: total %.0f, undergraduate %.0f, graduate %.0f, first-time degree-seeking %.0f" % (sum(tot.values()), sum(ug.values()), sum(ef[k].get("12", 0) for k in dg), sum(ft.values())))
bs = collections.defaultdict(float); bn = collections.Counter()
for k in dg: bs[SECT[dg[k]["SECTOR"]]] += ug[k]; bn[SECT[dg[k]["SECTOR"]]] += 1
U = sum(ug.values())
for s in bs: print(f"  {s:28s} institutions {bn[s]:5d}  undergrads {bs[s]:10.0f} ({100*bs[s]/U:4.1f}%)  mean UG {bs[s]/bn[s]:6.0f}")
a = np.array([tot[k] for k in dg if tot[k] > 0])
print("Total enrollment per institution p10/p25/p50/p75/p90:", [round(float(np.percentile(a, p))) for p in (10, 25, 50, 75, 90)])
bins = [(0, 200), (200, 1000), (1000, 3000), (3000, 10000), (10000, 20000), (20000, 10**9)]
for lo, hi in bins:
    m = (a >= lo) & (a < hi); print(f"  enrollment {lo}-{hi}: {100*m.mean():.1f}% of institutions, {100*a[m].sum()/a.sum():.1f}% of students")

# Age of undergraduates (EF2023B; LSTUDY 2 = undergraduate; EFAGE09 = grand total)
age = collections.Counter()
for r in R(I+"ef2023b.csv"):
    if r["UNITID"] in dg and r["LSTUDY"] == "2": age[r["EFBAGE"]] += num(r["EFAGE09"])
lab = {"3": "<18", "4": "18-19", "5": "20-21", "6": "22-24", "8": "25-29", "9": "30-34", "10": "35-39", "11": "40-49", "12": "50-64", "13": "65+", "14": "unknown"}
T = age["1"]
print("\nUndergraduate age, fall 2023 (EF2023B):", ", ".join(f"{v} {100*age[k]/T:.1f}%" for k, v in lab.items()), "; under 25 %.1f%%" % (100*age["2"]/T))
for sec in ("1", "2", "4"):
    a2 = collections.Counter()
    for r in R(I+"ef2023b.csv"):
        if r["UNITID"] in dg and dg[r["UNITID"]]["SECTOR"] == sec and r["LSTUDY"] == "2": a2[r["EFBAGE"]] += num(r["EFAGE09"])
    print(f"  {SECT[sec]}: under 25 {100*a2['2']/a2['1']:.1f}%, 18-21 {100*(a2['4']+a2['5'])/a2['1']:.1f}%")

# Residence of first-time students (EF2022C): in-state share
res = collections.defaultdict(lambda: collections.Counter())
for r in R(I+"ef2022c.csv"):
    if r["UNITID"] not in dg: continue
    st = int(r["EFCSTATE"]); inst = int(dg[r["UNITID"]]["FIPS"]); sec = dg[r["UNITID"]]["SECTOR"]
    for var in ("EFRES01", "EFRES02"):
        v = num(r[var])
        if st == 99: res[(sec, var)]["total"] += v
        elif st == inst: res[(sec, var)]["in"] += v
        elif st == 90: res[(sec, var)]["foreign"] += v
        elif st in (57, 98): res[(sec, var)]["unknown"] += v
        elif st <= 56 or st in (60, 64, 66, 68, 69, 70, 72, 78): res[(sec, var)]["other_state"] += v
print("\nFall 2022 first-time degree/certificate-seeking undergraduates (EF2022C), share by residence:")
for var, name in (("EFRES01", "all first-time"), ("EFRES02", "graduated HS in past 12 months")):
    agg = collections.Counter()
    for (sec, v), c in res.items():
        if v == var: agg.update(c)
    print(f"  {name}: total {agg['total']:.0f}; in-state {100*agg['in']/agg['total']:.1f}%, other state {100*agg['other_state']/agg['total']:.1f}%, foreign {100*agg['foreign']/agg['total']:.1f}%, unknown {100*agg['unknown']/agg['total']:.1f}%")
    for sec in ("1", "2", "3", "4"):
        c = res[(sec, var)]
        if c["total"]: print(f"     {SECT[sec]}: in-state {100*c['in']/c['total']:.1f}%")

# Dorm capacity (IC2023 ROOMCAP), against fall 2023 enrollment
ic = {r["UNITID"]: r for r in R(I+"IC2023.csv")}
cap = collections.Counter(); enr = collections.Counter(); ftug = collections.Counter(); withh = collections.Counter(); nn = collections.Counter()
for k in dg:
    sec = dg[k]["SECTOR"]; r = ic.get(k)
    if not r: continue
    nn[sec] += 1
    if r["ROOM"] == "1": withh[sec] += 1; cap[sec] += max(num(r["ROOMCAP"]), 0)
    enr[sec] += tot[k]; ftug[sec] += ef[k].get("22", 0)
print("\nOn-campus housing capacity, IC2023 ROOMCAP (degree-granting Title IV, 50+DC):")
for sec in ("1", "2", "3", "4", "5", "6"):
    if nn[sec]: print(f"  {SECT[sec]:28s} with housing {100*withh[sec]/nn[sec]:5.1f}%  beds {cap[sec]:9.0f}  beds per student {cap[sec]/enr[sec]:.2f}  per FT undergrad {cap[sec]/max(ftug[sec],1):.2f}")
print("  total beds %.0f; total enrollment %.0f" % (sum(cap.values()), sum(enr.values())))

# Distance from tract population centres to the nearest institution
tr = []
for r in csv.DictReader(open("/home/player1/internot/datasets/geo/cenpop/CenPop2020_Mean_TR.txt", encoding="utf-8-sig")):
    if int(r["STATEFP"]) <= 56 and int(r["POPULATION"]) > 0: tr.append((int(r["POPULATION"]), float(r["LATITUDE"]), float(r["LONGITUDE"])))
tp = np.array([t[0] for t in tr], float)
def xyz(lat, lon):
    lat = np.radians(lat); lon = np.radians(lon); return np.c_[np.cos(lat)*np.cos(lon), np.cos(lat)*np.sin(lon), np.sin(lat)]
TX = xyz(np.array([t[1] for t in tr]), np.array([t[2] for t in tr]))
print("\nPopulation-weighted distance, 2020 tract centre -> nearest degree-granting institution (miles):")
for name, secs in (("any", {"1", "2", "3", "4", "5", "6"}), ("public 2-year", {"4"}), ("public 4-year", {"1"}), ("any public", {"1", "4"}), ("public or private nonprofit 4-year", {"1", "2"})):
    ks = [k for k in dg if dg[k]["SECTOR"] in secs and tot[k] > 0]
    SX = xyz(np.array([float(dg[k]["LATITUDE"]) for k in ks]), np.array([float(dg[k]["LONGITUD"]) for k in ks]))
    dist, _ = cKDTree(SX).query(TX); miles = 2*3958.8*np.arcsin(np.clip(dist/2, 0, 1))
    o = np.argsort(miles); cw = np.cumsum(tp[o])/tp.sum(); q = lambda p: miles[o][np.searchsorted(cw, p)]
    print(f"  {name:36s} n={len(ks):5d} p50 {q(.5):5.1f}  p75 {q(.75):5.1f}  p90 {q(.9):5.1f}; within 25 mi {100*tp[miles<=25].sum()/tp.sum():.1f}%, within 60 mi {100*tp[miles<=60].sum()/tp.sum():.1f}%")
