use crate::kernel::vc::backends::{GitBackend, HgBackend, JjBackend, SvnBackend};
use crate::kernel::vc::{run_cmd, FileStatus, VcBackend, VcState, VcStatus};

// ── VcState ────────────────────────────────────────────────────────────────

#[test]
fn vc_state_new_defaults() {
    let vc = VcState::new();
    assert_eq!(vc.backends.len(), 4);
    assert!(vc.active_backend.is_none());
    assert!(vc.repo_root.is_none());
    assert!(vc.last_status.is_none());
    assert!(!vc.request_open, "the panel starts closed");
    assert!(vc.diff_view.is_none(), "no diff view is open");
    assert!(vc.stashes.is_empty());
}

#[test]
fn vc_state_builtin_backend_names() {
    let vc = VcState::new();
    assert_eq!(vc.backends[0].name(), "git");
    assert_eq!(vc.backends[1].name(), "hg");
    assert_eq!(vc.backends[2].name(), "svn");
    assert_eq!(vc.backends[3].name(), "jj");
}

#[test]
fn vc_state_detect_for_returns_none_in_temp_dir() {
    let vc = VcState::new();
    let tmp = std::env::temp_dir().join("magma_vc_detect_none");
    std::fs::create_dir_all(&tmp).unwrap();
    let result = vc.detect_for(&tmp);
    assert!(result.is_none());
    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn vc_state_backend_none_by_default() {
    let vc = VcState::new();
    assert!(vc.backend().is_none());
}

#[test]
fn vc_state_backend_returns_some_when_active() {
    let mut vc = VcState::new();
    vc.active_backend = Some(0);
    let b = vc.backend();
    assert!(b.is_some());
    assert_eq!(b.unwrap().name(), "git");
}

#[test]
fn vc_state_backend_returns_none_for_bad_index() {
    let mut vc = VcState::new();
    vc.active_backend = Some(99);
    assert!(vc.backend().is_none());
}

#[test]
fn vc_state_debug_does_not_panic() {
    let vc = VcState::new();
    let _ = format!("{:?}", vc);
}

// ── FileStatus & VcStatus ──────────────────────────────────────────────────

#[test]
fn file_status_construct() {
    let fs = FileStatus { code: 'M', path: "src/main.rs".to_string() };
    assert_eq!(fs.code, 'M');
    assert_eq!(fs.path, "src/main.rs");
}

#[test]
fn vc_status_construct() {
    let s = VcStatus {
        backend_name: "git".to_string(),
        branch: "main".to_string(),
        has_staging: true,
        staged: vec![],
        unstaged: vec![],
        untracked: vec![],
            conflicts: vec![],
    };
    assert_eq!(s.branch, "main");
    assert!(s.has_staging);
    assert!(s.staged.is_empty());
    assert!(s.unstaged.is_empty());
    assert!(s.untracked.is_empty());
}

// ── run_cmd ────────────────────────────────────────────────────────────────

#[test]
fn run_cmd_success() {
    let tmp = std::env::temp_dir();
    let r = run_cmd(&tmp, "echo", &["magma-vc-test"]);
    assert!(r.is_ok());
    assert_eq!(r.unwrap().trim(), "magma-vc-test");
}

#[test]
fn run_cmd_not_found() {
    let tmp = std::env::temp_dir();
    let r = run_cmd(&tmp, "nonexistent_magma_vc_cmd", &[]);
    assert!(r.is_err());
}

// ── Backend name() ─────────────────────────────────────────────────────────

#[test]
fn backend_names() {
    assert_eq!(GitBackend.name(), "git");
    assert_eq!(HgBackend.name(), "hg");
    assert_eq!(SvnBackend.name(), "svn");
    assert_eq!(JjBackend.name(), "jj");
}

// ── ShellTemplateBackend ───────────────────────────────────────────────────


