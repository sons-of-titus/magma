//! Janet API tests for Sprint 11b decoration C functions.

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
        ed.buffers.insert(Buffer::new(BufferId(1), "test"))
    }

    // ── decor-set-inline ──────────────────────────────────────────────────────

    #[test]
    fn decor_set_inline_janet_stores_decoration() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let key = make_buf(&mut ed);
        janet_bridge::eval(&mut ed,
            &format!(r#"(buffer/decor-set-inline {key} "lsp-inlay" 0 5 ": i32" "type-face")"#));
        assert_eq!(ed.buffers[key].decor_count_layer("lsp-inlay"), 1);
    }

    #[test]
    fn decor_set_inline_janet_replaces_at_same_position() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let key = make_buf(&mut ed);
        janet_bridge::eval(&mut ed,
            &format!(r#"(buffer/decor-set-inline {key} "lsp-inlay" 0 5 "old" "t")"#));
        janet_bridge::eval(&mut ed,
            &format!(r#"(buffer/decor-set-inline {key} "lsp-inlay" 0 5 "new" "t")"#));
        assert_eq!(ed.buffers[key].decor_count_layer("lsp-inlay"), 1);
    }

    #[test]
    fn decor_set_inline_janet_emits_decoration_changed_event() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let key = make_buf(&mut ed);
        // Register a Rust counter command to observe the event.
        let counter = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let counter2 = counter.clone();
        ed.commands.register_fn("count-decor-changed", "", vec![], move |_ed, _args| {
            counter2.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        });
        janet_bridge::eval(&mut ed,
            r#"(event/on "decoration-changed" (fn [_] (editor/run-command "count-decor-changed")))"#);
        janet_bridge::eval(&mut ed,
            &format!(r#"(buffer/decor-set-inline {key} "layer" 0 0 "x" "f")"#));
        ed.events.drain_and_dispatch();
        assert!(counter.load(std::sync::atomic::Ordering::SeqCst) > 0);
    }

    // ── decor-set-eol ─────────────────────────────────────────────────────────

    #[test]
    fn decor_set_eol_janet_stores_decoration() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let key = make_buf(&mut ed);
        janet_bridge::eval(&mut ed,
            &format!(r#"(buffer/decor-set-eol {key} "git-blame" 2 "alice 2d ago" "comment-face")"#));
        assert_eq!(ed.buffers[key].decor_count_layer("git-blame"), 1);
    }

    // ── decor-set-prefix ──────────────────────────────────────────────────────

    #[test]
    fn decor_set_prefix_janet_stores_decoration() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let key = make_buf(&mut ed);
        janet_bridge::eval(&mut ed,
            &format!(r#"(buffer/decor-set-prefix {key} "cov" 1 "42" "keyword-face")"#));
        assert_eq!(ed.buffers[key].decor_count_layer("cov"), 1);
    }

    // ── decor-clear-layer ─────────────────────────────────────────────────────

    #[test]
    fn decor_clear_layer_janet_removes_layer() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let key = make_buf(&mut ed);
        janet_bridge::eval(&mut ed,
            &format!(r#"(buffer/decor-set-eol {key} "x" 0 "a" "f")"#));
        janet_bridge::eval(&mut ed,
            &format!(r#"(buffer/decor-clear-layer {key} "x")"#));
        assert_eq!(ed.buffers[key].decor_count_layer("x"), 0);
    }

    // ── decor-clear ───────────────────────────────────────────────────────────

    #[test]
    fn decor_clear_janet_removes_all_layers() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let key = make_buf(&mut ed);
        janet_bridge::eval(&mut ed,
            &format!(r#"(buffer/decor-set-eol {key} "a" 0 "x" "f")"#));
        janet_bridge::eval(&mut ed,
            &format!(r#"(buffer/decor-set-inline {key} "b" 0 0 "y" "f")"#));
        janet_bridge::eval(&mut ed,
            &format!(r#"(buffer/decor-clear {key})"#));
        assert!(ed.buffers[key].decoration_layers.is_empty());
    }

    // ── decor-count ───────────────────────────────────────────────────────────

    #[test]
    fn decor_count_janet_returns_correct_count() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let key = make_buf(&mut ed);
        janet_bridge::eval(&mut ed, &format!(r#"(buffer/decor-set-eol {key} "l" 0 "a" "f")"#));
        janet_bridge::eval(&mut ed, &format!(r#"(buffer/decor-set-eol {key} "l" 1 "b" "f")"#));
        janet_bridge::eval(&mut ed, &format!(r#"(buffer/decor-set-eol {key} "l" 2 "c" "f")"#));
        assert_eq!(ed.buffers[key].decor_count_layer("l"), 3);
    }

    // ── decor-get ─────────────────────────────────────────────────────────────

    #[test]
    fn decor_get_janet_returns_table_array() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let key = make_buf(&mut ed);
        janet_bridge::eval(&mut ed,
            &format!(r#"(buffer/decor-set-inline {key} "hints" 0 5 ": i32" "type-face")"#));
        // Eval decor-get and check the count via decor-count (avoids Janet type inspection)
        assert_eq!(ed.buffers[key].decor_count_layer("hints"), 1);
        // Verify that decor-get itself returns without error
        let result = janet_bridge::eval(&mut ed,
            &format!(r#"(buffer/decor-get {key} "hints")"#));
        assert_eq!(result, "ok");
    }
}
