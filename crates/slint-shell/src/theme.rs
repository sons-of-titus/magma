//! Palette and editor-theme projection for the Slint shell.
//!
//! The chrome reads the Slint `Theme` global; the editor pane reads
//! `Editor.theme` and `Editor.faces`.  Both are pushed from the same palette
//! so the surface never clashes with the shell.

use magma::kernel::render::surface::Style;
use magma::kernel::state::Editor;
use slint::ComponentHandle;

use crate::{AppWindow, Theme};

type Rgb = (u8, u8, u8);

pub struct Syntax {
    pub keyword:     Rgb,
    pub string:      Rgb,
    pub comment:     Rgb,
    pub type_:       Rgb,
    pub function:    Rgb,
    pub variable:    Rgb,
    pub constant:    Rgb,
    pub operator:    Rgb,
    pub punctuation: Rgb,
    pub error:       Rgb,
    pub warning:     Rgb,
}

pub struct Palette {
    pub crust: Rgb, pub mantle: Rgb, pub base: Rgb,
    pub surface0: Rgb, pub surface1: Rgb, pub overlay0: Rgb,
    pub subtext0: Rgb, pub subtext1: Rgb, pub text: Rgb,
    pub blue: Rgb, pub red: Rgb, pub green: Rgb,
    pub lavender: Rgb, pub warning: Rgb, pub border: Rgb,
    pub tab_active: Rgb, pub tab_hover: Rgb, pub selected: Rgb,
    pub syntax: Syntax,
}

pub fn palette(name: &str) -> Palette {
    match name {
        "intellij-light" => Palette {
            crust: (247, 248, 250), mantle: (240, 241, 244), base: (255, 255, 255),
            surface0: (235, 236, 240), surface1: (223, 225, 229), overlay0: (140, 140, 140),
            subtext0: (108, 112, 126), subtext1: (56, 58, 66), text: (30, 31, 34),
            blue: (53, 116, 240), red: (219, 59, 75), green: (32, 138, 60),
            lavender: (108, 112, 126), warning: (194, 125, 0), border: (209, 209, 209),
            tab_active: (252, 252, 253), tab_hover: (249, 250, 252), selected: (207, 224, 255),
            syntax: Syntax {
                keyword: (0, 51, 179), string: (6, 125, 23), comment: (140, 140, 140),
                type_: (0, 98, 122), function: (0, 98, 122), variable: (30, 31, 34),
                constant: (23, 80, 235), operator: (30, 31, 34), punctuation: (30, 31, 34),
                error: (219, 59, 75), warning: (194, 125, 0),
            },
        },
        "mocha" => Palette {
            crust: (17, 17, 27), mantle: (24, 24, 37), base: (30, 30, 46),
            surface0: (49, 50, 68), surface1: (69, 71, 90), overlay0: (108, 112, 134),
            subtext0: (166, 173, 200), subtext1: (186, 194, 222), text: (205, 214, 244),
            blue: (137, 180, 250), red: (243, 139, 168), green: (166, 227, 161),
            lavender: (180, 190, 254), warning: (250, 179, 135), border: (49, 50, 68),
            tab_active: (42, 42, 55), tab_hover: (35, 35, 48), selected: (49, 50, 68),
            syntax: Syntax {
                keyword: (203, 166, 247), string: (166, 227, 161), comment: (108, 112, 134),
                type_: (137, 180, 250), function: (250, 179, 135), variable: (205, 214, 244),
                constant: (249, 226, 175), operator: (137, 220, 235), punctuation: (166, 173, 200),
                error: (243, 139, 168), warning: (250, 179, 135),
            },
        },
        "latte" => Palette {
            crust: (220, 224, 235), mantle: (204, 208, 218), base: (239, 241, 245),
            surface0: (188, 192, 204), surface1: (172, 176, 190), overlay0: (140, 143, 161),
            subtext0: (92, 95, 119), subtext1: (76, 79, 105), text: (48, 52, 70),
            blue: (30, 102, 245), red: (210, 15, 57), green: (64, 160, 43),
            lavender: (114, 135, 253), warning: (223, 142, 29), border: (188, 192, 204),
            tab_active: (230, 233, 239), tab_hover: (220, 224, 232), selected: (188, 202, 226),
            syntax: Syntax {
                keyword: (136, 57, 239), string: (64, 160, 43), comment: (140, 143, 161),
                type_: (30, 102, 245), function: (254, 100, 11), variable: (76, 79, 105),
                constant: (223, 142, 29), operator: (4, 165, 229), punctuation: (92, 95, 119),
                error: (210, 15, 57), warning: (254, 100, 11),
            },
        },
        _ => Palette {
            crust: (30, 31, 34), mantle: (43, 45, 48), base: (30, 31, 34),
            surface0: (57, 59, 64), surface1: (67, 69, 74), overlay0: (111, 115, 122),
            subtext0: (157, 160, 168), subtext1: (206, 208, 214), text: (223, 225, 229),
            blue: (53, 116, 240), red: (247, 84, 100), green: (84, 145, 89),
            lavender: (154, 167, 176), warning: (214, 174, 88), border: (57, 59, 64),
            tab_active: (44, 44, 47), tab_hover: (37, 38, 42), selected: (46, 67, 110),
            syntax: Syntax {
                keyword: (207, 142, 109), string: (106, 171, 115), comment: (122, 126, 133),
                type_: (185, 188, 209), function: (86, 168, 245), variable: (188, 190, 196),
                constant: (42, 172, 184), operator: (169, 183, 198), punctuation: (169, 183, 198),
                error: (247, 84, 100), warning: (214, 174, 88),
            },
        },
    }
}

pub fn apply_slint(app: &AppWindow, p: &Palette) {
    let g = app.global::<Theme>();
    g.set_crust(color(p.crust));
    g.set_mantle(color(p.mantle));
    g.set_base(color(p.base));
    g.set_surface0(color(p.surface0));
    g.set_surface1(color(p.surface1));
    g.set_overlay0(color(p.overlay0));
    g.set_subtext0(color(p.subtext0));
    g.set_subtext1(color(p.subtext1));
    g.set_text(color(p.text));
    g.set_blue(color(p.blue));
    g.set_red(color(p.red));
    g.set_green(color(p.green));
    g.set_lavender(color(p.lavender));
    g.set_warning(color(p.warning));
    g.set_border(color(p.border));
    g.set_tab_active(color(p.tab_active));
    g.set_tab_hover(color(p.tab_hover));
    g.set_selected(color(p.selected));
}

/// Push the palette into the surface pipeline so the editor pane follows the
/// chrome.  Mirrors `GuiTheme::apply_editor` in the egui frontend.
pub fn apply_editor(ed: &mut Editor, p: &Palette) {
    let bg = p.base;
    let light = p.base.0 > 128;
    let mut set = |key: &str, rgb: Rgb| {
        ed.theme.insert(key.to_string(), rgb);
    };
    set("bg", p.base);
    set("fg", p.text);
    set("line-num", p.overlay0);
    set("line-num-bg", p.base);
    set("line-num-current", p.text);
    set("status-bg", p.mantle);
    set("status-fg", p.text);
    set("diag-error", p.red);
    set("diag-warn", p.warning);
    set("highlight-bg", p.surface0);
    set("highlight-fg", p.text);
    set("selection-bg", p.blue);
    set("selection-fg", if light { (255, 255, 255) } else { p.crust });
    set("gui-bg", p.base);
    set("diff-add", p.green);
    set("diff-del", p.red);
    set("diff-hunk", p.blue);

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
    ed.faces.insert("keyword-face".into(), face(s.keyword, false, false, false));
    ed.faces.insert("string-face".into(), face(s.string, false, false, false));
    ed.faces.insert("comment-face".into(), face(s.comment, false, true, true));
    ed.faces.insert("type-face".into(), face(s.type_, false, false, false));
    ed.faces.insert("function-face".into(), face(s.function, false, false, false));
    ed.faces.insert("variable-face".into(), face(s.variable, false, false, false));
    ed.faces.insert("constant-face".into(), face(s.constant, false, false, false));
    ed.faces.insert("operator-face".into(), face(s.operator, false, false, false));
    ed.faces.insert("punctuation-face".into(), face(s.punctuation, false, false, false));
    ed.faces.insert("error-face".into(), face(s.error, true, false, false));
    ed.faces.insert("warning-face".into(), face(s.warning, false, false, false));
}

pub fn color(rgb: Rgb) -> slint::Color {
    slint::Color::from_rgb_u8(rgb.0, rgb.1, rgb.2)
}
