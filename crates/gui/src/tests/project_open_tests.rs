//! Opening a project from the command surface.

use std::collections::HashMap;

use magma::kernel::command::{self, args::ArgValue, builtin};
use magma::kernel::state::Editor;
use magma::kernel::storage::disk::DiskFileSystem;

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
    assert_eq!(
        pm.project.name.as_deref(),
        Some(dir.file_name().unwrap().to_str().unwrap())
    );
    assert!(
        pm.recent_projects
            .first()
            .is_some_and(|p| p == dir.to_str().unwrap()),
        "recents: {:?}",
        pm.recent_projects
    );
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

#[test]
fn closed_project_can_be_reopened() {
    let dir = std::env::temp_dir().join(format!("magma-reopen-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("Cargo.toml"), "[package]").unwrap();
    let path = dir.to_str().unwrap();

    let (mut ed, _rx) = editor_with_background();
    run(&mut ed, "open-project", path).unwrap();
    assert_eq!(
        ed.project_manager.project.root.as_deref(),
        Some(dir.as_path())
    );

    crate::render::gui_shell::close_editor_session(&mut ed);
    assert!(
        ed.project_manager.project.root.is_none(),
        "close must clear the root"
    );
    assert!(
        ed.project_manager.recent_projects.iter().any(|p| p == path),
        "a closed project stays in recents: {:?}",
        ed.project_manager.recent_projects
    );

    // Quitting is what drives the close: it flips `running` off and the shell
    // returns to the welcome window.
    ed.running = false;
    run(&mut ed, "open-project", path).unwrap();
    assert_eq!(
        ed.project_manager.project.root.as_deref(),
        Some(dir.as_path()),
        "a closed project must reopen"
    );
    assert!(
        ed.running,
        "reopening re-arms the session, or the editor window closes straight back to welcome"
    );

    std::fs::remove_dir_all(&dir).ok();
}

// ── Index contract: project-relative paths ───────────────────────────────────
fn editor_with_background() -> (
    Editor,
    tokio::sync::mpsc::UnboundedReceiver<magma::kernel::runtime::BackgroundEvent>,
) {
    let mut ed = editor();
    let rt = std::sync::Arc::new(tokio::runtime::Runtime::new().unwrap());
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    ed.background = Some(magma::kernel::runtime::BackgroundHandle::new(rt, tx));
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
            Ok(magma::kernel::runtime::BackgroundEvent::Index(
                magma::kernel::index::Event::ProjectIndexed { files, .. },
            )) => break files,
            _ if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            _ => panic!("no ProjectIndexed event arrived"),
        }
    };

    assert!(
        files.contains(&"src/main.rs".to_string()),
        "indexed paths must be project-relative, got: {files:?}"
    );
    assert!(
        files.iter().all(|f| !f.starts_with('/')),
        "absolute paths would root the explorer at the filesystem root: {files:?}"
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn most_recent_existing_project_skips_missing_directories() {
    let dir = std::env::temp_dir().join(format!("magma-mru-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    let recents = vec![
        "/nonexistent/magma/project".to_string(),
        dir.to_string_lossy().into_owned(),
    ];
    assert_eq!(
        magma::kernel::state::persist::most_recent_existing_project(&recents).as_deref(),
        Some(dir.to_str().unwrap()),
        "a vanished most-recent project must fall through to the next live one"
    );
    assert_eq!(
        magma::kernel::state::persist::most_recent_existing_project(&[]),
        None
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn open_project_expands_tilde() {
    let Ok(home) = std::env::var("HOME") else {
        return;
    };
    let name = format!(".magma-tilde-test-{}", std::process::id());
    let dir = std::path::Path::new(&home).join(&name);
    std::fs::create_dir_all(&dir).unwrap();

    let mut ed = editor();
    run(&mut ed, "open-project", &format!("~/{name}")).unwrap();

    assert_eq!(ed.project_manager.project.root.as_ref(), Some(&dir));
    std::fs::remove_dir_all(&dir).ok();
}

// ── find-files off the UI thread ─────────────────────────────────────

#[test]
fn find_files_walks_on_the_pool_and_presents_results() {
    use magma::kernel::command::{args::ArgValue, execute_command};
    use magma::kernel::runtime::{BackgroundEvent, process_background_event};
    use std::time::{Duration, Instant};

    let root = std::env::temp_dir().join("magma_find_files_async");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("src/deep")).unwrap();
    std::fs::create_dir_all(root.join("target")).unwrap();
    std::fs::write(root.join("src/main.rs"), "").unwrap();
    std::fs::write(root.join("src/deep/util.rs"), "").unwrap();
    std::fs::write(root.join("target/artifact.o"), "").unwrap();

    let (mut ed, _bg, mut rx) = crate::tests::helpers::editor_with_background();
    // Search is project-scoped: the walk starts at the open project root.
    let mut open_args = std::collections::HashMap::new();
    open_args.insert(
        "path".to_string(),
        magma::kernel::command::args::ArgValue::Path(root.to_string_lossy().into_owned()),
    );
    execute_command(&mut ed, "open-project", &open_args).unwrap();

    let mut args = std::collections::HashMap::new();
    args.insert("pattern".to_string(), ArgValue::String("util".to_string()));
    execute_command(&mut ed, "find-files", &args).unwrap();

    // The command returns before the walk finishes; the results land as
    // a finder buffer from the Project(FileSearchListed) event.
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut presented = false;
    while Instant::now() < deadline {
        while let Ok(ev) = rx.try_recv() {
            if let BackgroundEvent::Project(magma::kernel::project::Event::FileSearchListed {
                files,
                ..
            }) = &ev
                && files.iter().any(|f| f.ends_with("util.rs"))
            {
                presented = true;
            }
            process_background_event(&mut ed, ev);
        }
        if presented {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        presented,
        "the walk must complete off-thread and land the results"
    );

    let finder = ed
        .buffers
        .iter()
        .find(|(_, a)| a.lock().unwrap().name.starts_with("*finder:"));
    assert!(finder.is_some(), "a finder buffer was presented");
    let text = finder.unwrap().1.lock().unwrap().slice(0, 40);
    assert!(
        text.contains("src/deep/util.rs"),
        "results carry relative paths"
    );
    assert!(!text.contains("target"), "ignored dirs are skipped");
    let _ = std::fs::remove_dir_all(&root);
}
