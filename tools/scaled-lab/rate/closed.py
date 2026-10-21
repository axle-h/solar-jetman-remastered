from traces import *
def closed(H,render,tick,dur=60,jit=0.0003,seed=7):
    def gen(ctl):
        rnd=random.Random(seed); T=1/H; p=0.0
        while p<dur:
            ready=p+getattr(ctl,'lastwork',0.0)
            k=max(1,math.ceil(ready/T-1e-9)-round(p/T)) if True else 1
            p=(round(p/T)+k)*T
            yield (p,p+rnd.uniform(-jit,jit))
    return gen
import sys
def go(name,H,render,tick):
    c=Ctl(vsync=True)
    wt=tick
    r=run(closed(H,render,tick),c,work_tick=tick,work_frame=render)
    print(summarize(name,r))
    return r
print("closed loop: vsync display, frame work = render + ticks*cost")
go("144Hz render 2ms tick 0.5us",144,0.002,5e-7)
go("144Hz render 9ms (GPU slow)",144,0.009,5e-7)
go("144Hz render 4ms tick 3ms (slow wasm)",144,0.004,0.003)
go("144Hz render 6ms tick 6ms (overload)",144,0.006,0.006)
go("60Hz render 12ms tick 6ms (overload)",60,0.012,0.006)
# harsh jitter
for H,j in ((144,0.001),(240,0.001),(60,0.002)):
    c=Ctl(vsync=True); r=run(vsync_trace(lambda t:H,60,jit=j),c); print(summarize(f"vsync {H} jitter +-{j*1000:g}ms",r))
# 59.94 variants
for mode in ('lock','wall'):
    c=Ctl(vsync=True,snap=mode); r=run(vsync_trace(lambda t:59.94,300),c); print(mode,summarize("59.94 x 300 s",r))
