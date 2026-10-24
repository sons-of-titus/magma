//! External file-change reload: unmodified buffers auto-revert from disk,
//! modified buffers are never clobbered, and watcher echoes of our own
//! saves do not pollute undo history.

use crate::kernel::runtime::{process_background_event, BackgroundEvent};
use crate::tests::helpers;

fn temp_file(tag: &str, content: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("magma_reload_{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("{tag}.txt");
    std::fs::write(&path, content).unwrap();
    std::fs::canonicalize(&path).unwrap()
}

fn open_file_in_editor(ed: &mut crate::kernel::state::Editor, path: &std::path::Path) {
    let mut args = std::collections::HashMap::new();
    args.insert("path".to_string(),
        crate::kernel::command::args::ArgValue::Path(path.to_string_lossy().into_owned()));
    crate::kernel::command::execute_command(ed, "open-file", &args).unwrap();
}

fn disk_changed_event(path: &std::path::Path) -> BackgroundEvent {
    BackgroundEvent::FileChanged {
        path: std::fs::canonicalize(path).unwrap().to_string_lossy().into_owned(),
        kind: "modified".to_string(),
    }
}

#[test]
fn unmodified_buffer_auto_reverts_from_disk() {
    let path = temp_file("revert", "line one\nline two\nline three\n");
    let mut ed = helpers::make_editor();
    open_file_in_editor(&mut ed, &path);

    // Move the cursor into line two before the external change.
    let key = helpers::focused_key(&ed);
    ed.views.get_mut(&key).unwrap().set_cursor(15);

    std::fs::write(&path, "REWRITTEN\nexternally\n").unwrap();
    process_background_event(&mut ed, disk_changed_event(&path));

    let text = helpers::buf_text(&ed);
    assert_eq!(text, "REWRITTEN\nexternally\n", "unmodified buffer follows disk");
    let buf = ed.buffers.get(key).unwrap().lock().unwrap();
    assert!(!buf.modified(), "reverted buffer is clean");
    drop(buf);

    // Cursor position survives: offset 15 was (line 1, column 6) in the
    // old text and must map to (line 1, column 6) of the new text.
    let view = ed.views.get(&key).unwrap();
    let buf = view.buffer.lock().unwrap();
    let p = buf.position_of(view.cursor.offset);
    assert_eq!((p.line, p.column), (1, 6), "cursor keeps line/column across reload");
}

#[test]
fn modified_buffer_is_never_clobbered() {
    let path = temp_file("clobber", "original\n");
    let mut ed = helpers::make_editor();
    open_file_in_editor(&mut ed, &path);

    let key = helpers::focused_key(&ed);
    ed.views.get_mut(&key).unwrap().insert(0, "typed ");

    std::fs::write(&path, "attacker-controlled\n").unwrap();
    process_background_event(&mut ed, disk_changed_event(&path));

    assert_eq!(helpers::buf_text(&ed), "typed original\n",
        "unsaved edits must win over the disk file");
    assert!(ed.status_message.is_some(), "the conflict is reported");
}

#[test]
fn watcher_echo_of_own_save_leaves_undo_untouched() {
    let path = temp_file("echo", "content\n");
    let mut ed = helpers::make_editor();
    open_file_in_editor(&mut ed, &path);

    let key = helpers::focused_key(&ed);
    ed.views.get_mut(&key).unwrap().insert(0, "more ");
    let undo_before = ed.buffers.get(key).unwrap().lock().unwrap().undo_len();
    helpers::run(&mut ed, "save-buffer");

    // The watcher fires for our own save; the identical content must not
    // create an undo transaction.
    process_background_event(&mut ed, disk_changed_event(&path));
    let buf = ed.buffers.get(key).unwrap().lock().unwrap();
    assert_eq!(buf.undo_len(), undo_before, "no undo entry for an identical reload");
    assert!(!buf.modified(), "save + echo leaves the buffer clean");
}

#[test]
fn removed_file_reports_instead_of_acting() {
    let path = temp_file("removed", "doomed\n");
    let mut ed = helpers::make_editor();
    open_file_in_editor(&mut ed, &path);

    std::fs::remove_file(&path).unwrap();
    process_background_event(&mut ed, BackgroundEvent::FileChanged {
        path: path.to_string_lossy().into_owned(),
        kind: "removed".to_string(),
    });

    assert_eq!(helpers::buf_text(&ed), "doomed\n", "buffer content survives");
    assert!(ed.status_message.is_some(), "removal is surfaced to the user");
}

#[test]
fn reload_is_undoable() {
    let path = temp_file("undo", "first version\n");
    let mut ed = helpers::make_editor();
    open_file_in_editor(&mut ed, &path);

    std::fs::write(&path, "second version\n").unwrap();
    process_background_event(&mut ed, disk_changed_event(&path));
    assert_eq!(helpers::buf_text(&ed), "second version\n");

    let key = helpers::focused_key(&ed);
    let undone = ed.views.get_mut(&key).unwrap().undo();
    assert!(undone, "the reload is one undo step");
    assert_eq!(helpers::buf_text(&ed), "first version\n");
}
