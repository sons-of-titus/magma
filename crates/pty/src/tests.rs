use super::*;

use portable_pty::{Child, CommandBuilder, PtySize, native_pty_system};

/// Spawn a real shell under a PTY so the session has genuine handles.  The
/// child is returned as well: tests reap or kill it themselves.
fn spawn_session(
    id: usize,
    label: &str,
    script: &str,
) -> (TerminalSession, Box<dyn Child + Send + Sync>) {
    let pair = native_pty_system()
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("openpty");
    let mut cmd = CommandBuilder::new("sh");
    cmd.arg("-c");
    cmd.arg(script);
    let child = pair.slave.spawn_command(cmd).expect("spawn");
    let killer = child.clone_killer();
    let pid = child.process_id().map(|p| p as i32);
    let writer = pair.master.take_writer().expect("writer");
    let session = TerminalSession::new(
        id,
        label.to_string(),
        Some(label.to_string()),
        std::env::temp_dir(),
        TerminalScreen::default(),
        Arc::new(Mutex::new(writer)),
        Mutex::new(pair.master),
        pid,
        Mutex::new(killer),
    );
    (session, child)
}

#[test]
fn renumbering_is_gapless_per_label_and_skips_renamed_tabs() {
    let (a, _ca) = spawn_session(0, "zsh", "read _");
    let (b, _cb) = spawn_session(1, "zsh", "read _");
    let (c, _cc) = spawn_session(2, "sh -c read _", "read _");
    let mut sessions = vec![a, b, c];
    assert!(sessions[1].rename("build"));

    TerminalSession::renumber_all(&mut sessions);

    assert_eq!(sessions[0].title, "zsh 1");
    assert_eq!(sessions[1].title, "build", "a renamed tab is user-owned");
    assert_eq!(sessions[2].title, "sh -c read _ 1");

    // Closing a session makes the remaining ones renumber from 1 again.
    sessions[0].kill();
    sessions.remove(0);
    TerminalSession::renumber_all(&mut sessions);
    assert_eq!(sessions[0].title, "build");
    assert_eq!(sessions[1].title, "sh -c read _ 1");

    for s in &mut sessions {
        s.kill();
    }
}

#[test]
fn renaming_reports_whether_the_name_changed() {
    let (mut s, _c) = spawn_session(0, "zsh", "read _");
    assert_eq!(s.title, "zsh");

    assert!(s.rename("build"), "a real rename reports a change");
    assert_eq!(s.title, "build");
    assert!(!s.rename("build"), "an unchanged name is not a rename");
    assert_eq!(s.title, "build");

    s.kill();
}

#[test]
fn input_and_resize_reach_the_pty() {
    let (s, mut child) = spawn_session(0, "cat", "read _");
    assert!(
        s.send_input("echo hi\n").is_ok(),
        "the writer accepts input"
    );
    assert!(s.resize(120, 40).is_ok(), "the master resizes");
    assert!(
        child.try_wait().expect("try_wait").is_none(),
        "still running"
    );
    drop(s);
    let _ = child.kill();
}

#[test]
fn killing_a_session_ends_the_child() {
    let (mut s, mut child) = spawn_session(0, "sleep", "sleep 30");
    s.kill();

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let mut exited = false;
    while std::time::Instant::now() < deadline {
        if child.try_wait().expect("try_wait").is_some() {
            exited = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(exited, "kill terminates the child");
}

#[test]
fn a_session_without_a_child_pid_still_kills_through_its_handle() {
    let (mut s, mut child) = spawn_session(0, "sleep", "sleep 30");
    s.pid = None;
    s.kill();

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let mut exited = false;
    while std::time::Instant::now() < deadline {
        if child.try_wait().expect("try_wait").is_some() {
            exited = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(exited, "the ChildKiller handle covers a missing pid");
}
