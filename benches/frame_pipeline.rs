//! Frame-pipeline and index-throughput benchmarks (rework.md §8).
//!
//! The frame pipeline measures what a repaint actually costs at the
//! kernel boundary: background-event drain + surface render of the
//! visible region + minimap cache behavior — O(visible), never O(document).
//! The index benches measure extraction throughput over synthetic
//! projects.  The gate: per-frame cost must be flat in document size.

use std::time::Instant;

use kernel::command::builtin;
use kernel::index;
use kernel::render::frame::render_frame; // noqa
use kernel::render::surface::Surface;
use kernel::runtime::BackgroundHandle;
use kernel::state::Editor;
use kernel::storage::disk::DiskFileSystem;

fn editor_with(content: &str) -> Editor {
    let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
    builtin::register_builtin_commands(&mut ed);
    let key = ed.create_buffer_from_str("bench.rs", content);
    if let Some(win) = ed.view_tree.focused_window_mut() {
        win.buffer_id = Some(key);
    }
    ed
}

fn synthetic_doc(lines: usize) -> String {
    let mut s = String::with_capacity(lines * 40);
    for i in 0..lines {
        s.push_str(&format!(
            "fn item_{i}() {{ let value_{i} = {i} * 2; }} // fn keyword\n"
        ));
    }
    s
}

struct Stats {
    samples_ns: Vec<u64>,
}

impl Stats {
    fn new() -> Self {
        Stats {
            samples_ns: Vec::new(),
        }
    }

    fn push(&mut self, ns: u64) {
        self.samples_ns.push(ns);
    }

    fn report(&self, name: &str) {
        let mut v = self.samples_ns.clone();
        v.sort_unstable();
        let n = v.len();
        if n == 0 {
            return;
        }
        let pct = |p: usize| v[(p * (n - 1)) / 100];
        println!(
            "{name:<44} p50 {:>8.1}µs  p95 {:>8.1}µs  p99 {:>8.1}µs  (n={n})",
            pct(50) as f64 / 1000.0,
            pct(95) as f64 / 1000.0,
            pct(99) as f64 / 1000.0,
        );
    }
}

fn main() {
    println!("── Frame pipeline (kernel render boundary) ─────────────");

    // Surface render of 80x40 visible cells, document size swept.
    for (label, lines) in [
        ("render 80x40 @ 1k lines", 1_000),
        ("render 80x40 @ 100k lines", 100_000),
    ] {
        let ed = editor_with(&synthetic_doc(lines));
        let mut stats = Stats::new();
        for _ in 0..300 {
            let mut surface = Surface::new(80, 40);
            let t = Instant::now();
            render_frame(&ed, &mut surface);
            stats.push(t.elapsed().as_nanos() as u64);
        }
        stats.report(label);
    }

    // Frame cost when the buffer CHANGED: frame_dirty invalidation must
    // stay O(1) to observe (no all-buffers signature hashing).
    let mut ed = editor_with(&synthetic_doc(10_000));
    let mut stats = Stats::new();
    for i in 0..300 {
        let key = ed.focused_buffer_key();
        if let Some(arc) = ed.buffers.get(key) {
            let mut buf = arc.lock().unwrap();
            let end = buf.len();
            buf.insert(end, &format!("x{i};"));
        }
        let t = Instant::now();
        let _dirty = ed.frame_dirty;
        ed.frame_dirty = false;
        stats.push(t.elapsed().as_nanos() as u64);
    }
    stats.report("invalidation observe+clear @ 10k lines");

    // Background-event drain cost (empty channel steady state).
    let runtime = std::sync::Arc::new(tokio::runtime::Runtime::new().unwrap());
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let mut ed = editor_with(&synthetic_doc(1_000));
    ed.background = Some(BackgroundHandle::new(runtime.clone(), tx.clone()));
    let mut stats = Stats::new();
    for _ in 0..300 {
        let t = Instant::now();
        while let Ok(event) = rx.try_recv() {
            kernel::runtime::process_background_event(&mut ed, event);
        }
        stats.push(t.elapsed().as_nanos() as u64);
    }
    stats.report("drain empty background channel");

    println!("── Index throughput ────────────────────────────");

    let files: Vec<String> = (0..1_000)
        .map(|i| synthetic_doc(50).replace("item_", &format!("item{i}_")))
        .collect();

    let mut stats = Stats::new();
    for (i, file) in files.iter().enumerate() {
        let t = Instant::now();
        let rec = index::record_from_text(&format!("mod{i}.rs"), 1, file);
        let mut idx = index::IndexStore::default();
        idx.apply(rec);
        stats.push(t.elapsed().as_nanos() as u64);
    }
    stats.report("extract+apply 2k-line file (1k files)");

    // Query latency on a full store.
    let mut idx = index::IndexStore::default();
    for (i, file) in files.iter().enumerate() {
        idx.apply(index::record_from_text(&format!("mod{i}.rs"), 1, file));
    }
    let mut stats = Stats::new();
    for prefix in ["item1", "item99", "fn", "zzz"] {
        let t = Instant::now();
        let hits = idx.symbols_with_prefix(prefix);
        stats.push(t.elapsed().as_nanos() as u64);
        let _ = hits;
    }
    stats.report("workspace symbol query (50k symbols)");

    // Large-file scaling: extraction must be linear-ish, not quadratic.
    for lines in [5_000, 50_000] {
        let doc = synthetic_doc(lines);
        let t = Instant::now();
        let rec = index::record_from_text("big.rs", 1, &doc);
        let elapsed = t.elapsed();
        println!(
            "{lines:>7}-line extraction {:>10.1}ms ({} symbols)",
            elapsed.as_secs_f64() * 1000.0,
            rec.symbols.len()
        );
    }
}
