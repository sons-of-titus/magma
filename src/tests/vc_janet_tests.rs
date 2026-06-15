use crate::buffer::Buffer;
use crate::command::builtin;
use crate::fs::disk::DiskFileSystem;
use crate::janet_bridge;
use crate::state::id::BufferId;
use crate::state::Editor;

fn make_editor(content: &str) -> Editor {
    let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
    builtin::register_builtin_commands(&mut ed);
    let id = ed.allocate_buffer_id();
    let buf = Buffer::from_string(BufferId(id), "test", content);
    let entry = ed.buffers.vacant_entry();
    let key = entry.key();
    entry.insert(buf);
    if let Some(win) = ed.windows.focused_window_mut() {
        win.buffer_id = Some(key);
    }
    ed
}

#[test]
fn vc_register_backend_adds_backend() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("");
    janet_bridge::init(&mut ed);

    let count_before = ed.vc.backends.len();

    let r = janet_bridge::eval(
        &mut ed,
        r#"(vc/register-backend {:name "fossil" :detect-marker ".fslckout"})"#,
    );
    assert_eq!(r, "ok", "vc/register-backend must not error");

    assert_eq!(
        ed.vc.backends.len(),
        count_before + 1,
        "backend count must increase by 1"
    );
    assert_eq!(
        ed.vc.backends.last().unwrap().name(),
        "fossil",
        "last registered backend must be 'fossil'"
    );
}

#[test]
fn vc_register_backend_missing_name_signals_error() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("");
    janet_bridge::init(&mut ed);

    let count_before = ed.vc.backends.len();

    let r = janet_bridge::eval(
        &mut ed,
        r#"(vc/register-backend {:detect-marker ".fslckout"})"#,
    );
    assert_ne!(r, "ok", "expected error but got ok: {r}");

    assert_eq!(
        ed.vc.backends.len(),
        count_before,
        "backends must not change after error"
    );
}

#[test]
fn vc_register_backend_missing_detect_signals_error() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("");
    janet_bridge::init(&mut ed);

    let count_before = ed.vc.backends.len();

    let r = janet_bridge::eval(
        &mut ed,
        r#"(vc/register-backend {:name "fossil"})"#,
    );
    assert_ne!(r, "ok", "expected error but got ok: {r}");

    assert_eq!(
        ed.vc.backends.len(),
        count_before,
        "backends must not change after error"
    );
}

#[test]
fn vc_register_backend_wrong_arg_type_signals_error() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("");
    janet_bridge::init(&mut ed);

    let count_before = ed.vc.backends.len();

    let r = janet_bridge::eval(
        &mut ed,
        r#"(vc/register-backend "not-a-table")"#,
    );
    assert_ne!(r, "ok", "expected error but got ok: {r}");

    assert_eq!(
        ed.vc.backends.len(),
        count_before,
        "backends must not change after error"
    );
}

#[test]
fn vc_register_backend_no_args_signals_error() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("");
    janet_bridge::init(&mut ed);

    let r = janet_bridge::eval(&mut ed, r#"(vc/register-backend)"#);
    assert_ne!(r, "ok", "expected error but got ok: {r}");
}

#[test]
fn vc_register_backend_multiple_backends() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("");
    janet_bridge::init(&mut ed);

    let r1 = janet_bridge::eval(
        &mut ed,
        r#"(vc/register-backend {:name "fossil" :detect-marker ".fslckout"})"#,
    );
    assert_eq!(r1, "ok");

    let r2 = janet_bridge::eval(
        &mut ed,
        r#"(vc/register-backend {:name "bzr" :detect-marker ".bzr"})"#,
    );
    assert_eq!(r2, "ok");

    let backends: Vec<&str> = ed.vc.backends.iter().map(|b| b.name()).collect();
    assert!(
        backends.contains(&"fossil"),
        "backends must contain 'fossil': {backends:?}"
    );
    assert!(
        backends.contains(&"bzr"),
        "backends must contain 'bzr': {backends:?}"
    );
}

#[test]
fn vc_register_backend_with_optional_fields() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("");
    janet_bridge::init(&mut ed);

    let r = janet_bridge::eval(
        &mut ed,
        r#"(vc/register-backend {:name "custom"
                                   :detect-marker ".custom"
                                   :status-cmd "echo M file.txt"
                                   :has-staging true})"#,
    );
    assert_eq!(r, "ok", "register-backend with optional fields must not error");

    let last = ed.vc.backends.last().unwrap();
    assert_eq!(last.name(), "custom");
}
