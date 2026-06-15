//! Font/theme helpers for the egui GUI backend.

use eframe::egui;

/// Map mode name to mode-indicator colour for the status bar pill.
pub fn mode_color(mode: &str) -> egui::Color32 {
    match mode {
        "INSERT"  => egui::Color32::from_rgb(80,  161, 79),
        "VISUAL"  => egui::Color32::from_rgb(180, 130, 30),
        "REPLACE" => egui::Color32::from_rgb(210, 60,  60),
        "COMMAND" => egui::Color32::from_rgb(140, 80,  200),
        _         => egui::Color32::from_rgb(0,   122, 204),
    }
}

/// Apply `FontConfig` to egui: registers loaded font bytes and rebuilds families.
pub fn apply_fonts_to_egui(ctx: &egui::Context, font_config: &crate::state::FontConfig) {
    if font_config.loaded_fonts.is_empty() && font_config.fallback.is_empty()
        && font_config.family == "Monospace"
    {
        return;
    }
    let mut font_defs = egui::FontDefinitions::default();
    for (alias, bytes) in &font_config.loaded_fonts {
        font_defs
            .font_data
            .insert(alias.clone(), egui::FontData::from_owned(bytes.clone()));
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
    ctx.set_fonts(font_defs);
}
