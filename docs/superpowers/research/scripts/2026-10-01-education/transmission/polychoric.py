import numpy as np, pandas as pd
from scipy.stats import norm, multivariate_normal
from scipy.optimize import minimize_scalar
def bvn_cdf(a,b,r):
    a=np.clip(a,-9,9); b=np.clip(b,-9,9)
    return multivariate_normal(mean=[0,0],cov=[[1,r],[r,1]]).cdf([a,b])
def cells(tab,r):
    t=np.asarray(tab,float); p=t/t.sum()
    ra=np.concatenate([[-9],norm.ppf(np.cumsum(p.sum(1))[:-1]),[9]])
    cb=np.concatenate([[-9],norm.ppf(np.cumsum(p.sum(0))[:-1]),[9]])
    F=np.array([[bvn_cdf(ra[i],cb[j],r) for j in range(len(cb))] for i in range(len(ra))])
    P=F[1:,1:]-F[:-1,1:]-F[1:,:-1]+F[:-1,:-1]
    return np.maximum(P,1e-12)
def polychoric(tab):
    t=np.asarray(tab,float)
    res=minimize_scalar(lambda r: -(t*np.log(cells(t,r))).sum(), bounds=(-0.99,0.99), method='bounded', options={'xatol':1e-4})
    return res.x
def fit_report(tab, rowlab, collab):
    t=np.asarray(tab,float); r=polychoric(t); P=cells(t,r)
    obs=t/t.sum(1,keepdims=True)*100; fit=P/P.sum(1,keepdims=True)*100
    df=pd.DataFrame(np.round(obs,1),index=rowlab,columns=collab); df2=pd.DataFrame(np.round(fit,1),index=rowlab,columns=collab)
    return r, df, df2
