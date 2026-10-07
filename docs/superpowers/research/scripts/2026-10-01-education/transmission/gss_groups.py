import pandas as pd, numpy as np
d=pd.read_parquet('gss_sub.parquet'); d['w']=d.wtssps
d=d[(d.age>=30)&d.cohort.notna()]
d['p4']=d[['padeg','madeg']].max(axis=1).clip(upper=3); d['c4']=d.degree.clip(upper=3)
d=d.dropna(subset=['p4','c4'])
d['grp']=np.where(d.hispanic.fillna(1)>1,'Hispanic',np.select([d.race==1,d.race==2],['White NH','Black NH'],'Other NH'))
d['ba']=(d.c4==3).astype(float)
n4=['<HS','HS','AA','BA+']
def tab(s,by):
    g=s.groupby([by,'p4']).apply(lambda x: pd.Series({'ba':np.average(x.ba,weights=x.w)*100,'n':len(x)}),include_groups=False)
    out=g.ba.unstack().round(1); out.columns=['parent '+n4[int(c)] for c in out.columns]
    out['n']=g.n.unstack().sum(1).astype(int)
    return out
print('### P(child BA+) % by higher parent degree and heritage, cohorts 1950-94, respondents 30+ (Hispanic identified from 2000 surveys only)')
print(tab(d[(d.cohort>=1950)],'grp').to_string())
print('\n### P(child BA+) % by higher parent degree and sex')
d['sexl']=np.where(d.sex==1,'men','women')
for a,b in [(1930,1950),(1950,1970),(1970,1995)]:
    print(f'cohorts {a}-{b-1}'); print(tab(d[(d.cohort>=a)&(d.cohort<b)],'sexl').to_string())
print('\n### foreign-born respondents (born==2) vs native, cohorts 1950-94')
d['nat']=np.where(d.born==2,'foreign-born','US-born')
print(tab(d[(d.cohort>=1950)&d.born.notna()],'nat').to_string())
