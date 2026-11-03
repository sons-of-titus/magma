# Brutal IDE Review — November 2026

A hostile, engineering-grade architecture review of Magma against the target:
*a native IDE with JetBrains-level UX and productivity, better startup,
responsiveness, memory, and simplicity*. Every claim was traced to source;
nothing was graded from documentation, intentions, or the existing audit
program. Evidence uses `file:line` from the working tree on 2026-11-02.

Method: six parallel deep-dive audits (text engine, language intelligence,
indexing/project model, concurrency/startup, UI/rendering, plugins/VCS/debug),
each reading the actual code paths, plus verification of every headline claim
in this document against the tree. The test suite was run: the full workspace
suite passed before the remediation tranche below (2,055+ tests), and every
fix listed here ships with regression tests.

---

## What You Actually Have

A **programmable code editor with unusual kernel discipline**, not an IDE.

The categories from the review charter:

- **Editor**: yes, and better than most hobby editors — a rope-backed
  document core with normalized parallel-edit transactions, a real anchor
  system, inverse-transaction undo, property tests against a String oracle,
  and PASS/FAIL perf gates on the user path
  (`src/tests/document_property_tests.rs`, `benches/perf_gates.rs`).
- **Code editor**: partially — tree-sitter highlighting across ~52
  dynamically-loaded grammars (`crates/semantic/src/ts_registry.rs:223-270`),
  a genuinely well-built LSP client with revision stamps, supersede,
  `$/cancelRequest`, and a restart budget (`crates/semantic/src/lsp/`),
  completion from four sources, diagnostics from LSP and build tools.
- **IDE**: no. There is no reference resolution anywhere, no find-usages,
  no project model beyond "open folder", no index that survives a large
  repository (see the 512-file bug below), no outline, no navigation
  history, no splits in the shell, and a renderer that is a terminal
  emulator's cell grid.

## What You Think You Have

The existing self-audit (`docs/audit/capability-matrix.md`) is more honest
than most, but three self-images contradict the code:

1. **"Project model/index: Working, 4/5."** The walk pipeline was silently
   broken above 512 files: `Batcher::flush` cleared its buffer after each
   batch (`crates/kernel/src/project/walk.rs:84-94`) while the foreground
   handler *replaced* state per batch
   (`crates/kernel/src/runtime/handlers/index.rs:36-51`), so any project
   over one batch truncated `project.files` to the final partial batch,
   wiped the tree and quick-open down to ≤511 files, and — via
   `retain_paths` — **permanently destroyed the persisted index** on the
   next flush. Verified empirically: a 1,200-file walk yielded 176 files.
   No test exercised a batch boundary. Fixed in this review (below).
2. **"Syntax highlighting: tree-sitter incremental."** Incremental parsing
   machinery exists and is tested (`ts_registry.rs:161-212`), but the
   shipped dataflow was *coalesced full reparse*: every parse batch copied
   the entire buffer into a String under the buffer mutex
   (`ts_schedule.rs:46`) and reparsed with no old tree
   (`ts_registry.rs:139-146`); the highlight query then ran over the whole
   tree with the whole text and marshalled whole-document capture lists
   through Janet (`treesitter_api.rs:165`, `ts_registry.rs:102-135`,
   `builtins/treesitter.janet:121-150`). Partially fixed below.
3. **"Glyph/atlas (GPU path)."** `crates/gpu` contains no GPU code at all —
   zero wgpu/skia dependencies, a CPU-side fixed-grid glyph atlas, and a
   dead vertex path only tests call (`gpu_atlas.rs:1-4`, `:307-429`). All
   text is CPU-rasterized into one flat RGBA image per pane per frame.

## What You Are Underestimating

1. **The renderer is a load-bearing wall, and it is the wrong wall.**
   `Surface { width, height, cells: Vec<Cell> }` — one `char` per cell
   (`crates/surface/src/lib.rs:52-56`) — makes variable-width text,
   shaping, ligatures, subpixel AA, and CJK *architecturally impossible*.
   The glyph atlas is a hardcoded charset (ASCII/Latin-1/Greek/Cyrillic);
   committed CJK renders as tofu (`gpu_atlas.rs:77-79`, `blit.rs:89-92`),
   IME preedit is unimplemented (grep: zero hits), so a Chinese, Japanese,
   or Korean user cannot see their own file. This is not a gap; it is a
   replacement.
2. **The semantic layer is scaffolding with tests.** `SymbolIndex` is a
   name-keyed HashMap fed only on buffer-create and buffer-save
   (`builtins/symbols.janet:8-24`); `ProjectGraph.add_dependency` has zero
   production callers; `references` is never written
   (`symbol_index.rs:167-168`). The workspace-wide "index" is keyword-line
   extraction that describes itself as "Deliberately not a parser"
   (`crates/index/src/extract.rs:4-6`). Every navigation either
   string-matches a name or outsources to LSP. Find-usages does not exist
   (no `textDocument/references` anywhere); rename is LSP-only, defaults
   to the literal string `"new_name"` (`builtins/plugins/lsp.janet:154-158`),
   and has no preview.
3. **Latency is protected by convention, not enforcement.** The 4ms budget
   bounds only the background-event drain in the 20ms timer
   (`slint-shell/src/main.rs:340-347`); the event-bus/Janet drain inside
   `refresh` has no deadline (`sync/mod.rs:76`). Janet plugin callbacks run
   synchronously on the UI thread with no watchdog or preemption — one
   infinite loop freezes the IDE. `catch_unwind` appears nowhere in the
   repository. Cancellation exists in three places; `spawn_on` closures
   carry no token at all (`bgpool/src/lib.rs:94-130`).
4. **UI-thread I/O is normal, not exceptional**: `editor/shell` runs a
   blocking `sh -c` on the UI thread (`editor_io_api.rs:82-85`);
   `vc-open`/`vc-blame` spawn `git rev-parse` synchronously
   (`command/builtin/vcs.rs:146-165`); the mergetool runs to completion on
   the UI thread; the index flush serializes every record and `fs::write`s
   it inside the 20ms tick (`handlers/fs.rs:159-166` →
   `index/src/store.rs:68-83`); `open-file` reads disk synchronously by
   explicit design (`buffer.rs:130-133`).
5. **Diagnostics don't survive edits.** Stored as raw line numbers, never
   transformed through the transaction system the kernel already has
   (`buffer.rs:54-56`); gutter markers point at the wrong lines until the
   server republishes.

## What Will Kill This Project

1. **The scale cliff.** The pipeline had never run against a real
   repository — the 512-file truncation proves it. Even fixed, a 100k-file
   open does a full stat scan plus re-extraction, O(N²/512) foreground
   batch application under the old contract, and a 30s-cadence full JSONL
   rewrite on the UI thread. Symbol search is a linear scan over every
   record (`index/query.rs:10-53`).
2. **The rendering wall** (above). A JetBrains user within ten minutes
   hits: no splits, no back/forward, no outline, no recent-files, CJK
   tofu, invisible IME composition, non-interactive minimap, no terminal
   keyboard focus, a find-in-files that freezes the UI
   (`find_bar.rs:162-218`), no keybinding UI, and menus that never display
   shortcuts.
3. **Extension code that can take the editor down** — no VM time budget,
   no panic boundary, `plugin/unload` that doesn't unload code
   (`plugin_api.rs:181-189`). No third party can ship into that.
4. **The capability system is decorative**: `EditorAccess` is never
   checked anywhere; `task/define`, `task/run`, and `scheduler/submit`
   executed shell commands with no capability check (fixed below).

## What Is Surprisingly Strong

Preserve these — they are the reason this is "continue, but restructure"
rather than "start over":

- **The transaction/anchor/undo core** is a legitimate 8/10: normalized
  pre-transaction edits, one shared anchor transform used by marks,
  breakpoints, decorations, forks, and cursors, inverse-transaction undo,
  and property tests over 100-seed multibyte edit sequences
  (`document.rs:107-173`, `anchor.rs`, `document_property_tests.rs:229-407`).
- **Epoch discipline**: revision stamps on every LSP request, generation
  guards on walk/extract/session/save/blame, revision-checked word scans
  *mid-loop*. This is better than most editors at this stage.
- **The LSP client's protocol spine**: framing, correlation, supersede +
  cancel, crash drain, bounded restart budget, ABI-validated dynamic
  grammars across 52 languages.
- **The DAP client and debugger panel are real work** with tests against a
  scripted adapter (`dap_client.rs`, 40 debug system tests).
- **Durability**: atomic saves (temp+fsync+rename+dir-fsync), a versioned,
  crash-safe index format, watcher auto-revert with dirty protection, and
  a tested crash-recovery subsystem.
- **The test culture**: 2,055+ behavioral tests, real PTYs and real git
  round-trips, perf gates with honest commentary
  (`docs/audit/benchmark.md` refuses to overclaim).
- **The kernel/shell separation is real**: the shell never touches buffer
  internals; everything routes through the command registry; the same
  Surface serves GUI, TUI, and Janet.

## Performance Claims: Measured vs. Asserted

**Measured** (benches run on darwin/arm64): startup 0.70ms avg (≤50ms
gate); typing p99 695µs @10KB, 205µs @100KB and @1MB; render_frame flat in
document size (p50 60µs @1k lines, 43µs @100k); retained memory 2.4MB after
200 edits on a 1MB doc; index extract+query in the tens of µs. All gates
pass — **within the kernel boundary**.

**Unmeasured**: the actual per-keystroke shell cost (full-grid signature
hash + full-pane CPU blit ≈ 2.4M pixels, no dirty regions), time-to-usable
under a large project, and under-load latency. The benches stop at the
kernel boundary, and `docs/audit/benchmark.md` says so honestly.

**Fantasy**: "GPU rendering" (there is none), "incremental parsing" (was
bypassed by the production dataflow; now partially real — see below),
"native = fast" (the 50Hz polling timer alone floors interaction latency
at ≥1 tick + full-screen blit).

## JetBrains Parity Matrix

| Capability | Status | Gap | Difficulty |
|---|---|---|---|
| Editing core | Implemented | Multi-cursor prototype; no column/rect selection | Medium |
| Rendering | Stub | Cell grid; no shaping/CJK/IME/dirty-rects; CPU-only | Very high (replace) |
| Completion | Partial | 4 async sources, revision-gated; **no fuzzy matcher** | Low-Medium |
| Diagnostics | Partial | LSP+build, version-gated ingest; stale between edits | Medium |
| Navigation | Partial | goto-def two-tier; **find-usages missing**; ctags fallback | High |
| Refactoring | Stub | LSP-only rename, `"new_name"` default, no preview | High |
| Indexing | Partial | Persistent versioned store; broken >512 files (fixed); no query structures | Medium-High |
| Project model | Stub | Marker-file detection; nothing parsed; no modules/roots/SDKs | High |
| Debugging | Implemented (partial) | Real DAP client+panel; no conditions (fixed), first-scope vars | Medium |
| Build integration | Partial | Process exec + regex diag parsing; no build-system models | High |
| Git | Partial | 4 backends, hunk staging, blame, stash; save-only gutter, no merge UI | Medium |
| Plugins | Partial | 324 documented cfuns; no isolation, no unload, no tool-window API | High |
| Search | Partial | ripgrep via task; UI search sync-on-thread; dead Rust event | Medium |
| UI customization | Partial | 8 themes, settings GUI; no keymap UI, no shortcut display | Low-Medium |

## Architecture Kill List

1. **CRITICAL — The fixed monospace cell grid** (`surface/src/lib.rs:52`).
   Failure: CJK/IME unusable, no shaping ever. Fix: replace Surface + blit
   with a glyph-positioned layer (Skia/raqote/wgpu) with a runtime glyph
   cache. Rewrite of the render layer: yes.
2. **CRITICAL — No reference resolution** (semantic engine dead code,
   `symbol_index.rs`, `project_graph.rs`). Failure: find-usages/rename/
   inspections are impossible without LSP. Fix: persistent per-file symbol
   trees + a resolve pass, fed by the watcher. Rewrite: no, but it is
   net-new engineering.
3. **CRITICAL — Index pipeline breaks above one batch** (fixed today;
   `walk.rs` / `handlers/index.rs`). Lesson: no e2e test at batch
   boundaries existed. Fix shipped with a 1,200-file regression test.
4. **HIGH — Whole-document foreground work per parse batch**
   (`treesitter.janet:121-150`, `treesitter_api.rs:165`): full-tree query
   and whole-document span lists through Janet. Fix: query only edited
   ranges, keep spans anchored to the transaction stream.
5. **HIGH — No plugin isolation** (no `catch_unwind`, no VM budget,
   `runtime.rs`). Failure: one bad plugin = frozen or aborted IDE. Fix:
   fiber step-budgets + panic boundaries + supervised restarts.
6. **HIGH — UI-thread I/O** (`editor_io_api.rs:82`, `vcs.rs:146-165`,
   `handlers/fs.rs:163`, `buffer.rs:130-133`). Failure: multi-second
   stalls from git probes, mergetool, index flush, network mounts.
7. **HIGH — Enforcement gap** (`sync/mod.rs:76`): event-bus/Janet drain
   has no deadline; the event bus's re-entrancy guard missed A→B→A cycles
   (fixed today with a cascade cap).
8. **MEDIUM — Diagnostics as bare line numbers** (`buffer.rs:54-56`), never
   transformed; LSP edits applied as N sequential transactions
   (`edit_apply.rs:86-100`). Fix: route through the anchor system that
   already exists.
9. **MEDIUM — No query structures in the index** (`query.rs:10-53` linear
   scans), no single-file watcher invalidation (full re-walk per storm).
10. **MEDIUM — Synchronous project search on the UI thread**
    (`find_bar.rs:162-218`) while a `GrepResultsBatch` background event
    has a handler and no producer (dead code).

## What Was Fixed In This Review

Eleven defects, each with regression tests, all gates honored (new C
functions documented in `docs/api/janet-api.md`; 1,162 tests passing across
the touched crates, zero new warnings):

1. **512-file walk truncation** — batches are now deltas accumulated
   against a walk id; stale re-walk batches are dropped; the persisted
   index is no longer destroyed. Tests: >512-file handler accumulation,
   1,200-file end-to-end walk, stale-walk interleave.
2. **Tree-sitter parse pipeline** — parses read an O(1) rope snapshot
   (no full-document String under the buffer mutex), and single-edit
   batches take true incremental reparse with honest tree-sitter points;
   multi-edit batches reparse from the snapshot. (`ts_schedule.rs`,
   `ts_registry.rs::parse_rope`)
3. **Fallback syntaxer** — line bounds now come from the line index, not
   an O(edit-offset) prefix copy, per keystroke.
4. **`insert_many`** snaps to char boundaries instead of silently dropping
   the whole transaction.
5. **Anchor discipline restored** — extra cursors and the selection anchor
   now transform through the transaction stream on every named command
   (undo included); multi-cursor typing no longer relies on
   `(i+1)*klen` arithmetic that broke when the primary cursor wasn't the
   lowest offset.
6. **LSP timeout sweep** — expired requests now send `$/cancelRequest`
   and emit `RequestTimedOut` (surfaced as a status message) instead of
   vanishing silently; sweep also runs on every request path.
7. **`didChange` JSON escaping** — a single-pass peg escapes every control
   character (`\u00XX`) so `\b`/`\f`/etc. in a file can no longer corrupt
   the server stream.
8. **Sync-kind negotiation** — the initialize result's
   `textDocumentSync.change` is captured; range changes are sent only to
   servers that declared incremental sync, full text otherwise (valid
   against kinds 1 and 2 alike). New `(lsp/sync-kind)` documented.
9. **DAP breakpoint conditions** — `condition` is serialized into
   `setBreakpoints` (with JSON escaping) and `(debug/set-breakpoint-condition)`
   sets/clears it; previously the field existed and was never sent.
10. **Event-bus cascade cap** — handler-emitted events carry a depth;
    beyond 8 hops they are dropped, so A→B→A handler cycles terminate
    instead of livelocking the drain (white-box + end-to-end Janet tests).
11. **Capability enforcement** — `task/define`, `task/run`, and
    `scheduler/submit` now require ProcessExecution, closing the
    arbitrary-shell-execution hole the capability docs claimed was closed.

## The Three Things I Would Fix First

1. **Replace the rendering surface.** A glyph-positioned text layer with a
   runtime glyph cache, dirty-region invalidation, and event-driven
   frames. Everything else in the UI (CJK, IME, ligatures, minimap,
   smooth scroll) is downstream of this one decision.
2. **Build the semantic layer for real.** Persistent per-file symbol
   trees, reference resolution, watcher-driven single-file invalidation,
   and query structures in the index. This is JetBrains' actual moat, and
   nothing in the current tree approximates it.
3. **Make the latency contract enforced, not aspirational.** A deadline on
   the event-bus/Janet drain, a watchdog on handler execution, real
   cancellation tokens plumbed into `spawn_on`, and `catch_unwind` at the
   Janet boundary. Move the remaining UI-thread I/O off the thread.

## What I Would Stop Building Immediately

- More bundled language-plugin manifests until the LSP integration layer
  is deep (references, documentSymbol, formatting, completionItem/resolve).
- The "ecosystem" naming (there is no marketplace; `ecosystem.janet` is a
  require + mark ring).
- More chrome polish (themes, panels) until the interaction model has
  splits, navigation history, and an outline.
- Dead scaffolding: `MemoryRegion`, the unpopulated
  `Module`/`Dependency`/`Workspace` types, the unenforced `EditorAccess`
  (enforce it or delete it), `plugin/unload` that doesn't unload.

## What Must Exist Before Calling This an IDE

1. goto-definition, find-usages, and rename working **cross-file without
   LSP** for at least one language, with a rename preview.
2. A project index that demonstrably survives a 100k-file repository and a
   20k-file branch switch — with end-to-end tests at scale (the 512-file
   bug is the cautionary tale).
3. Interactive typing on a 20MB file with syntax on, measured p99.
4. Splits, navigation history, recent files, and an outline view in the
   shell; terminal keyboard focus; asynchronous find-in-files.
5. CJK input and rendering working end-to-end (preedit + glyphs).
6. A third-party plugin that provides diagnostics and a completion source
   without modifying the core, surviving a `panic!` and a 10-second loop
   without taking the editor down.

## Final Verdict

**If the team continues on the current trajectory, is a genuinely
JetBrains-class native IDE realistically achievable without a major
architectural reset?**

The kernel: yes. The document engine, threading model, protocol clients,
and persistence layer are keepable and worth keeping — this is the rare
hobby-scale project whose foundations would survive review.

The product: no. Two walls require deliberate architectural resets — the
cell-grid renderer (a replacement, not an evolution) and the absent
semantic/index layer (net-new engineering of the hardest kind). Add the
isolation and enforcement work, and the honest ledger reads:

| Dimension | Score |
|---|---|
| Editor maturity | 6 / 10 |
| IDE maturity | 2.5 / 10 |
| Language intelligence | 3 / 10 |
| Architecture | 5.5 / 10 |
| Performance potential | 6.5 / 10 |
| UX maturity | 2.5 / 10 |
| Extensibility | 5 / 10 |
| JetBrains parity | 2 / 10 |

**Estimated distance to a credible JetBrains alternative: ~85% of the hard
engineering remains.** The visible polish (a working shell, themes, a
debugger panel, 52 grammars) is the least of it. What remains is the part
JetBrains spent a decade on: resolution, indexes, incremental propagation,
and an interaction model people live inside. The foundation is good enough
to build that on. Build it.
