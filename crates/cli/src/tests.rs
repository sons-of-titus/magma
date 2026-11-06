use super::*;

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

fn run(list: &[&str]) -> Options {
    match parse(args(list)).unwrap() {
        Command::Run(o) => o,
        other => panic!("expected Run, got {other:?}"),
    }
}

#[test]
fn no_arguments_means_welcome_defaults() {
    let o = run(&[]);
    assert!(o.paths.is_empty());
    assert!(!o.reopen_last);
    assert_eq!(o.log_level, None);
}

#[test]
fn help_and_version_short_circuit() {
    assert_eq!(parse(args(&["-h"])).unwrap(), Command::Help);
    assert_eq!(parse(args(&["x", "--version"])).unwrap(), Command::Version);
    assert!(version_line().starts_with("magma "));
}

#[test]
fn debug_and_log_level_set_the_level() {
    assert_eq!(run(&["-d"]).log_level, Some(logging::Level::Debug as u8));
    assert_eq!(
        run(&["--log-level", "trace"]).log_level,
        Some(logging::Level::Trace as u8)
    );
    assert_eq!(
        run(&["--log-level=WARN"]).log_level,
        Some(logging::Level::Warn as u8)
    );
}

#[test]
fn bad_input_is_rejected() {
    assert!(parse(args(&["--nope"])).is_err());
    assert!(parse(args(&["--log-level"])).is_err());
    assert!(parse(args(&["--log-level", "loud"])).is_err());
}

#[test]
fn double_dash_makes_dashed_names_paths() {
    let o = run(&["--reopen", "--", "-weird", "b"]);
    assert!(o.reopen_last);
    assert_eq!(o.paths, vec!["-weird", "b"]);
}

#[test]
fn resolve_directory_file_and_line() {
    let dir = std::env::temp_dir().join(format!("magma-cli-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    std::fs::write(dir.join("sub/a.txt"), "x").unwrap();
    let root = std::fs::canonicalize(&dir).unwrap();

    let o = Options { paths: vec!["sub".into()], ..Default::default() };
    let l = resolve(&o, &dir).unwrap();
    assert_eq!(l.project, Some(root.join("sub")));
    assert!(l.files.is_empty());

    let o = Options { paths: vec!["sub/a.txt:42:7".into()], ..Default::default() };
    let l = resolve(&o, &dir).unwrap();
    assert_eq!(l.project, Some(root.join("sub")));
    assert_eq!(l.files, vec![(root.join("sub/a.txt"), Some(42))]);

    let o = Options { paths: vec!["sub/a.txt".into(), ".".into()], ..Default::default() };
    let l = resolve(&o, &dir).unwrap();
    assert_eq!(l.project, Some(root.clone()));

    let o = Options { paths: vec!["missing".into()], ..Default::default() };
    assert!(resolve(&o, &dir).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn server_and_connect_take_optional_addresses() {
    assert_eq!(
        parse(args(&["--server"])).unwrap(),
        Command::Server { addr: DEFAULT_ADDR.into() }
    );
    assert_eq!(
        parse(args(&["--server", "0.0.0.0:9000"])).unwrap(),
        Command::Server { addr: "0.0.0.0:9000".into() }
    );
    assert_eq!(
        parse(args(&["--connect=host:1"])).unwrap(),
        Command::Connect { addr: "host:1".into() }
    );
}
