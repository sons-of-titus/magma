# Magma — Target Architecture (Rework)

Companion to `rework-map.md` (as-is truth). This document is the law the
rework implements. It is written before the code; deviations get recorded
here, not silently ignored.

---

## 1. Ownership model

```
Foreground thread (GUI loop / headless server loop)
  owns Editor by value — no global lock on the interactive path
    ├── documents arena   slab<Arc<Mutex<Buffer>>>   (shared with save workers only)
    ├── interaction state views, cursors, modes, marks, registers
    ├── command/keymap/event infrastructure
    └── service handles   index, lsp, vcs, terminal, scheduler, dap …

Background threads (tokio)
  never touch Editor.  They own their subsystem state and speak in
  revision-tagged messages on one channel.

Presentation (GUI thread = foreground thread)
  consumes prepared state; O(visible) per frame; invalidation-driven.
```

Rules:

1. **One owner.** `Editor` is owned by the foreground loop. The
   `Arc<RwLock<Editor>>` alias disappears from production wiring; the
   headless server is a second foreground loop that owns its own editor
   and services requests from a channel.
2. **Background results are data, not shared state.** Every async result
   carries the buffer key + document revision it was computed against.
   The foreground applies it only if the revision still matches, or
   explicitly reconciles. Stale → discarded.
3. **Buffers stay `Arc<Mutex<Buffer>>`** for exactly two external
   consumers: the async save path (snapshot out, `mark_saved_at` in) and
   streaming consumers (tree-sitter). Nothing else takes a buffer lock.
4. **No `Arc<RwLock<Everything>>`.** Any new cross-thread state gets an
   owner and a message type, not a lock.

## 2. Revision discipline

`Revision(u64)` per document (already exists). Extended to a tagged
result envelope used by every async consumer:

```rust
struct Revisioned<T> { buffer: usize, revision: u64, payload: T }
```

Applies to: diagnostics, completion, hover, definition, semantic
highlights, tree-sitter trees (already keyed by revision — must actually
move), index entries, VCS decorations, search results. `changes_since`
is the incremental feed; when history has been compacted past the
requested revision the consumer gets `None` (full re-sync) — never
silently-wrong partial data.

## 3. Execution pipeline

```
OS input → normalize → command resolve → mutation (µs)
  → revision++ → schedule follow-ups (tree-sitter, diagnostics, index)
  → return to UI
```

Hard rules:

- A keystroke never synchronously runs: LSP, tree-sitter, regex scans,
  plugins, project analysis, disk IO, full-document copies.
- Follow-up work goes through a **priority scheduler** with bounded
  concurrency and coalescing:

  | Lane | Concurrency | Examples |
  |------|-------------|----------|
  | P0 interactive | inline | keystroke mutations, completion popup on cache |
  | P1 foreground  | 2 | diagnostics/parse of visible buffers, completion fetch |
  | P2 indexing    | cpu-1 | project walk, symbol extraction |
  | P3 maintenance | 1 | persistence flush, VCS refresh, cleanup |

  Later submission of the same coalesce-key cancels the queued earlier
  one (typing wins over stale parses).

## 4. Language services

- **LSP**: real lifecycle — spawn → initialize (awaited, timeout) →
  initialized → didOpen/didChange (versioned) → requests with per-server
  pending map → cancellation → shutdown/exit; crash detection with
  restart policy; responses correlated by (server, request id), not by
  path; stale responses rejected by revision.
- **tree-sitter**: one persistent tree per buffer, `InputEdit` applied
  from `changes_since`, reparsed on the scheduler (P1), revision-checked
  on completion. Grammars compiled in where available; dynamic loading
  keeps an explicit registry with real error reporting.
- **Diagnostics**: typed `Diagnostic` everywhere; the string formatting
  is display-only, never a transport.

## 5. Project model & index

- Workspace → projects → targets/tasks model with provider detection
  (Cargo first), not `Vec<String>`.
- **Persistent incremental index** under the user cache dir:
  schema-versioned, per-file records (mtime/size/hash → symbols,
  language, metadata), crash-safe via temp+rename, loaded on open,
  updated incrementally on file events and saves, discardable on schema
  mismatch or corruption. Observable counters (files, symbols, queue
  depth, last latency).
- Symbol extraction: tree-sitter queries when a grammar exists; a real
  regex scanner per language family as fallback. Both real code paths,
  both revision-tagged.

## 6. Subsystem contracts

- **Terminal**: real emulator (grid, cursor addressing, SGR colors,
  scroll regions, alt screen, resize), PTY lifecycle with exit codes,
  scrollback bounded. The buffer-append "terminal" is deleted.
- **VCS**: async through the scheduler; typed status/diff/hunk models;
  git first via CLI wrapper (an honest adapter, not the architecture).
- **Search**: parallel, cancellable, streaming results, ignore rules.
- **Build/run**: task = program+args+env+cwd; streaming output; real
  process kill; exit codes.
- **DAP**: awaited initialize, pending-map correlation, graceful
  terminate, verified breakpoints.
- **Janet**: UI/config scripting on the foreground thread only. It does
  not do background work; Rust subsystems do and report via events.
  No silent builtin load failures — errors are surfaced. Dead C APIs and
  dead modules are deleted.

## 7. Rendering

- Invalidation: a monotonically increasing epoch bumped by every state
  mutation entry point. The GUI repaints when the epoch moves, blinks on
  a timer, and never locks-all-buffers to decide.
- Frame work is O(visible): surface reuse, minimap and sidebar caches
  keyed by revision, highlight spans from the incremental tree.

## 8. Performance contract (measured, not aspirational)

Benchmarks in `benches/` measure end-to-end pipelines (they exist for
typing/dispatch today; the rework adds frame-pipeline and index
benchmarks). Targets recorded in `docs/architecture/performance.md`
after measurement; CI fails on regression past threshold.

## 9. Verification

- `cargo test` must pass at every phase boundary; janet tests included.
- New subsystems get pure-Rust tests (`src/tests/<feature>_tests.rs`)
  plus Janet API tests where exposed.
- Integration tests drive the real production wiring (foreground loop +
  channel + services), not a parallel hand-built stack (task §27).
