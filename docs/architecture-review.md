# Magma — Brutal Architectural Review

Method: full source read of the text engine, state, scripting bridge, render path, command/event stack, storage, plus empirical verification (serial `cargo test --features janet`: 1319/1319 pass in 16.6s; full parallel runs stall; a focused test proving the undo-mark restore bug). Trust code over docs.

---

## 1. Executive Verdict

The architecture's *intent* — Rust owns state/rendering, Janet owns behavior — is sound, and there is a real, tested feature surface. But the two load-bearing subsystems are built wrong. **The cursor/position model and the render path are both O(n) per keystroke, per frame, per pane, per sub-view.** The undo system is a linear in-memory stack whose mark restoration is provably incorrect (verified by test: a mark restored to position 3 where the correct answer is 5). Data loss is reachable through correct-looking workflows (autosave race, merge-clobbered undo, non-atomic writes). This is a prototype that has taken on the scope of a production editor without ever paying off its core-technical-debt. It will not survive unchanged at millions of lines.

---

## 2. The 10 Worst Problems

1. **Cursor moves cost O(cursor-position).** `BufferView::set_cursor` does `buf.rope.slice(0, offset).to_string()` then walks it char-by-char to find the line/column (`text_engine/view.rs:35-41`, `text_engine/position.rs:14-46`). Applies to every typed character and every keypress. A 1 MB file means a ~1 MB copy *and* scan *per keystroke*. This is hot-path suicide and it is not micro-optimization — it is an algorithmic defect.
2. **The render path copies the entire file, multiple times, every frame.** `EditorView::render` builds `buf.slice(0, buf.len())` (`render/editor_view.rs:61`); `compute_scroll_state` does another (`render/frame.rs:83-88`). The GUI repaints continuously with no damage tracking (`render/gui.rs:317` and confirmed). With one editor pane + minimap + status bar + breadcrumb + popup you get 5+ full-text copies per frame. On typing, a 10 MB file is copied ~60×/sec.
3. **Whole-file reparse/rehighlight on every change.** Both syntax pipelines re-scan the full buffer: the Janet keyword/regex passes (`builtins/syntax_core.janet:124-153`, five passes, per-char `string/slice`) and tree-sitter (`builtins/treesitter.janet:293-317`, which copies the entire buffer, re-parses from scratch, runs the query, and has a "debounce" comment with no debounce code). There is no incremental or dirty-range highlighting anywhere.
4. **Undo mark restoration is wrong, and undo is a linear Vec, not a tree.** `UndoGroup.mark_states` is overwritten on each 200 ms-window merge (`text_engine/undo.rs:92-119`); `undo_mark_states()` returns `history_group[cursor].mark_states`, i.e. the state *before* the group *before* the one being undone; `Buffer::undo` then restores positions from it (`text_engine/mod.rs:184-206`). Empirically: after undoing an insert, a mark at true 5 goes to 3. Redo, branching, and merging undo histories are silently lost rather than represented. Cursor/selection are never part of undo.
5. **The linear-scan undo drain-window caps grouped ops at 100 global inserts** (`undo.rs:106-119`) — large edits simply get split into broken undo chunks — while an *open insert session* is unbounded (`undo.rs:97-104`), so a single insert mode session can hold the last 100,000 ops in one group.
6. **Data loss is reachable.** Autosave snapshots the text, writes off the main thread, then `mark_saved()` — which can clear the modified flag while newer edits exist (`main.rs:200-235`). Saves are non-atomic (`std::fs::write`, no temp+rename, no fsync; `storage/disk.rs:22-31`), synchronous on the editor thread (`command/builtin/buffer.rs:35`), and `open-file` blocks the UI thread on a synchronous read (`buffer.rs:56`).
7. **`EDITOR_PTR` creates `&mut Editor` from a raw pointer on every Janet C call** (`scripting/mod.rs:21-41`), while the caller already holds an active `&mut` through `RwLockWriteGuard`. Re-entrant aliasing of `&mut` is UB-in-theory; the comment "no aliasing UB arises in practice" is the soundness review of this FFI bridge. `JANET_VM_LOCK` is dead code in production.
8. **Two windows on the same buffer share one cursor.** `window-split` assigns the *same* `buffer_id` to the new pane (`command/builtin/window.rs:54-71`) and views are keyed by buffer (`state/mod.rs` `views: HashMap<usize, BufferView>`). Split-editing one file = both panes fighting over one cursor and one scroll position.
9. **The parallel test suite deadlocks.** Serial: 1319 pass / 0 fail, clean. Default parallel threads: three runs stalled indefinitely (each a lock held forever is the consistent signature, and a stuck Janet test holding `JANET_VM_LOCK` cascades). A suite that hangs instead of failing hides regressions by preventing them from being diagnosed.
10. **The repository violates its own gates.** `runtime.rs` is 615 lines (own rule: 400), build emits warnings (own rule: "no new warnings" — `gui_dialog.rs:12` unused `Color32`, `dired.rs:7` unused `Path`), and horizontal scrolling is absent: long lines truncate and the cursor is simply not drawn when off-screen (`editor_view.rs:154-197`).

---

## 3. What Is Actually Good

- **The Rust/Janet split itself.** The boundary is real: Janet gets a typed C-function API, the VM only ever touches the editor through `Editor`, and user-facing policy (keybindings, modes, syntax) genuinely lives in Janet. This is the correct shape for the project and it survived contact with a huge feature list.
- **The low-level rope layer.** Ropey-based insert/delete/slice (including multibyte) is correct and covered by 33 focused tests. The bottleneck is above the rope, not the rope.
- **The test suite has real teeth.** 1319 passing tests; `text_engine` tests cover multibyte boundaries; Janet tests assert observable Rust state, not just `"ok"`. The `janet_test!` locking discipline documented in AGENTS.md is real and mostly respected.
- **Feature breadth is astonishing** — tree-sitter, LSP-ish semantics, VC backends, net API, Dired, project model, task scheduler, multi-window GUI, mouse — all wired through one coherent event/command bus.
- **Reasonable, boring infrastructure choices in places:** layered keymap (buffer-local → mode layers → global), typed events with payload structs, `Arc<RwLock>` buffer storage, clearly documented API docs (`docs/api/janet-api.md`).

---

## 4. Architecture

The stated contract — Rust owns state, Janet owns behavior — is honestly followed. The problem is that Rust's side is under-engineered while Janet's side is over-scoped: `builtins/*.janet` re-implements syntax scanning line by line instead of being given a line index, and the "semantic engine" (an LSP client in Rust with a static, unversioned request queue — `semantic/client.rs:23-36`) appears designed but not load-bearing. There is no rendering pipeline to speak of — no damage tracking, no layer caching, no view shorthand, no incremental line index. The event bus is synchronous broadcast with subscribe/unsubscribe; dispatch drains per keypress. This is a *plausible* architecture executed at prototype grade: correct on small buffers serially, collapsing under load, with several correctness holes at real scale.

---

## 5. Core Data Model

- Buffer text: rope (good), but every API conversion is byte→char→byte via linear scans.
- Position: `{line, column, offset}` in one struct with no invariant — computed on demand by scanning (`position.rs:14-46`). No line index, no grapheme awareness (selection/`delete` can land inside a combining sequence), no tab-width or wide-cell arithmetic (column = char index; CJK/emoji cells misalign rendering at `editor_view.rs:155`).
- Undo: linear `Vec<UndoGroup>`, in-memory, non-persistent, no branch/redo merging, cursor+selection never recorded.
- Marks: positions in buffer space, adjusted per op — but restored from stale snapshots (bug #4).
- Views: one `BufferView` per buffer id, so *split windows on the same file share the cursor*; scrollbars optional per pane.
- State: `Editor` god-struct (354 lines, 60+ fields) with a dead field (`ts_trees`) and a global singleton `Option<Selection>`.

Verdict: the data model needs a line-index + position-by-index scheme and a persistent undo tree; the current one is the origin of most hot-path and correctness defects.

---

## 6. Performance

No profiling needed — the complexity is in the code:

- Typing → `set_cursor` + `recompute_cursor_at` → O(n) copy+scan; then per-frame O(n) copies ×5 sub-views; scroll recompute O(n); selection-range render O(n); tree-sitter show O(n) reparse (only on Janet-driven edits — pure Rust-path typing leaves highlights stale, a correctness bug and half of the reason the whole-file refresh is even being done).
- Benchmarks (`benches/rope_ops.rs`, `benches/buffer_ops.rs`) benchmark the *rope*, which is fine, and never measure `set_cursor`, `render_frame`, undo, or dispatch — so the project's performance assumptions were never challenged by its own CI.
- The mapper's reply: every subsystem independently re-reads the entire buffer because there is no shared incremental view. At 1 MB files the editor falls below typable latency; at 10 MB it stalls. Long-line editing leaves the cursor undrawable.

---

## 7. Memory

- Allocates a fresh `String` for the buffer across most hot paths (cursor recompute, selection range, minimap, status). Peak per-frame working set is several full-file strings.
- Undo retains full text for every op chunk in a group (rope snapshot), unbounded in an insert session.
- The GUI runs at frame-rate repaint with no frame-to-frame reuse.
- No large-file strategy: everything assumes the file fits in memory as multiple full copies.

---

## 8. Concurrency

- The VM runs only on the main thread and the `JANET_VM_LOCK` is test-only — that much is internally consistent.
- The real problems: (a) the `EDITOR_PTR` `&mut` aliasing hazard; (b) the autosave `mark_saved()` race; (c) the semantic-engine global request queue with no versioning → stale LSP diagnostics applied while typing; (d) a full-suite deadlock under parallel test execution; (e) synchronous file I/O on the editor thread. There is no task graph, no channel for VM work, and no ownership discipline for who may touch which `Editor` internals.

---

## 9. Abstraction Audit

**KEEP**
- `Editor` as a single root behind `Arc<RwLock>`; buffers as `Arc<Mutex<Buffer>>`.
- The Janet C-function bridge + `docs/api/janet-api.md` contract.
- Layered keymap; typed event/pub-sub; command registry with `ArgSpec`.
- Ropeny rope wrapper and its tests.
- The Janet-vs-Rust ownership split and `builtins/` layout.

**SIMPLIFY**
- `ts_trees: HashMap<usize, Vec<u8>>` (dead field) → delete or make a real cache.
- The linear scan undo → replace with a persistent tree (rope + mark map), not "simplify the tree back to a Vec."
- The "semantic engine"/LSP-in-Rust → replace with a thin transport + Janet-side policy, dropping the static request queue.
- The syntax-core 5-pass full-buffer Regex scanning → fold into one dirty-range pass exposed by a Rust line index.

**DELETE**
- `JANET_VM_LOCK` production deadlock-wait code path (keep for tests only, as is actually true).
- Edit-marker merging by 200 ms window in undo — silent history destruction.
- The stale "debounce" comments in treesitter/highlighting where no debounce exists.

**MISSING**
- A line index (O(1) offset↔line/col, grapheme + tab/wide-cell aware) owned by the Buffer.
- Damage tracking + dirty ranges for render and highlighting.
- Atomic writes (temp+rename+fsync), async save queue, snapshot-versioned `mark_saved`.
- Undo of cursor/selection; per-window view state keyed by (window, buffer).
- Key/pointer-press latency contracts (e.g. nothing O(n) on the input path).

---

## 10. Correctness Hazards

1. Undo mark restoration (proven wrong by test).
2. Autosave `mark_saved()` race (data loss) + non-atomic writes + UI-blocking IO on the editor thread.
3. Merge-cap splitting large edits → broken undo sequences.
4. `EDITOR_PTR` `&mut` aliasing (UB latent, optimization-sensitive).
5. Stale LSP diagnostics (unversioned global request queue).
6. Highlighting desync: pure Rust typing path never emits `buffer-changed`, so highlights lag behind until a Janet edit or full refresh — and every Janet edit pays a full-file reparse.
7. Selection/mark stale after undo (not recorded, not revalidated against clamp).
8. Two panes sharing one cursor for split same-file editing.
9. No horizontal scroll → content silently unviewable past pane width; cursor undrawn.
10. Column-as-char-index breaks wide glyph and tab alignment.

---

## 11. Missing Fundamentals

- Horizontal scrolling / cursor visibility guarantee on long lines.
- Incremental & validated highlighting (per-change, not whole-file).
- A line/column + grapheme model with cell-width awareness.
- Persistent, tree-based undo history with cursor/selection restoration.
- Atomic, async saves with safe modified-flag semantics.
- Neovim-style change tracking / jump list keyed to edits that survive undo.
- Buffer-memory-mapping / lazily-loaded lines for files beyond RAM-copy scale.

---

## 12. Feature Priorities

Given the deficits, the honest order is: (1) kill the O(n) hot paths (line index + view cache) — without this nothing else scales; (2) fix undo correctness + trees; (3) atomic/async saves + fix the autosave race — this is a data-loss bug today; (4) damage-based rendering; (5) only then continue breadth (net/VC/LSP/dired are semantics on a core that cannot yet hold a 5 MB file comfortably).

---

## 13. Comparison With Existing Editors

- **kakoune/helix/micro** — each is built on a piece-table/rope + a line index and a damage loop; magma's feature list rivals theirs but its hot paths are their design bottom, inverted.
- **vim/neovim** — undo is a persistent tree with working `undojoin`, marks are versioned, saves are atomic-enough, layouts are per-window; every one of those is either missing or wrong here.
- This project is best compared to a **well-executed 2014 prototype-era editor**: vision exceeds mechanism. The roadmap says "millions of lines" — at that scale the editor as built would not just lag, it would lose work.

---

## 14. Rewrite Decision

Decision: **Partially redesign, with a substantial rewrite of the text core, and surgical deletion of parasitic subsystems. Do not fork or rewrite the whole application.**

- Preserve: the Rust/Janet split, the event/command/keymap scaffolding, the buffer-rope layer, the test discipline, and the existing feature surface.
- Substantially rewrite: position/cursor model (add line index, grapheme+wide-cell handling), the whole render/highlight path (damage tracking, dirty ranges), undo (persistent tree), and I/O (async saves, atomic writes, safe modified-flag).
- Delete/replace: the unversioned semantic-engine request queue, dead `ts_trees`, the `EDITOR_PTR` aliasing pattern (move to a scoped `&mut` hand-off per nested call), and the 200 ms merge window.

A full rewrite is the wrong call — it would torch 1319 passing tests and a genuinely coherent scripting surface to fix four localized subsystems. A "keep fighting around it" call is equally wrong: the current core actively corrupts undo state and data-safety semantics at scale.

---

## 15. Final Scorecard (1–10)

| Dimension | Score | Reason |
|---|---|---|
| Architecture (Rust/Janet split) | 8 | Correct shape, honestly followed |
| Core data model | 3 | O(n) positions, no line index, no grapheme model |
| Performance | 2 | O(n) per keystroke; 5+ full-file copies/frame |
| Memory | 3 | Full-buffer strings as the default working unit |
| Undo/redo | 2 | Wrong mark restore (proven); no tree; merge corruption |
| I/O & durability | 3 | Non-atomic, UI-blocking, autosave race |
| Concurrency/FFI soundness | 3 | `EDITOR_PTR` aliasing; parallel-suite deadlock |
| Test suite | 7 | 1319 passing, real; but hangs and doesn't gate hot paths |
| Docs/API structure | 7 | `janet-api.md` formulaic and complete |
| Feature breadth | 7 | Massive; but built on a core that can't hold up |
| Overall | **4 / 10** | Promising shell, dangerous core |

---

## 16. One-Paragraph Verdict

Magma is an editor with the ambition of Neovim, the scripting model of Kakoune, and the text engine of a toy: on every keystroke it re-reads and re-parses the whole file, its undo system can put marks in the wrong place and secretly glue or shred history, and its save path can lose data while looking successful. The architecture's founding decision — a typed Rust kernel plus a Janet behavior layer — is the right one and should be preserved; what must perish is the O(n)-everywhere position/render model, the linear-scan undo, and the unversioned async plumbing that currently owns your backspace key and your modified-flag. Fix the four subsystems decisively (line index, damage tracking, undo tree, atomic async saves), keep the tests and the scripting surface, and the foundation under those 1319 tests will finally be trustworthy. Until then, this is not yet an editor you'd hand a 100 KB file you cared about.

---

*Verification note:* `cargo test --features janet` serial → 1319 passed / 0 failed in 16.6s and the crate compiles (with new warnings). Default parallel runs stalled in 3 of 4 full attempts; the undo-mark bug was reproduced with a dedicated test. Stale-LSP-diagnostic and LSP-debounce behaviors could not be fully established from code alone and are flagged as such.