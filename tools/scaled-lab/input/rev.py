import numpy as np
from rot import RATES,T
rng=np.random.default_rng(3)
n=20000
def run(R,g,reset_on_gap=True,N=n):
    d1=0.1+rng.random(N)*0.4; d2=0.1+rng.random(N)*0.4; u=rng.random(N); phi=rng.random(N)
    # original: sample times (u+k)T
    K=np.arange(0,60)[:,None]; st=(u[None,:]+K)*T
    Lc=((st<d1)).sum(0); Rc=((st>=d1+g)&(st<d1+g+d2)).sum(0)
    orig=Rc-Lc
    f=int(round(65536*30/R)); P=65536; a0=f-32768
    tot=d1+g+d2; J=int(tot.max()*R)+3
    net=np.zeros(N,dtype=np.int64); acc=np.zeros(N,dtype=np.int64); prev=np.zeros(N,bool)
    for j in range(J):
        t=(phi+j)/R
        L=t<d1; Rr=(t>=d1+g)&(t<d1+g+d2); anyh=L|Rr
        press=anyh&~prev
        acc=np.where(press,a0,np.where(anyh,acc+f,acc))
        rot=press|(anyh&~press&(acc>=P))
        acc=np.where(rot&~press,acc-P,acc)
        net+=np.where(rot,np.where(L,-1,1),0)
        prev=anyh
    return net-orig, orig
for g in (0,0.01,0.02,0.04,0.08):
    print("gap ms",g*1000, " ".join(f"{R}:{(lambda d:(d.mean(),np.abs(d).max(),(np.abs(d)>1).mean()))(run(R,g)[0])[0]:+.2f}/{np.abs(run(R,g)[0]).max()}" for R in RATES))
