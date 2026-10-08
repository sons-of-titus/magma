//! GUI theme system — Phase 9.
//!
//! One palette drives all native chrome (tabs, sidebar, status bar, tool
//! windows, dialogs, minimap) plus the egui visuals and the editor-pane
//! colour map (`Editor.theme`).  Two presets: Catppuccin Mocha (dark,
//! default) and Catppuccin Latte (light).  The active preset is selected by
//! the `gui-theme` option (`"mocha"` | `"latte"`), so it persists through
//! the workspace like any other option.

use eframe::egui::{self, Color32};
use crate::kernel::state::Editor;

/// Shared icon glyphs — one consistent set across chrome widgets.
///
/// Every codepoint must be covered by egui's default font fallback chain
/// (Ubuntu-Light → NotoEmoji → emoji-icon-font); an uncovered glyph renders
/// as epaint's replacement tofu square.  Enforced by
/// `every_theme_icon_glyph_is_covered_by_egui_default_fonts`.
pub mod icons {
    pub const TERMINAL: &str = "⊟";
    pub const SEARCH:   &str = "⌖";
    pub const FILE:     &str = "⛶";
    pub const SYMBOL:   &str = "ƒ";
    pub const COMMAND:  &str = "⌘";
    pub const SETTINGS: &str = "⚙";
    pub const CLOSE:    &str = "×";
    pub const SEPARATOR: &str = "›";
    pub const GIT_BRANCH: &str = "⚒";
}

#[derive(Debug, Clone)]
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
}

impl GuiTheme {
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
        }
    }

    /// Resolve the theme named by the `gui-theme` option.
    pub fn from_options(options: &std::collections::HashMap<String, String>) -> Self {
        match options.get("gui-theme").map(|s| s.as_str()) {
            Some("latte") => Self::latte(),
            _             => Self::mocha(),
        }
    }

    /// True when the option names the light preset.
    pub fn option_is_light(options: &std::collections::HashMap<String, String>) -> bool {
        options.get("gui-theme").map(|s| s.as_str()) == Some("latte")
    }

    /// Install egui visuals derived from this palette: dark/light base,
    /// rounded widgets, hover states, and window shadows.
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

        let r = egui::CornerRadius::same(6);
        vis.widgets.noninteractive.bg_fill        = self.base;
        vis.widgets.noninteractive.bg_stroke      = egui::Stroke::new(1.0, self.surface0);
        vis.widgets.noninteractive.fg_stroke      = egui::Stroke::new(1.0, self.subtext0);
        vis.widgets.noninteractive.corner_radius  = r;
        vis.widgets.inactive.bg_fill              = self.surface0;
        vis.widgets.inactive.bg_stroke            = egui::Stroke::new(1.0, self.surface0);
        vis.widgets.inactive.fg_stroke            = egui::Stroke::new(1.0, self.text);
        vis.widgets.inactive.corner_radius        = r;
        vis.widgets.hovered.bg_fill               = self.surface1;
        vis.widgets.hovered.bg_stroke             = egui::Stroke::new(1.0, self.blue);
        vis.widgets.hovered.fg_stroke             = egui::Stroke::new(1.0, self.text);
        vis.widgets.hovered.corner_radius         = r;
        vis.widgets.active.bg_fill                = self.surface1;
        vis.widgets.active.bg_stroke              = egui::Stroke::new(1.0, self.blue);
        vis.widgets.active.fg_stroke              = egui::Stroke::new(1.0, self.text);
        vis.widgets.active.corner_radius          = r;
        vis.widgets.open.bg_fill                  = self.surface0;
        vis.widgets.open.corner_radius            = r;

        vis.window_corner_radius = egui::CornerRadius::same(8);
        vis.window_stroke   = egui::Stroke::new(1.0, self.surface0);
        vis.window_shadow    = egui::Shadow {
            offset: [0, 4],
            blur:   14,
            spread: 2,
            color:  if self.light {
                Color32::from_black_alpha(60)
            } else {
                Color32::from_black_alpha(140)
            },
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
        m.insert("diag-warn".into(),        t(Color32::from_rgb(250, 179, 135)));
        m.insert("highlight-bg".into(),     t(self.surface0));
        m.insert("highlight-fg".into(),     t(Color32::from_rgb(203, 166, 247)));
        m.insert("selection-bg".into(),     t(self.blue));
        m.insert("selection-fg".into(),     t(if self.light {
            Color32::WHITE
        } else {
            self.crust
        }));
        m.insert("gui-bg".into(),           t(self.base));
    }
}
