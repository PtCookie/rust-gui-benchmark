#!/usr/bin/env python3
"""Benchmark harness: runs each framework prototype under Xvfb and records
startup time, process-tree PSS memory, CPU time and the in-app frame stats."""
import json, os, subprocess, sys, time, statistics, resource, signal

ROOT = os.path.dirname(os.path.abspath(__file__))
DATA = f"{ROOT}/data"
TARGET = f"{ROOT}/target/release"
RES = f"{ROOT}/results"
os.makedirs(RES, exist_ok=True)

FRAMEWORKS = {
    "electron": dict(cmd=[f"{ROOT}/electron/node_modules/.bin/electron", "--no-sandbox", f"{ROOT}/electron/main.cjs"], env={}),
    "tauri": dict(cmd=[f"{TARGET}/bench-tauri"], env={"WEBKIT_DISABLE_DMABUF_RENDERER": "1"}),
    "slint": dict(cmd=[f"{TARGET}/bench-slint"], env={"SLINT_BACKEND": "winit-software"}),
    "gpui": dict(cmd=[f"{TARGET}/bench-gpui"], env={"BENCH_FRAMES": "200", "VK_ICD_FILENAMES": "/usr/share/vulkan/icd.d/lvp_icd.json", "ZED_ALLOW_EMULATED_GPU": "1"}),
}
DISPLAY = os.environ.get("BENCH_DISPLAY", ":97")


def tree_pids(root):
    """all descendants of root (including root) via /proc ppid links"""
    kids = {}
    for p in os.listdir("/proc"):
        if not p.isdigit():
            continue
        try:
            with open(f"/proc/{p}/stat") as f:
                s = f.read()
            ppid = int(s[s.rindex(")") + 2:].split()[1])
            kids.setdefault(ppid, []).append(int(p))
        except Exception:
            pass
    out, stack = [], [root]
    while stack:
        x = stack.pop()
        out.append(x)
        stack.extend(kids.get(x, []))
    return out


def cpu_s(pids):
    tick = os.sysconf("SC_CLK_TCK")
    tot = 0
    for p in pids:
        try:
            with open(f"/proc/{p}/stat") as f:
                st = f.read()
            fl = st[st.rindex(")") + 2:].split()
            tot += (int(fl[11]) + int(fl[12])) / tick  # utime + stime
        except Exception:
            pass
    return tot


def pss_kb(pids):
    tot = 0
    for p in pids:
        try:
            with open(f"/proc/{p}/smaps_rollup") as f:
                for line in f:
                    if line.startswith("Pss:"):
                        tot += int(line.split()[1])
                        break
        except Exception:
            pass
    return tot


def run_one(fw, scenario, idx, timeout=240):
    cfg = FRAMEWORKS[fw]
    out = f"{RES}/{fw}-{scenario}-{idx}.json"
    for f in (out, out + ".ready"):
        if os.path.exists(f):
            os.remove(f)
    env = dict(os.environ)
    env.update(cfg["env"])
    env.update(DISPLAY=DISPLAY, BENCH_SCENARIO=scenario, BENCH_OUT=out,
               BENCH_REPO=f"{DATA}/git-blobless.git", BENCH_DB=f"{DATA}/events.db")
    ru0 = resource.getrusage(resource.RUSAGE_CHILDREN)
    t0 = time.monotonic()
    log = open(f"{RES}/{fw}-{scenario}-{idx}.log", "w")
    p = subprocess.Popen(cfg["cmd"], env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
    ready_ms = None
    samples_before, samples_after = [], []
    cpu_at_ready = None
    cpu_max = 0.0  # running max of the tree's summed CPU (helper processes may exit between samples)
    last_sample = 0
    while True:
        now = time.monotonic()
        if ready_ms is None and os.path.exists(out + ".ready"):
            ready_ms = (now - t0) * 1000
            cpu_at_ready = cpu_s(tree_pids(p.pid))
        if now - last_sample >= 0.2:
            last_sample = now
            pids = tree_pids(p.pid)
            v = pss_kb(pids)
            (samples_after if ready_ms is not None else samples_before).append(v)
            cpu_max = max(cpu_max, cpu_s(pids))
        if p.poll() is not None:
            break
        if now - t0 > timeout:
            os.killpg(p.pid, signal.SIGKILL)
            p.wait()
            break
        time.sleep(0.005)
    total_s = time.monotonic() - t0
    try:
        os.killpg(p.pid, signal.SIGKILL)  # reap stragglers (helper processes)
    except Exception:
        pass
    ru1 = resource.getrusage(resource.RUSAGE_CHILDREN)
    res = {}
    if os.path.exists(out):
        res = json.load(open(out))
    cpu_total = (ru1.ru_utime + ru1.ru_stime) - (ru0.ru_utime + ru0.ru_stime)
    frames = res.get("frames") or 900
    res.update(
        cpu_at_ready_s=cpu_at_ready,
        cpu_run_s=(cpu_max - cpu_at_ready) if cpu_at_ready is not None else None,
        cpu_ms_per_frame=((cpu_max - cpu_at_ready) / frames * 1000) if cpu_at_ready is not None else None,
        fw=fw, scenario=scenario, run=idx, exit_code=p.returncode,
        ready_ms=ready_ms, total_s=total_s,
        cpu_s=(ru1.ru_utime + ru1.ru_stime) - (ru0.ru_utime + ru0.ru_stime),
        pss_peak_mb=max(samples_before + samples_after, default=0) / 1024,
        pss_after_ready_median_mb=(statistics.median(samples_after) / 1024) if samples_after else None,
    )
    json.dump(res, open(out, "w"), indent=2)
    return res


def main():
    fws = sys.argv[1].split(",") if len(sys.argv) > 1 and sys.argv[1] != "all" else list(FRAMEWORKS)
    scs = sys.argv[2].split(",") if len(sys.argv) > 2 else ["commits", "grid"]
    n = int(sys.argv[3]) if len(sys.argv) > 3 else 3
    xvfb = subprocess.Popen(["Xvfb", DISPLAY, "-screen", "0", "1280x800x24", "-nolisten", "tcp"],
                            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    time.sleep(1.0)
    wm = None
    if os.environ.get("BENCH_WM", "openbox") != "none":
        # a window manager is required: GPUI's X11 frame loop only starts after MapNotify
        wm = subprocess.Popen([os.environ.get("BENCH_WM", "openbox")], env=dict(os.environ, DISPLAY=DISPLAY),
                              stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        time.sleep(1.5)
    try:
        for fw in fws:
            for sc in scs:
                for i in range(1, n + 1):
                    r = run_one(fw, sc, i)
                    print(f"{fw:9s} {sc:8s} #{i} exit={r.get('exit_code')} ready={r.get('ready_ms') and round(r['ready_ms'])}ms "
                          f"avg_int={r.get('interval_avg_ms') and round(r['interval_avg_ms'], 2)} "
                          f"p95={r.get('interval_p95_ms') and round(r['interval_p95_ms'], 1)} "
                          f"pss_peak={round(r['pss_peak_mb'])}MB err={str(r.get('error', ''))[:80]}", flush=True)
    finally:
        if wm:
            wm.terminate()
        xvfb.terminate()


if __name__ == "__main__":
    main()
