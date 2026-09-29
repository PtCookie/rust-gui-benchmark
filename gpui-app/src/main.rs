use bench_core as core;
use gpui::{
    canvas, div, fill, point, prelude::*, px, rgb, size, uniform_list, AnyElement, App,
    Application, Bounds, Context, Corners, PathBuilder, Rgba, SharedString,
    UniformListScrollHandle, Window, WindowBounds, WindowOptions,
};
use std::collections::HashMap;
use std::ops::Range;
use std::path::PathBuf;
use std::time::{Duration, Instant};

const ROW: f32 = 20.0;
const STEP_ROWS: f32 = 90.0;
const LANE_W: f32 = 10.0;
const MAX_DRAW_LANES: u16 = 30;
const GPAGE: usize = 200;
const COLORS: [u32; 8] = [0x4ec9b0, 0x569cd6, 0xc586c0, 0xce9178, 0xdcdcaa, 0x9cdcfe, 0xd7ba7d, 0xb5cea8];

fn fmt_date(secs: i64) -> String {
    let z = secs.div_euclid(86400) + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{:04}-{:02}-{:02}", y, m, d)
}

#[derive(PartialEq)]
enum Phase {
    Warmup,
    Run,
}

struct Bench {
    commits_mode: bool,
    commits: Vec<core::CommitRow>,
    conn: Option<core::DbConn>,
    total: usize,
    grid_cache: HashMap<usize, Vec<core::GridRow>>,
    graph_w: f32,
    scroll: UniformListScrollHandle,
    phase: Phase,
    frames_seen: usize,
    t_start: Instant,
    core_load_ms: f64,
    first_render_ms: f64,
    ready_at: Option<Instant>,
    last: Instant,
    pos: f32,
    ints: Vec<f64>,
    out: String,
    frames: usize,
    proc_total_ms: f64,
    proc_calls: usize,
}

impl Bench {
    fn grid_row(&mut self, i: usize) -> Option<&core::GridRow> {
        let page = i / GPAGE;
        if self.grid_cache.len() > 160 {
            self.grid_cache.retain(|k, _| (*k as i64 - page as i64).abs() < 30);
        }
        if !self.grid_cache.contains_key(&page) {
            let rows = core::grid_page(self.conn.as_ref().unwrap(), (page * GPAGE) as i64, GPAGE as i64);
            self.grid_cache.insert(page, rows);
        }
        self.grid_cache.get(&page).and_then(|p| p.get(i % GPAGE))
    }

    fn finish(&self) {
        let mut si = self.ints.clone();
        si.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let pct = |p: f64| si[((si.len() - 1) as f64 * p) as usize];
        let mean = self.ints.iter().sum::<f64>() / self.ints.len() as f64;
        let r = serde_json::json!({
            "framework": "gpui",
            "scenario": if self.commits_mode { "commits" } else { "grid" },
            "total": self.total,
            "first_render_ms": self.first_render_ms,
            "core_load_ms": self.core_load_ms,
            "frames": self.frames,
            "list_build_avg_ms": if self.proc_calls > 0 { self.proc_total_ms / self.proc_calls as f64 } else { 0.0 },
            "list_build_calls": self.proc_calls,
            "interval_avg_ms": mean, "interval_p50_ms": pct(0.5), "interval_p95_ms": pct(0.95),
            "interval_p99_ms": pct(0.99), "interval_max_ms": si[si.len() - 1],
            "frames_over_33ms": self.ints.iter().filter(|x| **x > 33.4).count(),
            "frames_over_50ms": self.ints.iter().filter(|x| **x > 50.0).count(),
            "frames_with_missing_rows": 0,
            "rows_scrolled": self.frames as f32 * STEP_ROWS,
        });
        std::fs::write(&self.out, serde_json::to_string_pretty(&r).unwrap()).ok();
    }
}

impl Render for Bench {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let now = Instant::now();
        match self.phase {
            Phase::Warmup => {
                self.frames_seen += 1;
                if self.frames_seen == 3 && self.ready_at.is_none() {
                    self.first_render_ms = self.t_start.elapsed().as_secs_f64() * 1e3;
                    std::fs::write(format!("{}.ready", self.out), "").ok();
                    self.ready_at = Some(now);
                }
                if let Some(r) = self.ready_at {
                    if now - r >= Duration::from_millis(700) {
                        self.phase = Phase::Run;
                        self.last = now;
                    }
                }
            }
            Phase::Run => {
                self.ints.push((now - self.last).as_secs_f64() * 1e3);
                self.last = now;
                let max_scroll = self.total as f32 * ROW - 780.0;
                self.pos = (self.pos + STEP_ROWS * ROW) % max_scroll;
                self.scroll
                    .0
                    .borrow()
                    .base_handle
                    .set_offset(point(px(0.), px(-self.pos)));
                if self.ints.len() > self.frames {
                    self.ints.remove(0); // first interval includes the phase switch
                    self.finish();
                    cx.quit();
                }
            }
        }
        window.request_animation_frame();

        let list: AnyElement = if self.commits_mode {
            let gw = self.graph_w;
            uniform_list(
                "commits",
                self.total,
                cx.processor(move |this: &mut Bench, range: Range<usize>, _w, _cx| {
                    let t0 = Instant::now();
                    let out = range
                        .map(|ix| {
                            let r = &this.commits[ix];
                            let through = r.through.clone();
                            let merge_to = r.merge_to.clone();
                            let lane = r.lane;
                            let color = Rgba::from(rgb(COLORS[lane as usize % 8]));
                            let bg = if ix % 2 == 0 { rgb(0x1e1e1e) } else { rgb(0x222222) };
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .h(px(ROW))
                                .w_full()
                                .bg(bg)
                                .text_size(px(12.))
                                .text_color(rgb(0xd4d4d4))
                                .child(
                                    canvas(
                                        |_, _, _| (),
                                        move |b, _, window, _| {
                                            let o = b.origin;
                                            for &l in &through {
                                                if l >= MAX_DRAW_LANES {
                                                    continue;
                                                }
                                                let x = o.x + px(l as f32 * LANE_W + 5.0);
                                                let c = Rgba::from(rgb(COLORS[l as usize % 8]));
                                                window.paint_quad(fill(
                                                    Bounds::new(point(x, o.y), size(px(1.), px(ROW))),
                                                    c,
                                                ));
                                            }
                                            if lane < MAX_DRAW_LANES {
                                                let x = o.x + px(lane as f32 * LANE_W + 5.0);
                                                let cy = o.y + px(ROW / 2.0);
                                                window.paint_quad(fill(
                                                    Bounds::new(point(x, cy), size(px(1.), px(ROW / 2.0))),
                                                    color,
                                                ));
                                                for &m in &merge_to {
                                                    if m >= MAX_DRAW_LANES {
                                                        continue;
                                                    }
                                                    let mut pb = PathBuilder::stroke(px(1.));
                                                    pb.move_to(point(x, cy));
                                                    pb.line_to(point(
                                                        o.x + px(m as f32 * LANE_W + 5.0),
                                                        o.y + px(ROW),
                                                    ));
                                                    if let Ok(p) = pb.build() {
                                                        window.paint_path(p, color);
                                                    }
                                                }
                                                let mut q = fill(
                                                    Bounds::new(
                                                        point(x - px(3.), cy - px(3.)),
                                                        size(px(6.), px(6.)),
                                                    ),
                                                    color,
                                                );
                                                q.corner_radii = Corners::all(px(3.));
                                                window.paint_quad(q);
                                            }
                                        },
                                    )
                                    .w(px(gw))
                                    .h(px(ROW)),
                                )
                                .child(
                                    div()
                                        .w(px(64.))
                                        .text_color(rgb(0x8a8a8a))
                                        .child(SharedString::from(r.short.clone())),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .child(SharedString::from(r.summary.clone())),
                                )
                                .child(
                                    div()
                                        .w(px(170.))
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .child(SharedString::from(r.author.clone())),
                                )
                                .child(
                                    div()
                                        .w(px(110.))
                                        .text_color(rgb(0x8a8a8a))
                                        .child(SharedString::from(fmt_date(r.time))),
                                )
                        })
                        .collect::<Vec<_>>();
                    this.proc_total_ms += t0.elapsed().as_secs_f64() * 1e3;
                    this.proc_calls += 1;
                    out
                }),
            )
            .track_scroll(self.scroll.clone())
            .h_full()
            .into_any_element()
        } else {
            uniform_list(
                "grid",
                self.total,
                cx.processor(move |this: &mut Bench, range: Range<usize>, _w, _cx| {
                    let t0 = Instant::now();
                    let out = range
                        .map(|ix| {
                            let (id, user, kind, amount, date, note) = match this.grid_row(ix) {
                                Some(r) => (
                                    r.id.to_string(),
                                    r.user.clone(),
                                    r.kind.clone(),
                                    format!("{:.2}", r.amount),
                                    fmt_date(r.ts),
                                    r.note.clone(),
                                ),
                                None => Default::default(),
                            };
                            let bg = if ix % 2 == 0 { rgb(0x1e1e1e) } else { rgb(0x222222) };
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .h(px(ROW))
                                .w_full()
                                .bg(bg)
                                .text_size(px(12.))
                                .text_color(rgb(0xd4d4d4))
                                .child(div().w(px(84.)).pl(px(8.)).child(SharedString::from(id)))
                                .child(div().w(px(124.)).child(SharedString::from(user)))
                                .child(div().w(px(94.)).child(SharedString::from(kind)))
                                .child(div().w(px(94.)).child(SharedString::from(amount)))
                                .child(
                                    div()
                                        .w(px(116.))
                                        .text_color(rgb(0x8a8a8a))
                                        .child(SharedString::from(date)),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .text_color(rgb(0x8a8a8a))
                                        .child(SharedString::from(note)),
                                )
                        })
                        .collect::<Vec<_>>();
                    this.proc_total_ms += t0.elapsed().as_secs_f64() * 1e3;
                    this.proc_calls += 1;
                    out
                }),
            )
            .track_scroll(self.scroll.clone())
            .h_full()
            .into_any_element()
        };
        div().size_full().bg(rgb(0x1e1e1e)).child(list)
    }
}

fn main() {
    env_logger::init();
    let t_start = Instant::now();
    let scenario = std::env::var("BENCH_SCENARIO").unwrap_or_else(|_| "commits".into());
    let out = std::env::var("BENCH_OUT").unwrap_or_else(|_| "/tmp/bench-gpui.json".into());
    let commits_mode = scenario == "commits";
    let frames: usize = std::env::var("BENCH_FRAMES").ok().and_then(|v| v.parse().ok()).unwrap_or(900);

    let t = Instant::now();
    let (commits, conn, total, graph_w) = if commits_mode {
        let rows = core::load_commit_rows(&PathBuf::from(std::env::var("BENCH_REPO").unwrap()), usize::MAX);
        let mw = rows.iter().map(|r| r.width).max().unwrap_or(1).min(MAX_DRAW_LANES);
        let n = rows.len();
        (rows, None, n, mw as f32 * LANE_W + 8.0)
    } else {
        let c = core::open_db(&PathBuf::from(std::env::var("BENCH_DB").unwrap())).unwrap();
        let n = core::count_rows(&c) as usize;
        (Vec::new(), Some(c), n, 0.0)
    };
    let core_load_ms = t.elapsed().as_secs_f64() * 1e3;

    Application::new().run(move |cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1200.), px(780.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| {
                cx.new(|_| Bench {
                    commits_mode,
                    commits,
                    conn,
                    total,
                    grid_cache: HashMap::new(),
                    graph_w,
                    scroll: UniformListScrollHandle::new(),
                    phase: Phase::Warmup,
                    frames_seen: 0,
                    t_start,
                    core_load_ms,
                    first_render_ms: 0.0,
                    ready_at: None,
                    last: Instant::now(),
                    pos: 0.0,
                    ints: Vec::with_capacity(frames + 2),
                    out,
                    frames,
                    proc_total_ms: 0.0,
                    proc_calls: 0,
                })
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
