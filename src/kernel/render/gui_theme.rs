//! GUI theme system — Phase 9.
//!
//! One palette drives all native chrome (tabs, sidebar, status bar, tool
//! windows, dialogs, minimap) plus the egui visuals and the editor-pane
//! colour map (`Editor.theme`).  Presets: IntelliJ New UI (dark, default)
//! and IntelliJ Light, plus the Catppuccin Mocha/Latte pair.  The active
//! preset is selected by the `gui-theme` option (`"intellij-dark"` |
//! `"intellij-light"` | `"mocha"` | `"latte"`), so it persists through the
//! workspace like any other option.

use eframe::egui::{self, Color32};
use crate::kernel::state::Editor;
use super::surface::Style;

/// Shared icon glyphs — one consistent set across chrome widgets.
///
/// Glyphs come from the bundled Nerd Font symbols (`magma-icons`), which
/// `gui_fonts::font_definitions` appends to every family.  Coverage is
/// enforced by `every_theme_icon_glyph_is_covered_by_bundled_fonts`.
pub mod icons {
    pub const TERMINAL: &str = "\u{EA85}"; // cod-terminal
    pub const SEARCH:   &str = "\u{EA6D}"; // cod-search
    pub const FILE:     &str = "\u{EA7B}"; // cod-file
    pub const SYMBOL:   &str = "\u{F121}"; // fa-code
    pub const COMMAND:  &str = "\u{F4B5}"; // oct-command-palette
    pub const SETTINGS: &str = "\u{EAF8}"; // cod-gear
    pub const CLOSE:    &str = "\u{EA76}"; // cod-close
    pub const SEPARATOR: &str = "\u{F460}"; // oct-chevron-right
    pub const GIT_BRANCH: &str = "\u{EA68}"; // cod-source-control
    pub const PROJECTS: &str = "\u{F03A}"; // fa-list
    pub const LEARN:    &str = "\u{F02D}"; // fa-book
    pub const RUN:      &str = "\u{F04B}"; // fa-play
    pub const DEBUG:    &str = "\u{F188}"; // fa-bug
    pub const STOP:     &str = "\u{F04D}"; // fa-stop
    pub const BUILD:    &str = "\u{F0AD}"; // fa-wrench
    pub const TEST:     &str = "\u{F0C3}"; // fa-flask
    pub const AGENT:    &str = "\u{F0D0}"; // fa-magic
    pub const OUTPUT:   &str = "\u{F024}"; // fa-flag
    pub const PAUSE:    &str = "\u{F04C}"; // fa-pause
    pub const STEP_OVER: &str = "\u{F149}"; // fa-level-down
    pub const STEP_IN:  &str = "\u{F063}"; // fa-arrow-down
    pub const STEP_OUT: &str = "\u{F062}"; // fa-arrow-up
    pub const RESTART:  &str = "\u{F021}"; // fa-refresh
    pub const RESET:    &str = "\u{F0E2}"; // fa-undo
    pub const CARET_RIGHT: &str = "\u{F0DA}"; // fa-caret-right
    pub const CARET_DOWN:  &str = "\u{F0D7}"; // fa-caret-down
}

/// A color `t` of the way from `c` toward white, used to build the
/// elevated surfaces (active tab) that JetBrains separates from their
/// strip by a hairline of luminance rather than a border.
fn lighten(c: Color32, t: f32) -> Color32 {
    let mix = |v: u8| (v as f32 + (255.0 - v as f32) * t).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgb(mix(c.r()), mix(c.g()), mix(c.b()))
}

#[derive(Debug, Clone, PartialEq)]
pub struct GuiTheme {
    pub light:  bool,
    pub crust:    Color32,   // outermost chrome (status bar, inactive tabs)
    pub mantle:   Color32,   // docked tool windows
    pub base:     Color32,   // active tab, panels
    pub surface0: Color32,   // selection, separators
    pub surface1: Color32,   // hover fill
    pub overlay0: Color32,   // dim text, icons
    pub subtext0: Color32,   // secondary text
    pub subtext1: Color32,   // file rows
    pub text:     Color32,   // primary text
    pub blue:     Color32,   // accent: focus, links, git
    pub red:      Color32,   // errors, close hover
    pub green:    Color32,   // terminal output, success
    pub lavender: Color32,   // directories
    pub warning:  Color32,   // diagnostics, warnings
    pub border:   Color32,   // 1px dividers between chrome regions
    pub syntax:   Syntax,    // editor syntax faces, kept in step with the chrome
}

/// Editor syntax colours.  Pushed into `Editor.faces` by `apply_editor` so the
/// highlighter follows the chrome theme instead of the Catppuccin defaults.
#[derive(Debug, Clone, PartialEq)]
pub struct Syntax {
    pub keyword:     Color32,
    pub string:      Color32,
    pub comment:     Color32,
    pub type_:       Color32,
    pub function:    Color32,
    pub variable:    Color32,
    pub constant:    Color32,
    pub operator:    Color32,
    pub punctuation: Color32,
    pub error:       Color32,
    pub warning:     Color32,
}

impl GuiTheme {
    /// IntelliJ New UI Dark (the default) — flat greys, one blue accent.
    pub fn intellij_dark() -> Self {
        Self {
            light:    false,
            crust:    Color32::from_rgb(30,  31,  34),
            mantle:   Color32::from_rgb(43,  45,  48),
            base:     Color32::from_rgb(30,  31,  34),
            surface0: Color32::from_rgb(57,  59,  64),
            surface1: Color32::from_rgb(67,  69,  74),
            overlay0: Color32::from_rgb(111, 115, 122),
            subtext0: Color32::from_rgb(157, 160, 168),
            subtext1: Color32::from_rgb(206, 208, 214),
            text:     Color32::from_rgb(223, 225, 229),
            blue:     Color32::from_rgb(53,  116, 240),
            red:      Color32::from_rgb(247, 84,  100),
            green:    Color32::from_rgb(84,  145, 89),
            lavender: Color32::from_rgb(154, 167, 176),
            warning:  Color32::from_rgb(214, 174, 88),
            border:   Color32::from_rgb(57,  59,  64),
            syntax:   syntax_darcula(),
        }
    }

    /// IntelliJ Light.
    pub fn intellij_light() -> Self {
        Self {
            light:    true,
            crust:    Color32::from_rgb(247, 248, 250),
            mantle:   Color32::from_rgb(240, 241, 244),
            base:     Color32::from_rgb(255, 255, 255),
            surface0: Color32::from_rgb(235, 236, 240),
            surface1: Color32::from_rgb(223, 225, 229),
            overlay0: Color32::from_rgb(140, 140, 140),
            subtext0: Color32::from_rgb(108, 112, 126),
            subtext1: Color32::from_rgb(56,  58,  66),
            text:     Color32::from_rgb(30,  31,  34),
            blue:     Color32::from_rgb(53,  116, 240),
            red:      Color32::from_rgb(219, 59,  75),
            green:    Color32::from_rgb(32,  138, 60),
            lavender: Color32::from_rgb(108, 112, 126),
            warning:  Color32::from_rgb(194, 125, 0),
            border:   Color32::from_rgb(209, 209, 209),
            syntax:   syntax_darcula_light(),
        }
    }

    pub fn mocha() -> Self {
        Self {
            light:    false,
            crust:    Color32::from_rgb(17,  17,  27),
            mantle:   Color32::from_rgb(24,  24,  37),
            base:     Color32::from_rgb(30,  30,  46),
            surface0: Color32::from_rgb(49,  50,  68),
            surface1: Color32::from_rgb(69,  71,  90),
            overlay0: Color32::from_rgb(108, 112, 134),
            subtext0: Color32::from_rgb(166, 173, 200),
            subtext1: Color32::from_rgb(186, 194, 222),
            text:     Color32::from_rgb(205, 214, 244),
            blue:     Color32::from_rgb(137, 180, 250),
            red:      Color32::from_rgb(243, 139, 168),
            green:    Color32::from_rgb(166, 227, 161),
            lavender: Color32::from_rgb(180, 190, 254),
            warning:  Color32::from_rgb(250, 179, 135),
            border:   Color32::from_rgb(49,  50,  68),
            syntax:   syntax_mocha(),
        }
    }

    pub fn latte() -> Self {
        Self {
            light:    true,
            crust:    Color32::from_rgb(220, 224, 235),
            mantle:   Color32::from_rgb(204, 208, 218),
            base:     Color32::from_rgb(239, 241, 245),
            surface0: Color32::from_rgb(188, 192, 204),
            surface1: Color32::from_rgb(172, 176, 190),
            overlay0: Color32::from_rgb(140, 143, 161),
            subtext0: Color32::from_rgb(92,  95,  119),
            subtext1: Color32::from_rgb(76,  79,  105),
            text:     Color32::from_rgb(48,  52,  70),
            blue:     Color32::from_rgb(30,  102, 245),
            red:      Color32::from_rgb(210, 15,  57),
            green:    Color32::from_rgb(64,  160, 43),
            lavender: Color32::from_rgb(114, 135, 253),
            warning:  Color32::from_rgb(223, 142, 29),
            border:   Color32::from_rgb(188, 192, 204),
            syntax:   syntax_latte(),
        }
    }

    /// Resolve the theme named by the `gui-theme` option.
    pub fn from_options(options: &std::collections::HashMap<String, String>) -> Self {
        match options.get("gui-theme").map(|s| s.as_str()) {
            Some("intellij-light") => Self::intellij_light(),
            Some("latte")          => Self::latte(),
            Some("mocha")          => Self::mocha(),
            _                      => Self::intellij_dark(),
        }
    }

    /// True when the option names a light preset.
    pub fn option_is_light(options: &std::collections::HashMap<String, String>) -> bool {
        matches!(options.get("gui-theme").map(|s| s.as_str()),
            Some("intellij-light") | Some("latte"))
    }

    /// Fill for the selected editor tab: a hair lighter than the strip so the
    /// active tab reads as an elevated card (JetBrains New UI) instead of
    /// vanishing into a strip that shares the editor's background.
    pub fn tab_active_bg(&self) -> Color32 {
        lighten(self.crust, if self.light { 0.6 } else { 0.06 })
    }

    /// Fill for an inactive tab under the pointer — half-way to the active tab.
    pub fn tab_hover_bg(&self) -> Color32 {
        lighten(self.crust, if self.light { 0.3 } else { 0.03 })
    }

    /// Install egui visuals derived from this palette: dark/light base,
    /// squared low-radius widgets, 1px dividers, and a subtle shadow.
    pub fn apply_egui(&self, ctx: &egui::Context) {
        let mut vis = if self.light {
            egui::Visuals::light()
        } else {
            egui::Visuals::dark()
        };

        vis.panel_fill      = self.base;
        vis.window_fill     = self.base;
        vis.extreme_bg_color = self.crust;
        vis.faint_bg_color  = self.surface0;
        vis.selection.bg_fill = self.blue;
        vis.selection.stroke = egui::Stroke::new(1.0, self.text);

        // JetBrains chrome is near-square with hairline borders.
        let r = egui::CornerRadius::same(2);
        vis.widgets.noninteractive.bg_fill        = self.base;
        vis.widgets.noninteractive.bg_stroke      = egui::Stroke::new(1.0, self.border);
        vis.widgets.noninteractive.fg_stroke      = egui::Stroke::new(1.0, self.subtext0);
        vis.widgets.noninteractive.corner_radius  = r;
        vis.widgets.inactive.bg_fill              = self.surface0;
        vis.widgets.inactive.bg_stroke            = egui::Stroke::new(1.0, self.border);
        vis.widgets.inactive.fg_stroke            = egui::Stroke::new(1.0, self.text);
        vis.widgets.inactive.corner_radius        = r;
        vis.widgets.hovered.bg_fill               = self.surface1;
        vis.widgets.hovered.bg_stroke             = egui::Stroke::new(1.0, self.border);
        vis.widgets.hovered.fg_stroke             = egui::Stroke::new(1.0, self.text);
        vis.widgets.hovered.corner_radius         = r;
        vis.widgets.active.bg_fill                = self.surface1;
        vis.widgets.active.bg_stroke              = egui::Stroke::new(1.0, self.blue);
        vis.widgets.active.fg_stroke              = egui::Stroke::new(1.0, self.text);
        vis.widgets.active.corner_radius          = r;
        vis.widgets.open.bg_fill                  = self.surface0;
        vis.widgets.open.corner_radius            = r;

        vis.window_corner_radius = egui::CornerRadius::same(4);
        vis.window_stroke   = egui::Stroke::new(1.0, self.border);
        vis.window_shadow    = egui::Shadow {
            offset: [0, 2],
            blur:   8,
            spread: 0,
            color:  Color32::from_black_alpha(if self.light { 40 } else { 110 }),
        };
        vis.popup_shadow = vis.window_shadow;

        let theme = if self.light { egui::Theme::Light } else { egui::Theme::Dark };
        let mut style = (*ctx.style_of(theme)).clone();
        style.visuals = vis;
        style.spacing.item_spacing = egui::vec2(6.0, 4.0);
        ctx.set_style_of(theme, style);
        ctx.set_theme(theme);
    }

    /// Push the palette into the editor colour map so the Surface-based
    /// editor pane follows the GUI theme.
    pub fn apply_editor(&self, ed: &mut Editor) {
        let t = |c: Color32| (c.r(), c.g(), c.b());
        let m = &mut ed.theme;
        m.insert("bg".into(),               t(self.base));
        m.insert("fg".into(),               t(self.text));
        m.insert("line-num".into(),         t(self.overlay0));
        m.insert("line-num-bg".into(),      t(self.base));
        m.insert("line-num-current".into(), t(self.text));
        m.insert("status-bg".into(),        t(self.mantle));
        m.insert("status-fg".into(),        t(self.text));
        m.insert("diag-error".into(),       t(self.red));
        m.insert("diag-warn".into(),        t(self.warning));
        m.insert("highlight-bg".into(),     t(self.surface0));
        m.insert("highlight-fg".into(),     t(self.text));
        m.insert("selection-bg".into(),     t(self.blue));
        m.insert("selection-fg".into(),     t(if self.light {
            Color32::WHITE
        } else {
            self.crust
        }));
        m.insert("gui-bg".into(),           t(self.base));

        // Syntax faces follow the chrome theme so the editor text never
        // clashes with it (the Janet defaults are Catppuccin-only).
        let bg = t(self.base);
        let face = |fg: Color32, bold: bool, italic: bool, dim: bool| Style {
            fg: t(fg), bg, bold, italic, underline: false, strikethrough: false, dim,
        };
        let s = &self.syntax;
        ed.faces.insert("keyword-face".into(),     face(s.keyword,     false, false, false));
        ed.faces.insert("string-face".into(),      face(s.string,      false, false, false));
        ed.faces.insert("comment-face".into(),     face(s.comment,     false, true,  true));
        ed.faces.insert("type-face".into(),        face(s.type_,       false, false, false));
        ed.faces.insert("function-face".into(),    face(s.function,    false, false, false));
        ed.faces.insert("variable-face".into(),    face(s.variable,    false, false, false));
        ed.faces.insert("constant-face".into(),    face(s.constant,    false, false, false));
        ed.faces.insert("operator-face".into(),    face(s.operator,    false, false, false));
        ed.faces.insert("punctuation-face".into(), face(s.punctuation, false, false, false));
        ed.faces.insert("error-face".into(),       face(s.error,       true,  false, false));
        ed.faces.insert("warning-face".into(),     face(s.warning,     false, false, false));
    }
}

/// IntelliJ New UI Dark syntax colours (Darcula family).
fn syntax_darcula() -> Syntax {
    Syntax {
        keyword:     Color32::from_rgb(207, 142, 109),
        string:      Color32::from_rgb(106, 171, 115),
        comment:     Color32::from_rgb(122, 126, 133),
        type_:       Color32::from_rgb(185, 188, 209),
        function:    Color32::from_rgb(86,  168, 245),
        variable:    Color32::from_rgb(188, 190, 196),
        constant:    Color32::from_rgb(42,  172, 184),
        operator:    Color32::from_rgb(169, 183, 198),
        punctuation: Color32::from_rgb(169, 183, 198),
        error:       Color32::from_rgb(247, 84,  100),
        warning:     Color32::from_rgb(214, 174, 88),
    }
}

/// IntelliJ Light syntax colours.
fn syntax_darcula_light() -> Syntax {
    Syntax {
        keyword:     Color32::from_rgb(0,   51,  179),
        string:      Color32::from_rgb(6,   125, 23),
        comment:     Color32::from_rgb(140, 140, 140),
        type_:       Color32::from_rgb(0,   98,  122),
        function:    Color32::from_rgb(0,   98,  122),
        variable:    Color32::from_rgb(30,  31,  34),
        constant:    Color32::from_rgb(23,  80,  235),
        operator:    Color32::from_rgb(30,  31,  34),
        punctuation: Color32::from_rgb(30,  31,  34),
        error:       Color32::from_rgb(219, 59,  75),
        warning:     Color32::from_rgb(194, 125, 0),
    }
}

/// Catppuccin Mocha syntax colours (matches `builtins/init.janet`).
fn syntax_mocha() -> Syntax {
    Syntax {
        keyword:     Color32::from_rgb(203, 166, 247),
        string:      Color32::from_rgb(166, 227, 161),
        comment:     Color32::from_rgb(108, 112, 134),
        type_:       Color32::from_rgb(137, 180, 250),
        function:    Color32::from_rgb(250, 179, 135),
        variable:    Color32::from_rgb(205, 214, 244),
        constant:    Color32::from_rgb(249, 226, 175),
        operator:    Color32::from_rgb(137, 220, 235),
        punctuation: Color32::from_rgb(166, 173, 200),
        error:       Color32::from_rgb(243, 139, 168),
        warning:     Color32::from_rgb(250, 179, 135),
    }
}

/// Catppuccin Latte syntax colours.
fn syntax_latte() -> Syntax {
    Syntax {
        keyword:     Color32::from_rgb(136, 57,  239),
        string:      Color32::from_rgb(64,  160, 43),
        comment:     Color32::from_rgb(140, 143, 161),
        type_:       Color32::from_rgb(30,  102, 245),
        function:    Color32::from_rgb(254, 100, 11),
        variable:    Color32::from_rgb(76,  79,  105),
        constant:    Color32::from_rgb(223, 142, 29),
        operator:    Color32::from_rgb(4,   165, 229),
        punctuation: Color32::from_rgb(92,  95,  119),
        error:       Color32::from_rgb(210, 15,  57),
        warning:     Color32::from_rgb(254, 100, 11),
    }
}
