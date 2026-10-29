//! Palette and editor-theme projection for the Slint shell.
//!
//! The chrome reads the Slint `Theme` global; the editor pane reads
//! `Editor.theme` and `Editor.faces`.  Both are pushed from the same palette
//! so the surface never clashes with the shell.

use kernel::render::surface::Style;
use kernel::state::Editor;
use slint::ComponentHandle;

use crate::{AppWindow, Theme, WelcomeWindow};

pub type Rgb = (u8, u8, u8);

pub struct Syntax {
    pub keyword: Rgb,
    pub string: Rgb,
    pub comment: Rgb,
    pub type_: Rgb,
    pub function: Rgb,
    pub variable: Rgb,
    pub constant: Rgb,
    pub operator: Rgb,
    pub punctuation: Rgb,
    pub error: Rgb,
    pub warning: Rgb,
}

pub struct Palette {
    pub chrome: Rgb,
    pub surface_panel: Rgb,
    pub surface: Rgb,
    pub fill_muted: Rgb,
    pub fill: Rgb,
    pub fill_strong: Rgb,
    pub text_faint: Rgb,
    pub text_muted: Rgb,
    pub text_secondary: Rgb,
    pub text_primary: Rgb,
    pub accent: Rgb,
    pub danger: Rgb,
    pub success: Rgb,
    pub accent_muted: Rgb,
    pub warning: Rgb,
    pub border: Rgb,
    pub tab_active: Rgb,
    pub tab_hover: Rgb,
    pub selection: Rgb,
    pub syntax: Syntax,
    pub terminal: TerminalPalette,
}

/// Console colors for the terminal dock: default foreground/background, the
/// block cursor, and the 16 ANSI slots.  Each theme carries its own set so the
/// PTY output never falls back to another frontend's palette.
pub struct TerminalPalette {
    pub fg: Rgb,
    pub bg: Rgb,
    pub cursor: Rgb,
    pub ansi: [Rgb; 16],
}

pub fn palette(name: &str) -> Palette {
    if let Some(p) = crate::noctis::palette(name) {
        return p;
    }
    if let Some(p) = crate::dracula::palette(name) {
        return p;
    }
    if let Some(p) = crate::tokyo_night::palette(name) {
        return p;
    }
    if let Some(p) = crate::catppuccin::palette(name) {
        return p;
    }
    if matches!(
        name,
        "monokai" | "monokai-dark" | "ayu-dark" | "ayu" | "ayu-light"
    ) {
        return crate::new_palettes::palette(name);
    }
    crate::palettes::palette(name)
}

/// Push a palette into one window's `Theme` global instance.  Each
/// top-level component carries its own instance of the global; the editor
/// window and the launcher must stay in lockstep.
fn apply_palette(g: Theme, p: &Palette, mono_family: &str) {
    g.set_chrome(color(p.chrome));
    g.set_surface_panel(color(p.surface_panel));
    g.set_surface(color(p.surface));
    g.set_fill_muted(color(p.fill_muted));
    g.set_fill(color(p.fill));
    g.set_fill_strong(color(p.fill_strong));
    g.set_text_faint(color(p.text_faint));
    g.set_text_muted(color(p.text_muted));
    g.set_text_secondary(color(p.text_secondary));
    g.set_text_primary(color(p.text_primary));
    g.set_accent(color(p.accent));
    g.set_accent_muted(color(p.accent_muted));
    g.set_success(color(p.success));
    g.set_warning(color(p.warning));
    g.set_danger(color(p.danger));
    g.set_border(color(p.border));
    g.set_tab_active(color(p.tab_active));
    g.set_tab_hover(color(p.tab_hover));
    g.set_selection(color(p.selection));
    g.set_font_mono(mono_family.into());
}

pub fn apply_slint(app: &AppWindow, p: &Palette, mono_family: &str) {
    apply_palette(app.global::<Theme>(), p, mono_family);
}

pub fn apply_launcher(w: &WelcomeWindow, p: &Palette, mono_family: &str) {
    apply_palette(w.global::<Theme>(), p, mono_family);
}

/// Push the palette into the surface pipeline so the editor pane follows the
/// chrome.  Mirrors `GuiTheme::apply_editor` in the egui frontend.
pub fn apply_editor(ed: &mut Editor, p: &Palette) {
    let bg = p.surface;
    let light = p.surface.0 > 128;
    let mut set = |key: &str, rgb: Rgb| {
        ed.theme.insert(key.to_string(), rgb);
    };
    set("bg", p.surface);
    set("fg", p.text_primary);
    set("line-num", p.text_faint);
    set("line-num-bg", p.surface);
    set("line-num-current", p.text_primary);
    set("status-bg", p.surface_panel);
    set("status-fg", p.text_primary);
    set("diag-error", p.danger);
    set("diag-warn", p.warning);
    set("highlight-bg", p.fill_muted);
    set("highlight-fg", p.text_primary);
    // The execution line reads as an accent band — clearly not the text
    // selection, which stays accent-solid via selection-bg.
    set("exec-line-bg", blend(p.accent, p.surface, 0.22));
    // Caret follows the chrome accent so it reads as part of the UI, not
    // as a separate colour.
    set("cursor", p.accent);
    set("exec-line-fg", p.accent);
    set("selection-bg", p.accent);
    set(
        "selection-fg",
        if light { (255, 255, 255) } else { p.chrome },
    );
    set("gui-bg", p.surface);
    set("diff-add", p.success);
    set("diff-del", p.danger);
    set("diff-hunk", p.accent);

    let face = |fg: Rgb, bold: bool, italic: bool, dim: bool| Style {
        fg,
        bg,
        bold,
        italic,
        underline: false,
        strikethrough: false,
        dim,
    };
    let s = &p.syntax;
    ed.faces
        .insert("keyword-face".into(), face(s.keyword, false, false, false));
    ed.faces
        .insert("string-face".into(), face(s.string, false, false, false));
    ed.faces
        .insert("comment-face".into(), face(s.comment, false, true, true));
    ed.faces
        .insert("type-face".into(), face(s.type_, false, false, false));
    ed.faces.insert(
        "function-face".into(),
        face(s.function, false, false, false),
    );
    ed.faces.insert(
        "variable-face".into(),
        face(s.variable, false, false, false),
    );
    ed.faces.insert(
        "constant-face".into(),
        face(s.constant, false, false, false),
    );
    ed.faces.insert(
        "operator-face".into(),
        face(s.operator, false, false, false),
    );
    ed.faces.insert(
        "punctuation-face".into(),
        face(s.punctuation, false, false, false),
    );
    ed.faces
        .insert("error-face".into(), face(s.error, true, false, false));
    ed.faces
        .insert("warning-face".into(), face(s.warning, false, false, false));
}

pub fn color(rgb: Rgb) -> slint::Color {
    slint::Color::from_rgb_u8(rgb.0, rgb.1, rgb.2)
}

/// Blend `over` onto `base`: `amount` 1.0 is pure `over`, 0.0 pure `base`.
fn blend(over: Rgb, base: Rgb, amount: f32) -> Rgb {
    let mix = |o: u8, b: u8| (o as f32 * amount + b as f32 * (1.0 - amount)).round() as u8;
    (
        mix(over.0, base.0),
        mix(over.1, base.1),
        mix(over.2, base.2),
    )
}
