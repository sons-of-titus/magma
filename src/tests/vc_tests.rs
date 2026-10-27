use crate::kernel::vc::backends::{GitBackend, HgBackend, JjBackend, SvnBackend};
use crate::kernel::vc::{FileStatus, VcBackend, VcState, VcStatus, run_cmd};

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
    let fs = FileStatus {
        code: 'M',
        path: "src/main.rs".to_string(),
    };
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

// ── Mutating-op tracking (Commit panel busy state) ─────────────────────────

mod pending_ops {
    use crate::kernel::command::execute_command;
    use crate::kernel::runtime::process_background_event;
    use crate::kernel::vc::{VcRemoteOp, run_cmd, run_op};
    use std::collections::HashMap;
    use std::path::Path;

    fn scratch_repo(tag: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "magma-vc-pending-{tag}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("a.txt"), "one\n").unwrap();
        run_cmd(&root, "git", &["init", "-q"]).unwrap_or_default();
        run_cmd(
            &root,
            "git",
            &["-c", "user.email=t@t", "-c", "user.name=t", "add", "."],
        )
        .unwrap_or_default();
        run_cmd(
            &root,
            "git",
            &[
                "-c",
                "user.email=t@t",
                "-c",
                "user.name=t",
                "commit",
                "-q",
                "-m",
                "init",
            ],
        )
        .unwrap_or_default();
        root
    }

    fn git_available() -> bool {
        run_cmd(Path::new("."), "git", &["--version"]).is_ok()
    }

    #[test]
    fn is_mutating_covers_rewrites_and_skips_reads() {
        assert!(VcRemoteOp::Stage("a".into()).is_mutating());
        assert!(VcRemoteOp::StageAll.is_mutating());
        assert!(VcRemoteOp::UnstageAll.is_mutating());
        assert!(VcRemoteOp::Commit("m".into()).is_mutating());
        assert!(
            VcRemoteOp::Checkout {
                branch: "b".into(),
                create: false
            }
            .is_mutating()
        );
        assert!(!VcRemoteOp::Log.is_mutating());
        assert!(!VcRemoteOp::Diff(None).is_mutating());
        assert!(!VcRemoteOp::ListBranches.is_mutating());
        assert!(!VcRemoteOp::StashList.is_mutating());
    }

    #[test]
    fn mutating_spawn_bumps_the_counter_and_opdone_releases_it() {
        if !git_available() {
            eprintln!("git unavailable; skipping pending-ops test");
            return;
        }
        let root = scratch_repo("spawn");
        std::fs::write(root.join("b.txt"), "new\n").unwrap();

        let (mut ed, _handle, mut rx) = crate::tests::helpers::editor_with_background();
        ed.vc.active_backend_name = Some("git".into());
        ed.vc.repo_root = Some(root.clone());
        assert_eq!(ed.vc.pending_ops, 0);

        execute_command(&mut ed, "vc-stage-all", &HashMap::new()).unwrap();
        assert_eq!(
            ed.vc.pending_ops, 1,
            "a dispatched mutating op is in flight"
        );

        // The op's completion event releases the busy state.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while ed.vc.pending_ops > 0 {
            match rx.try_recv() {
                Ok(ev) => process_background_event(&mut ed, ev),
                Err(_) if std::time::Instant::now() < deadline => {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                Err(e) => panic!("op never completed: {e}"),
            }
        }
        assert_eq!(ed.vc.pending_ops, 0);

        // The untracked file is now staged.
        let git_status = run_cmd(&root, "git", &["status", "--porcelain"]).unwrap_or_default();
        assert!(
            git_status.contains("A  b.txt"),
            "stage-all staged the new file: {git_status}"
        );
    }

    #[test]
    fn read_only_ops_do_not_block_the_commit_button() {
        if !git_available() {
            eprintln!("git unavailable; skipping read-only ops test");
            return;
        }
        let root = scratch_repo("readonly");
        let (mut ed, _handle, mut rx) = crate::tests::helpers::editor_with_background();
        ed.vc.active_backend_name = Some("git".into());
        ed.vc.repo_root = Some(root);

        execute_command(&mut ed, "vc-branch-list", &HashMap::new()).unwrap();
        assert_eq!(
            ed.vc.pending_ops, 0,
            "list refreshes are read-only and never set the busy state"
        );
        while let Ok(ev) = rx.try_recv() {
            process_background_event(&mut ed, ev);
        }
    }

    #[test]
    fn stage_all_then_unstage_all_round_trips() {
        if !git_available() {
            eprintln!("git unavailable; skipping stage round-trip test");
            return;
        }
        let root = scratch_repo("roundtrip");
        std::fs::write(root.join("c.txt"), "tracked-later\n").unwrap();

        run_op(&root, "git", &VcRemoteOp::StageAll).unwrap();
        let staged = run_cmd(&root, "git", &["status", "--porcelain"]).unwrap_or_default();
        assert!(
            staged.contains("A  c.txt"),
            "expected c.txt staged: {staged}"
        );

        run_op(&root, "git", &VcRemoteOp::UnstageAll).unwrap();
        let unstaged = run_cmd(&root, "git", &["status", "--porcelain"]).unwrap_or_default();
        assert!(
            unstaged.contains("?? c.txt"),
            "expected c.txt untracked again: {unstaged}"
        );
    }
}
