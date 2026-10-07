import pandas as pd, numpy as np
lab=['<10','10-11','12','13-15','16+']
mid={1:7.5,2:10.5,3:12,4:14,5:16.5}
for f,title in [('sm/prevmar_tabpub.dta','prevailing marriages, wives 18-40'),('sm/newly_tabpub.dta','newlyweds')]:
    d=pd.read_stata(f,convert_categoricals=False)
    print('\n###',title)
    print('year src | n | same5 | indep5 | wife more | husband more | both 16+ | wife 16+ | husb 16+ | corr(midpoints)')
    keys=['y','cen'] 
    for (y,c),s in d.groupby(keys):
        W=s.WEIGHT.sum(); 
        if W==0: continue
        same=s[s.hedc==s.wedc].WEIGHT.sum()/W*100
        wm=s[s.wedc>s.hedc].WEIGHT.sum()/W*100; hm=s[s.hedc>s.wedc].WEIGHT.sum()/W*100
        mh=s.groupby('hedc').WEIGHT.sum()/W; mw=s.groupby('wedc').WEIGHT.sum()/W
        ind=sum(mh.get(k,0)*mw.get(k,0) for k in range(1,6))*100
        b16=s[(s.hedc==5)&(s.wedc==5)].WEIGHT.sum()/W*100
        x=s.hedc.map(mid); z=s.wedc.map(mid); w=s.WEIGHT
        mx=np.average(x,weights=w); mz=np.average(z,weights=w)
        r=np.average((x-mx)*(z-mz),weights=w)/np.sqrt(np.average((x-mx)**2,weights=w)*np.average((z-mz)**2,weights=w))
        if title.startswith('prev') and not (c==1 or y in (1962,1965,1975,1985,1995,2003)): continue
        print(f"{int(y)} {'cen' if c==1 else 'cps'} | {int(s.freq.sum())} | {same:.1f} | {ind:.1f} | {wm:.1f} | {hm:.1f} | {b16:.1f} | {mw[5]*100:.1f} | {mh[5]*100:.1f} | {r:.2f}")
    # 5x5 tables for 1960 and 2000 census
    if title.startswith('prev'):
        for y in [1940,1960,1980,2000]:
            s=d[(d.y==y)&(d.cen==1)]
            t=pd.crosstab(s.hedc,s.wedc,values=s.WEIGHT,aggfunc='sum')/s.WEIGHT.sum()*100
            t.index=['H '+lab[int(i)-1] for i in t.index]; t.columns=['W '+lab[int(i)-1] for i in t.columns]
            print(f'\n{y} census, % of couples (rows husband, cols wife)'); print(t.round(1).to_string())
