//! UI-independent core shared by every framework prototype.
//! Only the UI layer differs between Electron / Tauri / Slint / GPUI.

use rusqlite::{params, Connection};
pub use rusqlite::Connection as DbConn;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub type RawId = [u8; 20];

/// Minimal commit info produced by a history walk (topological order).
#[derive(Clone, Debug)]
pub struct RawCommit {
    pub id: RawId,
    pub parents: Vec<RawId>,
    pub author: String,
    pub time: i64,
    pub summary: String,
}

/// One row of the commit list, ready for any UI.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommitRow {
    pub short: String,
    pub author: String,
    pub time: i64,
    pub summary: String,
    /// lane index the commit dot sits on
    pub lane: u16,
    /// lanes occupied by lines passing through this row
    pub through: Vec<u16>,
    /// lanes that the extra (merge) parents connect to
    pub merge_to: Vec<u16>,
    /// max lane count at this row (for drawing width)
    pub width: u16,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GridRow {
    pub id: i64,
    pub user: String,
    pub kind: String,
    pub amount: f64,
    pub ts: i64,
    pub note: String,
}

pub fn hex7(id: &RawId) -> String {
    let mut s = String::with_capacity(7);
    for b in &id[..4] {
        s.push_str(&format!("{:02x}", b));
    }
    s.truncate(7);
    s
}

// ---------------------------------------------------------------- git: gix

pub fn walk_gix(path: &Path, limit: usize) -> Result<Vec<RawCommit>, Box<dyn std::error::Error>> {
    let repo = gix::open(path)?;
    let head = repo.head_id()?.detach();
    let walk = repo
        .rev_walk([head])
        .sorting(gix::revision::walk::Sorting::ByCommitTime(
            gix::traverse::commit::simple::CommitTimeOrder::NewestFirst,
        ))
        .all()?;
    let mut out = Vec::with_capacity(limit.min(1 << 20));
    for info in walk {
        if out.len() >= limit {
            break;
        }
        let info = info?;
        let commit = repo.find_commit(info.id)?;
        let decoded = commit.decode().map_err(|e| e.to_string())?;
        let author = decoded.author().map_err(|e| e.to_string())?;
        let mut id = [0u8; 20];
        id.copy_from_slice(info.id.as_bytes());
        let mut parents = Vec::with_capacity(info.parent_ids.len());
        for p in info.parent_ids.iter() {
            let mut b = [0u8; 20];
            b.copy_from_slice(p.as_bytes());
            parents.push(b);
        }
        let summary = decoded
            .message
            .to_string()
            .lines()
            .next()
            .unwrap_or("")
            .to_string();
        out.push(RawCommit {
            id,
            parents,
            author: author.name.to_string(),
            time: author.seconds(),
            summary,
        });
    }
    Ok(out)
}

// -------------------------------------------------------------- git: git2

pub fn walk_git2(path: &Path, limit: usize) -> Result<Vec<RawCommit>, Box<dyn std::error::Error>> {
    let repo = git2::Repository::open(path)?;
    let mut rw = repo.revwalk()?;
    rw.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::TIME)?;
    rw.push_head()?;
    let mut out = Vec::with_capacity(limit.min(1 << 20));
    for oid in rw {
        if out.len() >= limit {
            break;
        }
        let oid = oid?;
        let c = repo.find_commit(oid)?;
        let mut id = [0u8; 20];
        id.copy_from_slice(oid.as_bytes());
        let parents = c
            .parent_ids()
            .map(|p| {
                let mut b = [0u8; 20];
                b.copy_from_slice(p.as_bytes());
                b
            })
            .collect();
        out.push(RawCommit {
            id,
            parents,
            author: c.author().name().unwrap_or("").to_string(),
            time: c.time().seconds(),
            summary: c.summary().ok().flatten().unwrap_or("").to_string(),
        });
    }
    Ok(out)
}

// ------------------------------------------------------------- graph layout

/// Classic lane assignment. Input must be in topological order (children first).
pub fn layout(commits: &[RawCommit]) -> Vec<CommitRow> {
    let mut lanes: Vec<Option<RawId>> = Vec::new();
    let mut rows = Vec::with_capacity(commits.len());
    for c in commits {
        // lanes waiting for this commit (several children may converge)
        let waiting: Vec<usize> = lanes
            .iter()
            .enumerate()
            .filter_map(|(i, l)| (l.as_ref() == Some(&c.id)).then_some(i))
            .collect();
        let lane = match waiting.first() {
            Some(&i) => i,
            None => match lanes.iter().position(|l| l.is_none()) {
                Some(i) => i,
                None => {
                    lanes.push(None);
                    lanes.len() - 1
                }
            },
        };
        let through: Vec<u16> = lanes
            .iter()
            .enumerate()
            .filter(|(i, l)| l.is_some() && *i != lane && !waiting.contains(i))
            .map(|(i, _)| i as u16)
            .collect();
        for &i in &waiting {
            lanes[i] = None;
        }
        lanes[lane] = c.parents.first().copied();
        let mut merge_to = Vec::new();
        for p in c.parents.iter().skip(1) {
            let slot = match lanes.iter().position(|l| l.as_ref() == Some(p)) {
                Some(i) => i,
                None => {
                    let i = match lanes.iter().position(|l| l.is_none()) {
                        Some(i) => i,
                        None => {
                            lanes.push(None);
                            lanes.len() - 1
                        }
                    };
                    lanes[i] = Some(*p);
                    i
                }
            };
            merge_to.push(slot as u16);
        }
        while matches!(lanes.last(), Some(None)) {
            lanes.pop();
        }
        rows.push(CommitRow {
            short: hex7(&c.id),
            author: c.author.clone(),
            time: c.time,
            summary: c.summary.clone(),
            lane: lane as u16,
            through,
            merge_to,
            width: lanes.len().max(lane + 1) as u16,
        });
    }
    rows
}

pub fn load_commit_rows(path: &Path, limit: usize) -> Vec<CommitRow> {
    let raw = walk_git2(path, limit).expect("walk");
    layout(&raw)
}

// --------------------------------------------------------------------- sqlite

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

pub fn make_db(path: &Path, rows: usize) -> rusqlite::Result<()> {
    if path.exists() {
        let c = Connection::open(path)?;
        let n: i64 = c
            .query_row("SELECT COUNT(*) FROM events", [], |r| r.get(0))
            .unwrap_or(0);
        if n as usize == rows {
            return Ok(());
        }
        drop(c);
        std::fs::remove_file(path).ok();
    }
    let mut c = Connection::open(path)?;
    c.execute_batch(
        "PRAGMA journal_mode=OFF; PRAGMA synchronous=OFF;
         CREATE TABLE events(id INTEGER PRIMARY KEY, user TEXT, kind TEXT, amount REAL, ts INTEGER, note TEXT);",
    )?;
    let kinds = ["click", "view", "purchase", "refund", "login", "logout"];
    let mut rng = Rng(0x9E3779B97F4A7C15);
    let tx = c.transaction()?;
    {
        let mut st = tx.prepare("INSERT INTO events VALUES (?1,?2,?3,?4,?5,?6)")?;
        for i in 0..rows as i64 {
            let r = rng.next();
            st.execute(params![
                i,
                format!("user_{:05}", r % 50_000),
                kinds[(r >> 20) as usize % kinds.len()],
                (r % 1_000_000) as f64 / 100.0,
                1_600_000_000 + (r >> 8) as i64 % 100_000_000,
                format!("note {} lorem ipsum dolor sit amet", r % 9973),
            ])?;
        }
    }
    tx.commit()?;
    c.execute_batch("CREATE INDEX idx_user ON events(user);")?;
    Ok(())
}

pub fn open_db(path: &Path) -> rusqlite::Result<Connection> {
    Connection::open(path)
}

pub fn count_rows(c: &Connection) -> i64 {
    c.query_row("SELECT COUNT(*) FROM events", [], |r| r.get(0)).unwrap()
}

pub fn grid_page(c: &Connection, offset: i64, limit: i64) -> Vec<GridRow> {
    let mut st = c
        .prepare_cached("SELECT id,user,kind,amount,ts,note FROM events WHERE id >= ?1 ORDER BY id LIMIT ?2")
        .unwrap();
    st.query_map(params![offset, limit], |r| {
        Ok(GridRow {
            id: r.get(0)?,
            user: r.get(1)?,
            kind: r.get(2)?,
            amount: r.get(3)?,
            ts: r.get(4)?,
            note: r.get(5)?,
        })
    })
    .unwrap()
    .map(|x| x.unwrap())
    .collect()
}

/// resident set size of this process in KiB (Linux: /proc, macOS/Windows: memory-stats)
pub fn rss_kb() -> u64 {
    #[cfg(target_os = "linux")]
    {
        std::fs::read_to_string("/proc/self/status")
            .ok()
            .and_then(|s| {
                s.lines()
                    .find(|l| l.starts_with("VmRSS:"))
                    .and_then(|l| l.split_whitespace().nth(1)?.parse().ok())
            })
            .unwrap_or(0)
    }
    #[cfg(not(target_os = "linux"))]
    {
        memory_stats::memory_stats().map(|m| m.physical_mem as u64 / 1024).unwrap_or(0)
    }
}
