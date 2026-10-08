# GUI Redesign Plan

Source: `docs/gui-ux-review.md` (brutal IDE review, overall 3/10).  This plan
maps every P0/P1 finding to a sprint, in dependency order, grounded in the
**current** tree — several review findings are already stale (see Baseline).

## The one decision this plan makes

The review's §26 verdict — "two unmerged philosophies" — is resolved as
follows, and every sprint below enforces it:

> Magma is a GUI IDE whose editing model is vim and whose scripting layer is
> Janet.  It has **one command surface** (the registry: colon verbs, palette
> entries, and dialogs are all views onto it), **one keymap authority** (the
> keymap manager; the GUI may intercept only a small declared table), and
> **one feedback channel** (the status message line + `*Messages*` log).

Practical rules that follow:

1. No new user-facing behavior may exist in only one backend.  A colon verb
   and its GUI affordance must route through the same command.
2. GUI key pre-emption is allowed only via the declared table in
   `gui_dialog.rs::shortcut` + the quit guard — anything else goes through
   `dispatch_key`.  A lint (Sprint 4) rejects duplicate binds anywhere.
3. Rust owns surfaces and state; Janet owns behavior (per AGENTS.md).
   Every new GUI affordance dispatches an existing or new registry command.

## Baseline — already true in this tree (do not re-plan)

| Review finding | Status |
|---|---|
| P0: `:q`/window close destroy unsaved work | **Fixed** — guarded `quit` command on all three paths + Unsaved Changes dialog + `save-all` (uncommitted) |
| P0: silent 30s auto-save | **Fixed** — opt-in via `auto-save` option, default off |
| Fresh buffers report modified | **Fixed** — `UndoHistory` starts at the loaded baseline |
| Project index walk blocks UI | **Fixed** — `project/index-files` is async (`BackgroundEvent::FileIndexed` → ProjectGraph) |
| Recent projects / sessions | Exists (`project/register|recent`, `workspace/session-*`, `~/.magma/projects`) but **recents are not loaded at startup** and there is **no open flow** — Sprint 1 |
| Colon input invisible in GUI | **Fixed** — `MinibufferWidget` in the status bar |
| Completion popup missing in GUI | **Half-fixed** — popup renders on the Surface, but nothing in insert mode can trigger or cycle it — Sprint 4 |
| Semantic engine missing | API exists, tree-sitter backed, **never called from builtins/** — Sprint 3 |
| Quickfix | Infra exists (`QuickfixEntry`, make-parse, ripgrep via `quickfix/set`) but **no GUI panel**, results unreachable — Sprint 3 |

## Sprints

Each sprint closes with the AGENTS.md gates: `cargo build --features janet`
and `cargo test --features janet` zero failures, `janet-api.md` entries for
any new C function, file-size limits respected (note: `gui.rs` is at 400
lines — all new chrome goes into new files).

### Sprint 0 — Truth and quick wins  *(small)*

The cheap credibility repairs; no architecture.

- Docs sweep: remove/rewrite claims of a preserved TUI, the REPL toggle
  (`ctrl-`` ` ``), and getting-started's ASCII mockup of a screen that does
  not exist.  Document `:q` guard + `auto-save` (partially done).
- `PositionWidget` (gui_status.rs): report 1-based column (all other editors
  say Col 1; ours says Col 0).
- Command palette (gui_goto_dialogs.rs): stop dropping descriptions
  (`list_detailed` description discarded at the filter); stop silently
  hiding commands that take arguments — show them, flagged, and route
  argument-less invocation to the minibuffer prompt instead of filtering.
- Startup: log "Magma UI ready" into `*Messages*` without surfacing a tab;
  first launch shows the editor alone (welcome screen arrives in Sprint 1).

Closes: review §19 "docs describe a screen that doesn't exist", P2 modeline,
half of the palette P1.

### Sprint 1 — First minute: project open  *(medium, highest impact)*

The review's #1 wall: "no way to open a project from the UI".

- Add `rfd` (native folder/file picker) behind the `gui` feature.  New
  command `open-project` (Rust, arg: path) → `project/set-root` +
  `project/index-files`; the GUI Open-Folder affordance dispatches it with
  the picked path.
- Load recent projects at startup (`persist::load_recent_projects` has no
  production caller) and render a **welcome overlay** in the central pane
  when no project is open and only `*scratch*` exists: Open Folder, recent
  projects (click → `open-project`), and a five-line key cheat sheet
  (`:`, `ctrl-p`, `ctrl-o`, `ctrl-s`, `ctrl-q`).  New file
  `gui_welcome.rs`; `gui.rs` only gains a dispatch branch.
- Sidebar empty state: "No project open — ctrl-shift-N to open a folder".
  Bind Open Folder to that key and to the tool strip.
- File watching for the project root: rescan (debounced) on FS events so
  the tree reacts to new/deleted files; `file_index_dirty` gets a consumer.

Acceptance: fresh launch → welcome with recents; open folder → sidebar
fills async; restart → recents offered.  Closes review P0 #4 (project open),
§8 (empty tree at 10 files).

### Sprint 2 — The editor speaks: one feedback channel  *(small, unblocks 3+)*

- `Editor.status_message: Option<(String, Instant)>` + transient render in
  the status bar (MessageWidget, error vs info styling from the theme).
- New Janet API `editor/echo msg &key type` (logs via `editor/log-message`
  AND sets the transient message).  Document in `janet-api.md`; test both
  sides.
- Route every dropped result into it: `execute_command` failures in
  gui.rs/gui_dialog.rs (`let _ =` sites), colon-mode errors (colon_mode.janet
  `print` → `editor/echo`), task-completed/task-failed handlers (task.janet
  `print` → `editor/echo`).
- `:q` refusal, save failures, "Unknown command" become visible everywhere.

Acceptance: any failing action shows a readable message within one frame;
`*Messages*` retains the log.  Closes review P0 #6 (error UX = 1/10) and
makes the colon `:q` refusal visible (gap left by the quit-guard fix).

### Sprint 3 — The dev loop  *(large)*

Edit → build → see failures → jump.  All plumbing exists; this sprint wires
surfaces onto it.

- **Output/quickfix panel** (`tool_window.rs` or new `gui_quickfix.rs`):
  task-completed/task-failed fetch `task/output` into a scrollable panel;
  compiler-error lines flow through the existing make-parse into
  `quickfix_list`; panel rows are clickable → `FocusBufferAndGoto` action
  (pattern already exists in DialogAction).  `:cn/:cp` stay the keyboard
  path; panel highlights the current entry.
- **Find-in-files on disk**: switch the tool window's source from open
  buffers to the ripgrep path grep.janet already uses (`quickfix/set`);
  render quickfix entries, clickable, with n/N stepping.
- **Symbol index actually runs**: builtins hook `BUFFER_CREATED` /
  `BUFFER_AFTER_SAVE` → `(semantic/index-buffer)` (async via scheduler;
  gated on `ts_languages`).  Goto-Symbol dialog and breadcrumb symbols
  become live; go-to-definition falls back to `semantic/definitions` when
  no LSP is attached.
- Search dialog: next/prev hit stepping (n/N while open), persistent
  highlight of the current hit, clickable results.

Acceptance: `:build` failing → panel shows the error; click → cursor on the
line; ctrl-shift-o lists symbols of the open file.  Closes review P0 #5
remnant, P1 symbol index, P1 unclickable find results, P1 invisible build
output.

### Sprint 4 — One mental model  *(medium)*

Kill the "two parallel realities" seam (review §5, §15).

- **Completion reachable**: bind `ctrl-space` (insert layer,
  editor_keybindings.janet) → `completion-trigger`; `ctrl-n`/`ctrl-p` cycle
  when `editor.completion.visible`, else fall through to the keymap.  The
  GUI consults completion state before palette pre-emption — one documented
  precedence rule.
- **Keymap lint**: `keymap/validate` (Janet API + Rust scan) rejects
  duplicate binds in a layer; run in tests and warn on `push_layer`.
  Add a test proving ctrl-d-style double binds can't ship.
- **Single file finder**: Goto-File (index-backed) is the only file
  finder surface; `:find` aliases it; palette lists commands only.
- **Sidebar conventions**: single-click select, Enter/double-click open;
  right-click context menu via egui `secondary_clicked()`: New File,
  New Folder, Rename, Delete (→ trash with confirm), Copy Path.  Destructive
  verbs go through Janet commands with an `editor/echo` confirmation trail.
- Search dialog, tabs, breadcrumb: one activation convention documented in
  gui-architecture.md.

Acceptance: a vim user and a mouse user can each predict where every action
lives.  Closes review P1 key fractures, P2 sidebar operations, §15
consistency items.

### Sprint 5 — Chrome consolidation  *(medium; partially deferrable)*

- Merge tabs + breadcrumb into one navigation band (JetBrains-style);
  the tab row gains full-path tooltips and overflow handling.
- Vertical splits land (view_tree already models `Split(Vertical)`; the
  layout engine needs the recursive pass), plus keyboard pane resize
  (`ctrl-w` `>`/`<`/`+`/`-`).
- Closing the last tab opens `*scratch*` instead of silently no-op'ing.
- Status bar widgets become clickable where an action exists (git branch →
  branch list, position → goto-line).

Closes review §2 (six bands → two), §9 tab/window gaps.

### Sprint 6 — Settings that mean something  *(medium)*

- Typed option registry in Rust: `{name → type, default, description,
  category}`, settable from Janet as today (string map stays the storage).
- Settings dialog renders categories with typed widgets, validation,
  defaults, and per-option reset.  Buffer-local vs global shown explicitly.
- Every option documented (generated from the registry into
  `docs/guide/options.md`).

Closes review §13 (settings = 3/10).

### Sprint 7 — Deep tooling, decision-gated  *(large; explicit go/no-go each)*

- **Terminal**: docked terminal accepts input and scrolls (route keys to the
  terminal buffer; today it is a read-only viewer).
- **LSP**: auto-load `plugins/lsp.janet` when a project declares a server;
  surface hover (popup), goto-definition, rename in the GUI.  If this is
  not funded, say so in docs — the review's "theater" criticism is worse
  than absence.
- **Debug**: same rule — a real variables/frames/callstack pane, or remove
  the debug GUI surface.  No middle state.

## Non-goals (deliberate)

- No TUI restoration; TUI references are deleted from docs, not code.
- No new icon set or theme work beyond Sprint 5 layout (Catppuccin stays).
- No plugin marketplace / config-format work — configuration stays Janet.

## Sequencing rationale

Trust (done) → first minute (S1) → feedback channel (S2, unblocks 3) →
dev loop (S3, the reason an IDE gets opened) → consistency (S4, stops the
bleeding of trust back in) → chrome/settings/deep tooling (S5–S7) once the
core promises hold.  Sprints 0–2 are each a few days; 3–4 are the bulk.
