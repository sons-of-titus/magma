# GUI Architecture — Chrome vs Editor Pane

The GUI backend is a native IDE shell (egui) wrapped around the same
`Surface`-based editor core the TUI uses. The split is deliberate:

- **The editor pane** — buffer text, gutter, cursor, folds, syntax colours —
  renders through the character-cell `Surface`, painted either by the wgpu
  GPU pipeline or the CPU fallback. Both backends share this path.
- **Everything else** — tabs, sidebar, status bar, breadcrumbs, minimap,
  tool windows, dialogs — is native egui chrome. It never touches the
  `Surface`.

All GUI code is `#[cfg(feature = "gui")]`. The TUI build
(`cargo build --no-default-features --features janet`) contains none of it,
and the TUI's observable behaviour is unchanged: `render_frame()` draws the
full-screen `Surface` (tab bar row, status bar, popups) exactly as before
when called with `editor_pane_only = false`.

## Frame composition

`GuiApp::update()` (in `gui.rs`) assembles the shell top-down. Panels are
declared outside-in, so the first panel declared is the outermost:

```
TopBottomPanel::bottom("magma_status")       ← StatusBar (mode pill, git, position)
TopBottomPanel::bottom("magma_tools_bottom") ← docked ToolWindows (terminal, find)
TopBottomPanel::top("magma_tabs")            ← TabBar (drag-reorder, close, modified)
TopBottomPanel::top("magma_breadcrumb")      ← BreadcrumbBar (path › symbol)
SidePanel::left("magma_tool_strip")          ← 28px tool-window icon strip
SidePanel::left("magma_sidebar")             ← ProjectTree (ctrl-\ toggles)
CentralPanel                                 ← editor pane (Surface + strip)
Window(layer)                                ← modal dialogs, floating tool windows
```

## Module map

| Module | Responsibility |
|--------|----------------|
| `gui.rs` | `GuiApp` — shell composition, action application, key routing |
| `gui_input.rs` | egui events → Magma key strings + raw mouse events |
| `gui_editor_pane.rs` | Central pane: Surface build, GPU/CPU paint, scrollbar, mouse dispatch |
| `gui_tabs.rs` | Tab bar (close, modified dot, drag-reorder, animated underline) |
| `gui_sidebar.rs` | Project file tree (collapse, icons, double-click open) |
| `gui_status.rs` | Status bar + `StatusWidget` trait |
| `gui_breadcrumb.rs` | Path segments + innermost symbol tail; reveal-in-sidebar |
| `gui_minimap.rs` | Code-overview strip (sampled lines, syntax colours, viewport rect) |
| `gui_dialog.rs` | `DialogManager`, `DialogAction`, Settings dialog |
| `gui_search_dialog.rs` | In-buffer search + replace-all |
| `gui_goto_dialogs.rs` | Goto-file / goto-symbol / command-palette pickers |
| `gui_fuzzy.rs` | Pure subsequence matcher with ranking |
| `gui_theme.rs` | `GuiTheme` palette (Mocha/Latte), egui visuals, editor colour map |
| `tool_window.rs` | `ToolWindow` trait + manager, terminal & find-in-files windows |
| `gui_layout.rs` | Panel visibility/size state |

## Widget contract

Chrome widgets never mutate the `Editor` directly. They take `&Editor`
(readonly) and return an action enum (`TabAction`, `DialogAction`,
`BreadcrumbAction`, `EditorPaneAction::GotoLine`). The shell applies actions
under a write lock after the widget returns. This keeps rendering lock-free
and mutation centralized in `gui.rs`.

## Input flow

```
egui events
  → gui_input::collect()          key strings ("ctrl-p", "return", …) + pixel mouse events
  → gui.rs key loop:
        ctrl-q            quit
        ctrl-\            toggle sidebar
        ctrl-shift-l      toggle theme (gui-theme option)
        DialogManager::shortcut(key)  ctrl-p / ctrl-shift-f / ctrl-o / ctrl-shift-o / ctrl-comma
        dialog open?      Escape closes; every other key is consumed by the dialog
        otherwise         input::dispatch_key (keymap + Janet handlers, same as TUI)
  → gui_editor_pane             pixel → cell coords, editor rect only
```

Modifier encoding includes shift (`ctrl-shift-f` ≠ `ctrl-f`), so chords are
distinguishable. Special keys (arrows, F-keys, Enter) keep their plain names
regardless of modifiers, matching the TUI's grammar.

## Editor pane strip

The central pane reserves a right-hand strip: a scrollbar by default, or the
minimap when `minimap-enabled` is `true`. Scrolling is cursor-driven (vim
semantics), so both the scrollbar thumb and the minimap viewport rectangle
derive from `compute_scroll_state`, and click/drag on either dispatches
`goto-line`. Mouse events landing in the strip never reach `dispatch_mouse`.

## Theme system

`GuiTheme` (`gui_theme.rs`) holds one palette. Two presets — Catppuccin
Mocha (dark) and Latte (light) — are selected by the `gui-theme` option, so
the choice persists through the workspace like any option. On change the
shell pushes the palette in two directions:

- `apply_egui()` — widget visuals: fills, hover/active states, rounding,
  window shadows, spacing.
- `apply_editor()` — the `Editor.theme` colour map (`bg`, `fg`, `status-*`,
  `selection-*`, `gui-bg`, …), which `render_frame` and both paint paths
  already consume.

All chrome widgets take `&GuiTheme`; there are no per-widget colour
constants.

## Dialogs and colon-mode

Dialogs are an alternative UX, not a replacement: colon-mode (`:`) and the
Janet minibuffer are untouched and remain the scripting interface. When a
dialog is open it owns the keyboard; the shell's key loop stops dispatching
to the editor so typing reaches the dialog's text fields only.

Dialog actions map onto existing commands — `open-file`, `goto-line`, the
command registry — so anything Janet registers (`ed.commands` fallback in
`execute_command`) is reachable from the palette for free.

## Behaviour preservation checklist (Phase 10)

- `render_frame(&ed, &mut surface, false)` output is byte-identical to
  pre-overhaul (TUI path; `tab_bar_rows`/status bar logic untouched).
- Janet modeline, tab-bar hooks, and all event handlers fire only in TUI
  mode; the GUI loop never emits `RENDER_FRAME`.
- `cargo build --no-default-features --features janet` compiles with zero
  GUI code.
- `cargo test --features janet` runs the full suite including the
  `gui_overhaul` feature-area tests.
