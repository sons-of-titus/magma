//! Janet API tests for the unified input pipeline (Sprint 11d).
//! Tests editor/on-input and editor/consume-input behaviour.

#[cfg(feature = "janet")]
mod tests {
    use crate::state::Editor;
    use crate::state::id::BufferId;
    use crate::buffer::Buffer;
    use crate::command::builtin;
    use crate::fs::disk::DiskFileSystem;
    use crate::input::dispatch_key;
    use crate::janet_bridge;

    fn make_editor(content: &str) -> Editor {
        let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
        builtin::register_builtin_commands(&mut ed);
        let id = ed.allocate_buffer_id();
        let buf = Buffer::from_string(BufferId(id), "test", content);
        let entry = ed.buffers.vacant_entry();
        let key = entry.key();
        entry.insert(buf);
        if let Some(win) = ed.windows.focused_window_mut() { win.buffer_id = Some(key); }
        ed
    }

    #[test]
    fn on_input_fn_starts_nil() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor("");
        janet_bridge::init(&mut ed);
        assert_eq!(ed.on_input_fn.as_deref(), Some("vim-on-input"));
    }

    #[test]
    fn on_input_fn_registration() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor("");
        janet_bridge::init(&mut ed);
        janet_bridge::eval(
            r#"(defn my-interceptor [k] nil) (editor/on-input "my-interceptor")"#);
        assert_eq!(ed.on_input_fn.as_deref(), Some("my-interceptor"));
    }

    #[test]
    fn on_input_interceptor_consuming_key() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor("hello");
        janet_bridge::init(&mut ed);
        // Register an interceptor that consumes ALL keys via editor/consume-input
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
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor("hello");
        janet_bridge::init(&mut ed);
        // Interceptor that does NOT call consume-input — key passes through
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
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor("");
        janet_bridge::init(&mut ed);
        janet_bridge::eval(
            r#"(defn some-fn [k] (editor/consume-input)) (editor/on-input "some-fn")"#);
        assert!(ed.on_input_fn.is_some());
        janet_bridge::eval("(editor/on-input nil)");
        assert!(ed.on_input_fn.is_none());
    }

    #[test]
    fn input_consumed_flag_cleared_each_dispatch() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor("ab");
        janet_bridge::init(&mut ed);
        // Interceptor that only consumes 'a'
        janet_bridge::eval(
            r#"(defn selective [k] (when (= k "a") (editor/consume-input))) (editor/on-input "selective")"#);
        ed.keymaps.set_layer("vim", "a", "append");
        ed.keymaps.set_layer("vim", "x", "delete-char");
        let buf_id = ed.windows.focused_window()
            .and_then(|wid| ed.windows.buffer(wid)).unwrap();
        ed.buffers.get_mut(buf_id).unwrap().set_cursor(0);
        // 'a' is consumed, so no command runs
        dispatch_key(&mut ed, "a");
        // 'x' is not consumed, so delete-char runs
        dispatch_key(&mut ed, "x");
        let text = ed.buffers.get(buf_id).unwrap()
            .slice(0, ed.buffers.get(buf_id).unwrap().len());
        assert_eq!(text, "b", "first char should have been deleted by x");
    }
}
