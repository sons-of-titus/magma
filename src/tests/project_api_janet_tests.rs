use crate::buffer::Buffer;
use crate::command::builtin;
use crate::fs::disk::DiskFileSystem;
use crate::state::id::BufferId;
use crate::state::Editor;
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
fn project_root_returns_nil_by_default() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("");
    janet_bridge::init(&mut ed);

    let r = janet_bridge::eval(&mut ed, "(project/root)");
    assert_eq!(r, "ok", "project/root default must not error");
    assert!(ed.project_manager.project.root.is_none(), "project root must be None by default");
}

#[test]
fn project_set_root_creates_project_state() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("");
    janet_bridge::init(&mut ed);

    let tmp = std::env::temp_dir().join("sprint4_janet_set_root");
    std::fs::create_dir_all(&tmp).unwrap();
    let root_str = tmp.to_string_lossy().to_string();

    let expr = format!("(project/set-root \"{}\")", root_str);
    let r = janet_bridge::eval(&mut ed, &expr);
    assert_eq!(r, "ok", "project/set-root must not error");

    assert!(ed.project_manager.project.root.is_some(), "project root must be set");
    let root_got = ed.project_manager.project.root.as_ref().map(|p| p.to_string_lossy().into_owned());
    assert_eq!(root_got.as_deref(), Some(root_str.as_str()));
    assert!(ed.project_manager.current_project.is_some(), "current_project must be set");

    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn project_set_root_clears_when_nil() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("");
    janet_bridge::init(&mut ed);

    // Set a root first
    let tmp = std::env::temp_dir().join("sprint4_janet_clear");
    std::fs::create_dir_all(&tmp).unwrap();
    let root_str = tmp.to_string_lossy().to_string();
    let expr = format!("(project/set-root \"{}\")", root_str);
    janet_bridge::eval(&mut ed, &expr);

    // Clear it
    let r = janet_bridge::eval(&mut ed, "(project/set-root nil)");
    assert_eq!(r, "ok");
    assert!(ed.project_manager.project.root.is_none(), "project root must be cleared");

    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn project_set_name_works() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("");
    janet_bridge::init(&mut ed);

    let tmp = std::env::temp_dir().join("sprint4_janet_name");
    std::fs::create_dir_all(&tmp).unwrap();
    let root_str = tmp.to_string_lossy().to_string();
    let s = format!("(project/set-root \"{}\")", root_str);
    janet_bridge::eval(&mut ed, &s);

    let r = janet_bridge::eval(&mut ed, "(project/set-name \"my-project\")");
    assert_eq!(r, "ok");
    assert_eq!(ed.project_manager.project.name.as_deref(), Some("my-project"));

    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn project_option_set_and_get() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("");
    janet_bridge::init(&mut ed);

    let tmp = std::env::temp_dir().join("sprint4_janet_opts");
    std::fs::create_dir_all(&tmp).unwrap();
    let root_str = tmp.to_string_lossy().to_string();
    let s = format!("(project/set-root \"{}\")", root_str);
    janet_bridge::eval(&mut ed, &s);

    janet_bridge::eval(&mut ed, r#"(project/option-set "build-command" "cargo check")"#);
    assert_eq!(
        ed.project_manager.project.options.get("build-command").map(|s| s.as_str()),
        Some("cargo check")
    );

    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn project_register_and_list() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("");
    janet_bridge::init(&mut ed);

    let tmp = std::env::temp_dir().join("sprint4_janet_reg");
    let tmp2 = std::env::temp_dir().join("sprint4_janet_reg2");
    std::fs::create_dir_all(&tmp).unwrap();
    std::fs::create_dir_all(&tmp2).unwrap();
    let r1 = tmp.to_string_lossy().to_string();
    let r2 = tmp2.to_string_lossy().to_string();

    let expr1 = format!("(project/register \"proj-a\" \"{}\")", r1);
    let expr2 = format!("(project/register \"proj-b\" \"{}\")", r2);

    let r = janet_bridge::eval(&mut ed, &expr1);
    assert_eq!(r, "ok");
    let r = janet_bridge::eval(&mut ed, &expr2);
    assert_eq!(r, "ok");

    assert!(ed.project_manager.projects.contains_key("proj-a"));
    assert!(ed.project_manager.projects.contains_key("proj-b"));
    assert_eq!(ed.project_manager.projects.len(), 2);

    std::fs::remove_dir_all(&tmp).ok();
    std::fs::remove_dir_all(&tmp2).ok();
}

#[test]
fn project_unregister_removes() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("");
    janet_bridge::init(&mut ed);

    let tmp = std::env::temp_dir().join("sprint4_janet_unreg");
    std::fs::create_dir_all(&tmp).unwrap();
    let r = tmp.to_string_lossy().to_string();
    let expr = format!("(project/register \"my-proj\" \"{}\")", r);
    janet_bridge::eval(&mut ed, &expr);

    assert!(ed.project_manager.projects.contains_key("my-proj"));

    janet_bridge::eval(&mut ed, "(project/unregister \"my-proj\")");
    assert!(!ed.project_manager.projects.contains_key("my-proj"));

    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn project_buffer_set_and_get() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("");
    janet_bridge::init(&mut ed);

    let key = ed.windows.focused_window()
        .and_then(|wid| ed.windows.buffer(wid))
        .unwrap_or(0);

    let expr = format!("(project/buffer-set-project {} \"my-proj\")", key);
    let r = janet_bridge::eval(&mut ed, &expr);
    assert_eq!(r, "ok");

    assert_eq!(
        ed.project_manager.buffer_projects.get(&key).map(|s| s.as_str()),
        Some("my-proj")
    );

    let get_expr = format!("(project/buffer-project {})", key);
    let r = janet_bridge::eval(&mut ed, &get_expr);
    assert_eq!(r, "ok");
}

#[test]
fn project_files_empty_by_default() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("");
    janet_bridge::init(&mut ed);

    let r = janet_bridge::eval(&mut ed, "(project/files)");
    assert_eq!(r, "ok", "project/files default must not error");
    assert!(ed.project_manager.project.files.is_empty(), "files must be empty by default");
}

#[test]
fn project_set_workspace_members_via_janet() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("");
    janet_bridge::init(&mut ed);

    let tmp = std::env::temp_dir().join("sprint4_janet_ws");
    let m1 = tmp.join("sub-a");
    let m2 = tmp.join("sub-b");
    std::fs::create_dir_all(&m1).unwrap();
    std::fs::create_dir_all(&m2).unwrap();
    let root_str = tmp.to_string_lossy().to_string();
    let s = format!("(project/set-root \"{}\")", root_str);
    janet_bridge::eval(&mut ed, &s);

    let m1s = m1.to_string_lossy().to_string();
    let m2s = m2.to_string_lossy().to_string();
    let expr = format!(
        r#"(project/set-workspace-members [{{:name "sub-a" :root "{}"}} {{:name "sub-b" :root "{}"}}])"#,
        m1s, m2s
    );
    let r = janet_bridge::eval(&mut ed, &expr);
    assert_eq!(r, "ok");

    let ws = ed.project_manager.project.workspace.as_ref().expect("workspace must be set");
    assert_eq!(ws.members.len(), 2);
    assert_eq!(ws.members[0].name, "sub-a");
    assert_eq!(ws.members[1].name, "sub-b");

    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn project_set_current_member_fires_event() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("");
    janet_bridge::init(&mut ed);

    let tmp = std::env::temp_dir().join("sprint4_janet_member");
    let m1 = tmp.join("core");
    std::fs::create_dir_all(&m1).unwrap();
    let root_str = tmp.to_string_lossy().to_string();
    let s = format!("(project/set-root \"{}\")", root_str);
    janet_bridge::eval(&mut ed, &s);

    let m1s = m1.to_string_lossy().to_string();
    let expr = format!(
        r#"(project/set-workspace-members [{{:name "core" :root "{}"}}])"#,
        m1s
    );
    janet_bridge::eval(&mut ed, &expr);

    // Set up a Rust command for the event handler (avoids nested fiber issue)
    let event_count: std::sync::Arc<std::sync::Mutex<i32>> =
        std::sync::Arc::new(std::sync::Mutex::new(0));
    let cnt = event_count.clone();
    ed.events.on("project-member-focused", move |_data| {
        *cnt.lock().unwrap() += 1;
        None
    });

    let r = janet_bridge::eval(&mut ed, r#"(project/set-current-member "core")"#);
    assert_eq!(r, "ok");
    ed.events.drain_and_dispatch();

    let count = *event_count.lock().unwrap();
    assert!(count > 0, "project-member-focused event must fire");

    std::fs::remove_dir_all(&tmp).ok();
}
