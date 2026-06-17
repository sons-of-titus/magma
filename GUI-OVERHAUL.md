# Magma — GUI Overhaul: JetBrains-Style Native Interface

## From Terminal-in-a-Window to Native IDE Experience

This document defines the migration from the current GUI (a terminal-style
character grid rendered in an egui canvas) to a modern IDE interface inspired
by IntelliJ IDEA / JetBrains products — while **preserving the TUI behaviour**.

The TUI (`--tui`) continues to work.  Every change here is
`#[cfg(feature = "gui")]` — zero impact on terminal users.

## No Backward Compatibility — Ever

This migration explicitly rejects all backward compatibility.

Every PR in every phase must:
- **Delete old GUI code immediately** — the old Surface-based chrome (sidebar,
  status bar in Surface, Janet-drawn modeline for GUI) is removed in the same
  PR that introduces the native egui replacement.
- **Not keep dead code paths** — no `if cfg!(gui)` conditionals that preserve
  the old rendering path.  The TUI retains its path; the GUI gets the new one.
- **Not feature-gate old widgets** — if a widget is replaced, the old code is
  deleted.  There is no `--legacy-sidebar` flag.
- **Change the contract where needed** — `render_frame()` is modified in place
  to support the GUI's `editor_pane_only` mode.  No copy, no compat wrapper.

The rule from AGENTS.md applies: *"No feature flags or backwards-compat shims
when you can just change the code."*  A PR that introduces a shim, alias, or
deprecation warning "to ease migration" is not mergeable.

---

## Current Architecture

### Two Backends, One Surface

```
Editor State
    ↓
render_frame() → Surface (2D char grid of Cell { ch, Style { fg, bg, bold, … } })
    ├── EditorView      (buffer text, gutter, cursor, highlights, folds)
    ├── TerminalView    (PTY output, auto-scroll)
    ├── SidebarView     (file tree — 35 lines, bare minimum)
    ├── Overlays        (z-ordered floating windows)
    ├── Status bar      (bottom row, single line)
    └── Completion popups
    ↓
Janet hooks modify Surface (modeline, tab bar via surface/set-cell)
    ↓
Backend draws Surface:
    TUI:  crossterm + ratatui Paragraph widgets
    GUI:  egui painter calls (CPU path) or wgpu shader (GPU path)
```

The `Surface` (`src/kernel/render/surface.rs:44`) is the universal
intermediate — a character-cell grid.  Both backends consume it identically.
This is the **constraint** that makes the current GUI a "terminal in a window":
every UI element (tabs, sidebar, status bar) is rendered into the same
monospace character grid.

### GUI Backend (`src/kernel/render/gui.rs`)

`GuiApp` implements `eframe::App::update()`:

1. Translate egui input → Magma key strings (`translate_event`)
2. Build a `Surface` sized to the visible character grid
3. Call `render_frame()` into the Surface
4. Two output paths:
   - **GPU path** (`gpu_atlas.rs` + `gpu_paint_callback.rs`): Surface cells →
     `GlyphInstance`/`RectInstance` vertex data → custom WGSL shader.
     Atlas uses placeholder white pixels (no real glyph rasterization yet).
   - **CPU path** (`gui_render.rs`): each cell drawn via `painter.text()`.

### What Exists Today

| Feature | Implementation | Quality |
|---------|---------------|---------|
| Editor text area | Surface → char grid | Mature |
| Syntax highlighting | `highlight_pass.rs` — multi-pass | Mature |
| Gutter (line nums, git, diagnostics, breakpoints) | `gutter_providers.rs` 6 providers | Mature |
| Tabs (top bar) | `tab_bar.janet` — drawn into Surface | Functional, no close btn, no drag |
| Sidebar (file tree) | `sidebar_view.rs` — 35 lines | Minimal — flat list, no tree |
| Status bar | `status_and_popup.rs` + `modeline.janet` | Functional, single line |
| Completion popup | Drawn into Surface | Functional |
| Window splits | `view_tree.rs` — horizontal weight-based | Simple, no recursive nesting |
| GPU rendering | wgpu shader pipeline | Works, atlas is placeholder |
| Fonts | `gui_fonts.rs` — registered via egui | Works, monospace only |

---

## Target Architecture

### Native Widgets + Surface Editor Pane

The core insight: **the Surface stays for the editor pane only**.  All chrome —
tabs, sidebar, toolbar, status bar — uses native egui widgets.  The GUI
`update()` method becomes:

```
GuiApp::update() entry:
  ├── Top panel:      egui menu bar + toolbar widget
  ├── Left panel:     egui TreeView for project files
  ├── Center area:    Split into:
  │   ├── Tab bar:    egui custom tab widget (with close / modified / drag)
  │   ├── Breadcrumb: egui horizontal layout (file path segments)
  │   ├── Editor:     Surface-based char grid (existing GPU/CPU path)
  │   └── Scrollbar:  egui scroll area + minimap
  ├── Bottom panel:   egui status bar with widget slots
  └── Floating:       egui windows for tool panels, search, settings
```

```
                    ┌──────────────────────────────────────┐
                    │ Menu Bar  [File Edit View Navigate …] │
                    ├──────────────────────────────────────┤
                    │ Toolbar  [🞃 ▼ ▲ ⌂ …]                │
                    ├──────┬───────────────────────────────┤
                    │      │ Tab Bar  [file1.rs ✕│file2.rs ✕] │
                    │ Side │ Breadcrumb  src/foo/bar/baz.rs   │
                    │ bar  ├───────────────────────────────┤
                    │ 📦   │                                │
                    │ src  │  1  │ pub fn main() {          │
                    │ lib  │  2  │     let x = 42;          │
                    │ Cargo│  3  │     println!("{x}");     │
                    │      │  4  │ }                        │
                    │      │    ── Surface editor pane ──   │
                    │      │                                │
                    ├──────┴───────────────────────────────┤
                    │ NORMAL │ main.rs  │ UTF-8 │ 3:12  │ Git:main │
                    └──────────────────────────────────────┘
```

### How the Two Paths Diverge (and Converge)

```
render_frame() → Surface → (editor pane only, not full screen)
                             │
              ┌────────────────┴────────────────┐
              │                                 │
         TUI renderer                      GUI renderer
              │                                 │
         ratatui draws               egui layout:
         Surface everywhere            TopPanel (menu + toolbar)
                                       LeftPanel (project tree)
                                       CenterPanel (tabs + breadcrumb + Surface editor)
                                       BottomPanel (status bar)
                                       Windows (tool panels)
```

### File Layout (New)

```
src/kernel/render/
  ├── mod.rs                    — RenderTrait trait (unchanged)
  ├── surface.rs                — Surface struct (unchanged)
  ├── frame.rs                  — render_frame() — renders editor pane only (modify)
  ├── view.rs                   — View trait (unchanged)
  ├── view_tree.rs              — Pane layout (unchanged)
  ├── editor_view.rs            — Buffer text + gutter + cursor (unchanged)
  ├── terminal_view.rs          — PTY output (unchanged)
  ├── sidebar_view.rs           — Deleted — replaced by egui LeftPanel
  ├── tui.rs                    — TuiRenderer (unchanged)
  ├── gui.rs                    — GuiApp — major rewrite: egui layout shell
  ├── gui_render.rs             — CPU fallback for Surface (unchanged, but only for editor pane)
  ├── gui_fonts.rs              — Font helpers (unchanged)
  ├── gui_tabs.rs        [NEW]  — Native tab bar widget (close, modified, drag)
  ├── gui_sidebar.rs     [NEW]  — Project tree widget (collapse, icons, multi-select)
  ├── gui_status.rs      [NEW]  — Status bar widget (mode pill, git branch, position)
  ├── gui_breadcrumb.rs  [NEW]  — Breadcrumb navigation bar
  ├── gui_toolbar.rs     [NEW]  — Toolbar with action buttons
  ├── gui_minimap.rs     [NEW]  — Scrollbar + minimap code overview
  ├── gui_dialog.rs      [NEW]  — Modal dialogs (search, settings, goto)
  ├── gpu_atlas.rs              — Glyph atlas — upgrade with real rasterization
  ├── gpu_paint_callback.rs     — wgpu callback (unchanged)
  ├── gpu_rasterizer.rs  [NEW]  — Real glyph rasterization (ab_glyph/rusttype)
  ├── decorations.rs            — Decoration pass (unchanged)
  ├── gutter.rs                 — GutterProvider trait (unchanged)
  ├── gutter_providers.rs       — Gutter implementations (unchanged)
  ├── highlight_pass.rs         — Syntax highlighting (unchanged)
  └── status_and_popup.rs       — Shared status bar — removed from GUI, kept for TUI
```

---

## Migration Phases

### Phase 0 — Architectural Split: Chrome vs Editor Pane ✅ COMPLETE

**Goal:** Separate GUI chrome (tabs, sidebar, status) from the Surface-based
editor pane.  The GUI no longer draws everything through the Surface — it
renders the editor pane *inside* a native egui layout.

| Step | What | Status |
|------|------|--------|
| 0.1 | `gui_layout.rs` — `GuiLayout` struct (sidebar open/closed, width) | ✅ Done |
| 0.2 | `GuiApp::update()` — layered panels: bottom status, top tabs, left sidebar, central editor | ✅ Done |
| 0.3 | `gui_tabs.rs` — `TabBar` widget: tabs with close buttons, modified indicator, active highlight | ✅ Done |
| 0.4 | `gui_sidebar.rs` — `ProjectTree` widget: flat file list, replaces `sidebar_view.rs` | ✅ Done |
| 0.5 | `gui_status.rs` — `StatusBar` widget: mode pill, buffer name, cursor position | ✅ Done |
| 0.6 | `render_frame()` — `editor_pane_only: bool` param; when true, skips tab bar row, status bar, and completion popup | ✅ Done |
| 0.7 | Wired: `GuiApp::update()` calls egui panels with Surface editor in center | ✅ Done |
| 0.8 | Deleted `sidebar_view.rs` | ✅ Done |
| 0.9 | Removed `draw_status()` from `gui_render.rs`; `present()` renders all surface rows | ✅ Done |

**Verification:** `cargo test --features janet` — 1238 passed, 0 failed.

**Files changed:** Created `gui_layout.rs`, `gui_tabs.rs`, `gui_sidebar.rs`,
`gui_status.rs`. Deleted `sidebar_view.rs`. Modified `gui.rs` (rewrite),
`gui_render.rs`, `frame.rs`, `mod.rs`, `main.rs`, three test files.

---

### Phase 1 — Native Tab Bar ✅ COMPLETE

**Goal:** Replace the Janet-drawn tab bar (a row of text in the Surface) with a
native egui tab widget that matches JetBrains behaviour.

| Step | What | Status |
|------|------|--------|
| 1.1 | Define `TabItem` struct: `label`, `buffer_id`, `modified`, `closable`, `icon` | ✅ Done |
| 1.2 | Implement `TabBar` egui widget: horizontal strip of tabs with close ("✕") buttons on hover, modified indicator (blue dot), active tab highlight, inactive tabs dimmed | ✅ Done |
| 1.3 | Implement tab click → focus pane; close click → close buffer | ✅ Done |
| 1.4 | Implement drag-to-reorder tabs (swap pane positions in ViewTree) | ✅ Done |
| 1.5 | Wire tab bar to `editor.view_tree` — each Pane becomes a tab; closing a tab removes the pane | ✅ Done |
| 1.6 | Surface-based tab bar in `frame.rs` — `if editor.tab_bar_enabled { … }` path clarified as TUI-only | ✅ Done |
| 1.7 | `builtins/tab_bar.janet` retained for TUI; `render-tab-bar` event is not emitted from the GUI loop | ✅ Done |

**JetBrains reference:** IntelliJ tabs show filename (not full path), have a
context menu (close others, close all), and show a tooltip with the full path.
All of these are follow-ups — step 1.0 is a working tab bar.

**Verification:** `cargo test --features janet` — 1238 passed, 0 failed.
`cargo build --features janet,gui` — clean. TUI build (`--no-default-features
--features janet`) — clean.

**Files changed:** Rewrote `gui_tabs.rs` (painter-based, `TabItem`, drag-to-reorder,
hover-only close, Catppuccin Mocha colours). Added `ViewTree::reorder()` to
`view_tree.rs`. Added TUI-only comment to `frame.rs`.

---

### Phase 2 — Project Sidebar (Tree View) ✅ COMPLETE

**Goal:** Replace the 35-line `sidebar_view.rs` flat file list with a
full-featured tree sidebar like IntelliJ's Project view.

| Step | What | Status |
|------|------|--------|
| 2.1 | Define `SidebarState` — `expanded_dirs: HashSet<String>`, `selected: Option<String>` | ✅ Done |
| 2.2 | Implement `ProjectTree` egui widget: recursive tree rendering from `ProjectManager.files`, with collapse/expand icons, indentation, file name + extension | ✅ Done |
| 2.3 | Implement collapse/expand — clicking the arrow toggles directory expansion; state persisted in `SidebarState.expanded_dirs` | ✅ Done |
| 2.4 | Implement click-to-open: single click selects, double click opens file via `open-file` command | ✅ Done |
| 2.5 | Implement file icons: `.rs` → 🦀, `.janet` → 🟢, `.py` → 🐍, `.toml` → ⚙, directory → 📁/📂 | ✅ Done |
| 2.6 | Add resize handle: `SidePanel::resizable(true)` | ✅ Done |
| 2.7 | Add toggle shortcut: `ctrl-\` toggles sidebar visibility | ✅ Done |
| 2.8 | `sidebar_view.rs` was already deleted in Phase 0 | ✅ Done |

**Verification:** `cargo test --features janet` — 1238 passed, 0 failed.
`cargo build --features janet,gui` — clean. TUI build — clean.

**Files changed:** Rewrote `gui_sidebar.rs` (`SidebarState`, `FileNode` tree,
recursive `render_nodes`, `ProjectTree::show` returning `Option<String>`).
Updated `gui.rs` (`sidebar_state` field, ctrl-\ toggle, `open-file` dispatch,
`Key::Backslash` in `translate_event`).

---

### Phase 3 — Proper Status Bar

**Goal:** Replace the single-line Surface-drawn status bar with a rich
widget-based status bar like IntelliJ's — mode indicator, git branch,
cursor position, file encoding, file type, and pluggable widget slots.

| Step | What | Risk |
|------|------|------|
| 3.1 | Define `StatusWidget` trait: `fn render(&self, ui: &egui::Ui, editor: &Editor)` — any subsystem can contribute a widget | Low |
| 3.2 | Implement `ModeWidget` — colored pill showing vim mode (NORMAL/INSERT/VISUAL), same colors as current `gui_fonts::mode_color` | Low |
| 3.3 | Implement `PositionWidget` — `Ln 3, Col 12` with line/col from cursor | Low |
| 3.4 | Implement `GitBranchWidget` — git branch name from `*ml-branch*` or equivalent | Low |
| 3.5 | Implement `FileTypeWidget` — buffer's major mode name (e.g. "Rust", "Janet") | Low |
| 3.6 | Implement `EncodingWidget` — "UTF-8" (constant for now; extensible later) | Low |
| 3.7 | Build `StatusBar` widget: bottom panel with left-aligned widgets (mode, filename) and right-aligned widgets (encoding, filetype, cursor, git) | Low |
| 3.8 | Remove Surface-based status bar rendering from GUI path — `render_status_bar` and `render_command_completion_popup` in `frame.rs` become TUI-only | Low |
| 3.9 | Remove modeline rendering from GUI path — Janet `draw-modeline` event handler is no longer needed for GUI | Low |

**Verification:** Status bar shows mode, file info, cursor position, git branch.
Widgets are aligned left/right correctly.  Resizing the window repositions
content.

---

### Phase 4 — Real Font Rendering + GPU Pipeline Upgrade

**Goal:** Replace the placeholder white-pixel glyph atlas with real glyph
rasterization for crisp, anti-aliased text rendering in the GPU path.

| Step | What | Risk |
|------|------|------|
| 4.1 | Add `ab_glyph` dependency to `Cargo.toml` (lightweight font rasterizer) | Low |
| 4.2 | Implement `GpuGlyphAtlas::rasterize(&mut self, font_bytes: &[u8])` — rasterizes `ATLAS_CHARS` into the atlas pixel buffer using `ab_glyph` | Medium — font metrics, positioning |
| 4.3 | Update `generate_placeholder_pixels()` → `generate_atlas_pixels()` — real glyph bitmaps instead of solid white | Low |
| 4.4 | Extend `ATLAS_CHARS` — add common ASCII + basic Unicode range (Latin-1 Supplement, U+00A0–U+00FF) | Low |
| 4.5 | Update wgpu texture upload to use rasterized pixel data | Low |
| 4.6 | Add fallback for missing glyphs: render as a box character (□) | Low |
| 4.7 | Implement sub-pixel positioning: glyph instances use fractional pixel positions for smoother text | Medium — shader change |
| 4.8 | Add font-ligature support (optional, depends on `ab_glyph` capabilities) | Medium — may need `allsorts` |

**Verification:** GPU path renders sharp, anti-aliased text.  Atlas contains
real glyphs.  All printable ASCII characters render correctly.  `cargo test
--features janet,gui` — zero failures.

---

### Phase 5 — Tool Windows + Floating Panels

**Goal:** Implement JetBrains-style tool windows — terminal, find in files,
search/replace, git log, commit — as egui floating/dockable panels.

| Step | What | Risk |
|------|------|------|
| 5.1 | Define `ToolWindow` trait: `fn title(&self) -> &str`, `fn icon(&self) -> char`, `fn ui(&mut self, ui: &egui::Ui, editor: &Editor)` | Low |
| 5.2 | Implement `ToolWindowManager` — registry of tool windows + visibility state + dock position (left/right/bottom/floating) | Medium |
| 5.3 | Implement terminal tool window: wraps `TerminalView` in an egui panel | Low |
| 5.4 | Implement find-in-files tool window: search bar + results list | Medium — needs search infrastructure |
| 5.5 | Wire tool window buttons into sidebar edge (vertical strip of tool icons, IntelliJ-style) | Medium |
| 5.6 | Implement dock/undock: tool windows can be dragged to a side or float as a separate egui `Window` | Medium — egui drag support |

**Verification:** Tool windows open, close, dock, float.  Terminal tool window
shows PTY output.  Toggle buttons on sidebar edge work.

---

### Phase 6 — Breadcrumbs + Navigation Bar

**Goal:** Show a breadcrumb bar above the editor (like IntelliJ's navigation
bar) showing the current file's path as clickable segments.

| Step | What | Risk |
|------|------|------|
| 6.1 | Implement `BreadcrumbBar` widget: horizontal segments separated by "›" or "/", each segment clickable | Low |
| 6.2 | Derive breadcrumb segments from the focused buffer's path — each path component is a segment | Low |
| 6.3 | Click on a segment opens the corresponding directory in the sidebar | Low |
| 6.4 | Add symbol breadcrumbs (optional): `fn main` shown after the file name when cursor is on a function — requires semantic engine integration | Low |
| 6.5 | Wire toggling breadcrumb visibility via option | Low |

**Verification:** Breadcrumb shows above editor.  Clicking segments navigates.
Updates when buffer changes.

---

### Phase 7 — Scrollbar with Code Overview (Minimap)

**Goal:** Replace the terminal-style scroll with a proper egui scroll area and
optionally a Sublime/VS-Code-style minimap showing a scaled-down view of the
code.

| Step | What | Risk |
|------|------|------|
| 7.1 | Wrap the editor Surface in an egui `ScrollArea` — vertical scroll only | Low |
| 7.2 | Implement `Minimap` widget: renders a scaled-down version of the buffer content in the scrollbar gutter | High — GPU perf, scaling |
| 7.3 | Minimap uses a separate scaled Surface or direct glyph rendering at reduced size | Medium — may need a downscaled Surface |
| 7.4 | Click on minimap scrolls to that position in the editor | Low |
| 7.5 | Minimap highlights (syntax colours, search results) | Medium — needs highlight pass integration |
| 7.6 | Minimap toggle — user option, default off | Low |

**Verification:** Scrollbar works.  Minimap (when enabled) shows a scaled
overview, click-scrolls to position.  `cargo test` — zero failures.

---

### Phase 8 — Modal Dialogs + Search UI

**Goal:** Replace inline command-line prompt with proper modal dialogs for
search, replace, goto-file, settings, and command palette.

| Step | What | Risk |
|------|------|------|
| 8.1 | Implement `SearchDialog` — egui `Window` with search input, results list, replace input (like IntelliJ Ctrl+Shift+F) | Medium — needs search infrastructure |
| 8.2 | Implement `GotoFileDialog` — fuzzy file search from `ProjectManager.files` | Medium — needs fuzzy matching |
| 8.3 | Implement `GotoSymbolDialog` — symbol search from `SemanticEngine.symbol_index` | Medium — needs symbol prefix search |
| 8.4 | Implement `CommandPalette` — fuzzy command search from command registry | Low |
| 8.5 | Implement `SettingsDialog` — option browser/editor in egui panels | Medium — two-way option binding |
| 8.6 | Wire keyboard shortcuts — `cmd-p` → command palette, `cmd-shift-f` → search, `cmd-o` → goto file | Low |
| 8.7 | Keep colon-mode (`:`) functional — it remains the Janet scripting interface; dialogs are an alternative UX, not a replacement | Low |

**Verification:** All dialogs open, display content, accept input, and execute
the expected action.  Keyboard shortcuts work.

---

### Phase 9 — Theme Refinement + Visual Polish

**Goal:** Make the GUI look polished — proper spacing, corner rounding, shadow,
animation, icon set.

| Step | What | Risk |
|------|------|------|
| 9.1 | Define a GUI-specific theme system — separate from the terminal face system.  Colors for panels, widgets, hover, selection, focus | Low |
| 9.2 | Round corners on panels, tabs, and dialogs (egui `Rounding`) | Low |
| 9.3 | Add subtle drop shadows to floating panels (egui `Shadow`) | Low |
| 9.4 | Add hover/active/pressed state styling to all interactive widgets | Low |
| 9.5 | Animate tab transitions, panel openings (egui `lerp` + repaint) | Medium — animation loop |
| 9.6 | Add a proper icon set (use a small icon font or SVG via egui `Image`) | Medium — asset management |
| 9.7 | Dark mode / light mode toggle — wire to theme system | Low |

**Verification:** GUI looks polished.  Hover states work.  Tabs animate.
Themes switch correctly.

---

### Phase 10 — TUI Preserved + Feature Parity

**Goal:** Verify that the TUI backend (`--tui`) is completely untouched and all
features work identically in both backends.

| Step | What | Risk |
|------|------|------|
| 10.1 | Run full test suite with `--tui` flag — all tests pass | Low |
| 10.2 | Verify all Janet event handlers (tab-bar, modeline) still fire correctly in TUI mode | Low |
| 10.3 | Verify `render_frame()` in TUI mode still draws the full screen Surface as before | Low |
| 10.4 | Verify TUI window splits, tab bar, status bar, sidebar all render identically | Low |
| 10.5 | Document the TUI/GUI split in `docs/architecture/gui-architecture.md` | Low |

**Verification:** `--tui` produces identical output to pre-migration.  All gate
conditions pass.

---

## Files: Created, Modified, Deleted

### Created

| File | Phase | Purpose |
|------|-------|---------|
| `src/kernel/render/gui_layout.rs` | 0 | Layout state, panel visibility, sidebar width |
| `src/kernel/render/gui_tabs.rs` | 1 | Tab bar widget |
| `src/kernel/render/gui_sidebar.rs` | 2 | Project tree sidebar widget |
| `src/kernel/render/gui_status.rs` | 3 | Status bar widget + `StatusWidget` trait |
| `src/kernel/render/gui_breadcrumb.rs` | 6 | Breadcrumb navigation bar |
| `src/kernel/render/gui_toolbar.rs` | 0 | Toolbar with action buttons |
| `src/kernel/render/gui_minimap.rs` | 7 | Minimap code overview |
| `src/kernel/render/gui_dialog.rs` | 8 | Modal dialogs |
| `src/kernel/render/gpu_rasterizer.rs` | 4 | Real glyph rasterization |
| `src/kernel/render/tool_window.rs` | 5 | Tool window manager + trait |
| `docs/architecture/gui-architecture.md` | 10 | Architecture documentation |

### Modified

| File | Phase | Change |
|------|-------|--------|
| `src/kernel/render/gui.rs` | 0 | Major rewrite — egui layout shell |
| `src/kernel/render/frame.rs` | 0 | `editor_pane_only` mode; conditional status bar |
| `src/kernel/render/mod.rs` | 0 | Add new module declarations |
| `src/kernel/render/gpu_atlas.rs` | 4 | Real rasterization replaces placeholder |
| `src/main.rs` | 0 | GUI window config (bigger default size, title) |
| `Cargo.toml` | 4 | Add `ab_glyph` dependency |

### Deleted

| File | Phase | Reason |
|------|-------|--------|
| `src/kernel/render/sidebar_view.rs` | 2 | Replaced by `gui_sidebar.rs` |

### TUI — Behaviour Preserved

The TUI continues to render through the Surface as it always has.  Shared
abstractions (`Surface`, `render_frame`, `ViewTree`, `EditorView`) may be
refactored to support the new GUI architecture — the TUI's observable
behaviour is preserved, its code is not frozen.

TUI-specific code (`tui.rs`, `builtins/tab_bar.janet`, `builtins/modeline.janet`)
stays in place and continues to work through `--tui`.

---

## Dependency Graph

```
Phase 0 (Architectural Split) — prerequisite for everything
  │
  ├──► Phase 1 (Tab Bar) — no deps beyond Phase 0
  ├──► Phase 2 (Sidebar) — no deps beyond Phase 0
  ├──► Phase 3 (Status Bar) — no deps beyond Phase 0
  │
  ├──► Phase 4 (Font Rendering) — independent, can start anytime
  │
  ├──► Phase 5 (Tool Windows) — depends on Phase 0 + 1
  ├──► Phase 6 (Breadcrumbs) — depends on Phase 0
  ├──► Phase 7 (Minimap) — depends on Phase 0 + 4 (GPU rendering)
  ├──► Phase 8 (Dialogs) — depends on Phase 0 + Semantic Engine
  │
  └──► Phase 9 (Visual Polish) — depends on all above
        └──► Phase 10 (TUI verification) — final phase
```

**Recommended execution order:**
1. Phase 0 (one sprint) — architectural split, everything depends on it
2. Phase 1 + Phase 2 + Phase 3 (parallel, one sprint) — visible chrome
3. Phase 4 + Phase 5 (parallel, one sprint) — rendering quality + tool windows
4. Phase 6 + Phase 7 + Phase 8 (parallel, two sprints) — navigation + search
5. Phase 9 + Phase 10 (parallel, one sprint) — polish + verification

**Total: ~6 sprints** (assuming 2-week sprints, ~12 weeks).

Phases 1–3 can ship independently — each adds immediate visible value.

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Phase 0 breaks TUI rendering | Medium | Critical | `--tui` flag regression-tested in every commit |
| Surface abstraction is wrong abstraction for native GUI | Low | Medium | Surface stays for editor pane only — chrome uses native widgets |
| egui panels don't compose well with Surface GPU path | Medium | Medium | Fall back to CPU path for editor pane if GPU path is incompatible |
| Janet-drawn modeline/tab-bar conflict with native widgets | Medium | Low | Render modeline/tab-bar only in TUI mode; GUI uses native equivalents |
| Font atlas rasterization perf is too slow | Low | Medium | Build atlas once per font change, not per frame; cache glyphs |
| egui drag-and-drop for tab reordering is complex | Medium | Low | Defer drag-reorder to Phase 9; tabs work without it |
| Minimap rendering is too expensive | Medium | Low | Make minimap optional, default off; use downscaled Surface |
| Test coverage for GUI-only features | High | Medium | Integration tests snapshot Surface content; manual GUI testing for chrome |

---

## Completion Gates (applies to every phase)

1. **Tests:** `cargo test --features janet,gui` — zero failures.
2. **TUI preserved:** `cargo run -- --tui` produces identical output to
   pre-migration head.  All TUI tests pass.
3. **File size:** No file > 400 lines (Rust) or > 200 lines (Janet).  New GUI
   widget files are separate.
4. **Documentation:** Every new Janet C function in `docs/api/janet-api.md`;
   architecture decisions in `docs/architecture/`.
5. **No backward compatibility:** Old GUI chrome code is **deleted** in the
   same PR that introduces its replacement — no aliases, no shims, no
   deprecation warnings, no `--legacy-gui` flag, no feature-gated old widgets.
   TUI code may be refactored where shared abstractions change, but its
   observable behaviour is preserved.
6. **Feature-gated:** Every new file is `#[cfg(feature = "gui")]`; the TUI
   build (`--no-default-features --features janet`) compiles and passes all
   tests.
