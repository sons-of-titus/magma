//! The welcome card and menus advertise chords; every advertised chord must
//! resolve against the shipped keymap.  The cheat sheet regaining the user's
//! trust is the point of this file: one dead advertised key is the bug the
//! 2026-11 GUI review called out.

#[macro_use]
mod helpers;

use kernel::command::builtin;
use kernel::state::Editor;
use kernel::storage::disk::DiskFileSystem;
use scripting;

/// An editor with the shipped builtins loaded — no keymap reset, so the
/// standard bindings from `editor_keybindings.janet` are live.
fn shipped_editor() -> Box<Editor> {
    let mut ed = Box::new(Editor::new(Box::new(DiskFileSystem::new())));
    builtin::register_builtin_commands(&mut ed);
    scripting::init(&mut ed);
    ed
}

/// (chord, command, where the UI advertises it)
const ADVERTISED: &[(&str, &str, &str)] = &[
    ("ctrl-f", "gui-find", "welcome card + Edit menu"),
    ("ctrl-shift-f", "gui-project-find", "welcome card + Edit menu"),
    ("ctrl-l", "gui-goto-line", "welcome card + Edit menu"),
    ("ctrl-shift-p", "gui-command-palette", "welcome card"),
    ("ctrl-s", "save-buffer", "File menu"),
    ("ctrl-z", "undo", "Edit menu"),
    ("ctrl-w", "close-buffer", "tab close convention"),
    ("ctrl-q", "quit", "quit convention"),
];

#[test]
fn every_advertised_chord_resolves_to_its_command() {
    let _lock = scripting::JANET_VM_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let ed = shipped_editor();
    for (chord, command, advertised) in ADVERTISED {
        assert_eq!(
            ed.keymaps.resolve(chord).as_deref(),
            Some(*command),
            "{chord} is advertised ({advertised}) but not bound to {command}"
        );
    }
}

#[test]
fn the_commands_advertised_chords_reach_exist() {
    let _lock = scripting::JANET_VM_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let ed = shipped_editor();
    for (chord, command, _) in ADVERTISED {
        assert!(
            ed.commands.exists(command) || ed.keymaps.resolve(chord).is_some(),
            "{command} must be a real command for {chord}"
        );
    }
    // The chrome requests the keymap's gui commands produce are the ones
    // the shell drains — the names must stay in sync with actions/picker.
    for request in [
        "gui-find",
        "gui-replace",
        "gui-project-find",
        "gui-goto-file",
        "gui-goto-symbol",
        "gui-buffers",
        "gui-command-palette",
        "gui-goto-line",
        "gui-branches",
    ] {
        assert!(ed.commands.exists(request), "{request} missing from the registry");
    }
}

#[test]
fn completion_chords_still_belongs_to_completion() {
    let _lock = scripting::JANET_VM_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let ed = shipped_editor();
    assert_eq!(
        ed.keymaps.resolve("ctrl-p").as_deref(),
        Some("completion-prev"),
        "ctrl-p cycles completions while typing; the palette lives on ctrl-shift-p"
    );
}
