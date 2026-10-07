# One address-based zone per block group (the smallest containing it), then:
# how many zones a tract's block groups fall in, and zone sizes in that assignment.
import sys, numpy as np, pandas as pd, geopandas as gpd
z = gpd.read_file(sys.argv[1], engine="pyogrio", columns=["level", "defacto", "openEnroll", "Shape_Area"])
z = z[(z.defacto == "0") & (z.openEnroll != "1")].copy(); z["zid"] = np.arange(len(z))
df = pd.read_csv("/home/player1/internot/datasets/geo/cenpop/CenPop2020_Mean_BG.txt", dtype=str, encoding="utf-8-sig")
df["POPULATION"] = df["POPULATION"].astype(int); df = df[(df.STATEFP.astype(int) <= 56) & (df.POPULATION > 0)]
df["TRACT"] = df.STATEFP + df.COUNTYFP + df.TRACTCE
bg = gpd.GeoDataFrame(df, geometry=gpd.points_from_xy(df.LONGITUDE.astype(float), df.LATITUDE.astype(float)), crs="EPSG:4269").to_crs(z.crs)
q = lambda a, ps=(10, 25, 50, 75, 90): [int(round(float(np.percentile(a, p)))) for p in ps]
for lev, name in (("1", "Primary"), ("2", "Middle"), ("3", "High")):
    j = gpd.sjoin(bg[["POPULATION", "TRACT", "geometry"]], z[z.level == lev], predicate="within", how="inner")
    j = j.sort_values("Shape_Area").reset_index().drop_duplicates("index")  # smallest zone per block group
    pop = j.groupby("zid").POPULATION.sum()
    per_tract = j.groupby("TRACT").zid.nunique(); tp = j.groupby("TRACT").POPULATION.sum()
    # only tracts whose block groups are all covered by address-based zones of this level
    allbg = bg.groupby("TRACT").size(); cov = j.groupby("TRACT").size()
    full = cov.index[cov.values == allbg.reindex(cov.index).values]
    pt = per_tract.reindex(full); w = tp.reindex(full)
    print(f"{name}: zones with population {len(pop)}; population per zone p10/p25/p50/p75/p90 {q(pop)}; tracts fully covered {len(full)}")
    print(f"   population in tracts whose block groups fall in 1 / 2 / 3+ zones: {100*w[pt==1].sum()/w.sum():.1f} / {100*w[pt==2].sum()/w.sum():.1f} / {100*w[pt>=3].sum()/w.sum():.1f}%")
    zt = j.groupby("zid").TRACT.nunique(); print(f"   distinct tracts touched per zone p10/p50/p90 {q(zt,(10,50,90))}")
