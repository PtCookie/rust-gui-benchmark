// Shared frontend for Electron and Tauri. Identical code, different IPC adapter.
declare global {
  interface Window {
    __TAURI__?: { core: { invoke: (cmd: string, args?: unknown) => Promise<any> } };
    benchApi?: Record<string, (args?: unknown) => Promise<any>>;
  }
}

const ROW = 20;
const STEP_ROWS = 90; // rows scrolled per frame (fast fling)
const FRAMES = 900;
const LANE_W = 10;
const MAX_DRAW_LANES = 30;
const COLORS = ["#4ec9b0", "#569cd6", "#c586c0", "#ce9178", "#dcdcaa", "#9cdcfe", "#d7ba7d", "#b5cea8"];

const call = (cmd: string, args: unknown = {}): Promise<any> =>
  window.__TAURI__
    ? window.__TAURI__.core.invoke(cmd, (cmd === "report" ? { r: args } : args) as any) // tauri commands take named args
    : window.benchApi![cmd](args);

const mark = (name: string) => { call("mark", { name }).catch(() => {}); };
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const raf = () => new Promise<number>((r) => requestAnimationFrame(r));

function pct(sorted: number[], p: number) {
  return sorted[Math.min(sorted.length - 1, Math.floor((sorted.length - 1) * p))];
}

async function main() {
  const t0 = performance.now();
  const info = await call("open");
  const scenario: "commits" | "grid" = info.scenario;
  const total: number = info.total;
  const maxWidth: number = info.max_width ?? 1;
  const pageSize = scenario === "commits" ? 500 : 200;
  const fetchCmd = scenario === "commits" ? "commits_chunk" : "grid_page";

  const wrap = document.getElementById("wrap") as HTMLDivElement;
  const spacer = document.getElementById("spacer") as HTMLDivElement;
  const layer = document.getElementById("layer") as HTMLDivElement;
  const canvas = document.getElementById("g") as HTMLCanvasElement;
  spacer.style.height = total * ROW + "px";

  const vh = window.innerHeight;
  const vw = window.innerWidth - 16;
  const gw = scenario === "commits" ? Math.min(maxWidth, MAX_DRAW_LANES) * LANE_W + 8 : 0;
  canvas.width = gw;
  canvas.height = vh;
  canvas.style.display = gw ? "block" : "none";
  const ctx = canvas.getContext("2d")!;

  // column layout
  const cols =
    scenario === "commits"
      ? [
          [gw, 64],
          [gw + 68, Math.max(200, vw - gw - 68 - 300)],
          [vw - 300 + 8, 170],
          [vw - 120, 110],
        ]
      : [
          [8, 80],
          [92, 120],
          [216, 90],
          [310, 90],
          [404, 110],
          [520, Math.max(120, vw - 530)],
        ];

  const nRows = Math.ceil(vh / ROW) + 3;
  const rows: { el: HTMLDivElement; spans: HTMLSpanElement[]; idx: number }[] = [];
  for (let i = 0; i < nRows; i++) {
    const el = document.createElement("div");
    el.className = "row";
    const spans = cols.map(([x, w], c) => {
      const s = document.createElement("span");
      s.style.left = x + "px";
      s.style.width = w + "px";
      if (scenario === "commits" ? c === 0 || c === 3 : c >= 4) s.className = "dim";
      el.appendChild(s);
      return s;
    });
    layer.appendChild(el);
    rows.push({ el, spans, idx: -1 });
  }

  const cache = new Map<number, any[]>();
  const inflight = new Set<number>();
  const ensure = (p: number) => {
    if (p < 0 || p * pageSize >= total || cache.has(p) || inflight.has(p)) return;
    inflight.add(p);
    call(fetchCmd, { offset: p * pageSize, count: pageSize }).then((r) => {
      cache.set(p, r);
      inflight.delete(p);
    });
  };
  const prefetch = (pos: number) => {
    const first = Math.floor(pos / ROW / pageSize);
    const last = Math.floor((pos + vh) / ROW / pageSize);
    for (let p = first - 1; p <= last + 3; p++) ensure(p);
    if (cache.size > 160) {
      for (const k of cache.keys()) if (k < first - 30 || k > last + 30) cache.delete(k);
    }
  };
  const rowAt = (i: number) => {
    const page = cache.get(Math.floor(i / pageSize));
    return page ? page[i % pageSize] : undefined;
  };

  const fmtDate = (s: number) => new Date(s * 1000).toISOString().slice(0, 10);

  let missing = 0;
  const render = (pos: number) => {
    const first = Math.floor(pos / ROW);
    const off = pos - first * ROW;
    prefetch(pos);
    let anyMissing = false;
    if (gw) ctx.clearRect(0, 0, gw, vh);
    for (let k = 0; k < rows.length; k++) {
      const r = rows[k];
      const i = first + k;
      const y = k * ROW - off;
      r.el.style.transform = `translateY(${y}px)`;
      if (i >= total) {
        r.el.style.display = "none";
        continue;
      }
      r.el.style.display = "";
      const d = rowAt(i);
      if (!d) {
        anyMissing = true;
        if (r.idx !== -2) for (const s of r.spans) s.textContent = "";
        r.idx = -2;
        continue;
      }
      if (r.idx !== i) {
        r.idx = i;
        if (scenario === "commits") {
          r.spans[0].textContent = d.short;
          r.spans[1].textContent = d.summary;
          r.spans[2].textContent = d.author;
          r.spans[3].textContent = fmtDate(d.time);
        } else {
          r.spans[0].textContent = String(d.id);
          r.spans[1].textContent = d.user;
          r.spans[2].textContent = d.kind;
          r.spans[3].textContent = d.amount.toFixed(2);
          r.spans[4].textContent = fmtDate(d.ts);
          r.spans[5].textContent = d.note;
        }
      }
      if (gw) {
        const cy = y + ROW / 2;
        for (const l of d.through as number[]) {
          if (l >= MAX_DRAW_LANES) continue;
          ctx.strokeStyle = COLORS[l % 8];
          ctx.beginPath();
          ctx.moveTo(l * LANE_W + 5, y);
          ctx.lineTo(l * LANE_W + 5, y + ROW);
          ctx.stroke();
        }
        if (d.lane < MAX_DRAW_LANES) {
          const x = d.lane * LANE_W + 5;
          ctx.strokeStyle = COLORS[d.lane % 8];
          ctx.beginPath();
          ctx.moveTo(x, cy);
          ctx.lineTo(x, y + ROW);
          for (const m of d.merge_to as number[]) {
            if (m >= MAX_DRAW_LANES) continue;
            ctx.moveTo(x, cy);
            ctx.lineTo(m * LANE_W + 5, y + ROW);
          }
          ctx.stroke();
          ctx.fillStyle = COLORS[d.lane % 8];
          ctx.beginPath();
          ctx.arc(x, cy, 3, 0, 6.2832);
          ctx.fill();
        }
      }
    }
    if (anyMissing) missing++;
  };

  // first page then first paint
  prefetch(0);
  while (!cache.has(0)) await sleep(1);
  render(0);
  await raf();
  await raf();
  const firstRenderMs = performance.now() - t0;
  mark("s0_before_ready");
  await call("ready");
  mark("s1_after_ready");
  await sleep(700);
  mark("s2_after_sleep");

  // autoscroll benchmark
  const maxScroll = total * ROW - vh;
  let pos = 0;
  const ints: number[] = [];
  const work: number[] = [];
  mark("s3_loop_start");
  let stalled = false;
  let wd = setTimeout(() => { stalled = true; }, 5000);
  const stallReport = () =>
    call("report", { error: `stall: vis=${document.visibilityState} focus=${document.hasFocus()} frames=${ints.length}`, scenario, total });
  let last = await Promise.race([raf(), new Promise<number>((r) => setTimeout(() => r(-1), 5000))]);
  if (last < 0) return stallReport();
  missing = 0;
  for (let f = 0; f < FRAMES; f++) {
    const t = await Promise.race([raf(), new Promise<number>((r) => setTimeout(() => r(-1), 5000))]);
    if (t < 0) return stallReport();
    const w0 = performance.now();
    pos = (pos + STEP_ROWS * ROW) % maxScroll;
    wrap.scrollTop = pos;
    render(pos);
    work.push(performance.now() - w0);
    if (f % 100 === 0) mark("f" + f);
    ints.push(t - last);
    last = t;
  }
  clearTimeout(wd);
  const si = [...ints].sort((a, b) => a - b);
  const sw = [...work].sort((a, b) => a - b);
  const mean = (a: number[]) => a.reduce((x, y) => x + y, 0) / a.length;
  await call("report", {
    scenario,
    total,
    first_render_ms: firstRenderMs,
    frames: FRAMES,
    interval_avg_ms: mean(ints),
    interval_p50_ms: pct(si, 0.5),
    interval_p95_ms: pct(si, 0.95),
    interval_p99_ms: pct(si, 0.99),
    interval_max_ms: si[si.length - 1],
    frames_over_33ms: ints.filter((x) => x > 33.4).length,
    frames_over_50ms: ints.filter((x) => x > 50).length,
    work_avg_ms: mean(work),
    work_p95_ms: pct(sw, 0.95),
    frames_with_missing_rows: missing,
    rows_scrolled: FRAMES * STEP_ROWS,
  });
}

main().catch(async (e) => {
  try {
    await call("report", { error: String(e && (e.stack || e)) });
  } catch {}
});

export {};
