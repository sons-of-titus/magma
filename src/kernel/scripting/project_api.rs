//! Janet API — project core: root, name, files, index-files, per-project options.

use evil_janet::*;
use super::conv;
use super::with_editor;
use crate::kernel::event::payload::*;
use crate::kernel::event::keys;
use crate::kernel::project::Project;

/// (project/root) → string or nil
unsafe extern "C-unwind" fn c_project_root(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| match &ed.project_manager.project.root {
        Some(p) => conv::string(&p.to_string_lossy()),
        None => conv::nil(),
    })
}

/// (project/set-root path) → nil
unsafe extern "C-unwind" fn c_project_set_root(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        match conv::get_str(argc, argv, 0) {
            Some(path) => crate::kernel::project::open_root(ed, &path),
            None => {
                let was_active = ed.project_manager.project.root.is_some();
                ed.project_manager.project = Project::default();
                ed.project_manager.current_project = None;
                if was_active {
                    ed.events.emit_typed(keys::events::PROJECT_CLOSED, EmptyPayload);
                }
            }
        }
        conv::nil()
    })
}

/// (project/name) → string or nil
unsafe extern "C-unwind" fn c_project_name(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| match &ed.project_manager.project.name {
        Some(n) => conv::string(n),
        None => conv::nil(),
    })
}

/// (project/set-name name) → nil
unsafe extern "C-unwind" fn c_project_set_name(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let name = match conv::get_str(argc, argv, 0) {
            Some(s) => s,
            None => return conv::nil(),
        };
        ed.project_manager.project.name = Some(name.clone());
        if let Some(ref cur) = ed.project_manager.current_project.clone()
            && let Some(proj) = ed.project_manager.projects.get_mut(cur) {
                proj.name = Some(name);
            }
        conv::nil()
    })
}

/// (project/files) → [string ...]
unsafe extern "C-unwind" fn c_project_files(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let files = &ed.project_manager.project.files;
        let arr = janet_wrap_array(janet_array(files.len() as i32));
        for f in files {
            janet_array_push(janet_unwrap_array(arr), conv::string(f));
        }
        arr
    })
}

/// (project/index-files) → nil
unsafe extern "C-unwind" fn c_project_index_files(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        crate::kernel::project::spawn_file_index(ed);
        conv::nil()
    })
}

/// (project/option-get key) → string or nil
unsafe extern "C-unwind" fn c_project_option_get(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let key = match conv::get_str(argc, argv, 0) {
            Some(s) => s,
            None => return conv::nil(),
        };
        match ed.project_manager.project.options.get(&key) {
            Some(val) => conv::string(val),
            None => conv::nil(),
        }
    })
}

/// (project/option-set key val) → nil
unsafe extern "C-unwind" fn c_project_option_set(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let key = match conv::get_str(argc, argv, 0) {
            Some(s) => s,
            None => return conv::nil(),
        };
        let val = match conv::get_str(argc, argv, 1) {
            Some(s) => s,
            None => return conv::nil(),
        };
        ed.project_manager.project.options.insert(key.clone(), val.clone());
        if let Some(ref cur) = ed.project_manager.current_project.clone()
            && let Some(proj) = ed.project_manager.projects.get_mut(cur) {
                proj.options.insert(key, val);
            }
        conv::nil()
    })
}

// ── Phase 5: Project Model API ────────────────────────────────────────────────

/// (project/current) → table or nil
///
/// Returns a table describing the active project:
///   `{:name :root :language :run-configs [name …] :active-config name-or-nil}`
/// Returns nil when no project is active.
unsafe extern "C-unwind" fn c_project_current(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let pm = &ed.project_manager;
        let proj = &pm.project;
        let root = match proj.root.as_ref() {
            Some(r) => r.to_string_lossy().into_owned(),
            None => return conv::nil(),
        };
        let name = proj.name.clone().unwrap_or_default();
        let tbl = janet_wrap_table(janet_table(6));
        let t = janet_unwrap_table(tbl);
        janet_table_put(t, conv::keyword("name"),     conv::string(&name));
        janet_table_put(t, conv::keyword("root"),     conv::string(&root));
        janet_table_put(t, conv::keyword("language"), conv::string(&proj.language));

        let rc_arr = janet_wrap_array(janet_array(pm.run_configs.configs.len() as i32));
        for cfg in &pm.run_configs.configs {
            janet_array_push(janet_unwrap_array(rc_arr), conv::string(&cfg.name));
        }
        janet_table_put(t, conv::keyword("run-configs"), rc_arr);

        let active = match &pm.run_configs.active {
            Some(a) => conv::string(a),
            None => conv::nil(),
        };
        janet_table_put(t, conv::keyword("active-config"), active);

        tbl
    })
}

/// (project/build) → task-id or signals error
unsafe extern "C-unwind" fn c_project_build(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let bg = match ed.background.clone() {
            Some(bg) => bg,
            None => conv::signal_err("project/build: no background runtime"),
        };
        match ed.project_manager.build(&mut ed.task_scheduler, &bg) {
            Ok(id) => {
                crate::kernel::run_config::announce_launch(ed, id);
                conv::integer(id as i32)
            }
            Err(e) => conv::signal_err(&e),
        }
    })
}

/// (project/test) → task-id or signals error
unsafe extern "C-unwind" fn c_project_test(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let bg = match ed.background.clone() {
            Some(bg) => bg,
            None => conv::signal_err("project/test: no background runtime"),
        };
        match ed.project_manager.test(&mut ed.task_scheduler, &bg) {
            Ok(id) => {
                crate::kernel::run_config::announce_launch(ed, id);
                conv::integer(id as i32)
            }
            Err(e) => conv::signal_err(&e),
        }
    })
}

/// (project/run) → task-id or signals error
unsafe extern "C-unwind" fn c_project_run(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let bg = match ed.background.clone() {
            Some(bg) => bg,
            None => conv::signal_err("project/run: no background runtime"),
        };
        match ed.project_manager.run(&mut ed.task_scheduler, &bg) {
            Ok(id) => {
                crate::kernel::run_config::announce_launch(ed, id);
                conv::integer(id as i32)
            }
            Err(e) => conv::signal_err(&e),
        }
    })
}

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"project/root".as_ptr() as *const _,
            cfun: Some(c_project_root as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return the project root path or nil".as_ptr() as *const _,
        },
        JanetReg {
            name: c"project/set-root".as_ptr() as *const _,
            cfun: Some(c_project_set_root as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Set the project root directory".as_ptr() as *const _,
        },
        JanetReg {
            name: c"project/name".as_ptr() as *const _,
            cfun: Some(c_project_name as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return the project name or nil".as_ptr() as *const _,
        },
        JanetReg {
            name: c"project/set-name".as_ptr() as *const _,
            cfun: Some(c_project_set_name as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Set the project name".as_ptr() as *const _,
        },
        JanetReg {
            name: c"project/files".as_ptr() as *const _,
            cfun: Some(c_project_files as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return the cached project file list".as_ptr() as *const _,
        },
        JanetReg {
            name: c"project/index-files".as_ptr() as *const _,
            cfun: Some(c_project_index_files as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Walk the project root and cache file list asynchronously".as_ptr() as *const _,
        },
        JanetReg {
            name: c"project/option-get".as_ptr() as *const _,
            cfun: Some(c_project_option_get as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Get a per-project option".as_ptr() as *const _,
        },
        JanetReg {
            name: c"project/option-set".as_ptr() as *const _,
            cfun: Some(c_project_option_set as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Set a per-project option".as_ptr() as *const _,
        },
        JanetReg {
            name: c"project/current".as_ptr() as *const _,
            cfun: Some(c_project_current as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return {:name :root :language :run-configs :active-config} for the active project, or nil".as_ptr() as *const _,
        },
        JanetReg {
            name: c"project/build".as_ptr() as *const _,
            cfun: Some(c_project_build as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Dispatch the project build target through the Task System; returns task-id".as_ptr() as *const _,
        },
        JanetReg {
            name: c"project/test".as_ptr() as *const _,
            cfun: Some(c_project_test as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Dispatch the project test target through the Task System; returns task-id".as_ptr() as *const _,
        },
        JanetReg {
            name: c"project/run".as_ptr() as *const _,
            cfun: Some(c_project_run as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Dispatch the project run target through the Task System; returns task-id".as_ptr() as *const _,
        },
    ]
}
