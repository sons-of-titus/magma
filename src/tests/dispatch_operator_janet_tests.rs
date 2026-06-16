use crate::janet_bridge;
use crate::input::{dispatch_key, focused_buffer_id};

#[test]
fn check_command_registered() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor_with_buffer("hello");
    assert!(!ed.commands.exists("vim-enter-command"));
    crate::janet_bridge::init(&mut ed);
    assert!(ed.commands.exists("vim-enter-command"),
        "vim-enter-command should be registered after janet_bridge::init");
}

#[test]
fn init_then_resolve_colon() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor_with_buffer("hello");
    crate::janet_bridge::init(&mut ed);
    let buf_id = focused_buffer_id(&ed);
    let cmd = ed.keymaps.resolve_for_buffer(":", Some(buf_id as u64));
    eprintln!("Resolved ':' -> {:?}", cmd);
    assert_eq!(cmd, Some("vim-enter-command".to_string()));
}

#[test]
fn init_then_press_colon() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor_with_buffer("hello");
    crate::janet_bridge::init(&mut ed);

    assert!(ed.commands.exists("vim-enter-command"));

    dispatch_key(&mut ed, ":");
    eprintln!("Mode after ':' press: {:?}", ed.editor_mode.name);
    assert_eq!(ed.editor_mode.name, "command",
        "mode should be 'command' after pressing ':', got {:?}", ed.editor_mode.name);
}
