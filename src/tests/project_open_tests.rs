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
