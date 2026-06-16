use crate::kernel::command::builtin;
use crate::kernel::storage::disk::DiskFileSystem;
use crate::kernel::keymap::KeymapManager;
use crate::kernel::state::Editor;
use crate::kernel::scripting;

/// Create an editor with a clean keymap state after Janet init.
/// We reset keymaps after `init()` so tests are not polluted by the
/// default bindings and active layers that `init.janet` installs.
fn make_clean_editor() -> Box<Editor> {
    let mut ed = Box::new(Editor::new(Box::new(DiskFileSystem::new())));
    builtin::register_builtin_commands(&mut ed);
    // init() must be called while ed is already heap-allocated so that
    // EDITOR_PTR (set inside init) stays valid after this function returns
    // the Box to the caller.  A stack-allocated Editor would make the
    // pointer dangle after the return-value move.
    scripting::init(&mut ed);
    ed.keymaps = KeymapManager::new();
    ed
}

// ── keymap/set (global) ──────────────────────────────────────────────

#[test]
fn set_global_binding() {
    let _lock = scripting::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_clean_editor();
    let r = scripting::eval(r#"(keymap/set "ctrl-s" "save-buffer")"#);
    assert_eq!(r, "ok");
    assert_eq!(ed.keymaps.resolve("ctrl-s"), Some("save-buffer".into()));
}

#[test]
fn set_multiple_global_bindings() {
    let _lock = scripting::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_clean_editor();

    scripting::eval(r#"(keymap/set "h" "cursor-left")"#);
    scripting::eval(r#"(keymap/set "j" "cursor-down")"#);
    scripting::eval(r#"(keymap/set "k" "cursor-up")"#);
    assert_eq!(ed.keymaps.resolve("h"), Some("cursor-left".into()));
    assert_eq!(ed.keymaps.resolve("j"), Some("cursor-down".into()));
    assert_eq!(ed.keymaps.resolve("k"), Some("cursor-up".into()));
    assert_eq!(ed.keymaps.resolve("l"), None);
}

#[test]
fn set_overwrites_previous_global_binding() {
    let _lock = scripting::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_clean_editor();

    scripting::eval(r#"(keymap/set "ctrl-s" "save")"#);
    scripting::eval(r#"(keymap/set "ctrl-s" "write")"#);
    assert_eq!(ed.keymaps.resolve("ctrl-s"), Some("write".into()));
}

// ── keymap/set (named layer) ─────────────────────────────────────────

#[test]
fn set_with_layer() {
    let _lock = scripting::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_clean_editor();
    let r = scripting::eval(r#"(keymap/set "h" "cursor-left" "vim")"#);
    assert_eq!(r, "ok");

    // Not active yet → should not resolve
    assert!(!ed.keymaps.is_layer_active("vim"));
    assert_eq!(ed.keymaps.resolve("h"), None);

    // Activate the layer
    ed.keymaps.push_layer("vim");
    assert_eq!(ed.keymaps.resolve("h"), Some("cursor-left".into()));
}

#[test]
fn set_with_global_layer_is_global() {
    let _lock = scripting::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_clean_editor();
    let r = scripting::eval(r#"(keymap/set "ctrl-q" "quit" "global")"#);
    assert_eq!(r, "ok");
    assert_eq!(ed.keymaps.resolve("ctrl-q"), Some("quit".into()));
}

// ── keymap/unset ─────────────────────────────────────────────────────

#[test]
fn unset_global() {
    let _lock = scripting::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_clean_editor();

    ed.keymaps.set("ctrl-q", "quit");
    let r = scripting::eval(r#"(keymap/unset "ctrl-q")"#);
    assert_eq!(r, "ok");
    assert_eq!(ed.keymaps.resolve("ctrl-q"), None);
}

#[test]
fn unset_with_layer() {
    let _lock = scripting::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_clean_editor();

    ed.keymaps.set_layer("vim", "h", "cursor-left");
    ed.keymaps.push_layer("vim");
    assert_eq!(ed.keymaps.resolve("h"), Some("cursor-left".into()));

    let r = scripting::eval(r#"(keymap/unset "h" "vim")"#);
    assert_eq!(r, "ok");
    assert_eq!(ed.keymaps.resolve("h"), None);
}

#[test]
fn unset_with_global_layer_is_global() {
    let _lock = scripting::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_clean_editor();

    ed.keymaps.set("ctrl-q", "quit");
    let r = scripting::eval(r#"(keymap/unset "ctrl-q" "global")"#);
    assert_eq!(r, "ok");
    assert_eq!(ed.keymaps.resolve("ctrl-q"), None);
}

#[test]
fn unset_nonexistent_key_is_safe() {
    let _lock = scripting::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_clean_editor();
    let r = scripting::eval(r#"(keymap/unset "never-set")"#);
    assert_eq!(r, "ok");
}

#[test]
fn unset_nonexistent_layer_is_safe() {
    let _lock = scripting::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_clean_editor();
    let r = scripting::eval(r#"(keymap/unset "h" "nonexistent-layer")"#);
    assert_eq!(r, "ok");
}

// ── keymap/describe ──────────────────────────────────────────────────

#[test]
fn describe_bound_key() {
    let _lock = scripting::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_clean_editor();

    ed.keymaps.set("ctrl-s", "save-buffer");

    let r = scripting::eval(r#"(keymap/describe "ctrl-s")"#);
    assert_eq!(r, "ok", "keymap/describe must not error for a bound key");
    assert_eq!(ed.keymaps.describe("ctrl-s"), Some("save-buffer".into()));
}

#[test]
fn describe_unbound_key() {
    let _lock = scripting::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_clean_editor();
    let r = scripting::eval(r#"(keymap/describe "nonexistent-key")"#);
    assert_eq!(r, "ok", "keymap/describe must not error for an unbound key");
    assert_eq!(ed.keymaps.describe("nonexistent-key"), None);
}

#[test]
fn describe_with_layer() {
    let _lock = scripting::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_clean_editor();

    ed.keymaps.set_layer("vim", "h", "cursor-left");
    ed.keymaps.push_layer("vim");

    let r = scripting::eval(r#"(keymap/describe "h")"#);
    assert_eq!(r, "ok");
    assert_eq!(ed.keymaps.describe("h"), Some("cursor-left".into()));
}

// ── keymap/list ──────────────────────────────────────────────────────

#[test]
fn list_all_layers() {
    let _lock = scripting::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_clean_editor();

    ed.keymaps.set("ctrl-q", "quit");
    ed.keymaps.set_layer("vim", "h", "cursor-left");

    let r = scripting::eval(r#"(keymap/list)"#);
    assert_eq!(r, "ok", "keymap/list without args must not error");
}

#[test]
fn list_specific_layer() {
    let _lock = scripting::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_clean_editor();

    ed.keymaps.set_layer("vim", "h", "cursor-left");
    ed.keymaps.set_layer("vim", "j", "cursor-down");

    let r = scripting::eval(r#"(keymap/list "vim")"#);
    assert_eq!(r, "ok", "keymap/list with layer must not error");
}

#[test]
fn list_global_layer() {
    let _lock = scripting::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_clean_editor();

    ed.keymaps.set("ctrl-s", "save");
    ed.keymaps.set("ctrl-q", "quit");

    let r = scripting::eval(r#"(keymap/list "global")"#);
    assert_eq!(r, "ok");
}

#[test]
fn list_nonexistent_layer_returns_empty() {
    let _lock = scripting::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_clean_editor();
    let r = scripting::eval(r#"(keymap/list "nobody-home")"#);
    assert_eq!(r, "ok", "keymap/list for nonexistent layer must not error");
}

// ── keymap/push-layer and keymap/pop-layer ───────────────────────────

#[test]
fn push_activates_layer() {
    let _lock = scripting::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_clean_editor();

    assert!(!ed.keymaps.is_layer_active("vim"));
    let r = scripting::eval(r#"(keymap/push-layer "vim")"#);
    assert_eq!(r, "ok");
    assert!(ed.keymaps.is_layer_active("vim"));
}

#[test]
fn push_is_idempotent_via_janet() {
    let _lock = scripting::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_clean_editor();

    scripting::eval(r#"(keymap/push-layer "vim")"#);
    scripting::eval(r#"(keymap/push-layer "vim")"#);
    // Only one active entry
    assert_eq!(ed.keymaps.active_layers(), vec!["global".to_string(), "vim".to_string()]);
}

#[test]
fn pop_deactivates_layer() {
    let _lock = scripting::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_clean_editor();

    ed.keymaps.push_layer("insert");
    assert!(ed.keymaps.is_layer_active("insert"));

    let r = scripting::eval(r#"(keymap/pop-layer "insert")"#);
    assert_eq!(r, "ok");
    assert!(!ed.keymaps.is_layer_active("insert"));
}

#[test]
fn pop_inactive_layer_is_safe_via_janet() {
    let _lock = scripting::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_clean_editor();
    let r = scripting::eval(r#"(keymap/pop-layer "never-pushed")"#);
    assert_eq!(r, "ok");
}

// ── keymap/list-layers ───────────────────────────────────────────────

#[test]
fn list_layers_returns_global_and_active() {
    let _lock = scripting::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_clean_editor();

    ed.keymaps.push_layer("vim");
    ed.keymaps.push_layer("insert");

    let r = scripting::eval(r#"(keymap/list-layers)"#);
    assert_eq!(r, "ok", "keymap/list-layers must not error");
    assert_eq!(ed.keymaps.active_layers(), vec!["global", "vim", "insert"]);
}

#[test]
fn list_layers_only_global_when_none_active() {
    let _lock = scripting::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_clean_editor();
    let r = scripting::eval(r#"(keymap/list-layers)"#);
    assert_eq!(r, "ok");
    assert_eq!(ed.keymaps.active_layers(), vec!["global"]);
}

// ── Integration ──────────────────────────────────────────────────────

#[test]
fn layer_priority_via_janet() {
    let _lock = scripting::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_clean_editor();

    // Set global + layer bindings via Janet
    scripting::eval(r#"(keymap/set "ctrl-q" "global-quit")"#);
    scripting::eval(r#"(keymap/set "h" "cursor-left" "vim")"#);
    scripting::eval(r#"(keymap/set "ctrl-q" "vim-quit" "vim")"#);

    // No layers active → global resolves
    assert_eq!(ed.keymaps.resolve("ctrl-q"), Some("global-quit".into()));

    // Push vim layer
    scripting::eval(r#"(keymap/push-layer "vim")"#);
    assert_eq!(ed.keymaps.resolve("h"),       Some("cursor-left".into()));
    assert_eq!(ed.keymaps.resolve("ctrl-q"),  Some("vim-quit".into()));

    // Unset layer binding via Janet
    scripting::eval(r#"(keymap/unset "ctrl-q" "vim")"#);
    assert_eq!(ed.keymaps.resolve("ctrl-q"),  Some("global-quit".into()));
}

#[test]
fn full_lifecycle_via_janet() {
    let _lock = scripting::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_clean_editor();

    // Set global binding
    scripting::eval(r#"(keymap/set "ctrl-s" "save")"#);
    assert_eq!(ed.keymaps.resolve("ctrl-s"), Some("save".into()));

    // Set layer binding
    scripting::eval(r#"(keymap/set "i" "enter-insert-mode" "vim")"#);
    assert_eq!(ed.keymaps.resolve("i"), None); // vim not active yet

    // Activate vim
    scripting::eval(r#"(keymap/push-layer "vim")"#);
    assert_eq!(ed.keymaps.resolve("i"), Some("enter-insert-mode".into()));

    // Set insert layer with esc
    ed.keymaps.set_layer("insert", "esc", "exit-insert-mode");
    scripting::eval(r#"(keymap/push-layer "insert")"#);

    // Describe via Janet
    let r = scripting::eval(r#"(keymap/describe "esc")"#);
    assert_eq!(r, "ok");

    // Pop insert
    scripting::eval(r#"(keymap/pop-layer "insert")"#);
    assert!(!ed.keymaps.is_layer_active("insert"));
    assert_eq!(ed.keymaps.resolve("esc"), None);

    // List layers
    let r = scripting::eval(r#"(keymap/list-layers)"#);
    assert_eq!(r, "ok");
    assert_eq!(ed.keymaps.active_layers(), vec!["global", "vim"]);
}

#[test]
fn keymap_list_after_modifications() {
    let _lock = scripting::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_clean_editor();

    ed.keymaps.set("ctrl-q", "quit");
    ed.keymaps.set_layer("test-layer", "a", "cmd-a");
    ed.keymaps.set_layer("test-layer", "b", "cmd-b");

    let r = scripting::eval(r#"(keymap/list "test-layer")"#);
    assert_eq!(r, "ok");

    let r = scripting::eval(r#"(keymap/list)"#);
    assert_eq!(r, "ok");
}
