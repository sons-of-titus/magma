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

/// Alias the bundled JetBrains Mono pair is registered under.
pub const BUNDLED_ALIAS: &str = "magma-mono";
/// Alias the preferred system monospace font is registered under.
pub const SYSTEM_MONO_ALIAS: &str = "system-mono";

/// Ordered candidate paths for the default editor face.  Nerd Font variants
/// come first — their single-width icon glyphs (powerline, devicons, file
/// icons) render inside the editor grid — then the plain system monospace,
/// then portable fallbacks so the default is sensible beyond macOS.
pub fn default_font_candidates() -> Vec<std::path::PathBuf> {
    let mut paths: Vec<std::path::PathBuf> = Vec::new();
    if let Some(home) = std::env::var_os("HOME") {
        let fonts = std::path::PathBuf::from(home).join("Library/Fonts");
        paths.push(fonts.join("MonacoNerdFontMono-Regular.ttf"));
        paths.push(fonts.join("MonacoNerdFont-Regular.ttf"));
        paths.push(fonts.join("JetBrainsMonoNerdFontMono-Regular.ttf"));
        paths.push(fonts.join("Monaco.ttf"));
    }
    for p in [
        "/System/Library/Fonts/Monaco.ttf",
        "/Library/Fonts/Monaco.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
        "C:/Windows/Fonts/consola.ttf",
    ] {
        paths.push(std::path::PathBuf::from(p));
    }
    paths
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
            BUNDLED_ALIAS.to_string(),
            include_bytes!("../../../assets/fonts/JetBrainsMono-Regular.ttf").to_vec(),
        );
        self.loaded_fonts.insert(
            format!("{BUNDLED_ALIAS}-bold"),
            include_bytes!("../../../assets/fonts/JetBrainsMono-Bold.ttf").to_vec(),
        );
        self.family = BUNDLED_ALIAS.to_string();
    }

    /// Load the first readable default font (a Nerd Font Monaco when
    /// installed, else the plain system monospace) and make it the primary
    /// family.  Returns `true` when one was loaded.
    pub fn load_system_mono(&mut self) -> bool {
        for path in default_font_candidates() {
            if let Ok(bytes) = std::fs::read(&path)
                && bytes.len() > 1000
            {
                self.loaded_fonts.insert(SYSTEM_MONO_ALIAS.to_string(), bytes);
                self.family = SYSTEM_MONO_ALIAS.to_string();
                return true;
            }
        }
        false
    }

    /// Default editor font: prefer a system monospace (Monaco on macOS) and
    /// fall back to the bundled JetBrains Mono.  No-op when a font was
    /// already loaded from config or Janet, so an explicit choice wins.
    pub fn bootstrap_default_font(&mut self) {
        if !self.loaded_fonts.is_empty() {
            return;
        }
        if !self.load_system_mono() {
            self.bootstrap_bundled();
        }
    }

    /// Bytes of the primary family, when it names a loaded font.  The GPU
    /// atlas must rasterize exactly this face, so resolution goes through
    /// the family alias rather than an arbitrary registry entry.
    pub fn primary_font(&self) -> Option<&Vec<u8>> {
        self.loaded_fonts.get(&self.family)
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
