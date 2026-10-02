import csv, collections
G='/home/player1/internot/datasets/geo/'
cz={}
for r in csv.DictReader(open(G+'cz/ers_2020-commuting-zones.csv', encoding='utf-8-sig')): cz[r['FIPStxt']]=int(r['CZ2020'])
for g in ['09001','09003','09005','09007','09009','09011','09013','09015']: cz[g]=70
# 2011-2015 county codes: Shannon SD 46113 -> 46102, Wade Hampton 02270 -> 02158, Bedford city 51515 merged into 51019; Valdez-Cordova 02261 -> 02063/02066 (both same CZ?)
alias={'46113':'46102','02270':'02158','51515':'51019','02261':'02063'}
def C(g): return cz.get(alias.get(g,g))
def num(s):
    s=s.strip(); return int(s) if s and s!='.' else 0
tot=collections.defaultdict(dict); flows=collections.Counter(); xcz=collections.Counter(); unm=collections.Counter()
with open(G+'flows/acs2011_2015_CtyxCty_ager_US.txt', encoding='latin-1') as f:
    for l in f:
        sa=l[0:3]; ca=l[3:6]; sb=l[6:9]; cb=l[9:12]; age=l[13:15]
        if not sa.isdigit() or int(sa)>56: continue
        A=sa[1:]+ca
        if age not in tot[A]:
            toks=l[82:192].split(); v=[num(t) for t in toks[:14]]
            tot[A][age]=dict(pop=v[0],mov=v[4],same=v[6],dc=v[8],ds=v[10],ab=v[12])
        if not sb.isdigit() or int(sb)>56: continue
        B=sb[1:]+cb
        if A==B: continue
        fl=num(l[376:383])
        flows[age]+=fl
        if C(A) is None or C(B) is None: unm[age]+=fl; continue
        if C(A)==C(B): xcz[age]+=fl
labels={'01':'1-4','02':'5-17','03':'18-19','04':'20-24','05':'25-29','06':'30-34','07':'35-39','08':'40-44','09':'45-49','10':'50-54','11':'55-59','12':'60-64','13':'65-69','14':'70-74','15':'75+'}
print('unmatched', dict(unm), sorted({a for a in tot if C(a) is None}))
print('age  pop(M)  moverate  cty%  CZ%  state%  crossCZ_rate  crossState_rate')
for age in sorted({a for t in tot.values() for a in t}):
    S=lambda k: sum(t[age][k] for t in tot.values() if age in t)
    pop=S('pop'); same=S('same'); dc=S('dc'); ds=S('ds'); D=same+dc+ds
    print(age, labels.get(age,'?'), round(pop/1e6,2), round(D/pop,3), round(same/D,3), round((same+xcz[age])/D,3), round((same+dc)/D,3), round((dc+ds-xcz[age])/pop,4), round(ds/pop,4), 'flowcheck', flows[age]-(dc+ds))
print()
print('age  CZ% lower (missing flows cross-CZ)  CZ% upper (missing flows like observed)')
allS=collections.Counter()
for age in sorted({a for t in tot.values() for a in t}):
    S=lambda k: sum(t[age][k] for t in tot.values() if age in t)
    same=S('same'); dc=S('dc'); ds=S('ds'); D=same+dc+ds
    lo=(same+xcz[age])/D; hi=(same+(dc+ds)*xcz[age]/flows[age])/D
    print(labels[age], round(lo,3), round(hi,3), 'share of intercounty in same CZ', round(xcz[age]/flows[age],3))
    for k,v in (('same',same),('dc',dc),('ds',ds),('pop',S('pop')),('fl',flows[age]),('x',xcz[age])): allS[k]+=v
D=allS['same']+allS['dc']+allS['ds']
print('ALL 2011-15: mover rate', round(D/allS['pop'],3), 'cty', round(allS['same']/D,3), 'CZ lo', round((allS['same']+allS['x'])/D,3), 'CZ hi', round((allS['same']+(allS['dc']+allS['ds'])*allS['x']/allS['fl'])/D,3), 'state', round((allS['same']+allS['dc'])/D,3))
