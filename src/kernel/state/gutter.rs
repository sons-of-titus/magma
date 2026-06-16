/// A per-line gutter indicator set by an extension.
#[derive(Debug, Clone)]
pub struct GutterSign {
    pub face: String,
    /// Display text for this sign (may be multi-char, e.g. "● " or "▎").
    pub text: String,
    pub priority: i32,
}

/// A named column in the gutter, registered by Janet extensions.
#[derive(Debug, Clone)]
pub struct GutterColumn {
    pub name: String,
    /// Display width in cells.  0 means dynamic (used by `:line-numbers`).
    pub width: usize,
    pub visible: bool,
    pub face: String,
}

/// Icons used by the `:folding` gutter column.
#[derive(Debug, Clone)]
pub struct FoldIcons {
    pub open: String,
    pub closed: String,
    pub face: String,
}

impl Default for FoldIcons {
    fn default() -> Self {
        FoldIcons {
            open: "▾".to_string(),
            closed: "▸".to_string(),
            face: "fold-face".to_string(),
        }
    }
}

/// All gutter display state: columns, signs, line-number format, fold icons.
pub struct GutterState {
    pub columns: Vec<GutterColumn>,
    pub signs: std::collections::HashMap<usize, std::collections::HashMap<usize, Vec<GutterSign>>>,
    pub column_signs: std::collections::HashMap<(String, usize), std::collections::HashMap<usize, Vec<GutterSign>>>,
    pub line_number_fn: Option<String>,
    pub fold_icons: FoldIcons,
}

impl Default for GutterState {
    fn default() -> Self {
        GutterState {
            columns: Vec::new(),
            signs: std::collections::HashMap::new(),
            column_signs: std::collections::HashMap::new(),
            line_number_fn: None,
            fold_icons: FoldIcons::default(),
        }
    }
}
