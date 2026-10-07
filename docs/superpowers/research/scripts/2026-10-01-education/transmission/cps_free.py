import pandas as pd, numpy as np
from polychoric import polychoric
cols=['PH_SEQ','A_LINENO','A_AGE','A_SEX','A_SPOUSE','PECOHAB','A_HGA','MARSUPWT','PRDTRACE','PEHSPNON','PENATVTY']
p=pd.read_csv('pppub23.csv',usecols=cols)
h=pd.read_csv('hhpub23.csv',usecols=['H_SEQ','GESTFIPS'])
p=p.merge(h,left_on='PH_SEQ',right_on='H_SEQ',how='left')
p['e4']=np.select([p.A_HGA<=38,p.A_HGA==39,p.A_HGA<=42,p.A_HGA>=43],[0,1,2,3],-1)
p['eth']=np.where(p.PEHSPNON==1,'H',np.select([p.PRDTRACE==1,p.PRDTRACE==2,p.PRDTRACE==3,p.PRDTRACE==4,p.PRDTRACE==5],['W','B','AIAN','A','A'],'O'))
p['fb']=(p.PENATVTY>=100).astype(int)
p['coh5']=(2023-p.A_AGE)//5*5
p['w']=p.MARSUPWT/100.0
adults=p[(p.A_AGE>=25)&(p.e4>=0)]
def couples(kind):
    m=p[(p.A_SEX==1)&((p.A_SPOUSE>0) if kind=='married' else (p.PECOHAB>0))].copy(); m['pl']=m.A_SPOUSE if kind=='married' else m.PECOHAB
    f=p[p.A_SEX==2]
    return m.merge(f,left_on=['PH_SEQ','pl'],right_on=['PH_SEQ','A_LINENO'],suffixes=('_h','_w'))
for kind in ['married','cohab']:
    c=couples(kind); c=c[(c.A_AGE_w>=25)&(c.A_AGE_w<=64)&(c.A_AGE_h>=25)]
    obs=pd.crosstab(c.e4_h,c.e4_w,values=c.w_h,aggfunc='sum').fillna(0)
    n=len(c); obsn=obs/obs.values.sum()*n
    print(f'\n#### {kind} opposite-sex couples, both 25+, wife 25-64, CPS ASEC 2023 (n={n})')
    print(f'observed: same level {np.trace(obs.values)/obs.values.sum()*100:.1f}%  polychoric r={polychoric(obsn.values):.3f}')
    for keys in [['coh5'],['eth'],['GESTFIPS'],['coh5','eth'],['coh5','eth','GESTFIPS'],['coh5','eth','GESTFIPS','fb']]:
        def dist(sex):
            a=adults[adults.A_SEX==sex]
            g=a.groupby(keys+['e4']).w.sum().unstack(fill_value=0)
            return g.div(g.sum(1),axis=0)
        Dh=dist(1); Dw=dist(2)
        hk=[k+'_h' if k in ('coh5','eth','fb') else k for k in keys]; wk=[k+'_w' if k in ('coh5','eth','fb') else k for k in keys]
        if 'GESTFIPS' in keys:
            hk=[('GESTFIPS_h' if k=='GESTFIPS' else k) for k in hk]; wk=[('GESTFIPS_h' if k=='GESTFIPS' else k) for k in wk]
        ph=Dh.reindex(pd.MultiIndex.from_frame(c[hk]) if len(hk)>1 else c[hk[0]]).values
        pw=Dw.reindex(pd.MultiIndex.from_frame(c[wk]) if len(wk)>1 else c[wk[0]]).values
        ok=~(np.isnan(ph).any(1)|np.isnan(pw).any(1))
        E=np.einsum('n,ni,nj->ij',c.w_h.values[ok],ph[ok],pw[ok])
        En=E/E.sum()*ok.sum()
        print(f'free (independent within {"x".join(keys)} cells): same level {np.trace(E)/E.sum()*100:.1f}%  polychoric r={polychoric(En):.3f}  (couples matched {ok.sum()})')
