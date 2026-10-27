//! Janet C function for applying LSP WorkspaceEdit JSON.
//! The apply logic lives in `semantic::edit_apply` (shared with the
//! kernel's code-action and applyEdit paths).

use evil_janet::*;
use super::conv;
use super::with_editor;

/// (lsp/apply-edit edit-json) → nil
unsafe extern "C-unwind" fn c_lsp_apply_edit(argc: i32, argv: *mut Janet) -> Janet { unsafe {
    let edit_json = match conv::get_str(argc, argv, 0) {
        Some(j) => j,
        None => return conv::nil(),
    };

    with_editor(|ed| {
        let json: serde_json::Value = match serde_json::from_str(&edit_json) {
            Ok(v) => v,
            Err(e) => {
                debug!("lsp/apply-edit: invalid JSON: {e}");
                return conv::nil();
            }
        };
        kernel::semantic::edit_apply::apply_workspace_edit(ed, &json);
        conv::nil()
    })
}}

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"lsp/apply-edit".as_ptr() as *const _,
            cfun: Some(c_lsp_apply_edit as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Apply a WorkspaceEdit JSON object to open buffers".as_ptr() as *const _,
        },
    ]
}
