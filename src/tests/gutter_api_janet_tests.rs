//! Janet API tests for the named-column gutter system (Sprint 11c).

#[cfg(feature = "janet")]
mod tests {
    use crate::command::builtin;
    use crate::buffer::Buffer;
    use crate::fs::disk::DiskFileSystem;
    use crate::janet_bridge;
    use crate::state::Editor;
    use crate::state::id::BufferId;

    fn make_editor() -> Editor {
        let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
        builtin::register_builtin_commands(&mut ed);
        ed
    }

    fn make_buf(ed: &mut Editor) -> usize {
        ed.buffers.insert(Buffer::new(BufferId(1), "test.rs"))
    }

    // ── gutter/define-column ─────────────────────────────────────────────────

    #[test]
    fn define_column_registers_column() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        // gutter.janet already calls define-column for the built-in columns.
        assert!(!ed.gutter.columns.is_empty());
        let names: Vec<&str> = ed.gutter.columns.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&":line-numbers"));
    }

    #[test]
    fn define_column_preserves_insertion_order() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let names: Vec<&str> = ed.gutter.columns.iter().map(|c| c.name.as_str()).collect();
        let bp_pos   = names.iter().position(|&n| n == ":breakpoints").unwrap_or(usize::MAX);
        let ln_pos   = names.iter().position(|&n| n == ":line-numbers").unwrap_or(usize::MAX);
        let fold_pos = names.iter().position(|&n| n == ":folding").unwrap_or(usize::MAX);
        assert!(bp_pos < ln_pos, "breakpoints should come before line-numbers");
        assert!(ln_pos < fold_pos, "line-numbers should come before folding");
    }

    #[test]
    fn define_column_updates_existing() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        janet_bridge::eval(&mut ed, r#"(gutter/define-column ":test-col" 3 "my-face")"#);
        janet_bridge::eval(&mut ed, r#"(gutter/define-column ":test-col" 5 "other-face")"#);
        let col = ed.gutter.columns.iter().find(|c| c.name == ":test-col").unwrap();
        assert_eq!(col.width, 5);
        assert_eq!(col.face, "other-face");
        // Should not have duplicated the column.
        let count = ed.gutter.columns.iter().filter(|c| c.name == ":test-col").count();
        assert_eq!(count, 1);
    }

    // ── gutter/show-column / gutter/hide-column ───────────────────────────────

    #[test]
    fn hide_column_marks_invisible() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        janet_bridge::eval(&mut ed, r#"(gutter/hide-column ":line-numbers")"#);
        let col = ed.gutter.columns.iter().find(|c| c.name == ":line-numbers").unwrap();
        assert!(!col.visible);
    }

    #[test]
    fn show_column_marks_visible() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        janet_bridge::eval(&mut ed, r#"(gutter/hide-column ":line-numbers")"#);
        janet_bridge::eval(&mut ed, r#"(gutter/show-column ":line-numbers")"#);
        let col = ed.gutter.columns.iter().find(|c| c.name == ":line-numbers").unwrap();
        assert!(col.visible);
    }

    // ── gutter/set-column-face ────────────────────────────────────────────────

    #[test]
    fn set_column_face_updates_face() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        janet_bridge::eval(&mut ed, r#"(gutter/set-column-face ":diagnostics" "diag-face")"#);
        let col = ed.gutter.columns.iter().find(|c| c.name == ":diagnostics").unwrap();
        assert_eq!(col.face, "diag-face");
    }

    // ── gutter/column-list ────────────────────────────────────────────────────

    #[test]
    fn column_list_returns_all_columns() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let result = janet_bridge::eval(&mut ed, "(length (gutter/column-list))");
        // Should return "ok" (the eval doesn't capture the return value as a string, but at least not error)
        assert_eq!(result, "ok");
        assert!(!ed.gutter.columns.is_empty());
    }

    // ── gutter/sign-set ───────────────────────────────────────────────────────

    #[test]
    fn sign_set_stores_sign_in_column() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let key = make_buf(&mut ed);
        janet_bridge::eval(&mut ed,
            &format!(r#"(gutter/sign-set ":diagnostics" {key} 5 "E" "error-face" 10)"#));
        let col_key = (":diagnostics".to_string(), key);
        assert!(ed.gutter.column_signs.contains_key(&col_key));
        let signs = &ed.gutter.column_signs[&col_key][&5];
        assert_eq!(signs.len(), 1);
        assert_eq!(signs[0].text, "E");
        assert_eq!(signs[0].priority, 10);
    }

    #[test]
    fn sign_set_replaces_same_priority() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let key = make_buf(&mut ed);
        janet_bridge::eval(&mut ed,
            &format!(r#"(gutter/sign-set ":diagnostics" {key} 0 "W" "warning-face" 5)"#));
        janet_bridge::eval(&mut ed,
            &format!(r#"(gutter/sign-set ":diagnostics" {key} 0 "E" "error-face" 5)"#));
        let col_key = (":diagnostics".to_string(), key);
        let signs = &ed.gutter.column_signs[&col_key][&0];
        assert_eq!(signs.len(), 1, "same priority should replace");
        assert_eq!(signs[0].text, "E");
    }

    #[test]
    fn sign_set_emits_gutter_sign_changed_event() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let key = make_buf(&mut ed);
        let counter = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let counter2 = counter.clone();
        ed.commands.register_fn("count-sign-changed", "", vec![], move |_ed, _args| {
            counter2.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        });
        janet_bridge::eval(&mut ed,
            r#"(event/on "gutter-sign-changed" (fn [_] (editor/run-command "count-sign-changed")))"#);
        janet_bridge::eval(&mut ed,
            &format!(r#"(gutter/sign-set ":vcs" {key} 2 "▎" "gutter-vcs-changed" 50)"#));
        ed.events.drain_and_dispatch();
        assert!(counter.load(std::sync::atomic::Ordering::SeqCst) > 0);
    }

    // ── gutter/sign-clear ─────────────────────────────────────────────────────

    #[test]
    fn sign_clear_removes_all_signs_in_column() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let key = make_buf(&mut ed);
        janet_bridge::eval(&mut ed,
            &format!(r#"(gutter/sign-set ":vcs" {key} 0 "▎" "f" 1)"#));
        janet_bridge::eval(&mut ed,
            &format!(r#"(gutter/sign-set ":vcs" {key} 1 "▎" "f" 1)"#));
        janet_bridge::eval(&mut ed,
            &format!(r#"(gutter/sign-clear ":vcs" {key})"#));
        assert!(!ed.gutter.column_signs.contains_key(&(":vcs".to_string(), key)));
    }

    // ── gutter/sign-clear-line ────────────────────────────────────────────────

    #[test]
    fn sign_clear_line_removes_signs_on_one_line() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let key = make_buf(&mut ed);
        janet_bridge::eval(&mut ed,
            &format!(r#"(gutter/sign-set ":vcs" {key} 0 "▎" "f" 1)"#));
        janet_bridge::eval(&mut ed,
            &format!(r#"(gutter/sign-set ":vcs" {key} 1 "▎" "f" 1)"#));
        janet_bridge::eval(&mut ed,
            &format!(r#"(gutter/sign-clear-line ":vcs" {key} 0)"#));
        let col_key = (":vcs".to_string(), key);
        let line_map = &ed.gutter.column_signs[&col_key];
        assert!(!line_map.contains_key(&0));
        assert!(line_map.contains_key(&1));
    }

    // ── gutter/signs ──────────────────────────────────────────────────────────

    #[test]
    fn signs_returns_sign_list_without_error() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let key = make_buf(&mut ed);
        janet_bridge::eval(&mut ed,
            &format!(r#"(gutter/sign-set ":diagnostics" {key} 3 "E" "error-face" 10)"#));
        let result = janet_bridge::eval(&mut ed,
            &format!(r#"(gutter/signs ":diagnostics" {key})"#));
        assert_eq!(result, "ok");
    }

    // ── gutter/set-line-number-format ─────────────────────────────────────────

    #[test]
    fn set_line_number_format_stores_fn_name() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        janet_bridge::eval(&mut ed, r#"(gutter/set-line-number-format "gutter-lnum-relative")"#);
        assert_eq!(ed.gutter.line_number_fn.as_deref(), Some("gutter-lnum-relative"));
    }

    #[test]
    fn set_line_number_format_nil_clears_fn() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        janet_bridge::eval(&mut ed, r#"(gutter/set-line-number-format "gutter-lnum-absolute")"#);
        janet_bridge::eval(&mut ed, r#"(gutter/set-line-number-format nil)"#);
        assert!(ed.gutter.line_number_fn.is_none());
    }

    // ── gutter/set-fold-icons ─────────────────────────────────────────────────

    #[test]
    fn set_fold_icons_stores_icons() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        janet_bridge::eval(&mut ed, r#"(gutter/set-fold-icons "v" ">" "my-fold-face")"#);
        assert_eq!(ed.gutter.fold_icons.open, "v");
        assert_eq!(ed.gutter.fold_icons.closed, ">");
        assert_eq!(ed.gutter.fold_icons.face, "my-fold-face");
    }

    #[test]
    fn set_fold_icons_defaults_from_gutter_janet() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        // gutter.janet sets fold icons on init
        assert_eq!(ed.gutter.fold_icons.open, "▾");
        assert_eq!(ed.gutter.fold_icons.closed, "▸");
    }
}
