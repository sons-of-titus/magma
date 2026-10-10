//! Font/theme helpers for the egui GUI backend.

use std::sync::Arc;

use eframe::egui;

/// Bundled Nerd Font symbols — the fallback of last resort behind both
/// families, so icon-pack glyphs render everywhere without replacing the
/// user's text fonts.  Kept out of `FontConfig.loaded_fonts` on purpose:
/// the GPU glyph atlas rasterizes the first entry there and must not see
/// a symbols-only font.
pub const ICON_FONT_ALIAS: &str = "magma-icons";
pub const ICON_FONT: &[u8] =
    include_bytes!("../../../assets/fonts/SymbolsNerdFont-Regular.ttf");

/// egui font definitions: egui defaults + user-loaded fonts + the bundled
/// icon font.  The GUI and the glyph-coverage tests must agree on this.
pub fn font_definitions(font_config: &crate::kernel::state::FontConfig) -> egui::FontDefinitions {
    let mut font_defs = egui::FontDefinitions::default();
    for (alias, bytes) in &font_config.loaded_fonts {
        font_defs
            .font_data
            .insert(alias.clone(), Arc::new(egui::FontData::from_owned(bytes.clone())));
    }
    if !font_defs.font_data.contains_key(ICON_FONT_ALIAS) {
        font_defs.font_data.insert(
            ICON_FONT_ALIAS.to_string(),
            Arc::new(egui::FontData::from_owned(ICON_FONT.to_vec())),
        );
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
pub fn apply_fonts_to_egui(ctx: &egui::Context, font_config: &crate::kernel::state::FontConfig) {
    ctx.set_fonts(font_definitions(font_config));
}
