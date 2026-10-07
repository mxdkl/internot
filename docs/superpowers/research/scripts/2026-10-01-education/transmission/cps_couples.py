import pandas as pd, numpy as np
cols=['PH_SEQ','A_LINENO','A_AGE','A_SEX','A_SPOUSE','PECOHAB','A_HGA','MARSUPWT','PRDTRACE','PEHSPNON','PENATVTY']
p=pd.read_csv('pppub23.csv',usecols=cols)
h=pd.read_csv('hhpub23.csv',usecols=['H_SEQ','GESTFIPS'])
p=p.merge(h,left_on='PH_SEQ',right_on='H_SEQ',how='left')
yrs={31:0,32:2.5,33:5.5,34:7.5,35:9,36:10,37:11,38:12,39:12,40:13,41:14,42:14,43:16,44:18,45:19,46:20}
p['yrs']=p.A_HGA.map(yrs)
def cat4(a):
    return np.select([a<=38,a==39,a<=42,a>=43],[0,1,2,3],-1)
def cat6(a):
    return np.select([a<=38,a==39,a==40,a<=42,a==43,a>=44],[0,1,2,3,4,5],-1)
p['e4']=cat4(p.A_HGA); p['e6']=cat6(p.A_HGA)
p['eth']=np.where(p.PEHSPNON==1,'H',np.select([p.PRDTRACE==1,p.PRDTRACE==2,p.PRDTRACE==3,p.PRDTRACE==4,p.PRDTRACE==5],['W','B','AIAN','A','A'],'O'))
p['fb']=(p.PENATVTY>=100)  # foreign born (country code >=100)
p['coh5']=(2023-p.A_AGE)//5*5
p['w']=p.MARSUPWT/100.0
key=p.set_index(['PH_SEQ','A_LINENO'])
def couples(kind):
    if kind=='married':
        m=p[(p.A_SEX==1)&(p.A_SPOUSE>0)].copy(); m['pl']=m.A_SPOUSE
    else:
        m=p[(p.A_SEX==1)&(p.PECOHAB>0)].copy(); m['pl']=m.PECOHAB
    f=p[p.A_SEX==2][['PH_SEQ','A_LINENO','A_AGE','yrs','e4','e6','eth','coh5','fb']]
    c=m.merge(f,left_on=['PH_SEQ','pl'],right_on=['PH_SEQ','A_LINENO'],suffixes=('_h','_w'))
    return c
names4=['<HS','HS','SomeColl/AA','BA+']
for kind in ['married','cohab']:
    c=couples(kind)
    print('\n######',kind,'opposite-sex couples, n=',len(c),'weighted (k)=',round(c.w.sum()/1000))
    for lo,hi,lab in [(18,99,'all ages'),(25,34,'wife 25-34'),(35,44,'wife 35-44'),(45,54,'wife 45-54'),(55,64,'wife 55-64'),(65,99,'wife 65+')]:
        s=c[(c.A_AGE_w>=lo)&(c.A_AGE_w<=hi)]
        W=s.w.sum()
        t=pd.crosstab(s.e4_h,s.e4_w,values=s.w,aggfunc='sum').fillna(0)/W*100
        t.index=['H '+names4[i] for i in t.index]; t.columns=['W '+names4[i] for i in t.columns]
        hom=sum(s.w[s.e4_h==s.e4_w])/W*100
        wmore=sum(s.w[s.e4_w>s.e4_h])/W*100; hmore=sum(s.w[s.e4_h>s.e4_w])/W*100
        # independence baseline
        mh=s.groupby('e4_h').w.sum()/W; mw=s.groupby('e4_w').w.sum()/W
        indep=sum(mh.get(k,0)*mw.get(k,0) for k in range(4))*100
        def wcorr(x,y,w):
            mx=np.average(x,weights=w); my=np.average(y,weights=w)
            return np.average((x-mx)*(y-my),weights=w)/np.sqrt(np.average((x-mx)**2,weights=w)*np.average((y-my)**2,weights=w))
        r=wcorr(s.yrs_h,s.yrs_w,s.w)
        # 6-cat homogamy
        hom6=sum(s.w[s.e6_h==s.e6_w])/W*100
        print(f'\n-- {lab}: n={len(s)}, corr(years)={r:.3f}, same 4-level={hom:.1f}% (indep {indep:.1f}%), same 6-level={hom6:.1f}%, wife higher={wmore:.1f}%, husband higher={hmore:.1f}%, both BA+={sum(s.w[(s.e4_h==3)&(s.e4_w==3)])/W*100:.1f}%')
        if lab in ('all ages','wife 25-34','wife 55-64','wife 65+'):
            print(t.round(1).to_string())
    # "free" correlation via cells: cohort5 x eth x state, and subsets
    s=c[(c.A_AGE_w>=25)]
    def wmean_map(df,keys,val):
        g=df.groupby(keys).apply(lambda d: np.average(d[val],weights=d.w),include_groups=False)
        return g
    # use all adults of each sex 25+ for cell means
    for keys in [['coh5'],['eth'],['GESTFIPS'],['coh5','eth'],['coh5','eth','GESTFIPS']]:
        men=p[(p.A_SEX==1)&(p.A_AGE>=25)]; wom=p[(p.A_SEX==2)&(p.A_AGE>=25)]
        mm=men.groupby(keys).apply(lambda d: np.average(d.yrs,weights=d.w),include_groups=False).rename('mh')
        mw=wom.groupby(keys).apply(lambda d: np.average(d.yrs,weights=d.w),include_groups=False).rename('mw')
        hk=[k+'_h' if k in ('coh5','eth') else k for k in keys]; wk=[k+'_w' if k in ('coh5','eth') else k for k in keys]
        x=s.merge(mm,left_on=hk,right_index=True,how='left').merge(mw,left_on=wk,right_index=True,how='left').dropna(subset=['mh','mw'])
        cov=np.average((x.mh-np.average(x.mh,weights=x.w))*(x.mw-np.average(x.mw,weights=x.w)),weights=x.w)
        sh=np.sqrt(np.average((x.yrs_h-np.average(x.yrs_h,weights=x.w))**2,weights=x.w)); sw=np.sqrt(np.average((x.yrs_w-np.average(x.yrs_w,weights=x.w))**2,weights=x.w))
        print(f'free corr via cell means {keys}: {cov/(sh*sw):.3f}  (couples n={len(x)})')
    # same for natives only
