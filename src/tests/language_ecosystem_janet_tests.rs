use crate::kernel::scripting;

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
        scripting::eval(r#"(language/register "rust" "/path/to/provider.janet")"#);
        let result = scripting::eval_result(
            r#"(get (first (language/list)) :name)"#
        ).unwrap_or_default();
        assert!(result.contains("rust"), "name not found in result: {result}");
    });
}

#[test]
fn language_register_stores_provider_path() {
    janet_test!(ed, {
        scripting::eval(r#"(language/register "rust" "/a/provider.janet" "/a/syntax.scm" "/a/config.janet")"#);
        let result = scripting::eval_result(
            r#"(get (first (language/list)) :provider-path)"#
        ).unwrap_or_default();
        assert!(result.contains("/a/provider.janet"), "provider-path not found: {result}");
    });
}

#[test]
fn language_register_loaded_initially_false() {
    janet_test!(ed, {
        scripting::eval(r#"(language/register "janet")"#);
        let result = scripting::eval_result(
            r#"(get (first (language/list)) :loaded)"#
        ).unwrap_or_default();
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
        let result = scripting::eval_result(r#"(language/unregister "unknown")"#).unwrap_or_default();
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

// ── language/load ─────────────────────────────────────────────────────────────

#[test]
fn language_load_returns_error_for_unregistered() {
    janet_test!(ed, {
        let result = scripting::eval_result(r#"(language/load "nosuchlang")"#).unwrap_or_default();
        assert_eq!(result, ":error");
    });
}

#[test]
fn language_load_returns_error_for_missing_path() {
    janet_test!(ed, {
        scripting::eval(r#"(language/register "go")"#);
        let result = scripting::eval_result(r#"(language/load "go")"#).unwrap_or_default();
        assert_eq!(result, ":error");
    });
}

#[test]
fn language_load_marks_language_loaded_on_success() {
    use std::io::Write as _;
    janet_test!(ed, {
        let tmp = std::env::temp_dir().join(format!("magma_jl_load_{}", std::process::id()));
        std::fs::create_dir_all(&tmp).ok();
        let provider = tmp.join("provider.janet");
        let mut f = std::fs::File::create(&provider).unwrap();
        let _ = f.write_all(b"# empty provider");

        let provider_str = provider.to_str().unwrap();
        let reg_code = format!(r#"(language/register "testlang" "{provider_str}")"#);
        scripting::eval(&reg_code);

        let result = scripting::eval_result(r#"(language/load "testlang")"#).unwrap_or_default();
        std::fs::remove_dir_all(&tmp).ok();

        assert_eq!(result, ":ok", "load returned: {result}");

        let loaded = scripting::eval_result(
            r#"(get (find (fn [l] (= (get l :name) "testlang")) (language/list)) :loaded)"#
        ).unwrap_or_default();
        assert_eq!(loaded, "true");
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
        let name0 = scripting::eval_result(
            r#"(get (get (language/list) 0) :name)"#
        ).unwrap_or_default();
        let name1 = scripting::eval_result(
            r#"(get (get (language/list) 1) :name)"#
        ).unwrap_or_default();
        let name2 = scripting::eval_result(
            r#"(get (get (language/list) 2) :name)"#
        ).unwrap_or_default();

        assert!(name0.contains("janet"),  "index 0 should be janet, got: {name0}");
        assert!(name1.contains("python"), "index 1 should be python, got: {name1}");
        assert!(name2.contains("rust"),   "index 2 should be rust, got: {name2}");
    });
}

// ── discover helper ───────────────────────────────────────────────────────────

#[test]
fn language_discover_registers_subdirs() {
    use std::io::Write as _;
    janet_test!(ed, {
        let tmp = std::env::temp_dir().join(format!("magma_jl_disc_{}", std::process::id()));
        let lang_dir = tmp.join("mylang");
        std::fs::create_dir_all(&lang_dir).unwrap();
        let mut f = std::fs::File::create(lang_dir.join("provider.janet")).unwrap();
        let _ = f.write_all(b"# provider");

        let root_str = tmp.to_str().unwrap().replace('\\', "/");
        let code = format!(r#"(language/discover "{root_str}")"#);
        scripting::eval(&code);
        std::fs::remove_dir_all(&tmp).ok();

        let result = scripting::eval_result("(length (language/list))").unwrap_or_default();
        let count: i32 = result.parse().unwrap_or(0);
        assert!(count >= 1, "expected at least 1 language, got {result}");
    });
}
