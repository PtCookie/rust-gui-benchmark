use bench_core::*;
use std::path::PathBuf;
use std::time::Instant;

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

fn pct(v: &mut Vec<f64>, p: f64) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[((v.len() as f64 - 1.0) * p) as usize]
}

fn main() {
    let repo: PathBuf = std::env::args().nth(1).expect("repo path").into();
    let db: PathBuf = std::env::args().nth(2).expect("db path").into();
    let rows: usize = std::env::args().nth(3).and_then(|s| s.parse().ok()).unwrap_or(1_000_000);

    let mut out = serde_json::Map::new();
    let rss0 = rss_kb();

    // ---- git walk: gix vs git2 (both decode author/summary)
    let t = Instant::now();
    let g = walk_gix(&repo, usize::MAX);
    let gix_ms = ms(t);
    match &g {
        Ok(v) => {
            out.insert("gix_commits".into(), v.len().into());
        }
        Err(e) => {
            out.insert("gix_error".into(), e.to_string().into());
        }
    }
    out.insert("gix_walk_ms".into(), gix_ms.into());
    drop(g);

    let t = Instant::now();
    let raw = walk_git2(&repo, usize::MAX).unwrap();
    out.insert("git2_walk_ms".into(), ms(t).into());
    out.insert("git2_commits".into(), raw.len().into());

    // ---- graph layout
    let t = Instant::now();
    let rows_v = layout(&raw);
    out.insert("layout_ms".into(), ms(t).into());
    out.insert(
        "max_lanes".into(),
        rows_v.iter().map(|r| r.width).max().unwrap_or(0).into(),
    );
    out.insert("core_rss_after_load_mb".into(), (rss_kb().saturating_sub(rss0) as f64 / 1024.0).into());

    // ---- IPC payload cost: whole vec vs 500-row chunks
    let t = Instant::now();
    let whole = serde_json::to_vec(&rows_v).unwrap();
    out.insert("json_whole_ms".into(), ms(t).into());
    out.insert("json_whole_mb".into(), (whole.len() as f64 / 1048576.0).into());
    let t = Instant::now();
    let mut total = 0usize;
    for ch in rows_v.chunks(500) {
        total += serde_json::to_vec(ch).unwrap().len();
    }
    out.insert("json_chunked_500_ms".into(), ms(t).into());
    out.insert("json_chunked_mb".into(), (total as f64 / 1048576.0).into());
    let t = Instant::now();
    let _back: Vec<CommitRow> = serde_json::from_slice(&whole).unwrap();
    out.insert("json_whole_parse_ms".into(), ms(t).into());
    drop(whole);

    // ---- sqlite grid
    let t = Instant::now();
    make_db(&db, rows).unwrap();
    out.insert("db_create_ms".into(), ms(t).into());
    let c = open_db(&db).unwrap();
    let t = Instant::now();
    let n = count_rows(&c);
    out.insert("db_count_ms".into(), ms(t).into());
    out.insert("db_rows".into(), n.into());

    let mut rng = 0x1234_5678_9abc_def0u64;
    let mut lat = Vec::new();
    for _ in 0..300 {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        let off = (rng % (n as u64 - 100)) as i64;
        let t = Instant::now();
        let p = grid_page(&c, off, 100);
        std::hint::black_box(&p);
        lat.push(ms(t));
    }
    out.insert("db_page100_p50_ms".into(), pct(&mut lat.clone(), 0.5).into());
    out.insert("db_page100_p95_ms".into(), pct(&mut lat, 0.95).into());

    let rss1 = rss_kb();
    let t = Instant::now();
    let all = grid_page(&c, 0, n);
    out.insert("db_load_all_ms".into(), ms(t).into());
    out.insert("db_load_all_rss_mb".into(), (rss_kb().saturating_sub(rss1) as f64 / 1024.0).into());
    out.insert("db_load_all_len".into(), all.len().into());

    println!("{}", serde_json::to_string_pretty(&out).unwrap());
}
