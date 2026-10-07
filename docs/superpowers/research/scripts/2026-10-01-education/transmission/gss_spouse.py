import pandas as pd, numpy as np
d=pd.read_parquet('gss_sub.parquet'); d['w']=d.wtssps
def wcorr(x,y,w):
    mx=np.average(x,weights=w); my=np.average(y,weights=w)
    return np.average((x-mx)*(y-my),weights=w)/np.sqrt(np.average((x-mx)**2,weights=w)*np.average((y-my)**2,weights=w))
d['dec']=(d.year//10*10).astype(int)
m=d[(d.marital==1)&(d.age>=25)&(d.age<=64)].dropna(subset=['educ','speduc'])
print('### GSS married respondents 25-64: corr(educ, speduc), degree homogamy, who has more (by degree, 4 levels <HS/HS/AA/BA+), by survey decade')
def l4(x): return np.where(x>=3,3,x)
for dec,s in m.groupby('dec'):
    r=wcorr(s.educ,s.speduc,s.w)
    t=s.dropna(subset=['degree','spdeg'])
    a=l4(t.degree); b=l4(t.spdeg)
    # husband/wife: respondent sex 1 male -> husband=resp
    hus=np.where(t.sex==1,a,b); wif=np.where(t.sex==1,b,a)
    W=t.w.sum()
    same=(t.w*(hus==wif)).sum()/W*100; wm=(t.w*(wif>hus)).sum()/W*100; hm=(t.w*(hus>wif)).sum()/W*100
    # years: wife more years
    hy=np.where(s.sex==1,s.educ,s.speduc); wy=np.where(s.sex==1,s.speduc,s.educ)
    wmy=(s.w*(wy>hy)).sum()/s.w.sum()*100; hmy=(s.w*(hy>wy)).sum()/s.w.sum()*100; sy=(s.w*(hy==wy)).sum()/s.w.sum()*100
    print(f'{dec}s n={len(s)} r_years={r:.3f} | 4-level: same {same:.1f}% wife more {wm:.1f}% husband more {hm:.1f}% | years: same {sy:.1f}% wife more {wmy:.1f}% husband more {hmy:.1f}%')
c=d[(d.age>=25)&(d.age<=64)].dropna(subset=['educ','coeduc'])
print('\ncohabiting partner (coeduc available years):', sorted(c.year.unique()), 'n=',len(c),'r=',round(wcorr(c.educ,c.coeduc,c.w),3))
