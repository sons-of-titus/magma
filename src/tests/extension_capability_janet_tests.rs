//! Janet API tests for Phase 10 — Plugin Capability System.

use crate::tests::helpers::acquire_janet_lock;
use crate::kernel::scripting;

macro_rules! jt {
    ($name:ident, $code:expr, $expected:expr) => {
        #[test]
        fn $name() {
            let _lock = acquire_janet_lock();
            let mut ed = crate::tests::helpers::make_editor();
            scripting::init(&mut ed);
            let result = scripting::eval_result($code).unwrap_or_else(|e| e);
            assert_eq!(result, $expected, "Janet: {}", $code);
        }
    };
}

// ── extension/list ────────────────────────────────────────────────────────────

jt!(extension_list_empty_on_start,
    "(length (extension/list))",
    "0");

// ── extension/manifest when none active ──────────────────────────────────────

jt!(extension_manifest_nil_when_none_active,
    "(= nil (extension/manifest))",
    "true");

// ── extension/capabilities for unknown ───────────────────────────────────────

jt!(extension_capabilities_nil_for_unknown,
    "(= nil (extension/capabilities \"ghost\"))",
    "true");

// ── extension/declare basic ───────────────────────────────────────────────────

jt!(extension_declare_returns_ok,
    r#"(= :ok (extension/declare "test-ext" "1.0" "test.janet" @[:editor-access]))"#,
    "true");

// ── extension/declare registers the extension ─────────────────────────────────

jt!(extension_declare_registers_in_list,
    r#"
    (extension/declare "listed-ext" "1.0" "listed.janet" @[:editor-access])
    (def exts (extension/list))
    (def found (find (fn [e] (= (get e :name) "listed-ext")) exts))
    (not (nil? found))
    "#,
    "true");

// ── extension/capabilities after declare ──────────────────────────────────────

jt!(extension_capabilities_after_declare,
    r#"
    (extension/declare "cap-ext" "1.0" "cap.janet" @[:filesystem :network])
    (def caps (extension/capabilities "cap-ext"))
    (and
      (not (nil? caps))
      (= (length caps) 2))
    "#,
    "true");

// ── extension/manifest returns current extension ──────────────────────────────

jt!(extension_manifest_returns_current,
    r#"
    (extension/declare "active-ext" "2.0" "active.janet" @[:editor-access])
    (def m (extension/manifest))
    (and
      (not (nil? m))
      (= (get m :name) "active-ext")
      (= (get m :version) "2.0"))
    "#,
    "true");

// ── extension/undeclare clears active context ─────────────────────────────────

jt!(extension_undeclare_clears_manifest,
    r#"
    (extension/declare "temp-ext" "1.0" "temp.janet" @[])
    (extension/undeclare)
    (= nil (extension/manifest))
    "#,
    "true");

// ── extension list count grows with declarations ──────────────────────────────

jt!(extension_list_grows,
    r#"
    (extension/declare "ext-a" "1.0" "a.janet" @[])
    (extension/undeclare)
    (extension/declare "ext-b" "1.0" "b.janet" @[])
    (extension/undeclare)
    (>= (length (extension/list)) 2)
    "#,
    "true");

// ── extension manifest has entry-point field ──────────────────────────────────

jt!(extension_manifest_has_entry_point,
    r#"
    (extension/declare "ep-ext" "1.0" "my-entry.janet" @[])
    (def m (extension/manifest))
    (= (get m :entry-point) "my-entry.janet")
    "#,
    "true");

// ── extension manifest has capabilities array ──────────────────────────────────

jt!(extension_manifest_has_capabilities_array,
    r#"
    (extension/declare "array-ext" "1.0" "a.janet" @[:filesystem :network])
    (def m (extension/manifest))
    (def caps (get m :capabilities @[]))
    (= (length caps) 2)
    "#,
    "true");

// ── capability keyword values ─────────────────────────────────────────────────

jt!(extension_capabilities_contain_correct_keywords,
    r#"
    (extension/declare "kw-ext" "1.0" "kw.janet" @[:filesystem :editor-access])
    (def caps (extension/capabilities "kw-ext"))
    (and
      (not (nil? (find (fn [c] (= c :filesystem)) caps)))
      (not (nil? (find (fn [c] (= c :editor-access)) caps))))
    "#,
    "true");

// ── core code is trusted (no capability restriction) ─────────────────────────

jt!(core_code_can_call_fs_cwd,
    "(string? (fs/cwd))",
    "true");
