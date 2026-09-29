#!/usr/bin/env python3
"""Aggregate results/*.json (runs 1..N) into median tables."""
import glob, json, statistics, sys, os
ROOT = os.path.dirname(os.path.abspath(__file__))
FW = ["electron", "tauri", "slint", "gpui"]
SC = ["commits", "grid"]

def load(fw, sc):
    rows = []
    for f in sorted(glob.glob(f"{ROOT}/results/{fw}-{sc}-[0-9].json")):
        r = json.load(open(f))
        if r.get("exit_code") == 0 and "error" not in r:
            rows.append(r)
    return rows

def med(rows, k):
    v = [r[k] for r in rows if r.get(k) is not None]
    return statistics.median(v) if v else None

def spread(rows, k):
    v = [r[k] for r in rows if r.get(k) is not None]
    return (min(v), max(v)) if v else (None, None)

def f(x, d=1):
    return "-" if x is None else f"{x:.{d}f}"

def table(sc):
    print(f"\n### {sc}\n")
    print("| framework | runs | ready (ms) | core load (ms) | framework overhead (ms) | frame avg (ms) | p95 | p99 | max | >33ms frames | CPU ms/frame | startup CPU (s) | PSS after ready (MB) | PSS peak (MB) |")
    print("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|")
    for fw in FW:
        rows = load(fw, sc)
        if not rows:
            print(f"| {fw} | 0 | (no successful runs) |" + " |" * 11)
            continue
        ready, core = med(rows, "ready_ms"), med(rows, "core_load_ms")
        ovh = None if ready is None or core is None else ready - core
        print("| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |".format(
            fw, len(rows), f(ready, 0), f(core, 0), f(ovh, 0),
            f(med(rows, "interval_avg_ms"), 2), f(med(rows, "interval_p95_ms")), f(med(rows, "interval_p99_ms")),
            f(med(rows, "interval_max_ms")), f(med(rows, "frames_over_33ms"), 0),
            f(med(rows, "cpu_ms_per_frame"), 1), f(med(rows, "cpu_at_ready_s"), 2),
            f(med(rows, "pss_after_ready_median_mb"), 0), f(med(rows, "pss_peak_mb"), 0)))

for sc in SC:
    table(sc)
print("\n(run-to-run spread, ready_ms / cpu_ms_per_frame)")
for fw in FW:
    for sc in SC:
        rows = load(fw, sc)
        if rows:
            a, b = spread(rows, "ready_ms"); c, d = spread(rows, "cpu_ms_per_frame")
            print(f"  {fw:9s} {sc:8s} ready {f(a,0)}..{f(b,0)}   cpu/frame {f(c)}..{f(d)}")
