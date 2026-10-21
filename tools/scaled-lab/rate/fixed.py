from traces import *
def trace_for(H,dur=30,seed=5): return vsync_trace(lambda t:H,dur,seed=seed)
print("fixed rate on display (30 s, vsync, jitter 0.3ms, 0.3% misses) -- judder rms/p99/max ms ; ticks/frame pattern")
rows=[(60,144),(75,144),(100,144),(120,144),(144,144),(165,144),(200,144),(144,60),(120,60),(200,60),(90,60),(100,60),(60,240),(120,240),(200,240),(137,144),(60,59.94),(60,60)]
for R,H in rows:
    out=[]
    for pol in ('hold','interp'):
        for mode in ('lock','wall'):
            c=Ctl(vsync=True,fixed=R,snap=mode); r=run(trace_for(H),c,policy=pol)
            e=sorted(abs(x) for x in r['err']); rms=math.sqrt(sum(x*x for x in e)/len(e))
            out.append(f"{pol[0]}/{mode[0]} {rms:5.2f}/{e[int(len(e)*.99)]:5.2f}/{e[-1]:5.2f}")
    print(f"R={R:3d} on {H:6}: "+" | ".join(out))
