//! Tests for the run-configuration subsystem: persistence, defaults,
//! args/cwd execution through the TaskScheduler, rerun, stop, and
//! build-before-run chaining.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crate::kernel::project::{ProjectManager, open_root};
use crate::kernel::run_config::{RunConfig, RunConfigManager, defaults_for};
use crate::kernel::runtime::{BackgroundEvent, BackgroundHandle};
use crate::kernel::state::Editor;
use crate::kernel::task::TaskScheduler;
use crate::kernel::storage::disk::DiskFileSystem;

fn make_config(name: &str, command: &str, args: &[&str]) -> RunConfig {
    RunConfig {
        name: name.to_string(),
        command: command.to_string(),
        args: args.iter().map(|s| s.to_string()).collect(),
        cwd: None,
        env: Default::default(),
        build_before_run: None,
        debuggable: false,
    }
}

fn bg_pair() -> (Arc<tokio::runtime::Runtime>, BackgroundHandle, tokio::sync::mpsc::UnboundedReceiver<BackgroundEvent>) {
    let runtime = Arc::new(tokio::runtime::Runtime::new().unwrap());
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let handle = BackgroundHandle::new(runtime.clone(), tx);
    (runtime, handle, rx)
}

fn temp_root(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("run_config_{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

// ── Persistence ───────────────────────────────────────────────────────────────

#[test]
fn save_then_load_round_trips_configs_and_active() {
    let root = temp_root("roundtrip");
    let mut store = RunConfigManager::default();
    let mut cfg = make_config("launch", "/bin/echo", &["hi"]);
    cfg.cwd = Some("sub".to_string());
    cfg.build_before_run = Some("build".to_string());
    cfg.debuggable = true;
    store.upsert(cfg);
    store.upsert(make_config("build", "make", &[]));
    store.active = Some("launch".to_string());

    store.save(&root).unwrap();

    let mut loaded = RunConfigManager::default();
    assert!(loaded.load(&root), "saved file must load");
    assert_eq!(loaded.configs.len(), 2);
    let launch = loaded.named("launch").unwrap();
    assert_eq!(launch.args, vec!["hi"]);
    assert_eq!(launch.cwd.as_deref(), Some("sub"));
    assert_eq!(launch.build_before_run.as_deref(), Some("build"));
    assert!(launch.debuggable);
    assert_eq!(loaded.active.as_deref(), Some("launch"));

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn to_launch_args_maps_program_args_name_and_cwd() {
    let root = PathBuf::from("/proj");
    let mut cfg = make_config("run", "/bin/echo", &["hi"]);
    cfg.cwd = Some("subdir".to_string());

    let args = cfg.to_launch_args(&root);

    assert_eq!(args["request"], "launch");
    assert_eq!(args["program"], "/bin/echo");
    assert_eq!(args["args"][0], "hi");
    assert_eq!(args["name"], "run");
    assert_eq!(args["cwd"], "/proj/subdir",
        "a relative cwd resolves against the project root");

    cfg.cwd = Some("/abs/dir".to_string());
    assert_eq!(cfg.to_launch_args(&root)["cwd"], "/abs/dir",
        "an absolute cwd is not re-rooted under the project");
}

#[test]
fn load_missing_file_returns_false() {
    let root = temp_root("missing");
    let mut store = RunConfigManager::default();
    assert!(!store.load(&root));
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn load_ignores_unknown_active_name() {
    let root = temp_root("badactive");
    let mag = root.join(".magma");
    std::fs::create_dir_all(&mag).unwrap();
    std::fs::write(mag.join("run-configs.json"),
        r#"{"configs":[{"name":"a","command":"x"}],"active":"nope"}"#).unwrap();
    let mut store = RunConfigManager::default();
    assert!(store.load(&root));
    assert_eq!(store.active, None, "dangling active must not survive a load");
    std::fs::remove_dir_all(&root).ok();
}

// ── Detection defaults ────────────────────────────────────────────────────────

#[test]
fn defaults_cover_known_languages_with_expected_shapes() {
    let rust = defaults_for("rust");
    assert_eq!(rust.len(), 3);
    let run = rust.iter().find(|c| c.name == "run").unwrap();
    assert_eq!(run.command, "cargo");
    assert_eq!(run.args, vec!["run"]);
    assert!(run.debuggable, "cargo run is the debuggable default");

    assert!(defaults_for("python").iter().all(|c| c.build_before_run.is_none()));
    assert!(defaults_for("cobol").is_empty(), "unknown language gets no defaults");
}

#[test]
fn load_or_defaults_prefers_disk_and_active_selection() {
    let root = temp_root("ordefaults");
    let mag = root.join(".magma");
    std::fs::create_dir_all(&mag).unwrap();
    std::fs::write(mag.join("run-configs.json"),
        r#"{"configs":[{"name":"only","command":"true"}],"active":"only"}"#).unwrap();

    let mut store = RunConfigManager::default();
    store.load_or_defaults(&root, "rust");
    assert_eq!(store.configs.len(), 1, "disk wins over defaults");
    assert_eq!(store.active.as_deref(), Some("only"));

    let empty_root = temp_root("ordefaults_empty");
    let mut store2 = RunConfigManager::default();
    store2.load_or_defaults(&empty_root, "rust");
    assert_eq!(store2.configs.len(), 3, "no file → detection defaults");
    std::fs::remove_dir_all(&root).ok();
    std::fs::remove_dir_all(&empty_root).ok();
}

// ── Execution through the TaskScheduler ───────────────────────────────────────

fn wait_for_completions(
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<BackgroundEvent>,
    mut ed: Option<&mut Editor>,
    expect: usize,
) -> Vec<(i32, String)> {
    let mut done = Vec::new();
    for _ in 0..300 {
        while let Ok(ev) = rx.try_recv() {
            if let BackgroundEvent::TaskRunCompleted { id: _, exit_code, ref stdout, .. } = ev {
                done.push((exit_code, stdout.clone()));
                // Full runtime path: marks the task finished and runs the
                // run-config continuation hook.
                if let Some(ed) = ed.as_deref_mut() {
                    crate::kernel::runtime::process_background_event(ed, ev);
                }
            }
        }
        if done.len() >= expect { return done; }
        std::thread::sleep(Duration::from_millis(20));
    }
    done
}

#[test]
fn args_mode_spawns_program_directly() {
    let (_rt, bg, mut rx) = bg_pair();
    let mut sched = TaskScheduler::new();
    let root = temp_root("argsmode");

    let mut store = RunConfigManager::default();
    let id = store.launch_config_helper(make_config("greet", "/bin/echo", &["hello", "world"]), &mut sched, &bg, &root);
    let id = id.unwrap();

    let completions = wait_for_completions(&mut rx, None, 1);
    assert_eq!(completions.len(), 1, "echo must complete");
    let (code, out) = &completions[0];
    assert_eq!(*code, 0, "echo exits 0 (task {id})");
    assert!(out.contains("hello world"), "stdout carries the args: {out}");

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn shell_mode_still_uses_sh_c() {
    let (_rt, bg, mut rx) = bg_pair();
    let mut sched = TaskScheduler::new();
    let root = temp_root("shellmode");

    let mut store = RunConfigManager::default();
    store.launch_config_helper(make_config("greet", "echo inline", &[]), &mut sched, &bg, &root).unwrap();

    let completions = wait_for_completions(&mut rx, None, 1);
    assert_eq!(completions.len(), 1);
    assert!(completions[0].1.contains("inline"));

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn relative_cwd_resolves_against_project_root() {
    let (_rt, bg, mut rx) = bg_pair();
    let mut sched = TaskScheduler::new();
    let root = temp_root("cwd");
    std::fs::create_dir_all(root.join("sub")).unwrap();

    let mut cfg = make_config("where", "sh", &["-c", "pwd"]);
    cfg.cwd = Some("sub".to_string());
    let mut store = RunConfigManager::default();
    store.launch_config_helper(cfg, &mut sched, &bg, &root).unwrap();

    let completions = wait_for_completions(&mut rx, None, 1);
    assert!(completions[0].1.trim().ends_with("sub"), "cwd must resolve to root/sub: {:?}", completions);

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn finished_config_can_rerun() {
    let (_rt, bg, mut rx) = bg_pair();
    let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
    let root = temp_root("rerun");

    // Launch through the editor's scheduler: the runtime completion path
    // marks that one finished, and a finished task must relaunch.
    let mut store = RunConfigManager::default();
    store.launch_config_helper(make_config("greet", "/bin/echo", &["once"]), &mut ed.task_scheduler, &bg, &root).unwrap();
    wait_for_completions(&mut rx, Some(&mut ed), 1);

    store.launch_config_helper(make_config("greet", "/bin/echo", &["twice"]), &mut ed.task_scheduler, &bg, &root)
        .expect("a completed config must be launchable again");
    let completions = wait_for_completions(&mut rx, Some(&mut ed), 1);
    assert_eq!(completions.len(), 1);
    assert!(completions[0].1.contains("twice"));

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn stop_cancels_the_running_task() {
    let (_rt, bg, _rx) = bg_pair();
    let mut sched = TaskScheduler::new();
    let root = temp_root("stop");

    let mut store = RunConfigManager::default();
    store.launch_config_helper(make_config("slow", "sleep", &["5"]), &mut sched, &bg, &root).unwrap();
    store.stop("slow", &mut sched).expect("stop must find the running task");
    store.launch_config_helper(make_config("slow", "sleep", &["0"]), &mut sched, &bg, &root)
        .expect("after a cancel the config must be launchable again");
    std::fs::remove_dir_all(&root).ok();
}

// ── Build-before-run chaining (via the editor's runtime hook) ─────────────────

#[test]
fn build_before_run_launches_target_after_build_succeeds() {
    let (_rt, bg, mut rx) = bg_pair();
    let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
    ed.background = Some(bg);
    let root = temp_root("chain");
    open_root(&mut ed, &root.to_string_lossy());

    let mut build = make_config("build", "/bin/echo", &["building"]);
    build.name = "build".to_string();
    let mut run = make_config("serve", "/bin/echo", &["serving"]);
    run.build_before_run = Some("build".to_string());
    ed.project_manager.run_configs.upsert(build);
    ed.project_manager.run_configs.upsert(run);

    let dep_id = ed.project_manager
        .launch_config("serve", &mut ed.task_scheduler, ed.background.as_ref().unwrap())
        .expect("launch with build-first must start the build step");
    assert_eq!(ed.project_manager.run_configs.pending.as_ref().map(|(id, _)| *id), Some(dep_id));

    // The build completes → continue_pending launches the target.
    let completions = wait_for_completions(&mut rx, Some(&mut ed), 2);
    assert_eq!(completions.len(), 2, "both build and target must complete");
    assert!(completions.iter().any(|(_, out)| out.contains("building")));
    assert!(completions.iter().any(|(_, out)| out.contains("serving")));
    assert!(ed.project_manager.run_configs.pending.is_none(), "chain clears when done");

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn failed_build_blocks_the_target() {
    let (_rt, bg, mut rx) = bg_pair();
    let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
    ed.background = Some(bg);
    let root = temp_root("chainfail");
    open_root(&mut ed, &root.to_string_lossy());

    let mut fail = make_config("build", "sh", &["-c", "exit 3"]);
    fail.name = "build".to_string();
    let mut run = make_config("serve", "/bin/echo", &["never"]);
    run.build_before_run = Some("build".to_string());
    ed.project_manager.run_configs.upsert(fail);
    ed.project_manager.run_configs.upsert(run);

    ed.project_manager
        .launch_config("serve", &mut ed.task_scheduler, ed.background.as_ref().unwrap())
        .unwrap();

    let completions = wait_for_completions(&mut rx, Some(&mut ed), 1);
    assert_eq!(completions.len(), 1, "only the failing build runs");
    assert_eq!(completions[0].0, 3);
    assert!(ed.project_manager.run_configs.pending.is_none());

    std::fs::remove_dir_all(&root).ok();
}

// ── ProjectManager integration ────────────────────────────────────────────────

#[test]
fn detect_then_launch_config_runs_the_default_test_config() {
    let (_rt, bg, mut rx) = bg_pair();
    let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
    ed.background = Some(bg);
    let root = temp_root("detect_launch");
    std::fs::write(root.join("Cargo.toml"), "[package]").unwrap();
    let mut pm = ProjectManager::default();
    pm.detect(&root.to_string_lossy());
    pm.project.root = Some(root.clone());
    // Swap cargo test for an echo so the test does not build anything real.
    pm.run_configs.upsert(make_config("test", "/bin/echo", &["ran-test"]));

    let id = pm.launch_config("test", &mut ed.task_scheduler, ed.background.as_ref().unwrap()).unwrap();
    let task = ed.task_scheduler.tasks.get(&id).unwrap();
    assert_eq!(task.command, "/bin/echo");
    assert_eq!(task.args, vec!["ran-test"]);

    let completions = wait_for_completions(&mut rx, None, 1);
    assert!(completions[0].1.contains("ran-test"));

    std::fs::remove_dir_all(&root).ok();
}

/// Test shim for `RunConfigManager::launch_task` — private by design, so
/// tests go through the manager with a throwaway store.
impl RunConfigManager {
    fn launch_config_helper(
        &mut self,
        cfg: RunConfig,
        scheduler: &mut TaskScheduler,
        bg: &BackgroundHandle,
        root: &std::path::Path,
    ) -> Result<u64, String> {
        self.upsert(cfg);
        let name = self.configs.last().unwrap().name.clone();
        self.launch(&name, scheduler, bg, root)
    }
}
