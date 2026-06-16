use crate::kernel::scripting;

#[test]
fn vc_register_backend_adds_backend() {
    janet_test!(ed, {
        let count_before = ed.vc.backends.len();

        let r = scripting::eval(
            r#"(vc/register-backend {:name "fossil" :detect-marker ".fslckout"})"#,
        );
        assert_eq!(r, "ok", "vc/register-backend must not error");

        assert_eq!(
            ed.vc.backends.len(),
            count_before + 1,
            "backend count must increase by 1"
        );
        assert_eq!(
            ed.vc.backends.last().unwrap().name(),
            "fossil",
            "last registered backend must be 'fossil'"
        );
    });
}

#[test]
fn vc_register_backend_missing_name_signals_error() {
    janet_test!(ed, {
        let count_before = ed.vc.backends.len();

        let r = scripting::eval(
            r#"(vc/register-backend {:detect-marker ".fslckout"})"#,
        );
        assert_ne!(r, "ok", "expected error but got ok: {r}");

        assert_eq!(
            ed.vc.backends.len(),
            count_before,
            "backends must not change after error"
        );
    });
}

#[test]
fn vc_register_backend_missing_detect_signals_error() {
    janet_test!(ed, {
        let count_before = ed.vc.backends.len();

        let r = scripting::eval(
            r#"(vc/register-backend {:name "fossil"})"#,
        );
        assert_ne!(r, "ok", "expected error but got ok: {r}");

        assert_eq!(
            ed.vc.backends.len(),
            count_before,
            "backends must not change after error"
        );
    });
}

#[test]
fn vc_register_backend_wrong_arg_type_signals_error() {
    janet_test!(ed, {
        let count_before = ed.vc.backends.len();

        let r = scripting::eval(
            r#"(vc/register-backend "not-a-table")"#,
        );
        assert_ne!(r, "ok", "expected error but got ok: {r}");

        assert_eq!(
            ed.vc.backends.len(),
            count_before,
            "backends must not change after error"
        );
    });
}

#[test]
fn vc_register_backend_no_args_signals_error() {
    janet_test!(ed, {
        let r = scripting::eval(r#"(vc/register-backend)"#);
        assert_ne!(r, "ok", "expected error but got ok: {r}");
    });
}

#[test]
fn vc_register_backend_multiple_backends() {
    janet_test!(ed, {
        let r1 = scripting::eval(
            r#"(vc/register-backend {:name "fossil" :detect-marker ".fslckout"})"#,
        );
        assert_eq!(r1, "ok");

        let r2 = scripting::eval(
            r#"(vc/register-backend {:name "bzr" :detect-marker ".bzr"})"#,
        );
        assert_eq!(r2, "ok");

        let backends: Vec<&str> = ed.vc.backends.iter().map(|b| b.name()).collect();
        assert!(
            backends.contains(&"fossil"),
            "backends must contain 'fossil': {backends:?}"
        );
        assert!(
            backends.contains(&"bzr"),
            "backends must contain 'bzr': {backends:?}"
        );
    });
}

#[test]
fn vc_register_backend_with_optional_fields() {
    janet_test!(ed, {
        let r = scripting::eval(
            r#"(vc/register-backend {:name "custom"
                                       :detect-marker ".custom"
                                       :status-cmd "echo M file.txt"
                                       :has-staging true})"#,
        );
        assert_eq!(r, "ok", "register-backend with optional fields must not error");

        let last = ed.vc.backends.last().unwrap();
        assert_eq!(last.name(), "custom");
    });
}
