#[macro_use]
mod helpers;
// Janet API tests for Phase 10 — Plugin Capability System.

use helpers::acquire_janet_lock;
use scripting;

macro_rules! jt {
    ($name:ident, $code:expr, $expected:expr) => {
        #[test]
        fn $name() {
            let _lock = acquire_janet_lock();
            let mut ed = helpers::make_editor();
            scripting::init(&mut ed);
            let _editor_scope = scripting::EditorScope::new(&mut ed);
            let result = scripting::eval_result($code).unwrap_or_else(|e| e);
            assert_eq!(result, $expected, "Janet: {}", $code);
        }
    };
}

// ── extension/list ────────────────────────────────────────────────────────────

jt!(
    extension_list_empty_on_start,
    "(length (extension/list))",
    "0"
);

// ── extension/manifest when none active ──────────────────────────────────────

jt!(
    extension_manifest_nil_when_none_active,
    "(= nil (extension/manifest))",
    "true"
);

// ── extension/capabilities for unknown ───────────────────────────────────────

jt!(
    extension_capabilities_nil_for_unknown,
    "(= nil (extension/capabilities \"ghost\"))",
    "true"
);

// ── extension/declare basic ───────────────────────────────────────────────────

jt!(
    extension_declare_returns_ok,
    r#"(= :ok (extension/declare "test-ext" "1.0" "test.janet" @[:editor-access]))"#,
    "true"
);

// ── extension/declare registers the extension ─────────────────────────────────

jt!(
    extension_declare_registers_in_list,
    r#"
    (extension/declare "listed-ext" "1.0" "listed.janet" @[:editor-access])
    (def exts (extension/list))
    (def found (find (fn [e] (= (get e :name) "listed-ext")) exts))
    (not (nil? found))
    "#,
    "true"
);

// ── extension/capabilities after declare ──────────────────────────────────────

jt!(
    extension_capabilities_after_declare,
    r#"
    (extension/declare "cap-ext" "1.0" "cap.janet" @[:filesystem :network])
    (def caps (extension/capabilities "cap-ext"))
    (and
      (not (nil? caps))
      (= (length caps) 2))
    "#,
    "true"
);

// ── extension/manifest returns current extension ──────────────────────────────

jt!(
    extension_manifest_returns_current,
    r#"
    (extension/declare "active-ext" "2.0" "active.janet" @[:editor-access])
    (def m (extension/manifest))
    (and
      (not (nil? m))
      (= (get m :name) "active-ext")
      (= (get m :version) "2.0"))
    "#,
    "true"
);

// ── extension/undeclare clears active context ─────────────────────────────────

jt!(
    extension_undeclare_clears_manifest,
    r#"
    (extension/declare "temp-ext" "1.0" "temp.janet" @[])
    (extension/undeclare)
    (= nil (extension/manifest))
    "#,
    "true"
);

// ── extension list count grows with declarations ──────────────────────────────

jt!(
    extension_list_grows,
    r#"
    (extension/declare "ext-a" "1.0" "a.janet" @[])
    (extension/undeclare)
    (extension/declare "ext-b" "1.0" "b.janet" @[])
    (extension/undeclare)
    (>= (length (extension/list)) 2)
    "#,
    "true"
);

// ── extension manifest has entry-point field ──────────────────────────────────

jt!(
    extension_manifest_has_entry_point,
    r#"
    (extension/declare "ep-ext" "1.0" "my-entry.janet" @[])
    (def m (extension/manifest))
    (= (get m :entry-point) "my-entry.janet")
    "#,
    "true"
);

// ── extension manifest has capabilities array ──────────────────────────────────

jt!(
    extension_manifest_has_capabilities_array,
    r#"
    (extension/declare "array-ext" "1.0" "a.janet" @[:filesystem :network])
    (def m (extension/manifest))
    (def caps (get m :capabilities @[]))
    (= (length caps) 2)
    "#,
    "true"
);

// ── capability keyword values ─────────────────────────────────────────────────

jt!(
    extension_capabilities_contain_correct_keywords,
    r#"
    (extension/declare "kw-ext" "1.0" "kw.janet" @[:filesystem :editor-access])
    (def caps (extension/capabilities "kw-ext"))
    (and
      (not (nil? (find (fn [c] (= c :filesystem)) caps)))
      (not (nil? (find (fn [c] (= c :editor-access)) caps))))
    "#,
    "true"
);

// ── core code is trusted (no capability restriction) ─────────────────────────

jt!(core_code_can_call_fs_cwd, "(string? (fs/cwd))", "true");

// ── Trust model: no self-escalation, runtime enforcement (F-28, R-18) ────────

jt!(
    self_undeclare_leaves_context_revoked_not_core,
    r#"
    (extension/declare "tricky" "1.0" "t.janet" @[])
    (def denied-in-ext (protect (fs/cwd)))
    (extension/undeclare)
    (def denied-after (protect (fs/cwd)))
    (and (not (first denied-in-ext))
         (not (first denied-after)))
    "#,
    "true"
);

#[test]
fn event_handler_runs_in_its_registration_context() {
    let _lock = acquire_janet_lock();
    let mut ed = helpers::make_editor();
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);

    scripting::eval(
        r#"
        (extension/declare "capped" "1.0" "c.janet" @[])
        (var saw-fs-ok false)
        (event/on "capped-evt" (fn [_] (set saw-fs-ok (first (protect (fs/cwd))))))
        (extension/undeclare)
    "#,
    );
    // The handler runs later, from core dispatch: it must still be
    // confined to the (empty) capability set of "capped".
    scripting::eval(r#"(event/emit "capped-evt" {})"#);
    ed.dispatch_events_scoped();
    let saw = scripting::eval_result("saw-fs-ok").unwrap();
    assert_eq!(
        saw, "false",
        "a handler must run inside its registering extension's capability set"
    );
}

#[test]
fn command_runs_in_its_definition_context() {
    let _lock = acquire_janet_lock();
    let mut ed = helpers::make_editor();
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);

    scripting::eval(
        r#"
        (extension/declare "cmdcap" "1.0" "cc.janet" @[])
        (var cmd-fs-ok false)
        (command/define "captest-cmd" (fn [] (set cmd-fs-ok (first (protect (fs/cwd))))))
        (extension/undeclare)
    "#,
    );
    // Run from core dispatch.
    scripting::eval(r#"(editor/run-command "captest-cmd")"#);
    let saw = scripting::eval_result("cmd-fs-ok").unwrap();
    assert_eq!(
        saw, "false",
        "a command must run inside its defining extension's capability set"
    );
}

#[test]
fn core_registered_handlers_stay_trusted() {
    let _lock = acquire_janet_lock();
    let mut ed = helpers::make_editor();
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);

    scripting::eval(
        r#"
        (var core-fs-ok false)
        (event/on "core-evt" (fn [_] (set core-fs-ok (first (protect (fs/cwd))))))
        (event/emit "core-evt" {})
    "#,
    );
    ed.dispatch_events_scoped();
    let saw = scripting::eval_result("core-fs-ok").unwrap();
    assert_eq!(saw, "true", "core-registered handlers keep full trust");
}

#[test]
fn task_on_complete_runs_in_its_registration_context() {
    let _lock = acquire_janet_lock();
    let mut ed = helpers::make_editor();
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);

    scripting::eval(
        r#"
        (extension/declare "cbcap" "1.0" "cb.janet" @[])
        (var saw-manifest false)
        (task/on-complete 4242 (fn [_] (set saw-manifest (not (nil? (extension/manifest))))))
        (extension/undeclare)
    "#,
    );
    // The callback fires from core dispatch when the task finishes.
    scripting::execute_task_complete_callback_scoped(4242);
    let saw = scripting::eval_result("saw-manifest").unwrap();
    assert_eq!(
        saw, "true",
        "a task/on-complete callback must run inside its registering extension's capability set"
    );
}

#[test]
fn core_registered_task_callback_stays_trusted() {
    let _lock = acquire_janet_lock();
    let mut ed = helpers::make_editor();
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);

    scripting::eval(
        r#"
        (var core-manifest-nil false)
        (task/on-complete 4243 (fn [_] (set core-manifest-nil (nil? (extension/manifest)))))
    "#,
    );
    scripting::execute_task_complete_callback_scoped(4243);
    let saw = scripting::eval_result("core-manifest-nil").unwrap();
    assert_eq!(
        saw, "true",
        "core-registered task callbacks keep full trust"
    );
}

// ── ProcessExecution gates every shell-executing surface ─────────────────────
// task/define, task/run, and scheduler/submit all execute shell commands;
// an extension without the capability must be refused at the API boundary.

jt!(
    task_define_requires_process_execution,
    r#"(do
         (extension/declare "unproc" "1.0" "u.janet" @[:editor-access])
         (def [ok err] (protect (task/define "build" "cargo build")))
         (and (not ok) (not (nil? (string/find "ProcessExecution" err)))))"#,
    "true"
);

jt!(
    task_run_requires_process_execution,
    r#"(do
         (extension/declare "unproc-run" "1.0" "u.janet" @[:editor-access])
         (def [ok err] (protect (task/run "some-task")))
         (and (not ok) (not (nil? (string/find "ProcessExecution" err)))))"#,
    "true"
);

jt!(
    scheduler_submit_requires_process_execution,
    r#"(do
         (extension/declare "unproc-sched" "1.0" "u.janet" @[:editor-access])
         (def [ok err] (protect (scheduler/submit "grep" "rg x")))
         (and (not ok) (not (nil? (string/find "ProcessExecution" err)))))"#,
    "true"
);

jt!(
    task_define_allowed_with_process_execution,
    r#"(do
         (extension/declare "proc" "1.0" "p.janet" @[:process-execution])
         (number? (task/define "noop-task" "true")))"#,
    "true"
);
