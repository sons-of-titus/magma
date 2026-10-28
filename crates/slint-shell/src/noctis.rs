use crate::theme::{Palette, Rgb, Syntax, TerminalPalette};

fn hx(s: &str) -> Rgb {
    let s = s.trim_start_matches('#');
    let r = u8::from_str_radix(&s[0..2], 16).unwrap_or(0);
    let g = u8::from_str_radix(&s[2..4], 16).unwrap_or(0);
    let b = u8::from_str_radix(&s[4..6], 16).unwrap_or(0);
    (r, g, b)
}

fn darken(c: Rgb, n: u8) -> Rgb {
    (c.0.saturating_sub(n), c.1.saturating_sub(n), c.2.saturating_sub(n))
}
fn lighten(c: Rgb, n: u8) -> Rgb {
    (c.0.saturating_add(n), c.1.saturating_add(n), c.2.saturating_add(n))
}
fn mid(a: Rgb, b: Rgb) -> Rgb {
    (
        ((a.0 as u16 + b.0 as u16) / 2) as u8,
        ((a.1 as u16 + b.1 as u16) / 2) as u8,
        ((a.2 as u16 + b.2 as u16) / 2) as u8,
    )
}

fn build(bg: &str, fg: &str, cursor: &str, line_num: &str, term_bg: &str, term_fg: &str, light: bool) -> Palette {
    let bg = hx(bg);
    let fg = hx(fg);
    let cursor = hx(cursor);
    let line_num = hx(line_num);
    let term_bg = hx(term_bg);
    let term_fg = hx(term_fg);

    let (chrome, surface_panel, fill_muted, fill, fill_strong, tab_active, tab_hover, selection, text_muted, text_secondary, border) =
        if light {
            let chrome = darken(bg, 18);
            let surface_panel = darken(bg, 10);
            let fill_muted = darken(bg, 6);
            let fill = darken(bg, 12);
            let fill_strong = darken(bg, 20);
            let tab_active = bg;
            let tab_hover = darken(bg, 4);
            let selection = fill_strong;
            let text_muted = mid(line_num, fg);
            let text_secondary = mid(text_muted, fg);
            let border = fill_muted;
            (chrome, surface_panel, fill_muted, fill, fill_strong, tab_active, tab_hover, selection, text_muted, text_secondary, border)
        } else {
            let chrome = darken(bg, 8);
            let surface_panel = darken(bg, 4);
            let fill_muted = lighten(bg, 12);
            let fill = lighten(bg, 22);
            let fill_strong = lighten(bg, 32);
            let tab_active = lighten(bg, 6);
            let tab_hover = lighten(bg, 3);
            let selection = fill_strong;
            let text_muted = mid(line_num, fg);
            let text_secondary = mid(text_muted, fg);
            let border = fill_muted;
            (chrome, surface_panel, fill_muted, fill, fill_strong, tab_active, tab_hover, selection, text_muted, text_secondary, border)
        };

    let syntax = Syntax {
        keyword: hx("#df769b"),
        string: hx("#49e9a6"),
        comment: hx("#5b858b"),
        type_: hx("#d67e5c"),
        function: hx("#16a3b6"),
        variable: hx("#e4b781"),
        constant: hx("#d5971a"),
        operator: fg,
        punctuation: fg,
        error: hx("#e34e1c"),
        warning: hx("#e69533"),
    };

    let ansi = [
        hx("#324a4d"), hx("#e66533"), hx("#49e9a6"), hx("#e4b781"),
        hx("#49ace9"), hx("#df769b"), hx("#49d6e9"), hx("#b2cacd"),
        hx("#47686c"), hx("#e97749"), hx("#60ebb1"), hx("#e69533"),
        hx("#60b6eb"), hx("#e798b3"), hx("#60dbeb"), hx("#c1d4d7"),
    ];

    Palette {
        chrome,
        surface_panel,
        surface: bg,
        fill_muted,
        fill,
        fill_strong,
        text_faint: line_num,
        text_muted,
        text_secondary,
        text_primary: fg,
        accent: cursor,
        danger: hx("#e34e1c"),
        success: hx("#49e9a6"),
        accent_muted: mid(cursor, fg),
        warning: hx("#e69533"),
        border,
        tab_active,
        tab_hover,
        selection,
        syntax,
        terminal: TerminalPalette {
            fg: term_fg,
            bg: term_bg,
            cursor,
            ansi,
        },
    }
}

pub fn palette(name: &str) -> Option<Palette> {
    match name {
        "noctis" => Some(build("#052529", "#b2cacd", "#85efff", "#4e6b6e", "#03191b", "#b2cacd", false)),
        "noctis-azureus" | "azureus" => Some(build("#07273b", "#becfda", "#b3e2ff", "#4d6c80", "#051b29", "#becfda", false)),
        "noctis-bordo" | "bordo" => Some(build("#322a2d", "#cbbec2", "#ffb3cd", "#715b63", "#272022", "#cbbec2", false)),
        "noctis-hibernus" | "hibernus" => Some(build("#f4f6f6", "#005661", "#0092a8", "#a0abac", "#e1eeef", "#005661", true)),
        "noctis-lilac" | "lilac" => Some(build("#f2f1f8", "#0c006b", "#5c49e9", "#9d9ab1", "#e9e7f3", "#0c006b", true)),
        "noctis-lux" | "lux" => Some(build("#fef8ec", "#005661", "#0092a8", "#a0abac", "#f6edda", "#005661", true)),
        "noctis-minimus" | "minimus" => Some(build("#1b2932", "#c5cdd3", "#b3d2e6", "#5d6e79", "#0e1920", "#c5cdd3", false)),
        "noctis-obscuro" | "obscuro" => Some(build("#031417", "#b2cacd", "#85efff", "#4e6b6e", "#020c0e", "#b2cacd", false)),
        "noctis-sereno" | "sereno" => Some(build("#062e32", "#b2cacd", "#85efff", "#4e6b6e", "#041d20", "#b2cacd", false)),
        "noctis-uva" | "uva" => Some(build("#292640", "#c5c2d6", "#bbb3ff", "#5c5973", "#1f1d30", "#c5c2d6", false)),
        "noctis-viola" | "viola" => Some(build("#30243d", "#ccbfd9", "#d9b3ff", "#665973", "#291d35", "#ccbfd9", false)),
        _ => None,
    }
}