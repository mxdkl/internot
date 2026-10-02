import csv, math, random, collections, sys
G='/home/player1/internot/datasets/geo/'
czid=int(sys.argv[1])
cz={}
for r in csv.DictReader(open(G+'cz/ers_2020-commuting-zones.csv', encoding='utf-8-sig')): cz[r['FIPStxt']]=(int(r['CZ2020']), r['CZName'])
counties={g for g,(c,_) in cz.items() if c==czid}
name=next(nm for g,(c,nm) in cz.items() if c==czid)
tr=[]
for r in csv.DictReader(open(G+'cenpop/CenPop2020_Mean_TR.txt', encoding='utf-8-sig')):
    g=r['STATEFP']+r['COUNTYFP']
    if g in counties and int(r['POPULATION'])>0:
        tr.append((r['STATEFP']+r['COUNTYFP']+r['TRACTCE'], int(r['POPULATION']), float(r['LATITUDE']), float(r['LONGITUDE'])))
n=len(tr); P=sum(t[1] for t in tr)
lat0=sum(t[2]*t[1] for t in tr)/P; kx=69.17*math.cos(math.radians(lat0))
xy=[(t[3]*kx, t[2]*69.0) for t in tr]; w=[t[1] for t in tr]
def kmeans(idx, k, seed=1, iters=30):
    if k<=1: return {i:0 for i in idx}
    rnd=random.Random(seed); cents=[xy[rnd.choices(idx, weights=[w[i] for i in idx])[0]]]
    while len(cents)<k:
        d2=[min((xy[i][0]-c[0])**2+(xy[i][1]-c[1])**2 for c in cents)*w[i] for i in idx]
        cents.append(xy[rnd.choices(idx, weights=d2)[0]])
    for _ in range(iters):
        a={i:min(range(k), key=lambda c:(xy[i][0]-cents[c][0])**2+(xy[i][1]-cents[c][1])**2) for i in idx}
        sx=[0.0]*k; sy=[0.0]*k; sw=[0.0]*k
        for i in idx: sx[a[i]]+=xy[i][0]*w[i]; sy[a[i]]+=xy[i][1]*w[i]; sw[a[i]]+=w[i]
        cents=[(sx[c]/sw[c], sy[c]/sw[c]) if sw[c]>0 else cents[c] for c in range(k)]
    return a
# levels: county, ~1M, ~250k, ~40k (nested), then tract
path=[[t[0][:5]] for t in tr]
groups={(): list(range(n))}
def refine(groups, size, by_county=False):
    new={}
    for key,idx in groups.items():
        if by_county:
            sub=collections.defaultdict(list)
            for i in idx: sub[tr[i][0][:5]].append(i)
            for c,ii in sub.items(): new[key+(c,)]=ii
        else:
            pop=sum(w[i] for i in idx); k=max(1, round(pop/size))
            a=kmeans(idx, k)
            sub=collections.defaultdict(list)
            for i in idx: sub[a[i]].append(i)
            for c,ii in sub.items(): new[key+(c,)]=ii
    return new
levels=[('county',None),('~1M',1_000_000),('~250k',250_000),('~40k',40_000)]
lab=[None]*n
for nm,size in levels:
    groups=refine(groups, size, by_county=(size is None))
code={}
for key,idx in groups.items():
    for i in idx: code[i]=key
print(name, 'tracts', n, 'pop', P, 'leaf groups', len(groups))
D=lambda i,j: math.hypot(xy[i][0]-xy[j][0], xy[i][1]-xy[j][1])
nn=[0.5*min(D(i,j) for j in range(n) if j!=i) if n>1 else 0.5 for i in range(n)]
share=collections.Counter()
for i in range(n):
    ws=[w[j]*((nn[i] if i==j else D(i,j))+0.5)**-2.0 for j in range(n)]; s=sum(ws)
    for j in range(n):
        m=w[i]*ws[j]/s
        if i==j: share['same tract']+=m; continue
        ci,cj=code[i],code[j]
        lvl=next(k for k in range(len(ci)) if ci[k]!=cj[k]) if ci!=cj else len(ci)
        share[(['county','~1M','~250k','~40k','tract in ~40k'][lvl])]+=m
for k in ['county','~1M','~250k','~40k','tract in ~40k','same tract']:
    print(f'  highest level changed = {k}: {share[k]/P:.3f}')
