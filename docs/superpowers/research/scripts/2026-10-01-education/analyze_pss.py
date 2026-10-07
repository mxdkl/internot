# PSS 2023-24 public-use file, weighted by PFNLWT (the survey's final school weight).
import csv, numpy as np, collections
rows = list(csv.DictReader(open("/home/player1/internot/datasets/education/pss/pss2324_pu.csv", encoding="latin-1")))
w = np.array([float(r["PFNLWT"]) for r in rows]); n = np.array([float(r["NUMSTUDS"]) for r in rows])
print("PSS 2023-24 rows", len(rows), "weighted schools %.0f" % w.sum(), "weighted students %.0f" % (w*n).sum())
print("lat/lon present:", sum(1 for r in rows if r["LATITUDE24"] not in ("", "-1")))
def wq(x, wt, ps):
    o = np.argsort(x); cw = np.cumsum(wt[o]) / wt.sum(); return [float(x[o][np.searchsorted(cw, p)]) for p in ps]
print("Enrollment per school, weighted p10/p25/p50/p75/p90:", wq(n, w, (.1, .25, .5, .75, .9)), "mean %.0f" % ((w*n).sum()/w.sum()))
print("Student-weighted median school size:", wq(n, w*n, (.5,)))
for cut in (25, 50, 100, 300, 500):
    m = n < cut; print(f"  schools < {cut}: {100*w[m].sum()/w.sum():.1f}% of schools, {100*(w*n)[m].sum()/(w*n).sum():.1f}% of students")
for var, lab in [("RELIG", {"1": "Catholic", "2": "Other religious", "3": "Nonsectarian"}), ("LEVEL", {"1": "Elementary", "2": "Secondary", "3": "Combined"}),
                 ("TYPOLOGY", {"1": "Catholic parochial", "2": "Catholic diocesan", "3": "Catholic private", "4": "Conservative Christian", "5": "Other religious, affiliated", "6": "Other religious, unaffiliated", "7": "Nonsectarian regular", "8": "Nonsectarian special program", "9": "Nonsectarian special education"})]:
    print(var)
    for code, name in lab.items():
        m = np.array([r[var] == code for r in rows])
        if m.any(): print(f"  {name:32s} schools {w[m].sum():8.0f} ({100*w[m].sum()/w.sum():4.1f}%)  students {(w*n)[m].sum():9.0f} ({100*(w*n)[m].sum()/(w*n).sum():4.1f}%)  mean size {(w*n)[m].sum()/w[m].sum():5.0f}")
print("Locale (ULOCALE24 first digit): % schools / % students")
for g, name in {"1": "City", "2": "Suburb", "3": "Town", "4": "Rural"}.items():
    m = np.array([r["ULOCALE24"][:1] == g for r in rows])
    print(f"  {name}: {100*w[m].sum()/w.sum():.1f} / {100*(w*n)[m].sum()/(w*n).sum():.1f}")
