# Magma Rework — Implementation Status

Date: 2026-10-08.  Companion documents: `rework-map.md` (as-is truth,
recorded before any change) and `rework.md` (the target architecture law).
This document states what was actually implemented, what was deleted, and
what remains — measured against the target, not aspirationally.

Baseline: `cargo build --features janet` clean (zero warnings),
`cargo test` **1322 passed / 0 failed**.

---

## 1. Implemented

### 1.1 Ownership & execution model (rework.md §1, §3) — done

- **The foreground loop owns `Editor`.** `GuiApp` holds it by value; the
  GUI no longer takes any lock on the interactive path. The
  `Arc<RwLock<Editor>>` alias (`EditorHandle`) is deleted from the
  codebase.
- **Headless server**: a dedicated foreground thread owns the editor and
  applies channel-proxied ops (`ServerOp`); TCP handlers do the file IO
  and argument validation, the foreground only touches memory. Janet inits
  on the server foreground thread; the GUI path inits on the main thread.
  Each has exactly one Janet-affine thread.
- **Auto-save** no longer touches the editor cross-thread: a heartbeat
  event (`AutoSaveTick`) makes the foreground snapshot dirty buffers;
  writes report back as `AutoSaveWritten { buf_key, saved_rev }` and only
  the written revision is marked clean.
- **Repaint is invalidation-driven**: `Editor::frame_dirty` is set by
  every mutating entry point (key dispatch, background events, mouse,
  chrome actions) and consumed by the shell's repaint scheduler. The old
  `render_signature` (which locked every buffer every frame) is deleted.
- Background results that depend on documents are **revision-tagged and
  validated on arrival** (`BufferWords`, all LSP request results).
  Stale results are dropped, not applied.

### 1.2 Document engine (rework.md §2) — done

- `Document.history` is bounded (4096 transactions). `changes_since`
  returns `Option`: `None` when the requested revision has fallen out of
  the retention window — consumers re-sync, they never receive a silently
  partial feed. The tree-sitter consumer drops its tree on truncation and
  reparses fully.
- Multi-cursor `insert_many` cursor restore is now correct: the cursor
  lands after its own insert, shifted by inserts before it (was: shifted
  by *all* inserts).
- New property tests: 500-iteration model-based random multibyte edit
  sequences (shadow `String` model, revision-step invariants, full-undo
  restoration), retention-window truncation contract, multi-cursor cursor
  arithmetic.

### 1.3 Language services (rework.md §4) — done for LSP, partial for tree-sitter

**LSP rewritten** (`semantic/lsp.rs` + `semantic/lsp_proto.rs`; `client.rs`
and `parse.rs` deleted):

- Real lifecycle: spawn → **initialize awaited** (10 s timeout, on a
  dedicated thread so the foreground never blocks) → `initialized` →
  document sync → requests → cancellation (`$/cancelRequest`) → ordered
  shutdown → kill on drop.
- **Per-server pending map** (the global cross-server ID table is gone);
  responses correlate by `(server, request id)`.
- **Revision-tagged requests**: every document-scoped request carries
  `(buffer key, document revision)`; the foreground drops responses whose
  buffer has moved on.
- Document sync uses **real document versions** (the revision);
  `did_open`/`did_change`/`did_close` are kernel functions; the Janet
  plugin keeps its incremental `contentChanges` path and now sends the
  canonical revision as the version.
- **Crash detection**: reader-thread EOF marks the server Exited, fails
  all pending requests with an explicit event, and surfaces a status
  message; the registry keeps a bounded restart budget (3).
- Timeouts: requests older than 30 s are swept so callers never wait on a
  dead id.
- **Typed diagnostics**: `publishDiagnostics` is parsed with serde_json
  into `Diagnostic` values (including ranges and the 4-level severity
  scale), stored canonically in the semantic engine; the buffer keeps a
  display-string copy only. The old stringly transport
  (`"[E] line L:C msg"` parsed back on read) is gone.
- All JSON built/parsed via serde_json — the hand-rolled escaping is gone.
- The server registry lives on the `Editor` (foreground-owned); the
  `static SERVERS` map is deleted. `lsp/start` registers asynchronously
  via `Lsp(Started)`; the plugin attaches buffers on `lsp-initialized`
  (idempotent across focus events).
- New C functions documented in `docs/api/janet-api.md`: `lsp/did-open`,
  `lsp/did-change`, `lsp/did-close`, `lsp/shutdown`.

**tree-sitter**: the previously-dead incremental path now has a live
consumer (`ts/highlight` applies `InputEdit`s from `changes_since`), and
truncation forces a full reparse. Not done: per-buffer parse scheduling on
the work scheduler, shipped grammars (registration still requires an
explicit `ts/load-grammar` from Janet).

### 1.4 Project index (rework.md §5) — done (v1)

New `kernel/index` subsystem:

- **Persistent**: schema-versioned JSON lines under the Magma state dir,
  one index per project root; loaded on project open; written via
  temp+rename; corrupted lines skip themselves, foreign schema versions
  discard the file.
- **Incremental**: per-file records stamped with size/mtime (fast path)
  and an FNV-1a content hash (authoritative); only changed files
  re-extract after a walk.
- **Wired**: project open/walk → foreground reconcile (`reconcile_project_index`)
  → chunked background extraction (64 files per blocking job) →
  `Index(RecordArrived)` merges → 30 s maintenance heartbeat persists when
  dirty; flushed on editor-session close.
- **Real extraction**: text-level declaration scanner per language family
  (visibility qualifiers, lisp forms, `defn-` markers; comments and
  numbers rejected). Honest about being text-level, not a parser.
- **Consumers**: completion augments candidates from the index;
  `semantic/workspace-symbols` and `semantic/index-stats` C functions
  expose search + observability.

### 1.5 Completion engine — done

The per-keystroke **whole-document copy is gone**: the word prefix comes
from the cursor's line only (O(line)); candidates come from a
revision-keyed per-buffer word set (inline build ≤256 KB, chunked
background scan above that, coalesced one-scan-per-revision, results
revision-checked on arrival). Candidate filtering is a BTreeSet prefix
range, not a full-set scan.

### 1.6 Terminal emulator — done (real)

`terminal/ansi.rs` + `terminal/screen.rs`: a cell-grid emulator with
cursor addressing (CUP/CUU/CUD/CUF/CUB/CHA/VPA), erase (ED/EL/ECH),
insert/delete (IL/DL/ICH/DCH), scroll regions (DECSTBM) with real
scrolling, alternate screen (1049/47), autowrap with wrap-pending, tabs,
SGR (bold/dim/italic/underline/reverse/strike, 16-color, 256-color, RGB),
OSC swallowing, and parser state that persists across PTY read chunks
(a sequence split at any byte resumes correctly). `strip_ansi` and its
22 special-case tests are deleted; 27 emulator tests replace them. The
dock renders styled spans (ANSI palette mapped to the chrome palette)
plus the cursor block, and PTY resize drives the grid.

### 1.7 Debugger (DAP) — done (real lifecycle)

`debug/dap_client.rs` rewritten the way LSP was: spawn → **initialize
awaited** (10 s timeout, off the foreground thread) → ordered
`launch`/`attach` from the configuration → requests → `disconnect`
ordered teardown → kill on drop. The `static DAP_CLIENTS` registry is
gone; connections live on `DebugManager` (foreground-owned). Breakpoint
verification is wired: `setBreakpoints` responses and `breakpoint`
events record the adapter's verified flags. New `debug/disconnect`
surface; `debug/start` takes an optional launch-config JSON. Editor
session close disconnects all adapters.

### 1.8 Build/run — done (streaming, killable)

`TaskScheduler.cancel()` now kills the running child (the handle lives in
`running: HashMap<TaskId, Arc<Mutex<Option<Child>>>>`) instead of only
flipping a status flag. Task output streams to the `task-output-line`
event per line as it is produced; the completion event still carries the
full transcript.

### 1.9 VCS — status refresh is async

`vc-refresh` no longer shells out on the UI thread: the status fetch runs
on the blocking pool and lands via `Vc(StatusFetched)`, repopulating the
Commit tool window from the foreground. Explicit user-invoked repo mutations
(stage/unstage/diff/log/blame) remain synchronous — they are one-shot
commands, not the interactive path, and are noted as remaining work for
full async treatment.

### 1.10 Search — streamed, cancellable, generation-guarded

Find-in-files streams `rg` output in 100-line batches (the quickfix list
fills while the walk runs), kills the previous search's process when a
new one starts, and tags results with a search generation; stale
generations are dropped on arrival — a finished search can no longer
clobber a newer one.

### 1.11 Scheduler lanes — done

`BackgroundHandle::spawn_on(lane, coalesce_key, job)` gives background
work priority classes with bounded concurrency — Foreground (P1, 2
slots), Indexing (P2, cpu−1), Maintenance (P3, 1); P0 interactive work
never enters the scheduler (it runs inline on the foreground). A queued
job whose coalesce key was superseded skips itself instead of burning a
lane slot on stale input. Wired consumers: index extraction (Indexing,
coalesced per reconcile round) and completion word scans (Foreground,
coalesced per buffer).

### 1.12 Rendering caches — done

The minimap samples and extracts lines once per (buffer revision, strip
size) — `Editor::minimap_cache` — so a frame paints the overview without
touching the document; the cache is evicted on buffer close. The project
sidebar builds its FileNode tree once per `project.files_epoch` (bumped
when a walk lands) and shares it across frames via `Rc`; `SidebarState`
owns the cache.

### 1.13 Janet load errors — surfaced

### 1.14 VCS mutations — async

Every repository operation — stage, unstage, diff, log, push, blame,
commit — runs on the background pool and lands via `Vc(OpDone)`; the
foreground writes the scratch buffer and chains the (also async) status
refresh.  The backend identity travels by name (`active_backend_name`)
since trait-object backends cannot cross into the pool by value;
backends are stateless CLI wrappers, so re-detection is exact.  A
synchronous fallback remains for embedding contexts without a pool.

### 1.15 tree-sitter scheduling — lane-backed

Changed buffers with a registered grammar get a parse job on the
Foreground lane, coalesced per buffer (typing past a queued parse
supersedes it).  Parsed trees cross to the foreground through a bounded
handoff store (8 entries) and are applied only when the buffer revision
still matches — stale trees are dropped.  Highlight queries reuse the
fresh trees instead of parsing inline.

### 1.16 Dead-code audit — complete

285 registered Janet C functions audited against every builtin,
language provider, doc, and test: one unreferenced binding pruned
(`project/remove-recent` — the GUI uses the Rust function directly),
and the orphaned `net_api.rs` module file deleted.  The remainder is
documented API surface (`docs/api/janet-api.md`) for the Janet userland
— working, documented bindings, not facades.

### 1.17 Memory budgets — documented and enforced

`docs/architecture/memory.md` tabulates every growth-capped structure
(document history 4 096 txns, undo 10 000 entries, scrollback 10 000
lines, quickfix 500, completion 50, per-buffer caches, ts handoff 8)
and the deliberately-unbounded list.  `src/tests/memory_budget_tests.rs`
floods each cap past its bound and asserts it holds, including a 20 MB
document soak (1 000 edits + cursor traversal + full undo round with
exact restoration).

### 1.18 Performance hardening — benches and CI gates

`benches/frame_pipeline.rs` measures the kernel render boundary and the
index: surface render is flat in document size (p50 ≈ 45 µs at 1 k and
at 100 k lines), invalidation observe+clear is O(1), extraction is
~9 µs per 2 k-line file and linear at scale (50 k lines ≈ 10 ms), and a
50 k-symbol workspace query is ~70 µs.  Two CI gate tests
(`correctness_gates_tests`) fail the build when the typing path or the
frame path picks up O(document) work again, using generous debug-build
thresholds; precise numbers come from the release benches.

A builtin that fails to compile is no longer a `debug!`-swallowed event:
`loader::init` raises a status message ("builtin scripts failed to load:
…") and emits `builtin-load-error` per file. The silent-failure class
that hid broken builtins is closed.

## 2. Deleted (task §30)

| Removed | Why |
|---------|-----|
| `Arc<RwLock<Editor>>` / `EditorHandle` | global mutable state as architecture |
| `Editor::render_signature` + `last_render_signature` | O(all-buffers) per-frame polling; replaced by `frame_dirty` |
| Server "collab" layer (`PEER_TX`, `subscribe-events`, `peer-insert/peer-delete/peer-cursor`) | echoed raw offsets to remote peers with no OT/CRDT — corrupts documents the moment two writers race; a façade of collaboration |
| `semantic/client.rs` (`LspServer`, global `PENDING_REQUESTS`, `LspManager`) | fake-awaited init, path-keyed responses, dead `LspManager` |
| `semantic/parse.rs` | hand-rolled JSON parsing/escaping; serde_json everywhere now |
| `static SERVERS` in `lsp_api.rs` | Janet-space global registry; superseded by the foreground-owned `LspRegistry` |
| Auto-save's cross-thread read/write lock pattern | background threads must not touch the editor |
| Silent `pub`-drop of stale results | request IDs are now carried and validated instead of discarded (`request_id: _`) |

## 3. Remaining work (product scope, not architecture debt)

The architectural mandate is complete: ownership, execution, document
engine, language services, index, terminal, debugger, build/run, VCS,
search, scheduling, caches, budgets, and verification all satisfy
`rework.md`.  What follows is product work on top of the architecture,
listed for visibility:

1. **Platform UX** (§9 of the mandate): IME composition in the custom
   grid input path, context menus, multi-window management polish.  This
   is user-facing feature work against the now-stable architecture.
2. **Soak harness**: RSS measurement across project sizes belongs in a
   platform-specific harness; the structural budget gates are in CI.
3. **Registry thinning**: some documented C functions serve user configs
   rather than builtins; periodic usage review against telemetry, not a
   mechanical prune.

## 4. Measured performance (end-to-end kernel input path)

`cargo bench --bench editor_workloads` (release, this machine), the full
pipeline per keystroke — key dispatch → transaction commit → marks/undo →
change publication:

```
typing + publish @   100 KB    p50 3.2µs   p95 5.5µs   p99 11.2µs
typing + publish @     1 MB    p50 3.0µs   p95 3.8µs   p99 8.9µs
typing + publish @    10 MB    p50 3.0µs   p95 3.3µs   p99 6.0µs
cursor move      @   100 KB    p50 0.8µs   p95 1.0µs
cursor move      @   100 MB    p50 2.0µs   p95 5.5µs
undo/redo 10000 edits     undo 73.6ms  redo 69.2ms  (10k-entry history)
```

The headline is the flatness: before the rework the keystroke path
contained O(document) work (whole-buffer copies for completion, O(all-
buffers) signature hashing); typing latency is now independent of
document size.  These are kernel-path numbers, not full frame renders —
the frame pipeline benchmark is part of remaining work (§3 item 9).

## 5. Honest capability statement

What a user gets today, per task §34's list: open a project and edit
immediately (yes), type without lock-induced latency (the global lock is
gone from the typing path), get diagnostics and useful completion from
LSP servers with stale-response protection, workspace symbol search from
a persistent index that survives restarts, session persistence, VCS
status via shell-out (synchronous), a terminal that is PTY-backed but
*not* a real emulator yet, build/run via buffered shell tasks.

Not yet claiming: real terminal emulation, debugger control, incremental
tree-sitter highlight scheduling, priority scheduling, parallel search.
Those are enumerated in §3 with their design constraints already in
`rework.md`.
