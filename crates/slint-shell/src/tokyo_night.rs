use crate::theme::{Palette, Syntax, TerminalPalette};

pub fn palette(name: &str) -> Option<Palette> {
    match name {
        "tokyo-night" | "tokyonight" | "tokyo-night-night" => Some(night()),
        "tokyo-night-storm" | "tokyonight-storm" => Some(storm()),
        "tokyo-night-moon" | "tokyonight-moon" => Some(moon()),
        "tokyo-night-light" | "tokyo-night-day" | "tokyonight-light" | "tokyonight-day" => Some(light()),
        _ => None,
    }
}

fn night() -> Palette {
    let ansi = [
        (65, 72, 104), (247, 118, 142), (158, 206, 106), (224, 175, 104),
        (122, 162, 247), (187, 154, 247), (125, 207, 255), (169, 177, 214),
        (65, 72, 104), (247, 118, 142), (158, 206, 106), (224, 175, 104),
        (122, 162, 247), (187, 154, 247), (125, 207, 255), (192, 202, 245),
    ];
    Palette {
        chrome: (22, 22, 30),
        surface_panel: (31, 35, 53),
        surface: (26, 27, 38),
        fill_muted: (36, 40, 59),
        fill: (41, 46, 66),
        fill_strong: (59, 66, 97),
        text_faint: (86, 95, 137),
        text_muted: (115, 122, 162),
        text_secondary: (169, 177, 214),
        text_primary: (192, 202, 245),
        accent: (122, 162, 247),
        danger: (247, 118, 142),
        success: (158, 206, 106),
        accent_muted: (157, 124, 216),
        warning: (224, 175, 104),
        border: (36, 40, 59),
        tab_active: (31, 35, 53),
        tab_hover: (26, 27, 38),
        selection: (51, 70, 124),
        syntax: Syntax {
            keyword: (187, 154, 247),
            string: (158, 206, 106),
            comment: (86, 95, 137),
            type_: (42, 195, 222),
            function: (122, 162, 247),
            variable: (169, 177, 214),
            constant: (255, 158, 100),
            operator: (137, 221, 255),
            punctuation: (169, 177, 214),
            error: (247, 118, 142),
            warning: (224, 175, 104),
        },
        terminal: TerminalPalette {
            fg: (169, 177, 214),
            bg: (26, 27, 38),
            cursor: (192, 202, 245),
            ansi,
        },
    }
}

fn storm() -> Palette {
    let ansi = [
        (65, 72, 104), (247, 118, 142), (158, 206, 106), (224, 175, 104),
        (122, 162, 247), (187, 154, 247), (125, 207, 255), (169, 177, 214),
        (65, 72, 104), (247, 118, 142), (158, 206, 106), (224, 175, 104),
        (122, 162, 247), (187, 154, 247), (125, 207, 255), (192, 202, 245),
    ];
    Palette {
        chrome: (28, 31, 48),
        surface_panel: (36, 40, 59),
        surface: (36, 40, 59),
        fill_muted: (47, 51, 77),
        fill: (53, 59, 88),
        fill_strong: (65, 72, 104),
        text_faint: (86, 95, 137),
        text_muted: (115, 122, 162),
        text_secondary: (169, 177, 214),
        text_primary: (192, 202, 245),
        accent: (122, 162, 247),
        danger: (247, 118, 142),
        success: (158, 206, 106),
        accent_muted: (157, 124, 216),
        warning: (224, 175, 104),
        border: (47, 51, 77),
        tab_active: (41, 46, 66),
        tab_hover: (36, 40, 59),
        selection: (51, 70, 124),
        syntax: Syntax {
            keyword: (187, 154, 247),
            string: (158, 206, 106),
            comment: (86, 95, 137),
            type_: (42, 195, 222),
            function: (122, 162, 247),
            variable: (169, 177, 214),
            constant: (255, 158, 100),
            operator: (137, 221, 255),
            punctuation: (169, 177, 214),
            error: (247, 118, 142),
            warning: (224, 175, 104),
        },
        terminal: TerminalPalette {
            fg: (169, 177, 214),
            bg: (36, 40, 59),
            cursor: (192, 202, 245),
            ansi,
        },
    }
}

fn moon() -> Palette {
    let ansi = [
        (68, 74, 115), (255, 117, 127), (195, 232, 141), (255, 199, 119),
        (130, 170, 255), (192, 153, 255), (134, 225, 252), (130, 139, 184),
        (68, 74, 115), (255, 117, 127), (195, 232, 141), (255, 199, 119),
        (130, 170, 255), (192, 153, 255), (134, 225, 252), (200, 211, 245),
    ];
    Palette {
        chrome: (26, 28, 42),
        surface_panel: (41, 44, 69),
        surface: (34, 36, 54),
        fill_muted: (47, 51, 77),
        fill: (59, 66, 97),
        fill_strong: (68, 74, 115),
        text_faint: (99, 109, 166),
        text_muted: (130, 139, 184),
        text_secondary: (130, 139, 184),
        text_primary: (200, 211, 245),
        accent: (130, 170, 255),
        danger: (255, 117, 127),
        success: (195, 232, 141),
        accent_muted: (192, 153, 255),
        warning: (255, 199, 119),
        border: (47, 51, 77),
        tab_active: (41, 44, 69),
        tab_hover: (34, 36, 54),
        selection: (68, 74, 115),
        syntax: Syntax {
            keyword: (192, 153, 255),
            string: (195, 232, 141),
            comment: (99, 109, 166),
            type_: (134, 225, 252),
            function: (130, 170, 255),
            variable: (130, 139, 184),
            constant: (255, 150, 108),
            operator: (137, 221, 255),
            punctuation: (130, 139, 184),
            error: (255, 117, 127),
            warning: (255, 199, 119),
        },
        terminal: TerminalPalette {
            fg: (200, 211, 245),
            bg: (34, 36, 54),
            cursor: (200, 211, 245),
            ansi,
        },
    }
}

fn light() -> Palette {
    let ansi = [
        (52, 59, 88), (140, 67, 81), (51, 99, 92), (143, 94, 21),
        (41, 89, 170), (123, 67, 186), (0, 108, 134), (112, 114, 128),
        (52, 59, 88), (140, 67, 81), (51, 99, 92), (143, 94, 21),
        (41, 89, 170), (123, 67, 186), (0, 108, 134), (52, 59, 88),
    ];
    Palette {
        chrome: (214, 216, 223),
        surface_panel: (214, 216, 223),
        surface: (230, 231, 237),
        fill_muted: (200, 202, 212),
        fill: (188, 190, 204),
        fill_strong: (170, 173, 190),
        text_faint: (108, 110, 117),
        text_muted: (85, 90, 110),
        text_secondary: (60, 65, 90),
        text_primary: (52, 59, 88),
        accent: (41, 89, 170),
        danger: (140, 67, 81),
        success: (56, 95, 13),
        accent_muted: (101, 53, 157),
        warning: (143, 94, 21),
        border: (200, 202, 212),
        tab_active: (230, 231, 237),
        tab_hover: (220, 222, 230),
        selection: (188, 190, 204),
        syntax: Syntax {
            keyword: (101, 53, 157),
            string: (56, 95, 13),
            comment: (108, 110, 117),
            type_: (41, 89, 170),
            function: (41, 89, 170),
            variable: (52, 59, 88),
            constant: (143, 94, 21),
            operator: (52, 59, 88),
            punctuation: (52, 59, 88),
            error: (140, 67, 81),
            warning: (143, 94, 21),
        },
        terminal: TerminalPalette {
            fg: (52, 59, 88),
            bg: (214, 216, 223),
            cursor: (52, 59, 88),
            ansi,
        },
    }
}