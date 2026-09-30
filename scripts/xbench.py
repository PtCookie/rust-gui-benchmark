#!/usr/bin/env python3
"""Cross-platform benchmark harness (Linux / macOS / Windows), psutil based.

Runs one framework prototype for each scenario N times and records startup time,
process-tree memory/CPU and the in-app frame statistics. On Linux run it under
Xvfb + a window manager (see ci_inner.sh)."""
import argparse, glob, json, os, platform, statistics, subprocess, sys, time
from pathlib import Path
import psutil

ROOT = Path(__file__).resolve().parent.parent
OS = platform.system()  # Linux | Darwin | Windows
EXE = ".exe" if OS == "Windows" else ""


def esc(s):
    return s.replace("%", "%25").replace("\r", "%0D").replace("\n", "%0A")


def annotate(level, title, msg):
    t = esc(title).replace(",", "%2C").replace(":", "%3A")
    print(f"::{level} title={t}::{esc(msg)}", flush=True)


def app_cmd(fw, target_dir):
    if fw == "electron":
        d = ROOT / "electron" / "node_modules" / "electron" / "dist"
        exe = {"Linux": d / "electron", "Darwin": d / "Electron.app" / "Contents" / "MacOS" / "Electron",
               "Windows": d / "electron.exe"}[OS]
        cmd = [str(exe)]
        if OS == "Linux":
            cmd.append("--no-sandbox")
        return cmd + [str(ROOT / "electron" / "main.cjs")]
    sub = {"tauri": "tauri/src-tauri", "slint": "slint-app", "slint-skia": "slint-app", "gpui": "gpui-app"}[fw]
    name = {"tauri": "bench-tauri", "slint": "bench-slint", "slint-skia": "bench-slint", "gpui": "bench-gpui"}[fw]
    tdir = "target-skia" if fw == "slint-skia" else "target"  # slint-skia: cargo build --features skia --target-dir slint-app/target-skia
    base = Path(target_dir) if target_dir else ROOT / sub / tdir / "release"
    return [str(base / (name + EXE))]


def app_env(fw, real_gpu=False):
    env = {}
    if fw == "slint":
        env["SLINT_BACKEND"] = "winit-software"
    if fw == "slint-skia":
        env["SLINT_BACKEND"] = "winit-skia"
    if fw == "electron" and not real_gpu:
        env["BENCH_DISABLE_GPU"] = "1"  # CI runners have no GPU
    if fw == "gpui" and not real_gpu:
        env["BENCH_FRAMES"] = "200"  # software rasterisation is slow; keep runs short
        env["ZED_ALLOW_EMULATED_GPU"] = "1"
        icds = glob.glob("/usr/share/vulkan/icd.d/lvp_icd*.json")
        if OS == "Linux" and icds:
            env["VK_ICD_FILENAMES"] = icds[0]
    return env


def webkit_helpers(baseline):
    """macOS: WKWebView runs WebContent/GPU/Networking as XPC services that are NOT children of the app.
    Count the ones that appeared after `baseline` (pids seen before launch) as belonging to the app."""
    out = []
    for q in psutil.process_iter(["pid", "name"]):
        try:
            if q.info["pid"] not in baseline and (q.info["name"] or "").startswith("com.apple.WebKit."):
                out.append(q)
        except (psutil.Error, OSError):
            pass
    return out


def sample(proc, baseline=None):
    rss = pss = cpu = 0.0
    try:
        procs = [proc] + proc.children(recursive=True)
    except psutil.Error:
        return rss, pss, cpu, []
    if baseline is not None:
        procs += [q for q in webkit_helpers(baseline) if q not in procs]
    for q in procs:
        try:
            m = q.memory_full_info() if OS == "Linux" else q.memory_info()
            rss += m.rss
            pss += getattr(m, "pss", 0)
            t = q.cpu_times()
            cpu += t.user + t.system
        except (psutil.Error, OSError):
            pass
    return rss / 2**20, pss / 2**20, cpu, procs


def kill_all(procs):
    for q in procs:
        try:
            q.kill()
        except (psutil.Error, OSError):
            pass


def run_one(fw, sc, i, a):
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    base = out / f"{a.tag}-{fw}-{sc}-{i}.json"
    ready = Path(str(base) + ".ready")
    for f in (base, ready):
        f.unlink(missing_ok=True)
    env = os.environ.copy()
    env.update(app_env(fw, getattr(a, "real_gpu", False)))
    env.update(dict(kv.split("=", 1) for kv in a.env))
    env.update(BENCH_SCENARIO="commits" if sc == "idle" else sc, BENCH_OUT=str(base), BENCH_REPO=str(Path(a.data) / "git-blobless.git"),
               BENCH_DB=str(Path(a.data) / "events.db"))
    if sc == "idle":
        env.update(BENCH_MODE="idle", BENCH_IDLE_SECS=str(a.idle_secs))
    logf = open(str(base) + ".log", "w")
    t0 = time.monotonic()
    baseline = {q.pid for q in psutil.process_iter()} if (fw == "tauri" and OS == "Darwin") else None
    try:
        p = subprocess.Popen(app_cmd(fw, a.target_dir), env=env, cwd=ROOT, stdout=logf, stderr=subprocess.STDOUT)
    except OSError as e:
        return {"fw": fw, "scenario": sc, "run": i, "exit_code": None, "error": f"spawn failed: {e}"}
    proc = psutil.Process(p.pid)
    ready_ms = cpu_at_ready = None
    cpu_max, last, timed_out, last_procs = 0.0, 0.0, False, []
    ready_t = cpu_max_t = None
    rss_after, pss_after, rss_all = [], [], []
    while True:
        now = time.monotonic()
        if ready_ms is None and ready.exists():
            ready_ms = (now - t0) * 1000
            cpu_at_ready = sample(proc, baseline)[2]
            ready_t = now
        if now - last >= 0.25:
            last = now
            rss, pss, cpu, procs = sample(proc, baseline)
            if procs:
                last_procs = procs
            rss_all.append(rss)
            if ready_ms is not None:
                rss_after.append(rss)
                pss_after.append(pss)
            if cpu > cpu_max:
                cpu_max, cpu_max_t = cpu, now
        if p.poll() is not None:
            break
        if now - t0 > a.timeout:
            timed_out = True
            kill_all(last_procs + [proc])
            p.wait()
            break
        time.sleep(0.01)
    total_s = time.monotonic() - t0
    kill_all(last_procs)  # helper processes that outlived the parent
    logf.close()
    res = json.load(open(base)) if base.exists() else {}
    frames = res.get("frames") or 900
    res.update(
        os=OS, arch=platform.machine(), fw=fw, scenario=sc, run=i, tag=a.tag,
        exit_code=p.returncode, timed_out=timed_out, ready_ms=ready_ms, total_s=total_s,
        rss_peak_mb=max(rss_all, default=0),
        rss_after_ready_median_mb=statistics.median(rss_after) if rss_after else None,
        pss_after_ready_median_mb=statistics.median(pss_after) if pss_after and OS == "Linux" else None,
        cpu_at_ready_s=cpu_at_ready,
        cpu_ms_per_frame=((cpu_max - cpu_at_ready) / frames * 1000) if (cpu_at_ready is not None and sc != "idle") else None,
    )
    if sc == "idle" and cpu_at_ready is not None and cpu_max_t is not None and cpu_max_t > ready_t:
        # CPU used between ready and the last sample, as % of one core while the app does nothing
        res["idle_cpu_pct"] = (cpu_max - cpu_at_ready) / (cpu_max_t - ready_t) * 100.0
        res["cpu_ms_per_frame"] = None
    iv = res.get("interval_avg_ms")
    if iv:
        res["fps"] = 1000.0 / iv
        if res.get("cpu_ms_per_frame") is not None:
            # CPU time per second of wall clock, in % of one core (100 = one core fully busy; >100 = multi-core)
            res["cpu_core_pct"] = res["cpu_ms_per_frame"] * res["fps"] / 10.0
    json.dump(res, open(base, "w"), indent=2)
    return res


def med(rows, k):
    v = [r[k] for r in rows if r.get(k) is not None]
    return statistics.median(v) if v else None


def fmt(x, d=1):
    return "-" if x is None else f"{x:.{d}f}"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--fw", required=True, choices=["electron", "tauri", "slint", "slint-skia", "gpui"])
    ap.add_argument("--scenarios", default="commits,grid,idle", help="commits, grid (scroll fling) and idle (do nothing after startup)")
    ap.add_argument("--idle-secs", type=float, default=10.0)
    ap.add_argument("--runs", type=int, default=2)
    ap.add_argument("--out", default="results")
    ap.add_argument("--data", default="data")
    ap.add_argument("--tag", default="default")
    ap.add_argument("--env", action="append", default=[], help="KEY=VALUE, repeatable")
    ap.add_argument("--target-dir", default=None)
    ap.add_argument("--real-gpu", action="store_true", help="real machine with a GPU: no software-rendering workarounds, full 900-frame GPUI run")
    ap.add_argument("--timeout", type=int, default=240)
    a = ap.parse_args()

    failed_scenarios, md = [], []
    for sc in a.scenarios.split(","):
        ok = []
        for i in range(1, a.runs + 1):
            r = run_one(a.fw, sc, i, a)
            good = r.get("exit_code") == 0 and "error" not in r and (r.get("idle_s") is not None if sc == "idle" else r.get("interval_avg_ms") is not None)
            print(f"[{a.fw}/{sc}/{a.tag}] run {i}: exit={r.get('exit_code')} timeout={r.get('timed_out')} "
                  f"ready={fmt(r.get('ready_ms'), 0)}ms frame_avg={fmt(r.get('interval_avg_ms'), 2)}ms idle_cpu={fmt(r.get('idle_cpu_pct'))}%", flush=True)
            if good:
                ok.append(r)
            else:
                logp = Path(a.out) / f"{a.tag}-{a.fw}-{sc}-{i}.json.log"
                tail = "\n".join(logp.read_text(errors="replace").splitlines()[-25:]) if logp.exists() else ""
                annotate("warning", f"run failed {a.fw}/{sc}/{OS}/{a.tag}",
                         f"exit={r.get('exit_code')} timed_out={r.get('timed_out')} err={r.get('error')}\n{tail}")
        if not ok:
            failed_scenarios.append(sc)
            continue
        m = {k: med(ok, k) for k in ("ready_ms", "core_load_ms", "interval_avg_ms", "interval_p95_ms",
                                     "interval_max_ms", "frames_over_33ms", "fps", "cpu_ms_per_frame", "cpu_core_pct", "idle_cpu_pct",
                                     "rss_after_ready_median_mb", "pss_after_ready_median_mb", "rss_peak_mb")}
        line = " ".join(f"{k}={fmt(v, 1)}" for k, v in m.items() if v is not None)
        annotate("notice", f"result {a.fw}/{sc}/{OS}/{platform.machine()}/{a.tag} n={len(ok)}", line)
        md.append(f"| {a.fw} | {sc} | {OS} | {a.tag} | {len(ok)} | {fmt(m['ready_ms'], 0)} | "
                  f"{fmt(m['interval_avg_ms'], 2)} | {fmt(m['fps'], 0)} | {fmt(m['interval_p95_ms'])} | {fmt(m['frames_over_33ms'], 0)} | "
                  f"{fmt(m['cpu_ms_per_frame'])} | {fmt(m['cpu_core_pct'], 0)} | {fmt(m['idle_cpu_pct'])} | {fmt(m['rss_after_ready_median_mb'], 0)} |")
    summ = os.environ.get("GITHUB_STEP_SUMMARY")
    if summ and md:
        with open(summ, "a") as f:
            f.write("| fw | scenario | os | tag | runs | ready ms | frame avg ms | fps | p95 | >33ms | CPU ms/frame | CPU % of 1 core | idle CPU % | RSS MB |\n"
                    "|---|---|---|---|---|---|---|---|---|---|---|---|---|---|\n" + "\n".join(md) + "\n")
    return 1 if failed_scenarios else 0


if __name__ == "__main__":
    sys.exit(main())
