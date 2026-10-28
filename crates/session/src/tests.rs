use super::*;

fn temp_dir(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("session-{tag}-{}-{nanos}", std::process::id()))
}

#[test]
fn the_session_file_lives_under_dot_magma() {
    let path = session_file(Path::new("/project"));
    assert_eq!(path, Path::new("/project/.magma/session.json"));
}

#[test]
fn write_then_read_round_trips_every_field() {
    let root = temp_dir("roundtrip");
    let state = SessionState {
        buffers: vec![SessionBuffer {
            path: "/a.rs".into(),
            cursor: 12,
        }],
        focused: Some("/a.rs".into()),
        breakpoints: vec![SessionBreakpoint {
            file: "/a.rs".into(),
            line: 3,
            enabled: false,
        }],
        watches: vec!["counter".into()],
    };

    write(&root, &state).unwrap();
    assert_eq!(read(&root), Some(state));

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn write_creates_the_magma_directory() {
    let root = temp_dir("mkdir");
    write(&root, &SessionState::default()).unwrap();
    assert!(root.join(".magma").is_dir());
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn the_file_is_pretty_json_with_a_trailing_newline() {
    let root = temp_dir("format");
    write(&root, &SessionState::default()).unwrap();

    let text = std::fs::read_to_string(session_file(&root)).unwrap();
    assert!(text.ends_with('\n'));
    assert!(
        text.contains("{\n"),
        "expected pretty-printed JSON, got {text:?}"
    );
    assert!(serde_json::from_str::<SessionState>(&text).is_ok());

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn a_missing_session_reads_as_none() {
    assert!(read(&temp_dir("missing")).is_none());
}

#[test]
fn a_corrupt_session_reads_as_none() {
    let root = temp_dir("corrupt");
    std::fs::create_dir_all(root.join(".magma")).unwrap();
    std::fs::write(session_file(&root), "{not json").unwrap();

    assert!(read(&root).is_none());

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn every_field_defaults_when_absent() {
    let state: SessionState = serde_json::from_str("{}").unwrap();
    assert!(state.buffers.is_empty());
    assert!(state.focused.is_none());
    assert!(state.breakpoints.is_empty());
    assert!(state.watches.is_empty());
}

#[test]
fn a_breakpoint_without_the_enabled_flag_is_enabled() {
    let state: SessionState =
        serde_json::from_str(r#"{"breakpoints":[{"file":"/a.rs","line":3}]}"#).unwrap();
    assert!(state.breakpoints[0].enabled);
}

#[test]
fn an_explicitly_disabled_breakpoint_stays_disabled() {
    let state: SessionState =
        serde_json::from_str(r#"{"breakpoints":[{"file":"/a.rs","line":3,"enabled":false}]}"#)
            .unwrap();
    assert!(!state.breakpoints[0].enabled);
}

#[test]
fn writing_twice_replaces_the_previous_session() {
    let root = temp_dir("replace");
    let first = SessionState {
        focused: Some("/first.rs".into()),
        ..Default::default()
    };
    let second = SessionState {
        focused: Some("/second.rs".into()),
        ..Default::default()
    };

    write(&root, &first).unwrap();
    write(&root, &second).unwrap();
    assert_eq!(read(&root).unwrap().focused.as_deref(), Some("/second.rs"));

    std::fs::remove_dir_all(&root).ok();
}
