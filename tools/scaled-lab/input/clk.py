import numpy as np
from rot import RATES,T
print("rate | f(Q16) | 1h drift Q16 ms | 1h drift Q32 ms | 30Hz event delay mean/max ms | 60Hz-frame delay max ms | dur 48 steps err(ms) mean/max | 96 | tap latency orig mean/max vs scaled mean/max ms")
for R in RATES:
    f16=round(65536*30/R); f32=round((1<<32)*30/R)
    ticks=3600*R
    d16=(ticks*f16/65536)/30-3600; d32=(ticks*f32/2**32)/30-3600
    # 30Hz events: gt after tick n (Q16) = n*f16 ; event k fires at first n with n*f16>=k*65536
    n=np.arange(1,int(10*R)+1); gt=n*f16
    k=np.arange(1,301)
    tick_of=np.searchsorted(gt,k*65536,side='left')+1   # tick index (1-based)
    tt=tick_of/R; ideal=k/30; dl=(tt-ideal)*1000
    # frame events 60Hz
    k2=np.arange(1,601); tick2=np.searchsorted(gt,k2*32768,side='left')+1; dl2=(tick2/R-k2/60)*1000
    # durations with random start phase: start at tick s (timer set), expire when acc>=D*65536 : error vs D/30 s measured from tick time of start
    out=[]
    for D in (48,96):
        s=np.random.default_rng(0).integers(0,R,400)
        # elapsed ticks m minimal with m*f16 >= D*65536 (acc started 0 at the set tick)
        m=int(np.ceil(D*65536/f16)); err=(m/R - D/30)*1000
        out.append(f"{err:+.2f}")
    # latency of press: orig mean 16.7 max 33.3 ; scaled mean .5/R
    print(f"{R:>3} | {f16:>5} | {d16*1000:+.2f} | {d32*1000:+.4f} | {dl.mean():.2f}/{dl.max():.2f} | {dl2.max():.2f} | {out[0]} | {out[1]} | 16.7/33.3 vs {500/R:.1f}/{1000/R:.1f}")
