//! TerminalManager lifecycle tests — real PTY sessions driven through the
//! same background-event pump the GUI frame loop runs.

use std::time::Duration;

use crate::kernel::runtime::BackgroundEvent;
use crate::kernel::terminal::{set_focused, TerminalManager};
use crate::kernel::state::Editor;

// ── TerminalManager: lifecycle ─────────────────────────────────────────────

type Events = tokio::sync::mpsc::UnboundedReceiver<BackgroundEvent>;

fn manager_with_channel() -> (
    TerminalManager,
    tokio::sync::mpsc::UnboundedSender<BackgroundEvent>,
    Events,
) {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    (TerminalManager::default(), tx, rx)
}

fn shell_script(script: &str) -> Vec<String> {
    vec!["-c".to_string(), script.to_string()]
}

/// Drain background events into the manager until `until` holds or the
/// deadline passes — the same pump the GUI frame loop runs.
fn pump(
    mgr: &mut TerminalManager,
    rx: &mut Events,
    until: impl Fn(&TerminalManager) -> bool,
    secs: u64,
) {
    let deadline = std::time::Instant::now() + Duration::from_secs(secs);
    while std::time::Instant::now() < deadline && !until(mgr) {
        match rx.try_recv() {
            Ok(BackgroundEvent::TerminalOutput { terminal_id, data }) => {
                mgr.apply_output(terminal_id, &data);
            }
            Ok(BackgroundEvent::TerminalExited { terminal_id, .. }) => {
                mgr.close(terminal_id);
            }
            _ => std::thread::sleep(Duration::from_millis(20)),
        }
    }
}

#[test]
fn spawn_streams_output_into_session_screen() {
    let (mut mgr, tx, mut rx) = manager_with_channel();
    let id = mgr.spawn("sh", &shell_script("echo magma-ok-4242; sleep 60"), &std::env::temp_dir(), tx)
        .expect("spawn");
    assert!(mgr.get(id).is_some(), "session lives in the manager, not the buffer slab");

    pump(&mut mgr, &mut rx, |m| {
        m.get(id).is_some_and(|s| s.screen.text().contains("magma-ok-4242"))
    }, 10);
    let session = mgr.get(id).expect("session alive while its shell sleeps");
    assert!(session.screen.text().contains("magma-ok-4242"),
        "screen: {:?}", session.screen.text());
    mgr.close(id);
    let _ = rx.try_recv();
}

#[test]
fn spawn_unknown_id_send_input_errors() {
    let (mgr, _tx, _rx) = manager_with_channel();
    assert!(mgr.send_input(999, "x").is_err());
}

#[test]
fn exiting_shell_closes_the_session_entirely() {
    let (mut mgr, tx, mut rx) = manager_with_channel();
    let id = mgr.spawn("sh", &shell_script("exit 3"), &std::env::temp_dir(), tx)
        .expect("spawn");

    pump(&mut mgr, &mut rx, |m| m.get(id).is_none(), 10);
    assert!(mgr.get(id).is_none(), "`exit` removes the session — no exited tab lingers");
    assert!(mgr.sessions.is_empty());
    let _ = rx.try_recv();
}

#[test]
fn default_names_renumber_gaplessly_when_a_session_closes() {
    let (mut mgr, tx, mut rx) = manager_with_channel();
    let a = mgr.spawn("sh", &shell_script("read _"), &std::env::temp_dir(), tx.clone()).unwrap();
    let b = mgr.spawn("sh", &shell_script("read _"), &std::env::temp_dir(), tx.clone()).unwrap();
    let c = mgr.spawn("sh", &shell_script("read _"), &std::env::temp_dir(), tx).unwrap();
    let title = |m: &TerminalManager, id: usize| m.get(id).unwrap().title.clone();
    assert_eq!(title(&mgr, a), "sh -c read _ 1");
    assert_eq!(title(&mgr, b), "sh -c read _ 2");
    assert_eq!(title(&mgr, c), "sh -c read _ 3");

    mgr.close(a);
    assert_eq!(title(&mgr, b), "sh -c read _ 1", "the next shell slides into the freed number");
    assert_eq!(title(&mgr, c), "sh -c read _ 2");
    mgr.close(b);
    assert_eq!(title(&mgr, c), "sh -c read _ 1");
    mgr.close(c);
    let _ = rx.try_recv();
}

#[test]
fn renamed_tabs_keep_their_name_while_defaults_renumber() {
    let (mut mgr, tx, mut rx) = manager_with_channel();
    let a = mgr.spawn("sh", &shell_script("read _"), &std::env::temp_dir(), tx.clone()).unwrap();
    let b = mgr.spawn("sh", &shell_script("read _"), &std::env::temp_dir(), tx.clone()).unwrap();
    let c = mgr.spawn("sh", &shell_script("read _"), &std::env::temp_dir(), tx).unwrap();
    mgr.rename(b, "build");
    let title = |m: &TerminalManager, id: usize| m.get(id).unwrap().title.clone();

    mgr.close(a);
    assert_eq!(title(&mgr, b), "build", "a renamed tab is never renumbered");
    assert_eq!(title(&mgr, c), "sh -c read _ 1", "numbering skips renamed tabs");
    mgr.close(c);
    assert_eq!(title(&mgr, b), "build");
    mgr.close(b);
    let _ = rx.try_recv();
}

#[test]
fn committing_rename_without_changes_keeps_default_numbering() {
    let (mut mgr, tx, mut rx) = manager_with_channel();
    let a = mgr.spawn("sh", &shell_script("read _"), &std::env::temp_dir(), tx.clone()).unwrap();
    let b = mgr.spawn("sh", &shell_script("read _"), &std::env::temp_dir(), tx).unwrap();
    let title = |m: &TerminalManager, id: usize| m.get(id).unwrap().title.clone();

    // Double-click accident: commit the editor with the same text.
    let unchanged = title(&mgr, b);
    mgr.rename(b, &unchanged);

    mgr.close(a);
    assert_eq!(title(&mgr, b), "sh -c read _ 1",
        "the untouched tab still participates in renumbering — no duplicate numbers");
    mgr.close(b);
    let _ = rx.try_recv();
}

#[test]
fn closing_the_focused_session_hands_focus_to_the_newest_survivor() {
    let (mut mgr, tx, mut rx) = manager_with_channel();
    let first = mgr.spawn("sh", &shell_script("read _"), &std::env::temp_dir(), tx.clone()).unwrap();
    let second = mgr.spawn("sh", &shell_script("read _"), &std::env::temp_dir(), tx).unwrap();
    mgr.focused = Some(first);

    mgr.close(first);
    assert!(mgr.get(first).is_none());
    assert_eq!(mgr.focused, Some(second), "focus moves to the survivor");

    mgr.close(second);
    assert_eq!(mgr.focused, None, "the last exit releases the keyboard");
    let _ = rx.try_recv();
}

#[test]
fn send_input_reaches_interactive_shell() {
    let (mut mgr, tx, mut rx) = manager_with_channel();
    let id = mgr.spawn("sh", &[], &std::env::temp_dir(), tx).expect("spawn");

    mgr.send_input(id, "echo typed-ping-777\n").expect("send");
    pump(&mut mgr, &mut rx, |m| {
        m.get(id).is_some_and(|s| s.screen.text().contains("typed-ping-777"))
    }, 10);
    assert!(mgr.get(id).expect("alive").screen.text().contains("typed-ping-777"));
    mgr.close(id);
}

#[test]
fn spawned_sessions_get_distinct_default_titles() {
    let (mut mgr, tx, mut rx) = manager_with_channel();
    let a = mgr.spawn("sh", &shell_script("read _"), &std::env::temp_dir(), tx.clone()).unwrap();
    let b = mgr.spawn("sh", &shell_script("read _"), &std::env::temp_dir(), tx).unwrap();
    let ta = mgr.get(a).unwrap().title.clone();
    let tb = mgr.get(b).unwrap().title.clone();
    assert_ne!(ta, tb, "sibling tabs must be distinguishable: {ta} vs {tb}");
    assert!(ta.contains("sh") && tb.contains("sh"), "titles: {ta}, {tb}");
    mgr.close(a);
    mgr.close(b);
    let _ = rx.try_recv();
}

#[test]
fn rename_updates_session_title_at_any_time() {
    let (mut mgr, tx, mut rx) = manager_with_channel();
    let id = mgr.spawn("sh", &shell_script("read _"), &std::env::temp_dir(), tx).unwrap();

    mgr.rename(id, "build server");
    assert_eq!(mgr.get(id).map(|s| s.title.as_str()), Some("build server"));

    mgr.send_input(id, "echo still-alive\n").expect("session still interactive after rename");
    mgr.rename(id, "");
    assert_eq!(mgr.get(id).map(|s| s.title.as_str()), Some(""), "rename applies even to an empty name");
    mgr.close(id);
    let _ = rx.try_recv();
}

#[test]
fn send_input_bumps_the_input_sequence() {
    use std::sync::atomic::Ordering;

    let (mut mgr, tx, mut rx) = manager_with_channel();
    let id = mgr.spawn("sh", &shell_script("read _"), &std::env::temp_dir(), tx).expect("spawn");
    let before = mgr.input_seq.load(Ordering::Relaxed);

    mgr.send_input(id, "hi\n").expect("send");
    assert_eq!(mgr.input_seq.load(Ordering::Relaxed), before + 1, "delivered stdin bumps the seq");

    mgr.send_input(999, "x").expect_err("unknown id");
    assert_eq!(mgr.input_seq.load(Ordering::Relaxed), before + 1, "failed sends stay silent");
    mgr.close(id);
    let _ = rx.try_recv();
}

#[test]
fn close_kills_and_removes_session() {
    let (mut mgr, tx, mut rx) = manager_with_channel();
    let id = mgr.spawn("sh", &shell_script("read _"), &std::env::temp_dir(), tx)
        .expect("spawn");
    assert_eq!(mgr.sessions.len(), 1);

    mgr.close(id);
    assert!(mgr.sessions.is_empty(), "closed session is gone");
    assert_eq!(mgr.focused, None);
    let _ = rx.try_recv(); // drain exit events from the killed child
}

#[test]
fn active_id_prefers_focused_then_newest() {
    let (mut mgr, tx, mut rx) = manager_with_channel();
    let first = mgr.spawn("sh", &shell_script("read _"), &std::env::temp_dir(), tx.clone()).unwrap();
    let second = mgr.spawn("sh", &shell_script("read _"), &std::env::temp_dir(), tx).unwrap();
    assert_eq!(mgr.active_id(), Some(second), "newest session is active by default");
    mgr.focused = Some(first);
    assert_eq!(mgr.active_id(), Some(first), "focus wins");
    mgr.close(first);
    mgr.close(second);
    let _ = rx.try_recv();
}

#[test]
fn resize_is_accepted_by_the_pty() {
    let (mut mgr, tx, mut rx) = manager_with_channel();
    let id = mgr.spawn("sh", &shell_script("read _"), &std::env::temp_dir(), tx)
        .expect("spawn");
    assert!(mgr.resize(id, 120, 40).is_ok(), "resize reaches the PTY master");
    mgr.close(id);
    let _ = rx.try_recv();
}

// ── Editor-level surface ─────────────────────────────────────────────────────

#[test]
fn editor_send_input_routes_through_manager() {
    let mut ed = Editor::new(Box::new(crate::kernel::storage::disk::DiskFileSystem::new()));
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let id = ed.io.terminals.spawn("sh", &shell_script("read _"), &std::env::temp_dir(), tx).unwrap();

    assert!(crate::kernel::terminal::send_input(&ed, id, "echo hi").is_ok());
    let err = crate::kernel::terminal::send_input(&ed, 999, "echo hi").unwrap_err();
    assert!(err.contains("Not a terminal"), "got: {err}");
    ed.io.terminals.close(id);
}

#[test]
fn set_focused_engages_and_releases_terminal_mode() {
    let mut ed = Editor::new(Box::new(crate::kernel::storage::disk::DiskFileSystem::new()));
    ed.keymaps.push_layer("editor");

    set_focused(&mut ed, Some(1));
    assert_eq!(ed.editor_mode.name, "terminal");
    assert_eq!(ed.io.terminals.focused, Some(1));
    assert!(!ed.keymaps.is_layer_active("editor"), "editor bindings off while typing into a pty");

    set_focused(&mut ed, None);
    assert_eq!(ed.editor_mode.name, "normal");
    assert_eq!(ed.io.terminals.focused, None);
    assert!(ed.keymaps.is_layer_active("editor"), "editor bindings restored");
}

#[test]
fn shell_exit_closes_session_and_restores_editor_mode() {
    let mut ed = Editor::new(Box::new(crate::kernel::storage::disk::DiskFileSystem::new()));
    ed.keymaps.push_layer("editor");
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let id = ed.io.terminals.spawn("sh", &shell_script("read _"), &std::env::temp_dir(), tx.clone()).unwrap();
    set_focused(&mut ed, Some(id));

    crate::kernel::runtime::process_background_event(
        &mut ed,
        BackgroundEvent::TerminalExited { terminal_id: id, exit_code: 0 },
    );

    assert!(ed.io.terminals.get(id).is_none(), "session closed by exit");
    assert_eq!(ed.io.terminals.focused, None);
    assert_eq!(ed.editor_mode.name, "normal", "the keyboard returns to the editor");
    assert!(ed.keymaps.is_layer_active("editor"), "editor bindings restored");
    let _ = rx.try_recv();
}

#[test]
fn shell_exit_with_survivors_keeps_terminal_focus_and_mode() {
    let mut ed = Editor::new(Box::new(crate::kernel::storage::disk::DiskFileSystem::new()));
    ed.keymaps.push_layer("editor");
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let a = ed.io.terminals.spawn("sh", &shell_script("read _"), &std::env::temp_dir(), tx.clone()).unwrap();
    let b = ed.io.terminals.spawn("sh", &shell_script("read _"), &std::env::temp_dir(), tx).unwrap();
    set_focused(&mut ed, Some(a));

    crate::kernel::runtime::process_background_event(
        &mut ed,
        BackgroundEvent::TerminalExited { terminal_id: a, exit_code: 0 },
    );

    assert!(ed.io.terminals.get(a).is_none());
    assert_eq!(ed.io.terminals.focused, Some(b), "focus lands on the surviving shell");
    assert_eq!(ed.editor_mode.name, "terminal", "no click needed to keep typing");
    assert!(!ed.keymaps.is_layer_active("editor"), "terminal bindings still hold");
    ed.io.terminals.close(b);
    let _ = rx.try_recv();
}

#[test]
fn close_all_kills_every_session_and_releases_focus() {
    let (mut mgr, tx, mut rx) = manager_with_channel();
    let first = mgr.spawn("sh", &shell_script("read _"), &std::env::temp_dir(), tx.clone()).unwrap();
    let _second = mgr.spawn("sh", &shell_script("read _"), &std::env::temp_dir(), tx).unwrap();
    mgr.focused = Some(first);

    mgr.close_all();
    assert!(mgr.sessions.is_empty(), "no session survives editor close");
    assert_eq!(mgr.focused, None);
    let _ = rx.try_recv();
}
