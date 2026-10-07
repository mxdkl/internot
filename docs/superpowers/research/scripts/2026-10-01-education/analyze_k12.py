# Summaries of the CCD 2024-25 public school universe, joined to EDGE 2024-25
# geocodes, plus nearest-school catchments over 2020 tract population centres.
import csv, collections, math
import numpy as np
from scipy.spatial import cKDTree

E = "/home/player1/internot/datasets/education/"
def num(s):
    try: return float(s)
    except: return None

d = {}
for r in csv.DictReader(open(E+"ccd/ccd_sch_029_2425_w_1a_073025.csv", encoding="latin-1")):
    if int(r["FIPST"]) > 56: continue
    d[r["NCESSCH"]] = r
virt = {r["NCESSCH"]: r["VIRTUAL"] for r in csv.DictReader(open(E+"ccd/ccd_sch_129_2425_w_1a_073025.csv", encoding="latin-1"))}
teach = {}
for r in csv.DictReader(open(E+"ccd/ccd_sch_059_2425_l_1a_073025.csv", encoding="latin-1")):
    if r["TOTAL_INDICATOR"] == "Education Unit Total":
        v = num(r["TEACHERS"]);
        if v is not None and v > 0: teach[r["NCESSCH"]] = v
tot = {}
for r in csv.DictReader(open(E+"ccd/derived_membership_total_2425.csv")):
    v = num(r["STUDENT_COUNT"])
    if v is not None and v > 0: tot[r["NCESSCH"]] = v
grade = collections.defaultdict(dict)
for r in csv.DictReader(open(E+"ccd/derived_membership_by_grade_2425.csv")):
    v = num(r["STUDENT_COUNT"])
    if v is not None and v > 0: grade[r["NCESSCH"]][r["GRADE"]] = v
loc = {}
for line in open(E+"edge/EDGE_GEOCODE_PUBLICSCH_2425.TXT", encoding="latin-1"):
    f = line.rstrip("\r\n").split("|")
    loc[f[0]] = (f[11], float(f[12]), float(f[13]))

OPEN = {"1", "3", "4", "5", "8"}
ops = [k for k, r in d.items() if r["SY_STATUS"] in OPEN and k in tot]
print("CCD 2024-25, 50 states + DC: directory rows", len(d), "; operating with membership > 0:", len(ops), "; students", int(sum(tot[k] for k in ops)))
c = collections.Counter(); s = collections.Counter()
for k in ops:
    c[d[k]["SCH_TYPE_TEXT"]] += 1; s[d[k]["SCH_TYPE_TEXT"]] += tot[k]
print("By type (schools, students):", [(t, c[t], int(s[t])) for t in c])
ch = [k for k in ops if d[k]["CHARTER_TEXT"] == "Yes"]
vv = [k for k in ops if virt.get(k, "").startswith("FULLVIRTUAL") or virt.get(k) == "FACEVIRTUAL"]
print("Virtual codes:", collections.Counter(virt.get(k, "?") for k in ops))
print("Charter: schools", len(ch), "students", int(sum(tot[k] for k in ch)), "share of students %.1f%%" % (100*sum(tot[k] for k in ch)/sum(tot[k] for k in ops)))
fullv = [k for k in ops if virt.get(k) == "FULLVIRTUAL"]
print("Full virtual: schools", len(fullv), "students", int(sum(tot[k] for k in fullv)))

reg = [k for k in ops if d[k]["SCH_TYPE"] == "1" and virt.get(k) != "FULLVIRTUAL"]
print("\nRegular, not fully virtual:", len(reg), "students", int(sum(tot[k] for k in reg)))
def pct(a, ps=(10, 25, 50, 75, 90)):
    a = np.sort(np.asarray(a, float)); return [round(float(np.percentile(a, p))) for p in ps]
def wmedian(a):
    a = np.sort(np.asarray(a, float)); cw = np.cumsum(a); return float(a[np.searchsorted(cw, cw[-1]/2)])
print("By LEVEL: n, students, mean, p10/p25/p50/p75/p90, student-weighted median")
for lev in ["Elementary", "Middle", "High", "Other", "Prekindergarten", "Secondary", "Not reported", "Not applicable", "Ungraded"]:
    a = [tot[k] for k in reg if d[k]["LEVEL"] == lev]
    if a: print(f"  {lev:16s} {len(a):6d} {int(sum(a)):9d} {np.mean(a):6.0f} {pct(a)} {wmedian(a):.0f}")
a = [tot[k] for k in reg]; print(f"  {'All regular':16s} {len(a):6d} {int(sum(a)):9d} {np.mean(a):6.0f} {pct(a)} {wmedian(a):.0f}")

print("\nGrade spans (GSLO-GSHI), regular schools: top 20 by count: span, schools, % schools, % students, median enrollment")
sp = collections.defaultdict(list)
for k in reg: sp[d[k]["GSLO"] + "-" + d[k]["GSHI"]].append(tot[k])
T = sum(tot[k] for k in reg)
for key, a in sorted(sp.items(), key=lambda x: -len(x[1]))[:20]:
    print(f"  {key:8s} {len(a):6d} {100*len(a)/len(reg):5.1f} {100*sum(a)/T:5.1f} {np.median(a):6.0f}")

GR = ["Kindergarten"] + ["Grade %d" % i for i in range(1, 13)]
print("\nStudents per grade per school (regular schools enrolling that grade): grade, schools, students, p10/p25/p50/p75/p90, student-weighted median")
for g in ["Kindergarten", "Grade 1", "Grade 3", "Grade 5", "Grade 6", "Grade 7", "Grade 9", "Grade 10", "Grade 12"]:
    a = [grade[k][g] for k in reg if g in grade[k]]
    print(f"  {g:12s} {len(a):6d} {int(sum(a)):8d} {pct(a)} {wmedian(a):.0f}")

print("\nLocale groups (EDGE LOCALE first digit): median enrollment and schools, regular")
LG = {"1": "City", "2": "Suburb", "3": "Town", "4": "Rural"}
for lev in ["Elementary", "Middle", "High"]:
    row = []
    for g, name in LG.items():
        a = [tot[k] for k in reg if d[k]["LEVEL"] == lev and k in loc and loc[k][0][:1] == g]
        row.append(f"{name} n={len(a)} med={np.median(a):.0f} mean={np.mean(a):.0f}")
    print(" ", lev, "; ".join(row))
print("Share of regular schools / students by locale:", [(name, round(100*sum(1 for k in reg if k in loc and loc[k][0][:1]==g)/len(reg),1), round(100*sum(tot[k] for k in reg if k in loc and loc[k][0][:1]==g)/T,1)) for g, name in LG.items()])

print("\nPupil/teacher ratio (sum students / sum FTE teachers), schools with both")
for lev in ["Elementary", "Middle", "High", "Other"]:
    ks = [k for k in reg if d[k]["LEVEL"] == lev and k in teach]
    print(f"  {lev:10s} {sum(tot[k] for k in ks)/sum(teach[k] for k in ks):.1f}  (schools {len(ks)})")
ks = [k for k in ops if k in teach]; print(f"  all operating {sum(tot[k] for k in ks)/sum(teach[k] for k in ks):.1f}")

# Nearest-school catchments over 2020 tract population centres.
tr = []
for r in csv.DictReader(open("/home/player1/internot/datasets/geo/cenpop/CenPop2020_Mean_TR.txt", encoding="utf-8-sig")):
    if int(r["STATEFP"]) > 56: continue
    p = int(r["POPULATION"]);
    if p > 0: tr.append((p, float(r["LATITUDE"]), float(r["LONGITUDE"])))
tp = np.array([t[0] for t in tr], float)
def xyz(lat, lon):
    lat = np.radians(lat); lon = np.radians(lon)
    return np.c_[np.cos(lat)*np.cos(lon), np.cos(lat)*np.sin(lon), np.sin(lat)]
TX = xyz(np.array([t[1] for t in tr]), np.array([t[2] for t in tr]))
R = 3958.8
print("\nNearest-school catchments: %d populated tracts, population %d" % (len(tr), tp.sum()))
for g in ["Grade 3", "Grade 7", "Grade 10"]:
    ks = [k for k in reg if g in grade[k] and k in loc]
    SX = xyz(np.array([loc[k][1] for k in ks]), np.array([loc[k][2] for k in ks]))
    dist, idx = cKDTree(SX).query(TX)
    miles = 2*R*np.arcsin(np.clip(dist/2, 0, 1))
    o = np.argsort(miles); cw = np.cumsum(tp[o]) / tp.sum()
    q = lambda p: miles[o][np.searchsorted(cw, p)]
    print(f"  {g}: schools {len(ks)}; pop-weighted distance tract centre -> nearest school, miles: p25 {q(.25):.2f} p50 {q(.5):.2f} p75 {q(.75):.2f} p90 {q(.9):.2f}")
    within = lambda m: tp[miles <= m].sum()/tp.sum()*100
    print(f"     share of population within 1 / 2 / 5 / 10 mi: {within(1):.1f} / {within(2):.1f} / {within(5):.1f} / {within(10):.1f}%")
    ntr = np.bincount(idx, minlength=len(ks)); npop = np.bincount(idx, weights=tp, minlength=len(ks))
    genr = np.array([grade[k][g] for k in ks])
    print(f"     schools nearest to no tract centre: {100*(ntr==0).mean():.1f}% of schools, {100*genr[ntr==0].sum()/genr.sum():.1f}% of {g} students")
    has = ntr > 0
    print(f"     tracts per school (schools with >=1): p10/p25/p50/p75/p90 {pct(ntr[has])}; population per such catchment p10/p50/p90 {pct(npop[has], (10,50,90))}")
    ratio = npop[has] / genr[has]
    print(f"     catchment population per enrolled {g} student: p10/p50/p90 {[round(float(x),0) for x in np.percentile(ratio,(10,50,90))]}; national ratio {tp.sum()/genr.sum():.0f}")
