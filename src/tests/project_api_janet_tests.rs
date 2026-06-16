use crate::kernel::scripting;

#[test]
fn project_root_returns_nil_by_default() {
    janet_test!(ed, {
        let r = scripting::eval("(project/root)");
        assert_eq!(r, "ok", "project/root default must not error");
        assert!(ed.project_manager.project.root.is_none(), "project root must be None by default");
    });
}

#[test]
fn project_set_root_creates_project_state() {
    janet_test!(ed, {
        let tmp = std::env::temp_dir().join("sprint4_janet_set_root");
        std::fs::create_dir_all(&tmp).unwrap();
        let root_str = tmp.to_string_lossy().to_string();

        let expr = format!("(project/set-root \"{}\")", root_str);
        let r = scripting::eval(&expr);
        assert_eq!(r, "ok", "project/set-root must not error");

        assert!(ed.project_manager.project.root.is_some(), "project root must be set");
        let root_got = ed.project_manager.project.root.as_ref().map(|p| p.to_string_lossy().into_owned());
        assert_eq!(root_got.as_deref(), Some(root_str.as_str()));
        assert!(ed.project_manager.current_project.is_some(), "current_project must be set");

        std::fs::remove_dir_all(&tmp).ok();
    });
}

#[test]
fn project_set_root_clears_when_nil() {
    janet_test!(ed, {
        // Set a root first
        let tmp = std::env::temp_dir().join("sprint4_janet_clear");
        std::fs::create_dir_all(&tmp).unwrap();
        let root_str = tmp.to_string_lossy().to_string();
        let expr = format!("(project/set-root \"{}\")", root_str);
        scripting::eval(&expr);

        // Clear it
        let r = scripting::eval("(project/set-root nil)");
        assert_eq!(r, "ok");
        assert!(ed.project_manager.project.root.is_none(), "project root must be cleared");

        std::fs::remove_dir_all(&tmp).ok();
    });
}

#[test]
fn project_set_name_works() {
    janet_test!(ed, {
        let tmp = std::env::temp_dir().join("sprint4_janet_name");
        std::fs::create_dir_all(&tmp).unwrap();
        let root_str = tmp.to_string_lossy().to_string();
        let s = format!("(project/set-root \"{}\")", root_str);
        scripting::eval(&s);

        let r = scripting::eval("(project/set-name \"my-project\")");
        assert_eq!(r, "ok");
        assert_eq!(ed.project_manager.project.name.as_deref(), Some("my-project"));

        std::fs::remove_dir_all(&tmp).ok();
    });
}

#[test]
fn project_option_set_and_get() {
    janet_test!(ed, {
        let tmp = std::env::temp_dir().join("sprint4_janet_opts");
        std::fs::create_dir_all(&tmp).unwrap();
        let root_str = tmp.to_string_lossy().to_string();
        let s = format!("(project/set-root \"{}\")", root_str);
        scripting::eval(&s);

        scripting::eval(r#"(project/option-set "build-command" "cargo check")"#);
        assert_eq!(
            ed.project_manager.project.options.get("build-command").map(|s| s.as_str()),
            Some("cargo check")
        );

        std::fs::remove_dir_all(&tmp).ok();
    });
}

#[test]
fn project_register_and_list() {
    janet_test!(ed, {
        let tmp = std::env::temp_dir().join("sprint4_janet_reg");
        let tmp2 = std::env::temp_dir().join("sprint4_janet_reg2");
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::create_dir_all(&tmp2).unwrap();
        let r1 = tmp.to_string_lossy().to_string();
        let r2 = tmp2.to_string_lossy().to_string();

        let expr1 = format!("(project/register \"proj-a\" \"{}\")", r1);
        let expr2 = format!("(project/register \"proj-b\" \"{}\")", r2);

        let r = scripting::eval(&expr1);
        assert_eq!(r, "ok");
        let r = scripting::eval(&expr2);
        assert_eq!(r, "ok");

        assert!(ed.project_manager.projects.contains_key("proj-a"));
        assert!(ed.project_manager.projects.contains_key("proj-b"));
        assert_eq!(ed.project_manager.projects.len(), 2);

        std::fs::remove_dir_all(&tmp).ok();
        std::fs::remove_dir_all(&tmp2).ok();
    });
}

#[test]
fn project_unregister_removes() {
    janet_test!(ed, {
        let tmp = std::env::temp_dir().join("sprint4_janet_unreg");
        std::fs::create_dir_all(&tmp).unwrap();
        let r = tmp.to_string_lossy().to_string();
        let expr = format!("(project/register \"my-proj\" \"{}\")", r);
        scripting::eval(&expr);

        assert!(ed.project_manager.projects.contains_key("my-proj"));

        scripting::eval("(project/unregister \"my-proj\")");
        assert!(!ed.project_manager.projects.contains_key("my-proj"));

        std::fs::remove_dir_all(&tmp).ok();
    });
}

#[test]
fn project_buffer_set_and_get() {
    janet_test!(ed, {
        let key = ed.view_tree.focused_window()
            .and_then(|wid| ed.view_tree.buffer(wid))
            .unwrap_or(0);

        let expr = format!("(project/buffer-set-project {} \"my-proj\")", key);
        let r = scripting::eval(&expr);
        assert_eq!(r, "ok");

        assert_eq!(
            ed.project_manager.buffer_projects.get(&key).map(|s| s.as_str()),
            Some("my-proj")
        );

        let get_expr = format!("(project/buffer-project {})", key);
        let r = scripting::eval(&get_expr);
        assert_eq!(r, "ok");
    });
}

#[test]
fn project_files_empty_by_default() {
    janet_test!(ed, {
        let r = scripting::eval("(project/files)");
        assert_eq!(r, "ok", "project/files default must not error");
        assert!(ed.project_manager.project.files.is_empty(), "files must be empty by default");
    });
}

#[test]
fn project_set_workspace_members_via_janet() {
    janet_test!(ed, {
        let tmp = std::env::temp_dir().join("sprint4_janet_ws");
        let m1 = tmp.join("sub-a");
        let m2 = tmp.join("sub-b");
        std::fs::create_dir_all(&m1).unwrap();
        std::fs::create_dir_all(&m2).unwrap();
        let root_str = tmp.to_string_lossy().to_string();
        let s = format!("(project/set-root \"{}\")", root_str);
        scripting::eval(&s);

        let m1s = m1.to_string_lossy().to_string();
        let m2s = m2.to_string_lossy().to_string();
        let expr = format!(
            r#"(project/set-workspace-members [{{:name "sub-a" :root "{}"}} {{:name "sub-b" :root "{}"}}])"#,
            m1s, m2s
        );
        let r = scripting::eval(&expr);
        assert_eq!(r, "ok");

        let ws = ed.project_manager.project.workspace.as_ref().expect("workspace must be set");
        assert_eq!(ws.members.len(), 2);
        assert_eq!(ws.members[0].name, "sub-a");
        assert_eq!(ws.members[1].name, "sub-b");

        std::fs::remove_dir_all(&tmp).ok();
    });
}

#[test]
fn project_set_current_member_fires_event() {
    janet_test!(ed, {
        let tmp = std::env::temp_dir().join("sprint4_janet_member");
        let m1 = tmp.join("core");
        std::fs::create_dir_all(&m1).unwrap();
        let root_str = tmp.to_string_lossy().to_string();
        let s = format!("(project/set-root \"{}\")", root_str);
        scripting::eval(&s);

        let m1s = m1.to_string_lossy().to_string();
        let expr = format!(
            r#"(project/set-workspace-members [{{:name "core" :root "{}"}}])"#,
            m1s
        );
        scripting::eval(&expr);

        let event_count: std::sync::Arc<std::sync::Mutex<i32>> =
            std::sync::Arc::new(std::sync::Mutex::new(0));
        let cnt = event_count.clone();
        ed.events.on("project-member-focused", move |_data| {
            *cnt.lock().unwrap() += 1;
            None
        });

        let r = scripting::eval(r#"(project/set-current-member "core")"#);
        assert_eq!(r, "ok");
        ed.events.drain_and_dispatch();

        let count = *event_count.lock().unwrap();
        assert!(count > 0, "project-member-focused event must fire");

        std::fs::remove_dir_all(&tmp).ok();
    });
}
