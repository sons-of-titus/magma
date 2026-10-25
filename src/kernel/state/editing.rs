/// Editing-session state: the unnamed register, macro recording, the change
/// list, and the last character motion.  Second slice of the R-24 Editor
/// consolidation (see `SearchState`): related fields move behind one named
/// sub-struct.
#[derive(Debug, Clone, Default)]
pub struct EditingState {
    /// Contents of the unnamed register — what `p`/`P` paste.
    pub yanked_text: Option<String>,
    /// True while the register keystroke after `"` is still being collected.
    pub insert_pending_register: bool,
    /// Cursor offset where the last insert session began (`.`/`gi`).
    pub last_insert_pos: usize,
    /// Register name while a macro is being recorded.
    pub recording_macro: Option<String>,
    /// Recorded macros: register name → the keys.
    pub macros: std::collections::HashMap<String, Vec<String>>,
    /// Cursor positions the buffer has visited (`g;`/`g,`).
    pub change_list: Vec<usize>,
    /// Cursor into `change_list` for `g;`/`g,`.
    pub change_list_idx: usize,
    /// Last `f`/`F`/`t`/`T` motion: (target char, forward, till).
    pub last_find: Option<(char, bool, bool)>,
}
