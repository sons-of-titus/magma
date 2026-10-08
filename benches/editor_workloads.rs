//! Editor-workload benchmarks (docs/core-redesign.md §13).
//!
//! These measure interactive paths — typing, cursor movement, rendering,
//! undo traversal, change publication — not the rope in isolation.  The
//! gate: latency must scale with the affected region, never with the
//! whole document.

use std::time::Instant;

use magma::kernel::command::{self, builtin};
use magma::kernel::render::frame::render_frame;
use magma::kernel::render::surface::Surface;
use magma::kernel::state::Editor;
use magma::kernel::storage::disk::DiskFileSystem;

fn editor_with(content: &str) -> Editor {
    let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
    builtin::register_builtin_commands(&mut ed);
    let key = ed.create_buffer_from_str("bench", content);
    if let Some(win) = ed.view_tree.focused_window_mut() {
        win.buffer_id = Some(key);
    }
    ed
}

fn synthetic_doc(lines: usize) -> String {
    let mut s = String::with_capacity(lines * 40);
    for i in 0..lines {
        s.push_str(&format!("fn item_{i}() {{ let value_{i} = {i} * 2; }} // fn keyword\n"));
    }
    s
}

struct Stats {
    samples_ns: Vec<u64>,
}

impl Stats {
    fn new() -> Self {
        Stats { samples_ns: Vec::new() }
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
            "{name:<44} p50 {:>8.1}µs  p95 {:>8.1}µs  p99 {:>8.1}µs  max {:>9.1}µs  (n={n})",
            pct(50) as f64 / 1000.0,
            pct(95) as f64 / 1000.0,
            pct(99) as f64 / 1000.0,
            v[n - 1] as f64 / 1000.0,
        );
    }
}

const SAMPLES: usize = 400;

fn bench_typing(doc_bytes: usize) {
    let lines = doc_bytes / 40;
    let text = synthetic_doc(lines);
    let mut ed = editor_with(&text);
    let key = ed.view_tree.focused_window().and_then(|w| ed.view_tree.buffer(w)).unwrap();
    let mut stats = Stats::new();
    // Type in the middle of the document.
    let mid = text.len() / 2;
    ed.views.get_mut(&key).unwrap().set_cursor(mid);
    for i in 0..SAMPLES {
        let t0 = Instant::now();
        let view = ed.views.get_mut(&key).unwrap();
        let cur = view.cursor_offset();
        view.insert(cur, "x");
        ed.publish_buffer_changes();
        stats.push(t0.elapsed().as_nanos() as u64);
        let _ = i;
    }
    stats.report(&format!("typing + publish @ {doc_bytes:>9} B"));
}

fn bench_cursor(doc_bytes: usize) {
    let lines = doc_bytes / 40;
    let text = synthetic_doc(lines);
    let mut ed = editor_with(&text);
    let key = ed.view_tree.focused_window().and_then(|w| ed.view_tree.buffer(w)).unwrap();
    let mut stats = Stats::new();
    let step = (text.len() / SAMPLES).max(1);
    for i in 0..SAMPLES {
        let offset = (i * step).min(text.len());
        let t0 = Instant::now();
        ed.views.get_mut(&key).unwrap().set_cursor(offset);
        stats.push(t0.elapsed().as_nanos() as u64);
    }
    stats.report(&format!("cursor move          @ {doc_bytes:>9} B"));
}

fn bench_render(doc_bytes: usize) {
    let lines = doc_bytes / 40;
    let text = synthetic_doc(lines);
    let ed = editor_with(&text);
    let mut surface = Surface::new(120, 40);
    // Warm-up (lazily built state, caches).
    render_frame(&ed, &mut surface, false);
    let mut stats = Stats::new();
    for _ in 0..60 {
        let t0 = Instant::now();
        render_frame(&ed, &mut surface, false);
        stats.push(t0.elapsed().as_nanos() as u64);
    }
    stats.report(&format!("render_frame 40 rows @ {doc_bytes:>9} B"));
}

fn bench_undo(count: usize) {
    let text = synthetic_doc(200);
    let mut ed = editor_with(&text);
    let key = ed.view_tree.focused_window().and_then(|w| ed.view_tree.buffer(w)).unwrap();
    for i in 0..count {
        let view = ed.views.get_mut(&key).unwrap();
        let cur = view.cursor_offset();
        view.insert(cur, &format!("x{i};"));
    }
    let t0 = Instant::now();
    let undone = {
        let view = ed.views.get_mut(&key).unwrap();
        let mut n = 0;
        while view.undo() {
            n += 1;
        }
        n
    };
    let undo_ns = t0.elapsed().as_nanos() as u64;
    let t0 = Instant::now();
    let redone = {
        let view = ed.views.get_mut(&key).unwrap();
        let mut n = 0;
        while view.redo() {
            n += 1;
        }
        n
    };
    let redo_ns = t0.elapsed().as_nanos() as u64;
    println!(
        "undo/redo {count:>6} edits          {undone:>6} undone in {:>8.2}ms, {redone:>6} redone in {:>8.2}ms",
        undo_ns as f64 / 1e6,
        redo_ns as f64 / 1e6,
    );
}

fn bench_save_snapshot(doc_bytes: usize) {
    let lines = doc_bytes / 40;
    let text = synthetic_doc(lines);
    let mut ed = editor_with(&text);
    let key = ed.view_tree.focused_window().and_then(|w| ed.view_tree.buffer(w)).unwrap();
    let mut stats = Stats::new();
    for _ in 0..30 {
        let t0 = Instant::now();
        // The UI-blocking portion of a save: snapshot + revision capture.
        {
            let buf = ed.buffers.get(key).unwrap().lock().unwrap();
            let content = buf.slice(0, buf.len());
            std::hint::black_box(&content);
        }
        let view = ed.views.get_mut(&key).unwrap();
        view.buffer.lock().unwrap().mark_saved();
        stats.push(t0.elapsed().as_nanos() as u64);
    }
    stats.report(&format!("save snapshot        @ {doc_bytes:>9} B"));
}

fn main() {
    println!("── Magma editor workloads ──────────────────────────────────────────");
    for size in [100_000usize, 1_000_000, 10_000_000] {
        bench_typing(size);
    }
    for size in [100_000usize, 1_000_000, 10_000_000, 100_000_000] {
        bench_cursor(size);
    }
    for size in [100_000usize, 1_000_000, 10_000_000] {
        bench_render(size);
    }
    for n in [100usize, 10_000] {
        bench_undo(n);
    }
    for size in [1_000_000usize, 10_000_000] {
        bench_save_snapshot(size);
    }
    let _ = command::execute_command; // keep import used
}
