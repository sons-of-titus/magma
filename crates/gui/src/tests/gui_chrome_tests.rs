//! GUI chrome contract tests — the JetBrains overhaul's structural invariants:
//! dock assignment, layout defaults, the IntelliJ theme, palette removal, and
//! the Commit window's command surface.  Rendering itself is exercised
//! manually (no headless egui).


use crate::render::gui_dialog::{DialogKind, DialogManager};
use crate::render::gui_layout::{GuiLayout, LeftTool};
use crate::render::gui_theme::GuiTheme;
use crate::render::tool_window::{
    AGENT_TOOL, DEBUG_TOOL, OUTPUT_TOOL, TERMINAL_TOOL, DockPosition, ToolWindowManager,
};
use kernel::state::options::OPTION_SPECS;
use crate::tests::helpers::make_editor;

// ── Dock assignment ───────────────────────────────────────────────────────────

#[test]
fn tool_windows_dock_where_jetbrains_puts_them() {
    let m = ToolWindowManager::new();
    assert_eq!(m.entries[TERMINAL_TOOL].dock, DockPosition::Bottom);
    assert_eq!(m.entries[OUTPUT_TOOL].dock, DockPosition::Bottom);
    assert_eq!(m.entries[DEBUG_TOOL].dock, DockPosition::Left,
        "JetBrains docks the Debugger in the left dock, beside Project/Commit");
    assert_eq!(m.entries[AGENT_TOOL].dock, DockPosition::Right,
        "the agent panel docks on the right of the editor");
}

#[test]
fn the_left_dock_offers_project_commit_and_debugger() {
    let l = GuiLayout::default();
    assert_eq!(l.left_tool, LeftTool::Project);
    assert!(l.debug_width > l.sidebar_width,
        "the Debugger dock is wider than the tree for the Frames tab");
    // Every left rail tool is reachable; the Debugger is one of them.
    let _ = [LeftTool::Project, LeftTool::Commit, LeftTool::Debug];
}

#[test]
fn toggling_the_agent_sets_the_right_active_window() {
    let mut m = ToolWindowManager::new();
    assert!(!m.entries[AGENT_TOOL].visible);
    m.toggle(AGENT_TOOL);
    assert!(m.entries[AGENT_TOOL].visible);
    assert_eq!(m.active_right, AGENT_TOOL);
    assert_eq!(m.active_right_index(), Some(AGENT_TOOL));
    m.toggle(AGENT_TOOL);
    assert_eq!(m.active_right_index(), None, "hiding clears the active right window");
}

#[test]
fn toggling_a_bottom_tool_does_not_touch_the_right_dock() {
    let mut m = ToolWindowManager::new();
    m.toggle(OUTPUT_TOOL);
    assert_eq!(m.active_bottom, OUTPUT_TOOL);
    assert_eq!(m.active_right, AGENT_TOOL, "the right active index is unchanged");
}

// ── Layout defaults ───────────────────────────────────────────────────────────

#[test]
fn layout_defaults_to_project_on_the_left_and_a_closed_agent_dock() {
    let l = GuiLayout::default();
    assert_eq!(l.left_tool, LeftTool::Project);
    assert!(l.sidebar_open);
    assert!(!l.right_open, "the agent dock starts hidden");
    assert_eq!(l.right_width, 340.0);
}

// ── Theme ─────────────────────────────────────────────────────────────────────

#[test]
fn default_theme_is_intellij_dark() {
    let opts = std::collections::HashMap::new();
    let theme = GuiTheme::from_options(&opts);
    assert!(!theme.light);
    assert_eq!(theme, GuiTheme::intellij_dark());
    assert!(!GuiTheme::option_is_light(&opts));
}

#[test]
fn light_theme_variants_report_light() {
    let mut opts = std::collections::HashMap::new();
    for name in ["intellij-light", "latte"] {
        opts.insert("gui-theme".to_string(), name.to_string());
        assert!(GuiTheme::option_is_light(&opts), "{name} must be light");
        assert!(GuiTheme::from_options(&opts).light);
    }
    for name in ["intellij-dark", "mocha"] {
        opts.insert("gui-theme".to_string(), name.to_string());
        assert!(!GuiTheme::option_is_light(&opts), "{name} must be dark");
    }
}

#[test]
fn active_tab_reads_lighter_than_its_strip() {
    use eframe::egui::Color32;
    let luminance = |c: Color32| c.r() as u32 + c.g() as u32 + c.b() as u32;
    for theme in [
        GuiTheme::intellij_dark(),
        GuiTheme::intellij_light(),
        GuiTheme::mocha(),
        GuiTheme::latte(),
    ] {
        let strip = luminance(theme.crust);
        assert!(luminance(theme.tab_active_bg()) > strip,
            "the selected tab must read as elevated, not vanish into the strip");
        assert!(luminance(theme.tab_hover_bg()) > strip);
        assert!(luminance(theme.tab_hover_bg()) <= luminance(theme.tab_active_bg()));
    }
}

#[test]
fn gui_theme_registry_offers_the_intellij_palettes() {
    let spec = OPTION_SPECS.iter().find(|s| s.name == "gui-theme").unwrap();
    assert!(spec.choices.contains(&"intellij-dark"));
    assert!(spec.choices.contains(&"intellij-light"));
    assert_eq!(spec.default, "intellij-dark");
}

#[test]
fn syntax_faces_follow_the_chrome_theme() {
    let mut ed = make_editor();
    GuiTheme::intellij_dark().apply_editor(&mut ed);
    let kw = ed.faces.get("keyword-face").copied().expect("keyword-face is set");
    assert_eq!(kw.fg, (207, 142, 109), "IntelliJ keyword colour");
    assert_eq!(kw.bg, (30, 31, 34), "syntax bg tracks the editor background");
    assert!(ed.faces.get("comment-face").unwrap().italic);

    GuiTheme::mocha().apply_editor(&mut ed);
    assert_eq!(ed.faces.get("keyword-face").unwrap().fg, (203, 166, 247),
        "switching themes must re-colour the syntax faces");
}

// ── Palette removal + settings binding ────────────────────────────────────────

#[test]
fn command_palette_is_gone_and_settings_has_a_jetbrains_chord() {
    assert_eq!(DialogManager::shortcut("ctrl-p"), None,
        "the command palette chord must no longer open anything");
    assert_eq!(DialogManager::shortcut("ctrl-alt-s"), Some(DialogKind::Settings));
    assert_eq!(DialogManager::shortcut("ctrl-comma"), Some(DialogKind::Settings));
}

#[test]
fn goto_pickers_survive_the_palette_removal() {
    assert_eq!(DialogManager::shortcut("ctrl-shift-n"), Some(DialogKind::GotoFile));
    assert_eq!(DialogManager::shortcut("ctrl-shift-o"), Some(DialogKind::GotoSymbol));
}

// ── Commit window command surface ─────────────────────────────────────────────

#[test]
fn commit_window_commands_are_registered() {
    let ed = make_editor();
    for cmd in [
        "vc", "vc-refresh", "vc-fetch", "vc-pull", "vc-push", "vc-log", "vc-blame",
        "vc-stage-file", "vc-unstage-file", "vc-discard-file", "vc-diff-file",
        "vc-commit-message", "vc-amend-message", "vc-branch-list", "vc-checkout-branch",
        "vc-branch-create", "vc-open-diff", "vc-stage-hunk", "vc-discard-hunk", "vc-close-diff",
        "vc-take-ours", "vc-take-theirs",
        "vc-stash", "vc-stash-pop", "vc-stash-apply", "vc-stash-drop",
    ] {
        assert!(ed.commands.exists(cmd), "{cmd} must be registered for the Commit window");
    }
}

#[test]
fn vc_is_a_gui_panel_with_no_modal_layer_or_status_buffer() {
    // The regression: opening VC used to push a *global* `vc` keymap layer and
    // open a magit-style `*vc*` text buffer, hijacking typing in every other
    // buffer — most visibly the scratch buffer it dropped you into.
    let mut ed = make_editor();
    let _ = kernel::command::execute_command(
        &mut ed, "vc", &std::collections::HashMap::new());

    assert!(!ed.keymaps.is_layer_active("vc"),
        "VC must not activate a global keymap layer");
    assert!(!ed.keymaps.is_layer_active("vc-hunks"),
        "hunk staging must not activate a global keymap layer");
    for layer in ed.keymaps.active_layers() {
        assert!(!layer.starts_with("vc"), "no vc* layer may stay active: {layer}");
    }
    assert!(ed.buffers.iter().all(|(_, a)| a.lock().unwrap().name != "*vc*"),
        "VC must not open a text status buffer");
}
