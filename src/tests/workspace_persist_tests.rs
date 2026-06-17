//! Pure-Rust tests for Phase 8 — Workspace Persistence.

use crate::kernel::storage::{WorkspaceManager, WorkspaceState, PersistedBuffer};
use crate::kernel::state::Editor;
use crate::kernel::storage::disk::DiskFileSystem;
use crate::tests::helpers::make_editor;

fn bare_editor() -> Editor {
    Editor::new(Box::new(DiskFileSystem::new()))
}

// ── WorkspaceState basics ────────────────────────────────────────────────────

#[test]
fn default_workspace_state_is_empty() {
    let state = WorkspaceState::default();
    assert!(state.buffers.is_empty());
    assert!(state.command_history.is_empty());
    assert!(state.options.is_empty());
    assert!(state.registers.is_empty());
}

#[test]
fn persisted_buffer_default() {
    let pb = PersistedBuffer::default();
    assert!(pb.name.is_empty());
    assert!(pb.path.is_none());
    assert!(pb.content.is_none());
}

// ── WorkspaceManager::snapshot ───────────────────────────────────────────────

#[test]
fn snapshot_bare_editor_yields_no_buffers() {
    let ed = bare_editor();
    let state = WorkspaceManager::snapshot(&ed);
    assert_eq!(state.buffers.len(), 0);
}

#[test]
fn snapshot_captures_scratch_buffer_content() {
    let mut ed = bare_editor();
    ed.create_buffer_from_str("scratch", "hello world");
    let state = WorkspaceManager::snapshot(&ed);
    assert_eq!(state.buffers.len(), 1);
    let pb = &state.buffers[0];
    assert_eq!(pb.name, "scratch");
    assert!(pb.path.is_none());
    assert_eq!(pb.content.as_deref(), Some("hello world"));
}

#[test]
fn snapshot_file_buffer_has_path_but_no_content() {
    let mut ed = bare_editor();
    let key = ed.create_buffer_from_str("main.rs", "fn main() {}");
    ed.buffers.get(key).unwrap().lock().unwrap().path = Some("/tmp/main.rs".to_string());
    let state = WorkspaceManager::snapshot(&ed);
    assert_eq!(state.buffers.len(), 1);
    let pb = &state.buffers[0];
    assert_eq!(pb.path.as_deref(), Some("/tmp/main.rs"));
    assert!(pb.content.is_none(), "file-backed buffers must not duplicate content");
}

#[test]
fn snapshot_skips_ephemeral_buffers() {
    let mut ed = bare_editor();
    let key = ed.create_buffer_from_str("*temp*", "ephemeral content");
    ed.buffers.get(key).unwrap().lock().unwrap().ephemeral = true;
    let state = WorkspaceManager::snapshot(&ed);
    assert_eq!(state.buffers.len(), 0);
}

#[test]
fn snapshot_captures_command_history() {
    let mut ed = bare_editor();
    ed.command_history = vec!["w".to_string(), "q".to_string()];
    let state = WorkspaceManager::snapshot(&ed);
    assert_eq!(state.command_history, vec!["w", "q"]);
}

#[test]
fn snapshot_captures_search_pattern() {
    let mut ed = bare_editor();
    ed.search_pattern = Some("foo".to_string());
    let state = WorkspaceManager::snapshot(&ed);
    assert_eq!(state.search_pattern.as_deref(), Some("foo"));
}

#[test]
fn snapshot_captures_options() {
    let mut ed = bare_editor();
    ed.options.insert("tab-width".to_string(), "4".to_string());
    let state = WorkspaceManager::snapshot(&ed);
    assert_eq!(state.options.get("tab-width").map(|s| s.as_str()), Some("4"));
}

#[test]
fn snapshot_captures_registers() {
    let mut ed = bare_editor();
    ed.registers.insert("a".to_string(), "copied".to_string());
    let state = WorkspaceManager::snapshot(&ed);
    assert_eq!(state.registers.get("a").map(|s| s.as_str()), Some("copied"));
}

#[test]
fn snapshot_captures_plugin_state() {
    let mut ed = bare_editor();
    ed.plugin_state.insert("theme".to_string(), "dark".to_string());
    let state = WorkspaceManager::snapshot(&ed);
    assert_eq!(state.plugin_state.get("theme").map(|s| s.as_str()), Some("dark"));
}

#[test]
fn snapshot_multiple_buffers_mix() {
    let mut ed = bare_editor();
    ed.create_buffer_from_str("scratch", "notes");
    let key = ed.create_buffer_from_str("src.rs", "code");
    ed.buffers.get(key).unwrap().lock().unwrap().path = Some("/code/src.rs".to_string());
    let key2 = ed.create_buffer_from_str("*ephemeral*", "skip me");
    ed.buffers.get(key2).unwrap().lock().unwrap().ephemeral = true;
    let state = WorkspaceManager::snapshot(&ed);
    assert_eq!(state.buffers.len(), 2, "ephemeral buffer must be excluded");
    let has_scratch = state.buffers.iter().any(|b| b.name == "scratch" && b.content.is_some());
    let has_file = state.buffers.iter().any(|b| b.path.as_deref() == Some("/code/src.rs") && b.content.is_none());
    assert!(has_scratch, "scratch buffer must be snapshotted with content");
    assert!(has_file, "file buffer must be snapshotted with path only");
}

// ── WorkspaceManager::apply ──────────────────────────────────────────────────

#[test]
fn apply_empty_state_leaves_editor_clean() {
    let mut ed = make_editor();
    let initial_buf_count = ed.buffers.len();
    let state = WorkspaceState::default();
    let count = WorkspaceManager::apply(&mut ed, &state).unwrap();
    assert_eq!(count, 0);
    assert_eq!(ed.buffers.len(), initial_buf_count, "no new buffers must be created");
}

#[test]
fn apply_restores_scratch_buffer_content() {
    let mut ed = make_editor();
    let state = WorkspaceState {
        buffers: vec![PersistedBuffer {
            name: "notes".to_string(),
            path: None,
            content: Some("my notes".to_string()),
        }],
        ..Default::default()
    };
    let count = WorkspaceManager::apply(&mut ed, &state).unwrap();
    assert_eq!(count, 1);
    let found = ed.buffers.iter().any(|(_, arc)| {
        let b = arc.lock().unwrap();
        b.name == "notes" && b.slice(0, b.len()) == "my notes"
    });
    assert!(found, "scratch buffer content must be restored");
}

#[test]
fn apply_restores_options() {
    let mut ed = make_editor();
    let mut options = std::collections::HashMap::new();
    options.insert("indent-width".to_string(), "2".to_string());
    let state = WorkspaceState { options, ..Default::default() };
    WorkspaceManager::apply(&mut ed, &state).unwrap();
    assert_eq!(ed.options.get("indent-width").map(|s| s.as_str()), Some("2"));
}

#[test]
fn apply_restores_command_history() {
    let mut ed = make_editor();
    let state = WorkspaceState {
        command_history: vec!["e foo.rs".to_string()],
        ..Default::default()
    };
    WorkspaceManager::apply(&mut ed, &state).unwrap();
    assert_eq!(ed.command_history, vec!["e foo.rs"]);
}

#[test]
fn apply_restores_search_pattern() {
    let mut ed = make_editor();
    let state = WorkspaceState {
        search_pattern: Some("needle".to_string()),
        ..Default::default()
    };
    WorkspaceManager::apply(&mut ed, &state).unwrap();
    assert_eq!(ed.search_pattern.as_deref(), Some("needle"));
}

#[test]
fn apply_restores_registers() {
    let mut ed = make_editor();
    let mut regs = std::collections::HashMap::new();
    regs.insert("x".to_string(), "regval".to_string());
    let state = WorkspaceState { registers: regs, ..Default::default() };
    WorkspaceManager::apply(&mut ed, &state).unwrap();
    assert_eq!(ed.registers.get("x").map(|s| s.as_str()), Some("regval"));
}

#[test]
fn apply_skips_missing_file_buffer() {
    let mut ed = make_editor();
    let state = WorkspaceState {
        buffers: vec![PersistedBuffer {
            name: "gone.rs".to_string(),
            path: Some("/nonexistent/gone.rs".to_string()),
            content: None,
        }],
        ..Default::default()
    };
    let count = WorkspaceManager::apply(&mut ed, &state).unwrap();
    assert_eq!(count, 0, "missing file buffer must be silently skipped");
}

// ── save_to / restore_from (file-system round-trip) ─────────────────────────

#[test]
fn save_to_restore_from_round_trip_scratch_buffer() {
    let dir = tempdir_path("ws_rt_1");
    let mut ed = bare_editor();
    ed.create_buffer_from_str("memo", "round-trip content");
    ed.command_history = vec!["w".to_string()];
    ed.options.insert("wrap".to_string(), "true".to_string());

    WorkspaceManager::save_to(&ed, &dir).unwrap();
    assert!(dir.join("workspace.json").exists());

    let mut ed2 = bare_editor();
    let count = WorkspaceManager::restore_from(&mut ed2, &dir).unwrap();
    assert_eq!(count, 1);
    assert_eq!(ed2.command_history, vec!["w"]);
    assert_eq!(ed2.options.get("wrap").map(|s| s.as_str()), Some("true"));
    let found = ed2.buffers.iter().any(|(_, arc)| {
        let b = arc.lock().unwrap();
        b.name == "memo" && b.slice(0, b.len()) == "round-trip content"
    });
    assert!(found);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn restore_from_missing_file_returns_error() {
    let dir = tempdir_path("ws_rt_missing");
    let mut ed = bare_editor();
    let result = WorkspaceManager::restore_from(&mut ed, &dir);
    assert!(result.is_err());
}

#[test]
fn workspace_json_is_valid_json() {
    let dir = tempdir_path("ws_json");
    let mut ed = bare_editor();
    ed.create_buffer_from_str("buf", "data");
    WorkspaceManager::save_to(&ed, &dir).unwrap();
    let content = std::fs::read_to_string(dir.join("workspace.json")).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).expect("workspace.json must be valid JSON");
    assert!(parsed.is_object());
    std::fs::remove_dir_all(&dir).ok();
}

// ── Named sessions ────────────────────────────────────────────────────────────

#[test]
fn session_save_load_round_trip() {
    let sessions_dir = tempdir_path("ws_sessions");
    let mut ed = bare_editor();
    ed.create_buffer_from_str("session-buf", "session content");
    ed.registers.insert("x".to_string(), "reg-value".to_string());

    WorkspaceManager::session_save_to(&ed, "mywork", &sessions_dir).unwrap();
    let path = sessions_dir.join("mywork.json");
    assert!(path.exists());

    let mut ed2 = bare_editor();
    let count = WorkspaceManager::session_load_from(&mut ed2, "mywork", &sessions_dir).unwrap();
    assert_eq!(count, 1);
    assert_eq!(ed2.registers.get("x").map(|s| s.as_str()), Some("reg-value"));
    std::fs::remove_dir_all(&sessions_dir).ok();
}

#[test]
fn session_list_from_returns_saved_sessions() {
    let sessions_dir = tempdir_path("ws_list");
    let ed = bare_editor();
    WorkspaceManager::session_save_to(&ed, "alpha", &sessions_dir).unwrap();
    WorkspaceManager::session_save_to(&ed, "beta", &sessions_dir).unwrap();

    let mut list = WorkspaceManager::session_list_from(&sessions_dir);
    list.sort();
    assert_eq!(list, vec!["alpha", "beta"]);
    std::fs::remove_dir_all(&sessions_dir).ok();
}

#[test]
fn session_list_from_empty_dir_returns_empty() {
    let sessions_dir = tempdir_path("ws_list_empty");
    std::fs::create_dir_all(&sessions_dir).ok();
    let list = WorkspaceManager::session_list_from(&sessions_dir);
    assert!(list.is_empty());
    std::fs::remove_dir_all(&sessions_dir).ok();
}

#[test]
fn session_filename_sanitises_special_chars() {
    let sessions_dir = tempdir_path("ws_sanitise");
    let ed = bare_editor();
    WorkspaceManager::session_save_to(&ed, "my/session", &sessions_dir).unwrap();
    let entries: Vec<_> = std::fs::read_dir(&sessions_dir)
        .unwrap()
        .filter_map(|e| Some(e.ok()?.file_name().to_string_lossy().to_string()))
        .collect();
    assert!(entries.iter().all(|n| !n.contains('/')));
    std::fs::remove_dir_all(&sessions_dir).ok();
}

#[test]
fn session_load_missing_returns_error() {
    let sessions_dir = tempdir_path("ws_load_missing");
    std::fs::create_dir_all(&sessions_dir).ok();
    let mut ed = bare_editor();
    let result = WorkspaceManager::session_load_from(&mut ed, "nonexistent", &sessions_dir);
    assert!(result.is_err());
    std::fs::remove_dir_all(&sessions_dir).ok();
}

// ── helpers ──────────────────────────────────────────────────────────────────

fn tempdir_path(tag: &str) -> std::path::PathBuf {
    let path = std::path::PathBuf::from(format!("/tmp/magma_test_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&path).ok();
    path
}
