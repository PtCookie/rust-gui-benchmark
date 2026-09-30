use bench_core as core;
use once_cell::sync::OnceCell;
use std::path::PathBuf;
use std::sync::Mutex;

static COMMITS: OnceCell<Vec<core::CommitRow>> = OnceCell::new();
static DB: OnceCell<Mutex<core::DbConn>> = OnceCell::new();
static CORE_MS: Mutex<f64> = Mutex::new(0.0);

fn env(k: &str) -> String {
    std::env::var(k).unwrap_or_default()
}

#[tauri::command]
async fn open() -> serde_json::Value {
    let scenario = std::env::var("BENCH_SCENARIO").unwrap_or_else(|_| "commits".into());
    let t = std::time::Instant::now();
    let (total, mw) = if scenario == "commits" {
        let rows = core::load_commit_rows(&PathBuf::from(env("BENCH_REPO")), usize::MAX);
        let mw = rows.iter().map(|r| r.width).max().unwrap_or(1);
        let n = rows.len();
        COMMITS.set(rows).ok();
        (n, mw)
    } else {
        let c = core::open_db(&PathBuf::from(env("BENCH_DB"))).unwrap();
        let n = core::count_rows(&c) as usize;
        DB.set(Mutex::new(c)).ok();
        (n, 1)
    };
    *CORE_MS.lock().unwrap() = t.elapsed().as_secs_f64() * 1e3;
    serde_json::json!({
        "scenario": scenario, "total": total, "max_width": mw,
        "mode": std::env::var("BENCH_MODE").unwrap_or_default(),
        "idle_secs": std::env::var("BENCH_IDLE_SECS").ok().and_then(|v| v.parse::<f64>().ok()).unwrap_or(10.0),
    })
}

#[tauri::command]
async fn commits_chunk(offset: usize, count: usize) -> Vec<core::CommitRow> {
    let v = COMMITS.get().unwrap();
    let s = offset.min(v.len());
    let e = (s + count).min(v.len());
    v[s..e].to_vec()
}

#[tauri::command]
async fn grid_page(offset: i64, count: i64) -> Vec<core::GridRow> {
    let c = DB.get().unwrap().lock().unwrap();
    core::grid_page(&c, offset, count)
}

#[tauri::command]
async fn mark(name: String) {
    std::fs::write(format!("{}.mark_{}", env("BENCH_OUT"), name), "").ok();
}

#[tauri::command]
async fn ready() {
    std::fs::write(format!("{}.ready", env("BENCH_OUT")), "").ok();
}

#[tauri::command]
async fn report(app: tauri::AppHandle, r: serde_json::Value) {
    let mut r = r;
    r["framework"] = "tauri".into();
    r["core_load_ms"] = (*CORE_MS.lock().unwrap()).into();
    std::fs::write(env("BENCH_OUT"), serde_json::to_string_pretty(&r).unwrap()).ok();
    app.exit(0);
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![open, commits_chunk, grid_page, ready, report, mark])
        .run(tauri::generate_context!())
        .expect("tauri run");
}
