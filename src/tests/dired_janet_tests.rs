use crate::kernel::scripting;
use crate::kernel::vc::dired;
use crate::kernel::command::builtin::register_builtin_commands;

#[test]
fn dired_open_creates_dired_buffer() {
    janet_test!(ed, {
        register_builtin_commands(&mut ed);
        let tmp = std::env::temp_dir().join("magma_janet_dired_test");
        std::fs::create_dir_all(&tmp).unwrap();
        let tmp = tmp.canonicalize().unwrap_or(tmp);
        std::fs::write(tmp.join("hello.txt"), "world").unwrap();

        let r = scripting::eval(&format!(r#"(editor/run-command "dired" "{}")"#,
            tmp.to_string_lossy()));
        assert_eq!(r, "ok", "dired command must succeed");

        assert!(ed.dired.active, "dired must be active");
        assert!(ed.dired.buf_key.is_some(), "dired buf_key must be set");
        assert_eq!(ed.dired.dir, tmp, "dired dir must match");

        // Verify the *dired* buffer exists with content
        let bk = ed.dired.buf_key.unwrap();
        assert!(ed.buffers.contains(bk), "buffer must exist in slab");
        let arc = ed.buffers.get(bk).unwrap();
        let buf = arc.lock().unwrap();
        assert_eq!(buf.name, "*dired*", "buffer must be named *dired*");
        assert!(buf.len() > 0, "buffer must have content");
        assert!(buf.slice(0, buf.len()).contains("hello.txt"),
            "buffer must contain the file name");

        std::fs::remove_dir_all(&tmp).ok();
    });
}

#[test]
fn dired_refresh_reloads_content() {
    janet_test!(ed, {
        register_builtin_commands(&mut ed);
        let tmp = std::env::temp_dir().join("magma_janet_dired_refresh");
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("a.txt"), "1").unwrap();

        scripting::eval(&format!(r#"(editor/run-command "dired" "{}")"#,
            tmp.to_string_lossy()));

        std::fs::write(tmp.join("b.txt"), "2").unwrap();
        let r = scripting::eval("(editor/run-command \"dired-refresh\")");
        assert_eq!(r, "ok", "dired-refresh must succeed");

        let bk = ed.dired.buf_key.unwrap();
        let arc = ed.buffers.get(bk).unwrap();
        let buf = arc.lock().unwrap();
        let text = buf.slice(0, buf.len());
        assert!(text.contains("b.txt"), "buffer must contain new file after refresh");

        std::fs::remove_dir_all(&tmp).ok();
    });
}

#[test]
fn dired_mark_changes_state() {
    janet_test!(ed, {
        register_builtin_commands(&mut ed);
        let tmp = std::env::temp_dir().join("magma_janet_dired_mark");
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("f.txt"), "x").unwrap();

        scripting::eval(&format!(r#"(editor/run-command "dired" "{}")"#,
            tmp.to_string_lossy()));

        // Navigate past '.' and '..' to reach f.txt, then mark it
        scripting::eval("(editor/run-command \"cursor-down\")");
        scripting::eval("(editor/run-command \"cursor-down\")");
        scripting::eval("(editor/run-command \"dired-mark\")");

        assert!(ed.dired.marks.contains_key("f.txt"),
            "f.txt must be marked after navigating to it");

        std::fs::remove_dir_all(&tmp).ok();
    });
}

#[test]
fn dired_toggle_hidden_redraws() {
    janet_test!(ed, {
        register_builtin_commands(&mut ed);
        let tmp = std::env::temp_dir().join("magma_janet_dired_hidden");
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join(".hidden"), "secret").unwrap();
        std::fs::write(tmp.join("visible"), "open").unwrap();

        scripting::eval(&format!(r#"(editor/run-command "dired" "{}")"#,
            tmp.to_string_lossy()));

        assert!(!ed.dired.show_hidden, "hidden files must start hidden");

        scripting::eval("(editor/run-command \"dired-toggle-hidden\")");
        assert!(ed.dired.show_hidden, "hidden must now be shown");

        scripting::eval("(editor/run-command \"dired-toggle-hidden\")");
        assert!(!ed.dired.show_hidden, "hidden must toggle back");

        std::fs::remove_dir_all(&tmp).ok();
    });
}

#[test]
fn dired_toggle_sort_cycles() {
    janet_test!(ed, {
        register_builtin_commands(&mut ed);
        let tmp = std::env::temp_dir().join("magma_janet_dired_sort");
        std::fs::create_dir_all(&tmp).unwrap();

        scripting::eval(&format!(r#"(editor/run-command "dired" "{}")"#,
            tmp.to_string_lossy()));

        assert_eq!(ed.dired.sort_field, dired::SortField::Name);
        scripting::eval("(editor/run-command \"dired-toggle-sort\")");
        assert_eq!(ed.dired.sort_field, dired::SortField::Size);
        scripting::eval("(editor/run-command \"dired-toggle-sort\")");
        assert_eq!(ed.dired.sort_field, dired::SortField::Date);
        scripting::eval("(editor/run-command \"dired-toggle-sort\")");
        assert_eq!(ed.dired.sort_field, dired::SortField::Name);

        std::fs::remove_dir_all(&tmp).ok();
    });
}

#[test]
fn dired_filter_filters_entries() {
    janet_test!(ed, {
        register_builtin_commands(&mut ed);
        let tmp = std::env::temp_dir().join("magma_janet_dired_filter");
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("alpha.txt"), "1").unwrap();
        std::fs::write(tmp.join("beta.txt"), "2").unwrap();

        scripting::eval(&format!(r#"(editor/run-command "dired" "{}")"#,
            tmp.to_string_lossy()));

        scripting::eval("(editor/run-command \"dired-filter\" \"alpha\")");
        assert_eq!(ed.dired.filter_pattern.as_deref(), Some("alpha"));

        scripting::eval("(editor/run-command \"dired-clear-filter\")");
        assert!(ed.dired.filter_pattern.is_none());

        std::fs::remove_dir_all(&tmp).ok();
    });
}

#[test]
fn dired_execute_deletion_removes_marked_files() {
    janet_test!(ed, {
        register_builtin_commands(&mut ed);
        let tmp = std::env::temp_dir().join("magma_janet_dired_delete");
        std::fs::create_dir_all(&tmp).unwrap();
        let fpath = tmp.join("delete_me.txt");
        std::fs::write(&fpath, "bye").unwrap();

        assert!(fpath.exists(), "file must exist before deletion");

        scripting::eval(&format!(r#"(editor/run-command "dired" "{}")"#,
            tmp.to_string_lossy()));

        // Mark the file with delete mark
        ed.dired.marks.insert("delete_me.txt".to_string(), dired::MarkType::Delete);

        let r = scripting::eval("(editor/run-command \"dired-execute-deletion\")");
        assert_eq!(r, "ok", "deletion must succeed");

        assert!(!fpath.exists(), "file must be deleted");
        assert!(ed.dired.marks.is_empty(), "marks must be cleared after deletion");

        std::fs::remove_dir_all(&tmp).ok();
    });
}
