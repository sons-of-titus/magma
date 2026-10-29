#[macro_use]
mod helpers;
use scripting;

// ── language/list ─────────────────────────────────────────────────────────────

#[test]
fn language_list_returns_array_type() {
    janet_test!(ed, {
        let result = scripting::eval_result("(type (language/list))").unwrap_or_default();
        assert_eq!(result, ":array");
    });
}

#[test]
fn language_list_initially_empty() {
    janet_test!(ed, {
        let result = scripting::eval_result("(length (language/list))").unwrap_or_default();
        assert_eq!(result, "0");
    });
}

// ── language/register ─────────────────────────────────────────────────────────

#[test]
fn language_register_returns_ok() {
    janet_test!(ed, {
        let result = scripting::eval_result(r#"(language/register "rust")"#).unwrap_or_default();
        assert_eq!(result, ":ok");
    });
}

#[test]
fn language_register_increments_list_length() {
    janet_test!(ed, {
        scripting::eval(r#"(language/register "rust")"#);
        scripting::eval(r#"(language/register "python")"#);
        let result = scripting::eval_result("(length (language/list))").unwrap_or_default();
        assert_eq!(result, "2");
    });
}

#[test]
fn language_register_stores_name() {
    janet_test!(ed, {
        scripting::eval(r#"(language/register "rust")"#);
        let result =
            scripting::eval_result(r#"(get (first (language/list)) :name)"#).unwrap_or_default();
        assert!(
            result.contains("rust"),
            "name not found in result: {result}"
        );
    });
}

#[test]
fn language_register_stores_asset_paths() {
    janet_test!(ed, {
        scripting::eval(r#"(language/register "rust" "/a/syntax.scm" "/a/config.janet")"#);
        let syntax = scripting::eval_result(r#"(get (first (language/list)) :syntax-path)"#)
            .unwrap_or_default();
        let config = scripting::eval_result(r#"(get (first (language/list)) :config-path)"#)
            .unwrap_or_default();
        assert!(
            syntax.contains("/a/syntax.scm"),
            "syntax-path not found: {syntax}"
        );
        assert!(
            config.contains("/a/config.janet"),
            "config-path not found: {config}"
        );
    });
}

#[test]
fn language_register_loaded_initially_false() {
    janet_test!(ed, {
        scripting::eval(r#"(language/register "janet")"#);
        let result =
            scripting::eval_result(r#"(get (first (language/list)) :loaded)"#).unwrap_or_default();
        assert_eq!(result, "false");
    });
}

// ── language/unregister ───────────────────────────────────────────────────────

#[test]
fn language_unregister_removes_entry() {
    janet_test!(ed, {
        scripting::eval(r#"(language/register "rust")"#);
        scripting::eval(r#"(language/unregister "rust")"#);
        let result = scripting::eval_result("(length (language/list))").unwrap_or_default();
        assert_eq!(result, "0");
    });
}

#[test]
fn language_unregister_missing_returns_nil() {
    janet_test!(ed, {
        let result =
            scripting::eval_result(r#"(language/unregister "unknown")"#).unwrap_or_default();
        assert_eq!(result, "nil");
    });
}

// ── language/for-buffer ───────────────────────────────────────────────────────

#[test]
fn language_for_buffer_returns_nil_when_unset() {
    janet_test!(ed, {
        let result = scripting::eval_result("(language/for-buffer 0)").unwrap_or_default();
        assert_eq!(result, "nil");
    });
}

#[test]
fn language_for_buffer_returns_language_after_ts_set() {
    janet_test!(ed, {
        let buf_id = ed.create_buffer("test.rs");
        ed.ts_languages.insert(buf_id, "rust".to_string());

        let code = format!("(language/for-buffer {buf_id})");
        let result = scripting::eval_result(&code).unwrap_or_default();
        assert!(result.contains("rust"), "expected 'rust', got: {result}");
    });
}

// ── language/list sorted output ───────────────────────────────────────────────

#[test]
fn language_list_sorted_alphabetically() {
    janet_test!(ed, {
        scripting::eval(r#"(language/register "rust")"#);
        scripting::eval(r#"(language/register "janet")"#);
        scripting::eval(r#"(language/register "python")"#);

        // Verify sorted order by checking each position directly.
        let name0 =
            scripting::eval_result(r#"(get (get (language/list) 0) :name)"#).unwrap_or_default();
        let name1 =
            scripting::eval_result(r#"(get (get (language/list) 1) :name)"#).unwrap_or_default();
        let name2 =
            scripting::eval_result(r#"(get (get (language/list) 2) :name)"#).unwrap_or_default();

        assert!(
            name0.contains("janet"),
            "index 0 should be janet, got: {name0}"
        );
        assert!(
            name1.contains("python"),
            "index 1 should be python, got: {name1}"
        );
        assert!(
            name2.contains("rust"),
            "index 2 should be rust, got: {name2}"
        );
    });
}
