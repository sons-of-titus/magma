//! Font/theme helpers for the egui GUI backend.

use std::sync::{Arc, OnceLock};

use eframe::egui;

/// Bundled Nerd Font symbols — the fallback of last resort behind both
/// families, so icon-pack glyphs render everywhere without replacing the
/// user's text fonts.  Kept out of `FontConfig.loaded_fonts` on purpose:
/// the GPU glyph atlas rasterizes the first entry there and must not see
/// a symbols-only font.
pub const ICON_FONT_ALIAS: &str = "magma-icons";
pub const ICON_FONT: &[u8] = include_bytes!("../../../../assets/fonts/SymbolsNerdFont-Regular.ttf");

/// Alias the preferred system UI sans is registered under.
pub const UI_FONT_ALIAS: &str = "magma-ui";

/// Candidate system sans faces for chrome text, most-preferred first.  Plain
/// TrueType/OpenType only — collections (`.ttc`) are skipped because egui's
/// parser does not accept them.
fn ui_font_candidates() -> Vec<std::path::PathBuf> {
    let mut paths: Vec<std::path::PathBuf> = Vec::new();
    if let Some(home) = std::env::var_os("HOME") {
        let fonts = std::path::PathBuf::from(home).join("Library/Fonts");
        paths.push(fonts.join("Inter-Regular.ttf"));
        paths.push(fonts.join("Inter.ttf"));
    }
    for p in [
        "/System/Library/Fonts/Supplemental/Arial.ttf",
        "/System/Library/Fonts/Supplemental/Helvetica.ttf",
        "/usr/share/fonts/truetype/inter/Inter-Regular.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        "C:/Windows/Fonts/segoeui.ttf",
    ] {
        paths.push(std::path::PathBuf::from(p));
    }
    paths
}

/// Load (once) and validate a system sans so chrome text renders in a real
/// UI face, distinct from the mono editor font.  Returns `None` when no
/// candidate parses — egui's default sans stands in.
fn system_ui_font() -> Option<&'static Vec<u8>> {
    static CACHE: OnceLock<Option<Vec<u8>>> = OnceLock::new();
    CACHE
        .get_or_init(|| {
            for path in ui_font_candidates() {
                let Ok(bytes) = std::fs::read(&path) else {
                    continue;
                };
                if bytes.len() < 1000 {
                    continue;
                }
                // Only hand egui bytes that ab_glyph accepts — a bad face
                // panics inside egui's font builder.
                if ab_glyph::FontArc::try_from_vec(bytes.clone()).is_ok() {
                    return Some(bytes);
                }
            }
            None
        })
        .as_ref()
}

/// egui font definitions: egui defaults + user-loaded fonts + a system UI
/// sans for `Proportional` + the bundled icon font.  The GUI and the
/// glyph-coverage tests must agree on this.
pub fn font_definitions(font_config: &magma::kernel::state::FontConfig) -> egui::FontDefinitions {
    let mut font_defs = egui::FontDefinitions::default();
    for (alias, bytes) in &font_config.loaded_fonts {
        font_defs.font_data.insert(
            alias.clone(),
            Arc::new(egui::FontData::from_owned(bytes.clone())),
        );
    }
    if !font_defs.font_data.contains_key(ICON_FONT_ALIAS) {
        font_defs.font_data.insert(
            ICON_FONT_ALIAS.to_string(),
            Arc::new(egui::FontData::from_owned(ICON_FONT.to_vec())),
        );
    }
    // A real UI sans leads the proportional chain so chrome text is not the
    // mono editor face.
    if let Some(bytes) = system_ui_font() {
        if !font_defs.font_data.contains_key(UI_FONT_ALIAS) {
            font_defs.font_data.insert(
                UI_FONT_ALIAS.to_string(),
                Arc::new(egui::FontData::from_owned(bytes.clone())),
            );
        }
        let prop = font_defs
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default();
        if !prop.iter().any(|name| name == UI_FONT_ALIAS) {
            prop.insert(0, UI_FONT_ALIAS.to_string());
        }
    }
    let mut mono = Vec::new();
    if font_config.family != "Monospace" && font_config.family != "Proportional" {
        mono.push(font_config.family.clone());
    }
    for alias in &font_config.fallback {
        mono.push(alias.clone());
    }
    if !mono.is_empty() {
        let existing = font_defs
            .families
            .entry(egui::FontFamily::Monospace)
            .or_default();
        let old = std::mem::take(existing);
        mono.extend(old);
        *existing = mono;
    }
    for family in [egui::FontFamily::Monospace, egui::FontFamily::Proportional] {
        let chain = font_defs.families.entry(family).or_default();
        if !chain.iter().any(|name| name == ICON_FONT_ALIAS) {
            chain.push(ICON_FONT_ALIAS.to_string());
        }
    }
    font_defs
}

/// Apply `FontConfig` to egui: registers loaded font bytes and rebuilds families.
pub fn apply_fonts_to_egui(ctx: &egui::Context, font_config: &magma::kernel::state::FontConfig) {
    ctx.set_fonts(font_definitions(font_config));
}
