# Deliverable F — Implemented Improvements

All changes build clean (`cargo build --features janet`, 0 warnings) and
the full suite passes: **1,446 tests, 0 failures** (baseline 1,426; +20
regression tests, +1 test rewritten against real behavior).

| # | Finding | Root cause | Files changed | Design decision | Tests |
|---|---|---|---|---|---|
| 1 | F-03 brace panic | byte slicing without boundary checks | `command/builtin/motion.rs` | snap cursor down to boundary; track bracket candidate's true byte index so every slice end is a boundary | `command_tests.rs` ×3 |
| 2 | F-04 event self-emit hang | no re-entrancy guard | `event/mod.rs` | `dispatching` name-stack; re-entrant emit of an in-flight name is dropped (a cycle can only be recursion) | `event_tests.rs` ×1, `janet_tests.rs` ×1 |
| 3 | F-09 handler mutex deadlock | Mutex guard held across VM call | `scripting/event_api.rs` | copy the GC-rooted fn value out, drop guard, then call | `janet_tests.rs` ×1 |
| 4 | F-05 task pipe deadlock | stdout drained to EOF before stderr | `task/scheduler.rs` | two reader threads join; both sinks stream events as before | `task_system_tests.rs` ×1 (1 MB stderr) |
| 5 | F-06 PTY UTF-8 corruption | lossy decode per 4 KiB chunk | `terminal/mod.rs` | incremental decoder; incomplete tail carried across reads; invalid bytes → U+FFFD and decoding resumes | `terminal_tests.rs` ×3 |
| 6 | F-07 VC cursor mapping | `path_at_line` disagreed with `build_display` | `vc/mod.rs` | walk mirrors the rendered layout exactly; `VC_HEADER_LINES` now names the true first-entry line (3) | `vc_tests.rs` ×1 rewritten to assert against rendered text |
| 7 | F-08 stale buffers | FileChanged had no buffer-side handling | new `storage/reload.rs`; `runtime.rs` (+`BufferReloaded` event); `gui_shell.rs`; `builtins/ui.janet` | unmodified ⇒ auto-revert via one undoable transaction (cursor line/char-column restored); modified ⇒ warn, never clobber; identical content (own-save echo) ⇒ clean-mark only, no undo entry; removals reported; per-frame watch registration for out-of-project files | `file_reload_tests.rs` ×5 |
| 8 | F-10 coordinate model | 4 sites bypassed the grapheme/tab-stop layout | `render/frame.rs`, `render/editor_view.rs`, `input/mouse.rs`, `render/highlight_pass.rs` | single model (`text_layout::{cell_column, grapheme_cell_width}`) everywhere: cursor metrics take the `tab-width` option; clicks walk the same placement as the renderer; highlights/selection paint cell columns incl. wide-glyph trailing cells | `mouse_tests.rs` ×2, `render_tests.rs` ×2 |
| 9 | F-11 template injection | raw substitution into `sh -c` | `vc/backends.rs`, `scripting/vc_api.rs` docs, `docs/api/janet-api.md` | POSIX single-quote substituted values; templates must not quote placeholders (doc contract updated) | `vc_backend_tests.rs` ×1 (sentinel proves non-execution) |
| 10 | stale claim | ui.janet documented a reload that did not exist | `builtins/ui.janet` | kernel handles reporting; duplicate Janet handler removed | (behavior covered by F-08 tests) |

## Notes and deliberate non-changes

- `save-buffer` stays synchronous on purpose: the quit guard's
  dirty-buffer semantics depend on the save completing before the state
  is trusted. Moving it to the existing async-write path needs a
  "save in flight" state on `IoState` first — recorded in the roadmap.
- The event-bus guard changes semantics for self-re-entrant emits: they
  are dropped instead of looped. Full suite green confirms no shipped
  builtin relied on the loop.
- No new Janet C functions were added, so `docs/api/janet-api.md` needed
  only the `vc/register-backend` contract clarification.
