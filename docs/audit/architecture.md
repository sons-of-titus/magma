# Deliverable D — Architecture Review

## Current shape

    main.rs
      └─ Editor (owned by value on the UI thread — no Arc<Mutex<Editor>>)
          ├─ text_engine   rope Document → Buffer → BufferView (cursor anchors)
          ├─ event         synchronous queue-then-drain bus (+ re-entrancy guard)
          ├─ command       137 Rust builtins → Janet *janet-commands* fallthrough
          ├─ keymap        layered: buffer-local > LIFO layers > global
          ├─ scripting     Janet VM: 297 C fns, thread-local EDITOR_PTR scope
          ├─ semantic      LSP client, tree-sitter, symbol index
          ├─ task/run_config/debug/terminal/vc/project/index/session
          └─ background    tokio pool ──typed BackgroundEvent channel──▶ UI drain

Data flow per keystroke: egui key → `dispatch_key` (terminal /
completion / Janet interceptor / keymap) → command → Buffer txn →
pending_changes → `publish_buffer_changes` → event drain (LSP didChange,
gutter, watchers) → `frame_dirty` → next frame repaints.

## What is right (keep)

1. **Single-owner Editor + typed event channel.** No data races by
   construction; every cross-thread result lands through one funnel with
   staleness gates (revision for LSP, generation for search, supersede
   lanes for jobs). This is the strongest structural decision in the
   codebase.
2. **Transactional text engine.** One mutation gateway, undo groups with
   vim save semantics, anchors transform marks/breakpoints/cursors.
3. **Rust state / Janet behavior split.** Genuinely enforced — language
   knowledge lives in `builtins/*.janet`.

## What is wrong (and what to do)

### 1. The `Editor` god object (77 fields) — F-24

Every subsystem mutates `Editor` directly; nothing is encapsulated.
**Migration:** move cohesive groups behind sub-struct methods first
(io+terminals, vc, quickfix+search, ts+semantic) without changing call
sites' semantics; the goal is fewer public fields, not a rewrite.
Effort: incremental, one subsystem per PR. Risk: low.

### 2. `BackgroundEvent` mega-enum + 900-line `runtime.rs` — F-24

Every subsystem's result type couples to one enum and one match.
**Migration:** convert `process_background_event` into a registry of
per-subsystem handlers (`fn(&mut Editor, &BackgroundEventKind)`), split
`runtime.rs` by domain (the project's own 400-line gate already demands
it). New variants stop touching a shared file.

### 3. Two text engines — F-25

`Document` (canonical, revisioned, test-covered) is not the live path.
**Migration:** port `Buffer`'s facade onto `Document` behind the same
public methods; the transaction tests already pin the semantics.

### 4. Stringly diagnostics — F-26

`Buffer.diagnostics: Vec<String>` is re-parsed at three render sites.
**Migration:** store `Vec<Diagnostic {severity, line, col, message}>`;
keep the display-string formatting in exactly one place; hover/gutter/
quickfix consume the struct. Do this before building any diagnostics UI
or the UI will grow a fourth parser.

### 5. Extension trust model — F-28

Capabilities are advisory and self-escalatable. Either enforce
(`EditorAccess` checks at every capability-gated cfun via a scope guard)
or delete the capability layer and document "plugins are fully trusted,
like Emacs". Half-enforced is the worst state.

## Performance bottlenecks (evidence)

- Frame cost is O(visible) and benchmarked flat — the pipeline is not
  the problem.
- Allocation churn is: per visible line per frame a `String` slice, two
  HashMaps (`fold_map`, `diag_lines`) rebuilt per frame
  (`editor_view.rs:90,107`). A per-revision row cache would cut the
  steady-state cost, but measure first (`benches/frame_pipeline`).
- O(document) per keypress in motions/search (F-12) — the largest
  algorithmic gap vs JetBrains at big file sizes.
- `apply_highlights` iterates every highlight range in the document each
  frame before culling to the viewport (fine at hundreds, wasteful at
  tree-sitter densities on huge files).

## Reliability weaknesses

219 `lock().unwrap()` (one panic poisons the UI thread), no
`shutdown_timeout` on runtime drop (exit hang, F-31), unbounded event
channel (memory growth under terminal flood), cooperative-only
cancellation. All documented in findings; none block daily use, all
block "professional" scoring.
