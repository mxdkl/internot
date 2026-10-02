import csv, math, random, collections, sys
G='/home/player1/internot/datasets/geo/'
counties=set(sys.argv[1].split(','))
tr=[]
for r in csv.DictReader(open(G+'cenpop/CenPop2020_Mean_TR.txt', encoding='utf-8-sig')):
    g=r['STATEFP']+r['COUNTYFP']
    if g in counties and int(r['POPULATION'])>0:
        tr.append((r['STATEFP']+r['COUNTYFP']+r['TRACTCE'], int(r['POPULATION']), float(r['LATITUDE']), float(r['LONGITUDE'])))
puma={}
for r in csv.DictReader(open(G+'puma/2020_Census_Tract_to_2020_PUMA.txt', encoding='utf-8-sig')):
    puma[r['STATEFP']+r['COUNTYFP']+r['TRACTCE']]=r['STATEFP']+r['PUMA5CE']
n=len(tr); P=sum(t[1] for t in tr)
lat0=sum(t[2]*t[1] for t in tr)/P
kx=69.17*math.cos(math.radians(lat0)); ky=69.0
xy=[(t[3]*kx, t[2]*ky) for t in tr]; w=[t[1] for t in tr]
print('tracts', n, 'pop', P, 'PUMAs', len({puma[t[0]] for t in tr}))
D=[[math.hypot(xy[i][0]-xy[j][0], xy[i][1]-xy[j][1]) for j in range(n)] for i in range(n)]
# within-tract distance: half the typical spacing: 0.5*sqrt(area) ~ use nearest-neighbour distance/2
for i in range(n):
    D[i][i]=0.5*min(D[i][j] for j in range(n) if j!=i)
def kernel(L, form):
    # returns per-origin normalized destination weight function
    if form=='exp': return lambda d: math.exp(-d/L)
    return lambda d: (d+0.5)**(-L)
def stats(L, form, parts_list):
    f=kernel(L, form)
    dist=[]  # weighted samples (d, weight) aggregated by bins
    hist=collections.Counter(); tot=0.0
    cross=[0.0]*len(parts_list)
    for i in range(n):
        row=D[i]; ws=[w[j]*f(row[j]) for j in range(n)]; s=sum(ws)
        oi=w[i]/s
        for j in range(n):
            m=ws[j]*oi
            if m==0: continue
            hist[round(row[j],1)]+=m
            for k,parts in enumerate(parts_list):
                if parts[i]!=parts[j]: cross[k]+=m
        tot+=w[i]
    acc=0; med=None; q=[None,None,None]
    for d in sorted(hist):
        acc+=hist[d]
        if med is None and acc>=0.5*tot: med=d
        if q[0] is None and acc>=0.25*tot: q[0]=d
        if q[2] is None and acc>=0.75*tot: q[2]=d
    within1=sum(v for d,v in hist.items() if d<=1.0)/tot
    return med, q[0], q[2], within1, [c/tot for c in cross]
def kmeans(k, iters=40, seed=1):
    rnd=random.Random(seed)
    # balanced-ish: weighted k-means++ init
    cents=[xy[rnd.choices(range(n), weights=w)[0]]]
    while len(cents)<k:
        d2=[min((xy[i][0]-c[0])**2+(xy[i][1]-c[1])**2 for c in cents)*w[i] for i in range(n)]
        cents.append(xy[rnd.choices(range(n), weights=d2)[0]])
    for _ in range(iters):
        a=[min(range(k), key=lambda c:(xy[i][0]-cents[c][0])**2+(xy[i][1]-cents[c][1])**2) for i in range(n)]
        sx=[0.0]*k; sy=[0.0]*k; sw=[0.0]*k
        for i in range(n): sx[a[i]]+=xy[i][0]*w[i]; sy[a[i]]+=xy[i][1]*w[i]; sw[a[i]]+=w[i]
        cents=[(sx[c]/sw[c], sy[c]/sw[c]) if sw[c]>0 else cents[c] for c in range(k)]
    return a
names=[]; parts_list=[]
cty=[t[0][:5] for t in tr]; names.append('county'); parts_list.append(cty)
pu=[puma[t[0]] for t in tr]; names.append('PUMA'); parts_list.append(pu)
for size in [2_500_000, 1_000_000, 500_000, 250_000]:
    k=max(1, round(P/size))
    if k<2: continue
    names.append(f'kmeans~{size//1000}k (k={k})'); parts_list.append(kmeans(k))
for form, Ls in (('exp',[2.0, 4.0, 8.0]), ('pow',[2.5, 2.0])):
    for L in Ls:
        med,q1,q3,w1,cr=stats(L, form, parts_list)
        print(f'{form} L={L}: median {med} mi (IQR {q1}-{q3}), within 1 mi {w1:.2f}; crossing: ' + ', '.join(f'{nm} {c:.3f}' for nm,c in zip(names,cr)))
