#!/usr/bin/env python3
"""Summarize results/*.json (written by xbench.py) into a markdown table of medians.
usage: python scripts/summarize.py [results_dir]"""
import glob, json, re, statistics, sys, collections
d = sys.argv[1] if len(sys.argv) > 1 else "results"
groups = collections.defaultdict(list)
for f in sorted(glob.glob(f"{d}/*-*-*-[0-9].json")):
    r = json.load(open(f))
    if r.get("exit_code") == 0 and "error" not in r and (r.get("interval_avg_ms") is not None or r.get("idle_s") is not None):
        m = re.match(r"^(.*)-(electron|tauri|slint-skia|slint-widgets|slint|gpui)-(commits|grid|idle|diff)-\d+$", f.replace("\\", "/").split("/")[-1][:-5])
        if m:
            groups[(m.group(2), m.group(3), m.group(1))].append(r)
keys = ["ready_ms", "core_load_ms", "interval_avg_ms", "interval_p95_ms", "frames_over_33ms",
        "fps", "cpu_ms_per_frame", "cpu_core_pct", "idle_cpu_pct", "pss_after_ready_median_mb", "rss_after_ready_median_mb", "rss_peak_mb"]
def med(rs, k):
    v = [r[k] for r in rs if r.get(k) is not None]
    return f"{statistics.median(v):.1f}" if v else "-"
print("| fw | scenario | tag | runs | ready ms | core load ms | frame avg ms | p95 ms | >33ms frames | fps | CPU ms/frame | CPU % of 1 core | idle CPU % | PSS MB (Linux) | RSS MB | RSS peak MB |")
print("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|")
for (fw, sc, tag), rs in sorted(groups.items()):
    print(f"| {fw} | {sc} | {tag} | {len(rs)} | " + " | ".join(med(rs, k) for k in keys) + " |")
