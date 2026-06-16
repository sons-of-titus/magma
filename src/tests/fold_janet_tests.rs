use crate::kernel::scripting;
use crate::tests::helpers;

// ── buffer/fold ───────────────────────────────────────────────────────

#[test]
fn buffer_fold_adds_fold_range() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor_with_buffer("line1\nline2\nline3\n");
    scripting::init(&mut ed);
    let key = helpers::focused_key(&ed);

    let result = scripting::eval(
        &format!("(buffer/fold {} 0 12)", key));
    assert_eq!(result, "ok");

    let folds = &ed.buffers.get(key).unwrap().folds;
    assert_eq!(folds.len(), 1);
    assert_eq!(folds[0], (0, 12));
}

#[test]
fn buffer_unfold_removes_fold_range() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor_with_buffer("line1\nline2\nline3\n");
    scripting::init(&mut ed);
    let key = helpers::focused_key(&ed);

    let _ = scripting::eval(
        &format!("(buffer/fold {} 0 12)", key));
    let result = scripting::eval(
        &format!("(buffer/unfold {} 0 12)", key));
    assert_eq!(result, "ok");

    let folds = &ed.buffers.get(key).unwrap().folds;
    assert!(folds.is_empty());
}

#[test]
fn buffer_unfold_all_removes_all_folds() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor_with_buffer("line1\nline2\nline3\n");
    scripting::init(&mut ed);
    let key = helpers::focused_key(&ed);

    let _ = scripting::eval(
        &format!("(buffer/fold {} 0 6)", key));
    let _ = scripting::eval(
        &format!("(buffer/fold {} 12 18)", key));
    let result = scripting::eval(
        &format!("(buffer/unfold-all {})", key));
    assert_eq!(result, "ok");

    let folds = &ed.buffers.get(key).unwrap().folds;
    assert!(folds.is_empty());
}

#[test]
fn buffer_folds_returns_empty_for_no_folds() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor_with_buffer("hello");
    scripting::init(&mut ed);
    let key = helpers::focused_key(&ed);

    let result = scripting::eval(
        &format!("(buffer/folds {})", key));
    assert_eq!(result, "ok");
}
