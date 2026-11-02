# Findings register — November 2026 round

Severity: P0 critical · P1 major · P2 significant · P3 minor.
Confidence: H (read the code / reproduced) · M (strong inference).
Status: FIXED entries were repaired in this round with regression tests
(`improvements.md`); OPEN entries are tracked in `roadmap.md`.

## Editor core

| ID | Sev | Conf | Status | Finding |
|---|---|---|---|---|
| EC-01 | P1 | H (reproduced) | **FIXED** | `BufferView::transform_range` (`crates/text/src/view.rs:74`) compared a range shifted into post-edit-*i* coordinates against edit *i+1*'s pre-transaction range: on any multi-edit transaction a span start preceding a later edit was double-shifted or a tail clipped, corrupting highlights/folds while the cursor (anchor transform) stayed correct. Reproduced with the repo's own crate: highlight `(9,12)` in `"0123456789abcdef"`, one transaction inserting 10 bytes at 0 and at 11 → old code `(29,32)`; correct is `(19,32)`. |
| EC-02 | P1 | H | **FIXED** | `move_cursor` (`command/builtin/helpers.rs:50`) materialized `buf.slice(0, pos)` per cursor-left keypress (`Buffer::slice` always copies) — a 1 MB allocation per arrow key at the end of a 1 MB buffer — and stepped by char, not grapheme, splitting combining clusters the renderer treats as one cell. Now: bounded 64-byte window + `text_layout::prev/next_grapheme`. |
| EC-03 | P1 | H | **FIXED** | The find bar runs `search-buffer` on every keystroke (`slint-shell/src/actions/find_bar.rs:21`); `SearchQuery::find_from` copied the whole document per call and `find_all` called it per match — O(matches × document) per keystroke (≈10 GB for a full set on 1 MB). Now: one bounded tail copy per query, iterated in place; limit checked before scanning. |
| EC-04 | P2 | H | **FIXED** | `move-word-forward` chained word-start→word-end→next-start: from whitespace/punctuation it hopped one word too far (`"foo bar"` from the space jumped past `bar`). Now lands on the next word start. |
| EC-05 | P2 | H | **FIXED** | `move_to_line_col` clamped a *char*-count column against a *byte* line length and added it to the line start: on multibyte lines cursor-up/down drifted left and could land mid-char (snapped to column 0). Now converts the char column to a byte boundary. |
| EC-06 | P1 | H | **FIXED** | `apply_to_all_cursors` (`input/mod.rs:279`) inserted per cursor via `set_cursor`+`insert` without updating `extra_cursors[i].pos` — the second keystroke inserted at stale offsets and corrupted text; N cursors also produced N transactions/undo steps. Now: one `insert_many` transaction, every cursor repositioned after its own insertion. |
| EC-07 | P2 | H | **FIXED** | Regex mode and capture-group expansion are unreachable: `search-buffer`/`replace-*` hardcode `regex_mode = false`. Now: the commands take an optional `regex` flag, the find bar ships a `.*` toggle (persisted in `SearchState::regex` so `n`/`N` step the same set), and `replace-next` expands capture groups like `replace-all`. Tests: regex search + capture replacement round-trips. |
| EC-08 | P2 | H | **FIXED** | `BufferView` enforced `read_only` in `insert` but not `delete`/`replace`/`delete_many`/`insert_many`: kernel-managed read-only buffers (`*messages*` etc.) were editable from the default keymap. |
| EC-09 | P2 | H | **FIXED** | Backspace-class commands were O(cursor offset) per keystroke: `rfind_char_before` walks the whole prefix (`char_span(0, pos)`). Added `rfind_char_backwards` (geometric backward windows — cost is the distance to the match) and `prev_char_boundary` (O(log n) rope lookup); rewired backspace, delete-word-back, adjust_number, word/find motions, `*`/`#`. |
| EC-10 | P2 | H | **FIXED** | Atomic actions committed as two transactions (all five now single-transaction `view.replace`), and typing had no insert session — "hello" was five undo steps. The typing path now opens the insert session on the first keystroke and `execute_command` closes it, so a typed run is one undo step until any explicit command interrupts. Tests: one-step undo of "hello"; a command between keystrokes splits the run. |
| EC-11 | P2 | H | **FIXED** | `repeat-search`/`reverse-search` used a case-**sensitive** literal scan while the find bar highlights a smart-case match set: pressing `n` visited only the exact-case subset. Both now compile the same `SearchQuery` and step the highlighted set. |
| EC-12 | P2 | H | **FIXED** | Backward search wrap was O(matches × document) (`find_sub_from` in a loop). `SearchQuery::rfind_before` materializes once. |
| EC-13 | P3 | H | **FIXED** | Per-view-fork coherence: the shared buffer now records a capped tail of committed transactions (`txn_seq`/`recent_txns`), every fork tracks its sync point, and `sync_fork()` replays what it missed into its cursor/annotations/decorations. Forks sync on `view_of_buffer_mut`, `for_each_view_of_buffer_mut`, and before each render (`dispatch_events_scoped`). Test: edit through one fork, the split fork's highlights and cursor follow. |
| EC-14 | P3 | M | **FIXED** | Decoration layers are line/col keyed and never transformed. Every mutation now captures each decoration's byte offset pre-commit and re-derives (line, col) post-commit through the cursor's anchor transform (new `decor_view.rs`; hooks in insert/delete/replace/many/undo/redo). Tests: insert above moves an EOL hint down; insert inside the line shifts the column; undo restores. |
| EC-15 | P3 | H | **FIXED** | No auto-open of folds when navigation lands inside one. `emit_cursor_moved` — the funnel every navigation path already goes through — now opens any fold containing the caret. |
| EC-16 | P3 | H | **FIXED** | `find-forward` is strictly forward (a target under the cursor no longer matches itself); `goto-screen-top/middle/bottom` derive from the real viewport (`pane_scroll_top_synced` + pane height) instead of hardcoded 20/10/EOB; the `g;` change list records by buffer-revision change instead of a name blocklist (mutations Janet-side now register, `save-buffer` no longer pollutes); LSP `applyEdit` is one undo session per file. |

## Project model / files / persistence

| ID | Sev | Conf | Status | Finding |
|---|---|---|---|---|
| PM-01 | P1 | H | **FIXED** | The watcher's event filter did `std::fs::canonicalize(p).ok()?` (`storage/watcher.rs:44`): a removed path cannot be canonicalized, so every `EventKind::Remove` was dropped before reaching reload/index — external deletions were invisible; saving later silently recreated the file. (Handler-level code was correct — the tests inject events directly, which masked the gap.) Now keeps the raw path; real-watcher regression test added. |
| PM-02 | P1 | H | **FIXED** | Project switch leaves the old project's LSP servers attached with the stale root. `open_root` now calls `lsp.shutdown_all()` on switch — servers were rooted in the outgoing project, and a survivor would answer project B with A's workspace. Test: a live fake server (`cat`) rooted in A is gone after switching to B. |
| PM-03 | P2 | H | **FIXED** | Switch leaks per-project subsystems. `open_root` now closes terminals and disconnects DAP alongside the LSP shutdown; the watcher gains `retain_project_roots`/`clear_watched_files`, called by the frame sync when the project changes (outgoing recursive roots and per-file watches stop feeding events). |
| PM-04 | P2 | H | **FIXED** | `find-files`' private walker used `path.is_dir()` (follows links) with no guard: a symlink cycle (`ln -s . loop`) hung the pool thread forever. Now symlinked dirs are excluded (like the fixed project walk); regression test creates the cycle. |
| PM-05 | P2 | H | **FIXED** | `session.json` was the only persistence hot path written non-atomically (`std::fs::write`); a crash mid-write silently resets the user's working set (open files, cursors, breakpoints). Now `fs::atomic_write` (temp + fsync + rename), matching crash snapshots/index/settings. |
| PM-06 | P2 | H | **FIXED** | After a switch, symbol queries served the previous project's index until the new walk completed. `open_root` now loads the new project's persisted index immediately (`IndexStore::open_project`), which also makes warm-start completions work again. Pinned by the switch-lifecycle test (no symbols from A leak into B). |
| PM-07 | P2 | M-H | PARTLY FIXED | The per-event foreground cost is gone: the canonical root comes from an open-time cache, list membership is a maintained `HashSet` (was a linear scan over the whole file list per event), and the scripting-surface `file-changed` emission is throttled to one per 50 ms during bursts (the tree rescan is driven by `file_index_dirty`, not the event). Still open: the rescan remains a full re-walk (no incremental tree update) and the channel stays unbounded — bounded by the drain budget in practice, an architectural item for the next round. |
| PM-08 | P2 | H | PARTLY FIXED | Split panes now persist: the session records window forks with file-backed buffers (in window order) and restore re-forks them (sync and async paths); the round trip is pinned by a test. Still open: pane geometry (directions/ratios), tool-window/dock state, and run-config selection — the next fidelity slice. |
| PM-09 | P3 | H | **FIXED** | `apply_reload` re-checks `modified()` at apply time and warns instead of clobbering. Test: edits landing after the event-time check survive the stale reload. |
| PM-10 | P3 | H | **FIXED** | `drop_snapshot` runs on the same 1-slot Maintenance lane as `write_snapshot` — the two manifest writers can no longer interleave. |
| PM-11 | P3 | H | **FIXED** | The clean-exit snapshot clear runs synchronously in `persist_session` (deleting a handful of files is milliseconds; the detached thread lost the exit race). |
| PM-12 | P3 | M | **FIXED** | `watch_project` returns its error and the frame sync surfaces it as a user message ("file watching unavailable … the project tree will not refresh automatically"); oversized unsaved buffers now log a warning when crash recovery skips them. |

## Rendering / GUI shell

| ID | Sev | Conf | Status | Finding |
|---|---|---|---|---|
| GUI-1 | P1 | H | **FIXED** | The pane blit rendered only glyph+fg/bg/bold: `italic`, `underline`, `strikethrough`, `dim` were dropped (the builtin theme ships `comment-face`/`unused-face` with `dim`, `deprecated-face` with `strikethrough`), and `surface_signature` didn't hash those bits, so an attribute-only change (async highlight landing, dead-code dimming) reused a stale frame. Now: all five attributes rendered (sheared oblique for italic, underline/strike rows, dim halves fg) and hashed; 6 new blit tests. |
| GUI-2 | P2 | H | **FIXED** | The Output pane projected `last_task` on every refresh — full String clone + per-line location parse per keystroke, and `last_task` was uncapped (a 50 MB log = ~100 MB of allocation per keystroke). Now: the kernel caps the pane copy at 512 KiB (tail), bumps `last_task_generation` on every write, and the shell rebuilds the row projection only when the generation changes. 2 kernel tests. |
| GUI-3 | P3 | H | PARTLY FIXED | The two unbounded per-keystroke projections are gated: problem rows rebuild only when the kernel bumps `quickfix_epoch`, and Find-panel rows only when the search generation changes (`set_results` bumps it). Tabs/breadcrumbs/terminal-tail remain O(small) constructions per keystroke by design — acceptable until the next dock lands, then grow per-surface dirty flags. |

## Not re-audited this round

LSP lifecycle edge cases, tasks/debugger/PTY depth, VCS operations, and
extension/security posture were scheduled for re-audit but could not run
(agent-quota limit).  October's scores for those subsystems carry over
unverified; their newest commits (semantic tokens, folding queries,
task-build/task-test dispatch) are the most likely spots for October-class
findings and should be first in the next round.
