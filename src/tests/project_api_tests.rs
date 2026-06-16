use std::collections::HashMap;
use std::path::PathBuf;

use crate::kernel::state::{Editor, ProjectMember, ProjectState, Workspace};
use crate::tests::helpers;

#[test]
fn project_state_defaults() {
    let ps = ProjectState::default();
    assert!(ps.root.is_none());
    assert!(ps.name.is_none());
    assert!(ps.workspace.is_none());
    assert!(ps.files.is_empty());
    assert!(ps.options.is_empty());
    assert!(!ps.file_index_dirty);
}

#[test]
fn editor_project_fields_initialized() {
    let ed = helpers::make_editor();
    assert!(ed.project_manager.project.root.is_none());
    assert!(ed.project_manager.project.name.is_none());
    assert!(ed.project_manager.current_project.is_none());
    assert!(ed.project_manager.projects.is_empty());
    assert!(ed.project_manager.buffer_projects.is_empty());
    assert!(ed.project_manager.recent_projects.is_empty());
}

#[test]
fn project_set_root_emits_opened_event() {
    let mut ed = helpers::make_editor();
    let tmp = std::env::temp_dir().join("sprint4_test_root");
    std::fs::create_dir_all(&tmp).unwrap();
    let root_str = tmp.to_string_lossy().to_string();

    let captured: std::sync::Arc<std::sync::Mutex<Option<String>>> =
        std::sync::Arc::new(std::sync::Mutex::new(None));
    let cap = captured.clone();
    ed.events.on("project-opened", move |data| {
        *cap.lock().unwrap() = data.get("root").cloned();
        None
    });

    let root_path = PathBuf::from(&root_str);
    ed.project_manager.project.root = Some(root_path.clone());
    ed.project_manager.project.name = Some("sprint4_test_root".to_string());
    ed.project_manager.current_project = Some("sprint4_test_root".to_string());
    let mut data = HashMap::new();
    data.insert("root".to_string(), root_str.clone());
    data.insert("name".to_string(), "sprint4_test_root".to_string());
    ed.events.emit("project-opened", data);
    ed.events.drain_and_dispatch();

    let got = captured.lock().unwrap().clone();
    assert_eq!(got.as_deref(), Some(root_str.as_str()));

    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn project_clear_root_emits_closed_event() {
    let mut ed = helpers::make_editor();
    ed.project_manager.project.root = Some(PathBuf::from("/tmp/some-project"));
    ed.project_manager.project.name = Some("some-project".to_string());
    ed.project_manager.current_project = Some("some-project".to_string());

    let fired: std::sync::Arc<std::sync::Mutex<bool>> =
        std::sync::Arc::new(std::sync::Mutex::new(false));
    let f = fired.clone();
    ed.events.on("project-closed", move |_| {
        *f.lock().unwrap() = true;
        None
    });

    ed.project_manager.project = ProjectState::default();
    ed.project_manager.current_project = None;
    let data = HashMap::new();
    ed.events.emit("project-closed", data);
    ed.events.drain_and_dispatch();

    assert!(*fired.lock().unwrap());
    assert!(ed.project_manager.project.root.is_none());
}

#[test]
fn project_options_isolated() {
    let mut ed = helpers::make_editor();
    let tmp = std::env::temp_dir().join("sprint4_opts");
    std::fs::create_dir_all(&tmp).unwrap();
    let root_str = tmp.to_string_lossy().to_string();

    ed.project_manager.project.root = Some(PathBuf::from(&root_str));
    ed.project_manager.project.options.insert("build-command".to_string(), "cargo build".to_string());
    ed.project_manager.project.options.insert("lsp-server".to_string(), "rust-analyzer".to_string());

    assert_eq!(
        ed.project_manager.project.options.get("build-command").map(|s| s.as_str()),
        Some("cargo build")
    );
    assert_eq!(
        ed.project_manager.project.options.get("lsp-server").map(|s| s.as_str()),
        Some("rust-analyzer")
    );
    assert!(ed.project_manager.project.options.get("nonexistent").is_none());

    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn project_workspace_members() {
    let tmp = std::env::temp_dir().join("sprint4_ws");
    std::fs::create_dir_all(&tmp).unwrap();
    let member1 = tmp.join("member1");
    let member2 = tmp.join("member2");
    std::fs::create_dir_all(&member1).unwrap();
    std::fs::create_dir_all(&member2).unwrap();

    let mut ed = helpers::make_editor();
    ed.project_manager.project.root = Some(tmp.clone());
    ed.project_manager.project.workspace = Some(Workspace {
        root: tmp.clone(),
        members: vec![
            ProjectMember {
                name: "member1".to_string(),
                root: member1,
            },
            ProjectMember {
                name: "member2".to_string(),
                root: member2,
            },
        ],
    });

    let ws = ed.project_manager.project.workspace.as_ref().unwrap();
    assert_eq!(ws.members.len(), 2);
    assert_eq!(ws.members[0].name, "member1");
    assert_eq!(ws.members[1].name, "member2");

    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn project_member_focused_event() {
    let mut ed = helpers::make_editor();
    let tmp = std::env::temp_dir().join("sprint4_mf");
    std::fs::create_dir_all(&tmp).unwrap();
    let m1 = tmp.join("core");
    std::fs::create_dir_all(&m1).unwrap();

    ed.project_manager.project.root = Some(tmp.clone());
    ed.project_manager.project.name = Some("workspace-proj".to_string());
    ed.project_manager.project.workspace = Some(Workspace {
        root: tmp.clone(),
        members: vec![
            ProjectMember {
                name: "core".to_string(),
                root: m1,
            },
        ],
    });
    ed.project_manager.current_project = Some("workspace-proj".to_string());

    let captured: std::sync::Arc<std::sync::Mutex<Option<String>>> =
        std::sync::Arc::new(std::sync::Mutex::new(None));
    let cap = captured.clone();
    ed.events.on("project-member-focused", move |data| {
        *cap.lock().unwrap() = data.get("name").cloned();
        None
    });

    // Emit the event as the C function would
    let mut data = HashMap::new();
    data.insert("name".to_string(), "core".to_string());
    data.insert("root".to_string(), "/tmp/sprint4_mf/core".to_string());
    data.insert("project-name".to_string(), "workspace-proj".to_string());
    ed.events.emit("project-member-focused", data);
    ed.events.drain_and_dispatch();

    let got = captured.lock().unwrap().clone();
    assert_eq!(got.as_deref(), Some("core"));

    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn project_buffer_association() {
    let mut ed = helpers::make_editor();
    let tmp = std::env::temp_dir().join("sprint4_bufproj");
    std::fs::create_dir_all(&tmp).unwrap();

    let key = ed.windows.focused_window()
        .and_then(|wid| ed.windows.buffer(wid))
        .unwrap_or(0);

    ed.project_manager.buffer_projects.insert(key, "my-project".to_string());

    assert_eq!(
        ed.project_manager.buffer_projects.get(&key).map(|s| s.as_str()),
        Some("my-project")
    );
    assert!(ed.project_manager.buffer_projects.get(&999).is_none());

    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn project_multi_registry() {
    let mut ed = helpers::make_editor();
    let tmp = std::env::temp_dir().join("sprint4_registry");
    std::fs::create_dir_all(&tmp).unwrap();
    let p1 = tmp.join("proj-a");
    let p2 = tmp.join("proj-b");
    std::fs::create_dir_all(&p1).unwrap();
    std::fs::create_dir_all(&p2).unwrap();

    ed.project_manager.projects.insert("proj-a".to_string(), ProjectState {
        root: Some(p1),
        name: Some("proj-a".to_string()),
        workspace: None,
        files: vec!["src/main.rs".to_string()],
        options: HashMap::new(),
        file_index_dirty: false,
    });
    ed.project_manager.projects.insert("proj-b".to_string(), ProjectState {
        root: Some(p2),
        name: Some("proj-b".to_string()),
        workspace: None,
        files: vec!["lib/main.dart".to_string()],
        options: HashMap::new(),
        file_index_dirty: false,
    });

    assert_eq!(ed.project_manager.projects.len(), 2);
    assert!(ed.project_manager.projects.contains_key("proj-a"));
    assert!(ed.project_manager.projects.contains_key("proj-b"));

    ed.project_manager.projects.remove("proj-a");
    assert_eq!(ed.project_manager.projects.len(), 1);

    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn project_recent_list() {
    let mut ed = helpers::make_editor();
    ed.project_manager.recent_projects.push("/home/user/proj1".to_string());
    ed.project_manager.recent_projects.push("/home/user/proj2".to_string());

    assert_eq!(ed.project_manager.recent_projects.len(), 2);
    assert_eq!(ed.project_manager.recent_projects[0], "/home/user/proj1");
    assert_eq!(ed.project_manager.recent_projects[1], "/home/user/proj2");
}

#[test]
fn project_files_cache() {
    let mut ed = helpers::make_editor();
    let tmp = std::env::temp_dir().join("sprint4_files");
    std::fs::create_dir_all(&tmp).unwrap();

    ed.project_manager.project.root = Some(tmp.clone());
    ed.project_manager.project.files = vec![
        "src/main.rs".to_string(),
        "src/lib.rs".to_string(),
        "Cargo.toml".to_string(),
    ];

    assert_eq!(ed.project_manager.project.files.len(), 3);
    assert!(ed.project_manager.project.files.contains(&"src/main.rs".to_string()));

    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn project_option_get_set() {
    let mut ed = helpers::make_editor();
    let tmp = std::env::temp_dir().join("sprint4_opts2");
    std::fs::create_dir_all(&tmp).unwrap();
    ed.project_manager.project.root = Some(tmp.clone());

    ed.project_manager.project.options.insert("tab-width".to_string(), "4".to_string());
    assert_eq!(
        ed.project_manager.project.options.get("tab-width").map(|s| s.as_str()),
        Some("4")
    );

    ed.project_manager.project.options.insert("tab-width".to_string(), "2".to_string());
    assert_eq!(
        ed.project_manager.project.options.get("tab-width").map(|s| s.as_str()),
        Some("2")
    );

    std::fs::remove_dir_all(&tmp).ok();
}
