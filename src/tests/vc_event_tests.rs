//! Regression tests for the version-control subsystem's namespaced
//! background events.
//!
//! `vc::Event` owns status/op/branch/stash/diff results;
//! `BackgroundEvent::Vc(_)` carries them on the one foreground channel.
//! These tests pin the wrapper payloads, cross-subsystem ordering, and
//! dispatch effects.

use crate::kernel::runtime::{BackgroundEvent, process_background_event};
use crate::kernel::vc::{Event as VcEvent, StashEntry, VcStatus};

fn status(branch: &str) -> VcStatus {
    VcStatus {
        backend_name: "git".into(),
        branch: branch.into(),
        has_staging: true,
        staged: vec![],
        unstaged: vec![],
        untracked: vec![],
        conflicts: vec![],
    }
}

#[test]
fn vc_events_round_trip_through_the_shared_channel_in_send_order() {
    let (_ed, bg, mut rx) = crate::tests::helpers::editor_with_background();

    bg.send_event(BackgroundEvent::Vc(VcEvent::StatusFetched {
        backend_idx: 1,
        root: "/repo".into(),
        result: Ok(status("main")),
    }));
    bg.send_event(BackgroundEvent::FileReadResult {
        path: "/tmp/a".into(),
        content: Ok("x".into()),
        request_id: 6,
    });
    bg.send_event(BackgroundEvent::Vc(VcEvent::OpDone {
        scratch: Some("commit".into()),
        then_refresh: true,
        result: Err("boom".into()),
    }));
    bg.send_event(BackgroundEvent::Vc(VcEvent::BranchesReady {
        branches: vec!["main".into(), "dev".into()],
    }));
    bg.send_event(BackgroundEvent::Custom("ping".into(), "pong".into()));

    let mut seen = Vec::new();
    while let Ok(ev) = rx.try_recv() {
        seen.push(ev);
    }

    assert_eq!(seen.len(), 5, "no event may be dropped or duplicated");
    assert!(matches!(
        &seen[0],
        BackgroundEvent::Vc(VcEvent::StatusFetched { backend_idx: 1, root, result: Ok(s) })
            if root == &std::path::PathBuf::from("/repo") && s.branch.as_str() == "main"
    ));
    assert!(matches!(
        &seen[1],
        BackgroundEvent::FileReadResult { request_id: 6, content: Ok(body), .. }
            if body.as_str() == "x"
    ));
    assert!(matches!(
        &seen[2],
        BackgroundEvent::Vc(VcEvent::OpDone { scratch: Some(s), then_refresh: true, result: Err(e) })
            if s.as_str() == "commit" && e.as_str() == "boom"
    ));
    assert!(matches!(
        &seen[3],
        BackgroundEvent::Vc(VcEvent::BranchesReady { branches })
            if branches == &vec!["main".to_string(), "dev".to_string()]
    ));
    assert!(matches!(
        &seen[4],
        BackgroundEvent::Custom(name, payload)
            if name.as_str() == "ping" && payload.as_str() == "pong"
    ));
}

#[test]
fn status_dispatch_stores_the_latest_status() {
    let (mut ed, _bg, _rx) = crate::tests::helpers::editor_with_background();

    process_background_event(
        &mut ed,
        BackgroundEvent::Vc(VcEvent::StatusFetched {
            backend_idx: 1,
            root: "/repo".into(),
            result: Ok(status("feature")),
        }),
    );

    assert_eq!(
        ed.vc.last_status.as_ref().map(|s| s.branch.as_str()),
        Some("feature")
    );
}

#[test]
fn branches_and_stashes_dispatch_replace_the_window_state() {
    let (mut ed, _bg, _rx) = crate::tests::helpers::editor_with_background();

    process_background_event(
        &mut ed,
        BackgroundEvent::Vc(VcEvent::BranchesReady {
            branches: vec!["main".into()],
        }),
    );
    process_background_event(
        &mut ed,
        BackgroundEvent::Vc(VcEvent::StashListed {
            result: Ok(vec![StashEntry {
                index: 0,
                message: "wip".into(),
            }]),
        }),
    );

    assert_eq!(ed.vc.branches, vec!["main".to_string()]);
    assert_eq!(
        ed.vc.stashes,
        vec![StashEntry {
            index: 0,
            message: "wip".into()
        }]
    );
}
