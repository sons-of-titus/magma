use crate::janet_bridge;

// ── magma-api-version ─────────────────────────────────────────────────────

#[test]
fn magma_api_version_is_tuple_of_three_numbers() {
    janet_test!(ed, {
        let r = janet_bridge::eval(r#"(and (tuple? magma-api-version) (= 3 (length magma-api-version)))"#);
        assert_eq!(r, "ok");
    });
}

#[test]
fn magma_api_version_major_is_zero() {
    janet_test!(ed, {
        let r = janet_bridge::eval(r#"(= 0 (magma-api-version 0))"#);
        assert_eq!(r, "ok");
    });
}

// ── magma/api-compat? ─────────────────────────────────────────────────────

#[test]
fn api_compat_returns_true_for_same_version() {
    janet_test!(ed, {
        let r = janet_bridge::eval(r#"(magma/api-compat? 0 24)"#);
        assert_eq!(r, "ok");
    });
}

#[test]
fn api_compat_returns_false_for_future_major() {
    janet_test!(ed, {
        let r = janet_bridge::eval(r#"(not (magma/api-compat? 1 1))"#);
        assert_eq!(r, "ok");
    });
}

#[test]
fn api_compat_returns_false_for_future_minor() {
    janet_test!(ed, {
        let r = janet_bridge::eval(r#"(not (magma/api-compat? 0 99))"#);
        assert_eq!(r, "ok");
    });
}

// ── magma/trim ────────────────────────────────────────────────────────────

#[test]
fn trim_removes_leading_spaces() {
    janet_test!(ed, {
        let r = janet_bridge::eval(r#"(= "hello" (magma/trim "  hello"))"#);
        assert_eq!(r, "ok");
    });
}

#[test]
fn trim_removes_leading_tabs() {
    janet_test!(ed, {
        let r = janet_bridge::eval(r#"(= "hello" (magma/trim "\t\thello"))"#);
        assert_eq!(r, "ok");
    });
}

#[test]
fn trim_does_not_affect_trailing_spaces() {
    janet_test!(ed, {
        let r = janet_bridge::eval(r#"(= "hello  " (magma/trim "hello  "))"#);
        assert_eq!(r, "ok");
    });
}

#[test]
fn trim_empty_string_returns_empty() {
    janet_test!(ed, {
        let r = janet_bridge::eval(r#"(= "" (magma/trim ""))"#);
        assert_eq!(r, "ok");
    });
}

// ── magma/trim-trailing ───────────────────────────────────────────────────

#[test]
fn trim_trailing_removes_trailing_spaces() {
    janet_test!(ed, {
        let r = janet_bridge::eval(r#"(= "hello" (magma/trim-trailing "hello  "))"#);
        assert_eq!(r, "ok");
    });
}

#[test]
fn trim_trailing_removes_trailing_tabs() {
    janet_test!(ed, {
        let r = janet_bridge::eval(r#"(= "hello" (magma/trim-trailing "hello\t\t"))"#);
        assert_eq!(r, "ok");
    });
}

#[test]
fn trim_trailing_does_not_affect_leading_spaces() {
    janet_test!(ed, {
        let r = janet_bridge::eval(r#"(= "  hello" (magma/trim-trailing "  hello"))"#);
        assert_eq!(r, "ok");
    });
}

#[test]
fn trim_trailing_empty_string_returns_empty() {
    janet_test!(ed, {
        let r = janet_bridge::eval(r#"(= "" (magma/trim-trailing ""))"#);
        assert_eq!(r, "ok");
    });
}

// ── magma/ensure-trailing-newline ─────────────────────────────────────────

#[test]
fn ensure_trailing_newline_appends_newline_to_nonempty() {
    janet_test!(ed, {
        let r = janet_bridge::eval(r#"(= "hello\n" (string (magma/ensure-trailing-newline "hello")))"#);
        assert_eq!(r, "ok");
    });
}

#[test]
fn ensure_trailing_newline_does_not_duplicate_newline() {
    janet_test!(ed, {
        let r = janet_bridge::eval(r#"(= "hello\n" (string (magma/ensure-trailing-newline "hello\n")))"#);
        assert_eq!(r, "ok");
    });
}

#[test]
fn ensure_trailing_newline_empty_string_returns_newline() {
    janet_test!(ed, {
        let r = janet_bridge::eval(r#"(= "\n" (string (magma/ensure-trailing-newline "")))"#);
        assert_eq!(r, "ok");
    });
}

// ── magma/indent ──────────────────────────────────────────────────────────

#[test]
fn indent_returns_indented_text() {
    janet_test!(ed, {
        let r = janet_bridge::eval(r#"(= "  hello\n  world\n" (string (magma/indent "hello\nworld" 2 " ")))"#);
        assert_eq!(r, "ok");
    });
}

#[test]
fn indent_with_zero_count_returns_original() {
    janet_test!(ed, {
        let r = janet_bridge::eval(r#"(= "hello\n" (string (magma/indent "hello" 0 "  ")))"#);
        assert_eq!(r, "ok");
    });
}

// ── magma/deep-merge ──────────────────────────────────────────────────────

#[test]
fn deep_merge_returns_merged_table() {
    janet_test!(ed, {
        let r = janet_bridge::eval(
            r#"(let [a {:x 1 :y {:a 10}} b {:y {:b 20} :z 3}]
                 (def m (magma/deep-merge a b))
                 (and (= (m :x) 1) (= (get-in m [:y :a]) 10)
                      (= (get-in m [:y :b]) 20) (= (m :z) 3)))"#);
        assert_eq!(r, "ok");
    });
}

#[test]
fn deep_merge_does_not_mutate_original() {
    janet_test!(ed, {
        let r = janet_bridge::eval(
            r#"(let [a {:x 1} b {:x 2}]
                 (magma/deep-merge a b)
                 (= (a :x) 1))"#);
        assert_eq!(r, "ok");
    });
}

// ── magma/time-now ────────────────────────────────────────────────────────

#[test]
fn time_now_returns_string() {
    janet_test!(ed, {
        let r = janet_bridge::eval(r#"(string? (magma/time-now))"#);
        assert_eq!(r, "ok");
    });
}

#[test]
fn time_now_format_contains_digits() {
    janet_test!(ed, {
        let r = janet_bridge::eval(r#"(> (length (magma/time-now)) 8)"#);
        assert_eq!(r, "ok");
    });
}

// ── magma/debug-log ───────────────────────────────────────────────────────

#[test]
fn debug_log_returns_nil() {
    janet_test!(ed, {
        let r = janet_bridge::eval(r#"(nil? (magma/debug-log "test"))"#);
        assert_eq!(r, "ok");
    });
}

// ── No generic-namespace shadowing ────────────────────────────────────────

#[test]
fn old_string_trim_is_still_janet_builtin() {
    janet_test!(ed, {
        let r = janet_bridge::eval(r#"(= "hello" (string/trim "  hello  "))"#);
        assert_eq!(r, "ok");
    });
}
