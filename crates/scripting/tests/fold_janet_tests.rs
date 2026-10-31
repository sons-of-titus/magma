use scripting;
#[macro_use]
mod helpers;

// ── buffer/fold ───────────────────────────────────────────────────────

#[test]
fn buffer_fold_adds_fold_range() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor_with_buffer("line1\nline2\nline3\n");
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);
    let key = helpers::focused_key(&ed);

    let result = scripting::eval(&format!("(buffer/fold {} 0 12)", key));
    assert_eq!(result, "ok");

    let folds = ed.views.get(&key).unwrap().folds.clone();
    assert_eq!(folds.len(), 1);
    assert_eq!(folds[0], (0, 12));
}

#[test]
fn buffer_unfold_removes_fold_range() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor_with_buffer("line1\nline2\nline3\n");
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);
    let key = helpers::focused_key(&ed);

    let _ = scripting::eval(&format!("(buffer/fold {} 0 12)", key));
    let result = scripting::eval(&format!("(buffer/unfold {} 0 12)", key));
    assert_eq!(result, "ok");

    let folds = ed.views.get(&key).unwrap().folds.clone();
    assert!(folds.is_empty());
}

#[test]
fn buffer_unfold_all_removes_all_folds() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor_with_buffer("line1\nline2\nline3\n");
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);
    let key = helpers::focused_key(&ed);

    let _ = scripting::eval(&format!("(buffer/fold {} 0 6)", key));
    let _ = scripting::eval(&format!("(buffer/fold {} 12 18)", key));
    let result = scripting::eval(&format!("(buffer/unfold-all {})", key));
    assert_eq!(result, "ok");

    let folds = ed.views.get(&key).unwrap().folds.clone();
    assert!(folds.is_empty());
}

#[test]
fn buffer_folds_returns_empty_for_no_folds() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor_with_buffer("hello");
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);
    let key = helpers::focused_key(&ed);

    let result = scripting::eval(&format!("(buffer/folds {})", key));
    assert_eq!(result, "ok");
}

// ── fold ranges (candidates) ──────────────────────────────────────────

#[test]
fn buffer_fold_ranges_round_trip() {
    janet_test!(ed, {
        let key = helpers::focused_key(&ed);
        let result = scripting::eval(&format!(
            "(buffer/set-fold-ranges {key} [[0 12] [20 30]])",
            key = key
        ));
        assert_eq!(result, "ok");

        let ranges = ed.views.get(&key).unwrap().fold_ranges.clone();
        assert_eq!(ranges, vec![(0, 12), (20, 30)]);

        // Round-trip through the reader.
        let ok = scripting::eval_result(&format!(
            r#"(let [[a b] (buffer/fold-ranges {key})]
                 (and (= (first a) 0) (= (last a) 12)
                      (= (first b) 20) (= (last b) 30)))"#,
            key = key
        ))
        .unwrap();
        assert_eq!(ok, "true", "fold ranges must read back verbatim");
    });
}

#[test]
fn buffer_set_fold_ranges_with_no_argument_clears() {
    janet_test!(ed, {
        let key = helpers::focused_key(&ed);
        let _ = scripting::eval(&format!("(buffer/set-fold-ranges {key} [[0 12]])"));
        let _ = scripting::eval(&format!("(buffer/set-fold-ranges {key})"));
        assert!(ed.views.get(&key).unwrap().fold_ranges.is_empty());
    });
}

// ── fold commands ─────────────────────────────────────────────────────

#[test]
fn fold_all_collapses_every_candidate() {
    janet_test!(ed, {
        let key = helpers::focused_key(&ed);
        let _ = scripting::eval(&format!("(buffer/set-fold-ranges {key} [[0 8] [12 20]])"));
        let result = scripting::eval("(command/run \"fold-all\")");
        assert_eq!(result, "ok");

        let folds = ed.views.get(&key).unwrap().folds.clone();
        assert_eq!(folds, vec![(0, 8), (12, 20)]);
    });
}

#[test]
fn unfold_all_clears_folds_but_keeps_candidates() {
    janet_test!(ed, {
        let key = helpers::focused_key(&ed);
        let _ = scripting::eval(&format!("(buffer/set-fold-ranges {key} [[0 8]])"));
        let _ = scripting::eval("(command/run \"fold-all\")");
        let result = scripting::eval("(command/run \"unfold-all\")");
        assert_eq!(result, "ok");

        let view = ed.views.get(&key).unwrap();
        assert!(view.folds.is_empty(), "no active folds remain");
        assert_eq!(view.fold_ranges, vec![(0, 8)], "candidates survive");
    });
}

#[test]
fn toggle_fold_folds_then_unfolds_the_cursor_line() {
    janet_test!(ed, {
        let key = helpers::focused_key(&ed);
        let _ = scripting::eval(&format!("(buffer/set-fold-ranges {key} [[0 8]])"));
        let result = scripting::eval("(command/run \"toggle-fold\")");
        assert_eq!(result, "ok");
        assert_eq!(ed.views.get(&key).unwrap().folds, vec![(0, 8)]);

        // The cursor never moved: a second toggle opens the fold again.
        let _ = scripting::eval("(command/run \"toggle-fold\")");
        assert!(ed.views.get(&key).unwrap().folds.is_empty());
    });
}

#[test]
fn toggle_fold_on_a_line_without_candidates_is_a_noop() {
    janet_test!(ed, {
        let key = helpers::focused_key(&ed);
        let _ = scripting::eval(&format!("(buffer/set-fold-ranges {key} [[12 20]])"));
        let result = scripting::eval("(command/run \"toggle-fold\")");
        assert_eq!(result, "ok");
        assert!(ed.views.get(&key).unwrap().folds.is_empty());
    });
}
