import numpy as np, sys
rng=np.random.default_rng(1)
RATES=[30,60,75,90,100,120,144,165,200,137]
DS=np.array([0.01,0.02,0.033,0.05,0.075,0.1,0.15,0.2,0.3,0.4,0.5,0.75,1.0,1.5,2.0])
NS=10000
T=1/30
def orig_count(d,u):
    # samples at (u+k)T, in [0,d)
    x=d-u*T
    return np.where(x>0,np.floor(x/T)+1,0).astype(np.int64)
def scaled(d,phi,R,a0fn):
    f=int(round(65536*30/R)); P=65536
    ta=phi/R
    nh=np.where(d>ta,np.ceil((d-ta)*R-1e-9),0).astype(np.int64)
    nh=np.maximum(nh,1)  # latched tap -> one tick
    cnt=np.ones_like(d,dtype=np.int64)
    acc=np.full(d.shape,a0fn(f),dtype=np.int64)
    for j in range(1,int(nh.max())):
        m=j<nh
        acc=acc+np.where(m,f,0)
        r=m&(acc>=P)
        cnt+=r; acc-=np.where(r,P,0)
    return cnt,ta
a0s={'naive a0=0':lambda f:0,
     'f/2-32768':lambda f:f//2-32768,
     'f-32768 (inexact@30)':lambda f:f-32768,
     'f/2-32768 +f/4':lambda f:f//2-32768+f//4,
     'seed -P/2 (inexact)':lambda f:-32768}
def run(name,a0fn,verbose=False):
    out={}
    for R in RATES:
        D=np.repeat(DS,NS)+rng.random(len(DS)*NS)*T; u=rng.random(D.shape); phi=rng.random(D.shape)
        oc=orig_count(D,u)
        sc,ta=scaled(D,phi,R,a0fn)
        diff=(sc-oc).reshape(len(DS),NS)
        out[R]=(diff,ta)
    return out
if __name__=="__main__":
    for name,fn in a0s.items():
        print("==",name)
        print("R   "+" ".join(f"d={d:<5}" for d in DS[[3,5,8,10,12,14]]))
        for R in RATES:
            diff,_=run(name,fn)[R] if False else (None,None)
        res=run(name,fn)
        for R in RATES:
            diff=res[R][0]
            print(f"{R:<4}"+" ".join(f"{diff[i].mean():+.2f}/{diff[i].std():.2f}" for i in [3,5,8,10,12,14]),f" all|diff|>1: {(np.abs(diff[5:])>1).mean():.3f}")
