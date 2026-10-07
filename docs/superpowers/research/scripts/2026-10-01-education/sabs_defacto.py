import sys, numpy as np, pandas as pd, geopandas as gpd
z = gpd.read_file(sys.argv[1], engine="pyogrio", columns=["level", "defacto", "openEnroll"])
df = pd.read_csv("/home/player1/internot/datasets/geo/cenpop/CenPop2020_Mean_BG.txt", dtype=str, encoding="utf-8-sig")
df["POPULATION"] = df["POPULATION"].astype(int); df = df[(df.STATEFP.astype(int) <= 56) & (df.POPULATION > 0)]
bg = gpd.GeoDataFrame(df, geometry=gpd.points_from_xy(df.LONGITUDE.astype(float), df.LATITUDE.astype(float)), crs="EPSG:4269").to_crs(z.crs)
P = bg.POPULATION.sum()
for lev, name in (("1", "Primary"), ("2", "Middle"), ("3", "High")):
    zl = z[z.level == lev]
    j = gpd.sjoin(bg[["POPULATION", "geometry"]], zl, predicate="within", how="inner")
    df_ = j[j.defacto == "1"]; ad = j[(j.defacto == "0") & (j.openEnroll != "1")]; oe = j[j.openEnroll == "1"]
    pop = lambda s: bg.loc[bg.index.isin(s.index), "POPULATION"].sum() / P * 100
    print(f"{name}: population in de facto zones {pop(df_):.1f}%, in address-based zones {pop(ad):.1f}%, in open-enrollment zones {pop(oe):.1f}%; block groups in 2+ zones of this level {100*(j.index.value_counts()>1).mean():.1f}% of covered block groups")
