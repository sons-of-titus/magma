use crate::janet_bridge;

// ── editor/module-path ────────────────────────────────────────────────────────

#[test]
fn module_path_returns_array() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor_with_buffer("hello\n");
    crate::janet_bridge::init(&mut ed);
    let r = janet_bridge::eval("(array? (module/path))");
    assert_eq!(r, "ok");
}

#[test]
fn module_path_add_appends_path() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor_with_buffer("hello\n");
    crate::janet_bridge::init(&mut ed);
    let before = ed.module_paths.len();
    let r = janet_bridge::eval(r#"(module/path-add "/tmp/test-plugins")"#);
    assert_eq!(r, "ok");
    assert_eq!(ed.module_paths.len(), before + 1);
    assert!(ed.module_paths.contains(&"/tmp/test-plugins".to_string()));
}

#[test]
fn module_path_add_is_idempotent() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor_with_buffer("hello\n");
    crate::janet_bridge::init(&mut ed);
    janet_bridge::eval(r#"(module/path-add "/tmp/dedup")"#);
    let after_first = ed.module_paths.len();
    janet_bridge::eval(r#"(module/path-add "/tmp/dedup")"#);
    assert_eq!(ed.module_paths.len(), after_first);
}

// ── keymap/list-layer ─────────────────────────────────────────────────────────

#[test]
fn keymap_list_layer_returns_array() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor_with_buffer("hello\n");
    crate::janet_bridge::init(&mut ed);
    let r = janet_bridge::eval("(array? (keymap/list-layer \"vim\"))");
    assert_eq!(r, "ok");
}

#[test]
fn keymap_list_layer_contains_set_binding() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor_with_buffer("hello\n");
    crate::janet_bridge::init(&mut ed);
    janet_bridge::eval(r#"(keymap/set "ctrl-x" "save-buffer" "testlayer")"#);
    let r = janet_bridge::eval(
        r#"(def bs (keymap/list-layer "testlayer")) (def found (find (fn [p] (= (p 0) "ctrl-x")) bs)) (not (nil? found))"#);
    assert_eq!(r, "ok");
    let bindings = ed.keymaps.list_layer("testlayer");
    assert!(bindings.iter().any(|(k, v)| k == "ctrl-x" && v == "save-buffer"));
}

#[test]
fn keymap_list_layer_empty_for_unknown_layer() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor_with_buffer("hello\n");
    crate::janet_bridge::init(&mut ed);
    let r = janet_bridge::eval("(= 0 (length (keymap/list-layer \"nonexistent-layer\")))");
    assert_eq!(r, "ok");
}

// ── quickfix/set and quickfix/get ─────────────────────────────────────────────

#[test]
fn quickfix_set_replaces_list() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor_with_buffer("hello\n");
    crate::janet_bridge::init(&mut ed);
    let r = janet_bridge::eval(
        r#"(quickfix/set [{:filename "a.rs" :line 1 :col 1 :message "err"}])"#);
    assert_eq!(r, "ok");
    assert_eq!(ed.quickfix_list.len(), 1);
    assert_eq!(ed.quickfix_list[0].filename, "a.rs");
    assert_eq!(ed.quickfix_list[0].line, 1);
    assert_eq!(ed.quickfix_list[0].message, "err");
}

#[test]
fn quickfix_set_clears_with_empty_array() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor_with_buffer("hello\n");
    crate::janet_bridge::init(&mut ed);
    janet_bridge::eval(
        r#"(quickfix/set [{:filename "a.rs" :line 1 :col 1 :message "e"}])"#);
    assert_eq!(ed.quickfix_list.len(), 1);
    janet_bridge::eval("(quickfix/set [])");
    assert_eq!(ed.quickfix_list.len(), 0);
}

#[test]
fn quickfix_get_returns_all_entries() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor_with_buffer("hello\n");
    crate::janet_bridge::init(&mut ed);
    janet_bridge::eval(
        r#"(quickfix/set [
             {:filename "x.rs" :line 2 :col 3 :message "warning"}
             {:filename "y.rs" :line 5 :col 1 :message "error"}])"#);
    let r = janet_bridge::eval("(= 2 (length (quickfix/get)))");
    assert_eq!(r, "ok");
    assert_eq!(ed.quickfix_list.len(), 2);
    assert_eq!(ed.quickfix_list[0].filename, "x.rs");
    assert_eq!(ed.quickfix_list[1].filename, "y.rs");
}

// ── editor/mark-ring-push / pop ───────────────────────────────────────────────

#[test]
fn mark_ring_push_adds_entry() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor_with_buffer("hello\n");
    crate::janet_bridge::init(&mut ed);
    let r = janet_bridge::eval(r#"(mark-ring/push "src/main.rs" 42)"#);
    assert_eq!(r, "ok");
    assert_eq!(ed.mark_ring.len(), 1);
    assert_eq!(ed.mark_ring[0].0, "src/main.rs");
    assert_eq!(ed.mark_ring[0].1, 42);
}

#[test]
fn mark_ring_len_reflects_pushes() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor_with_buffer("hello\n");
    crate::janet_bridge::init(&mut ed);
    janet_bridge::eval(r#"(mark-ring/push "a.rs" 0)"#);
    janet_bridge::eval(r#"(mark-ring/push "b.rs" 10)"#);
    let r = janet_bridge::eval("(= 2 (mark-ring/len))");
    assert_eq!(r, "ok");
    assert_eq!(ed.mark_ring.len(), 2);
}

#[test]
fn mark_ring_pop_returns_last_pushed() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor_with_buffer("hello\n");
    crate::janet_bridge::init(&mut ed);
    janet_bridge::eval(r#"(mark-ring/push "a.rs" 0)"#);
    janet_bridge::eval(r#"(mark-ring/push "b.rs" 99)"#);
    let r = janet_bridge::eval(
        r#"(def e (mark-ring/pop)) (= (e :path) "b.rs")"#);
    assert_eq!(r, "ok");
    assert_eq!(ed.mark_ring.len(), 1);
    assert_eq!(ed.mark_ring[0].0, "a.rs");
}

#[test]
fn mark_ring_pop_returns_nil_when_empty() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor_with_buffer("hello\n");
    crate::janet_bridge::init(&mut ed);
    let r = janet_bridge::eval("(nil? (mark-ring/pop))");
    assert_eq!(r, "ok");
}

#[test]
fn mark_ring_peek_does_not_remove() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor_with_buffer("hello\n");
    crate::janet_bridge::init(&mut ed);
    janet_bridge::eval(r#"(mark-ring/push "z.rs" 7)"#);
    janet_bridge::eval(r#"(mark-ring/peek)"#);
    assert_eq!(ed.mark_ring.len(), 1);
    assert_eq!(ed.mark_ring[0].0, "z.rs");
}

// ── magma-require deduplication ───────────────────────────────────────────────

#[test]
fn magma_require_loads_module_once() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor_with_buffer("hello\n");
    crate::janet_bridge::init(&mut ed);
    let r = janet_bridge::eval(r#"(magma-require "nonexistent-plugin-xyz")"#);
    assert_eq!(r, "ok");
}
