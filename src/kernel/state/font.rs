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

impl FontConfig {
    /// Install the bundled JetBrains Mono when no font has been loaded
    /// (user config or Janet) so the GUI never falls back to egui's
    /// generic fonts.  Sets `changed` on the caller's behalf is NOT done
    /// here — the caller flips `font_changed` so the renderers pick it up.
    pub fn bootstrap_bundled(&mut self) {
        if !self.loaded_fonts.is_empty() {
            return;
        }
        self.loaded_fonts.insert(
            "magma-mono".to_string(),
            include_bytes!("../../../assets/fonts/JetBrainsMono-Regular.ttf").to_vec(),
        );
        self.loaded_fonts.insert(
            "magma-mono-bold".to_string(),
            include_bytes!("../../../assets/fonts/JetBrainsMono-Bold.ttf").to_vec(),
        );
        self.family = "magma-mono".to_string();
    }
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
