//! Janet C functions for workspace management (Sprint 4).
//! Registered as `extern "C-unwind"` functions via evil-janet.

use std::path::PathBuf;

use evil_janet::*;
use super::conv;
use super::with_editor;
use crate::kernel::event::payload::*;
use crate::kernel::event::keys;

/// (project/workspace?) → {:root "..." :members [...]} or nil
///
/// Return the workspace descriptor, or nil if no workspace is configured.
unsafe extern "C-unwind" fn c_project_workspace(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let ws = match &ed.project_manager.project.workspace {
            Some(ws) => ws,
            None => return conv::nil(),
        };
        let tbl = janet_wrap_table(janet_table(2));
        let t = janet_unwrap_table(tbl);
        janet_table_put(t, conv::keyword("root"), conv::string(&ws.root.to_string_lossy()));
        let members_arr = janet_wrap_array(janet_array(ws.members.len() as i32));
        for m in &ws.members {
            let mtbl = janet_wrap_table(janet_table(2));
            let mt = janet_unwrap_table(mtbl);
            janet_table_put(mt, conv::keyword("name"), conv::string(&m.name));
            janet_table_put(mt, conv::keyword("root"), conv::string(&m.root.to_string_lossy()));
            janet_array_push(janet_unwrap_array(members_arr), mtbl);
        }
        janet_table_put(t, conv::keyword("members"), members_arr);
        tbl
    })
}

/// (project/workspace-members) → [{:name "..." :root "..."} ...]
///
/// Return an array of all workspace member subprojects.
unsafe extern "C-unwind" fn c_project_workspace_members(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let members = match &ed.project_manager.project.workspace {
            Some(ws) => &ws.members,
            None => {
                let arr = janet_wrap_array(janet_array(0));
                return arr;
            }
        };
        let arr = janet_wrap_array(janet_array(members.len() as i32));
        for m in members {
            let tbl = janet_wrap_table(janet_table(2));
            let t = janet_unwrap_table(tbl);
            janet_table_put(t, conv::keyword("name"), conv::string(&m.name));
            janet_table_put(t, conv::keyword("root"), conv::string(&m.root.to_string_lossy()));
            janet_array_push(janet_unwrap_array(arr), tbl);
        }
        arr
    })
}

/// (project/set-workspace-members [{:name "..." :root "..."} ...]) → nil
///
/// Set the workspace member list for the current project.
unsafe extern "C-unwind" fn c_project_set_workspace_members(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        if argc < 1 {
            return conv::nil();
        }
        let members_val = *argv.add(0);
        if janet_checktype(members_val, JanetType_JANET_ARRAY) == 0
            && janet_checktype(members_val, JanetType_JANET_TUPLE) == 0
        {
            return conv::nil();
        }
        let len = janet_length(members_val);
        let mut members = Vec::new();
        for i in 0..len {
            let elem = janet_in(members_val, janet_wrap_number(i as f64));
            if janet_checktype(elem, JanetType_JANET_TABLE) == 0
                && janet_checktype(elem, JanetType_JANET_STRUCT) == 0
            {
                continue;
            }
            let name_ptr = janet_in(elem, conv::keyword("name"));
            let root_ptr = janet_in(elem, conv::keyword("root"));
            let name = if janet_checktype(name_ptr, JanetType_JANET_STRING) != 0 {
                let ptr = janet_unwrap_string(name_ptr);
                if ptr.is_null() { continue; }
                std::ffi::CStr::from_ptr(ptr as *const i8).to_string_lossy().into_owned()
            } else { continue; };
            let root_str = if janet_checktype(root_ptr, JanetType_JANET_STRING) != 0 {
                let ptr = janet_unwrap_string(root_ptr);
                if ptr.is_null() { continue; }
                std::ffi::CStr::from_ptr(ptr as *const i8).to_string_lossy().into_owned()
            } else { continue; };
            members.push(crate::kernel::state::ProjectMember {
                name,
                root: PathBuf::from(root_str),
            });
        }
        let root = ed.project_manager.project.root.clone().unwrap_or_default();
        ed.project_manager.project.workspace = Some(crate::kernel::state::Workspace { root, members });
        conv::nil()
    })
}

/// (project/set-current-member name) → nil
///
/// Switch the active subproject within a workspace.  Emits
/// `project-member-focused`.
unsafe extern "C-unwind" fn c_project_set_current_member(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let name = match conv::get_str(argc, argv, 0) {
            Some(s) => s,
            None => return conv::nil(),
        };
        let ws = match &ed.project_manager.project.workspace {
            Some(ws) => ws.clone(),
            None => return conv::nil(),
        };
        // Find the member by name
        let mut found = false;
        let mut member_root = PathBuf::new();
        for m in &ws.members {
            if m.name == name {
                found = true;
                member_root = m.root.clone();
                break;
            }
        }
        if !found {
            return conv::nil();
        }
        let project_name = ed.project_manager.project.name.clone().unwrap_or_default();
        // Save current project state to the registry
        if let Some(ref cur) = ed.project_manager.current_project.clone() {
            ed.project_manager.projects.insert(cur.clone(), ed.project_manager.project.clone());
        }
        // Load the member project state from registry or create fresh
        let member_state = ed.project_manager.projects.get(&name).cloned().unwrap_or_else(|| {
            crate::kernel::project::Project {
                root: Some(member_root.clone()),
                name: Some(name.clone()),
                ..Default::default()
            }
        });
        ed.project_manager.project = member_state;
        ed.project_manager.current_project = Some(name.clone());
        ed.project_manager.projects.insert(name.clone(), ed.project_manager.project.clone());
        ed.events.emit_typed(keys::events::PROJECT_MEMBER_FOCUSED, ProjectMemberFocusedPayload {
            name,
            root: member_root.to_string_lossy().into_owned(),
            project_name,
        });
        conv::nil()
    })
}

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"project/workspace?".as_ptr() as *const _,
            cfun: Some(c_project_workspace as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return the workspace descriptor or nil".as_ptr() as *const _,
        },
        JanetReg {
            name: c"project/workspace-members".as_ptr() as *const _,
            cfun: Some(c_project_workspace_members as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return workspace member subprojects".as_ptr() as *const _,
        },
        JanetReg {
            name: c"project/set-workspace-members".as_ptr() as *const _,
            cfun: Some(c_project_set_workspace_members as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Set the workspace member list".as_ptr() as *const _,
        },
        JanetReg {
            name: c"project/set-current-member".as_ptr() as *const _,
            cfun: Some(c_project_set_current_member as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Switch the active workspace member".as_ptr() as *const _,
        },
    ]
}
