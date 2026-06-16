use crate::kernel::state::Editor;
use crate::tests::helpers;

#[test]
fn buffer_read_only_defaults_to_false() {
    let ed = helpers::make_editor();
    let key = ed.windows.focused_window()
        .and_then(|wid| ed.windows.buffer(wid))
        .unwrap();
    assert!(!ed.buffers.get(key).unwrap().read_only,
        "new buffer must not be read-only by default");
}

#[test]
fn buffer_ephemeral_defaults_to_false() {
    let ed = helpers::make_editor();
    let key = ed.windows.focused_window()
        .and_then(|wid| ed.windows.buffer(wid))
        .unwrap();
    assert!(!ed.buffers.get(key).unwrap().ephemeral,
        "new buffer must not be ephemeral by default");
}

#[test]
fn buffer_read_only_can_be_set() {
    let mut ed = helpers::make_editor();
    let key = ed.windows.focused_window()
        .and_then(|wid| ed.windows.buffer(wid))
        .unwrap();
    ed.buffers.get_mut(key).unwrap().read_only = true;
    assert!(ed.buffers.get(key).unwrap().read_only,
        "read_only must be settable");
}

#[test]
fn buffer_ephemeral_can_be_set() {
    let mut ed = helpers::make_editor();
    let key = ed.windows.focused_window()
        .and_then(|wid| ed.windows.buffer(wid))
        .unwrap();
    ed.buffers.get_mut(key).unwrap().ephemeral = true;
    assert!(ed.buffers.get(key).unwrap().ephemeral,
        "ephemeral must be settable");
}

#[test]
fn read_only_buffer_insert_is_a_rust_level_bypass() {
    // Rust-level insert bypasses the read_only flag (only the Janet C fn
    // checks it).  This test documents that invariant.
    let mut ed = helpers::make_editor();
    let key = ed.windows.focused_window()
        .and_then(|wid| ed.windows.buffer(wid))
        .unwrap();
    ed.buffers.get_mut(key).unwrap().read_only = true;
    ed.buffers.get_mut(key).unwrap().insert(0, "hello");
    assert_eq!(
        ed.buffers.get(key).unwrap().slice(0, ed.buffers.get(key).unwrap().len()),
        "hello",
        "Rust-level insert must bypass read_only (used by log-message/show-help)"
    );
}
