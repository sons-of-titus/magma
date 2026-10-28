use crate::theme::{Palette, Syntax, TerminalPalette};

pub fn palette(name: &str) -> Option<Palette> {
    match name {
        "frappe" | "catppuccin-frappe" => Some(frappe()),
        "macchiato" | "catppuccin-macchiato" => Some(macchiato()),
        "catppuccin-mocha" => Some(crate::palettes::palette("mocha")),
        "catppuccin-latte" => Some(crate::palettes::palette("latte")),
        _ => None,
    }
}

fn frappe() -> Palette {
    let ansi = [
        (65, 69, 89), (231, 130, 132), (166, 209, 137), (229, 200, 144),
        (140, 170, 238), (202, 158, 230), (129, 200, 190), (198, 208, 245),
        (115, 121, 148), (231, 130, 132), (166, 209, 137), (229, 200, 144),
        (140, 170, 238), (202, 158, 230), (129, 200, 190), (198, 208, 245),
    ];
    Palette {
        chrome: (35, 38, 52),
        surface_panel: (41, 44, 60),
        surface: (48, 52, 70),
        fill_muted: (65, 69, 89),
        fill: (81, 87, 109),
        fill_strong: (98, 104, 128),
        text_faint: (115, 121, 148),
        text_muted: (165, 173, 206),
        text_secondary: (181, 191, 226),
        text_primary: (198, 208, 245),
        accent: (140, 170, 238),
        danger: (231, 130, 132),
        success: (166, 209, 137),
        accent_muted: (186, 187, 241),
        warning: (229, 200, 144),
        border: (65, 69, 89),
        tab_active: (41, 44, 60),
        tab_hover: (35, 38, 52),
        selection: (65, 69, 89),
        syntax: Syntax {
            keyword: (202, 158, 230),
            string: (166, 209, 137),
            comment: (115, 121, 148),
            type_: (140, 170, 238),
            function: (239, 159, 118),
            variable: (198, 208, 245),
            constant: (229, 200, 144),
            operator: (153, 209, 219),
            punctuation: (165, 173, 206),
            error: (231, 130, 132),
            warning: (239, 159, 118),
        },
        terminal: TerminalPalette {
            fg: (198, 208, 245),
            bg: (48, 52, 70),
            cursor: (198, 208, 245),
            ansi,
        },
    }
}

fn macchiato() -> Palette {
    let ansi = [
        (54, 58, 79), (237, 135, 150), (166, 218, 149), (238, 212, 159),
        (138, 173, 244), (198, 160, 246), (139, 213, 202), (202, 211, 245),
        (110, 115, 141), (237, 135, 150), (166, 218, 149), (238, 212, 159),
        (138, 173, 244), (198, 160, 246), (139, 213, 202), (202, 211, 245),
    ];
    Palette {
        chrome: (24, 25, 38),
        surface_panel: (30, 32, 48),
        surface: (36, 39, 58),
        fill_muted: (54, 58, 79),
        fill: (73, 77, 100),
        fill_strong: (91, 96, 120),
        text_faint: (110, 115, 141),
        text_muted: (165, 173, 203),
        text_secondary: (184, 192, 224),
        text_primary: (202, 211, 245),
        accent: (138, 173, 244),
        danger: (237, 135, 150),
        success: (166, 218, 149),
        accent_muted: (183, 189, 248),
        warning: (238, 212, 159),
        border: (54, 58, 79),
        tab_active: (30, 32, 48),
        tab_hover: (24, 25, 38),
        selection: (54, 58, 79),
        syntax: Syntax {
            keyword: (198, 160, 246),
            string: (166, 218, 149),
            comment: (110, 115, 141),
            type_: (138, 173, 244),
            function: (245, 169, 127),
            variable: (202, 211, 245),
            constant: (238, 212, 159),
            operator: (145, 215, 227),
            punctuation: (165, 173, 203),
            error: (237, 135, 150),
            warning: (245, 169, 127),
        },
        terminal: TerminalPalette {
            fg: (202, 211, 245),
            bg: (36, 39, 58),
            cursor: (202, 211, 245),
            ansi,
        },
    }
}