use crate::buffer::Buffer;
use crate::command::builtin;
use crate::fs::disk::DiskFileSystem;
use crate::janet_bridge;
use crate::state::id::BufferId;
use crate::state::Editor;

fn make_editor() -> Editor {
    let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
    builtin::register_builtin_commands(&mut ed);
    let id = ed.allocate_buffer_id();
    let buf = Buffer::from_string(BufferId(id), "test", "");
    let entry = ed.buffers.vacant_entry();
    let key = entry.key();
    entry.insert(buf);
    if let Some(win) = ed.windows.focused_window_mut() {
        win.buffer_id = Some(key);
    }
    ed
}

// ── editor/set-font ───────────────────────────────────────────────────────────

#[test]
fn editor_set_font_updates_family_and_size() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor();
    janet_bridge::init(&mut ed);

    let result = janet_bridge::eval(&mut ed, r#"(font/set "JetBrains Mono" 16)"#);
    assert_eq!(result, "ok");
    assert_eq!(ed.font_config.family, "JetBrains Mono");
    assert!((ed.font_config.size - 16.0).abs() < f32::EPSILON);
}

#[test]
fn editor_set_font_emits_font_changed() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor();
    janet_bridge::init(&mut ed);

    janet_bridge::eval(&mut ed, r#"(font/set "Hack" 14)"#);
    assert!(ed.font_changed);
}

// ── editor/font-size / editor/set-font-size ───────────────────────────────────

#[test]
fn editor_set_font_size_updates_size() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor();
    janet_bridge::init(&mut ed);

    let result = janet_bridge::eval(&mut ed, "(font/set-size 20)");
    assert_eq!(result, "ok");
    assert!((ed.font_config.size - 20.0).abs() < f32::EPSILON);
}

#[test]
fn editor_font_size_returns_current() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor();
    janet_bridge::init(&mut ed);

    janet_bridge::eval(&mut ed, "(font/set-size 18)");
    assert!((ed.font_config.size - 18.0).abs() < f32::EPSILON);
}

#[test]
fn editor_set_font_size_clamps_below_4() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor();
    janet_bridge::init(&mut ed);

    janet_bridge::eval(&mut ed, "(font/set-size 1)");
    assert!(ed.font_config.size >= 4.0);
}

// ── editor/set-ligatures ─────────────────────────────────────────────────────

#[test]
fn editor_set_ligatures_true() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor();
    janet_bridge::init(&mut ed);

    let result = janet_bridge::eval(&mut ed, "(font/set-ligatures true)");
    assert_eq!(result, "ok");
    assert!(ed.font_config.ligatures);
}

#[test]
fn editor_set_ligatures_false_after_true() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor();
    janet_bridge::init(&mut ed);

    janet_bridge::eval(&mut ed, "(font/set-ligatures true)");
    let result = janet_bridge::eval(&mut ed, "(font/set-ligatures false)");
    assert_eq!(result, "ok");
    assert!(!ed.font_config.ligatures);
}

// ── editor/set-glyph-width ───────────────────────────────────────────────────

#[test]
fn editor_set_glyph_width_single_char_string() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor();
    janet_bridge::init(&mut ed);

    let result = janet_bridge::eval(&mut ed, r#"(font/set-glyph-width "A" 2)"#);
    assert_eq!(result, "ok");
    assert_eq!(ed.font_config.glyph_widths.get(&'A'), Some(&2));
}

#[test]
fn editor_set_glyph_width_codepoint_integer() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor();
    janet_bridge::init(&mut ed);

    // 0xE000 = 57344
    let result = janet_bridge::eval(&mut ed, "(font/set-glyph-width 57344 2)");
    assert_eq!(result, "ok");
    assert_eq!(ed.font_config.glyph_widths.get(&'\u{E000}'), Some(&2));
}

// ── editor/set-glyph-width-range ─────────────────────────────────────────────

#[test]
fn editor_set_glyph_width_range_populates_chars() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor();
    janet_bridge::init(&mut ed);

    // 65='A', 67='C'
    let result = janet_bridge::eval(&mut ed, "(font/set-glyph-width-range 65 67 2)");
    assert_eq!(result, "ok");
    assert_eq!(ed.font_config.glyph_widths.get(&'A'), Some(&2));
    assert_eq!(ed.font_config.glyph_widths.get(&'B'), Some(&2));
    assert_eq!(ed.font_config.glyph_widths.get(&'C'), Some(&2));
    assert_eq!(ed.font_config.glyph_widths.get(&'D'), None);
}

// ── editor/set-nerd-font ─────────────────────────────────────────────────────

#[test]
fn editor_set_nerd_font_true_populates_pua_range() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor();
    janet_bridge::init(&mut ed);

    let result = janet_bridge::eval(&mut ed, "(font/set-nerd-font true)");
    assert_eq!(result, "ok");
    assert_eq!(ed.font_config.glyph_widths.get(&'\u{E000}'), Some(&2));
    assert_eq!(ed.font_config.glyph_widths.get(&'\u{F8FF}'), Some(&2));
    assert_eq!(ed.font_config.glyph_widths.get(&'\u{2665}'), Some(&2));
}

#[test]
fn editor_set_nerd_font_false_clears_overrides() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor();
    janet_bridge::init(&mut ed);

    janet_bridge::eval(&mut ed, "(font/set-nerd-font true)");
    let result = janet_bridge::eval(&mut ed, "(font/set-nerd-font false)");
    assert_eq!(result, "ok");
    assert_eq!(ed.font_config.glyph_widths.get(&'\u{E000}'), None);
}

// ── editor/set-font-fallback ─────────────────────────────────────────────────

#[test]
fn editor_set_font_fallback_updates_list() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor();
    janet_bridge::init(&mut ed);

    let result = janet_bridge::eval(&mut ed, r#"(font/set-fallback ["NerdFont" "Emoji"])"#);
    assert_eq!(result, "ok");
    assert_eq!(ed.font_config.fallback, vec!["NerdFont", "Emoji"]);
}

// ── editor/set-font-context ───────────────────────────────────────────────────

#[test]
fn editor_set_font_context_stores_override() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor();
    janet_bridge::init(&mut ed);

    let result = janet_bridge::eval(
        &mut ed,
        r#"(font/set-context "prose" {:family "Georgia" :size 18})"#,
    );
    assert_eq!(result, "ok");
    let ctx = ed.font_config.context_overrides.get("prose").unwrap();
    assert_eq!(ctx.family.as_deref(), Some("Georgia"));
    assert_eq!(ctx.size, Some(18.0));
}
