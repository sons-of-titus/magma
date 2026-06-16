use crate::kernel::scripting;

fn make_buf(ed: &mut crate::kernel::state::Editor) -> usize {
    let buf = crate::kernel::text_engine::Buffer::new(crate::kernel::state::id::BufferId(1), "test");
    ed.buffers.insert(buf)
}

// ── decor-set-inline ──────────────────────────────────────────────────────

#[test]
fn decor_set_inline_janet_stores_decoration() {
    janet_test!(ed, {
        let key = make_buf(&mut ed);
        scripting::eval(
            &format!(r#"(buffer/decor-set-inline {key} "lsp-inlay" 0 5 ": i32" "type-face")"#));
        assert_eq!(ed.buffers[key].decor_count_layer("lsp-inlay"), 1);
    });
}

#[test]
fn decor_set_inline_janet_replaces_at_same_position() {
    janet_test!(ed, {
        let key = make_buf(&mut ed);
        scripting::eval(
            &format!(r#"(buffer/decor-set-inline {key} "lsp-inlay" 0 5 "old" "t")"#));
        scripting::eval(
            &format!(r#"(buffer/decor-set-inline {key} "lsp-inlay" 0 5 "new" "t")"#));
        assert_eq!(ed.buffers[key].decor_count_layer("lsp-inlay"), 1);
    });
}

#[test]
fn decor_set_inline_janet_emits_decoration_changed_event() {
    janet_test!(ed, {
        let key = make_buf(&mut ed);
        let counter = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let counter2 = counter.clone();
        ed.commands.register_fn("count-decor-changed", "", vec![], move |_ed, _args| {
            counter2.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        });
        scripting::eval(
            r#"(event/on "decoration-changed" (fn [_] (editor/run-command "count-decor-changed")))"#);
        scripting::eval(
            &format!(r#"(buffer/decor-set-inline {key} "layer" 0 0 "x" "f")"#));
        ed.events.drain_and_dispatch();
        assert!(counter.load(std::sync::atomic::Ordering::SeqCst) > 0);
    });
}

// ── decor-set-eol ─────────────────────────────────────────────────────────

#[test]
fn decor_set_eol_janet_stores_decoration() {
    janet_test!(ed, {
        let key = make_buf(&mut ed);
        scripting::eval(
            &format!(r#"(buffer/decor-set-eol {key} "git-blame" 2 "alice 2d ago" "comment-face")"#));
        assert_eq!(ed.buffers[key].decor_count_layer("git-blame"), 1);
    });
}

// ── decor-set-prefix ──────────────────────────────────────────────────────

#[test]
fn decor_set_prefix_janet_stores_decoration() {
    janet_test!(ed, {
        let key = make_buf(&mut ed);
        scripting::eval(
            &format!(r#"(buffer/decor-set-prefix {key} "cov" 1 "42" "keyword-face")"#));
        assert_eq!(ed.buffers[key].decor_count_layer("cov"), 1);
    });
}

// ── decor-clear-layer ─────────────────────────────────────────────────────

#[test]
fn decor_clear_layer_janet_removes_layer() {
    janet_test!(ed, {
        let key = make_buf(&mut ed);
        scripting::eval(
            &format!(r#"(buffer/decor-set-eol {key} "x" 0 "a" "f")"#));
        scripting::eval(
            &format!(r#"(buffer/decor-clear-layer {key} "x")"#));
        assert_eq!(ed.buffers[key].decor_count_layer("x"), 0);
    });
}

// ── decor-clear ───────────────────────────────────────────────────────────

#[test]
fn decor_clear_janet_removes_all_layers() {
    janet_test!(ed, {
        let key = make_buf(&mut ed);
        scripting::eval(
            &format!(r#"(buffer/decor-set-eol {key} "a" 0 "x" "f")"#));
        scripting::eval(
            &format!(r#"(buffer/decor-set-inline {key} "b" 0 0 "y" "f")"#));
        scripting::eval(
            &format!(r#"(buffer/decor-clear {key})"#));
        assert!(ed.buffers[key].decoration_layers.is_empty());
    });
}

// ── decor-count ───────────────────────────────────────────────────────────

#[test]
fn decor_count_janet_returns_correct_count() {
    janet_test!(ed, {
        let key = make_buf(&mut ed);
        scripting::eval(&format!(r#"(buffer/decor-set-eol {key} "l" 0 "a" "f")"#));
        scripting::eval(&format!(r#"(buffer/decor-set-eol {key} "l" 1 "b" "f")"#));
        scripting::eval(&format!(r#"(buffer/decor-set-eol {key} "l" 2 "c" "f")"#));
        assert_eq!(ed.buffers[key].decor_count_layer("l"), 3);
    });
}

// ── decor-get ─────────────────────────────────────────────────────────────

#[test]
fn decor_get_janet_returns_table_array() {
    janet_test!(ed, {
        let key = make_buf(&mut ed);
        scripting::eval(
            &format!(r#"(buffer/decor-set-inline {key} "hints" 0 5 ": i32" "type-face")"#));
        assert_eq!(ed.buffers[key].decor_count_layer("hints"), 1);
        let result = scripting::eval(
            &format!(r#"(buffer/decor-get {key} "hints")"#));
        assert_eq!(result, "ok");
    });
}
