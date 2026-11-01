# Magma (Slint shell) — Brutal GUI/UX Review

**Reviewed:** 2026-11-01 · HEAD `4fa3e9e` · `target/debug/magma-slint` · macOS, 1280×760 window
**Method.** Drove the running binary: native menu bar, toolbar, project tree, tabs, docks, settings, dialogs, build/test/run. Captured the window with `screencapture`, read it back with OCR (text mode + per-word coordinates) and pixel statistics; the model doing this review cannot view images, so every visual claim below is either measured from the captured pixels or read from the Slint source with line citations. Where the shipped UI and the docs disagree, the UI wins.

**Evidence summary.** This is no longer the egui shell the previous review (3/10) described. There is a real native macOS menu bar, a working *Open Folder*, a populated project tree, a categorized Settings modal, a completion popup with a documentation pane, a bottom dock with Output/Problems/Find/Git/Terminal, and a quit guard that actually stops you. That is genuine progress and it is credited below.

It is also, in the two places developers spend most of their time — **search** and **navigation** — a facade. All four navigation commands (`Find in Buffers`, `Go to File`, `Go to Symbol`, `Run Command`) open the *same* 420×160 text box, and pressing Enter **executes whatever you typed as an editor command**. There is no find/replace in the editor at all. The Find dock cannot search. The welcome card advertises three shortcuts that do not exist. Build and Test are the same button, and in this repository both run `make` and fail.

---

## 1. The IDE as a Product

Launch is now believable: a welcome card in the editor area, a real menu bar (`File Edit View Navigate Run`), a 6-icon toolbar, a project rail, and a status bar. Opening a folder works, the tree fills, clicking a file opens a tab, and the `*make*` buffer appears in the dock with output. Closing with unsaved changes raises a Save/Discard dialog instead of destroying work. These are the things the previous review said were missing, and they are no longer missing.

Then you try to use it as an editor.

- `File` has **Open Folder, Save, Save All, Close Buffer, Quit**. No New File, no Open File, no Save As, no Close Window — and recent projects exist only on the welcome card, not in the menu. A file can otherwise only be opened by finding it in the tree or by typing `open-file <path>` into a box labelled *Run Command*.
- `Edit` has **Undo, Redo, Find in Buffers**. No Cut, Copy, Paste, Select All, Find, Replace, or Go to Line. On macOS, an Edit menu without Copy is not a stylistic choice; it is a broken contract.
- The welcome card's "cheat sheet" — the only onboarding surface in the product — advertises `ctrl-shift-o go to symbol`, `ctrl-shift-f find`, `ctrl-q quit` (`ui/components/welcome.slint:92`). **None of those bindings exist** (`builtins/editor_keybindings.janet:113-136`).
- There is no command palette. `Run Command…` is the generic execution dialog, which means discovering a command requires reading the source.

The product is therefore in a strange middle state: the shell is real, the engine is real, and the connective tissue between them is either absent or mislabelled. A new user's first ten minutes are: open folder ✓, click file ✓, press the advertised find key ✗, open the Find dock ✗, open "Find in Buffers" and get a command prompt ✗.

**Verdict for this section:** a credible IDE shell wrapped around an editor you cannot search.

---

## 2. Visual Design

The token layer is the strongest part of the UI. `theme.slint` defines a real scale — `font-ui` 12px, `font-small` 11px, 30px tab/menu bars, 22px breadcrumb, 24px status bar, 32px tool rail, 240px sidebar, 24px list rows, 3px radius — and the shipped palettes (IntelliJ light/dark, Catppuccin, Monokai, Ayu, Dracula, Tokyo Night, Noctis) are applied consistently to chrome, docks, dialogs, and overlays. Default light is a calm `#EBECEF`; the accent is used sparingly; the active tab has a restrained underline. It reads like an IDE, not like a demo.

The problems are not taste, they are signals:

- **Six horizontal bands before text**: toolbar, tab strip, breadcrumb, editor, status bar, bottom dock header. The breadcrumb is 22px of pure decoration (see §4), so the effective chrome-to-content ratio at 760px tall is poor.
- **Amber everywhere.** Non-error status messages are tinted `Theme.warning` — "Magma UI ready", the current file, task completion. A warning colour that fires constantly trains users to ignore the one channel that carries failures.
- **Three status pills look interactive and are not.** `status-bar.slint:38-40` declares `file-clicked`, `branch-clicked`, `position-clicked`; `:54/:69/:86` fire them. `app.slint` never connects a single one. On screen they are indistinguishable from working controls.
- **Internal names leak into the UI.** The tab and dock header for a build read `*make*(# make)` — buffer name plus command in parentheses, rendered verbatim. Editors show `core-redesign.md`, not `core-redesign.md*` with an emacs-style buffer string.
- **Icon-only toolbar, no tooltips.** Six 26×26 Nerd Font glyphs (run, debug, stop, build, test, settings) with the conventional green/red colouring. Run and Debug are discoverable by shape; Build and Test are two different-looking buttons that do exactly the same thing (§10), so the iconography is actively misleading.
- **No zoom.** Font family and size are adjustable in Settings, but there is no per-window zoom, no Cmd+= / Cmd+-, and no way to scale the *chrome* independently of the editor.
- **Empty states are generic.** The Find dock's empty state is a large glyph with "No results / Run a search to fill this list." — instructions for an action the UI does not offer.

**Score-driving judgement:** the palette and spacing earn a 6. The dead affordances, the amber monotone, and the raw buffer names keep it out of 7+ territory.

---

## 3. Editor Experience

What is real: rope buffers, undo/redo, multi-cursor, folds (tree-sitter queries), semantic tokens, dead-code dimming, a line-number gutter, an optional minimap, an IME bridge, and a completion popup (560px, list on the left, documentation pane on the right) that does render while typing. Hover cards and quickfix overlays exist. `ctrl-]` is bound to `goto-definition` and the LSP path is wired.

What is missing is the thing every developer does dozens of times an hour:

- **No find in buffer.** `Cmd+F` / `Ctrl+F` does nothing. There is no find bar, no incremental highlight, no replace, no "find in selection", no regex toggle. `editor_keybindings.janet` binds `ctrl-s/z/shift-z/w`, `ctrl-c/x/v/a`, `ctrl-p/n/space`, `ctrl-]`, `ctrl-alt-return`, but nothing for find or replace.
- **No find in project.** See §4/§5: the dock tab labelled "Find in Files" is a list with no input.
- **The editor surface is a bitmap blit.** `editor-view.slint:37-48` renders the pane as an `Image` with `image-rendering: pixelated`, gutter and minimap likewise (`:51-58`) or as separate images. The invisible `TextInput` at `:151` exists only to capture IME composition. Consequences: no native text services (system Find, dictionary, dictation, text replacement), no text selection by VoiceOver, no accessibility tree for the document, and no way for the OS to spell-check or expose the caret rect to assistive tech.
- **Completion is a word list, not a language model.** Observed content was buffer-word candidates (`maintain`, `maintains`, `maintained`). There is no signature help, no parameter hints, no snippet expansion UI, no ranking controls.
- **No split or grid.** No `split` action exists anywhere in the Slint shell; the editor is one pane, forever. Reading a header while editing its implementation requires two windows.

The editing core would be respectable in 2010 and is competitive on raw speed today. The *affordances* around it are where it loses.

---

## 4. Navigation

This is the section that decides the review.

Four menu items, four different labels, one implementation. `actions/settings.rs:257-266` maps `dialog:find`, `dialog:goto-file`, `dialog:goto-symbol`, and `dialog:run-command` to the same code path: set a title, clear the input, show `DialogView`. `dialog:ok` at `:271` then does:

```rust
execute_command(&mut s.editor, &cmd, &HashMap::new())
```

So:

- **Go to File** cannot open a file by name. Typing `main` produces `Unknown command: main` (observed live). To open a file you must already have it in the tree, or know the command syntax `open-file <path>`.
- **Go to Symbol** cannot list symbols. It runs your text as a command.
- **Find in Buffers** cannot list buffers. It runs your text as a command.
- **Run Command** is the only one whose label matches its behaviour.

This is not "unpolished". It is four navigation surfaces that are all a mislabelled command prompt, and it makes the menu bar itself untrustworthy: every item in `Navigate` is a lie except the tab-cycling pair.

The rest of navigation:

- **Breadcrumb is inert.** `breadcrumb.slint` is 31 lines of `Text` inside a `HorizontalLayout`, with no `TouchArea` and no callbacks. The path segments look navigable and do nothing.
- **No palette.** Search-based command discovery does not exist; `ctrl-p` is bound to `completion-prev` (`editor_keybindings.janet:121`), and the source comment notes it is "no longer GUI-reserved", so there is no reserved modifier left for a palette.
- **No recent files, no "reopen closed tab", no quick-switch between last two tabs.**
- **No go-to-line.** A basic editor has had `Cmd+L` since the 1980s; here there is neither binding nor dialog.

---

## 5. Information Architecture

The app now has three parallel column systems and two different notions of "tool window":

- **Left rail**: Project, Commit, Debugger (`View` menu, `rail:0..2`).
- **Bottom dock**: Terminal, Find in Files, Output, Problems, Git (five tabs, indices 0–4).
- **Menus**: five top-level menus whose items partially duplicate the rails (`Project`/`Commit`/`Debugger` in View; `Refresh Version Control` in Run).

The duplication is not just structural, it is semantic: **Find and Problems render the same data**. `projection/find_rows` (`projection/mod.rs:321`) maps `ed.quickfix_list`; `problem_rows` (`:339`) maps `ed.quickfix_list`. Two dock tabs, one dataset, different empty states. The Find tab's rows are `file:line` plus the diagnostic message — i.e. compiler diagnostics presented as search results.

Likewise, project search, buffer search, and symbol search are three names for one command prompt (§4), and the tree, the (dead) breadcrumb, and Go to File are three names for "find a file" with three different data sources.

There is no hierarchy that explains which surface owns what. Nothing in the UI tells you that Problems comes from the LSP, that Find is a duplicate, that Terminal is read-only, or that the Commit rail and the Git dock tab are different views of the same subsystem.

---

## 6. Keyboard UX

The keymap is a real, non-modal map with sensible defaults — `ctrl-s`, `ctrl-z`, `ctrl-shift-z`, `ctrl-w`, `ctrl-c/x/v/a`, `ctrl-p/n/space` for completion, `ctrl-]` for definition, `ctrl-alt-return`, `shift-K`, and `ctrl`←/→ for horizontal pan. `input.rs:12` correctly folds macOS `meta` into `ctrl`, so `Cmd+C` reaches the buffer copy command.

Then the map runs out exactly where modern editing begins:

- No `Cmd+F`, no `Cmd+H`, no `Cmd+G`/`Cmd+Shift+G`, no `Cmd+L`, no `Cmd+P`, no `Cmd+Shift+P`, no `Cmd+B`/`Cmd+J`/`Cmd+1..9`, no `Cmd+W` semantics (only `ctrl-w` → `close-buffer`), no Escape handling outside a few views.
- **Dialogs are keyboard-dead.** `dialog-view.slint` has no `FocusScope`, no `has-focus` binding on its `TextInput`, and no key handler for Escape. Opening "Go to File" leaves every keystroke in the editor behind the modal until you click the field. Escape does not close it. There is no focus trap: Tab can wander into the dimmed UI.
- **Settings and New Project do it right** (`settings-view.slint:187`, `new-project-view.slint:113` both use `FocusScope`), which makes the dialog regression a bug, not a philosophy.
- **The only teaching surface lies** (`welcome.slint:92`), so the user's mental model of the keyboard is wrong from minute one.
- **No keybinding editor.** The keymap is Janet code; discovering or rebinding a key means reading `builtins/editor_keybindings.janet`.

---

## 7. Mouse UX

- **No context menu anywhere.** Not in the editor (right-click is dispatched as a cursor move, with no menu), not on tabs, not on the tree, not on dock rows, not on the status bar. On a desktop IDE this is the single largest discoverability hole: right-click is where users look for rename, reveal, close others, copy path, stage hunk, and "open in terminal".
- **Dead click targets.** The status bar's file/branch/position pills have hover states, cursor semantics and callbacks — and no listeners. Clicking them does nothing, twice in a row, forever.
- **Small targets.** Tree rows 24px, dock rows 22px, toolbar buttons 26×26, close affordance 11px of glyph. Fine for a young user on a trackpad, poor for everyone else.
- **Accidental opens.** The project tree opens a file on single click. There is no selection-then-open convention, so browsing a directory with the mouse means opening every file you touch.
- **Unlabelled tree header buttons.** `↻` (refresh) and `−` (collapse) are 16px glyphs with no tooltips and no text.
- **Find dock rows are real links to diagnostics.** Clicking one opens the file at that line (`app.slint:689`), which is exactly why the tab passes for search until you notice there is no query field and the rows are compiler problems.

---

## 8. Project Explorer

This is the most improved area. Open Folder works, the tree indexes, directories expand, files open, and the tree reflects the real filesystem.

What is still missing defines a file manager, not an IDE:

- **No operations**: no new file, new folder, rename, delete (trash), duplicate, copy path, reveal in Finder, or open in terminal.
- **No keyboard**: the tree is not focusable; there is no arrow-key traversal, no Enter-to-open, no type-ahead filter.
- **No filter/search box**, no sort control, no show-hidden toggle.
- **No VCS decoration**: no modified/added/ignored colouring, no branch awareness, although a full VCS subsystem and gutter signs exist.
- **No drag and drop**, no multi-select, no file-type icons beyond a generic glyph pair.
- **Refresh is manual** via the unlabelled `↻`; there is no file watcher evidenced in the tree, so external changes can leave the tree stale until you press it.

---

## 9. Tabs and Windows

The tab strip is competent at small N: active underline, modified dot, close `×` on hover/active, and a readable 30px bar. Then:

- **No overflow.** `tab-strip.slint:118` is a `HorizontalLayout` with fixed per-tab minimum width and no `Flickable`, no scroll, no "…" overflow menu, no shrink-to-fit below the minimum. Past the window width, tabs are clipped with no way to reach them.
- **No context menu**: no Close, Close Others, Close All, Close to the Right, Copy Path, Reveal, Pin, or Split.
- **No pinned or preview tabs**, no tab groups, no tab reordering affordance visible.
- **No tooltip with the full path** — two `mod.rs` tabs are indistinguishable.
- **Titles are internal strings**: `*make*(# make)`.
- **Closing the last tab** leaves an empty pane with no "open a file" affordance beyond the tree.
- **No window management**: no split, no grid, no Zen mode, no full-screen-specific layout; one document at a time.

---

## 10. Terminal / Build / Run / Debug

Real progress: tasks now land in the Output dock with a `*make*` buffer, and Run detects and launches a run configuration. The dock header shows the task name and exit status.

The dev loop still breaks at the first step:

- **Build and Test are the same command.** `app.slint:392-393`: `build => { root.command("make"); }` and `test => { root.command("make"); }`. In this Cargo repository both produce `make: *** No targets specified and no makefile found. Stop.` and `*make* #2: exit code 2`. The toolbar is project-type-unaware: no Cargo, npm, Gradle, CMake, or pytest detection, no task configuration UI.
- **Build errors are not actionable.** Output is a text blob; compiler diagnostics are not parsed into the Problems list unless an LSP supplied them, and output lines are not clickable. The loop is: run build → read raw text → manually navigate.
- **Test is indistinguishable from build**, so there is no test result model at all: no pass/fail tree, no per-test timing, no re-run failed.
- **Terminal is read-only.** The dock's Terminal view renders a `Text` inside a `ScrollView`; there is no `TextInput` anywhere in `bottom-dock.slint`, so you cannot type into the shell you are watching.
- **Run is fragile by ordering.** Pressing Run before detection finishes surfaces the raw string `no run configuration named 'build'`, with no spinner and no retry.
- **Debug** has a rail and a dock panel, but the visible surface is still shallow: no launch-config editor, no breakpoint list with conditions, no watch expressions, no call-stack/variable tree with inline values, no console. A debugger you cannot configure is a gutter toggle with extra chrome.

---

## 11. Error UX

There is exactly one error surface: a string in the status bar. Everything routes there, in the same amber as success messages.

- Failed commands surface raw internal text: `Unknown command: main`, `no run configuration named 'build'`, `LSP initialize timed out for rust`. No sentence explains what to do next, and there is no link to a log, a retry, or the offending file.
- The quit guard's message leaks implementation: `No write since last change: core-redesign.md (use force-quit to override)` — a Vim `:q` string shown in a GUI modal to a user who just clicked the window's close button.
- No toasts, no notification centre, no error badge on the Problems tab, no inline squiggle explanation visible in the bitmap pane, no "copy error", no deduplication.
- **No log viewer.** `*Messages*` may exist, but nothing in the chrome points to it, and the status string is transient: it is replaced by the next action.
- The LSP timing out for Rust is reported as a status line, not as a degraded-mode banner, so the user keeps typing into a buffer with no intelligence and no explanation.

---

## 12. Git

The strongest subsystem after the editing core: commit panel with staging, gutter signs, git history dock, branch in the status bar, and `Refresh Version Control` in the Run menu.

The UX around it is thin:

- **The branch pill is dead** (same unwired callback as the file/position pills). The most natural place to switch branches does nothing.
- **No branch switcher UI**, no stash UI, no conflict resolution surface in the reviewed paths, no blame view reachable from the editor, no inline diff peek.
- **Commit and Git History are separate columns** (rail vs dock) with no cross-link; selecting a commit does not obviously open its diff.
- **No VCS decoration in the project tree**, so you cannot see what changed without opening the commit rail.
- **No undo for discard** observed; destructive VCS actions inherit the same unguarded style as the rest of the shell.

---

## 13. Settings

Big improvement, and one of the few places where the design is deliberate:

- Six categories (Appearance, Editing, Behavior, Plugins, Agent, Advanced), a working "Search settings" field, typed controls derived from the option registry, per-row reset, and "Changes apply immediately" with no Save button.
- Fonts are properly modelled: app font family and size, editor font family and size, ligatures, with a font list collected from the system (`actions/settings.rs:240-246`).
- Escape closes it, and it is centred at a comfortable 760×540.

Remaining problems:

- **Raw option keys as row titles**: `gui-theme`, `breadcrumb-enabled`, `minimap-enabled`. The user sees the developer's registry name, not a human label, and there is no description of what the option changes.
- **No keybinding page.** The single most requested settings surface in an editor is absent; keys remain Janet code.
- **No per-project or per-language settings**, even though the kernel has buffer-local and project-local stores.
- **No "reset all"**, no import/export, no settings sync, no diff of changed-from-default.
- **Agent settings** ask for a provider, API key, and model in a desktop modal — with no indication of where the key is stored or how to delete it.

---

## 14. Responsiveness

Slint + Rust + a bitmap editor blit is the right architecture for latency, and the app feels immediate: typing, scrolling, tab switching, tree expansion, and settings toggles all respond without visible delay. Theme toggling is instant. Completion appears while typing without a perceptible wait.

Caveats:

- The editor's bitmap surface is re-rasterised on the UI thread; very large files and long wrapped lines are the obvious risk, and no virtualization strategy is visible in the view.
- Project indexing and symbol indexing are synchronous call paths in the kernel (`project/index-files`, `semantic/index-buffer`), so a large tree or a cold symbol index will block, even if it has not in this small repository.
- No progress affordance exists for long operations. The only feedback is a status string that can be overwritten by the next keystroke.

Responsiveness is a strength; *perceived* responsiveness on a 50k-file monorepo is unproven.

---

## 15. Interaction Consistency

The catalog of mismatches is long enough to be a section of its own:

1. **Four navigation verbs, one dialog** (§4), and the dialog executes instead of navigating.
2. **Find == Problems** (§5): two tabs, one dataset, two empty states.
3. **Build == Test** (§10): two toolbar buttons, one command.
4. **Escape works in Settings and New Project, not in Dialogs** (§6): the modal family has two different keyboard contracts.
5. **Backdrop click closes the command dialog and discards typed text**, while the Settings modal must be dismissed deliberately — one modal family, two dismissal semantics.
6. **Click conventions**: tree = single-click open; dock tabs = single click; status pills = single click that does nothing; toolbar = single click; menu = click-hold on macOS.
7. **Terminal appears twice** conceptually — as a session buffer and as a read-only dock view — with different capabilities and no indication which is which.
8. **Dirty state** is a dot in the tab, plus a status string, plus a quit dialog — three encodings of one fact, none of them authoritative in the tree.

---

## 16. Accessibility

The weakest area in the product.

- **The document has no accessibility tree.** It is an image. VoiceOver reads the window chrome and cannot read the code. This is not a bug to schedule; it excludes an entire population from using the editor at all.
- **No accessible names** on icon-only toolbar buttons, tree header glyphs, or tab close buttons.
- **No focus ring for chrome**; keyboard focus is visible only inside a few Slint `TextInput`s (`controls.slint:98`, `commit-panel.slint:403`, `agent-dock.slint:792`).
- **No keyboard path to the project tree**, the dock tabs, or the status bar.
- **Text at 11px** for dock rows, breadcrumb, and secondary labels; 24px rows; 26px buttons; an 11px glyph close target.
- **No high-contrast mode**, no reduced-motion handling (the tab underline animates unconditionally), no OS appearance following (theme is a manual option), no dynamic type.
- **No zoom** beyond editor/app font size.
- **Colour-only state**: modified dot, diagnostic severity, git gutter marks.

---

## 17. Cognitive Load

Better than the egui era, still noisy. On an idle screen the user simultaneously sees: menu bar, toolbar (6 icons), tab strip, breadcrumb (dead), project tree, editor, status bar (3 dead pills), and a 5-tab dock. That is eight independent chrome regions, two of which are non-interactive decoration and three of which are lies (Find tab, breadcrumb, status pills).

The information that *should* dominate — what changed, what is broken, where am I — is either absent (no tree decoration, no error badge) or buried (Problems duplicates Find; branch is a dead pill).

The keymap is invisible, the command space is invisible, and the one cheat sheet that exists is wrong. Cognitive load here is not "too many features"; it is "too many unlabelled surfaces with unknown contracts".

---

## 18. Real Developer Workflows

**Open and understand a project.** Open Folder ✓ → tree ✓ → click file ✓ → breadcrumb gives the path but cannot navigate ✗ → find a symbol: menu item opens a command prompt ✗ → read the file and scroll. *Passes at reading, fails at navigation.*

**Fix a bug from a stack trace.** Open file ✓ → jump to line: no go-to-line ✗ → find the function: no find ✗ → edit ✓ → build ✓ (runs) → read raw output, errors not clickable ✗ → repeat. *Fails at the second step.*

**Find where something is used.** Project search: Find tab has no input ✗; `Find in Buffers` runs your query as a command ✗; fall back to `grep` in an external terminal. *Fails.*

**Refactor across files.** No rename, no references, no multi-file find/replace, no project-wide edit preview. *Unstartable.*

**Run tests.** Test button runs `make`, which is also Build; no test model, no per-test results, no re-run failed. *Theatre.*

**Debug.** Rail exists; no launch configuration editor, no watch, no variables surface verified, no console. *Not a workflow yet.*

**Review and commit.** Commit rail with staging ✓, git history dock ✓, gutter signs ✓, branch switching ✗ (dead pill), inline diff ✗. *Half a workflow, and the half that exists is usable.*

**Two files side by side.** No split. *Impossible.*

---

## 19. vs Modern IDEs

| Dimension | Magma | VS Code / JetBrains / Zed |
|---|---|---|
| Find / replace | Absent | Cmd+F, incremental, regex, replace-all, in-selection |
| Project search | Dock tab with no input | Dedicated, streaming, replace, filters |
| Go to file / symbol | Command prompt that executes text | Fuzzy palette, ranked, preview |
| Command palette | Absent | Universal |
| Context menus | None | Everywhere |
| Split editing | None | Arbitrary grids |
| Build/test | One `make` for both | Task-aware, per-project, structured results |
| Terminal | Read-only view | Real PTY, multiple sessions |
| Debug | Facade | Full DAP UI |
| Settings | Categorised, typed, raw keys | Human labels, searchable, per-scope |
| Accessibility | None for the document | Native text surfaces |
| Latency | Excellent | Good to excellent |

Magma wins on latency and on the coherence of its theme. It loses on every interaction contract that makes an IDE an IDE.

---

## 20. Anti-Patterns

Present, with evidence:

- **Fake affordances** ✓ (three status pills with hover states and no listeners).
- **Mislabelled commands** ✓ (`Find in Buffers` executes input; `Go to File` cannot open files; `Test` builds).
- **Duplicate surfaces for one dataset** ✓ (Find = Problems; three file finders).
- **Modal for routine work** ✓ (navigation, the most frequent action, is modal).
- **Modal without focus management** ✓ (no autofocus, no Escape, no trap).
- **Destructive action behind a search box** ✓ (typing `force-quit` into "Find in Buffers" and pressing Enter discards unsaved work without the quit guard).
- **Internal representations exposed** ✓ (`*make*(# make)`, `gui-theme`, `use force-quit to override`).
- **Status colour used as decoration** ✓ (amber for success).
- **Instructions for nonexistent actions** ✓ ("Run a search to fill this list").
- **Onboarding that teaches wrong shortcuts** ✓ (`welcome.slint:92`).
- **Feature theatre** ✓ (Test button, debug panel, breadcrumb).
- **Dead chrome** ✓ (breadcrumb, pills, unlabelled glyph buttons).

Absent, thankfully: fake progress bars, gratuitous animation, dark-pattern upsells, AI chat bolted onto every surface.

---

## 21. Brutal Scoring

| Category | Score |
|---|---:|
| First impression | **4** |
| Visual design | **6** |
| Editor | **5** |
| Navigation | **2** |
| Information architecture | **4** |
| Keyboard UX | **3** |
| Mouse UX | **3** |
| Project explorer | **4** |
| Tabs/windows | **4** |
| Search | **1** |
| Build/run | **3** |
| Debugging | **2** |
| Error UX | **2** |
| Git | **5** |
| Settings | **5** |
| Responsiveness | **7** |
| Accessibility | **1** |
| Cognitive load | **4** |
| Consistency | **3** |
| **Overall developer experience** | **4** |

---

## 22–23. Findings by Severity

### [P0] Every navigation command is a command prompt that executes what you type
**Location:** `crates/slint-shell/src/actions/settings.rs:257-266` (four commands share one dialog), `:271-284` (`dialog:ok` calls `execute_command`), `ui/views/dialog-view.slint:55-63`
**Problem:** `Find in Buffers…`, `Go to File…`, `Go to Symbol…`, and `Run Command…` all open the same 420×160 input. Enter runs the text as an editor command. Typing a file name yields `Unknown command: main`; typing a buffer name does the same. Typing `quit` closes the app, and typing `force-quit` discards unsaved buffers without passing through the quit guard (`misc.rs` registers both with no arguments; only `quit` checks dirty state).
**Why it matters:** The three navigation verbs a developer uses hundreds of times a day do not navigate, and the box will happily execute session-ending commands typed by someone who believes they are searching.
**Reproduction:** `Navigate » Go to File…` → type `main` → Enter. Watch the status bar.
**Expected:** A filterable list of files (and, for symbols, symbols), Enter opens, Escape closes.
**Actual:** `Unknown command: main`.
**Recommendation:** Split the four commands. Go to File/Symbol need real fuzzy pickers over indexed data. `Run Command` should be a palette that lists commands, not a blind text field. Never route free text from a *search* surface into `execute_command`.
**Priority:** P0

### [P0] There is no find or replace — in the buffer or the project
**Location:** `builtins/editor_keybindings.janet:113-136` (no find bindings); `ui/components/bottom-dock.slint:532,636` (Find view has no input); `crates/slint-shell/src/projection/mod.rs:321-333` (`find_rows` = `ed.quickfix_list`)
**Problem:** `Cmd+F` does nothing. The "Find in Files" dock tab contains only a results list, and that list is the LSP quickfix list — the same data the Problems tab shows. The empty state says "Run a search to fill this list" but there is no search control anywhere.
**Why it matters:** Search is the primary navigation primitive of a code editor. Without it, every task that spans more than the visible file forces the user out of the IDE.
**Reproduction:** Focus a buffer, press `Cmd+F`; then open the Find dock.
**Expected:** Find bar with incremental highlight; project search with query input, results grouped by file, enter/click to open.
**Actual:** Nothing, and a results list that is not search results.
**Recommendation:** Implement buffer find/replace first (it is self-contained), then a streaming project search with ripgrep semantics and a real input in the dock. Delete the `find_rows` alias to `quickfix_list` and give the tab its own model.
**Priority:** P0

### [P1] The onboarding card teaches three shortcuts that do not exist
**Location:** `ui/components/welcome.slint:92` vs `builtins/editor_keybindings.janet:113-136`
**Problem:** `ctrl-shift-o go to symbol`, `ctrl-shift-f find`, `ctrl-q quit` are advertised; none are bound. `ctrl-o` is actually `mark-ring-jump-back`, and `ctrl-p` is `completion-prev`.
**Why it matters:** The only in-product teaching surface is wrong, so the user's first hypothesis about the keyboard is false, and trust in every other hint drops with it.
**Reproduction:** Fresh launch, read the welcome card, press each key.
**Expected:** The card is generated from the live keymap (or removed).
**Actual:** Three dead shortcuts, one of which (`ctrl-f`) is the most important key in the app.
**Recommendation:** Build the cheat sheet from keymap data at render time; add a test that every advertised chord resolves.
**Priority:** P1

### [P1] Build and Test are the same button and both are project-type-blind
**Location:** `ui/app.slint:392-393`
**Problem:** Both toolbar buttons dispatch `command("make")`. In this Cargo repository both produce `make: *** No targets specified and no makefile found. Stop.` and `*make* #2: exit code 2`.
**Why it matters:** A green "Test" button that does not test is worse than no button: it produces false confidence and noise in the output panel.
**Reproduction:** Click Build, then Test, in any non-Make project.
**Expected:** Detected task provider (Cargo, npm, Gradle, CMake, pytest) with distinct build/test/run commands, or a task picker.
**Actual:** `make` twice.
**Recommendation:** Add a task-detection layer keyed on manifest files; disable Test when no test task exists rather than silently aliasing it.
**Priority:** P1

### [P1] Build/test output is a text blob with no actionable errors
**Location:** `crates/slint-shell/src/projection/mod.rs:334-341` (`output` = last task text), `ui/components/bottom-dock.slint` (Output text view)
**Problem:** Output renders raw stdout/stderr; compiler diagnostics are not parsed, output lines are not clickable, and the Problems tab is fed by the LSP only.
**Why it matters:** The edit→build→fix loop requires jumping from an error to its line. Without that, the build panel is a log viewer and the developer is back to manual scrolling.
**Reproduction:** Introduce a syntax error, press Build, click any error line.
**Expected:** Clickable `file:line:col` rows feeding Problems, with severity icons.
**Actual:** Unclickable text.
**Recommendation:** Parse common diagnostic formats into quickfix entries on task completion; make Output lines `TouchArea`s that open the file at the position.
**Priority:** P1

### [P1] Command dialogs have no keyboard contract
**Location:** `ui/views/dialog-view.slint` (no `FocusScope`, no `has-focus`, no Escape handler; backdrop `TouchArea` at `:16-18` closes and discards input)
**Problem:** The input is not focused on open, Escape does not close it, there is no focus trap, and a stray click outside dismisses the modal and loses typed text. Settings and New Project solve all of this correctly.
**Why it matters:** A modal that requires a mouse to start typing and a mouse to leave is a workflow interrupt on every use — and these modals are the only navigation UI.
**Reproduction:** Open `Navigate » Go to File…` and type without clicking.
**Expected:** Autofocus, Escape closes, Tab is trapped, outside click does not discard typed text silently.
**Actual:** Keystrokes go to the editor behind the modal.
**Recommendation:** Extract one `Modal` component with focus scope, Escape handling, focus trap, and consistent dismissal semantics; reuse it in dialogs and Settings.
**Priority:** P1

### [P1] Status-bar pills are clickable and dead
**Location:** `ui/components/status-bar.slint:38-40,54,69,86` (callbacks declared and fired); `ui/app.slint` (no connection)
**Problem:** File, branch, and position pills have hover states and fire callbacks that nothing listens to. A branch pill that cannot switch branches is a promise the product does not keep.
**Why it matters:** Dead controls in persistent chrome teach users that the UI is unreliable; they also waste the best real estate for branch switching and go-to-line.
**Reproduction:** Click the branch name, the file name, or `Ln x, Col y`.
**Expected:** Branch picker; copy/reveal path; go-to-line prompt (which would also fix the missing `Cmd+L`).
**Actual:** Nothing.
**Recommendation:** Wire the three callbacks, or remove hover affordance and pill styling until they do something.
**Priority:** P1

### [P1] Tabs have no overflow and no context actions
**Location:** `ui/components/tab-strip.slint:118` (`HorizontalLayout`, no scroll/overflow), fixed min tab width
**Problem:** Past the window width, tabs are clipped with no overflow menu, no scrolling, and no way to reach hidden buffers; there is no tab context menu at all.
**Why it matters:** A normal working set is 15–30 files. The tab strip stops working at ~8, and the only recovery is closing tabs you still need.
**Reproduction:** Open files until the strip fills, then try to reach the first tab.
**Expected:** Shrink-to-fit down to a floor, then scroll or an overflow list; right-click for Close/Close Others/Copy Path.
**Actual:** Clipped tabs.
**Recommendation:** Add a `Flickable` strip with wheel scrolling plus an overflow chevron; add a tab context menu.
**Priority:** P1

### [P1] There are no context menus anywhere
**Location:** editor right-click path (`crates/slint-shell/src/actions/editor.rs` mouse dispatch, no menu), `project-tree.slint`, `tab-strip.slint`, `bottom-dock.slint`
**Problem:** Right-click produces nothing in the editor, tree, tabs, output, or dock rows.
**Why it matters:** Context menus are the discovery layer for every secondary operation (rename, reveal, copy path, close others, stage hunk, run file). Without them, those operations either do not exist or require source knowledge.
**Reproduction:** Right-click anywhere.
**Expected:** Context-sensitive menus.
**Actual:** Nothing.
**Recommendation:** Implement one reusable popup menu (a `menu-popup.slint` already exists for the custom menu bar) and wire it to tree, tabs, editor, and output.
**Priority:** P1

### [P1] The document is a bitmap, so it has no accessibility or native text services
**Location:** `ui/components/editor-view.slint:37-48` (pane `Image`, `pixelated`), `:51-58` (gutter/minimap images), `:151` (IME-only `TextInput`)
**Problem:** Code is drawn as pixels. There is no accessibility tree for the document, no VoiceOver, no system Find, no spell-check, no dictation, no text-replacement, no OS-level selection services.
**Why it matters:** This is an exclusion, not an inconvenience; it also blocks a whole class of OS integrations that every other macOS editor gets for free.
**Reproduction:** Enable VoiceOver and try to read the buffer.
**Expected:** Accessible text element with role, value, and caret position.
**Actual:** An image.
**Recommendation:** Long term, move to a text-rendering surface with accessibility semantics. Short term, expose an accessibility value/selection proxy for the focused buffer as a stopgap and document the limitation.
**Priority:** P1

### [P1] The terminal dock cannot accept input
**Location:** `ui/components/bottom-dock.slint` (Terminal view is `ScrollView` + `Text`; no `TextInput` in the file)
**Problem:** The Terminal tab renders the session tail read-only. Typing is impossible, so it is a log viewer labelled "Terminal".
**Why it matters:** The dock occupies prime real estate and promises a shell; users will type into it and get nothing, then wonder if the app is frozen.
**Reproduction:** Open the Terminal tab and type.
**Expected:** A real PTY with input, selection, copy, and resize.
**Actual:** Static text.
**Recommendation:** Either wire input to the existing session bridge, or rename the tab to "Session Log" until it is a terminal.
**Priority:** P1

### [P1] The Edit menu has no clipboard or find items, and the breadcrumb is inert
**Location:** `crates/slint-shell/src/projection/mod.rs:262-269` (Edit menu contents), `ui/components/breadcrumb.slint` (no `TouchArea`)
**Problem:** `Edit` contains only Undo, Redo, Find in Buffers. There is no Copy, Paste, Cut, Select All, Find, Replace, or Go to Line. The breadcrumb's path segments are plain `Text` with no click handlers.
**Why it matters:** Menu contents are the app's public contract on macOS; absent clipboard items read as an unfinished port, and a breadcrumb that does nothing is wasted vertical space in the most expensive part of the window.
**Reproduction:** Open the Edit menu; click a breadcrumb segment.
**Expected:** Standard Edit items, enabled/disabled by selection; breadcrumbs that navigate to ancestor directories and symbols.
**Actual:** Three items and dead text.
**Recommendation:** Populate Edit from the command registry (the keymap already has copy/cut/paste/select-all), and either make breadcrumbs clickable or delete the bar and give the 22px back to the editor.
**Priority:** P1

### [P2] Settings exposes registry keys instead of names
**Location:** `ui/views/settings-view.slint` (row title = option name); `crates/slint-shell/src/actions/settings.rs` (registry projection)
**Problem:** Rows are titled `gui-theme`, `breadcrumb-enabled`, `minimap-enabled` with no description of the effect.
**Why it matters:** The settings UI is the one place a user should not need source knowledge; raw keys make it a config-file viewer with widgets.
**Recommendation:** Add a label/description registry beside the option registry; fall back to the key only for Advanced.
**Priority:** P2

### [P2] Internal strings are shown verbatim
**Location:** tab/dock titles (`*make*(# make)`), quit dialog (`use force-quit to override`), status (`Unknown command: main`, `no run configuration named 'build'`)
**Problem:** Vim/emacs buffer syntax and command-line error text appear in GUI chrome.
**Why it matters:** Every leaked internal string is a reminder that the GUI is a skin over a shell, and it makes error text unactionable for anyone who does not know the command language.
**Recommendation:** Map buffer names to display names; replace error strings with sentences plus a next action.
**Priority:** P2

### [P2] Amber is used for success, failure, and idle state
**Location:** `ui/app.slint` status projection; `Theme.warning`
**Problem:** Non-error messages render in warning colour, so severity is unreadable at a glance.
**Recommendation:** Reserve amber/red for problems; use neutral text for state and transient confirmations.
**Priority:** P2

### [P2] The project tree has no operations, no keyboard, and no VCS decoration
**Location:** `ui/components/project-tree.slint` (24px rows, single-click open, no focus, no context menu)
**Problem:** You cannot create, rename, delete, copy a path, reveal, filter, or see modified files from the tree; you cannot traverse it with the keyboard.
**Why it matters:** The tree is the map of the project; without operations it is a read-only outline, and without decoration it hides the information developers check most.
**Recommendation:** Add a context menu, keyboard focus with arrow/Enter, a filter field, and git status colouring. Switch to select-then-Enter with double-click as an alternative to single-click open.
**Priority:** P2

### [P2] Destructive actions are not consistently guarded
**Location:** `crates/slint-shell/src/actions/settings.rs:271-284` (arbitrary command execution from a search-looking box), VCS discard paths
**Problem:** The quit guard now protects unsaved buffers, but the command dialog will execute anything, and there is no confirmation or undo path around the destructive commands it can reach.
**Why it matters:** The one guard that exists is bypassable by typing into a modal that looks like a search field.
**Recommendation:** Whitelist commands in the dialog (or make it a real palette with confirmation for destructive entries).
**Priority:** P2

### [P3] No OS appearance following, no reduced motion, no zoom
**Location:** `theme` option; tab underline animation
**Problem:** Theme is a manual choice; animation ignores `prefers-reduced-motion`; there is no window zoom.
**Recommendation:** Follow system appearance by default; honour reduced motion; add Cmd+=/Cmd+- zoom.
**Priority:** P3

### [P3] Dock tab naming drifts from the code that labels it
**Location:** `crates/slint-shell/src/sync/mod.rs:300-308` (index 3 = "Git History") vs the rendered tab labels
**Problem:** The string table that names dock tabs and the tab labels themselves are maintained separately.
**Recommendation:** Derive tab labels from one list.
**Priority:** P3

---

## 24. Worst 10 Problems, Ranked

1. **Navigation commands execute text as commands.** Four menu items, one prompt, destructive by construction. The single most damaging lie in the product.
2. **No find or replace, anywhere.** The editor cannot search its own buffer; the "Find in Files" tab cannot search the project.
3. **Onboarding teaches nonexistent shortcuts.** Three of the only three advertised keys are dead, including Find.
4. **Build == Test, and both are project-blind.** The green button that is supposed to verify your work fails on this repository.
5. **Build errors are unclickable text.** The dev loop terminates in manual scrolling.
6. **Status pills are dead.** Branch, file, and position — the three most useful click targets in an IDE — do nothing.
7. **Dialogs are keyboard-hostile.** No autofocus, no Escape, no trap; the only navigation UI requires a mouse.
8. **Tabs clip at ~8 with no overflow.** The working set outgrows the container and hidden buffers are unreachable.
9. **No context menus anywhere.** Every secondary operation is undiscoverable or absent.
10. **The document is a bitmap.** No accessibility, no native text services, no system Find.

Honourable mentions: read-only terminal, inert breadcrumb, raw option keys, `*make*(# make)`.

---

## 25. Death by a Thousand Cuts

A breadcrumb you can read but not click. A branch name styled as a button that does nothing. A "Find" tab whose empty state tells you to run a search that has no input. A welcome card confidently teaching `ctrl-shift-f`. A build that ends in `*make*(# make)`. A settings page that says `minimap-enabled`. A dialog that swallows your first six keystrokes into the buffer behind it. A terminal that will not take a keystroke. A quit message that says `use force-quit to override`. An amber status bar that makes every message look like a warning, including "Magma UI ready".

None of these individually justifies abandoning the editor. Together they produce a constant low-level doubt: *is this thing working, or is it just not wired?* In a tool you use for eight hours a day, that doubt is the difference between an editor and a demo.

---

## 26. Design Philosophy

Two philosophies still share one shell, but the balance has shifted.

**A — the kernel's philosophy:** everything is a command, everything is text, navigation is a prompt, buffers have names like `*make*`, configuration is Janet. This is coherent, powerful, and completely invisible to a mouse user.

**B — the Slint shell's philosophy:** a JetBrains-shaped IDE with rails, docks, tabs, a native menu bar, a settings modal, and a toolbar. This is also coherent, and it is what the chrome now promises.

The shell has learned to *look* like B without adopting B's contracts. Navigation is still A (a prompt) behind B's labels ("Go to File"). Search is still A (a quickfix list) behind B's labels ("Find in Files"). Build is still A (`make`) behind B's labels ("Test"). The seam is exactly where the user stands: every control that says "browse", "search", or "verify" is a command in a costume.

The resolution is not to pick one philosophy wholesale; it is to make the shell's promises true when the shell makes them. If a control says *Find*, it must filter. If it says *Test*, it must test. If it says *Go to File*, it must list files. The kernel can stay command-first; the chrome cannot keep advertising capabilities it does not implement.

---

## 27. Brutal Verdict

### 4/10

**Would I use this every day?** No. I could open a project, read code, edit it, and commit — but I could not search, could not jump to a file by name, could not trust the Test button, and could not read a build error without scrolling. The one thing I would do most often is the one thing that is not implemented.

**Biggest strength:** the shell is now real and fast. Native menus, a working project tree, a categorised settings modal with immediate apply, a completion popup with documentation, a coherent theme system, and a quit guard. The engineering underneath (rope, folds, semantic tokens, multi-cursor, LSP) is better than the UI around it, and Slint gives it latency that VS Code cannot match.

**Biggest weakness:** the product lies about its own capabilities at the exact points where a developer forms their opinion — the cheat sheet, the Navigate menu, the Find tab, the Test button. Every one of those is a small, fixable surface; the aggregate is a trust problem.

**Biggest architectural UX mistake:** routing four different navigation intents into one free-text field that calls `execute_command`. It is a single decision (`actions/settings.rs:257`) that transforms search into code execution, makes three menu items false, and turns a search box into a destructive surface. Fixing it is days of work, not months.

**Biggest opportunity:** search. One real find/replace in the buffer plus one real project search would convert this from "an editor I can demo" into "an editor I can use for a task". Everything else — palette, context menus, overflow tabs — compounds from there.

**Delete:** the `dialog:find` / `dialog:goto-file` / `dialog:goto-symbol` shared prompt; the `find_rows` alias to `quickfix_list`; the dead status-bar callbacks (or wire them); the advertised-shortcut string in `welcome.slint`; the second Build button labelled Test.

**Redesign:** navigation as three real pickers (files, symbols, buffers) fed by the index; the Find dock as an actual search surface; the command dialog as a palette with a command list and confirmations; the terminal dock as either a terminal or a log.

**Add:** buffer find/replace, project search, tab overflow, one reusable context menu, clickable build diagnostics, keyboard focus in dialogs, tree operations, and an accessibility proxy for the document.

**Simplify:** five dock tabs where three would do (Find ≠ Problems, and Terminal is a log); three status pills that do nothing; six chrome bands to four. Give the pixels back to code.

---

**Final word.** The previous review said the engine was real and everything you touch was a construction site. That is still true, but the scaffolding has improved: there is now a building, a door, and a sign. The sign points to a room that is not there. Magma's next release should not add a single feature; it should make the four promises the chrome already makes — **Find, Go to File, Test, and the shortcuts on the welcome card** — actually true. Until then, the fastest path through this IDE is still an external `grep`, and no amount of theme polish fixes that.
