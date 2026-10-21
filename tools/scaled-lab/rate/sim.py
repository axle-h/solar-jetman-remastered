from fractions import Fraction
import random, statistics, math, sys, bisect
LIST=[60,75,90,100,120,144,165,200]
CAP=200; RMIN=60
SNAP_TOL=0.015
# ---------------- controller constants ----------------
WIN=1.0; MINN=5; EVAL=0.1; TIGHT=0.6; RES_MAX=0.2; PLL=0.05; SNAP_MAX=0.35
T_DOWN=1.0; T_UP=2.0; DEAD=0.12
MAXDT=0.1; BUSY_HI=0.8; BUSY_LOCK=20.0; STEP_MIN=2.0
LOCK_TOL=0.3; LONGWIN=4.0; T_DOWN_U=6.0; T_UP_U=1.5

class Ctl:
    def __init__(s, vsync=True, fixed=None, snap='lock'):
        roundto=5; s.roundto=5
        s.vsync=vsync; s.fixed=fixed; s.mode=snap  # 'lock' | 'wall' | 'exact'
        s.R=fixed or RMIN; s.fails=0; s.grid=None; s.Tg=None; s.resid=[]; s.blocks=0; s.win=[]; s.longwin=[]; s.next_eval=0; s.cand=None; s.cand_t=0; s.started=False
        s.Hnom=60; s.Hest=60.0; s.locked=False; s.hist=[]; s.busy_w=[]; s.block_up_until=-1; s.last_step=-9
        s.changes=[]; s.fps=60
    def sample(s, now, dt, work):
        if 0<dt<=0.25: s.win.append((now,dt,work)); s.longwin.append((now,dt,work))
        while len(s.longwin)>MINN and s.longwin[0][0]<now-LONGWIN: s.longwin.pop(0)
        while len(s.win)>MINN and s.win[0][0]<now-WIN: s.win.pop(0)
    def snap(s, now):
        """phase-locked vsync grid: number of display refreshes since the last frame, or None"""
        if s.Tg is None or s.grid is None or now-s.grid>MAXDT: s.grid=now; return None
        n=max(1,round((now-s.grid)/s.Tg)); err=(now-s.grid)-n*s.Tg
        s.resid.append((now,err/s.Tg))
        while s.resid and s.resid[0][0]<now-WIN: s.resid.pop(0)
        if abs(err)>SNAP_MAX*s.Tg: s.grid=now; s.fails+=1; return None
        s.fails=0
        s.grid+=n*s.Tg+PLL*err
        return n
    def candidate(s, now):
        ds=sorted(d for _,d,_ in s.win)
        n=len(ds); M=ds[n//2]; p10=ds[n//10]; p90=ds[min(n-1,(n*9)//10)]
        H=1/M; s.fps=H; s.Hest=H
        tight=(p90-p10)<=TIGHT*M
        while s.resid and s.resid[0][0]<now-WIN: s.resid.pop(0)
        rms=math.sqrt(sum(e*e for _,e in s.resid)/len(s.resid)) if len(s.resid)>=MINN else 0.0
        s.locked = s.vsync and tight and rms<RES_MAX
        if s.vsync and tight: s.Tg=1/H
        else: s.Tg=None
        busy=sum(w for *_,w in s.win)/max(1e-9,sum(d for _,d,_ in s.win))
        if s.locked:
            near=[L for L in LIST+[240,360] if abs(H-L)/L<SNAP_TOL]
            Hn=near[0] if near else round(H)
            s.Hnom=Hn
            if Hn>CAP: k=math.ceil(Hn/CAP); R=Hn//k
            elif Hn<RMIN: R=Hn*math.ceil(RMIN/Hn)
            else: R=Hn
            return R,busy
        else:
            ds2=sorted(d for t,d,_ in s.longwin)
            peak=1/ds2[max(0,len(ds2)//10)]    # 90th-percentile frame rate over 4 s
            R=next((L for L in LIST if L>=peak*0.97),CAP)   # smallest list rate not below the peak
            return R,busy
    def frame(s, now, dt, work):
        """returns R in force for this frame's ticks"""
        s.sample(now,dt,work)
        if s.fails>=6:                      # display rate changed under us: re-measure from the newest frames
            s.win=s.win[-6:]; s.resid=[]; s.next_eval=now; s.fails=0
        if s.fixed is None and now>=s.next_eval and len(s.win)>=MINN:
            s.next_eval=now+EVAL
            cand,busy=s.candidate(now)
            if not s.started and now>=WIN*0.5:
                s.started=True; s.set(now,cand)
            elif s.started:
                # falling behind: step down a notch
                if busy>BUSY_HI and now-s.last_step>STEP_MIN and s.R>RMIN:
                    lower=[L for L in LIST if L<s.R]
                    s.last_step=now; s.block_up_until=now+BUSY_LOCK*3**s.blocks; s.blocks=min(s.blocks+1,3)
                    s.set(now,lower[-1] if lower else RMIN); s.cand=None
                else:
                    s.debounce(now,cand)
        elif s.fixed is not None and now>=s.next_eval and len(s.win)>=MINN:
            s.next_eval=now+EVAL; s.candidate(now)
        return s.R
    def debounce(s,now,cand):
        R=s.R
        if cand==R or (not s.locked and abs(cand-R)<DEAD*R and cand<=R*1.0001+ (R*DEAD)):
            s.cand=None; return
        if s.cand is None or abs(cand-s.cand)>(0.003*s.cand if s.locked else DEAD*s.cand):
            s.cand=cand; s.cand_t=now; return
        need=(T_DOWN if cand<R else T_UP) if s.locked else (T_DOWN_U if cand<R else T_UP_U)
        if cand>R and now<s.block_up_until: return
        if now-s.cand_t>=need: s.set(now,cand); s.cand=None
    def set(s,now,R):
        if R!=s.R: s.changes.append((now,R))
        s.R=R

# ---------------- display / trace generators ----------------
# each yields present times p (truth) and measured m = p + noise
def vsync_trace(H_of_t, dur, jit=0.0003, miss=0.003, seed=1, extra=None):
    """H_of_t(t) -> refresh Hz. Present at refresh grid; extra(t)-> forced-late factor (frames take 2 refreshes)"""
    rnd=random.Random(seed); t=0.0; out=[]; t_next=0.0
    while t<dur:
        H=H_of_t(t); T=1/H
        k=1
        if rnd.random()<miss: k=2
        if extra and extra(t): k=max(k,2)
        t+=k*T
        out.append((t, t+rnd.uniform(-jit,jit)))
    return out
def gap_edit(tr, t0, gap):
    out=[]
    for p,m in tr:
        if p>=t0: out.append((p+gap,m+gap))
        else: out.append((p,m))
    return out
def vrr_trace(dur, lo=70, hi=140, seed=2):
    rnd=random.Random(seed); t=0.0; out=[]
    while t<dur:
        fps=(lo+hi)/2+(hi-lo)/2*math.sin(2*math.pi*t/9.0)*0.8+rnd.uniform(-8,8)
        fps=min(hi,max(lo,fps)); t+=1/fps
        out.append((t,t+rnd.uniform(-0.0001,0.0001)))
    return out
def uncapped_trace(dur, fps, seed=3, spike=0.01):
    rnd=random.Random(seed); t=0.0; out=[]
    while t<dur:
        d=1/fps*rnd.uniform(0.8,1.2)
        if rnd.random()<spike: d*=rnd.uniform(5,30)
        t+=d; out.append((t,t))
    return out
def drag_trace(dur=60):
    def H(t): return 144 if 20<=t<40 else 60
    return vsync_trace(H,dur)
def throttle_trace(dur=60):
    out=[]; t=0.0
    while t<dur:
        T=1.0 if 20<=t<30 else 1/60
        t+=T; out.append((t,t+random.uniform(-0.0003,0.0003)))
    return out

# ---------------- game loop ----------------
def run(trace, ctl, work_tick=0.0000005, work_frame=0.0004, policy='policy', verbose=False):
    """Replays a trace through ctl. returns dict of metrics"""
    prev_m=None; prev_p=0.0
    S=0.0           # game time (s), exact (sum 1/R)
    acc=Fraction(0)
    unit=None
    Rlog=[]; tpf=[]; err=[]; Dprev=None; pprev=None
    Rprev=None; ticks_total=0; extra_frames=0; zero_frames=0; burst=0; excess=0
    rem_time=0.0
    series=[]
    gamet0=None
    it = trace(ctl) if callable(trace) else iter(trace)
    for i,(p,m) in enumerate(it):
        dt_m = (m-prev_m) if prev_m is not None else 1/60
        dt_p = (p-prev_p) if prev_m is not None else 1/60
        prev_m=m; prev_p=p
        R=ctl.frame(m, dt_m, getattr(ctl,'lastwork',0.0))
        # game dt
        dtg=min(dt_m,MAXDT) if dt_m>0 else 0
        n=ctl.snap(m) if (ctl.mode=='lock') else None
        locked_dt = (n is not None and ctl.locked)
        if Rprev is not None and R!=Rprev:
            acc=acc*Fraction(R,Rprev)   # keep the fraction of a tick elapsed in time terms
        Rprev=R
        if locked_dt:
            acc+=Fraction(n*R,ctl.Hnom)   # ticks per vsync frame = R/Hnom, exact
        else:
            acc+=Fraction(int(round(dtg*1e6))*R,1_000_000)
        nt=int(acc); acc-=nt
        den=1
        S+=nt/R
        rem=float(acc)/R       # time remainder in seconds
        interp = ((not ctl.locked) or not (R % ctl.Hnom == 0 or ctl.Hnom % R == 0)) if policy=='policy' else (policy=='interp')
        D=S+rem-1/R if interp else S
        if gamet0 is None: gamet0=D
        if Dprev is not None:
            err.append(((D-Dprev)-dt_p)*1000 if dt_p<MAXDT else None)
        else: err.append(None)
        Dprev=D; 
        tpf.append(nt); Rlog.append((m,R))
        need=math.ceil(dt_p*R-1e-9) if dt_p<MAXDT else math.ceil(MAXDT*R)
        if p>2.0 and nt>need: excess+=1
        if p>2.0 and nt==0 and R>=1/max(dt_p,1e-9)*0.99: zero_frames+=1
        burst=max(burst,nt)
        ctl.lastwork=work_frame+nt*work_tick
        series.append((p,R,nt,D,dt_p,interp,m))
    return dict(Rlog=Rlog,tpf=tpf,errt=[(ser[0],e) for ser,e in zip(series,err)],err=[e for ser,e in zip(series,err) if e is not None and ser[0]>2.0],series=series,ctl=ctl,excess=excess,zero=zero_frames,burst=burst,nframes=len(series))

def summarize(name, res, t_events=()):
    series0=res['series'][0][0]-res['series'][0][4]
    ctl=res['ctl']; rl=res['Rlog']
    chg=ctl.changes
    err=sorted(abs(e) for e in res['err'])
    rms=math.sqrt(sum(e*e for e in err)/max(1,len(err)))
    p99=err[int(len(err)*0.99)] if err else 0
    # time of first/last change
    final=rl[-1][1]
    settle=chg[-1][0] if chg else 0
    chtxt=" ".join(f"{t:.1f}s->{r:g}" for t,r in chg[:12])+("..." if len(chg)>12 else "")
    lag=[(x[0]-series0)-x[3] for x in res["series"] if x[0]>3.0]
    lagm=1000*sum(lag)/max(1,len(lag))
    return f"{name:34} chg={len(chg):2d} lastchg={settle:5.1f}s  jit rms={rms:5.2f} p99={p99:5.2f} max={(err[-1] if err else 0):6.2f}ms  ticks/f max={res['burst']:3d} excess={res['excess']:5d} zero={res['zero']:5d} | {chtxt}"
