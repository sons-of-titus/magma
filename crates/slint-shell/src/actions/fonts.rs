//! Font application and persistence.  Font choices are stored as options
//! (`font.*`) so they ride the same settings persistence as everything else,
//! and are re-applied from those options before the first frame.

use crate::{AppState, AppWindow};
use kernel::state::persist;
use slint::ComponentHandle;

pub(crate) struct FontChanges {
    pub app_font_name: String,
    pub app_font_size: f32,
    pub editor_font_name: String,
    pub editor_font_size: f32,
    pub ligatures: bool,
}

/// Font settings shared by the settings page rows (applied on change) and
/// the legacy apply-font-settings callback.
pub(crate) fn apply_fonts(weak: &slint::Weak<AppWindow>, s: &mut AppState, f: FontChanges) {
    s.app_font_name = f.app_font_name.clone();
    s.app_font_size = f.app_font_size;
    if let Some(a) = weak.upgrade() {
        let g = a.global::<crate::Theme>();
        g.set_font_ui(f.app_font_size);
        g.set_font_small((f.app_font_size - 1.0).max(8.0));
        a.set_app_font_name(s.app_font_name.clone().into());
        a.set_app_font_size(f.app_font_size);
    }

    if !f.editor_font_name.trim().is_empty() {
        if !s
            .editor
            .font_config
            .loaded_fonts
            .contains_key(&f.editor_font_name)
        {
            if let Some(bytes) = load_system_font(&f.editor_font_name) {
                s.editor
                    .font_config
                    .loaded_fonts
                    .insert(f.editor_font_name.clone(), bytes);
            }
        }
        s.editor.font_config.family = f.editor_font_name;
    }
    s.editor.font_config.size = f.editor_font_size;
    s.editor.font_config.ligatures = f.ligatures;
    s.editor.font_changed = true;

    s.editor
        .options
        .insert("font.app-family".into(), f.app_font_name);
    s.editor
        .options
        .insert("font.app-size".into(), f.app_font_size.to_string());
    let family = s.editor.font_config.family.clone();
    s.editor.options.insert("font.family".into(), family);
    s.editor
        .options
        .insert("font.size".into(), f.editor_font_size.to_string());
    s.editor
        .options
        .insert("font.ligatures".into(), f.ligatures.to_string());
    persist::save_settings(&s.editor);

    if let Some(bytes) = s.editor.font_config.primary_font().map(|b| b.clone()) {
        let (cw, ch) = crate::pane::measure_cell(&bytes, f.editor_font_size);
        s.cell_w = cw.max(1);
        s.cell_h = ch.max(1);
        let (inset, beam) = crate::pane::caret_metrics_fallback(&bytes, f.editor_font_size);
        s.caret_inset = inset;
        s.caret_height = beam;
    }

    s.editor.frame_dirty = true;
    crate::sync::refresh(weak, s);
}

/// Re-apply persisted font options at boot, before the first frame.  A
/// no-op when nothing was persisted — defaults were already bootstrapped.
pub(crate) fn restore_persisted_fonts(weak: &slint::Weak<AppWindow>, s: &mut AppState) {
    let has_any = ["font.app-family", "font.app-size", "font.family", "font.size", "font.ligatures"]
        .iter()
        .any(|k| s.editor.options.contains_key(*k));
    if !has_any {
        return;
    }
    let options = &s.editor.options;
    let f = FontChanges {
        app_font_name: options
            .get("font.app-family")
            .cloned()
            .unwrap_or_else(|| s.app_font_name.clone()),
        app_font_size: options
            .get("font.app-size")
            .and_then(|v| v.parse().ok())
            .unwrap_or(s.app_font_size),
        editor_font_name: options
            .get("font.family")
            .cloned()
            .unwrap_or_else(|| s.editor.font_config.family.clone()),
        editor_font_size: options
            .get("font.size")
            .and_then(|v| v.parse().ok())
            .unwrap_or(s.editor.font_config.size),
        ligatures: options
            .get("font.ligatures")
            .map(|v| v == "true")
            .unwrap_or(s.editor.font_config.ligatures),
    };
    apply_fonts(weak, s, f);
}

fn load_system_font(family: &str) -> Option<Vec<u8>> {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    for face in db.faces() {
        if face.families.iter().any(|(n, _)| n == family) {
            if let Some((src, _)) = db.face_source(face.id) {
                match src {
                    fontdb::Source::File(path) => {
                        if let Ok(bytes) = std::fs::read(path)
                            && bytes.len() > 1000
                        {
                            return Some(bytes);
                        }
                    }
                    fontdb::Source::Binary(data) => {
                        return Some(data.as_ref().as_ref().to_vec());
                    }
                    fontdb::Source::SharedFile(_, data) => {
                        return Some(data.as_ref().as_ref().to_vec());
                    }
                }
            }
        }
    }
    None
}

pub(crate) fn collect_available_fonts(
    editor: &kernel::state::Editor,
    app_font_name: &str,
) -> Vec<String> {
    let mut names: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    for face in db.faces() {
        if let Some(fam) = face.families.first() {
            names.insert(fam.0.clone());
        }
    }
    // Include currently configured names.
    names.insert(editor.font_config.family.clone());
    if !app_font_name.is_empty() {
        names.insert(app_font_name.to_string());
    }
    names.into_iter().collect()
}
