//! Janet API — project registry: recent projects, multi-project registry, buffer-project mapping,
//! path existence checks, and raw file reads.

use std::collections::HashMap;
use std::path::PathBuf;

use evil_janet::*;
use super::conv;
use super::with_editor;

fn projects_file_path() -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    format!("{}/.local/share/magma/projects", home)
}

fn ensure_projects_dir() {
    let path = projects_file_path();
    if let Some(parent) = std::path::Path::new(&path).parent() {
        std::fs::create_dir_all(parent).ok();
    }
}

fn save_recent_projects(projects: &[String]) {
    ensure_projects_dir();
    let path = projects_file_path();
    let content = projects.join("\n");
    std::fs::write(&path, &content).ok();
}

/// (project/recent) → [string ...]
unsafe extern "C-unwind" fn c_project_recent(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let arr = janet_wrap_array(janet_array(ed.project_manager.recent_projects.len() as i32));
        for p in &ed.project_manager.recent_projects {
            janet_array_push(janet_unwrap_array(arr), conv::string(p));
        }
        arr
    })
}

/// (project/push-recent path) → nil
unsafe extern "C-unwind" fn c_project_push_recent(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let path = match conv::get_str(argc, argv, 0) {
            Some(s) => s,
            None => return conv::nil(),
        };
        ed.project_manager.recent_projects.retain(|p| p != &path);
        ed.project_manager.recent_projects.insert(0, path);
        ed.project_manager.recent_projects.truncate(50);
        save_recent_projects(&ed.project_manager.recent_projects);
        conv::nil()
    })
}

/// (project/path-exists? path) → boolean
unsafe extern "C-unwind" fn c_project_path_exists(argc: i32, argv: *mut Janet) -> Janet { unsafe {
    let path_str = match conv::get_str(argc, argv, 0) {
        Some(s) => s,
        None => return conv::nil(),
    };
    conv::boolean(std::path::Path::new(&path_str).exists())
}}

/// (project/fs-read path) → string or nil
unsafe extern "C-unwind" fn c_project_fs_read(argc: i32, argv: *mut Janet) -> Janet { unsafe {
    let path_str = match conv::get_str(argc, argv, 0) {
        Some(s) => s,
        None => return conv::nil(),
    };
    match std::fs::read_to_string(&path_str) {
        Ok(content) => conv::string(&content),
        Err(_) => conv::nil(),
    }
}}

/// (project/buffer-set-project buf project-name) → nil
unsafe extern "C-unwind" fn c_project_buffer_set_project(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let buf_key = match conv::get_int(argc, argv, 0) {
            Some(i) => i as usize,
            None => return conv::nil(),
        };
        let project_name = match conv::get_str(argc, argv, 1) {
            Some(s) => s,
            None => return conv::nil(),
        };
        if ed.buffers.contains(buf_key) {
            ed.project_manager.buffer_projects.insert(buf_key, project_name);
        }
        conv::nil()
    })
}

/// (project/buffer-project buf) → string or nil
unsafe extern "C-unwind" fn c_project_buffer_project(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let buf_key = match conv::get_int(argc, argv, 0) {
            Some(i) => i as usize,
            None => return conv::nil(),
        };
        match ed.project_manager.buffer_projects.get(&buf_key) {
            Some(name) => conv::string(name),
            None => conv::nil(),
        }
    })
}

/// (project/register name root) → nil
unsafe extern "C-unwind" fn c_project_register(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let name = match conv::get_str(argc, argv, 0) {
            Some(s) => s,
            None => return conv::nil(),
        };
        let root_str = match conv::get_str(argc, argv, 1) {
            Some(s) => s,
            None => return conv::nil(),
        };
        let ps = crate::state::ProjectState {
            root: Some(PathBuf::from(&root_str)),
            name: Some(name.clone()),
            ..Default::default()
        };
        ed.project_manager.projects.insert(name, ps);
        conv::nil()
    })
}

/// (project/unregister name) → nil
unsafe extern "C-unwind" fn c_project_unregister(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let name = match conv::get_str(argc, argv, 0) {
            Some(s) => s,
            None => return conv::nil(),
        };
        ed.project_manager.projects.remove(&name);
        if ed.project_manager.current_project.as_deref() == Some(&name) {
            ed.project_manager.current_project = None;
            ed.project_manager.project = crate::state::ProjectState::default();
            let data = HashMap::new();
            ed.events.emit("project-closed", data);
        }
        conv::nil()
    })
}

/// (project/list) → [{:name "..." :root "..."} ...]
unsafe extern "C-unwind" fn c_project_list(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let arr = janet_wrap_array(janet_array(ed.project_manager.projects.len() as i32));
        for (name, proj) in &ed.project_manager.projects {
            let tbl = janet_wrap_table(janet_table(2));
            let t = janet_unwrap_table(tbl);
            janet_table_put(t, conv::keyword("name"), conv::string(name));
            let root_str = proj.root.as_ref()
                .map(|r| r.to_string_lossy().into_owned())
                .unwrap_or_default();
            janet_table_put(t, conv::keyword("root"), conv::string(&root_str));
            janet_array_push(janet_unwrap_array(arr), tbl);
        }
        arr
    })
}

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"project/recent".as_ptr() as *const _,
            cfun: Some(c_project_recent as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return recently opened project paths".as_ptr() as *const _,
        },
        JanetReg {
            name: c"project/push-recent".as_ptr() as *const _,
            cfun: Some(c_project_push_recent as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Add a project path to the recent list and persist".as_ptr() as *const _,
        },
        JanetReg {
            name: c"project/path-exists?".as_ptr() as *const _,
            cfun: Some(c_project_path_exists as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Check if a file or directory exists".as_ptr() as *const _,
        },
        JanetReg {
            name: c"project/fs-read".as_ptr() as *const _,
            cfun: Some(c_project_fs_read as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Read a file's contents as a string".as_ptr() as *const _,
        },
        JanetReg {
            name: c"project/buffer-set-project".as_ptr() as *const _,
            cfun: Some(c_project_buffer_set_project as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Associate a buffer with a project".as_ptr() as *const _,
        },
        JanetReg {
            name: c"project/buffer-project".as_ptr() as *const _,
            cfun: Some(c_project_buffer_project as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return the project name for a buffer".as_ptr() as *const _,
        },
        JanetReg {
            name: c"project/register".as_ptr() as *const _,
            cfun: Some(c_project_register as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Register a project in the multi-project registry".as_ptr() as *const _,
        },
        JanetReg {
            name: c"project/unregister".as_ptr() as *const _,
            cfun: Some(c_project_unregister as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Remove a project from the registry".as_ptr() as *const _,
        },
        JanetReg {
            name: c"project/list".as_ptr() as *const _,
            cfun: Some(c_project_list as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"List all registered projects".as_ptr() as *const _,
        },
    ]
}
