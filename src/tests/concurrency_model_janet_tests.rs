//! Janet API tests for Phase 9 — Concurrency Model.

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

// ── scheduler/list ───────────────────────────────────────────────────────────

jt!(scheduler_list_empty_on_start,
    "(length (scheduler/list))",
    "0");

// ── scheduler/status ─────────────────────────────────────────────────────────

jt!(scheduler_status_not_found_for_unknown,
    "(scheduler/status 999999)",
    ":not-found");

// ── scheduler/progress ───────────────────────────────────────────────────────

jt!(scheduler_progress_nil_for_unknown,
    "(= nil (scheduler/progress 999999))",
    "true");

// ── scheduler/cancel ─────────────────────────────────────────────────────────

jt!(scheduler_cancel_unknown_returns_nil,
    "(= nil (scheduler/cancel 999))",
    "true");

// ── scheduler/submit ─────────────────────────────────────────────────────────

#[test]
fn scheduler_submit_returns_positive_id() {
    let _lock = acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor();

    use crate::kernel::runtime::{BackgroundHandle, BackgroundEvent};
    use std::sync::Arc;
    let rt = Arc::new(tokio::runtime::Runtime::new().unwrap());
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel::<BackgroundEvent>();
    ed.background = Some(BackgroundHandle::new(rt, tx));

    scripting::init(&mut ed);
    let result = scripting::eval_result(r#"(> (scheduler/submit "test-work" "true") 0)"#)
        .unwrap_or_default();
    assert_eq!(result, "true");
}

#[test]
fn scheduler_submit_appears_in_list() {
    let _lock = acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor();

    use crate::kernel::runtime::{BackgroundHandle, BackgroundEvent};
    use std::sync::Arc;
    let rt = Arc::new(tokio::runtime::Runtime::new().unwrap());
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel::<BackgroundEvent>();
    ed.background = Some(BackgroundHandle::new(rt, tx));

    scripting::init(&mut ed);
    let result = scripting::eval_result(
        r#"(do (scheduler/submit "my-work" "echo hello") (> (length (scheduler/list)) 0))"#
    ).unwrap_or_default();
    assert_eq!(result, "true");
}

// ── scheduler/status after registration ─────────────────────────────────────

#[test]
fn submitted_work_status_is_running_or_completed() {
    let _lock = acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor();

    use crate::kernel::runtime::{BackgroundHandle, BackgroundEvent};
    use std::sync::Arc;
    let rt = Arc::new(tokio::runtime::Runtime::new().unwrap());
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel::<BackgroundEvent>();
    ed.background = Some(BackgroundHandle::new(rt, tx));

    scripting::init(&mut ed);
    let result = scripting::eval_result(
        r#"(do
             (def id (scheduler/submit "quick-work" "true"))
             (def s (scheduler/status id))
             (or (= s :running) (= s :completed)))"#
    ).unwrap_or_default();
    assert_eq!(result, "true");
}

// ── scheduler/progress structure ─────────────────────────────────────────────

#[test]
fn progress_table_has_done_and_total_keys() {
    let _lock = acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor();

    use crate::kernel::runtime::{BackgroundHandle, BackgroundEvent};
    use std::sync::Arc;
    let rt = Arc::new(tokio::runtime::Runtime::new().unwrap());
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel::<BackgroundEvent>();
    ed.background = Some(BackgroundHandle::new(rt, tx));

    scripting::init(&mut ed);
    // Register a task manually via Rust to get a deterministic work-id
    let work_id = ed.scheduler.register_task(100, "check-work");
    ed.scheduler.update_progress(work_id, 3, 10);

    // Re-init EDITOR_PTR (it was set during init)
    crate::kernel::scripting::set_editor_ptr(&mut ed as *mut _);

    let code = format!("(do (def p (scheduler/progress {})) (and (= (get p :done) 3) (= (get p :total) 10)))", work_id);
    let result = scripting::eval_result(&code).unwrap_or_default();
    assert_eq!(result, "true");
}

// ── scheduler/cancel changes status ─────────────────────────────────────────

#[test]
fn cancel_changes_status_to_cancelled() {
    let _lock = acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor();
    scripting::init(&mut ed);

    let work_id = ed.scheduler.register_task(200, "cancel-me");
    crate::kernel::scripting::set_editor_ptr(&mut ed as *mut _);

    let code = format!(
        r#"(do (scheduler/cancel {}) (= (scheduler/status {}) :cancelled))"#,
        work_id, work_id
    );
    let result = scripting::eval_result(&code).unwrap_or_default();
    assert_eq!(result, "true");
}

// ── scheduler/list contents ──────────────────────────────────────────────────

#[test]
fn list_item_has_expected_keys() {
    let _lock = acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor();
    scripting::init(&mut ed);

    ed.scheduler.register_task(300, "list-test");
    crate::kernel::scripting::set_editor_ptr(&mut ed as *mut _);

    let result = scripting::eval_result(
        r#"(do
             (def items (scheduler/list))
             (def item  (get items 0))
             (and (has-key? item :id)
                  (has-key? item :name)
                  (has-key? item :status)
                  (has-key? item :done)
                  (has-key? item :total)))"#
    ).unwrap_or_default();
    assert_eq!(result, "true");
}

#[test]
fn list_item_name_matches_registered_name() {
    let _lock = acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor();
    scripting::init(&mut ed);

    ed.scheduler.register_task(400, "my-named-work");
    crate::kernel::scripting::set_editor_ptr(&mut ed as *mut _);

    let result = scripting::eval_result(
        r#"(do
             (def items (scheduler/list))
             (def item  (get items 0))
             (= (get item :name) "my-named-work"))"#
    ).unwrap_or_default();
    assert_eq!(result, "true");
}
