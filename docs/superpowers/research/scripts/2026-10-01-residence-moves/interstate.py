import pickle, collections, csv, math
d=pickle.load(open('pums/agg.pkl','rb')); TR=d['trip']; ST=d['stock']
cen={}
for r in csv.DictReader(open('/home/player1/internot/datasets/geo/cenpop/CenPop2020_Mean_ST.txt', encoding='utf-8-sig')):
    cen[int(r['STATEFP'])]=(float(r['LATITUDE']), float(r['LONGITUDE']), int(r['POPULATION']))
def hav(a,b):
    la1,lo1,la2,lo2=map(math.radians,(cen[a][0],cen[a][1],cen[b][0],cen[b][1]))
    h=math.sin((la2-la1)/2)**2+math.cos(la1)*math.cos(la2)*math.sin((lo2-lo1)/2)**2
    return 2*3958.8*math.asin(math.sqrt(h))
# census divisions by state FIPS
div={}
D={1:[9,23,25,33,44,50],2:[34,36,42],3:[17,18,26,39,55],4:[19,20,27,29,31,38,46],5:[10,11,12,13,24,37,45,51,54],6:[1,21,28,47],7:[5,22,40,48],8:[4,8,16,30,32,35,49,56],9:[2,6,15,41,53]}
for k,v in D.items():
    for s in v: div[s]=k
agl=['<18','18-24','25-34','35-49','50-64','65+']
# classification of native-born interstate movers
cls=collections.defaultdict(collections.Counter)
for (b,o,dd,ag),w in TR.items():
    if b==0: cls[ag]['foreign-born']+=w; continue
    if o==b: cls[ag]['primary (leaving birth state)']+=w
    elif dd==b: cls[ag]['return (to birth state)']+=w
    else: cls[ag]['onward']+=w
tot=collections.Counter()
for ag in range(6):
    t=sum(cls[ag].values()); tot.update(cls[ag])
    print(agl[ag], {k: round(v/t,3) for k,v in sorted(cls[ag].items())}, 'n(M)', round(t/1e6,2))
t=sum(tot.values()); print('ALL', {k: round(v/t,3) for k,v in sorted(tot.items())})
nat=sum(v for k,v in tot.items() if k!='foreign-born')
print('native only:', {k: round(v/nat,3) for k,v in sorted(tot.items()) if k!='foreign-born'})
# stock: share of natives living outside birth state
s_out=sum(w for (b,s),w in ST.items() if b and b!=s); s_nat=sum(w for (b,s),w in ST.items() if b)
print('natives living outside birth state', round(s_out/s_nat,3))
# static-region test among native interstate movers
J=collections.Counter(); 
for (b,o,dd,ag),w in TR.items():
    if b: J[(b,o,dd)]+=w
N=sum(J.values())
Pbo=collections.Counter(); Pod=collections.Counter(); Pbd=collections.Counter(); Po=collections.Counter(); Pb=collections.Counter()
for (b,o,dd),w in J.items(): Pbo[(b,o)]+=w; Pod[(o,dd)]+=w; Pbd[(b,dd)]+=w; Po[o]+=w; Pb[b]+=w
# model O: dest ~ P(d | o) (origin-keyed); model B: dest ~ P(d | b), excluding d==o (renormalize, since an interstate move can't stay)
def model_overlap(key):
    ov=0.0; dist_model=0.0; dist_act=0.0; near_m=0.0; near_a=0.0
    for (b,o),wbo in Pbo.items():
        # model destination distribution
        if key=='O': cand={dd:w for (oo,dd),w in Pod.items() if oo==o}
        elif key=='B': cand={dd:w for (bb,dd),w in Pbd.items() if bb==b and dd!=o}
        elif key=='Bdiv':
            cand=collections.Counter()
            for (bb,dd),w in Pbd.items():
                if div[bb]==div[b] and dd!=o: cand[dd]+=w
        elif key=='Bnoret':
            cand={dd:w for (bb,dd),w in Pbd.items() if bb==b and dd!=o}
        s=sum(cand.values())
        act={dd:J[(b,o,dd)] for dd in cand}  # actual
        for dd,w in cand.items():
            pm=wbo*w/s; pa=J.get((b,o,dd),0)
            ov+=min(pm,pa); dist_model+=pm*hav(o,dd); near_m+=pm*(hav(o,dd)<=300)
    for (b,o,dd),w in J.items(): dist_act+=w*hav(o,dd); near_a+=w*(hav(o,dd)<=300)
    return ov/N, dist_model/N, dist_act/N, near_m/N, near_a/N
for key,nm in (('O','dest | current origin state'),('B','dest | birth state'),('Bdiv','dest | birth division')):
    ov,dm,da,nm_,na=model_overlap(key)
    print(f'{nm}: overlap with actual joint {ov:.3f}; mean origin-dest distance model {dm:.0f} mi vs actual {da:.0f}; share <=300 mi model {nm_:.3f} vs actual {na:.3f}')
# same split by whether mover currently lives in birth state
for cond,nm in ((lambda b,o: o==b, 'origin = birth state'), (lambda b,o: o!=b, 'origin != birth state')):
    sub={k:v for k,v in J.items() if cond(k[0],k[1])}; n=sum(sub.values())
    ovO=ovB=0; dA=dB=0; nA=nB=0
    for (b,o),wbo in Pbo.items():
        if not cond(b,o): continue
        cO={dd:w for (oo,dd),w in Pod.items() if oo==o}; sO=sum(cO.values())
        cB={dd:w for (bb,dd),w in Pbd.items() if bb==b and dd!=o}; sB=sum(cB.values())
        for dd,w in cO.items(): ovO+=min(wbo*w/sO, J.get((b,o,dd),0))
        for dd,w in cB.items(): ovB+=min(wbo*w/sB, J.get((b,o,dd),0)); dB+=wbo*w/sB*hav(o,dd); nB+=wbo*w/sB*(hav(o,dd)<=300)
    for (b,o,dd),w in sub.items(): dA+=w*hav(o,dd); nA+=w*(hav(o,dd)<=300)
    print(f'{nm}: share of native interstate movers {n/N:.3f}; overlap origin-keyed {ovO/n:.3f}, birth-keyed {ovB/n:.3f}; mean dist actual {dA/n:.0f} vs birth-keyed {dB/n:.0f}; <=300mi actual {nA/n:.3f} vs birth-keyed {nB/n:.3f}')
# baseline: destination independent of everything (national distribution excluding origin)
Pd=collections.Counter()
for (o,dd),w in Pod.items(): Pd[dd]+=w
ov=0; dm=0; nm_=0
for (b,o),wbo in Pbo.items():
    c={dd:w for dd,w in Pd.items() if dd!=o}; s=sum(c.values())
    for dd,w in c.items(): pm=wbo*w/s; ov+=min(pm,J.get((b,o,dd),0)); dm+=pm*hav(o,dd); nm_+=pm*(hav(o,dd)<=300)
print(f'dest | nothing (national): overlap {ov/N:.3f}; mean dist {dm/N:.0f}; <=300 mi {nm_/N:.3f}')
