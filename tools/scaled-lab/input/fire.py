import numpy as np
from rot import RATES,T,NS
rng=np.random.default_rng(2)
DS=np.array([0.05,0.1,0.2,0.3,0.5,0.75,1.0,1.5,2.0])
def orig(d,u,N):
    x=d-u*T
    return np.where(x>0,np.floor(x/(N*T))+1,0).astype(np.int64)
def origk(u,k,N): return (u+k*N)*T
def scaled(d,phi,R,N,a0fn,kmax=4):
    f=int(round(65536*30/R)); P=65536; ta=phi/R
    nh=np.maximum(np.where(d>ta,np.ceil((d-ta)*R-1e-9),0).astype(np.int64),1)
    cnt=np.ones_like(d,dtype=np.int64); acc=np.full(d.shape,a0fn(f,N),dtype=np.int64)
    times=[ta.copy()]
    tk=[np.full(d.shape,np.nan) for _ in range(kmax)]; tk[0]=ta.copy()
    for j in range(1,int(nh.max())):
        m=j<nh; acc=acc+np.where(m,f,0); r=m&(acc>=N*P)
        for k in range(1,kmax):
            tk[k]=np.where(r&(cnt==k),ta+j/R,tk[k])
        cnt+=r; acc-=np.where(r,N*P,0)
    return cnt,tk
cands={'a0=0':lambda f,N:0,'a0=f-P/2':lambda f,N:f-32768,'a0=f-P/2 clip>=0 @30':lambda f,N:f-32768}
if __name__=="__main__":
    import sys
    for N in (6,4,2):
        print("== N",N)
        for name,fn in [('a0=0',cands['a0=0']),('a0=f-P/2',cands['a0=f-P/2']),('a0=(f-P/2)*N',lambda f,N:(f-32768)*N),('a0=(f-P)*N/2... ',lambda f,N:(f-65536)*N//2+32768*0)]:
            row=[]
            for R in RATES:
                D=np.repeat(DS,NS)+rng.random(len(DS)*NS)*N*T; u=rng.random(D.shape); phi=rng.random(D.shape)
                oc=orig(D,u,N); sc,_=scaled(D,phi,R,N,fn)
                diff=(sc-oc).reshape(len(DS),NS)
                big=D.reshape(len(DS),NS)>0.3
                row.append(f"{R}:{diff.mean():+.2f}/{np.abs(diff).max()}")
            print(name.ljust(18)," ".join(row))
