# Magma — Change-Propagation Architecture: Reconstruction and a Canonical Change/Revision Model

Companion to `docs/architecture-review.md`. Every claim cites `file:line`. Method: source
trace of one character insertion through every subsystem, plus the existing test suite
(same evidence base as the review).

---

## Part A — One character insertion, end to end

Magma has **two mutation entry points**, and the two divergent change-propagation
worlds are the core of the problem.

### A1. Entry point 1 — the editor's own typing path (the common case)

No Janet is involved, and **no event is emitted**.

1. `dispatch_key` (`src/kernel/input/mod.rs:88`) → no keymap hit → `accepts_text`
   → `handle_text_input` (`input/mod.rs:122-128`).
2. `handle_text_input` → `view.insert(cur, key)` (`input/mod.rs:158-187`, insert at
   `:184`).
3. `BufferView::insert` (`src/kernel/text_engine/view.rs:45-58`) locks the buffer's
   `Arc<Mutex<Buffer>>`, calls `Buffer::insert`, then recomputes its own cursor.
4. `Buffer::insert` (`src/kernel/text_engine/mod.rs:126-135`) does the *entire*
   per-edit work the engine will ever generate:
   - captures `mark_states = marks.all_positions()` (`:127`) — O(marks) copy;
   - `undo.record(Insert, mark_states)` (`:128-131`) — merge window decision;
   - `rope.insert` (`:132`) — the one genuinely incremental operation;
   - `marks.adjust_for_insert(offset, len)` (`:133`) — O(marks) shift;
   - `len = rope.len()` (`:134`).
5. `recompute_cursor_at` (`view.rs:126-132`) does `buf.rope.slice(0, offset).to_string()`
   + `Position::from_offset` (`src/kernel/text_engine/position.rs:14-46`) — an O(n)
   copy **and** char-by-char scan to rebuild a line/column the `Position` struct already
   stores.
6. `drain_and_dispatch` (`input/mod.rs:128`) runs on an **empty** queue.

**Everything downstream learns about this edit by polling the buffer, never by
notification.** The edit is invisible to the event bus, to syntax core, to tree-sitter,
and to LSP. That is not a staging problem; it is the designed state of the system.

### A2. Entry point 2 — the Janet path (macros, extensions, LSP-apply-edit, undo commands)

`(buffer/insert id pos text)` → `c_buffer_insert` (`src/kernel/scripting/buffer_text_api.rs:15-33`):

1. same `view.insert` as above;
2. `emit_buffer_changed` (`:9-13, :30`) queues `buffer-changed` with payload
   **`{ "buffer-id": "<id>" }` only** — no offset, no length, no kind, no revision
   (`:10-12`).
3. `drain_and_dispatch` (`src/kernel/event/mod.rs:112-122`) delivers the event
   synchronously to:

| Subscriber | Receives | Work it then does |
|---|---|---|
| `builtins/syntax_core.janet:153` (via `highlight-on-event` → `syntax/highlight-buffer`, `:124-140`) | just the id | full `buffer/slice` + 5 regex passes (keyword/comments/strings/numbers), per-char `string/slice` allocations |
| `builtins/treesitter.janet:311-317` (→ `ts/highlight-buffer`, `:293-307`) | just the id | `ts/ensure-language` + full-text copy into a fresh tree + full re-parse + full query (`src/kernel/scripting/treesitter_api.rs:32-50`). The `:309` "debounced" comment is a lie — there is no debounce anywhere |
| `builtins/plugins/lsp.janet:131-139` | just the id | `buffer/slice buf-id 0 len` (full copy, `:136`) + `json-escape` of the whole buffer + full-text `textDocument/didChange` with **hardcoded version `2`** (`:137-139`) |

### A3. The frame path — how the renderer "learns" (it never learns)

`render_frame` (`src/kernel/render/frame.rs:102-132`) runs per pane on every frame. The
GUI repaints continuously and unconditionally (`ctx.request_repaint()` at
`src/kernel/render/gui.rs:321`). For a single pane the per-frame cost after one character is:

- `buf.slice(0, buf.len())` — full copy (`src/kernel/render/editor_view.rs:61`);
- `compute_scroll_state_raw` — scans `text[..cursor]` twice, for line and column
  (`frame.rs:52-57`, `:69-76`);
- per visible row: `line_start_offset` rope walk (`editor_view.rs:110-111`);
- `apply_highlights` (`src/kernel/render/highlight_pass.rs:10-113`) — rebuilds the
  full highlight list from committed layer arrays (`:28-42`), then for every range
  walks visible rows mapping byte→column by scanning (`:61-82`). The layer ranges are
  **absolute byte offsets** computed against an *older* text revision — on the Rust
  typing path they never get refreshed, so after any plain edit they point at the
  wrong text (miscoloring, not just staleness);
- selection pass — `editor.selection_range` does another full-prefix slice and scan
  (`src/kernel/state/mod.rs:322`, `:326-328`), called from `highlight_pass.rs:86`;
- then, independently, the same buffer re-sliced whole again in:
  `gui_minimap.rs:71`, `gui_status.rs:77`, `gui_breadcrumb.rs:70`,
  `status_and_popup.rs:44`, and on click `input/mouse.rs:206`.

### A4. The persistence path

- **Autosave** (every 30 s, `src/main.rs:198-235`): polls `modified()` (derived from
  the undo save index, `src/kernel/text_engine/undo.rs:196-200`), takes a full
  `slice(0, len)` per dirty buffer (`main.rs:212`), `spawn_blocking` writes, then
  `mark_saved()` (`main.rs:230`) **even if edits landed after the snapshot** — a flag
  race that can clear "modified" over unsaved work.
- **save-buffer**: full slice + synchronous `fs.write` on the editor thread
  (`src/kernel/command/builtin/buffer.rs:21-38`).
- Non-atomic final write (`std::fs::write`, no temp+rename, no fsync;
  `src/kernel/storage/disk.rs:22-31`).

### A5. What one insert produces today, summarized

```
keystroke
 → rope.insert                        (incremental ✓)
 → undo.record O(marks)               (snapshot; also merge-clobbers mark states, undo.rs:102/117)
 → marks.adjust_for_insert O(m)       (eager; re-derived again by undo, then overwritten by a
                                       snapshot from the wrong group, undo.rs:174-186 + mod.rs:198-200)
 → view recompute_cursor O(n) copy+scan
 → [Rust path: nothing else]
 → [Janet path: buffer-changed {id}]
     → 3× full-buffer re-derivation: regex×5 / ts full reparse / LSP full-text didChange
 → render: 6× full-buffer slice + 4+ prefix scans, every frame, forever
 → autosave: another full slice per dirty buffer
```

The same byte exists in RAM as a full string **seven or more times within one frame**,
and the only piece of data that would make every consumer work incrementally — *what
changed* — is never produced.

---

## Part B — Duplication of derivation

The engine derives four facts it formally needs; each is derived in multiple places,
by different scan strategies, most of them O(n):

| Derived fact | Sites that derive it | Redundant? |
|---|---|---|
| **Full text string** | `editor_view.rs:61`, `frame.rs:88`, `gui_minimap.rs:71`, `gui_status.rs:77`, `gui_breadcrumb.rs:70`, `status_and_popup.rs:44`, `mouse.rs:206`, `main.rs:212`, `buffer.rs:28`, `lsp.janet:136`, `syntax_core`/`treesitter` scans | 6×/frame in GUI, 2× on save paths, 3× on Janet edits |
| **Cursor line & column** | `view.rs:38-40`, `:98-100`, `:111-112`, `:129-131`; `frame.rs:52-76`; `gui_status`; breadcrumb; minimap; `mouse.rs`; `editor_view.rs:202-212` (multi-cursor); `state/mod.rs:322` (selection) | ≥7 places, all prefix text scans, all derived from a `Position` that is already updated the same way |
| **Line-start per visible row** | `editor_view.rs:110-111`, `highlight_pass.rs:47-48` and `:94-95`, decorations, minimap, `mouse.rs:209` | recomputed per pass per row instead of shared/line-indexed |
| **Mark positions** | `mod.rs:133` (eager), `mod.rs:190/194` (re-derived inside undo), `mod.rs:198-200` (overwritten by a snapshot) | triple derivation, two of them in conflict — the third is the proven bug |
| **Modified flag** | `undo.rs:196-200` alone, polled by status bar and autosave every frame/30 s | derived from a mutable index (`saved_at`) that truncation turns into a lie |
| **Syntax layer validity** | specified nowhere | layers are unversioned byte ranges consumed blind by `highlight_pass.rs:61-82` |

The deepest duplication: **three independent systems each own a claim about "span
arithmetic after an edit"** — the undo log (owns "what changed, but records it
per-group with O(m) snapshots), the mark-adjusters (own "how offsets move", eagerly),
and the frame loop (owns "what changed since I last painted", by comparing nothing and
re-reading everything). None of them can answer "what changed between revision R and
now?" and all three re-derive the same math in incompatible ways.

---

## Part C — A single canonical Change/Revision model

### C1. Core idea

One edit produces one small algebraic object. Every consumer — undo, marks, cursors,
selections, syntax invalidation, rendering damage, LSP sync, persistence — consumes
that object's two functions (`transform`, `invalidate`). **No consumer rescans text.**
This preserves the Rust/Janet split: the model is Rust-owned state; Janet consumes it
through the C bridge like any other buffer state.

```rust
// src/kernel/text_engine/span.rs  — the ONLY place edit → consumer math happens
pub enum SpanEdit {
    Insert { pos: usize, len: usize },
    Delete { pos: usize, len: usize },
    Replace { pos: usize, del_len: usize, ins_len: usize },
}

pub struct Change {
    pub from_rev: u64,
    pub to_rev: u64,
    pub edit: SpanEdit,            // coordinates are in the from_rev (pre-change) buffer
}

impl Change {
    // forward offset transform: o -> o'
    pub fn transform(&self, offset: usize) -> usize { /* one line of algebra */ }
    pub fn inverse(&self) -> Change;                 // for undo
    pub fn line_span(&self, first_line_len_cached: ...) -> (usize, usize); // affected lines
}

// src/kernel/text_engine/change_log.rs
pub struct ChangeLog { ring: VecDeque<Change>, rev: u64 }

impl ChangeLog {
    fn record(&mut self, e: SpanEdit) { /* bump rev, push */ }
    pub fn since(&self, seen_rev: u64) -> impl Iterator<Item = &Change>; // newest-first
    pub fn transform(&self, offset: usize, since: u64) -> usize;        // compose O(changes)
}
```

### C2. Changes to `Buffer` (`text_engine/mod.rs:61`)

```rust
pub revision: u64,      // monotonic; bumped by every mutation including undo/redo
changes: ChangeLog,
saved_rev: u64,         // replaces undo.rs `saved_at: Option<usize>` index
```

- `Buffer::insert/delete/replace` become thin wrappers over one shared
  `record_change(edit)` that: records to `ChangeLog`, applies the rope op, and
  **stops touching marks entirely**.
- Marks become `(pos, rev)`; `mark_pos(id)` lazily applies `changes.transform(pos,
  mark.rev)` — O(changes since that mark last moved). Undo no longer needs
  `mark_states` snapshots at all; mark positions after undo/redo are just
  `transform`ed through the inverse changes. This deletes the wrong-group restore bug
  (`undo.rs:174-186` + `mod.rs:198-200`), the merge-clobber (`undo.rs:102/117`), and
  the O(m) snapshot per op in one change.
- `modified()` becomes `revision != saved_rev`, O(1), monotonic-safe: `mark_saved`
  records the revision it saved and can no longer clear later edits (kills the
  `main.rs:230` race).

### C3. The consumer contract

One function `changes_since(seen_rev) -> Changes` feeds everyone:

- **Cursor / line / column (BufferView)** — store `cursor_rev`; per change apply
  `transform` plus a cached line-delta (did `\n` enter/leave the span?). Deletes the
  O(n) rebuilds at `view.rs:35-41 / :126-132` and the `frame.rs` re-scans.
- **Rendering** — `EditorView` (and minimap/status/breadcrumb) each store
  `drawn_rev`; if `buffer.revision == drawn_rev` the pane skips entirely (real
  damage tracking, kills `gui.rs:321`'s blind full repaint). When changed, extract
  only the visible window via `line_start_offset` — a `slice(0,len)` is never needed.
- **Syntax invalidation** — `buffer-changed` payload becomes `{buffer-id, since-rev,
  edits:[{pos, del-len, ins-len}, ...]}`. `syntax_core.janet` and `treesitter.janet`
  re-scan only `edit.line_span()` ranges; the tree-sitter layer moves to incremental
  `ts_tree_edit` on the fresh deltas instead of full re-parse; highlight layers are
  stamped `seen_rev` and rebased through `transform`, making the current
  miscoloring-after-typing impossible.
- **LSP sync** — real `textDocument/didChange` contentChanges: one entry per edit
  with `{range, rangeLength, text}`, extracting only the inserted/deleted span.
  Version comes from `buffer.revision` (fixes the hardcoded `"2"` at `lsp.janet:138`).
  Every diagnostic publish carries its `rev`; the client drops it when
  `buffer.revision > rev` (fixes the `client.rs` PENDING_REQUESTS staleness).
- **Persistence** — autosave/save snapshot at a captured `rev`; `mark_saved(rev)`
  monotonic; save can be made incremental (append span deltas to a journal) without
  changing the model.

### C4. One edit, after the model

```
keystroke
 → rope.insert (incremental)
 → ChangeLog.record(Insert{pos, len}); revision += 1        (O(1))
 → cursor/selection transformed per window                    O(1)
 → event bus: [Rust path: skipped if nothing subscribed]
              [Janet path: buffer-changed {id, since-rev, edits}]  → syntax re-scan of
              dirty lines only; ts incremental edit; LSP range-based didChange
 → render: pane sees rev changed → re-extract visible window only
 → autosave/save: compare saved_rev vs revision; async; atomic write
```

### C5. Rollout (each step gated by the existing test discipline in AGENTS.md)

1. `span.rs` algebra + `ChangeLog`, pure-Rust property tests (transform compose,
   inverse round-trip, undoing into a redo branch).
2. Route `Buffer` mutators through `record_change`; marks lazy; `saved_rev`.
3. `BufferView` cursor through transform.
4. Render `drawn_rev` skip + visible-window extraction.
5. Richer `buffer-changed` payload; incremental Janet highlight / treesitter / LSP.
6. Autosave snapshot-versioned + atomic write.

All existing 1319 tests stay green at every step; new primitives get new tests under
`src/tests/`.

---

## Part D — Direct answers

**Does anything existing compute "what changed"?** Yes, three incompatible models:
the undo log (per-group snapshots), the mark adjusters (eager byte math), and the
frame loop (nothing — it re-reads everything and diffs nothing). The first two cannot
answer "what changed between rev R and now" and the third doesn't ask.

**What recomputes what another subsystem already knows?** In one frame, the cursor's
line/column is derived ≥7 separate times by prefix scans, the full buffer is sliced
6+ times, line starts are re-derived per row per pass, and on the Janet path the same
insert is re-parsed as a whole file by three independent interpreters. Mark positions
are derived three times inside a single undo, two of which contradict.