/// Per-context font override (e.g. "prose", "code", "status").
#[derive(Debug, Clone)]
pub struct ContextFont {
    pub family: Option<String>,
    pub size: Option<f32>,
}

/// Font configuration managed by Janet and read by both renderers.
#[derive(Debug, Clone)]
pub struct FontConfig {
    pub family: String,
    pub size: f32,
    pub weight: u16,
    pub fallback: Vec<String>,
    pub glyph_widths: std::collections::HashMap<char, u8>,
    pub ligatures: bool,
    pub context_overrides: std::collections::HashMap<String, ContextFont>,
    pub loaded_fonts: std::collections::HashMap<String, Vec<u8>>,
}

impl Default for FontConfig {
    fn default() -> Self {
        FontConfig {
            family: "Monospace".to_string(),
            size: 15.0,
            weight: 400,
            fallback: Vec::new(),
            glyph_widths: std::collections::HashMap::new(),
            ligatures: false,
            context_overrides: std::collections::HashMap::new(),
            loaded_fonts: std::collections::HashMap::new(),
        }
    }
}
