#[macro_use]
mod helpers;
use kernel::input::dispatch_key;
use scripting;

#[test]
fn on_input_fn_starts_nil() {
    janet_test!(ed, {
        assert_eq!(ed.on_input_fn.as_deref(), None);
    });
}

#[test]
fn on_input_fn_registration() {
    janet_test!(ed, {
        scripting::eval(r#"(defn my-interceptor [k] nil) (editor/on-input "my-interceptor")"#);
        assert_eq!(ed.on_input_fn.as_deref(), Some("my-interceptor"));
    });
}

#[test]
fn on_input_interceptor_consuming_key() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor_with_buffer("hello");
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);
    scripting::eval(r#"(defn eat-all [k] (editor/consume-input)) (editor/on-input "eat-all")"#);
    ed.keymaps.set_layer("editor", "x", "delete-char");
    let buf_id = ed
        .view_tree
        .focused_window()
        .and_then(|wid| ed.view_tree.buffer(wid))
        .unwrap();
    ed.views.get_mut(&buf_id).unwrap().set_cursor(0);
    dispatch_key(&mut ed, "x");
    let text = {
        let b = ed.buffers.get(buf_id).unwrap().lock().unwrap();
        b.slice(0, b.len())
    };
    assert_eq!(text, "hello", "interceptor should have consumed the key");
}

#[test]
fn on_input_interceptor_passing_key() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor_with_buffer("hello");
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);
    scripting::eval(r#"(defn pass-all [k] nil) (editor/on-input "pass-all")"#);
    ed.keymaps.set_layer("editor", "x", "delete-char");
    let buf_id = ed
        .view_tree
        .focused_window()
        .and_then(|wid| ed.view_tree.buffer(wid))
        .unwrap();
    ed.views.get_mut(&buf_id).unwrap().set_cursor(0);
    dispatch_key(&mut ed, "x");
    let text = {
        let b = ed.buffers.get(buf_id).unwrap().lock().unwrap();
        b.slice(0, b.len())
    };
    assert_eq!(text, "ello", "key should have been dispatched normally");
}

#[test]
fn on_input_clear_with_nil() {
    janet_test!(ed, {
        scripting::eval(r#"(defn some-fn [k] (editor/consume-input)) (editor/on-input "some-fn")"#);
        assert!(ed.on_input_fn.is_some());
        scripting::eval("(editor/on-input nil)");
        assert!(ed.on_input_fn.is_none());
    });
}

#[test]
fn input_consumed_flag_cleared_each_dispatch() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor_with_buffer("ab");
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);
    scripting::eval(
        r#"(defn selective [k] (when (= k "a") (editor/consume-input))) (editor/on-input "selective")"#,
    );
    ed.keymaps.set_layer("editor", "a", "append");
    ed.keymaps.set_layer("editor", "x", "delete-char");
    let buf_id = ed
        .view_tree
        .focused_window()
        .and_then(|wid| ed.view_tree.buffer(wid))
        .unwrap();
    ed.views.get_mut(&buf_id).unwrap().set_cursor(0);
    dispatch_key(&mut ed, "a");
    dispatch_key(&mut ed, "x");
    let text = {
        let b = ed.buffers.get(buf_id).unwrap().lock().unwrap();
        b.slice(0, b.len())
    };
    assert_eq!(text, "b", "first char should have been deleted by x");
}
