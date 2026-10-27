//! Palette and editor-theme projection for the Slint shell.
//!
//! The chrome reads the Slint `Theme` global; the editor pane reads
//! `Editor.theme` and `Editor.faces`.  Both are pushed from the same palette
//! so the surface never clashes with the shell.

use kernel::render::surface::Style;
use kernel::state::Editor;
use slint::ComponentHandle;

use crate::{AppWindow, Theme};

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
    match name {
        "intellij-light" => Palette {
            chrome: (247, 248, 250),
            surface_panel: (240, 241, 244),
            surface: (255, 255, 255),
            fill_muted: (235, 236, 240),
            fill: (223, 225, 229),
            fill_strong: (207, 209, 214),
            text_faint: (140, 140, 140),
            text_muted: (108, 112, 126),
            text_secondary: (56, 58, 66),
            text_primary: (30, 31, 34),
            accent: (53, 116, 240),
            danger: (219, 59, 75),
            success: (32, 138, 60),
            accent_muted: (108, 112, 126),
            warning: (194, 125, 0),
            border: (209, 209, 209),
            tab_active: (252, 252, 253),
            tab_hover: (249, 250, 252),
            selection: (207, 224, 255),
            syntax: Syntax {
                keyword: (0, 51, 179),
                string: (6, 125, 23),
                comment: (140, 140, 140),
                type_: (0, 98, 122),
                function: (0, 98, 122),
                variable: (30, 31, 34),
                constant: (23, 80, 235),
                operator: (30, 31, 34),
                punctuation: (30, 31, 34),
                error: (219, 59, 75),
                warning: (194, 125, 0),
            },
            terminal: TerminalPalette {
                fg: (30, 31, 34),
                bg: (255, 255, 255),
                cursor: (30, 31, 34),
                ansi: [
                    (0, 0, 0),
                    (219, 59, 75),
                    (32, 138, 60),
                    (194, 125, 0),
                    (53, 116, 240),
                    (166, 38, 190),
                    (0, 135, 175),
                    (108, 112, 126),
                    (140, 140, 140),
                    (229, 80, 95),
                    (60, 168, 88),
                    (214, 152, 32),
                    (90, 140, 240),
                    (190, 90, 210),
                    (36, 160, 200),
                    (30, 31, 34),
                ],
            },
        },
        "mocha" => Palette {
            chrome: (17, 17, 27),
            surface_panel: (24, 24, 37),
            surface: (30, 30, 46),
            fill_muted: (49, 50, 68),
            fill: (69, 71, 90),
            fill_strong: (88, 91, 112),
            text_faint: (108, 112, 134),
            text_muted: (166, 173, 200),
            text_secondary: (186, 194, 222),
            text_primary: (205, 214, 244),
            accent: (137, 180, 250),
            danger: (243, 139, 168),
            success: (166, 227, 161),
            accent_muted: (180, 190, 254),
            warning: (250, 179, 135),
            border: (49, 50, 68),
            tab_active: (42, 42, 55),
            tab_hover: (35, 35, 48),
            selection: (49, 50, 68),
            syntax: Syntax {
                keyword: (203, 166, 247),
                string: (166, 227, 161),
                comment: (108, 112, 134),
                type_: (137, 180, 250),
                function: (250, 179, 135),
                variable: (205, 214, 244),
                constant: (249, 226, 175),
                operator: (137, 220, 235),
                punctuation: (166, 173, 200),
                error: (243, 139, 168),
                warning: (250, 179, 135),
            },
            terminal: TerminalPalette {
                fg: (205, 214, 244),
                bg: (30, 30, 46),
                cursor: (205, 214, 244),
                ansi: [
                    (69, 71, 90),
                    (243, 139, 168),
                    (166, 227, 161),
                    (249, 226, 175),
                    (137, 180, 250),
                    (245, 194, 231),
                    (148, 226, 213),
                    (186, 194, 222),
                    (88, 91, 112),
                    (243, 139, 168),
                    (166, 227, 161),
                    (249, 226, 175),
                    (137, 180, 250),
                    (245, 194, 231),
                    (148, 226, 213),
                    (166, 173, 200),
                ],
            },
        },
        "latte" => Palette {
            chrome: (220, 224, 235),
            surface_panel: (204, 208, 218),
            surface: (239, 241, 245),
            fill_muted: (188, 192, 204),
            fill: (172, 176, 190),
            fill_strong: (150, 155, 170),
            text_faint: (140, 143, 161),
            text_muted: (92, 95, 119),
            text_secondary: (76, 79, 105),
            text_primary: (48, 52, 70),
            accent: (30, 102, 245),
            danger: (210, 15, 57),
            success: (64, 160, 43),
            accent_muted: (114, 135, 253),
            warning: (223, 142, 29),
            border: (188, 192, 204),
            tab_active: (230, 233, 239),
            tab_hover: (220, 224, 232),
            selection: (188, 202, 226),
            syntax: Syntax {
                keyword: (136, 57, 239),
                string: (64, 160, 43),
                comment: (140, 143, 161),
                type_: (30, 102, 245),
                function: (254, 100, 11),
                variable: (76, 79, 105),
                constant: (223, 142, 29),
                operator: (4, 165, 229),
                punctuation: (92, 95, 119),
                error: (210, 15, 57),
                warning: (254, 100, 11),
            },
            terminal: TerminalPalette {
                fg: (76, 79, 105),
                bg: (239, 241, 245),
                cursor: (76, 79, 105),
                ansi: [
                    (92, 95, 119),
                    (210, 15, 57),
                    (64, 160, 43),
                    (223, 142, 29),
                    (30, 102, 245),
                    (234, 118, 203),
                    (23, 146, 153),
                    (172, 176, 190),
                    (108, 111, 133),
                    (210, 15, 57),
                    (64, 160, 43),
                    (223, 142, 29),
                    (30, 102, 245),
                    (234, 118, 203),
                    (23, 146, 153),
                    (188, 192, 204),
                ],
            },
        },
        _ => Palette {
            chrome: (30, 31, 34),
            surface_panel: (43, 45, 48),
            surface: (30, 31, 34),
            fill_muted: (57, 59, 64),
            fill: (67, 69, 74),
            fill_strong: (78, 81, 87),
            text_faint: (111, 115, 122),
            text_muted: (157, 160, 168),
            text_secondary: (206, 208, 214),
            text_primary: (223, 225, 229),
            accent: (53, 116, 240),
            danger: (247, 84, 100),
            success: (84, 145, 89),
            accent_muted: (154, 167, 176),
            warning: (214, 174, 88),
            border: (57, 59, 64),
            tab_active: (44, 44, 47),
            tab_hover: (37, 38, 42),
            selection: (46, 67, 110),
            syntax: Syntax {
                keyword: (207, 142, 109),
                string: (106, 171, 115),
                comment: (122, 126, 133),
                type_: (185, 188, 209),
                function: (86, 168, 245),
                variable: (188, 190, 196),
                constant: (42, 172, 184),
                operator: (169, 183, 198),
                punctuation: (169, 183, 198),
                error: (247, 84, 100),
                warning: (214, 174, 88),
            },
            terminal: TerminalPalette {
                fg: (223, 225, 229),
                bg: (30, 31, 34),
                cursor: (223, 225, 229),
                ansi: [
                    (0, 0, 0),
                    (250, 83, 85),
                    (18, 110, 0),
                    (194, 195, 0),
                    (69, 129, 235),
                    (250, 84, 255),
                    (51, 194, 193),
                    (173, 173, 173),
                    (85, 85, 85),
                    (251, 113, 114),
                    (103, 255, 79),
                    (255, 255, 0),
                    (109, 157, 252),
                    (251, 130, 255),
                    (96, 212, 213),
                    (238, 238, 238),
                ],
            },
        },
    }
}

pub fn apply_slint(app: &AppWindow, p: &Palette, mono_family: &str) {
    let g = app.global::<Theme>();
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
