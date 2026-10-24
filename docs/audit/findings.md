# Deliverable C — Detailed Findings Register

> **Status authority:** fix/status tracking for every finding lives in
> the root `PROGRESS.md` (updated per wave). This file records the
> original evidence, root causes, and acceptance criteria; its
> "Open findings" list below is historical.

Severity: P0 critical, P1 major, P2 significant, P3 minor.
Confidence: H high (read the code / reproduced), M medium (strong
inference). "FIXED" entries were repaired in this audit with regression
tests (`improvements.md`).

## Fixed during this audit

### F-03 — `goto-matching-brace` panics on non-char-boundary cursor — P0 — H — FIXED
- Repro: place cursor on a mid-codepoint byte (possible after UTF-16→byte
  conversion) in a buffer containing brackets; run the command.
- Root cause: `text[pos..]` / `text[..=pos]` sliced without boundary
  checks (`command/builtin/motion.rs`).
- Fix: snap pos down to a boundary; track the bracket candidate's real
  byte index; slice ends are always boundaries now.
- Tests: `command_tests.rs::goto_matching_brace_*` (3 tests).

### F-04 — Event bus self-emit infinite loop — P1 — H — FIXED
- A handler emitting its own event name grew the pending queue forever;
  the UI-thread drain never returned (editor freeze).
- Root cause: no re-entrancy guard in `EventBus::drain_and_dispatch`.
- Fix: `dispatching` name stack; re-entrant emits of an in-flight name
  are dropped (`event/mod.rs`).
- Tests: `janet_tests.rs::self_emitting_handler_terminates`,
  `event_tests.rs::emit_during_dispatch_of_same_name_is_dropped`.

### F-05 — Task stdout-before-stderr pipe deadlock — P1 — H — FIXED
- A child producing >64 KiB stderr with no stdout blocked forever
  (`task/scheduler.rs` read stdout to EOF first).
- Fix: both pipes drain on concurrent reader threads.
- Test: `task_system_tests.rs::stderr_heavy_task_with_no_stdout_completes`
  (1 MB stderr, completes <1 s; hung forever pre-fix).

### F-06 — PTY UTF-8 corruption at chunk boundaries — P2 — H — FIXED
- `String::from_utf8_lossy` per 4 KiB read replaced split multibyte
  chars with U+FFFD (`terminal/mod.rs:222`).
- Fix: incremental decoder carrying the incomplete tail across reads.
- Tests: `terminal_tests.rs::utf8_*` (3 tests incl. 4-byte emoji split
  three ways).

### F-07 — VC status buffer cursor→file mapping off by two — P1 — H — FIXED
- `VC_HEADER_LINES=5` but `build_display` puts the first file at line 3;
  stage/unstage/diff acted on the wrong file or none
  (`vc/mod.rs:63,179`). The old unit test asserted against the wrong
  constant, never against the rendered text.
- Fix: `path_at_line` now mirrors `build_display` exactly; tests assert
  against the rendered display (`vc_tests.rs`).

### F-08 — Open buffers silently diverge from disk — P1 — H — FIXED
- `FileChanged` only marked the index dirty; no reload, no prompt; save
  overwrote external edits. `builtins/ui.janet` claimed "Open files are
  reloaded by the kernel" — false at the time.
- Fix: `storage/reload.rs` — unmodified buffers auto-revert (cursor
  line/column preserved, undoable, own-save echo is a no-op), modified
  buffers warn and are never clobbered; removals are reported. Watch
  registration now covers files outside the project root.
- Tests: `file_reload_tests.rs` (5 tests).

### F-09 — Janet event-handler mutex deadlock — P1 — H — FIXED
- `event/on`'s closure held the `JANET_HANDLERS` std-Mutex across the
  Janet call; any handler calling `event/on`/`event/off` re-locked the
  same non-reentrant mutex on the same thread → permanent editor hang.
- Fix: copy the function value out, drop the guard, then call.
- Test: `janet_tests.rs::handler_can_subscribe_from_inside_a_handler`.

### F-10 — Three coordinate-model disagreements — P2 — H — FIXED
- Renderer places glyphs at tab-stop/cell columns; the highlight pass
  painted by char index (`highlight_pass.rs:65`), mouse clicks treated
  tab as 1 cell (`mouse.rs:310`), cursor metrics hard-coded tab width 4
  (`frame.rs:80`), multi-cursor columns summed char widths. Highlights
  misaligned and clicks landed wrong on any tab-indented line.
- Fix: all four sites now use the `text_layout` grapheme/tab-stop model.
- Tests: `mouse_tests.rs` (tab + wide-char clicks), `render_tests.rs`
  (selection/highlight cell coverage).

### F-11 — Shell-template VCS backend injection — P1 — H — FIXED
- `{file}/{msg}/{dir}` were substituted raw into a `sh -c` line
  (`vc/backends.rs:361`); a commit message or filename containing
  `$(…)` executed.
- Fix: values are POSIX single-quoted on substitution; doc contract
  updated (`docs/api/janet-api.md`).
- Test: `vc_backend_tests.rs::template_values_are_shell_quoted_not_executed`
  (sentinel file proves no execution).

## Closed by Milestone 2 + trust wave (R-8..R-11, R-13/R-14, R-17..R-20)

- **F-12 closed** — motions and search run on rope chunk scans; no
  document-sized copies per keypress.
- **F-13/F-34 closed** — regex mode, smart case, replace-next/all with
  capture-group expansion, one undo group per replace-all.
- **F-22 closed** — git status parsed from `--porcelain -z`.
- **F-27 partially closed** — saves (fsync ×2) and the find-files walk
  leave the UI thread; open-file stays sync by documented design.
- **F-28 closed** — no self-escalation; handlers/commands run inside
  their registration context; capability gates hold at runtime.
- **F-29 closed** — server auth token handshake; tcp-listen loopback.
- **F-30 closed** — process groups; tree kills for tasks/processes/PTYs.
- **F-31 closed** — bounded runtime shutdown.
- **F-33 closed** — .gitignore-aware walk; symlink-immune traversal.

## Open findings "Close the Semantic Loops" wave (follow-up commits)

- **F-26 closed** — typed diagnostics end-to-end; zero display-string parsers.
- **F-17 closed** — goto-definition converts UTF-16 in both directions.
- **F-16 (a/b/c) closed** — LSP completion drives the popup, hover feeds the
  dwell card, code actions open a quick-fix picker that applies edits.
- **F-19 closed** — `workspace/applyEdit` is forwarded to the foreground,
  applied, and answered truthfully.
- **F-20/F-21 closed** — all finished tasks parse tool output (rustc spans,
  warnings, test panics, generic file:line:col) into diagnostics + quickfix;
  severity is preserved; a live cargo-build e2e test pins it.

## Open findings

### Editor core

- **F-12 — Motion/search commands are O(document) per keypress — P2 — H.**
  Every `move-word-*`, `find-*`, `repeat-search`, `goto-matching-brace`
  materializes `buf.slice(pos, len)` (the whole tail) into a String
  (`command/builtin/motion.rs`, `search.rs`). Invisible at 1k lines;
  measurable allocation churn at 1 MB. Fix: rope-chunk-scanning
  predicates on `Buffer`.
- **F-13 — Search is substring-only — P1 — H.** No regex, no smart-case,
  no replace; `keyword-context` prints matches via `debug!` to stderr —
  invisible in the GUI (`search.rs:114`).
- **F-14 — No IME preedit — P1 — H.** egui commits composed text only;
  CJK users cannot work. Fix requires preedit overlay in the GUI input
  path.
- **F-15 — No bidi/RTL — P2 — H.** Logical-order rendering reverses RTL
  text visually.

### Language intelligence

- **F-16 — LSP results never reach real UI — P0 — H.** Hover →
  `editor/log-message`; code actions → log line, no apply; completion →
  log; the GUI hover card uses the *text index*, not LSP
  (`builtins/plugins/lsp.janet:129-146`). The protocol work is done; the
  product feature is absent.
- **F-17 — goto-definition UTF-16/byte mixing — P2 — H.** Request sends
  byte column; response handler adds the server's UTF-16 column to a
  byte line start (`plugins/lsp.janet:113,124`). Correct on ASCII lines
  only. `buffer/lsp-position` exists and is used elsewhere.
- **F-18 — Diagnostics version ignored — P3 — H.** `LspDiagnostics`
  discards `version` (`runtime.rs:268`); stale diagnostics can underline
  wrong lines until the next refresh.
- **F-19 — Server→client requests answered with null — P2 — H.**
  `workspace/applyEdit` (server-driven edits, e.g. rename on some
  servers), `workspace/configuration` get empty responses
  (`semantic/lsp.rs:345`).

### Build/run/test

- **F-20 — Build output never becomes diagnostics — P0 — H.** Quickfix
  parsing fires only for `*make*`; run-config launches bypass the
  output sink entirely (`task/mod.rs:30`, `run_config/mod.rs:185`). A
  failing `cargo build` produces no clickable errors.
- **F-21 — Quickfix parser naive — P3 — H.** Any `word:number:` line
  becomes an error; warnings and multi-line notes undifferentiated.

### VCS

- **F-22 — Porcelain parsing breaks on renames/quoted paths — P2 — H.**
  `line[3..]` keeps `old -> new` and C-quoted paths; `git add` then
  fails (`vc/backends.rs:47`). Use `-z`.
- **F-23 — No branch/checkout/hunk/amend/stash/conflict UI — P1 — H.**
  File-level stage only; conflicts appear as raw markers, `UU` rows
  double-listed.

### Architecture/reliability

- **F-24 — `Editor` god object + 60-variant event enum + 900-line
  `runtime.rs` — P2 — H.** Every subsystem couples to all three;
  `runtime.rs` already exceeds the project's own 400-line gate.
- **F-25 — Two text engines — P2 — H.** Canonical `Document` gateway is
  test-only; live `Buffer` runs the old path (`document.rs` header).
- **F-26 — Stringly diagnostics — P2 — H.** `"[E] line 12:3 msg"`
  re-parsed at three render sites.
- **F-27 — UI-thread blocking I/O — P1 — H.** `save-buffer` (full copy +
  2×fsync), `find-files` (full walk), `open-file` (sync read), `rfd`
  modal mid-frame.
- **F-28 — Extension capability self-escalation — P1 — H.**
  `(extension/undeclare)` restores full trust; `EditorAccess` declared,
  never enforced (`scripting/extension_api.rs:32,111`).
- **F-29 — `--server` unauthenticated eval — P1 — H.** Any local
  process reaching the port gets arbitrary Janet execution + file
  writes; `net/tcp-listen` binds `0.0.0.0` unconditionally.
- **F-30 — No process-group kill — P2 — H.** Terminal/task kills leave
  grandchildren (`sh -c` children) running; no `setsid`/`killpg`.
- **F-31 — Runtime drop can hang at exit — P2 — M.** No
  `shutdown_timeout`; a lane-starved blocking task blocks the drop.
- **F-32 — 219 `lock().unwrap()` outside tests — P3 — H.** One poisoned
  buffer mutex panics the UI thread cascade.
- **F-33 — Project walk ignores .gitignore; symlink cycles recurse
  forever — P2 — H.** `project/mod.rs:246` skips only dot-dirs,
  node_modules, target.

### UX

- **F-34 — Find dialog regex/smart-case absent; no in-file replace — P1 — H.**
- **F-35 — GPU render path second-class — P3 — H.** ASCII-only atlas,
  no HiDPI re-raster, off by default.
- **F-36 — No crash recovery of unsaved buffers — P2 — H.** Session
  restores paths/cursors only; scratch excluded.
