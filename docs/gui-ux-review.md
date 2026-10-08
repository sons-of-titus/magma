# Magma — Brutal GUI/UX Review

Method: drove the binary in a real PTY and parsed raw ANSI renders; audited the exact rendering code (colors, pixel constants, interaction logic) with line citations; verified the GUI egui process live on a display. Where the shipped binary differs from the docs, the binary wins.

**Evidence summary.** Current HEAD (`default = ["janet", "gui"]`, `Cargo.toml:68`) is a GUI-only editor. The TUI renderer (`TuiRenderer`, `src/kernel/render/tui.rs`) has zero non-test callers. `main.rs:28` parses `--tui` into a variable literally named `_force_tui`; the GUI builds never use it. `--tui` is a no-op. GUI-OVERHAUL Phase 10 and `docs/guide/getting-started.md` claim a preserved TUI/REPL — the TUI is dead code and the docs describe screens that do not exist. A stale release build still renders a TUI; it was used to live-verify shared editor behaviors and is called out where cited.

---

## 1. The IDE as a Product

Fresh launch (`main.rs:33-47`): one `*scratch*` buffer. Janet startup (`builtins/ui.janet:94-102`) logs "Magma UI ready", which auto-creates a `*Messages*` tab you didn't ask for. Default window: an empty text area, a `*Messages*` buffer, an **empty project sidebar** (no project is loaded; nothing auto-invokes `project/index-files`), a breadcrumb bar, a status bar. There is:

- **No menu bar.** GUI-OVERHAUL's `File Edit View Navigate` menu was never implemented. `gui.rs` renders exactly six panels: bottom status, top tabs, top breadcrumb, left tool strip, left sidebar, central editor. No menu, no toolbar.
- **`gui_toolbar.rs` is listed as "created" in the GUI-OVERHAUL changelog but does not exist on disk.**
- **No way to open a project directory.** No open-folder/open-project surface exists anywhere in `src/` or `builtins/`. Files reach the sidebar only if you know to evaluate `(project/index-files)`.
- **No welcome screen, no tour, no empty-state guidance.** The getting-started ASCII mockup ("`*repl*` / Type (help)") describes a screen that does not exist.

User-facing behavior is split across four competing surfaces — vim key layers, colon mode, GUI modal dialogs, and a command palette — and none renders the others' state. In the GUI, colon-mode input has **no visible command line**: the minibuffer is drawn only in the dead TUI path (`frame.rs:145-147`), and `gui.rs` has no minibuffer widget. You can press `:` and type `w` and it saves while showing nothing on screen. Space is bound to nothing; which-key exists as a command but has no default binding.

---

## 2. Visual Design

Proportions are reasonable on paper (status `line_h + 8`, sidebar 220px, strips 28px, tabs 28px) but disjointed: five chrome bands stacked before text — tabs, breadcrumb, tool strip, sidebar, status — with the editor's top region sharing two unrelated bars with no visual grouping. Central editor has no distinct frame; the pane splits its right edge between a scrollbar and an off-by-default minimap.

- Single monospace face; no type hierarchy; no zoom; font size only via config code.
- Catppuccin Mocha/Latte (`gui_theme.rs:31-49`) is semantically consistent (blue=accent/git, red=close/error, green=terminal) — a genuine strength. But `diag-warn` and `highlight-fg` are hardcoded off-palette (`gui_theme.rs:373,375`), and GUI/editor palettes are synced by ad-hoc string emissary (`apply_editor`, `:361-393`).
- `gui_theme::icons` defines nine Unicode glyphs; four (`FILE`, `SYMBOL`, `COMMAND`, `SETTINGS`) are declared and unused; the breadcrumb writes a literal `"ƒ {}"` instead of `icons::SYMBOL`. Icon-only strips with ambiguous glyphs; only tool-strip icons have tooltips.
- Density is too low: at 1280×800, chrome-to-content ratio is embarrassing.

---

## 3. Editor Experience

The core is competent: rope buffers, real undo/redo, folds, multi-cursor, visual/replace/search layers, revision model. The experience around it fails:

- **Dirty state is unreliable.** GUI tab shows a modified dot (`gui_tabs.rs:147-154`) and status `[+]`, but a background **auto-save loop writes every modified file to disk every 30 seconds** (`main.rs:168-202`) unconditionally, then clears the modified flag. The "unsaved changes" indicator vanishes mid-edit; the file on disk changes under you; git status becomes a lie.
- **`:q` destroys work.** Live-tested: modified `main.rs`, `:q` + Enter → editor quit silently, work gone. `quit` (`src/kernel/command/builtin/misc.rs:17-24`) and `force-quit` (`:26-29`) are identical: both set `running = false`. No dirty-buffer check anywhere.
- **Completion is invisible in the GUI.** The popup is drawn only when `editor_pane_only == false` (`frame.rs:145-147`), i.e. only in the dead TUI. `grep completion gui.rs gui_editor_pane.rs` → zero matches. `ctrl-n` in the GUI cycles completions you cannot see; acceptance is blind.
- TUI-era render showed **no line numbers** (`number=true` at `ui.janet:98`, gutter provider registered, yet blank in capture) — render path was already broken once.
- Position widget uses **0-based column** (`gui_status.rs`) pretending to be vim's `Col 1`.

---

## 4–5. Navigation and Information Architecture

- **Symbol navigation is guaranteed empty.** Go-to-Symbol reads `SemanticEngine.symbol_index`, populated only by `(semantic/index-buffer)`, which no code in `builtins/` ever calls. The dialog can never find anything.
- **Find-in-files results are not clickable** (`tool_window.rs:287-298`); no open-on-Enter.
- **Search dialog has no next/prev stepping**, no persistent highlight; Enter jumps to the first hit and returns to the search box; replace is one-buffer literal, caps silently at 200 (`gui_search_dialog.rs:123-133`).
- **`ctrl-o`** = mark-ring-jump in `builtins/ecosystem.janet:99-101` but pre-empted as Goto-File by `gui.rs:162`. **`ctrl-p`** = find-files in the vim keymap, palette in the GUI. Same keys, different meanings, per backend.
- Three file-location UIs (palette/`find-files`, Goto-File buffer list, sidebar) with three data sources. Buffer/panes are also called "tabs" in one layer and "buffers" in another (`ctrl-w` window commands vs `tabnext`).
- The GUI dialogs did not replace colon mode; they shadowed it. Every feature lives in two places with different rules. No single mental model.

---

## 6. Keyboard UX

Vim layer is thorough (motions, text objects, window commands). Then:

- **`ctrl-d` bound twice** in the same layer: `vim_keybindings.janet:75` scroll-half-page-down; `:88` multi-cursor-next-occurrence. Later wins silently — basic vim users get phantom multi-cursors.
- GUI hijacks keys before the keymap: `ctrl-p`, `ctrl-o`, `ctrl-comma`, `ctrl-shift-f` all pre-empt it (`gui.rs:162`). Physical `cmd` on macOS is not the binding modifier; `ctrl` is a literal string (`gui_input.rs:151-158`).
- Palette filters out **all** argument-requiring commands (`gui_goto_dialogs.rs:77-79`), removing `open-file`, `save-buffer-as`, `goto-line` — the commands a palette exists for — and drops the description (`:79`).

---

## 7. Mouse UX

- **No context menu anywhere** — tabs, sidebar, editor, status, tool windows. The single most important discoverability surface is missing entirely.
- No middle-click close, no tab pinning. Tab drag-reorder works.
- Scroll wheel sends `px,py 0.0` (`gui_input.rs:46-66`): scroll-to-cursor targeting impossible.
- Sidebar `ROW_H = 22px`, 13px text; without a right-click menu to compensate it's a small, dead target. Tree rows single-click-select, double-click-open; directory single-click toggles expansion — two conventions in one widget.

---

## 8. Project Explorer

- **At 0 files:** empty until `(project/index-files)` is hand-evaluated. The one thing you cannot do from the UI is see your project.
- **At scale:** `project/index-files` is a synchronous recursive walk on the calling thread (`src/kernel/scripting/project_api.rs:111-130`) that blocks the UI thread; skips only dot-dirs/`node_modules`/`target` — **no gitignore, no depth cap, no file watching** (`file_index_dirty` has no rescan loop). No git status in the tree, no filter, no sort.
- No operations: no rename/new/delete/copy-path/reveal.

---

## 9. Tabs and Windows

Tab bar is the best chrome in the app: active underline with animation, hover-close, drag-reorder, modified dot. But: no pinned/preview tabs, no overflow handling, no tab context menu (close-others/all explicitly deferred), no filename-path tooltip. Closing the last tab collapses the pane with nowhere to type. Pane splits are horizontal-weight only ("no recursive nesting", `view_tree.rs`) — no editor grid; `ctrl-w±` resize does not exist.

---

## 10. Terminal / Build / Run / Debug

- **`:build`/`:test`/`:run` produce no visible output.** Task stdout/stderr lives in a `Task.output` struct; the user-facing lifecycle `►/✓/✗` lines print to process stdout (`builtins/task.janet:40-55`), which the GUI shows nowhere. The path that *does* surface output (`:!`/`:make` → `*shell*`/`*make*` buffers with quickfix) is undiscoverable.
- **GUI terminal tool window is display-only**: renders the active terminal buffer as read-only text (`tool_window.rs:185-210`); input only via the editor pane's `terminal-send-input`.
- **Debugging is a facade.** DAP client exists; there is no launch config, no variables, no stack, no watch, no debug pane. The complete GUI debug payload is a gutter toggle + `*Messages*` lines.

---

## 11. Error UX

- **No error surface anywhere in the GUI.** No toasts, no overlay, no status messages, no flash. Zero matches in `src/kernel/render/`.
- Every GUI action discards its result: `let _ = execute_command(...)` at `gui_dialog.rs:71,76,107` and `gui.rs:273-337`. Open a nonexistent file from Goto-File → dialog closes, nothing happens.
- Colon errors `print` to process stdout (`colon_mode.janet:181,68`): `Unknown command:` and `:e` failures are invisible in the GUI.

---

## 12. Git

The VCS layer is the most real-product part of the project (git/hg/svn/jj backends, status buffer with staging, diff/log/commit/blame, gutter signs). Entirely hidden behind colon commands; no GUI surface, no inline diff preview, no branch switcher, no stash. And: **no discard protection anywhere** — dired deletion (`dired-execute-deletion` → `remove_file`/`remove_dir_all`, bound `x`) is permanent, no trash, no confirm, no undo.

Accidentally destroy work? Yes — I did, in under a minute of testing.

---

## 13. Settings

- Settings dialog is an alphabetical dump of the raw `ed.options` string map (`gui_dialog.rs:146-153`): no categories, no types, no validation, no defaults, no reset. Keys are inconsistently styled (hyphenated `gui-theme` vs `number`).
- Buffer-local and project-local option stores are invisible to it; three parallel option systems, one UI.
- Fonts, themes, keybindings live in `~/.magma/init.janet` code. The docs boast "no separate config format — everything is code"; for a GUI-first product this hides everything meaningful while exposing trivia.

---

## 14. Responsiveness

Core loop (Rust + tokio, event-driven) is natively fast; locks are careful. But: sync file-tree walk blocks the UI thread; symbol indexing never runs; completion never renders so there is no completion latency to judge — only its absence; GPU path is env-flag gated with a placeholder atlas only recently real. Full frame redraw per event with no dirty gating.

---

## 15. Interaction Consistency

1. **Palette/insert/global `ctrl-p`**: three meanings (palette / completion-prev / find-files).
2. **`ctrl-o`**: Goto-File in GUI, mark-ring-jump in keymap.
3. **Activation conventions**: sidebar = single-select/double-open; tabs = single-open; breadcrumb = single-reveal.
4. **Dirty die:** `:q` and window-close never warn; auto-save clears dirty arbitrarily.
5. **Completion**: same keys, different lists (insert vs colon).
6. **Terminal**: buffer in the pane, read-only viewer in the dock.

---

## 16. Accessibility

Absent. No screen-reader semantics; no focus ring for chrome widgets; tab underline animates unconditionally (`animate_value_with_time`); no zoom; 22px hit targets; palette chosen for vibe rather than contrast math.

---

## 17. Cognitive Load

Low density, high noise. On an idle screen: tab bar, breadcrumb, sidebar, a six-icon strip of which only Terminal and Find-in-Files are wired (rest `#[allow(dead_code)]`), a six-field read-only status bar, four command surfaces, and nothing that can be right-clicked. Two of six visible controls are decorative.

---

## 18. Real Developer Workflows

- **Open and understand a project:** empty tree → no open-folder → `:e src/main.rs` → one file, empty symbol index. Dead at step 2.
- **Fix a bug:** no go-to-def, no references, no LSP affordances rendered in the GUI (not hover); `:build` → `✗` in a void, compiler error invisible. Dead at the build.
- **Large refactor:** no rename surface. Unstartable.
- **Debugging:** stale adapter, step with no frames/variables. Theater.
- **Git:** staging/diff/commit work via `:vc`; best workflow in the app, colon-only and mouse-barren.

---

## 19. vs Modern IDEs

Better: nothing in the UI; only the editing core (rope, undo, vim fidelity) and a coherent theme. Equivalent: vim motions, buffer/tab management, a sidebar, a filtering palette. Worse: every navigation surface, every tooling surface, all error handling, the six-band chrome stack. Missing: project open, menu bar, diff view, LSP affordances, completion visibility, debugger view, task/output panel, toasts, tab overflow, context menus, middle-click.

---

## 20. Anti-Patterns

Modals for a routine action ✓ (search); no menus but four hidden command surfaces ✓; icon-only unexplained controls ✓ (`⊟ ⌖`); permanent non-dismissible panels ✓ (breadcrumb, tool strip, status); redundant navigation ✓ (three file finders); three option stores ✓; hidden state ✓ (colon mode invisible); destructive actions unguarded ✓ (`:q`, dired `x`); accidental behavior ✓ (auto-save); UI requiring reading source ✓.

---

## 21. Brutal Scoring

| Category | Score |
|---|---:|
| First impression | **2** |
| Visual design | **5** |
| Editor | **6** |
| Navigation | **3** |
| Information architecture | **3** |
| Keyboard UX | **5** |
| Mouse UX | **3** |
| Project explorer | **2** |
| Tabs/windows | **5** |
| Search | **3** |
| Build/run | **2** |
| Debugging | **1** |
| Error UX | **1** |
| Git | **5** |
| Settings | **3** |
| Responsiveness | **5** |
| Accessibility | **1** |
| Cognitive load | **3** |
| Consistency | **3** |
| **Overall developer experience** | **3** |

---

## 22–23. Findings by Severity

### [P0] `:q` and the window close destroy unsaved work
**Location:** `quit`/`force-quit` (`src/kernel/command/builtin/misc.rs:17-29`), `gui.rs` ctrl-q, window close
**Problem:** No dirty-buffer check or confirmation. Live-tested: modified file → `:q` → exit, work gone.
**Why it matters:** Data loss is the only unforgivable editor sin.
**Reproduction:** Edit a file, `:q`.
**Expected:** Refuse with "No write since last change" or a save/discard dialog.
**Actual:** Silent exit.
**Recommendation:** `quit` checks `Editor::any_dirty()`; GUI window-close routes through the same path with a dialog.
**Priority:** P0

### [P0] Silent 30-second auto-save rewrites your files
**Location:** `main.rs:168-202`
**Why it matters:** Uninvited writes; clears dirty state; falsifies git status; writes half-typed code.
**Recommendation:** Explicit opt-in option, default off.
**Priority:** P0

### [P0] The TUI is dead but documented as preserved
**Location:** `main.rs:19,28`; `TuiRenderer` (no non-test callers); GUI-OVERHAUL Phase 10; getting-started.md
**Why it matters:** `--tui` silently does nothing; docs lie.
**Recommendation:** Restore a TUI entry path or stop claiming it exists and delete the dead module.
**Priority:** P0

### [P0] No way to open a project from the UI
**Location:** `gui.rs` (no menu/toolbar/dialog); nothing calls `project/index-files` (`project_api.rs:111-130`)
**Why it matters:** Sidebar, goto-file, and file navigation are empty until you evaluate an undocumented function.
**Recommendation:** "Open Folder" dialog setting project root, indexing async, bound to a menu/key and shown on first launch.
**Priority:** P0

### [P0] GUI has no completion popup and no error display
**Location:** `frame.rs:145-147` (popup TUI-only); zero error surfaces in `src/kernel/render/`
**Why it matters:** Completion is blind; failed actions are silent.
**Recommendation:** Render completion in the GUI pane; add status/log/toast surface; stop discarding `execute_command` results.
**Priority:** P0

### [P1] Symbol index is never populated
**Location:** `semantic_api.rs:111-131`; no caller in `builtins/`
**Why it matters:** Goto-Symbol and symbol breadcrumbs are guaranteed empty.
**Recommendation:** Run indexing on open/save, async.
**Priority:** P1

### [P1] Find-in-files results not clickable; search dialog has no next/prev
**Location:** `tool_window.rs:287-298`; `gui_search_dialog.rs:123-133`
**Why it matters:** Search is navigation; neither result list lets you navigate.
**Recommendation:** Click/Enter opens file at line; add n/N stepping.
**Priority:** P1

### [P1] Keys mean different things by backend and within a layer
**Location:** `vim_keybindings.janet:75,88` (ctrl-d double bind); `:197` vs `gui.rs:162` (ctrl-p); `ecosystem.janet:99` vs `gui.rs:162` (ctrl-o)
**Why it matters:** Silent behavioral forks destroy muscle memory.
**Recommendation:** One keybinding source of truth consulted after GUI dialogs; lint rejects duplicate binds.
**Priority:** P1

### [P1] Build/task/debug output is invisible
**Location:** `task.janet:40-55`; `tool_window.rs:185-210`
**Why it matters:** The dev loop's feedback is structurally discarded.
**Recommendation:** Route task output into a real panel with errors as clickable quickfix entries.
**Priority:** P1

### [P2] Sidebar has no operations and opens on double-click
**Location:** `gui_sidebar.rs`
**Recommendation:** Right-click menu (open, rename, new, delete→trash, copy path); single-select + Enter-open.
**Priority:** P2

### [P2-P3] Modeline/position is 0-based `Col 0`; docs claim a REPL that doesn't exist; unused icon constants; `diag-warn` off-palette
**Priority:** P2/P3

---

## 24. Worst 10 Problems, Ranked

1. **Data loss at exit (`:q` / close).** Daily risk, catastrophic cost, trivial to fix.
2. **Silent auto-save.** Every editing session; makes the editor untrustworthy.
3. **No project-open flow + empty tree.** Hourly friction; the #1 first-use wall.
4. **No error surface; errors discarded.** Invisible failures teach users to ignore the editor.
5. **Invisible GUI completion.** Every insert-mode completion attempt is blind.
6. **Symbol index never runs.** Nav feature that never yields results; cheap fix.
7. **Keybinding fractures (ctrl-d dup; ctrl-p/ctrl-o hijack).** Per-session confusion.
8. **Dead documented TUI.** Repo hygiene + user trust.
9. **Unclickable find results / no next-prev.** Search tasks fall back to grep in terminal.
10. **No destructive-action protection in dired/vc.** Delete files permanently.

---

## 25. Death by a Thousand Cuts

Double-click to open every file; a `ctrl-d` that spawns phantom cursors; a palette that omits `open-file` and shows no descriptions; a restart-free manual string edit for settings; a `*Messages*` tab you never asked for; a breadcrumb that reveals a directory instead of navigating; separators that are colored `|` glyphs instead of vertical rules; a fully non-interactive status bar; icon strips whose glyphs mean nothing. The aggregate cost is not seconds — it is a persistent low-grade signal that the editor is unfinished, which erodes trust faster than any single bug.

---

## 26. Design Philosophy

Two philosophies fight for the same shell. **A (inherited):** an Emacs-shaped, vim-textured, Janet-extensible TUI — everything is keys and code. **B (the GUI overhaul):** a JetBrains-shaped native IDE — chrome, dialogs, panels, discoverability via widgets. The overhaul bolted B's chrome onto A's behaviors without resolving the contradiction: colon mode is invisible and undocumented in the GUI; dialogs and colon commands coexist for the same feature; configuration is proudly code-only; every modern-IDE affordance (menu, context menu, error toast, project picker, completion popup) was never translated. The philosophy is not wrong — it is two unmerged halves.

---

## 27. Brutal Verdict

### 3/10

**Would I use this every day?** No — not for any workflow. It would lose my work, rewrite my files, and hide both failures.

**Biggest strength:** a coherent, native-speed editing core with real vim fidelity and a genuinely tasteful, consistent Catppuccin theme. The engine can edit; it cannot be a product around it.

**Biggest weakness:** the feedback loop is broken in both directions — you cannot make it *show* your project, completions, build output, or errors, and you cannot make it *stop* doing the one thing it shouldn't (saving, quitting). An editor you cannot believe.

**Biggest architectural UX mistake:** treating "JetBrains-style chrome" and "vim/colon internals" as compatible layers rendered side-by-side, instead of giving the GUI one surface model. Two UIs for one product, each half-finished, errors disposed into the seam between them.

**Biggest opportunity:** a first-run "Open Folder" that indexes async, a single output/error panel that build+task+debug+completion funnel into, and one context menu. Those three changes move this from tech demo to an editor someone could work in.

**Delete:** the dead TUI renderer and `--tui` parsing; unused icon constants; the `*Messages*`-on-launch; the auto-save loop (or its implicit enablement); the fake first-launch ASCII mockup.

**Redesign:** colon mode as the GUI's invisible command line (minibuffer or delete in favor of the palette); the search dialog; the sidebar; Settings.

**Add:** project folder picker; completion popup; error/toast surface; clickable task output with quickfix; real debug pane or none; middle-click close and context menus; persistent symbol indexing. Nothing beyond — most of the missing value is reconnecting surfaces that already exist.

**Simplify:** four overlapping command surfaces → two (editor keymap + one command/UI layer); six chrome bands → two; three option stores → one typed, searchable store.

---

**Final word.** Magma's engine room is real engineering. Everything a developer touches to reach it is an unfinished construction site, gated by silent failures and guarded against nothing. It is not a matter of polish — it is a matter of deciding once whether this is a TUI person's editor with a coat of GUI paint, or an IDE. Right now it is both, which makes it neither. The one fix that must land before any feature work: **make `:q` and the window close refuse to destroy unsaved changes, and delete or visibly disable the auto-saver.** Everything else can wait for the philosophy debate. Data loss cannot.