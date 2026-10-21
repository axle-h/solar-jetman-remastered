from sim import *
D=60
def T():
    tr={}
    for H in (60,75,120,144,165,240): tr[f"vsync {H}"]=vsync_trace(lambda t,H=H:H,D)
    tr["vsync 59.94"]=vsync_trace(lambda t:59.94,D)
    tr["vsync 143.86"]=vsync_trace(lambda t:143.86,D)
    tr["144 -> 72 load (20-30s)"]=vsync_trace(lambda t:144,D,extra=lambda t:20<=t<30)
    tr["VRR 70-140"]=vrr_trace(D)
    tr["vsync off 300"]=uncapped_trace(D,300)
    tr["vsync off 1000"]=uncapped_trace(D,1000)
    tr["drag 60<->144 (20-40s)"]=drag_trace(D)
    tr["sleep 30s at t=20 (60Hz)"]=gap_edit(vsync_trace(lambda t:60,D),20,30.0)
    tr["tab throttled 1Hz 20-30s"]=throttle_trace(D)
    return tr
if __name__=="__main__":
    tr=T()
    for name,t in tr.items():
        vs = not name.startswith(("vsync off","VRR"))
        for pol in ('policy',):
            c=Ctl(vsync=vs)
            r=run(t,c,policy=pol)
            print(summarize(name,r))
