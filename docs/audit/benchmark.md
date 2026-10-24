# Deliverable E — Benchmark Report

## Environment

macOS (darwin 25.6.0, arm64), debug profile, `--features janet`.
Benchmarks are the repository's own (`benches/`, hand-rolled percentile
harness, not criterion). JetBrains comparative numbers are **not**
measured here — no fair same-hardware same-project protocol was
available in this environment, so no comparative claims are made.

## Measured

### Frame pipeline (`benches/frame_pipeline.rs`)

Render-frame cost over synthetic documents of 1k / 10k / 100k lines:
cost is dominated by visible rows, not document length — flat scaling
confirmed (this property is also pinned by tests). This is the single
most important scalability fact about the editor: scrolling a 100k-line
file costs the same frame as a 1k-line file.

### Buffer/rope ops (`benches/buffer_ops.rs`, `rope_ops.rs`)

Insert/delete at head/middle/tail are ropey-log and unchanged by this
audit's fixes (no text-engine hot path was touched except
`BufferView::replace` usage in reload, which is off the typing path).

### Editor workloads (`benches/editor_workloads.rs`)

Typing-latency path (dispatch → txn → publish) remains synchronous and
short; the audit's changes add no work to it (the event-bus guard is a
small Vec scan only during dispatch).

### Task pipeline (new regression as a benchmark proxy)

`stderr_heavy_task_with_no_stdout_completes`: 1 MB stderr, zero stdout,
completes in <1 s post-fix; pre-fix it deadlocks (unbounded). This is a
correctness fix with a wall-clock observable.

## Not measured (honest gaps)

- Cold/warm startup, time-to-interactive: requires GUI launch harness;
  startup path is fully synchronous today (Janet init of 50 files +
  project reopen before first frame) and is a known P2 (F-24 area) —
  measure before optimizing.
- Typing latency p50/p95/p99 under load: needs an instrumented event
  loop; the O(document) motion slices (F-12) are the known worst case.
- Memory: unbounded event channel + per-line String churn are the two
  known growth sources; no leak was demonstrated.

## Comparison to JetBrains — method only

Any credible comparison needs identical project fixtures, release
builds, same machine, scripted keystroke replay, and screen-capture
latency measurement. None of that exists in this repo yet; building it
is roadmap item R-7. Until then, per-dimension comparisons in the
capability matrix are qualitative and evidence-cited, not numeric.
