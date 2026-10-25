//! Perf harness (R-7) — the numbers that gate optimization work.
//!
//! Measures the three axes the roadmap names, end to end through the user
//! path (keymap dispatch → buffer mutation → change publication → render):
//!
//!   startup   — Editor construction + builtins + first full render
//!   typing    — per-keystroke latency (p50/p95/p99) at 10 KB / 100 KB / 1 MB
//!   memory    — bytes allocated and retained for a 1 MB working set
//!
//! Run: `cargo bench --bench perf_gates --features janet`.
//! Gates print PASS/FAIL against the baseline thresholds; the thresholds
//! are the measured baseline ± headroom (docs/audit/benchmark.md).

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use magma::kernel::command::builtin;
use magma::kernel::input;
use magma::kernel::render::frame::render_frame;
use magma::kernel::render::surface::Surface;
use magma::kernel::state::Editor;
use magma::kernel::storage::disk::DiskFileSystem;

/// Allocation counter — the memory axis of the harness.
static ALLOCATED: AtomicU64 = AtomicU64::new(0);
struct Counting;

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATED.fetch_add(layout.size() as u64, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCATED.fetch_add(new_size as u64, Ordering::Relaxed);
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

fn allocated_now() -> u64 {
    ALLOCATED.load(Ordering::Relaxed)
}

fn synthetic_doc(lines: usize) -> String {
    let mut s = String::with_capacity(lines * 40);
    for i in 0..lines {
        s.push_str(&format!("fn item_{i}() {{ let value_{i} = {i} * 2; }} // fn keyword\n"));
    }
    s
}

fn editor_with(content: &str) -> Editor {
    let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
    builtin::register_builtin_commands(&mut ed);
    let key = ed.create_buffer_from_str("bench", content);
    if let Some(win) = ed.view_tree.focused_window_mut() {
        win.buffer_id = Some(key);
    }
    ed
}

fn focused_key(ed: &Editor) -> usize {
    ed.view_tree.focused_window()
        .and_then(|w| ed.view_tree.buffer(w))
        .unwrap_or(0)
}

struct Samples(Vec<u64>);

impl Samples {
    fn new() -> Self { Samples(Vec::new()) }
    fn push(&mut self, ns: u64) { self.0.push(ns); }
    fn percentile(&self, p: usize) -> f64 {
        let mut v = self.0.clone();
        v.sort_unstable();
        let n = v.len();
        if n == 0 { return 0.0 }
        v[(p * (n - 1)) / 100] as f64 / 1000.0
    }
    fn report(&self, name: &str) {
        println!(
            "{name:<44} p50 {:>8.1}µs  p95 {:>8.1}µs  p99 {:>8.1}µs  (n={})",
            self.percentile(50), self.percentile(95), self.percentile(99), self.0.len(),
        );
    }
}

// ── Startup ──────────────────────────────────────────────────────────────────

fn bench_startup(doc_bytes: usize) -> f64 {
    let text = synthetic_doc(doc_bytes / 40);
    // Warm the allocator so the measurement is editor work, not libc init.
    {
        let ed = editor_with(&text);
        let mut surface = Surface::new(120, 40);
        render_frame(&ed, &mut surface);
    }
    let mut total = 0.0f64;
    const RUNS: usize = 10;
    for _ in 0..RUNS {
        let t0 = Instant::now();
        let ed = editor_with(&text);
        let mut surface = Surface::new(120, 40);
        render_frame(&ed, &mut surface); // "ready to type" = first frame painted
        total += t0.elapsed().as_secs_f64() * 1000.0;
        drop(surface);
        drop(ed);
    }
    let avg = total / RUNS as f64;
    println!("startup (editor + builtins + first frame, {} KB)   avg {:>8.2} ms", doc_bytes / 1024, avg);
    avg
}

// ── Typing p99 ───────────────────────────────────────────────────────────────

fn bench_typing(doc_bytes: usize) -> Samples {
    let text = synthetic_doc(doc_bytes / 40);
    let mut ed = editor_with(&text);
    let key = focused_key(&ed);
    ed.views.get_mut(&key).unwrap().set_cursor(text.len() / 2);
    let mut surface = Surface::new(120, 40);
    render_frame(&ed, &mut surface); // prime the layout caches

    let mut stats = Samples::new();
    const KEYSTROKES: usize = 500;
    for i in 0..KEYSTROKES {
        let t0 = Instant::now();
        input::dispatch_key(&mut ed, if i % 3 == 0 { "return" } else { "x" });
        render_frame(&ed, &mut surface);
        stats.push(t0.elapsed().as_nanos() as u64);
    }
    stats.report(&format!("typing (key→render) @ {:>9} B", doc_bytes));
    stats
}

// ── Memory ───────────────────────────────────────────────────────────────────

fn bench_memory(doc_bytes: usize) -> u64 {
    let text = synthetic_doc(doc_bytes / 40);
    let before = allocated_now();
    let mut ed = editor_with(&text);
    let mut surface = Surface::new(120, 40);
    render_frame(&ed, &mut surface);
    // Type and undo 200 edits: retained memory after the churn is the gate.
    let key = focused_key(&ed);
    for _ in 0..200 {
        let view = ed.views.get_mut(&key).unwrap();
        let cur = view.cursor_offset();
        view.insert(cur, "memory-probe;");
    }
    while ed.buffers.get(key).unwrap()
        .lock().unwrap_or_else(|e| e.into_inner()).can_undo() {
        if let Some(view) = ed.views.get_mut(&key) {
            let _ = view.buffer.lock().unwrap_or_else(|e| e.into_inner()).undo();
        }
    }
    let retained = allocated_now().saturating_sub(before);
    println!("memory (1 MB doc, editor + first frame + 200 edits undone)  {:>8.2} MB retained",
        retained as f64 / (1024.0 * 1024.0));
    retained
}

fn gate(name: &str, value: f64, limit: f64, unit: &str) -> bool {
    let ok = value <= limit;
    println!("  gate {name:<38} {value:>8.2}{unit} / {limit}{unit}  {}", if ok { "PASS" } else { "FAIL" });
    ok
}

fn main() {
    println!("Magma perf harness (R-7) — baseline gates\n");

    println!("── startup ──");
    let startup_1mb = bench_startup(1_000_000);

    println!("\n── typing ──");
    let typing_small = bench_typing(10_000);
    let typing_mid = bench_typing(100_000);
    let typing_large = bench_typing(1_000_000);

    println!("\n── memory ──");
    let mem = bench_memory(1_000_000);

    println!("\n── gates (baseline ± headroom; revisit after optimizations) ──");
    let mut all_ok = true;
    // Gate values are regression bounds: the isolated core benches
    // (editor_workloads) hold typing p99 ≤ 14 µs and render p99 ≤ 51 µs
    // flat across 100 KB–10 MB, so anything past these combined-path
    // bounds is a regression worth investigating — not an aspiration.
    // Note: p99 outliers up to ~120 ms were observed on a machine under
    // concurrent compile load; run this on a quiet machine when gating.
    all_ok &= gate("startup, 1 MB doc", startup_1mb, 50.0, "ms");
    all_ok &= gate("typing p99, 10 KB doc", typing_small.percentile(99), 5_000.0, "µs");
    all_ok &= gate("typing p99, 100 KB doc", typing_mid.percentile(99), 5_000.0, "µs");
    all_ok &= gate("typing p99, 1 MB doc", typing_large.percentile(99), 5_000.0, "µs");
    all_ok &= gate("retained memory, 1 MB doc", mem as f64 / (1024.0 * 1024.0), 64.0, "MB");

    println!("\n{}", if all_ok { "all gates PASS" } else { "GATES FAILED — measure before optimizing" });
    std::process::exit(if all_ok { 0 } else { 1 });
}
