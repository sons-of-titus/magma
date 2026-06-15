//! Janet API — project core: root, name, files, index-files, per-project options.

use std::collections::HashMap;
use std::path::PathBuf;

use evil_janet::*;
use super::conv;
use super::with_editor;

fn walk_dir(dir: &std::path::Path) -> std::io::Result<Vec<String>> {
    let mut files = Vec::new();
    if dir.is_dir() {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                if name.starts_with('.') || name == "node_modules" || name == "target" {
                    continue;
                }
                if let Ok(mut sub) = walk_dir(&path) {
                    files.append(&mut sub);
                }
            } else if path.is_file()
                && let Some(s) = path.to_str() {
                    files.push(s.to_string());
                }
        }
    }
    Ok(files)
}

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
        let path_str = match conv::get_str(argc, argv, 0) {
            Some(s) => s,
            None => {
                let was_active = ed.project_manager.project.root.is_some();
                ed.project_manager.project = crate::state::ProjectState::default();
                ed.project_manager.current_project = None;
                if was_active {
                    let data = HashMap::new();
                    ed.events.emit("project-closed", data);
                }
                return conv::nil();
            }
        };
        let path = PathBuf::from(&path_str);
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path_str.clone());
        ed.project_manager.project.root = Some(path.clone());
        ed.project_manager.project.name = Some(name.clone());
        ed.project_manager.project.files = Vec::new();
        ed.project_manager.project.file_index_dirty = true;
        ed.project_manager.current_project = Some(name.clone());
        ed.project_manager.projects.insert(name.clone(), ed.project_manager.project.clone());
        let mut data = HashMap::new();
        data.insert("root".to_string(), path_str);
        data.insert("name".to_string(), name);
        ed.events.emit("project-opened", data);
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
        let root = match &ed.project_manager.project.root {
            Some(r) => r.clone(),
            None => return conv::nil(),
        };
        let name = ed.project_manager.project.name.clone().unwrap_or_default();
        let bg = match ed.background.as_ref() {
            Some(bg) => bg.clone(),
            None => return conv::nil(),
        };
        let sender = bg.sender.clone();
        ed.project_manager.project.file_index_dirty = false;
        bg.spawn_blocking(move || {
            let mut files = Vec::new();
            if let Ok(entries) = walk_dir(&root) {
                files = entries;
            }
            let _ = sender.send(crate::runtime::BackgroundEvent::FileIndexed {
                project_name: name,
                files,
            });
        });
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
    ]
}
