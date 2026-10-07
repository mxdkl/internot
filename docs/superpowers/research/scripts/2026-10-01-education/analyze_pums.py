# Education targets from ACS 2023 1-year PUMS (person weights PWGTP).
import numpy as np, pandas as pd
df = pd.read_parquet("pums_edu.parquet")
df = df[df["STATE"] <= 56]
w = "PWGTP"
YEARS = {1: 0, 2: 0, 3: 0, 4: 1, 5: 2, 6: 3, 7: 4, 8: 5, 9: 6, 10: 7, 11: 8, 12: 9, 13: 10, 14: 11, 15: 12, 16: 12, 17: 12, 18: 12.5, 19: 13.5, 20: 14, 21: 16, 22: 18, 23: 19, 24: 20}
def level(s):
    return np.select([s <= 15, s <= 17, s <= 20, s == 21], [0, 1, 2, 3], 4)
LV = ["<HS", "HS/GED", "Some coll/assoc", "BA", "Graduate"]
df["yrs"] = df["SCHL"].map(YEARS); df["lev"] = level(df["SCHL"].fillna(0).values)
df["race"] = np.where(df["HISP"] > 1, "Hispanic", df["RAC1P"].map({1: "White", 2: "Black", 3: "AIAN", 4: "AIAN", 5: "AIAN", 6: "API", 7: "API", 8: "Other/multi", 9: "Other/multi"}))
df["enr"] = df["SCH"].isin([2, 3])
def wm(m, sub): return 100 * sub.loc[m, w].sum() / sub[w].sum()

print("== 1. Enrolled (SCH = 2 public or 3 private/home), % by age, ACS 2023 (school attended in the last 3 months)")
for lo, hi in [(3, 4), (5, 6), (7, 13), (14, 17), (18, 19), (20, 24), (25, 29), (30, 34)]:
    s = df[(df.AGEP >= lo) & (df.AGEP <= hi)]
    row = f"  {lo}-{hi}: all {wm(s.enr, s):.1f}  male {wm(s.enr[s.SEX==1], s[s.SEX==1]):.1f}  female {wm(s.enr[s.SEX==2], s[s.SEX==2]):.1f}"
    for r in ["White", "Black", "Hispanic", "API", "AIAN"]:
        sr = s[s.race == r]; row += f"  {r} {wm(sr.enr, sr):.1f}"
    print(row)
print("  single years:", ", ".join(f"{a}: {wm(df[df.AGEP==a].enr, df[df.AGEP==a]):.1f}" for a in range(3, 31)))

print("\n== 2. Grade attended by age at interview (all persons of that age, %): preschool P, K, grades 1-12, UG = undergraduate, G = graduate, - = not enrolled")
gl = {1: "P", 2: "K", **{i: str(i - 2) for i in range(3, 15)}, 15: "UG", 16: "G"}
for a in range(3, 23):
    s = df[df.AGEP == a]; tot = s[w].sum()
    dist = s.groupby(s["SCHG"].fillna(0))[w].sum() / tot * 100
    items = sorted(((gl.get(int(k), "-"), v) for k, v in dist.items() if v >= 1.0), key=lambda x: -x[1])
    print(f"  age {a:2d}: " + ", ".join(f"{g} {v:.1f}" for g, v in items))
print("  (Interviews run all year, so age at interview vs grade mixes two school-year ages; the modal pair is the target.)")

print("\n== 3. Private (SCH = 3; includes home school) as % of enrolled, by grade")
k12 = df[df.SCHG.between(1, 16)]
for g in [1, 2, 3, 7, 11, 13, 14, 15, 16]:
    s = k12[k12.SCHG == g]; print(f"  {gl[g]:>3s}: {wm(s.SCH == 3, s):.1f}", end="")
s = k12[k12.SCHG.between(2, 14)]; print(f"\n  K-12 together: {wm(s.SCH == 3, s):.1f}%")

print("\n== 4. Highest level by birth cohort (age 25+; birth year ~ 2023 - age), % ; mean years; GED share of HS-only")
ad = df[df.AGEP >= 25].copy(); ad["by"] = 2023 - ad.AGEP
cuts = list(range(1928, 1999, 5))
ad["coh"] = pd.cut(ad["by"], bins=cuts, right=False)
for sex, nm in [(1, "Men"), (2, "Women")]:
    print(f"  {nm}: cohort  " + "  ".join(f"{l:>15s}" for l in LV) + "   mean yrs  GED/(HS+GED)")
    for c, s in ad[ad.SEX == sex].groupby("coh", observed=True):
        sh = s.groupby("lev")[w].sum() / s[w].sum() * 100
        ged = s.loc[s.SCHL == 17, w].sum() / s.loc[s.SCHL.isin([16, 17]), w].sum() * 100
        print(f"    {c.left}-{c.right-1}: " + "  ".join(f"{sh.get(i, 0):15.1f}" for i in range(5)) + f"   {np.average(s.yrs, weights=s[w]):6.2f}   {ged:5.1f}")
print("  By heritage, both sexes, BA+ % (and <HS %) for cohorts:")
for c in [pd.Interval(1943, 1948, closed="left"), pd.Interval(1958, 1963, closed="left"), pd.Interval(1973, 1978, closed="left"), pd.Interval(1988, 1993, closed="left")]:
    s = ad[ad.coh == c]; row = f"    {c.left}-{c.right-1}:"
    for r in ["White", "Black", "Hispanic", "API", "AIAN", "Other/multi"]:
        sr = s[s.race == r]; row += f"  {r} {wm(sr.lev >= 3, sr):.1f} ({wm(sr.lev == 0, sr):.1f})"
    print(row)
print("  Native vs foreign-born, ages 25-64, level %:")
for nat, nm in [(1, "native"), (2, "foreign-born")]:
    s = ad[(ad.AGEP <= 64) & (ad.NATIVITY == nat)]; sh = s.groupby("lev")[w].sum() / s[w].sum() * 100
    print(f"    {nm}: " + ", ".join(f"{LV[i]} {sh.get(i,0):.1f}" for i in range(5)))

print("\n== 5. Couples (householder + spouse/partner, both 25+): education matching")
ref = df[df.RELSHIPP == 20].set_index("SERIALNO"); par = df[df.RELSHIPP.isin([21, 22, 23, 24])].set_index("SERIALNO")
c = ref.join(par, lsuffix="_r", rsuffix="_p", how="inner")
c = c[(c.AGEP_r >= 25) & (c.AGEP_p >= 25)]
os_ = c[c.RELSHIPP_p.isin([21, 22])].copy()
male_r = os_.SEX_r == 1
for col in ["AGEP", "yrs", "lev", "race", "STATE", "SCHL"]:
    os_[col + "_h"] = np.where(male_r, os_[col + "_r"], os_[col + "_p"]); os_[col + "_w"] = np.where(male_r, os_[col + "_p"], os_[col + "_r"])
os_["wt"] = os_["PWGTP_r"]
def wcorr(x, y, wt):
    mx, my = np.average(x, weights=wt), np.average(y, weights=wt)
    return np.average((x - mx) * (y - my), weights=wt) / np.sqrt(np.average((x - mx) ** 2, weights=wt) * np.average((y - my) ** 2, weights=wt))
def free(sub, keys):
    # Within-stratum independent pairing (U-statistic: excludes each couple's own pairing).
    same_num = 0.0; hw_num = 0.0; W = 0.0
    for _, s in sub.groupby(keys, observed=True):
        wt = s.wt.values.astype(float); S = wt.sum(); S2 = (wt ** 2).sum()
        if len(s) < 2: continue
        den = S * S - S2
        same = sum(wt[s.lev_h.values == k].sum() * wt[s.lev_w.values == k].sum() for k in range(5)) - (wt ** 2 * (s.lev_h.values == s.lev_w.values)).sum()
        hw = (wt * s.yrs_h.values).sum() * (wt * s.yrs_w.values).sum() - (wt ** 2 * s.yrs_h.values * s.yrs_w.values).sum()
        same_num += S * same / den; hw_num += S * hw / den; W += S
    wt = sub.wt.values
    mh, mw = np.average(sub.yrs_h, weights=wt), np.average(sub.yrs_w, weights=wt)
    sh, sw = np.sqrt(np.average((sub.yrs_h - mh) ** 2, weights=wt)), np.sqrt(np.average((sub.yrs_w - mw) ** 2, weights=wt))
    return 100 * same_num / W, (hw_num / W - mh * mw) / (sh * sw)
os_["ab_h"] = (os_.AGEP_h // 5) * 5; os_["ab_w"] = (os_.AGEP_w // 5) * 5; os_["ab10"] = (os_.AGEP_h // 10) * 10
print("  opposite-sex couples, by man's age: n, corr(years), same level %, wife higher %, husband higher %; free under independence: national, by ages, by ages+state+heritage pair")
for lo, hi in [(25, 34), (35, 44), (45, 54), (55, 64), (65, 74), (75, 99), (25, 99)]:
    for kind, rel in [("married", [21]), ("cohabiting", [22])]:
        s = os_[(os_.AGEP_h >= lo) & (os_.AGEP_h <= hi) & os_.RELSHIPP_p.isin(rel)]
        if len(s) < 200: continue
        wt = s.wt
        same = 100 * wt[s.lev_h == s.lev_w].sum() / wt.sum(); wh = 100 * wt[s.lev_w > s.lev_h].sum() / wt.sum(); hh = 100 * wt[s.lev_h > s.lev_w].sum() / wt.sum()
        s = s.assign(one=1)
        f0 = free(s, ["one"]); f1 = free(s, ["ab_h", "ab_w"]); f2 = free(s, ["ab10", "STATE_h", "race_h", "race_w"])
        print(f"    {lo}-{hi} {kind:10s} n={len(s):6d} corr {wcorr(s.yrs_h, s.yrs_w, wt):.3f} same {same:.1f} wife> {wh:.1f} husb> {hh:.1f} | free: none {f0[0]:.1f}/{f0[1]:.3f}, ages {f1[0]:.1f}/{f1[1]:.3f}, ages+state+heritage {f2[0]:.1f}/{f2[1]:.3f}")
s = os_[os_.RELSHIPP_p == 21]; wt = s.wt
print("  married couples 25+, husband level (rows) x wife level (cols), % of couples:")
tab = pd.crosstab(s.lev_h, s.lev_w, values=wt, aggfunc="sum", normalize=True) * 100
print("    " + "  ".join(f"{l:>15s}" for l in LV))
for i in range(5): print(f"    {LV[i]:>15s} " + "  ".join(f"{tab.loc[i, j]:15.1f}" for j in range(5)))
ss = c[c.RELSHIPP_p.isin([23, 24])]
print(f"  same-sex couples 25+: n={len(ss)}, corr {wcorr(ss.yrs_r, ss.yrs_p, ss.PWGTP_r):.3f}, same level {100*ss.PWGTP_r[ss.lev_r==ss.lev_p].sum()/ss.PWGTP_r.sum():.1f}%")

print("\n== 6. Where enrolled undergraduates (SCHG 15) aged 18-24 live, %")
u = df[(df.SCHG == 15) & df.AGEP.between(18, 24)]
cat = np.select([u.RELSHIPP == 38, u.RELSHIPP.isin([25, 26, 27, 35]), u.RELSHIPP.isin([20, 21, 22, 23, 24]), u.RELSHIPP.isin([34, 36])], ["group quarters (dorm proxy)", "child of householder (with parents)", "own household (householder or partner)", "roommate/other nonrelative"], "other relative / institutional")
for sch, nm in [(None, "all"), (2, "public"), (3, "private")]:
    m = np.ones(len(u), bool) if sch is None else (u.SCH == sch).values
    t = pd.Series(u[w].values[m]).groupby(cat[m]).sum(); t = 100 * t / t.sum()
    print(f"  {nm}: " + "; ".join(f"{k} {v:.1f}" for k, v in t.sort_values(ascending=False).items()))
for a in range(18, 25):
    s = u[u.AGEP == a]; print(f"  age {a}: GQ {wm(s.RELSHIPP == 38, s):.1f}, with parents {wm(s.RELSHIPP.isin([25,26,27,35]), s):.1f}")
print("  Total enrolled undergraduates (weighted), all ages: %.0f; in noninstitutional GQ: %.0f" % (df.loc[df.SCHG == 15, w].sum(), df.loc[(df.SCHG == 15) & (df.RELSHIPP == 38), w].sum()))
