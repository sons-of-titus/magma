use super::*;

fn cwd() -> &'static Path {
    Path::new("/repo")
}

#[test]
fn cargo_error_span_becomes_error_diagnostic() {
    let text = "\
error[E0308]: mismatched types
 --> src/main.rs:12:5
  |
12 |     let x: u8 = \"text\";
  |                 ^^^^^^ expected `u8`, found `&str`";
    let d = parse_tool_output("cargo build", text, cwd());
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].path, "/repo/src/main.rs");
    assert_eq!(d[0].diag.severity, DiagnosticSeverity::Error);
    assert_eq!((d[0].diag.line, d[0].diag.column), (11, 4));
    assert_eq!(d[0].diag.message, "mismatched types");
}

#[test]
fn cargo_warning_is_warning_severity() {
    let text = "warning: unused variable: `x`\n --> src/lib.rs:3:9\n";
    let d = parse_tool_output("cargo build", text, cwd());
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].diag.severity, DiagnosticSeverity::Warning);
    assert_eq!(d[0].path, "/repo/src/lib.rs");
}

#[test]
fn cargo_test_panic_links_the_assertion_line() {
    let text = "---- it_works stdout ----\n\
thread 'it_works' panicked at src/lib.rs:8:5:\nassertion failed";
    let d = parse_tool_output("cargo test", text, cwd());
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].path, "/repo/src/lib.rs");
    assert_eq!(d[0].diag.line, 7);
}

#[test]
fn generic_and_pytest_lines_parse() {
    let text = "\
FAILED tests/test_app.py::test_index - AssertionError: bad state
tests/test_app.py:12: AssertionError: bad state
make[1]: *** [Makefile:9: all] Error 1";
    let d = parse_tool_output("pytest", text, cwd());
    assert!(d.iter().any(|t| t.path.ends_with("tests/test_app.py")
        && t.diag.line == 11), "traceback line maps: {d:?}");
    // `make[1]: *** ...` has no leading path shape and must not parse.
    assert!(d.iter().all(|t| !t.path.contains("Makefile")), "{d:?}");
}

#[test]
fn prose_with_colons_does_not_parse() {
    let d = parse_tool_output("x", "note: this: is just prose\nplain line", cwd());
    assert!(d.is_empty());
}

// ── Quickfix conversion ──────────────────────────────────────────────────────

#[test]
fn quickfix_entries_are_one_based() {
    let d = parse_tool_output("cargo build",
        "error: boom\n --> src/lib.rs:3:9\n", cwd());
    let qf = d[0].quickfix();
    assert_eq!(qf.filename, "/repo/src/lib.rs");
    assert_eq!((qf.line, qf.col), (3, 9));
    assert_eq!(qf.message, "boom");
    assert_eq!(qf.severity, DiagnosticSeverity::Error);
}

#[test]
fn a_default_quickfix_entry_is_an_empty_error() {
    let qf = QuickfixEntry::default();
    assert!(qf.filename.is_empty());
    assert!(qf.message.is_empty());
    assert_eq!((qf.line, qf.col), (0, 0));
    assert_eq!(qf.severity, DiagnosticSeverity::Error);
}
