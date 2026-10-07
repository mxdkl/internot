# SABS 2015-16 attendance zones against 2020 block-group and tract population
# centres: how many people and tracts a zone holds, and how often tracts are
# split between zones.
import sys, glob, collections
import numpy as np, pandas as pd, geopandas as gpd
G = "/home/player1/internot/datasets/geo/cenpop/"
shp = sys.argv[1]
z = gpd.read_file(shp, engine="pyogrio")
print("SABS rows", len(z), "crs", z.crs)
print("Level counts:", z["level"].value_counts().to_dict())
print("Defacto:", z["defacto"].value_counts().to_dict(), "openEnroll:", z["openEnroll"].value_counts().to_dict(), "MultiBdy:", z["MultiBdy"].value_counts().to_dict())
def pts(fn, extra):
    df = pd.read_csv(G + fn, dtype=str, encoding="utf-8-sig")
    df["POPULATION"] = df["POPULATION"].astype(int)
    df = df[(df["STATEFP"].astype(int) <= 56) & (df["POPULATION"] > 0)].copy()
    df["TRACT"] = df["STATEFP"] + df["COUNTYFP"] + df["TRACTCE"]
    g = gpd.GeoDataFrame(df, geometry=gpd.points_from_xy(df["LONGITUDE"].astype(float), df["LATITUDE"].astype(float)), crs="EPSG:4269")
    return g.to_crs(z.crs)
bg = pts("CenPop2020_Mean_BG.txt", ["BLKGRPCE"])
tr = pts("CenPop2020_Mean_TR.txt", [])
P = bg["POPULATION"].sum()
print("2020 block groups", len(bg), "population", P)
z["zid"] = np.arange(len(z))
zz = z[["zid", "ncessch", "level", "defacto", "openEnroll", "MultiBdy", "stAbbrev", "geometry"]]
jb = gpd.sjoin(bg[["POPULATION", "TRACT", "geometry"]], zz, predicate="within", how="inner")
jt = gpd.sjoin(tr[["POPULATION", "TRACT", "geometry"]], zz, predicate="within", how="inner")
def q(a, ps=(10, 25, 50, 75, 90)): return [int(round(float(np.percentile(a, p)))) for p in ps]
for lev, name in (("1", "Primary"), ("2", "Middle"), ("3", "High")):
    zl = zz[zz["level"] == lev]
    real = zl[(zl["defacto"] == "0") & (zl["openEnroll"] != "1")]
    b = jb[jb["level"] == lev]
    cov = b.drop_duplicates(subset=["TRACT", "geometry"])["POPULATION"].sum() if False else bg.loc[bg.index.isin(b.index), "POPULATION"].sum()
    print(f"\n{name}: zones {len(zl)} (de facto {int((zl['defacto']=='1').sum())}, open enrollment {int((zl['openEnroll']=='1').sum())}, address-based in non-de-facto districts {len(real)})")
    print(f"  2020 population inside any {name} zone: {100*cov/P:.1f}%")
    br = b[b["zid"].isin(real["zid"])]
    pop = br.groupby("zid")["POPULATION"].sum().reindex(real["zid"], fill_value=0)
    nbg = br.groupby("zid").size().reindex(real["zid"], fill_value=0)
    t = jt[(jt["level"] == lev) & jt["zid"].isin(real["zid"])]
    ntr = t.groupby("zid").size().reindex(real["zid"], fill_value=0)
    print(f"  address-based zones: population p10/p25/p50/p75/p90 {q(pop)}; mean {pop.mean():.0f}")
    print(f"  block-group centres per zone p10/p50/p90 {q(nbg,(10,50,90))}; tract centres per zone p10/p25/p50/p75/p90 {q(ntr)}; zones holding no tract centre {100*(ntr==0).mean():.1f}%")
    popw = np.repeat(pop.values, 1)
    o = np.sort(pop.values); cw = np.cumsum(o); print(f"  population-weighted median zone population {o[np.searchsorted(cw, cw[-1]/2)]:.0f}")
    # Are tracts split between zones? Among tracts whose block groups all fall in address-based zones of this level.
    bt = br.groupby("TRACT")["zid"].nunique()
    tp = bg.groupby("TRACT")["POPULATION"].sum()
    w = tp.reindex(bt.index).fillna(0)
    print(f"  tracts (by their block-group centres) in 1 / 2 / 3+ {name} zones: {100*w[bt==1].sum()/w.sum():.1f} / {100*w[bt==2].sum()/w.sum():.1f} / {100*w[bt>=3].sum()/w.sum():.1f}% of population")
    # Same with clusters of block groups unavailable; report the share of tracts not split.
