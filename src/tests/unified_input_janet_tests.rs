use crate::janet_bridge;
use crate::input::dispatch_key;

#[test]
fn on_input_fn_starts_nil() {
    janet_test!(ed, {
        assert_eq!(ed.on_input_fn.as_deref(), Some("vim-on-input"));
    });
}

#[test]
fn on_input_fn_registration() {
    janet_test!(ed, {
        janet_bridge::eval(
            r#"(defn my-interceptor [k] nil) (editor/on-input "my-interceptor")"#);
        assert_eq!(ed.on_input_fn.as_deref(), Some("my-interceptor"));
    });
}

#[test]
fn on_input_interceptor_consuming_key() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor_with_buffer("hello");
    crate::janet_bridge::init(&mut ed);
    janet_bridge::eval(
        r#"(defn eat-all [k] (editor/consume-input)) (editor/on-input "eat-all")"#);
    ed.keymaps.set_layer("vim", "x", "delete-char");
    let buf_id = ed.windows.focused_window()
        .and_then(|wid| ed.windows.buffer(wid)).unwrap();
    ed.buffers.get_mut(buf_id).unwrap().set_cursor(0);
    dispatch_key(&mut ed, "x");
    let text = ed.buffers.get(buf_id).unwrap()
        .slice(0, ed.buffers.get(buf_id).unwrap().len());
    assert_eq!(text, "hello", "interceptor should have consumed the key");
}

#[test]
fn on_input_interceptor_passing_key() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor_with_buffer("hello");
    crate::janet_bridge::init(&mut ed);
    janet_bridge::eval(
        r#"(defn pass-all [k] nil) (editor/on-input "pass-all")"#);
    ed.keymaps.set_layer("vim", "x", "delete-char");
    let buf_id = ed.windows.focused_window()
        .and_then(|wid| ed.windows.buffer(wid)).unwrap();
    ed.buffers.get_mut(buf_id).unwrap().set_cursor(0);
    dispatch_key(&mut ed, "x");
    let text = ed.buffers.get(buf_id).unwrap()
        .slice(0, ed.buffers.get(buf_id).unwrap().len());
    assert_eq!(text, "ello", "key should have been dispatched normally");
}

#[test]
fn on_input_clear_with_nil() {
    janet_test!(ed, {
        janet_bridge::eval(
            r#"(defn some-fn [k] (editor/consume-input)) (editor/on-input "some-fn")"#);
        assert!(ed.on_input_fn.is_some());
        janet_bridge::eval("(editor/on-input nil)");
        assert!(ed.on_input_fn.is_none());
    });
}

#[test]
fn input_consumed_flag_cleared_each_dispatch() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor_with_buffer("ab");
    crate::janet_bridge::init(&mut ed);
    janet_bridge::eval(
        r#"(defn selective [k] (when (= k "a") (editor/consume-input))) (editor/on-input "selective")"#);
    ed.keymaps.set_layer("vim", "a", "append");
    ed.keymaps.set_layer("vim", "x", "delete-char");
    let buf_id = ed.windows.focused_window()
        .and_then(|wid| ed.windows.buffer(wid)).unwrap();
    ed.buffers.get_mut(buf_id).unwrap().set_cursor(0);
    dispatch_key(&mut ed, "a");
    dispatch_key(&mut ed, "x");
    let text = ed.buffers.get(buf_id).unwrap()
        .slice(0, ed.buffers.get(buf_id).unwrap().len());
    assert_eq!(text, "b", "first char should have been deleted by x");
}
