use std::path::Path;

use crate::kernel::vc::backends::{GitBackend, HgBackend, JjBackend, SvnBackend};
use crate::kernel::vc::{build_display, path_at_line, run_cmd, FileStatus, VcBackend, VcState, VcStatus, VC_HEADER_LINES};

// ── VcState ────────────────────────────────────────────────────────────────

#[test]
fn vc_state_new_defaults() {
    let vc = VcState::new();
    assert_eq!(vc.backends.len(), 4);
    assert!(vc.active_backend.is_none());
    assert!(vc.buf_key.is_none());
    assert!(vc.repo_root.is_none());
    assert!(vc.last_status.is_none());
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
    };
    assert_eq!(s.branch, "main");
    assert!(s.has_staging);
    assert!(s.staged.is_empty());
    assert!(s.unstaged.is_empty());
    assert!(s.untracked.is_empty());
}

// ── build_display ──────────────────────────────────────────────────────────

#[test]
fn build_display_clean() {
    let status = VcStatus {
        backend_name: "git".to_string(),
        branch: "main".to_string(),
        has_staging: true,
        staged: vec![],
        unstaged: vec![],
        untracked: vec![],
    };
    let out = build_display(Path::new("/repo"), &status);
    assert!(out.contains("## main  [git]  /repo"));
    assert!(out.contains("Nothing to commit, working tree clean"));
}

#[test]
fn build_display_staged() {
    let status = VcStatus {
        backend_name: "git".to_string(),
        branch: "main".to_string(),
        has_staging: true,
        staged: vec![FileStatus { code: 'M', path: "src/main.rs".to_string() }],
        unstaged: vec![],
        untracked: vec![],
    };
    let out = build_display(Path::new("/repo"), &status);
    assert!(out.contains("Staged:"));
    assert!(out.contains("M  src/main.rs"));
    assert!(!out.contains("Nothing to commit"));
    assert!(out.contains("s stage"));
}

#[test]
fn build_display_unstaged() {
    let status = VcStatus {
        backend_name: "git".to_string(),
        branch: "main".to_string(),
        has_staging: true,
        staged: vec![],
        unstaged: vec![FileStatus { code: 'M', path: "src/lib.rs".to_string() }],
        untracked: vec![],
    };
    let out = build_display(Path::new("/repo"), &status);
    assert!(out.contains("Unstaged:"));
    assert!(out.contains("M  src/lib.rs"));
}

#[test]
fn build_display_untracked() {
    let status = VcStatus {
        backend_name: "git".to_string(),
        branch: "main".to_string(),
        has_staging: true,
        staged: vec![],
        unstaged: vec![],
        untracked: vec!["new_file.txt".to_string()],
    };
    let out = build_display(Path::new("/repo"), &status);
    assert!(out.contains("Untracked:"));
    assert!(out.contains("?  new_file.txt"));
}

#[test]
fn build_display_mixed() {
    let status = VcStatus {
        backend_name: "git".to_string(),
        branch: "main".to_string(),
        has_staging: true,
        staged: vec![FileStatus { code: 'A', path: "new.rs".to_string() }],
        unstaged: vec![FileStatus { code: 'M', path: "old.rs".to_string() }],
        untracked: vec!["ignored.log".to_string()],
    };
    let out = build_display(Path::new("/repo"), &status);
    assert!(out.contains("Staged:"));
    assert!(out.contains("A  new.rs"));
    assert!(out.contains("Unstaged:"));
    assert!(out.contains("M  old.rs"));
    assert!(out.contains("Untracked:"));
    assert!(out.contains("?  ignored.log"));
    assert!(out.contains("s stage"));
    assert!(!out.contains("Changed:"));
}

#[test]
fn build_display_no_staging_footer_omits_stage_commands() {
    let status = VcStatus {
        backend_name: "hg".to_string(),
        branch: "default".to_string(),
        has_staging: false,
        staged: vec![],
        unstaged: vec![FileStatus { code: 'M', path: "file.txt".to_string() }],
        untracked: vec![],
    };
    let out = build_display(Path::new("/repo"), &status);
    assert!(!out.contains("s stage"));
    assert!(!out.contains("u unstage"));
    assert!(out.contains("d diff"));
    assert!(out.contains("c commit"));
}

#[test]
fn build_display_no_staging_uses_changed_label() {
    let status = VcStatus {
        backend_name: "hg".to_string(),
        branch: "default".to_string(),
        has_staging: false,
        staged: vec![],
        unstaged: vec![FileStatus { code: 'M', path: "file.txt".to_string() }],
        untracked: vec![],
    };
    let out = build_display(Path::new("/repo"), &status);
    assert!(out.contains("Changed:"));
    assert!(!out.contains("Unstaged:"));
}

// ── path_at_line ───────────────────────────────────────────────────────────

#[test]
fn path_at_line_header_lines_return_none() {
    let status = VcStatus {
        backend_name: "git".to_string(),
        branch: "main".to_string(),
        has_staging: true,
        staged: vec![],
        unstaged: vec![],
        untracked: vec![],
    };
    for line in 0..VC_HEADER_LINES {
        assert!(path_at_line(&status, line).is_none(),
            "line {} should be header", line);
    }
}

#[test]
fn path_at_line_beyond_end_returns_none() {
    let status = VcStatus {
        backend_name: "git".to_string(),
        branch: "main".to_string(),
        has_staging: true,
        staged: vec![],
        unstaged: vec![],
        untracked: vec![],
    };
    assert!(path_at_line(&status, 100).is_none());
}

#[test]
fn path_at_line_returns_path_for_valid_line() {
    let status = VcStatus {
        backend_name: "git".to_string(),
        branch: "main".to_string(),
        has_staging: true,
        staged: vec![FileStatus { code: 'M', path: "staged.rs".to_string() }],
        unstaged: vec![FileStatus { code: 'M', path: "unstaged.rs".to_string() }],
        untracked: vec!["untracked.txt".to_string()],
    };
    // path_at_line maps files starting from VC_HEADER_LINES (5):
    // staged at 5, unstaged at 7 (after staged + blank), untracked after that
    let staged_line = VC_HEADER_LINES;
    let got = path_at_line(&status, staged_line);
    assert_eq!(got.as_deref(), Some("staged.rs"));
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


