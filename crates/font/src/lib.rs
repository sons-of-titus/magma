//! Editor font configuration.
//!
//! `FontConfig` holds the primary family, size, weight, glyph widths,
//! ligature preference, per-context overrides, and the bytes of every loaded
//! face.  It also owns the loading policy: the bundled JetBrains Mono pair,
//! the preferred system monospace, and the ordered candidate list.  Both
//! renderers read it; Janet writes it.
//!
//! Nothing here draws or rasterizes — the GUI font registry and the GPU
//! atlas live in the kernel's render layer.

#[cfg(test)]
mod tests;

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
/// Alias the bundled Symbols Nerd Font is registered under.
pub const SYMBOLS_NERD_ALIAS: &str = "symbols-nerd";
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

/// Ordered candidate paths for fallback faces covering non-ASCII scripts
/// (Arabic, Hebrew, Armenian, Georgian, symbols, extended Latin/Cyrillic, CJK).
pub fn fallback_font_candidates() -> Vec<std::path::PathBuf> {
    [
        "/System/Library/Fonts/SFArabic.ttf",
        "/System/Library/Fonts/Supplemental/Courier New.ttf",
        "/System/Library/Fonts/Supplemental/Tahoma.ttf",
        "/System/Library/Fonts/SFHebrew.ttf",
        "/System/Library/Fonts/SFArmenian.ttf",
        "/System/Library/Fonts/SFGeorgian.ttf",
        "/System/Library/Fonts/Apple Symbols.ttf",
        "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
        "/System/Library/Fonts/Supplemental/AppleGothic.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/noto/NotoSansArabic-Regular.ttf",
        "/usr/share/fonts/truetype/noto/NotoSansHebrew-Regular.ttf",
        "/usr/share/fonts/truetype/freefont/FreeMono.ttf",
        "/usr/share/fonts/truetype/freefont/FreeSans.ttf",
        "C:/Windows/Fonts/cour.ttf",
        "C:/Windows/Fonts/tahoma.ttf",
        "C:/Windows/Fonts/arial.ttf",
        "C:/Windows/Fonts/segoeui.ttf",
    ]
    .into_iter()
    .map(std::path::PathBuf::from)
    .collect()
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
                self.loaded_fonts
                    .insert(SYSTEM_MONO_ALIAS.to_string(), bytes);
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
        self.load_system_fallbacks();
    }

    /// Populate `fallback` and `loaded_fonts` with bundled and system script
    /// fallback faces so glyphs missing from the primary monospace font
    /// (Arabic, Hebrew, extended Latin/Cyrillic, symbols, CJK) still render.
    pub fn load_system_fallbacks(&mut self) {
        if self.family != BUNDLED_ALIAS && !self.fallback.iter().any(|f| f == BUNDLED_ALIAS) {
            self.loaded_fonts
                .entry(BUNDLED_ALIAS.to_string())
                .or_insert_with(|| {
                    include_bytes!("../../../assets/fonts/JetBrainsMono-Regular.ttf").to_vec()
                });
            self.fallback.push(BUNDLED_ALIAS.to_string());
        }
        if !self.fallback.iter().any(|f| f == SYMBOLS_NERD_ALIAS) {
            self.loaded_fonts
                .entry(SYMBOLS_NERD_ALIAS.to_string())
                .or_insert_with(|| {
                    include_bytes!("../../../assets/fonts/SymbolsNerdFont-Regular.ttf").to_vec()
                });
            self.fallback.push(SYMBOLS_NERD_ALIAS.to_string());
        }
        for path in fallback_font_candidates() {
            let Some(name) = path.file_name().map(|n| n.to_string_lossy().into_owned()) else {
                continue;
            };
            let alias = format!("fallback:{name}");
            if self.fallback.iter().any(|f| f == &alias) {
                continue;
            }
            if let Ok(bytes) = std::fs::read(&path)
                && bytes.len() > 1000
            {
                self.loaded_fonts.insert(alias.clone(), bytes);
                self.fallback.push(alias);
            }
        }
    }

    /// Bytes of the primary family, when it names a loaded font.  The GPU
    /// atlas must rasterize exactly this face, so resolution goes through
    /// the family alias rather than an arbitrary registry entry.
    pub fn primary_font(&self) -> Option<&Vec<u8>> {
        self.loaded_fonts.get(&self.family)
    }

    /// Ordered byte slices of the primary font followed by every loaded
    /// fallback font.
    pub fn font_chain(&self) -> Vec<&[u8]> {
        let mut chain = Vec::with_capacity(1 + self.fallback.len());
        if let Some(primary) = self.primary_font() {
            chain.push(primary.as_slice());
        }
        for name in &self.fallback {
            if name != &self.family
                && let Some(bytes) = self.loaded_fonts.get(name)
            {
                chain.push(bytes.as_slice());
            }
        }
        chain
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
