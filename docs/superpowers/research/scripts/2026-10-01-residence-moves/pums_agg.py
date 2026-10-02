import csv, io, zipfile, collections, pickle, sys
D='/home/player1/internot/datasets/acs/pums2023_1yr/'
hh={}
with zipfile.ZipFile(D+'csv_hus.zip') as z:
    for name in ('psam_husa.csv','psam_husb.csv'):
        with z.open(name) as f:
            r=csv.reader(io.TextIOWrapper(f, encoding='latin-1')); h=next(r); I={k:i for i,k in enumerate(h)}
            for row in r:
                if row[I['TYPEHUGQ']]!='1': continue
                hh[row[I['SERIALNO']]]=(row[I['TEN']], row[I['MV']], int(row[I['WGTP']]), row[I['HHT']], int(row[I['NP']]))
print('households', len(hh), file=sys.stderr)
age_tab=collections.defaultdict(collections.Counter)
trip=collections.Counter()
stock=collections.Counter()
hhdur=collections.Counter()
marr=collections.Counter()
with zipfile.ZipFile(D+'csv_pus.zip') as z:
    for name in ('psam_pusa.csv','psam_pusb.csv'):
        with z.open(name) as f:
            r=csv.reader(io.TextIOWrapper(f, encoding='latin-1')); h=next(r); I={k:i for i,k in enumerate(h)}
            iS=I['SERIALNO']; iA=I['AGEP']; iW=I['PWGTP']; iM=I['MIG']; iMS=I['MIGSP']; iP=I['POBP']; iST=I['STATE']; iR=I['RELSHIPP']; iMH=I['MARHM']; iMAR=I['MAR']
            for row in r:
                st=int(row[iST])
                if st>56: continue
                a=int(row[iA]); w=int(row[iW]); s=row[iS]
                hrec=hh.get(s)
                ten={'1':'own','2':'own','3':'rent','4':'nopay'}.get(hrec[0],'gq') if hrec else 'gq'
                mig=row[iM]
                if mig=='1': mc='stay'
                elif mig=='2': mc='abroad'
                elif mig=='3':
                    ms=int(row[iMS]) if row[iMS] else 0
                    if ms==st: mc='instate'
                    elif 1<=ms<=56: mc='interstate'
                    else: mc='abroad'
                else: mc='na'
                age_tab[a][(ten,mc)]+=w
                pob=int(row[iP]) if row[iP] else 0
                b=pob if 1<=pob<=56 else 0
                if a>=1: stock[(b, st)]+=w
                if mc=='interstate':
                    ag=0 if a<18 else 1 if a<25 else 2 if a<35 else 3 if a<50 else 4 if a<65 else 5
                    trip[(b, int(row[iMS]), st, ag)]+=w
                if row[iR]=='20' and hrec:
                    ag5=min(a//5*5, 85)
                    hhdur[(hrec[0], hrec[1], ag5)]+=hrec[2]
                if row[iMH]:
                    marr[(row[iMAR], row[iMH], mc, min(a//5*5,85))]+=w
pickle.dump(dict(age_tab=age_tab, trip=trip, stock=stock, hhdur=hhdur, marr=marr), open('/tmp/claude-1000/-home-player1-internot/98f822bb-8bcc-4b5e-b572-b6d08a6e2224/scratchpad/pums/agg.pkl','wb'))
print('done', file=sys.stderr)
