import csv, collections, sys
G='/home/player1/internot/datasets/geo/'
cz={}
for r in csv.DictReader(open(G+'cz/ers_2020-commuting-zones.csv', encoding='utf-8-sig')): cz[r['FIPStxt']]=int(r['CZ2020'])
CTOLD=['09001','09003','09005','09007','09009','09011','09013','09015']
for g in CTOLD: cz[g]=70
psu={}
for r in csv.DictReader(open(G+'cz/county20.csv', encoding='utf-8-sig')): psu[r['GEOID'].strip()]=int(r['CZ20'])
for g in CTOLD: psu[g]=psu['09110']
cbsa={}
for r in csv.DictReader(open(G+'cbsa/list1_2023.csv', encoding='latin-1')):
    cbsa[r['state_fips'].zfill(2)+r['county_fips'].zfill(3)]=(r['cbsa'], r['metro_micro'])
for r in csv.DictReader(open(G+'cz/counties10.csv', encoding='latin-1')):
    g=str(r['FIPS']).zfill(5)
    if g.startswith('09') and r['CBSA10'] not in ('NA',''): cbsa[g]=(r['CBSA10'], 'Metropolitan Statistical Area')
def num(s):
    s=s.strip(); return int(s) if s and s!='.' else 0
A_tot={}
flows=[]
with open(G+'flows/acs2016_2020_CtyxCty_US.txt', encoding='latin-1') as f:
    for l in f:
        sa=l[0:3]; ca=l[3:6]; sb=l[6:9]; cb=l[9:12]
        if not sa.isdigit() or int(sa)>56: continue
        A=sa[1:]+ca
        if A not in A_tot:
            toks=l[80:190].split()
            # pop,moe,nonmov,moe,movers,moe,same,moe,dc,moe,ds,moe,abroad,moe
            vals=[num(t) for t in toks[:14]]
            A_tot[A]=dict(pop=vals[0],non=vals[2],mov=vals[4],same=vals[6],dc=vals[8],ds=vals[10],ab=vals[12])
        if not sb.isdigit() or int(sb)>56: continue
        B=sb[1:]+cb
        if A==B: continue
        flows.append((A,B,num(l[374:381])))
