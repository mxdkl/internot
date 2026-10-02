exec(open('flows.py').read())
import math, collections
cc={}
for r in csv.DictReader(open(G+'cenpop/CenPop2020_Mean_CO.txt', encoding='utf-8-sig')):
    cc[r['STATEFP']+r['COUNTYFP']]=(int(r['POPULATION']), float(r['LATITUDE']), float(r['LONGITUDE']))
def hav(a,b):
    la1,lo1,la2,lo2=map(math.radians,(a[0],a[1],b[0],b[1]))
    h=math.sin((la2-la1)/2)**2+math.cos(la1)*math.cos(la2)*math.sin((lo2-lo1)/2)**2
    return 2*3958.8*math.asin(math.sqrt(h))
# CZ centroid & pop
czp=collections.Counter(); czlat=collections.Counter(); czlon=collections.Counter()
for g,(p,la,lo) in cc.items():
    if g[:2]=='72' or g not in cz: continue
    c=cz[g]; czp[c]+=p; czlat[c]+=p*la; czlon[c]+=p*lo
czc={c:(czlat[c]/czp[c], czlon[c]/czp[c]) for c in czp if czp[c]>0}
# county-level distance distribution of intercounty moves
miss=0
def wq(pairs, qs):
    pairs=sorted(pairs); tot=sum(w for _,w in pairs); out=[]; acc=0; i=0
    for q in qs:
        while acc < q*tot: acc+=pairs[i][1]; i+=1
        out.append(round(pairs[i-1][0]))
    return out
inter=[]; xczl=[]; F=collections.Counter()
for A,B,f in flows:
    if f==0: continue
    if A not in cc or B not in cc: miss+=f; continue
    d=hav(cc[A][1:], cc[B][1:]); inter.append((d,f))
    if cz[A]!=cz[B]:
        xczl.append((d,f)); F[(cz[B],cz[A])]+=f
print('missing', miss)
print('intercounty moves: county-centroid distance quantiles 10/25/50/75/90:', wq(inter,[.1,.25,.5,.75,.9]))
print('cross-CZ moves: quantiles 10/25/50/75/90:', wq(xczl,[.1,.25,.5,.75,.9]))
tot=sum(f for _,f in xczl)
for t in [50,100,250,500,1000]: print(f'  cross-CZ share within {t} mi: {sum(f for d,f in xczl if d<=t)/tot:.3f}')
# gravity: bins of CZ-pair distance; ratio flows/(Pi Pj)
bins=[25,50,75,100,150,200,300,400,600,800,1000,1500,2000,3000]
num=collections.Counter(); den=collections.Counter()
cl=list(czc)
for i in cl:
    for j in cl:
        if i==j: continue
        d=hav(czc[i],czc[j]); b=next((k for k,x in enumerate(bins) if d<x), len(bins))
        den[b]+=czp[i]*czp[j]; num[b]+=F.get((i,j),0)
print('bin_upper  flow/(PiPj) x1e12  pairs-mass')
pts=[]
for b in sorted(den):
    up=bins[b] if b<len(bins) else 99999; lo=bins[b-1] if b>0 else 0
    r=num[b]/den[b]*1e12; mid=math.sqrt(max(lo,10)*min(up,4000))
    print(lo, up, round(r,4), num[b])
    if num[b]>0: pts.append((math.log(mid), math.log(r), num[b]))
def slope(pts):
    n=len(pts); mx=sum(p[0] for p in pts)/n; my=sum(p[1] for p in pts)/n
    return sum((p[0]-mx)*(p[1]-my) for p in pts)/sum((p[0]-mx)**2 for p in pts)
print('log-log slope all bins', round(slope(pts),3), 'bins<300mi', round(slope([p for p in pts if p[0]<math.log(300)]),3), 'bins>=300mi', round(slope([p for p in pts if p[0]>=math.log(300)]),3))
