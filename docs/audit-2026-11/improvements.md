# Implemented improvements — November 2026 round

Every fix below landed with regression tests; the workspace suite went
from 2,028 passing (baseline, pre-change) to **2,045 passing / 0 failed**
afterwards, `cargo clippy` shows no new warnings, and the fixes touched
no hot path without a test or benchmark-backed reason to believe it is
safe.

## Text engine

**EC-01 — `transform_range` mixed coordinate systems (P1, reproduced).**
`crates/text/src/view.rs`.  Root cause: the loop shifted `s`/`e` into
post-edit-*i* coordinates, then compared them against edit *i+1*'s
pre-transaction range (edits are documented as parallel in
pre-coordinates).  Design: endpoints stay in pre-transaction coordinates
with separate accumulated `shift_s`/`shift_e`; overlap cases record the
survival boundary and keep scanning later edits; shifts apply once at the
end.  Mirrors `transform_anchor_through_edits`, which was already
correct.  Tests (`crates/text/src/tests.rs`): span between two inserts
shifts by the first delta only; span across two deletes keeps the
surviving middle; `delete_many` undoes in one step.

**EC-08 — read-only enforcement gaps (P2).** `crates/text/src/view.rs`:
`read_only` now checked in `delete`, `replace`, `delete_many`
(`insert_many` already had it).  Test: read-only buffer survives all four
mutation paths unchanged.

**New primitive: `Buffer::delete_many`** (`crates/text/src/buffer.rs`) —
multi-range delete in one transaction, the mirror of `insert_many`,
needed for multi-cursor backspace symmetry and for the transform tests.

## Editing commands (kernel)

**EC-02 — cursor-left/right copies + char stepping (P1).**
`command/builtin/helpers.rs::move_cursor`: bounded 64-byte rope window +
`text_layout::prev_grapheme`/`next_grapheme` — O(1) per keypress and
grapheme-correct (clusters longer than the window degrade to char steps;
no real-world cluster approaches 64 bytes).

**EC-09 — O(prefix) backward scans (P2).** New in `crates/text/src/scan.rs`:
- `rfind_char_backwards(before, pred)` — geometrically growing backward
  windows; cost is the distance to the match, never the prefix length
  (the forward `rfind_char_before` walks `char_span(0, before)`).
- `prev_char_boundary(pos)` — O(log n) rope boundary lookup.

Rewired: backspace (was a full prefix walk per keystroke),
delete-word-back, adjust_number, move-word-back, find-char-back motions,
`*`/`#`, keyword-context.

**EC-04 — word motion hopped one word too far from delimiters (P2).**
`command/builtin/motion.rs`: `move-word-forward` now goes to the next
word start directly when the cursor is on a delimiter (from inside a
word: end of this word, then next start — pinned by the existing
`word_motions_walk_boundaries` test); `move-word-back` lands on the
start of the word the cursor is inside.

**EC-05 — logical column vs byte length (P2).** `move_to_line_col`
converts the char-count column to a byte boundary within the target
line.  Test: up/down across a `ééé` line keeps column 1 on byte 6.

**EC-06 — multi-cursor typing corruption (P1).** `input/mod.rs::
apply_to_all_cursors`: one `insert_many` transaction for all cursors
(one undo step), then every cursor — primary and extras — repositioned
to sit after its own insertion (`p_i + (i+1)·len`).  Test: two cursors,
two keystrokes produce `XYhelloXY world`, extras in lockstep.

**EC-10 — two-step atomic commands (P2).** replace-char, join-lines,
toggle-case, lowercase/uppercase-region now use single-transaction
`view.replace` (one undo step, one `BufferChanged` each).

## Search

**EC-03 — find-bar keystroke cost (P1).** `command/builtin/search_query.rs`:
regex `find_from` materializes only the tail from `from`; `find_all`
materializes once and iterates in place; the limit is checked before
each scan.  Literal search stays chunk-scanned.

**EC-11/EC-12 — `n`/`N` vs the highlight set; quadratic wrap (P2).**
`command/builtin/search.rs::execute_search` compiles the same
`SearchQuery` the find bar used (smart case) and steps that match set;
new `SearchQuery::rfind_before` gives backward search with one
materialization instead of a per-match rescan loop; re-highlighting uses
the same query.  All 34 pre-existing search/command tests pass unchanged.

## Rendering (slint-shell)

**GUI-1 — style attributes never rendered or hashed (P1).**
`pane.rs` (896 lines, over the 400-line gate) was split into
`pane/mod.rs` (grid geometry, font measurement) and `pane/blit.rs`
(rasterization) — the rule requires the split before adding logic.
`blit_rgba` now renders `dim` (fg halved, same contract as the terminal
blit), `underline` (bottom cell row in fg), `strikethrough` (middle row),
and `italic` (oblique via sheared glyph sampling — no second face
rasterization).  `surface_signature` hashes every style bit it renders.
The italic test caught a real bug in my first shear (left-edge clamp
smeared ink backwards) — pixels left of the shift now sample nothing.
6 new tests; all 16 pane tests pass.

**GUI-2 — Output pane re-projected the whole log per keystroke (P2).**
Kernel: `last_task` writes go through a 512 KiB tail cap (char- and
line-safe; diagnostics parsing still sees the full `RunCompleted`
payload) and bump a new `last_task_generation` counter
(`state/mod.rs`, `runtime/handlers/task.rs`).  Shell: the row projection
is cached and rebuilt only when the generation changes
(`app_state.rs`, `sync/mod.rs`); `projection::output()` became
`output_title()` (the full-text clone was dropped on the floor every
refresh).  2 kernel tests.

## Project model

**PM-01 — watcher dropped every Remove event (P1).**
`storage/watcher.rs`: the event filter keeps the raw path when
`canonicalize` fails (deleted paths cannot canonicalize); the reload
layer already handled raw paths.  Regression test drives a **real**
notify watcher: create → watch → delete → assert a `removed` event
arrives (pre-fix: never).

**PM-04 — find-files hung forever on a symlink cycle (P2).**
`command/builtin/buffer.rs::walk_matching`: symlinked directories are
excluded (matching the project walk's rule); symlinked files still match
by name.  Regression test creates `ln -s . loop` and runs `find-files`
synchronously — pre-fix this test hangs.

**PM-05 — session.json written non-atomically (P2).**
`crates/session/src/lib.rs` now writes through `fs::atomic_write`
(temp + fsync + rename), matching crash snapshots, index store, and
settings — the one session file torn write destroyed.

## Second tranche — completing the register ("continue until all fixed")

**EC-07 — regex mode reachable.** `search-buffer`/`replace-next`/
`replace-all` take an optional `regex` flag; the find bar ships a `.*`
toggle (`SearchState::regex` remembers it so `n`/`N` step the same match
set); `replace-next` now expands capture groups.  Tests: regex search
highlights both `fn <name>` spans; `$(\w+)=(\d)` → `$2:$1` round-trips.

**EC-10 — typing insert session.** `handle_text_input` opens the undo
session on the first typed character; `execute_command` closes any open
session before running a named command.  A typed run is one undo step;
a command between keystrokes splits the run.  Two tests pin both halves.

**EC-13 — split-view coherence.** `Buffer` records a capped tail of
committed transactions (`txn_seq`, `recent_txns`, cap 256); every
`BufferView` tracks `synced_txn_seq` and `sync_fork()` replays missed
transactions into its cursor, annotations, and decorations.  Sync runs on
`view_of_buffer_mut`, `for_each_view_of_buffer_mut`, and
`dispatch_events_scoped` (the render funnel).  A gap beyond the tail
degrades to a cursor clamp rather than a wrong replay.  Test: edit
through one fork, the split fork follows.

**EC-14 — decoration transform.** New `decor_view.rs` (the decoration
impl, split out of `view.rs` per the size gate): mutations capture each
decoration's byte offset before committing and re-derive (line, col)
after through the anchor transform.  Tests: insert above moves an EOL
hint; insert inside the line shifts the column; undo restores.

**EC-15 — folds open under the caret.** `emit_cursor_moved` (every
navigation path funnels through it) opens any fold containing the
caret.

**EC-16 — motion and bookkeeping nits.** `find-forward` strictly forward;
`goto-screen-top/middle/bottom` from the real viewport; the `g;` change
list records by focused-buffer revision change (the name blocklist is
gone); LSP `applyEdit` groups per file into one undo session.

**PM-02/03/06 — project-switch lifecycle.** `open_root` shuts down LSP
servers, closes terminals, and disconnects DAP when switching; the frame
sync drops the outgoing project's recursive root watches and per-file
watches (`retain_project_roots`/`clear_watched_files`); the incoming
project's persisted index loads at open.  One test drives a real fake
language server (`cat`) through a switch and asserts all of it.

**PM-07 — watcher event cost.** Canonical root cached at open; list
membership O(1) via `Project::files_hash` (was a linear scan over the
whole file list per event); scripting-surface emission throttled to one
per 50 ms in bursts.

**PM-08 — split panes persist.** `SessionState::split_buffers` records
window forks (window order); sync and async restore re-fork them.  Round
trip pinned by a test (including that `open-project` itself performs the
restore).

**PM-09/10/11/12 — small correctness.** Reload re-checks dirtiness at
apply time (test: keystrokes during the async read survive); snapshot
delete joins the Maintenance lane (single manifest writer); clean-exit
clear runs synchronously; watch failures surface as user messages and
oversized-buffer crash-recovery skips warn.

**GUI-3 — projection gating.** Problem rows rebuild only on
`quickfix_epoch`, Find-panel rows only on the search generation.

## Verification (final)

- `cargo build --features janet` → 0 warnings.
- `cargo test --features janet --workspace` → **2,055 passed / 0 failed**
  (baseline before the round: 2,028/0; 27 tests added, 0 regressions).
- `cargo clippy` — no warnings introduced by this round's changes
  (remaining ones live in files the round did not touch).
- `pane.rs` split brings the shell's largest file from 896 to two files
  under the 400-line gate; `view.rs` decorations moved to `decor_view.rs`
  for the same reason.
