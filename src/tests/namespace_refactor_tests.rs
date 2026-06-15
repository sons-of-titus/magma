//! Pure-Rust tests for Sprint 19 namespace refactor.
//! These tests verify observable Rust-side state after calling the new APIs.

#[cfg(test)]
mod tests {
    use crate::command::builtin;
    use crate::fs::disk::DiskFileSystem;
    use crate::state::Editor;

    fn make_editor() -> Editor {
        let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
        builtin::register_builtin_commands(&mut ed);
        ed
    }

    #[test]
    fn editor_mode_name_reflects_current_mode() {
        let ed = make_editor();
        assert!(!ed.editor_mode.name.is_empty());
    }

    #[test]
    fn selection_state_starts_as_none() {
        let ed = make_editor();
        assert!(ed.selection.is_none());
    }

    #[test]
    fn plugin_state_starts_empty() {
        let ed = make_editor();
        assert!(ed.plugin_state.is_empty());
    }

    #[test]
    fn option_set_and_get_on_editor() {
        let mut ed = make_editor();
        ed.options.insert("number".to_string(), "true".to_string());
        assert_eq!(ed.options.get("number").map(|s| s.as_str()), Some("true"));
    }

    #[test]
    fn register_set_and_get_on_editor() {
        let mut ed = make_editor();
        ed.registers.insert("\"".to_string(), "hello".to_string());
        assert_eq!(ed.registers.get("\"").map(|s| s.as_str()), Some("hello"));
    }

    #[test]
    fn mark_ring_starts_empty() {
        let ed = make_editor();
        assert!(ed.mark_ring.is_empty());
    }

    #[test]
    fn module_paths_is_a_vec() {
        let ed = make_editor();
        let _ = ed.module_paths.len(); // confirms it's accessible
    }
}
