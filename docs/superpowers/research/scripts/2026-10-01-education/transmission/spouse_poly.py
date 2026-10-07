import pandas as pd, numpy as np
from polychoric import *
d=pd.read_stata('sm/prevmar_tabpub.dta',convert_categoricals=False)
for y in [1940,1960,1970,1980,1990,2000]:
    s=d[(d.y==y)&(d.cen==1)]
    t=pd.crosstab(s.hedc,s.wedc,values=s.freq,aggfunc='sum').fillna(0).values
    print(f'Schwartz-Mare prevailing marriages (wives 18-40), {y} census, 5 levels: polychoric r = {polychoric(t):.3f}')
s=d[(d.y==2003)&(d.cen==0)]
t=pd.crosstab(s.hedc,s.wedc,values=s.freq,aggfunc='sum').fillna(0).values
print(f'Schwartz-Mare 2003 CPS: polychoric r = {polychoric(t):.3f}')
# Greenwood et al 1960, 2005 (fractions; scale to n=10000 for the likelihood)
g1960=np.array([[0.323,0.138,0.019,0.004,0.001],[0.076,0.165,0.028,0.008,0.002],[0.018,0.051,0.027,0.008,0.002],[0.005,0.027,0.019,0.018,0.003],[0.003,0.016,0.017,0.016,0.008]])
g2005=np.array([[0.039,0.031,0.010,0.003,0.001],[0.023,0.192,0.082,0.037,0.012],[0.005,0.065,0.088,0.047,0.016],[0.002,0.030,0.045,0.104,0.037],[0.001,0.010,0.018,0.050,0.053]])
for lab,g in [('1960',g1960),('2005',g2005)]:
    print(f'Greenwood et al. Table 1 {lab}: polychoric r = {polychoric(g*10000):.3f}, diag={np.trace(g):.3f}, random diag ratio n/a')
