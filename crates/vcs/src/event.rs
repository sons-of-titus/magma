//! The version-control subsystem's background-event vocabulary.
//!
//! Repo-operation jobs and status/branch/stash/diff workers produce these off
//! the foreground thread; the editor wraps them in its own transport enum so
//! the foreground keeps one ordered channel.

use std::path::PathBuf;

use crate::{DiffData, StashEntry, VcStatus};

/// An event produced by a version-control operation or fetch.
#[derive(Debug, Clone)]
pub enum Event {
    /// A status fetch finished off the foreground thread.
    StatusFetched {
        backend_idx: usize,
        root: PathBuf,
        result: Result<VcStatus, String>,
    },
    /// A repository operation (stage/diff/log/push/blame/commit) finished.
    OpDone {
        scratch: Option<String>,
        then_refresh: bool,
        result: Result<String, String>,
    },
    /// A branch list for the Commit window's branch widget finished.
    BranchesReady { branches: Vec<String> },
    /// A stash list for the Commit window's stash section finished.
    StashListed {
        result: Result<Vec<StashEntry>, String>,
    },
    /// A one-file diff bundle (HEAD + worktree + patch) finished.
    DiffReady {
        path: String,
        result: Result<DiffData, String>,
    },
    /// Per-line blame for one buffer finished.  The foreground applies it
    /// only when the buffer's blame generation still matches, so a stale
    /// run can never repaint a view the user already toggled.
    BlameReady {
        buf_id: usize,
        generation: u64,
        result: Result<String, String>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_fetch_carries_backend_root_and_result() {
        let event = Event::StatusFetched {
            backend_idx: 2,
            root: "/repo".into(),
            result: Ok(VcStatus {
                backend_name: "git".into(),
                branch: "main".into(),
                has_staging: true,
                staged: vec![],
                unstaged: vec![],
                untracked: vec![],
                conflicts: vec![],
            }),
        };
        match event {
            Event::StatusFetched {
                backend_idx,
                root,
                result,
            } => {
                assert_eq!(backend_idx, 2);
                assert_eq!(root, PathBuf::from("/repo"));
                assert_eq!(result.unwrap().branch, "main");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn op_stash_diff_and_branch_events_keep_their_payloads() {
        let op = Event::OpDone {
            scratch: Some("commit".into()),
            then_refresh: true,
            result: Err("e".into()),
        };
        match op {
            Event::OpDone {
                scratch,
                then_refresh,
                result,
            } => {
                assert_eq!(scratch.as_deref(), Some("commit"));
                assert!(then_refresh);
                assert_eq!(result, Err("e".to_string()));
            }
            _ => panic!("wrong variant"),
        }

        let stash = Event::StashListed {
            result: Ok(vec![StashEntry {
                index: 0,
                message: "wip".into(),
            }]),
        };
        match stash {
            Event::StashListed {
                result: Ok(stashes),
            } => {
                assert_eq!(stashes[0].message, "wip");
            }
            _ => panic!("wrong variant"),
        }

        let diff = Event::DiffReady {
            path: "src/lib.rs".into(),
            result: Ok(DiffData::default()),
        };
        match diff {
            Event::DiffReady { path, result } => {
                assert_eq!(path, "src/lib.rs");
                assert!(result.unwrap().patch.is_empty());
            }
            _ => panic!("wrong variant"),
        }

        let branches_event = Event::BranchesReady {
            branches: vec!["main".into(), "dev".into()],
        };
        match branches_event {
            Event::BranchesReady { branches } => {
                assert_eq!(branches, vec!["main".to_string(), "dev".to_string()]);
            }
            _ => panic!("wrong variant"),
        }
    }
}
