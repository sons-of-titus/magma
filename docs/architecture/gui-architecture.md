# GUI Architecture — Chrome vs Editor Pane

The GUI is a native IDE shell (egui) wrapped around a `Surface`-based editor
core. The split is deliberate:

- **The editor pane** — buffer text, gutter, cursor, folds, syntax colours —
  renders through the character-cell `Surface`, painted either by the wgpu
  GPU pipeline or the CPU fallback. Both backends share this path.
- **Everything else** — tabs, sidebar, status bar, breadcrumbs, minimap,
  tool windows, dialogs — is native egui chrome. It never touches the
  `Surface`.

All GUI code is `#[cfg(feature = "gui")]`. Without the feature the binary
still builds for headless use (`--server`); interactive use requires it.

## Frame composition

`GuiApp::update()` (in `gui.rs`) assembles the shell top-down. Panels are
declared outside-in, so the first panel declared is the outermost:

```
TopBottomPanel::bottom("magma_status")       ← StatusBar (file, messages, git, position)
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
| `gui.rs` | `GuiApp` — shell composition and the frame pipeline |
| `gui_shell.rs` | Welcome flow, project watching, folder picker, key pre-dispatch, theme/GPU sync, action application |
| `gui_input.rs` | egui events → Magma key strings + raw mouse events |
| `gui_editor_pane.rs` | Central pane: Surface build, GPU/CPU paint, scrollbar, mouse dispatch |
| `gui_welcome.rs` | Welcome screen (open folder, recents, key hints) |
| `gui_tabs.rs` | Tab bar (close → scratch, modified dot, drag-reorder, animated underline) |
| `gui_sidebar.rs` + `gui_sidebar_state.rs` | Project file tree: single-click open, context menu (new/rename/delete/copy path), inline input row |
| `gui_status.rs` | Status bar, transient messages, clickable git branch |
| `gui_breadcrumb.rs` | Path segments + innermost symbol tail; reveal-in-sidebar |
| `gui_minimap.rs` | Code-overview strip (sampled lines, syntax colours, viewport rect) |
| `gui_dialog.rs` | `DialogManager`, `DialogAction`, quit confirmation |
| `gui_settings_dialog.rs` | Typed settings dialog over the option registry |
| `gui_search_dialog.rs` | In-buffer search, Enter/Shift-Enter hit walking, replace-all |
| `gui_goto_dialogs.rs` | Pickers: search-everywhere, goto-file, goto-symbol |
| `gui_fuzzy.rs` | Pure subsequence matcher with ranking |
| `gui_theme.rs` | `GuiTheme` palette (Mocha/Latte), egui visuals, editor colour map |
| `tool_window.rs` | `ToolWindow` trait + manager, terminal (with input row) |
| `gui_find_tool.rs` | Find-in-files (ripgrep on the project) + shared quickfix rows |
| `gui_output_tool.rs` | Task output + problems panel |
| `gui_debug_tool.rs` | DAP session view: threads, call stack (clickable), variables |
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
  → gui_shell::dispatch_shell_key (the ONLY keymap pre-emption; skipped while
                                   a terminal buffer owns the focus):
        ctrl-q            quit guard (dialog when dirty — see Behaviour notes)
        ctrl-\            toggle sidebar
        ctrl-shift-l      toggle theme (gui-theme option)
        ctrl-p            search everywhere (files + symbols + commands)
        ctrl-shift-n      go to file
        ctrl-shift-f      search open buffers
        ctrl-shift-o      go to symbol
        ctrl-comma        settings
        dialog open?      Escape closes; every other key is consumed by the dialog
        otherwise         input::dispatch_key (keymap + Janet handlers)
  → the window close button routes through the same quit guard
  → gui_editor_pane             pixel → cell coords, editor rect only
```

Modifier encoding includes shift (`ctrl-shift-f` ≠ `ctrl-f`), so chords are
distinguishable. Special keys (arrows, F-keys, Enter) keep their plain names
regardless of modifiers.  `keymap_guard_tests.rs` fails the build if a
keymap layer ever binds a GUI-reserved chord.

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

## Dialogs

Dialogs are command surfaces, not a separate brain: when a dialog is open it
owns the keyboard; the shell's key loop stops dispatching to the editor so
typing reaches the dialog's text fields only.  Janet-defined commands are
deliberately hidden from the palette — it lists the Rust-native registry
(`list_rust_commands`); Janet commands remain reachable via the keymap and
`editor/run-command`.

Dialog actions map onto existing commands — `open-file`, `goto-line`, the
command registry — so anything Rust registers is reachable from the palette
for free.

## Behaviour notes

- **Two-window model**: the primary window is the welcome surface; opening
  a project opens the editor shell in its own viewport
  (`show_viewport_immediate`) and hides welcome.  Quitting the editor or
  closing its window (both guarded by `quit`) returns to welcome; closing
  welcome exits the app.
- The shipped editor is **non-modal**: buffers always accept text, `esc`
  only clears the selection, and there is no command line — the palette is
  the command surface.  `editor/set-mode` and keymap layers remain as the
  substrate a community modal-editing plugin builds on.
- Quitting (ctrl-q, window close) is guarded by `quit`: with modified
  writable buffers it refuses (naming them), and the GUI offers an Unsaved
  Changes dialog (save all / discard / cancel).  `force-quit` is the
  explicit override.  Read-only
  buffers (shell/grep output) never block.
- The 30-second background auto-saver is opt-in: it runs only while the
  `auto-save` option is `true` (default off).
- `render_frame(&ed, &mut surface)` draws editor-pane content only — the
  surface has no tab-bar or status-bar rows; chrome is native egui.
- Janet modeline and tab-bar surface hooks are not emitted from the GUI
  loop; the GUI's status bar and tab bar are native widgets.
- `cargo build --no-default-features --features janet` compiles headless
  (server mode) with zero GUI code.
- `cargo test --features janet` runs the full suite including the
  `gui_overhaul` feature-area tests.
