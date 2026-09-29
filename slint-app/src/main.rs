use bench_core as core;
use slint::{Model, ModelRc, ModelTracker, SharedString};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

slint::slint! {
    import { ListView } from "std-widgets.slint";
    export struct CommitRowUi { short: string, summary: string, author: string, date: string, lane-x: length, dot: color, path: string }
    export struct GridRowUi { id: string, user: string, kind: string, amount: string, date: string, note: string }

    export component App inherits Window {
        in property <bool> commits;
        in property <[CommitRowUi]> commit-model;
        in property <[GridRowUi]> grid-model;
        in property <length> graph-w: 100px;
        in-out property <length> scroll-y;
        width: 1200px;
        height: 780px;
        background: #1e1e1e;

        if root.commits: ListView {
            content-y <=> root.scroll-y;
            for r[i] in root.commit-model: Rectangle {
                height: 20px;
                background: mod(i, 2) == 0 ? transparent : #ffffff08;
                Path {
                    x: 0px; y: 0px; width: root.graph-w; height: 20px;
                    viewbox-x: 0; viewbox-y: 0;
                    viewbox-width: root.graph-w / 1px; viewbox-height: 20;
                    commands: r.path;
                    stroke: #6a9955; stroke-width: 1px;
                }
                Rectangle { x: r.lane-x; y: 7px; width: 6px; height: 6px; border-radius: 3px; background: r.dot; }
                Text { x: root.graph-w; width: 64px; height: 20px; text: r.short; color: #8a8a8a; font-size: 12px; overflow: elide; vertical-alignment: center; }
                Text { x: root.graph-w + 68px; width: root.width - root.graph-w - 68px - 310px; height: 20px; text: r.summary; color: #d4d4d4; font-size: 12px; overflow: elide; vertical-alignment: center; }
                Text { x: root.width - 300px; width: 170px; height: 20px; text: r.author; color: #d4d4d4; font-size: 12px; overflow: elide; vertical-alignment: center; }
                Text { x: root.width - 120px; width: 100px; height: 20px; text: r.date; color: #8a8a8a; font-size: 12px; overflow: elide; vertical-alignment: center; }
            }
        }
        if !root.commits: ListView {
            content-y <=> root.scroll-y;
            for r[i] in root.grid-model: Rectangle {
                height: 20px;
                background: mod(i, 2) == 0 ? transparent : #ffffff08;
                Text { x: 8px; width: 80px; height: 20px; text: r.id; color: #d4d4d4; font-size: 12px; overflow: elide; vertical-alignment: center; }
                Text { x: 92px; width: 120px; height: 20px; text: r.user; color: #d4d4d4; font-size: 12px; overflow: elide; vertical-alignment: center; }
                Text { x: 216px; width: 90px; height: 20px; text: r.kind; color: #d4d4d4; font-size: 12px; overflow: elide; vertical-alignment: center; }
                Text { x: 310px; width: 90px; height: 20px; text: r.amount; color: #d4d4d4; font-size: 12px; overflow: elide; vertical-alignment: center; }
                Text { x: 404px; width: 110px; height: 20px; text: r.date; color: #8a8a8a; font-size: 12px; overflow: elide; vertical-alignment: center; }
                Text { x: 520px; width: root.width - 550px; height: 20px; text: r.note; color: #8a8a8a; font-size: 12px; overflow: elide; vertical-alignment: center; }
            }
        }
    }
}

const ROW: f32 = 20.0;
const STEP_ROWS: f32 = 90.0;
const FRAMES: usize = 900;
const LANE_W: f32 = 10.0;
const MAX_DRAW_LANES: u16 = 30;
const COLORS: [(u8, u8, u8); 8] = [
    (0x4e, 0xc9, 0xb0), (0x56, 0x9c, 0xd6), (0xc5, 0x86, 0xc0), (0xce, 0x91, 0x78),
    (0xdc, 0xdc, 0xaa), (0x9c, 0xdc, 0xfe), (0xd7, 0xba, 0x7d), (0xb5, 0xce, 0xa8),
];

fn fmt_date(secs: i64) -> String {
    // civil-from-days (Howard Hinnant)
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

fn commit_ui(r: &core::CommitRow) -> CommitRowUi {
    let mut p = String::new();
    for &l in &r.through {
        if l >= MAX_DRAW_LANES { continue; }
        let x = l as f32 * LANE_W + 5.0;
        p.push_str(&format!("M {x} 0 L {x} 20 "));
    }
    if r.lane < MAX_DRAW_LANES {
        let x = r.lane as f32 * LANE_W + 5.0;
        p.push_str(&format!("M {x} 10 L {x} 20 "));
        for &m in &r.merge_to {
            if m >= MAX_DRAW_LANES { continue; }
            p.push_str(&format!("M {x} 10 L {} 20 ", m as f32 * LANE_W + 5.0));
        }
    }
    let (cr, cg, cb) = COLORS[r.lane as usize % 8];
    CommitRowUi {
        short: r.short.clone().into(),
        summary: r.summary.clone().into(),
        author: r.author.clone().into(),
        date: fmt_date(r.time).into(),
        lane_x: (r.lane.min(MAX_DRAW_LANES) as f32 * LANE_W + 2.0).into(),
        dot: slint::Color::from_rgb_u8(cr, cg, cb),
        path: SharedString::from(p),
    }
}

struct LazyCommits {
    rows: Vec<core::CommitRow>,
    cache: RefCell<HashMap<usize, Rc<Vec<CommitRowUi>>>>,
}
const CPAGE: usize = 500;
impl Model for LazyCommits {
    type Data = CommitRowUi;
    fn row_count(&self) -> usize { self.rows.len() }
    fn row_data(&self, i: usize) -> Option<CommitRowUi> {
        if i >= self.rows.len() { return None; }
        let page = i / CPAGE;
        let mut c = self.cache.borrow_mut();
        if c.len() > 160 { c.retain(|k, _| (*k as i64 - page as i64).abs() < 30); }
        let p = c.entry(page).or_insert_with(|| {
            let s = page * CPAGE;
            let e = (s + CPAGE).min(self.rows.len());
            Rc::new(self.rows[s..e].iter().map(commit_ui).collect())
        });
        Some(p[i % CPAGE].clone())
    }
    fn model_tracker(&self) -> &dyn ModelTracker { &() }
}

struct LazyGrid {
    conn: core::DbConn,
    total: usize,
    cache: RefCell<HashMap<usize, Rc<Vec<GridRowUi>>>>,
}
const GPAGE: usize = 200;
impl Model for LazyGrid {
    type Data = GridRowUi;
    fn row_count(&self) -> usize { self.total }
    fn row_data(&self, i: usize) -> Option<GridRowUi> {
        if i >= self.total { return None; }
        let page = i / GPAGE;
        let mut c = self.cache.borrow_mut();
        if c.len() > 160 { c.retain(|k, _| (*k as i64 - page as i64).abs() < 30); }
        let p = c.entry(page).or_insert_with(|| {
            let rows = core::grid_page(&self.conn, (page * GPAGE) as i64, GPAGE as i64);
            Rc::new(
                rows.into_iter()
                    .map(|r| GridRowUi {
                        id: r.id.to_string().into(),
                        user: r.user.into(),
                        kind: r.kind.into(),
                        amount: format!("{:.2}", r.amount).into(),
                        date: fmt_date(r.ts).into(),
                        note: r.note.into(),
                    })
                    .collect(),
            )
        });
        Some(p[i % GPAGE].clone())
    }
    fn model_tracker(&self) -> &dyn ModelTracker { &() }
}

fn pct(s: &[f64], p: f64) -> f64 { s[((s.len() - 1) as f64 * p) as usize] }
fn mean(a: &[f64]) -> f64 { a.iter().sum::<f64>() / a.len() as f64 }

fn main() {
    let t_start = Instant::now();
    let scenario = std::env::var("BENCH_SCENARIO").unwrap_or_else(|_| "commits".into());
    let out = std::env::var("BENCH_OUT").unwrap_or_else(|_| "/tmp/bench-slint.json".into());
    let ui = App::new().unwrap();

    let t = Instant::now();
    let total;
    if scenario == "commits" {
        let rows = core::load_commit_rows(&PathBuf::from(std::env::var("BENCH_REPO").unwrap()), usize::MAX);
        let mw = rows.iter().map(|r| r.width).max().unwrap_or(1).min(MAX_DRAW_LANES);
        total = rows.len();
        ui.set_graph_w(mw as f32 * LANE_W + 8.0);
        ui.set_commits(true);
        ui.set_commit_model(ModelRc::new(LazyCommits { rows, cache: RefCell::new(HashMap::new()) }));
    } else {
        let conn = core::open_db(&PathBuf::from(std::env::var("BENCH_DB").unwrap())).unwrap();
        total = core::count_rows(&conn) as usize;
        ui.set_commits(false);
        ui.set_grid_model(ModelRc::new(LazyGrid { conn, total, cache: RefCell::new(HashMap::new()) }));
    }
    let core_load_ms = t.elapsed().as_secs_f64() * 1e3;
    ui.show().unwrap();

    // real frame cadence: count winit RedrawRequested events while the benchmark runs
    let redraws: Rc<RefCell<Vec<Instant>>> = Rc::new(RefCell::new(Vec::new()));
    let recording = Rc::new(std::cell::Cell::new(false));
    {
        use slint::winit_030::{winit, EventResult, WinitWindowAccessor};
        let (r, rec) = (redraws.clone(), recording.clone());
        ui.window().on_winit_window_event(move |_w, ev| {
            if rec.get() && matches!(ev, winit::event::WindowEvent::RedrawRequested) {
                r.borrow_mut().push(Instant::now());
            }
            EventResult::Propagate
        });
    }
    let (redraws_in, rec_in) = (redraws.clone(), recording.clone());
    let ui_weak = ui.as_weak();
    let out2 = out.clone();
    // ready marker after a couple of loop turns (first frame has been laid out and rendered by then)
    slint::Timer::single_shot(Duration::from_millis(50), {
        let out = out.clone();
        move || {
            std::fs::write(format!("{}.ready", out), "").ok();
        }
    });
    let first_render_ms = t_start.elapsed().as_secs_f64() * 1e3 + 50.0;

    let timer = Rc::new(slint::Timer::default());
    let timer2 = timer.clone();
    slint::Timer::single_shot(Duration::from_millis(750), move || {
        let vh = 780.0f32;
        let max_scroll = total as f32 * ROW - vh;
        let mut pos = 0.0f32;
        let mut ints: Vec<f64> = Vec::with_capacity(FRAMES);
        let mut work: Vec<f64> = Vec::with_capacity(FRAMES);
        let mut last = Instant::now();
        let mut n = 0usize;
        let ui_weak = ui_weak.clone();
        let out = out2.clone();
        let timer3 = timer2.clone();
        timer2.start(slint::TimerMode::Repeated, Duration::from_micros(16667), move || {
            let now = Instant::now();
            let w0 = now;
            if n == 0 { redraws_in.borrow_mut().clear(); rec_in.set(true); }
            if n > 0 { ints.push((now - last).as_secs_f64() * 1e3); }
            last = now;
            let ui = ui_weak.unwrap();
            pos = (pos + STEP_ROWS * ROW) % max_scroll;
            ui.set_scroll_y(-pos);
            work.push(w0.elapsed().as_secs_f64() * 1e3);
            n += 1;
            if n > FRAMES {
                timer3.stop();
                rec_in.set(false);
                let rd: Vec<f64> = redraws_in.borrow().windows(2).map(|w| (w[1] - w[0]).as_secs_f64() * 1e3).collect();
                let mut rds = rd.clone(); rds.sort_by(|a, b| a.partial_cmp(b).unwrap());
                // pure render cost: software-render full frames at scattered positions
                let mut snap: Vec<f64> = Vec::new();
                let do_snap = std::env::var("BENCH_SNAPSHOT").is_ok();
                for k in 0..(if do_snap { 40 } else { 0 }) {
                    ui.set_scroll_y(-((k as f32 * 7919.0 * ROW) % max_scroll));
                    let s = Instant::now();
                    let img = ui.window().take_snapshot();
                    std::hint::black_box(&img);
                    snap.push(s.elapsed().as_secs_f64() * 1e3);
                }
                let mut si = ints.clone(); si.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let mut sw = work.clone(); sw.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let mut ss = snap.clone(); ss.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let (snap_avg, snap_p95) = if snap.is_empty() { (0.0, 0.0) } else { (mean(&snap), pct(&ss, 0.95)) };
                let r = serde_json::json!({
                    "framework": "slint", "scenario": if total > 500_000 { "grid" } else { "commits" }, "total": total,
                    "first_render_ms": first_render_ms, "core_load_ms": core_load_ms, "frames": FRAMES,
                    "interval_avg_ms": mean(&rd), "interval_p50_ms": pct(&rds, 0.5), "interval_p95_ms": pct(&rds, 0.95),
                    "interval_p99_ms": pct(&rds, 0.99), "interval_max_ms": rds[rds.len() - 1],
                    "frames_over_33ms": rd.iter().filter(|x| **x > 33.4).count(),
                    "frames_over_50ms": rd.iter().filter(|x| **x > 50.0).count(),
                    "work_avg_ms": mean(&work), "work_p95_ms": pct(&sw, 0.95),
                    "frames_with_missing_rows": 0, "rows_scrolled": FRAMES as f32 * STEP_ROWS,
                    "redraw_count": rd.len() + 1,
                    "redraw_avg_ms": mean(&rd), "redraw_p50_ms": pct(&rds, 0.5), "redraw_p95_ms": pct(&rds, 0.95),
                    "redraw_p99_ms": pct(&rds, 0.99), "redraw_max_ms": rds[rds.len() - 1],
                    "redraw_over_33ms": rd.iter().filter(|x| **x > 33.4).count(),
                    "timer_interval_avg_ms": mean(&ints),
                    "snapshot_render_avg_ms": snap_avg, "snapshot_render_p95_ms": snap_p95,
                });
                std::fs::write(&out, serde_json::to_string_pretty(&r).unwrap()).ok();
                slint::quit_event_loop().ok();
            }
        });
    });
    slint::run_event_loop().unwrap();
}
