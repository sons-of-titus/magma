#[macro_use]
mod helpers;
// Quit through the Janet command surface — the guarded Rust `quit` command
// is what any keymap or plugin binding reaches.

use scripting;

#[test]
fn quit_refuses_when_dirty_and_is_catchable_from_janet() {
    janet_test!(ed, {
        let key = ed.create_buffer_from_str("f.txt", "content");
        if let Some(win) = ed.view_tree.focused_window_mut() {
            win.buffer_id = Some(key);
        }
        let view = ed.views.get_mut(&key).unwrap();
        view.insert(0, "x");

        // A keymap binding or plugin runs the same command; the refusal is
        // a catchable Janet error, not a silent no-op.
        let caught = scripting::eval_result(
            "(do (var res \"quit\") \
              (try (editor/run-command \"quit\") \
                ([e] (set res \"refused\"))) \
              res)",
        );
        assert_eq!(
            caught.unwrap(),
            "\"refused\"",
            "dirty quit must not go through"
        );
        assert!(ed.running, "refusal keeps the editor alive");

        let r = scripting::eval("(editor/run-command \"force-quit\")");
        assert_eq!(r, "ok");
        assert!(!ed.running, "force-quit is the explicit override");
    });
}

#[test]
fn quit_succeeds_when_clean() {
    janet_test!(ed, {
        let r = scripting::eval("(editor/run-command \"quit\")");
        assert_eq!(r, "ok");
        assert!(!ed.running);
    });
}

#[test]
fn save_and_quit_saves_the_focused_buffer() {
    janet_test!(ed, {
        let dir = std::env::temp_dir().join(format!("magma-sq-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("sq.txt");

        let key = ed.create_buffer_from_str("sq.txt", "v1");
        ed.buffers.get_mut(key).unwrap().lock().unwrap().path =
            Some(path.to_str().unwrap().to_string());
        if let Some(win) = ed.view_tree.focused_window_mut() {
            win.buffer_id = Some(key);
        }
        let view = ed.views.get_mut(&key).unwrap();
        view.insert(2, "-edited");

        let r = scripting::eval("(editor/run-command \"save-and-quit\")");
        assert_eq!(r, "ok");
        assert!(!ed.running);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "v1-edited");
        std::fs::remove_dir_all(&dir).ok();
    });
}
