import pandas as pd
cols=['year','age','cohort','sex','race','hispanic','educ','paeduc','maeduc','speduc','coeduc','degree','padeg','madeg','spdeg','codeg','marital','wtssall','wtssps','born','parborn','sibs','reg16','oversamp','formwt']
df=pd.read_stata('/home/player1/internot/datasets/education/transmission/gss/GSS_stata/gss7224_r3a.dta',columns=cols,convert_categoricals=False,convert_missing=False)
df.to_parquet('gss_sub.parquet')
print(df.shape); print(df.describe().T.to_string())
