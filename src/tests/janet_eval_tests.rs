//! Pure-Rust tests for the Janet Eval and MShell primitives (Sprint 12).
//! Only tests that do not require the Janet VM to be initialised.

use crate::kernel::state::Editor;
use crate::kernel::command::execute_command;
use crate::tests::helpers;

// ── Command registration ──────────────────────────────────────────────────────

#[test]
fn eval_region_is_registered() {
    let ed = helpers::make_editor();
    assert!(ed.commands.get_entry("eval-region").is_some());
}

#[test]
fn eval_buffer_is_registered() {
    let ed = helpers::make_editor();
    assert!(ed.commands.get_entry("eval-buffer").is_some());
}

// ── eval-region without selection ─────────────────────────────────────────────
// This test is safe without Janet because the command returns Err before the
// Janet block is entered (early return on no selection).

#[test]
fn eval_region_no_selection_returns_err() {
    let mut ed = helpers::make_editor();
    let result = execute_command(&mut ed, "eval-region", &std::collections::HashMap::new());
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("No selection"));
}

// ── process/spawn state ───────────────────────────────────────────────────────

#[test]
fn process_state_starts_empty() {
    let ed = helpers::make_editor();
    assert!(ed.io.processes.is_empty());
}

// ── filesystem primitives (std::env, no Janet) ───────────────────────────────

#[test]
fn cwd_is_accessible_via_std_env() {
    assert!(std::env::current_dir().is_ok());
}

#[test]
fn chdir_round_trip() {
    let original = std::env::current_dir().unwrap();
    std::env::set_current_dir("/tmp").unwrap();
    let new_cwd = std::env::current_dir().unwrap();
    assert!(new_cwd.to_string_lossy().contains("tmp"));
    std::env::set_current_dir(original).unwrap();
}
