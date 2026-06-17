//! Janet API tests for the GutterProvider architecture (Phase 7).

use crate::kernel::scripting;

// ── gutter/add-provider ───────────────────────────────────────────────────────

#[test]
fn add_provider_registers_provider() {
    janet_test!(ed, {
        // gutter.janet pre-registers ":line-numbers" etc. via gutter/add-provider.
        let names: Vec<&str> = ed.gutter.providers.iter().map(|p| p.name()).collect();
        assert!(names.contains(&":line-numbers"), "line-numbers should be registered");
        assert!(names.contains(&":diagnostics"), "diagnostics should be registered");
    });
}

#[test]
fn add_provider_preserves_insertion_order() {
    janet_test!(ed, {
        let names: Vec<&str> = ed.gutter.providers.iter().map(|p| p.name()).collect();
        let bp_pos   = names.iter().position(|&n| n == ":breakpoints").unwrap_or(usize::MAX);
        let ln_pos   = names.iter().position(|&n| n == ":line-numbers").unwrap_or(usize::MAX);
        let fold_pos = names.iter().position(|&n| n == ":folding").unwrap_or(usize::MAX);
        assert!(bp_pos < ln_pos, "breakpoints should come before line-numbers");
        assert!(ln_pos < fold_pos, "line-numbers should come before folding");
    });
}

#[test]
fn add_custom_provider_appends_to_list() {
    janet_test!(ed, {
        scripting::eval(r#"(gutter/add-provider "my-test-provider")"#);
        let names: Vec<&str> = ed.gutter.providers.iter().map(|p| p.name()).collect();
        assert!(names.contains(&"my-test-provider"));
    });
}

#[test]
fn add_provider_replaces_existing_by_name() {
    janet_test!(ed, {
        scripting::eval(r#"(gutter/add-provider "dup-provider")"#);
        scripting::eval(r#"(gutter/add-provider "dup-provider")"#);
        let count = ed.gutter.providers.iter().filter(|p| p.name() == "dup-provider").count();
        assert_eq!(count, 1);
    });
}

// ── gutter/remove-provider ────────────────────────────────────────────────────

#[test]
fn remove_provider_removes_by_name() {
    janet_test!(ed, {
        scripting::eval(r#"(gutter/add-provider "to-remove")"#);
        let before = ed.gutter.providers.iter().any(|p| p.name() == "to-remove");
        assert!(before, "provider should exist before removal");
        scripting::eval(r#"(gutter/remove-provider "to-remove")"#);
        let after = ed.gutter.providers.iter().any(|p| p.name() == "to-remove");
        assert!(!after, "provider should be gone after removal");
    });
}

// ── gutter/provider-update ────────────────────────────────────────────────────

#[test]
fn provider_update_stores_cells_in_sign_cache() {
    janet_test!(ed, {
        let key = ed.create_buffer("test.rs");
        scripting::eval(r#"(gutter/add-provider "my-marks")"#);
        let cells = format!(r#"@[{{:line 3 :text "●" :face "default"}}]"#);
        scripting::eval(&format!(r#"(gutter/provider-update "my-marks" {key} {cells})"#));
        let cached = ed.gutter.cached_cell("my-marks", key, 3);
        assert!(cached.is_some(), "cell should be cached after provider-update");
        assert_eq!(cached.unwrap().text, "●");
    });
}

#[test]
fn provider_update_replaces_previous_cells() {
    janet_test!(ed, {
        let key = ed.create_buffer("test.rs");
        scripting::eval(r#"(gutter/add-provider "my-marks")"#);
        let ca = format!(r#"@[{{:line 0 :text "A" :face "default"}}]"#);
        let cb = format!(r#"@[{{:line 0 :text "B" :face "default"}}]"#);
        scripting::eval(&format!(r#"(gutter/provider-update "my-marks" {key} {ca})"#));
        scripting::eval(&format!(r#"(gutter/provider-update "my-marks" {key} {cb})"#));
        let cached = ed.gutter.cached_cell("my-marks", key, 0).unwrap();
        assert_eq!(cached.text, "B");
    });
}

#[test]
fn provider_update_emits_gutter_provider_updated_event() {
    janet_test!(ed, {
        let key = ed.create_buffer("test.rs");
        let counter = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let counter2 = counter.clone();
        ed.commands.register_fn("count-gutter-updated", "", vec![], move |_ed, _args| {
            counter2.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        });
        scripting::eval(r#"(gutter/add-provider "evt-provider")"#);
        scripting::eval(
            r#"(event/on "gutter-provider-updated"
                 (fn [_] (editor/run-command "count-gutter-updated")))"#
        );
        scripting::eval(&format!(
            r#"(gutter/provider-update "evt-provider" {key} [])"#
        ));
        ed.events.drain_and_dispatch();
        assert!(counter.load(std::sync::atomic::Ordering::SeqCst) > 0);
    });
}

// ── gutter/provider-list ─────────────────────────────────────────────────────

#[test]
fn provider_list_returns_all_providers() {
    janet_test!(ed, {
        let result = scripting::eval("(length (gutter/provider-list))");
        assert_eq!(result, "ok");
        assert!(!ed.gutter.providers.is_empty());
    });
}

// ── gutter/set-fold-icons ────────────────────────────────────────────────────

#[test]
fn set_fold_icons_stores_icons() {
    janet_test!(ed, {
        scripting::eval(r#"(gutter/set-fold-icons "v" ">" "my-fold-face")"#);
        assert_eq!(ed.gutter.fold_icons.open, "v");
        assert_eq!(ed.gutter.fold_icons.closed, ">");
        assert_eq!(ed.gutter.fold_icons.face, "my-fold-face");
    });
}

#[test]
fn set_fold_icons_defaults_from_gutter_janet() {
    janet_test!(ed, {
        assert_eq!(ed.gutter.fold_icons.open, "▾");
        assert_eq!(ed.gutter.fold_icons.closed, "▸");
    });
}

// ── gutter/set-line-number-format ────────────────────────────────────────────

#[test]
fn set_line_number_format_stores_fn_name() {
    janet_test!(ed, {
        scripting::eval(r#"(gutter/set-line-number-format "gutter-lnum-relative")"#);
        assert_eq!(ed.gutter.line_number_fn.as_deref(), Some("gutter-lnum-relative"));
    });
}

#[test]
fn set_line_number_format_nil_clears_fn() {
    janet_test!(ed, {
        scripting::eval(r#"(gutter/set-line-number-format "gutter-lnum-absolute")"#);
        scripting::eval(r#"(gutter/set-line-number-format nil)"#);
        assert!(ed.gutter.line_number_fn.is_none());
    });
}
