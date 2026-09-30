use slint::{ModelRc, SharedString, VecModel};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

slint::slint! {
    import { ListView, TextEdit, LineEdit, TabWidget, Button } from "std-widgets.slint";
    export struct DiffLine { kind: int, old-no: string, new-no: string, text: string }

    component DiffView inherits Rectangle {
        background: #1e1e1e;
        in property <[DiffLine]> lines;
        in property <string> mono;
        in-out property <length> scroll-y;
        ListView {
            content-y <=> root.scroll-y;
            for l in root.lines: Rectangle {
                height: 18px;
                background: l.kind == 1 ? #1f3d24 : l.kind == 2 ? #4b1f22 : l.kind == 3 ? #2a3550 : transparent;
                Text { x: 0px; width: 44px; height: 18px; text: l.old-no; color: #7a7a7a; font-size: 12px; font-family: root.mono; horizontal-alignment: right; vertical-alignment: center; }
                Text { x: 48px; width: 44px; height: 18px; text: l.new-no; color: #7a7a7a; font-size: 12px; font-family: root.mono; horizontal-alignment: right; vertical-alignment: center; }
                Text { x: 100px; width: parent.width - 100px; height: 18px; text: l.text; overflow: elide; font-size: 12px; font-family: root.mono;
                       color: l.kind == 1 ? #b5f0b8 : l.kind == 2 ? #f0b5b8 : l.kind == 3 ? #9cb8f0 : #d4d4d4; vertical-alignment: center; }
            }
        }
    }

    export component App inherits Window {
        in property <bool> bench;
        in property <[DiffLine]> lines;
        in property <string> mono;
        in property <string> status;
        in-out property <length> scroll-y;
        in-out property <string> big-text;
        in-out property <int> tab: 0;
        callback load-big();
        width: 1200px;
        height: 780px;
        background: root.bench ? #1e1e1e : #f3f3f3;

        if root.bench: DiffView { lines: root.lines; mono: root.mono; scroll-y <=> root.scroll-y; }
        if !root.bench: TabWidget {
            current-index <=> root.tab;
            Tab {
                title: "한글 입력";
                VerticalLayout {
                    padding: 12px; spacing: 8px;
                    Text { color: #202020; wrap: word-wrap; text: "확인 항목: ① 한글 조합(ㅎ→하→한→한ㄱ→한글) 중 글자가 커서 위치에 밑줄로 표시되는가 ② 조합 중 Enter/Backspace/방향키 ③ 드래그 선택, Cmd/Ctrl+A·C·V·Z ④ IME 후보창이 커서 근처에 뜨는가 ⑤ 한/영 전환 ⑥ 이모지·한자 입력 ⑦ 조합 중 창 포커스를 바꿨다 돌아왔을 때"; }
                    Text { color: #202020; text: "렌더링 확인(입력이 아니라 표시): 😀 👍 🎉 漢字 한글 ABC"; font-size: 16px; }
                    LineEdit { placeholder-text: "여기에 한글을 입력해 보세요 (한 줄)"; }
                    TextEdit { text: "여러 줄 입력 영역입니다.\n조합 중 줄바꿈과 붙여넣기를 확인해 보세요.\n"; }
                }
            }
            Tab {
                title: "Diff 뷰";
                DiffView { lines: root.lines; mono: root.mono; scroll-y <=> root.scroll-y; }
            }
            Tab {
                title: "큰 텍스트";
                VerticalLayout {
                    padding: 8px; spacing: 8px;
                    HorizontalLayout { spacing: 12px;
                        Button { text: "약 5MB 텍스트 불러오기"; clicked => { root.load-big(); } }
                        Text { color: #202020; text: root.status; vertical-alignment: center; }
                    }
                    TextEdit { text <=> root.big-text; read-only: true; }
                }
            }
        }
    }
}

const ROW: f32 = 18.0;
const STEP_ROWS: f32 = 90.0;
const FRAMES: usize = 900;

fn gen_diff(n: usize) -> Vec<DiffLine> {
    // deterministic synthetic unified diff: hunks of ~40 lines, mixed English/Korean, a few very long lines
    let mut v = Vec::with_capacity(n);
    let (mut old, mut new) = (1u32, 1u32);
    let mut i = 0usize;
    while v.len() < n {
        v.push(DiffLine { kind: 3, old_no: "".into(), new_no: "".into(),
            text: format!("@@ -{old},38 +{new},41 @@ fn 함수_{i}(인자: &str) -> Result<(), Error>").into() });
        for k in 0..40usize {
            let long = if (i + k) % 97 == 0 { " // 아주 긴 한글 주석입니다 ".repeat(12) } else { String::new() };
            let (kind, o, nn, t): (i32, String, String, String) = match k % 7 {
                2 => (2, old.to_string(), String::new(), format!("    let value_{k} = compute(input_{i}); // 이전 구현{long}")),
                3 | 4 => (1, String::new(), new.to_string(), format!("    let value_{k} = compute_v2(input_{i}, &ctx)?; // 새 구현 완료{long}")),
                _ => (0, old.to_string(), new.to_string(), format!("    if items.len() > {k} {{ process(&items[{k}]); }} // 항목 처리{long}")),
            };
            match kind { 2 => old += 1, 1 => new += 1, _ => { old += 1; new += 1 } }
            v.push(DiffLine { kind, old_no: o.into(), new_no: nn.into(), text: t.into() });
        }
        i += 1;
    }
    v.truncate(n);
    v
}

fn pct(s: &[f64], p: f64) -> f64 { if s.is_empty() { 0.0 } else { s[((s.len() - 1) as f64 * p) as usize] } }
fn mean(a: &[f64]) -> f64 { if a.is_empty() { 0.0 } else { a.iter().sum::<f64>() / a.len() as f64 } }

/// Interactive mode only: log the raw winit input events, and work around Cmd+A/C/V/X/Z not
/// working while a non-Latin input source (e.g. Korean 2-set) is selected on macOS.
/// Slint matches its standard shortcuts against the key *text*; with the Korean input source that
/// text is "ㅊ" for the C key, so Cmd+C never matches. We re-dispatch those shortcuts as Latin text.
/// Disable with BENCH_CMD_FIX=0, silence the log with BENCH_LOG_INPUT=0.
fn install_input_hooks(ui: &App) {
    use slint::winit_030::{winit, EventResult, WinitWindowAccessor};
    use winit::event::{ElementState, WindowEvent as We};
    use winit::keyboard::{Key, KeyCode, PhysicalKey};

    let cmd_fix = std::env::var("BENCH_CMD_FIX").as_deref() != Ok("0");
    let log_input = std::env::var("BENCH_LOG_INPUT").as_deref() != Ok("0");
    eprintln!("widgets: cmd_fix={cmd_fix} log_input={log_input}");
    let t0 = Instant::now();
    let super_down = std::cell::Cell::new(false);
    let weak = ui.as_weak();
    ui.window().on_winit_window_event(move |_w, ev| {
        let ms = t0.elapsed().as_secs_f64() * 1e3;
        match ev {
            We::ModifiersChanged(m) => super_down.set(m.state().super_key()),
            We::KeyboardInput { event, .. } => {
                if log_input && event.state == ElementState::Pressed {
                    eprintln!("[{ms:9.1}] Key {:?} logical={:?} text={:?} cmd={}", event.physical_key, event.logical_key, event.text, super_down.get());
                }
                if cmd_fix && event.state == ElementState::Pressed && super_down.get() {
                    if let PhysicalKey::Code(code) = event.physical_key {
                        let latin = match code { KeyCode::KeyA => Some("a"), KeyCode::KeyC => Some("c"), KeyCode::KeyV => Some("v"),
                            KeyCode::KeyX => Some("x"), KeyCode::KeyZ => Some("z"), _ => None };
                        if let Some(t) = latin {
                            let already = matches!(&event.logical_key, Key::Character(c) if c.eq_ignore_ascii_case(t));
                            if !already {
                                if log_input { eprintln!("[{ms:9.1}] cmd_fix: re-dispatching Cmd+{t}"); }
                                weak.clone().upgrade_in_event_loop(move |ui| {
                                    use slint::platform::WindowEvent as Se;
                                    ui.window().dispatch_event(Se::KeyPressed { text: t.into() });
                                    ui.window().dispatch_event(Se::KeyReleased { text: t.into() });
                                }).ok();
                                return EventResult::PreventDefault;
                            }
                        }
                    }
                }
            }
            We::Ime(i) if log_input => eprintln!("[{ms:9.1}] Ime {:?}", i),
            We::Focused(f) if log_input => eprintln!("[{ms:9.1}] Focused({f})"),
            _ => {}
        }
        EventResult::Propagate
    });
}

fn main() {
    let t_start = Instant::now();
    let scenario = std::env::var("BENCH_SCENARIO").unwrap_or_default();
    let idle = std::env::var("BENCH_MODE").as_deref() == Ok("idle");
    let bench = scenario == "diff" || idle;
    let out = std::env::var("BENCH_OUT").unwrap_or_else(|_| "/tmp/bench-widgets.json".into());

    let ui = App::new().unwrap();
    let t = Instant::now();
    let n = 200_000;
    let lines = gen_diff(n);
    let load_ms = t.elapsed().as_secs_f64() * 1e3;
    ui.set_lines(ModelRc::new(VecModel::from(lines)));
    ui.set_mono(SharedString::from(if cfg!(target_os = "macos") { "Menlo" } else if cfg!(windows) { "Consolas" } else { "DejaVu Sans Mono" }));
    ui.set_bench(bench);
    if let Some(t) = std::env::var("BENCH_TAB").ok().and_then(|v| v.parse().ok()) { ui.set_tab(t); } // start on a given tab (screenshots)

    // manual "big text" tab: build ~5MB of text and report how long setting it took
    let ui_weak = ui.as_weak();
    ui.on_load_big(move || {
        let ui = ui_weak.unwrap();
        let t = Instant::now();
        let mut s = String::with_capacity(6_000_000);
        for i in 0..100_000 { s.push_str(&format!("{i:06}  로그 줄 번호 {i} — the quick brown fox jumps over the lazy dog\n")); }
        let build_ms = t.elapsed().as_secs_f64() * 1e3;
        let t = Instant::now();
        ui.set_big_text(s.into());
        ui.set_status(format!("텍스트 생성 {build_ms:.0} ms, 설정 {:.0} ms (첫 화면 갱신은 이후 체감으로 확인)", t.elapsed().as_secs_f64() * 1e3).into());
    });
    ui.show().unwrap();

    if !bench {
        eprintln!("widgets: interactive mode (renderer via SLINT_BACKEND)");
        install_input_hooks(&ui);
        slint::run_event_loop().unwrap();
        return;
    }

    let display_hz: f64 = std::env::var("BENCH_HZ").ok().and_then(|v| v.parse().ok()).unwrap_or_else(|| {
        use slint::winit_030::WinitWindowAccessor;
        ui.window()
            .with_winit_window(|w| w.current_monitor().and_then(|m| m.refresh_rate_millihertz()))
            .flatten()
            .map(|mhz| mhz as f64 / 1000.0)
            .filter(|hz| *hz >= 30.0 && *hz <= 500.0)
            .unwrap_or(60.0)
    });
    let tick = Duration::from_secs_f64(1.0 / display_hz);

    let first_render_ms = t_start.elapsed().as_secs_f64() * 1e3 + 50.0;
    slint::Timer::single_shot(Duration::from_millis(50), {
        let out = out.clone();
        move || { std::fs::write(format!("{out}.ready"), "").ok(); }
    });

    if idle {
        let idle_secs: f64 = std::env::var("BENCH_IDLE_SECS").ok().and_then(|v| v.parse().ok()).unwrap_or(10.0);
        let out_i = out.clone();
        slint::Timer::single_shot(Duration::from_millis(50) + Duration::from_secs_f64(idle_secs), move || {
            let r = serde_json::json!({ "framework": "slint-widgets", "scenario": "idle", "idle_s": idle_secs,
                "core_load_ms": load_ms, "first_render_ms": first_render_ms });
            std::fs::write(&out_i, serde_json::to_string_pretty(&r).unwrap()).ok();
            slint::quit_event_loop().ok();
        });
        slint::run_event_loop().unwrap();
        return;
    }

    // diff fling: same method as slint-app (redraw events counted through the winit hook)
    let redraws: Rc<RefCell<Vec<Instant>>> = Rc::new(RefCell::new(Vec::new()));
    let recording = Rc::new(std::cell::Cell::new(false));
    {
        use slint::winit_030::{winit, EventResult, WinitWindowAccessor};
        let (r, rec) = (redraws.clone(), recording.clone());
        ui.window().on_winit_window_event(move |_w, ev| {
            if rec.get() && matches!(ev, winit::event::WindowEvent::RedrawRequested) { r.borrow_mut().push(Instant::now()); }
            EventResult::Propagate
        });
    }
    let timer = Rc::new(slint::Timer::default());
    let (timer2, ui_weak, out2, redraws_in, rec_in) = (timer.clone(), ui.as_weak(), out.clone(), redraws.clone(), recording.clone());
    slint::Timer::single_shot(Duration::from_millis(750), move || {
        let max_scroll = n as f32 * ROW - 780.0;
        let mut pos = 0.0f32;
        let mut ints: Vec<f64> = Vec::with_capacity(FRAMES);
        let mut last = Instant::now();
        let mut k = 0usize;
        let timer3 = timer2.clone();
        timer2.start(slint::TimerMode::Repeated, tick, move || {
            let now = Instant::now();
            if k == 0 { redraws_in.borrow_mut().clear(); rec_in.set(true); }
            if k > 0 { ints.push((now - last).as_secs_f64() * 1e3); }
            last = now;
            pos = (pos + STEP_ROWS * ROW) % max_scroll;
            ui_weak.unwrap().set_scroll_y(-pos);
            k += 1;
            if k > FRAMES {
                timer3.stop();
                rec_in.set(false);
                let raw = redraws_in.borrow().len();
                let mut rd: Vec<f64> = redraws_in.borrow().windows(2).map(|w| (w[1] - w[0]).as_secs_f64() * 1e3).collect();
                let src = if rd.is_empty() { rd = ints.clone(); "timer" } else { "redraw" };
                let mut s = rd.clone(); s.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let r = serde_json::json!({
                    "framework": "slint-widgets", "scenario": "diff", "total": n, "core_load_ms": load_ms, "first_render_ms": first_render_ms,
                    "frames": FRAMES, "interval_avg_ms": mean(&rd), "interval_p50_ms": pct(&s, 0.5), "interval_p95_ms": pct(&s, 0.95),
                    "interval_p99_ms": pct(&s, 0.99), "interval_max_ms": s.last().copied().unwrap_or(0.0),
                    "frames_over_33ms": rd.iter().filter(|x| **x > 33.4).count(),
                    "frame_source": src, "display_hz": display_hz, "raw_redraws": raw,
                });
                std::fs::write(&out2, serde_json::to_string_pretty(&r).unwrap()).ok();
                slint::quit_event_loop().ok();
            }
        });
    });
    slint::run_event_loop().unwrap();
}
