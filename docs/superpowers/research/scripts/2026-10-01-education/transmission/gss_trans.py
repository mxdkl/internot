import pandas as pd, numpy as np
d=pd.read_parquet('gss_sub.parquet')
d['w']=d.wtssps
d=d[(d.age>=30)&d.cohort.notna()]
def wcorr(x,y,w):
    mx=np.average(x,weights=w); my=np.average(y,weights=w)
    c=np.average((x-mx)*(y-my),weights=w); vx=np.average((x-mx)**2,weights=w); vy=np.average((y-my)**2,weights=w)
    return c/np.sqrt(vx*vy), c/vy, np.sqrt(vx), np.sqrt(vy)
bins=[1880,1910,1920,1930,1940,1950,1960,1970,1980,1995]
labs=['1883-1909','1910-19','1920-29','1930-39','1940-49','1950-59','1960-69','1970-79','1980-94']
d['cg']=pd.cut(d.cohort,bins,right=False,labels=labs)
d['pmax_yrs']=d[['paeduc','maeduc']].max(axis=1)
d['pmean_yrs']=d[['paeduc','maeduc']].mean(axis=1,skipna=False)
d['pmax_deg']=d[['padeg','madeg']].max(axis=1)
print('### years of schooling: corr(child, parent) and slope (child on parent), GSS 1972-2024, respondents 30+, wtssps')
print('cohort | n | r(father) b(father) | r(mother) b(mother) | r(max) b(max) | r(mean) b(mean) | sd child | sd mean-parent')
for g in labs:
    s=d[d.cg==g]
    out=[g]
    for col in ['paeduc','maeduc','pmax_yrs','pmean_yrs']:
        t=s.dropna(subset=['educ',col])
        r,b,sx,sy=wcorr(t.educ,t[col],t.w)
        out.append(f'{r:.2f} {b:.2f}')
    t=s.dropna(subset=['educ','pmean_yrs'])
    r,b,sx,sy=wcorr(t.educ,t.pmean_yrs,t.w)
    print(' | '.join([out[0],str(len(t))]+out[1:]+[f'{sx:.2f}',f'{sy:.2f}']))
# by sex of child, joint regression
print('\n### joint regression educ ~ paeduc + maeduc (both reported), by child sex and cohort; standardized betas in brackets')
for g in [['1883-1909','1910-19','1920-29'],['1930-39','1940-49'],['1950-59','1960-69'],['1970-79','1980-94']]:
    for sx,lab in [(1,'sons'),(2,'daughters')]:
        t=d[d.cg.isin(g)&(d.sex==sx)].dropna(subset=['educ','paeduc','maeduc'])
        X=np.column_stack([np.ones(len(t)),t.paeduc,t.maeduc]); W=np.sqrt(t.w.values)
        beta=np.linalg.lstsq(X*W[:,None],t.educ.values*W,rcond=None)[0]
        sdy=np.sqrt(np.cov(t.educ,aweights=t.w)); sdp=np.sqrt(np.cov(t.paeduc,aweights=t.w)); sdm=np.sqrt(np.cov(t.maeduc,aweights=t.w))
        print(f'{g[0]}..{g[-1]} {lab}: n={len(t)} b_father={beta[1]:.3f} [{beta[1]*sdp/sdy:.2f}] b_mother={beta[2]:.3f} [{beta[2]*sdm/sdy:.2f}]')
# parent-parent correlation (spouse corr of the parent generation, as reported)
print('\n### corr(paeduc, maeduc) by child cohort (parents\' generation spouse correlation)')
for g in labs:
    t=d[d.cg==g].dropna(subset=['paeduc','maeduc'])
    print(g, len(t), round(wcorr(t.paeduc,t.maeduc,t.w)[0],3))
# transition tables
names={0:'<HS',1:'HS',2:'AA',3:'BA',4:'Grad'}
def lvl4(x): return np.where(x>=3,3,x)
d['c4']=lvl4(d.degree); d['p4']=lvl4(d.pmax_deg)
n4=['<HS','HS','AA (junior coll.)','BA+']
print('\n### child highest degree by higher parent degree (max of padeg, madeg), row %, respondents 30+')
for grp in [['1883-1909','1910-19','1920-29'],['1930-39','1940-49'],['1950-59','1960-69'],['1970-79','1980-94'],labs]:
    t=d[d.cg.isin(grp)].dropna(subset=['c4','p4'])
    tab=pd.crosstab(t.p4,t.c4,values=t.w,aggfunc='sum').fillna(0)
    rowp=(tab.div(tab.sum(axis=1),axis=0)*100).round(1)
    rowp.index=['parent '+n4[int(i)] for i in rowp.index]; rowp.columns=['child '+n4[int(i)] for i in rowp.columns]
    share=(tab.sum(axis=1)/tab.values.sum()*100).round(1)
    rowp['parent share %']=share.values
    rowp['n']=t.groupby('p4').size().values
    print(f'\ncohorts {grp[0]}..{grp[-1]} (n={len(t)})'); print(rowp.to_string())
    # overall child dist
    cd=t.groupby('c4').w.sum()/t.w.sum()*100
    print('child overall %:', cd.round(1).to_dict())
# BA among children with both parents BA+ vs neither etc
print('\n### P(child BA+) by parents\' BA status (both / one / neither BA+), respondents 30+')
d['pa_ba']=d.padeg>=3; d['ma_ba']=d.madeg>=3
for g in labs:
    t=d[(d.cg==g)].dropna(subset=['padeg','madeg','degree'])
    nb=t.pa_ba.astype(int)+t.ma_ba.astype(int)
    row=[g,str(len(t))]
    for k in [0,1,2]:
        s=t[nb==k]; row.append(f'{(s.w*(s.degree>=3)).sum()/s.w.sum()*100:.1f} ({len(s)})')
    row.append(f'{(t.w*(t.degree>=3)).sum()/t.w.sum()*100:.1f}')
    print(' | '.join(row))
