use bench_core as core;
use napi_derive::napi;
use once_cell::sync::OnceCell;
use std::path::PathBuf;
use std::sync::Mutex;

static COMMITS: OnceCell<Vec<core::CommitRow>> = OnceCell::new();
static DB: OnceCell<Mutex<core::DbConn>> = OnceCell::new();

#[napi(object)]
pub struct OpenInfo {
    pub scenario: String,
    pub total: f64,
    pub max_width: u32,
    pub core_load_ms: f64,
}

#[napi(object)]
pub struct CommitRowJs {
    pub short: String,
    pub author: String,
    pub time: f64,
    pub summary: String,
    pub lane: u32,
    pub through: Vec<u32>,
    #[napi(js_name = "merge_to")]
    pub merge_to: Vec<u32>,
    pub width: u32,
}

#[napi(object)]
pub struct GridRowJs {
    pub id: f64,
    pub user: String,
    pub kind: String,
    pub amount: f64,
    pub ts: f64,
    pub note: String,
}

#[napi]
pub fn open(scenario: String, repo: String, db: String) -> OpenInfo {
    let t = std::time::Instant::now();
    if scenario == "commits" {
        let rows = core::load_commit_rows(&PathBuf::from(repo), usize::MAX);
        let mw = rows.iter().map(|r| r.width).max().unwrap_or(1) as u32;
        let n = rows.len();
        COMMITS.set(rows).ok();
        OpenInfo { scenario, total: n as f64, max_width: mw, core_load_ms: t.elapsed().as_secs_f64() * 1e3 }
    } else {
        let c = core::open_db(&PathBuf::from(db)).unwrap();
        let n = core::count_rows(&c);
        DB.set(Mutex::new(c)).ok();
        OpenInfo { scenario, total: n as f64, max_width: 1, core_load_ms: t.elapsed().as_secs_f64() * 1e3 }
    }
}

#[napi]
pub fn commits_chunk(offset: u32, count: u32) -> Vec<CommitRowJs> {
    let v = COMMITS.get().unwrap();
    let s = (offset as usize).min(v.len());
    let e = (s + count as usize).min(v.len());
    v[s..e]
        .iter()
        .map(|r| CommitRowJs {
            short: r.short.clone(),
            author: r.author.clone(),
            time: r.time as f64,
            summary: r.summary.clone(),
            lane: r.lane as u32,
            through: r.through.iter().map(|x| *x as u32).collect(),
            merge_to: r.merge_to.iter().map(|x| *x as u32).collect(),
            width: r.width as u32,
        })
        .collect()
}

#[napi]
pub fn grid_page(offset: f64, count: u32) -> Vec<GridRowJs> {
    let c = DB.get().unwrap().lock().unwrap();
    core::grid_page(&c, offset as i64, count as i64)
        .into_iter()
        .map(|r| GridRowJs { id: r.id as f64, user: r.user, kind: r.kind, amount: r.amount, ts: r.ts as f64, note: r.note })
        .collect()
}
