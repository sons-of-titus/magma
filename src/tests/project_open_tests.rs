//! Opening a project from the command surface.

use std::collections::HashMap;

use crate::kernel::command::{self, builtin, args::ArgValue};
use crate::kernel::state::Editor;
use crate::kernel::storage::disk::DiskFileSystem;

fn editor() -> Editor {
    let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
    builtin::register_builtin_commands(&mut ed);
    ed
}

fn run(ed: &mut Editor, cmd: &str, path: &str) -> Result<(), String> {
    let mut args = HashMap::new();
    if !path.is_empty() {
        args.insert("path".to_string(), ArgValue::Path(path.to_string()));
    }
    command::execute_command(ed, cmd, &args)
}

#[test]
fn open_project_sets_root_name_and_recent() {
    let dir = std::env::temp_dir().join(format!("magma-open-proj-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("main.rs"), "fn main() {}").unwrap();

    let mut ed = editor();
    run(&mut ed, "open-project", dir.to_str().unwrap()).unwrap();

    let pm = &ed.project_manager;
    assert_eq!(pm.project.root.as_ref(), Some(&dir));
    assert_eq!(pm.project.name.as_deref(), Some(dir.file_name().unwrap().to_str().unwrap()));
    assert!(pm.recent_projects.first().is_some_and(|p| p == dir.to_str().unwrap()),
        "recents: {:?}", pm.recent_projects);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn open_project_rejects_non_directory() {
    let mut ed = editor();
    let err = run(&mut ed, "open-project", "/nonexistent/dir/for/magma").unwrap_err();
    assert!(err.contains("Not a directory"), "got: {err}");
    assert!(ed.project_manager.project.root.is_none());
}

#[test]
fn index_project_without_project_errors() {
    let mut ed = editor();
    let err = run(&mut ed, "index-project", "").unwrap_err();
    assert!(err.contains("No project open"), "got: {err}");
}

// ── Index contract: project-relative paths ───────────────────────────────────

fn editor_with_background()
    -> (Editor, tokio::sync::mpsc::UnboundedReceiver<crate::kernel::runtime::BackgroundEvent>) {
    let mut ed = editor();
    let rt = std::sync::Arc::new(tokio::runtime::Runtime::new().unwrap());
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    ed.background = Some(crate::kernel::runtime::BackgroundHandle::new(rt, tx));
    (ed, rx)
}

#[test]
fn open_project_indexes_files_relative_to_root() {
    let dir = std::env::temp_dir().join(format!("magma-rel-index-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(dir.join("src/main.rs"), "fn main() {}").unwrap();
    std::fs::write(dir.join("README.md"), "# t").unwrap();

    let (mut ed, mut rx) = editor_with_background();
    run(&mut ed, "open-project", dir.to_str().unwrap()).unwrap();

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let files = loop {
        match rx.try_recv() {
            Ok(crate::kernel::runtime::BackgroundEvent::FileIndexed { files, .. }) => break files,
            _ if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            _ => panic!("no FileIndexed event arrived"),
        }
    };

    assert!(files.contains(&"src/main.rs".to_string()),
        "indexed paths must be project-relative, got: {files:?}");
    assert!(files.iter().all(|f| !f.starts_with('/')),
        "absolute paths would root the explorer at the filesystem root: {files:?}");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn open_project_expands_tilde() {
    let Ok(home) = std::env::var("HOME") else { return };
    let name = format!(".magma-tilde-test-{}", std::process::id());
    let dir = std::path::Path::new(&home).join(&name);
    std::fs::create_dir_all(&dir).unwrap();

    let mut ed = editor();
    run(&mut ed, "open-project", &format!("~/{name}")).unwrap();

    assert_eq!(ed.project_manager.project.root.as_ref(), Some(&dir));
    std::fs::remove_dir_all(&dir).ok();
}
