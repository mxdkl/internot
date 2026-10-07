import pandas as pd, numpy as np
from polychoric import *
d=pd.read_parquet('gss_sub.parquet'); d['w']=d.wtssps
d=d[(d.age>=30)&d.cohort.notna()]
d['pmax_deg']=d[['padeg','madeg']].max(axis=1)
l4=lambda x: x.clip(upper=3)
groups={'1883-1929':(1883,1930),'1930-49':(1930,1950),'1950-69':(1950,1970),'1970-94':(1970,1995)}
n4=['<HS','HS','AA','BA+']
for g,(a,b) in groups.items():
    t=d[(d.cohort>=a)&(d.cohort<b)].dropna(subset=['degree','pmax_deg'])
    tab=pd.crosstab(l4(t.pmax_deg),l4(t.degree),values=t.w,aggfunc='sum').fillna(0)
    # scale to effective n = actual n to keep likelihood sensible
    tab=tab/tab.values.sum()*len(t)
    r,obs,fit=fit_report(tab.values,['parent '+x for x in n4],['child '+x for x in n4])
    print(f'\n### cohort {g}: polychoric r(max parent degree, child degree) = {r:.3f} (n={len(t)})')
    print('observed row %:'); print(obs.to_string()); print('bivariate-normal fit row %:'); print(fit.to_string())
    # years: correlation for comparison (mid-parent) and by race
for g,(a,b) in groups.items():
    out=[]
    for rc,lab in [(1,'white'),(2,'black')]:
        t=d[(d.cohort>=a)&(d.cohort<b)&(d.race==rc)].dropna(subset=['degree','pmax_deg'])
        tab=pd.crosstab(l4(t.pmax_deg),l4(t.degree),values=t.w,aggfunc='sum').fillna(0); tab=tab/tab.values.sum()*len(t)
        out.append(f'{lab} r={polychoric(tab.values):.3f} n={len(t)}')
    print(g,' | '.join(out))
