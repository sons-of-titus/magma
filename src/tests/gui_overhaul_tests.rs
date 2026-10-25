//! GUI overhaul — Phases 6–9 integration tests.
//!
//! Covers the pure/editor-touching halves of the new chrome: dialog actions,
//! search, pickers, breadcrumbs, and the theme system.  egui-rendering paths
//! are exercised manually (no headless egui in CI).

#![cfg(feature = "gui")]

use crate::kernel::render::gui_breadcrumb::{relative_to_project, segments, symbol_at};
use crate::kernel::render::gui_dialog::DialogAction;
use crate::kernel::render::gui_settings_dialog::sorted_options;
use crate::kernel::render::gui_goto_dialogs::{collect_entries, PickerEntry, PickerSource};
use crate::kernel::render::gui_search_dialog::SearchDialog;
use crate::kernel::render::gui_theme::GuiTheme;
use crate::kernel::semantic::provider::{Symbol, SymbolKind};
use crate::tests::helpers::{buf_text, cursor, make_editor, make_editor_with_buffer};

// ── DialogAction::SetOption ───────────────────────────────────────────────────

#[test]
fn set_option_action_writes_option() {
    let mut ed = make_editor();
    DialogAction::SetOption { name: "minimap-enabled".into(), value: "true".into() }
        .apply(&mut ed);
    assert_eq!(ed.options.get("minimap-enabled").map(|s| s.as_str()), Some("true"));
}

// ── DialogAction::GotoLine ────────────────────────────────────────────────────

#[test]
fn goto_line_action_moves_cursor() {
    let mut ed = make_editor_with_buffer("alpha\nbeta\ngamma\n");
    assert_eq!(cursor(&ed), 0);
    DialogAction::GotoLine(3).apply(&mut ed);
    assert_eq!(buf_text(&ed).chars().take(cursor(&ed)).filter(|&c| c == '\n').count(), 2);
}

// ── DialogAction::ReplaceAllInFocused ────────────────────────────────────────

#[test]
fn replace_all_rewrites_every_occurrence() {
    let mut ed = make_editor_with_buffer("foo bar foo");
    DialogAction::ReplaceAllInFocused { find: "foo".into(), replace: "qux".into(), regex: false }
        .apply(&mut ed);
    assert_eq!(buf_text(&ed), "qux bar qux");
}

#[test]
fn replace_all_empty_find_is_noop() {
    let mut ed = make_editor_with_buffer("unchanged");
    DialogAction::ReplaceAllInFocused { find: String::new(), replace: "x".into(), regex: false }
        .apply(&mut ed);
    assert_eq!(buf_text(&ed), "unchanged");
}

// ── DialogAction::ExecuteCommand ─────────────────────────────────────────────

#[test]
fn execute_command_action_runs_registry_command() {
    let mut ed = make_editor_with_buffer("one\ntwo\nthree");
    DialogAction::ExecuteCommand("goto-line".into()).apply(&mut ed);
    // goto-line with no arg jumps to the last line → offset 8 ("two\n" skipped).
    assert_eq!(cursor(&ed), 8);
}

// ── DialogAction::FocusBufferAndGoto ─────────────────────────────────────────

#[test]
fn focus_buffer_switches_pane_and_jumps() {
    let mut ed = make_editor_with_buffer("one\ntwo\nthree\n");
    let key2 = ed.create_buffer_from_str("other", "a\nb\nc\n");
    DialogAction::FocusBufferAndGoto { buf_id: key2, line: 3 }.apply(&mut ed);
    let focused = ed.view_tree.focused_window()
        .and_then(|wid| ed.view_tree.buffer(wid))
        .unwrap();
    assert_eq!(focused, key2);
    let text = {
        let buf = ed.buffers.get(key2).unwrap().lock().unwrap();
        buf.slice(0, buf.len())
    };
    let off = ed.views.get(&key2).unwrap().cursor_offset();
    assert_eq!(text[..off].matches('\n').count(), 2, "cursor should sit on line 3");
}

// ── DialogAction::OpenFile ───────────────────────────────────────────────────

#[test]
fn open_file_action_loads_disk_file() {
    let dir  = std::env::temp_dir().join("magma_gui_overhaul_tests");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("open_me.txt");
    std::fs::write(&path, "from disk\n").unwrap();
    let mut ed = make_editor();
    DialogAction::OpenFile(path.to_string_lossy().into_owned()).apply(&mut ed);
    let opened = ed.buffers.iter()
        .find(|(_, arc)| {
            arc.lock().unwrap().name == "open_me.txt"
        })
        .map(|(k, _)| k)
        .expect("open-file must create the buffer");
    let text = {
        let buf = ed.buffers.get(opened).unwrap().lock().unwrap();
        buf.slice(0, buf.len())
    };
    assert_eq!(text, "from disk\n");
}

#[test]
fn goto_symbol_action_opens_and_jumps() {
    let dir  = std::env::temp_dir().join("magma_gui_overhaul_tests");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("symbols.txt");
    std::fs::write(&path, "a\nb\nc\n").unwrap();
    let full = path.to_string_lossy().into_owned();
    let mut ed = make_editor_with_buffer("starter\n");
    DialogAction::GotoSymbol { file: full.clone(), line: 1 }.apply(&mut ed);
    let focused = ed.view_tree.focused_window()
        .and_then(|wid| ed.view_tree.buffer(wid))
        .unwrap();
    assert!(
        ed.buffers.get(focused).unwrap().lock().unwrap().path.as_deref() == Some(full.as_str()),
        "focused buffer should be the opened file"
    );
    let (text, cursor_off) = {
        let buf = ed.buffers.get(focused).unwrap().lock().unwrap();
        (buf.slice(0, buf.len()), ed.views.get(&focused).unwrap().cursor_offset())
    };
    assert_eq!(text[..cursor_off].matches('\n').count(), 1, "cursor on line 2");
}

// ── SearchDialog ─────────────────────────────────────────────────────────────

#[test]
fn search_finds_across_buffers() {
    let mut ed = make_editor_with_buffer("needle here\nnothing\n");
    let _b2 = ed.create_buffer_from_str("second", "plain\nhas needle too\n");
    let mut dlg = SearchDialog::new();
    dlg.query = "needle".into();
    dlg.search(&ed);
    assert_eq!(dlg.results().len(), 2);
    assert!(dlg.results().iter().any(|h| h.line == 2 && h.buf_name == "second"));
}

#[test]
fn search_smart_case() {
    let ed = make_editor_with_buffer("Needle\nneedle\n");
    let mut dlg = SearchDialog::new();
    dlg.query = "needle".into();
    dlg.search(&ed);
    assert_eq!(dlg.results().len(), 2, "lowercase query matches case-insensitively");

    dlg.query = "Needle".into();
    dlg.search(&ed);
    assert_eq!(dlg.results().len(), 1, "mixed-case query is exact");
}

#[test]
fn smart_case_predicate() {
    use crate::kernel::command::builtin::search_query::SearchQuery;
    match SearchQuery::compile("abc", false).unwrap() {
        SearchQuery::Regex(_) => {}           // lowercase ⇒ insensitive ⇒ regex path
        SearchQuery::Literal(_) => panic!("lowercase queries must compile case-insensitive"),
    }
    assert!(matches!(SearchQuery::compile("Abc", false).unwrap(), SearchQuery::Literal(_)),
        "mixed-case queries are exact literals");
    assert!(SearchQuery::compile("", false).is_err(), "empty queries are rejected");
}

// ── SettingsDialog helpers ───────────────────────────────────────────────────

#[test]
fn sorted_options_is_sorted() {
    let mut ed = make_editor();
    ed.options.insert("zeta".into(), "1".into());
    ed.options.insert("alpha".into(), "2".into());
    let opts = sorted_options(&ed);
    let names: Vec<&str> = opts.iter().map(|(k, _)| k.as_str()).collect();
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(names, sorted);
}

// ── Picker entries ───────────────────────────────────────────────────────────

#[test]
fn picker_files_join_project_root() {
    let mut ed = make_editor();
    ed.project_manager.project.root =
        Some(std::path::PathBuf::from("/tmp/proj"));
    ed.project_manager.project.files = vec!["src/main.rs".into()];
    let entries = collect_entries(&ed, PickerSource::Files);
    match &entries[0] {
        PickerEntry::File(p) => assert_eq!(p, "/tmp/proj/src/main.rs"),
        other => panic!("expected File entry, got {other:?}"),
    }
}

#[test]
fn picker_symbols_come_from_index() {
    let mut ed = make_editor();
    ed.semantic.symbol_index.add_symbols("/a.rs", vec![
        Symbol { name: "main".into(), kind: SymbolKind::Function,
                 file: String::new(), line: 4, column: 0, documentation: None },
    ]);
    let entries = collect_entries(&ed, PickerSource::Symbols);
    assert_eq!(entries.len(), 1);
    match &entries[0] {
        PickerEntry::Symbol { name, file, line } => {
            assert_eq!(name, "main");
            assert_eq!(file, "/a.rs");
            assert_eq!(*line, 4);
        }
        other => panic!("expected Symbol entry, got {other:?}"),
    }
}

#[test]
fn picker_commands_list_registry() {
    let ed = make_editor();
    let listed = ed.commands.list_detailed();
    assert!(listed.iter().any(|(name, _, _)| name == "goto-line"));
}

#[test]
fn picker_lists_janet_defined_commands() {
    let mut ed = make_editor();
    // A Janet-defined trampoline (as command/define registers) is a normal
    // registry citizen: the registry lists every command.
    ed.commands.register("janet-plumb", "Janet-defined command", vec![],
        Box::new(|_ed, _args| Ok(())));
    let listed = ed.commands.list_detailed();
    assert!(listed.iter().any(|(name, _, _)| name == "janet-plumb"),
        "Janet-defined commands must appear in the registry listing");
    assert!(listed.iter().any(|(name, _, _)| name == "goto-line"));
}

// ── CommandRegistry::list_detailed ───────────────────────────────────────────

#[test]
fn list_detailed_flags_required_args() {
    use crate::kernel::command::args::{ArgSpec, ArgType};
    let mut ed = make_editor();
    ed.commands.register_fn(
        "gui-test-needs-arg", "desc",
        vec![ArgSpec::new("path", ArgType::Path)],
        |_ed, _args| Ok(()),
    );
    let listed = ed.commands.list_detailed();
    let needs = listed.iter().find(|(n, _, _)| n == "gui-test-needs-arg").unwrap();
    assert!(needs.2, "required arg without default must set needs_args");
    let goto = listed.iter().find(|(n, _, _)| n == "goto-line").unwrap();
    assert!(!goto.2, "goto-line has an optional arg only");
}

// ── Breadcrumbs ──────────────────────────────────────────────────────────────

#[test]
fn breadcrumb_symbol_at_picks_innermost() {
    let mut ed = make_editor();
    ed.semantic.symbol_index.add_symbols("test", vec![
        Symbol { name: "outer".into(), kind: SymbolKind::Function,
                 file: "test".into(), line: 0, column: 0, documentation: None },
        Symbol { name: "inner".into(), kind: SymbolKind::Function,
                 file: "test".into(), line: 5, column: 0, documentation: None },
    ]);
    assert_eq!(symbol_at(&ed, "test", 7), Some("inner".to_string()));
    assert_eq!(symbol_at(&ed, "test", 2), Some("outer".to_string()));
    assert_eq!(symbol_at(&ed, "other", 7), None);
}

#[test]
fn breadcrumb_segments_helpers() {
    assert_eq!(segments("a/b/c.rs"), vec!["a", "b", "c.rs"]);
    assert_eq!(
        relative_to_project(Some(std::path::Path::new("/r")), "/r/x/y.rs"),
        "x/y.rs"
    );
}

// ── Theme ────────────────────────────────────────────────────────────────────

#[test]
fn theme_resolves_from_options() {
    let mut opts = std::collections::HashMap::new();
    assert!(!GuiTheme::from_options(&opts).light);
    opts.insert("gui-theme".to_string(), "latte".to_string());
    assert!(GuiTheme::from_options(&opts).light);
    opts.insert("gui-theme".to_string(), "mocha".to_string());
    assert!(!GuiTheme::from_options(&opts).light);
}

#[test]
fn theme_applies_editor_colors() {
    let mut ed = make_editor();
    GuiTheme::latte().apply_editor(&mut ed);
    assert_eq!(ed.theme_color("gui-bg"), (239, 241, 245));
    GuiTheme::mocha().apply_editor(&mut ed);
    assert_eq!(ed.theme_color("gui-bg"), (30, 30, 46));
}

// ── Icon glyph coverage ──────────────────────────────────────────────────────

#[test]
fn every_theme_icon_glyph_is_covered_by_bundled_fonts() {
    use crate::kernel::render::gui_theme::icons;
    use crate::kernel::state::FontConfig;
    use eframe::egui::epaint::text::{FontId, Fonts, TextOptions};

    let mut fonts = Fonts::new(
        TextOptions { max_texture_side: 4096, ..Default::default() },
        crate::kernel::render::gui_fonts::font_definitions(&FontConfig::default()),
    );
    let all = [
        ("TERMINAL",  icons::TERMINAL),
        ("SEARCH",    icons::SEARCH),
        ("FILE",      icons::FILE),
        ("SYMBOL",    icons::SYMBOL),
        ("COMMAND",   icons::COMMAND),
        ("SETTINGS",  icons::SETTINGS),
        ("CLOSE",     icons::CLOSE),
        ("SEPARATOR", icons::SEPARATOR),
        ("GIT_BRANCH", icons::GIT_BRANCH),
        ("PROJECTS",  icons::PROJECTS),
        ("LEARN",     icons::LEARN),
        ("RUN",       icons::RUN),
        ("DEBUG",     icons::DEBUG),
        ("STOP",      icons::STOP),
        ("BUILD",     icons::BUILD),
        ("TEST",      icons::TEST),
        ("AGENT",     icons::AGENT),
        ("OUTPUT",    icons::OUTPUT),
        ("PAUSE",     icons::PAUSE),
        ("STEP_OVER", icons::STEP_OVER),
        ("STEP_IN",   icons::STEP_IN),
        ("STEP_OUT",  icons::STEP_OUT),
        ("RESTART",   icons::RESTART),
        ("RESET",     icons::RESET),
        ("CARET_RIGHT", icons::CARET_RIGHT),
        ("CARET_DOWN",  icons::CARET_DOWN),
    ];
    for (name, icon) in all {
        for ch in icon.chars() {
            assert!(
                fonts.has_glyph(&FontId::proportional(13.0), ch),
                "icons::{name} contains {ch:?} (U+{:04X}), which is missing from \
                 the bundled font chain and renders as tofu (◻)",
                ch as u32,
            );
        }
    }
}

#[test]
fn every_sidebar_glyph_is_covered_by_bundled_fonts() {
    use crate::kernel::render::gui_sidebar::{FALLBACK_FILE, ARROW_CLOSED, ARROW_OPEN};
    use crate::kernel::state::FontConfig;
    use eframe::egui::epaint::text::{FontId, Fonts, TextOptions};

    let mut fonts = Fonts::new(
        TextOptions { max_texture_side: 4096, ..Default::default() },
        crate::kernel::render::gui_fonts::font_definitions(&FontConfig::default()),
    );
    let glyphs = [
        ("ARROW_OPEN", ARROW_OPEN),
        ("ARROW_CLOSED", ARROW_CLOSED),
        ("FALLBACK_FILE", FALLBACK_FILE),
    ];
    for (source, glyph) in glyphs {
        for ch in glyph.chars() {
            assert!(
                fonts.has_glyph(&FontId::proportional(13.0), ch),
                "sidebar glyph {source} is {ch:?} (U+{:04X}), which is missing from \
                 the bundled font chain and renders as tofu (◻)",
                ch as u32,
            );
        }
    }
}

// ── Project explorer tree construction ───────────────────────────────────────

#[test]
fn sidebar_tree_roots_at_the_project_not_the_filesystem() {
    use crate::kernel::render::gui_sidebar_tree;

    let files = vec![
        "src/main.rs".to_string(),
        "src/kernel/state.rs".to_string(),
        "README.md".to_string(),
    ];
    let tree = gui_sidebar_tree::build(&files);

    let top: Vec<&str> = tree.iter().map(|n| n.name.as_str()).collect();
    assert_eq!(top, vec!["src", "README.md"],
        "top-level nodes are the project's own entries, not Users/me/… chain");

    let src = tree.iter().find(|n| n.name == "src").unwrap();
    assert!(src.is_dir);
    assert_eq!(src.path, "src", "directory node paths are project-relative");
    let main = src.children.iter().find(|n| n.name == "main.rs").unwrap();
    assert!(!main.is_dir);
    assert_eq!(main.path, "src/main.rs",
        "file node paths are project-relative (the click handler joins the root once)");
}

#[test]
fn sidebar_state_resets_when_the_project_changes() {
    use crate::kernel::render::gui_sidebar_state::{PendingOp, SidebarState};

    let mut state = SidebarState::default();
    state.reset_for("/old/project".to_string());
    state.expanded_dirs.insert("src".to_string());
    state.selected = Some("src/main.rs".to_string());
    state.pending = Some(PendingOp::NewFile { dir: "/old/project".to_string() });
    state.confirm_delete = Some("/old/project/src".to_string());

    state.reset_for("/new/project".to_string());
    assert!(state.expanded_dirs.is_empty(), "expansions from the old project are gone");
    assert_eq!(state.selected, None);
    assert!(state.pending.is_none());
    assert!(state.confirm_delete.is_none());
    assert_eq!(state.root.as_deref(), Some("/new/project"));
}

#[test]
fn close_editor_session_ends_the_project_and_its_terminals() {
    use crate::kernel::render::gui_shell::close_editor_session;
    use crate::kernel::runtime::BackgroundEvent;

    let mut ed = crate::tests::helpers::make_editor();
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel::<BackgroundEvent>();
    ed.io.terminals
        .spawn("sh", &[], &std::env::temp_dir(), tx)
        .expect("spawn terminal");

    crate::kernel::project::open_root(&mut ed, "/tmp/some-project");
    assert!(ed.project_manager.project.root.is_some());
    assert_eq!(ed.io.terminals.sessions.len(), 1);

    close_editor_session(&mut ed);

    assert!(ed.project_manager.project.root.is_none(),
        "project must end with the editor or welcome re-opens it immediately");
    assert_eq!(ed.project_manager.current_project, None);
    assert!(ed.io.terminals.sessions.is_empty(), "terminal sessions are killed on editor close");
    assert_eq!(ed.io.terminals.focused, None);
}

#[test]
fn ctrl_space_translates_to_a_keymap_key() {
    use crate::kernel::render::gui_input;
    use eframe::egui::{Event, Key, Modifiers};

    let ev = Event::Key {
        key: Key::Space,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers { ctrl: true, ..Default::default() },
    };
    assert_eq!(gui_input::translate_event(&ev), Some("ctrl-space".to_string()),
        "ctrl-space must reach the keymap (completion-trigger's binding)");
}

#[test]
fn arrow_chords_carry_a_modifier_prefix() {
    use crate::kernel::render::gui_input;
    use eframe::egui::{Event, Key, Modifiers};

    let key = |key: Key, modifiers: Modifiers| Event::Key {
        key, physical_key: None, pressed: true, repeat: false, modifiers,
    };
    let ctrl_shift = Modifiers { ctrl: true, shift: true, ..Default::default() };
    let ctrl_only  = Modifiers { ctrl: true, ..Default::default() };
    let shift_only = Modifiers { shift: true, ..Default::default() };

    // ctrl/meta arrows gain a prefix so horizontal-scroll chords exist.
    assert_eq!(gui_input::translate_event(&key(Key::ArrowLeft, ctrl_shift)),
        Some("ctrl-shift-left".to_string()));
    assert_eq!(gui_input::translate_event(&key(Key::ArrowRight, ctrl_only)),
        Some("ctrl-right".to_string()));
    // Plain and shift-only forms are unchanged.
    assert_eq!(gui_input::translate_event(&key(Key::ArrowLeft, Modifiers::default())),
        Some("left".to_string()));
    assert_eq!(gui_input::translate_event(&key(Key::ArrowLeft, shift_only)),
        Some("shift-left".to_string()));
}
