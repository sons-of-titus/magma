use crate::Level;

#[test]
fn levels_are_ordered_low_to_high() {
    assert!(Level::Error < Level::Warn);
    assert!(Level::Warn < Level::Info);
    assert!(Level::Info < Level::Debug);
    assert!(Level::Debug < Level::Trace);
}

#[test]
fn level_names_map_to_levels() {
    assert_eq!(Level::from_name("error"), Some(Level::Error as u8));
    assert_eq!(Level::from_name("warn"), Some(Level::Warn as u8));
    assert_eq!(Level::from_name("warning"), Some(Level::Warn as u8));
    assert_eq!(Level::from_name("info"), Some(Level::Info as u8));
    assert_eq!(Level::from_name("debug"), Some(Level::Debug as u8));
    assert_eq!(Level::from_name("trace"), Some(Level::Trace as u8));
    assert_eq!(Level::from_name("verbose"), None);
}

#[test]
fn default_level_keeps_info_visible() {
    assert_eq!(crate::DEFAULT_LEVEL, Level::Info as u8);
}

#[test]
fn enabled_gates_on_the_configured_max_level() {
    let saved = crate::max_level();
    crate::set_max_level(Level::Warn as u8);
    assert!(crate::enabled(Level::Error));
    assert!(crate::enabled(Level::Warn));
    assert!(!crate::enabled(Level::Info));
    assert!(!crate::enabled(Level::Debug));
    assert!(!crate::enabled(Level::Trace));
    crate::set_max_level(saved);
}

#[test]
fn labels_match_their_levels() {
    assert_eq!(Level::Error.label(), "ERROR");
    assert_eq!(Level::Warn.label(), "WARN");
    assert_eq!(Level::Info.label(), "INFO");
    assert_eq!(Level::Debug.label(), "DEBUG");
    assert_eq!(Level::Trace.label(), "TRACE");
}

#[test]
fn file_sink_records_emitted_errors() {
    let path = std::env::temp_dir().join(format!("magma-log-test-{}.log", std::process::id()));
    let _ = std::fs::remove_file(&path);
    crate::init_file(&path).unwrap();
    crate::error!("sink probe {}", 42);
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("[ERROR]") && text.contains("sink probe 42"));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn log_err_passes_ok_and_swallows_err() {
    use crate::LogErr;
    assert_eq!(Ok::<_, String>(3).log_err(), Some(3));
    assert_eq!(Err::<i32, _>("boom").log_err(), None);
    assert_eq!(Err::<i32, _>("boom").log_warn(), None);
}
