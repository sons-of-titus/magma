//! The typed option registry — one catalog of every user-facing option.
//!
//! The storage stays the string map on `Editor` (settable from Janet via
//! `option/set`); this registry gives the Settings dialog types, defaults,
//! and descriptions, and keeps the docs honest.  Options not listed here
//! render under "Custom".

use crate::state::Editor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionKind {
    Bool,
    Int,
    Enum,
    Str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionCategory {
    Editing,
    Appearance,
    Behavior,
}

impl OptionCategory {
    pub fn title(self) -> &'static str {
        match self {
            OptionCategory::Editing => "Editing",
            OptionCategory::Appearance => "Appearance",
            OptionCategory::Behavior => "Behavior",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct OptionSpec {
    pub name: &'static str,
    /// Human title shown in the Settings UI; the registry `name` is for
    /// scripts and persistence, not for reading.
    pub label: &'static str,
    pub kind: OptionKind,
    /// Choices for `OptionKind::Enum`.
    pub choices: &'static [&'static str],
    pub default: &'static str,
    pub description: &'static str,
    pub category: OptionCategory,
}

/// The catalog.  Order is display order.
pub const OPTION_SPECS: &[OptionSpec] = &[
    OptionSpec {
        name: "gui-theme",
        label: "Theme",
        kind: OptionKind::Enum,
        choices: &[
            "intellij-dark",
            "intellij-light",
            "catppuccin-mocha",
            "catppuccin-latte",
            "catppuccin-frappe",
            "catppuccin-macchiato",
            "monokai",
            "ayu-dark",
            "ayu-light",
            "dracula",
            "alucard",
            "tokyo-night",
            "tokyo-night-storm",
            "tokyo-night-moon",
            "tokyo-night-light",
            "noctis",
            "noctis-azureus",
            "noctis-bordo",
            "noctis-hibernus",
            "noctis-lilac",
            "noctis-lux",
            "noctis-minimus",
            "noctis-obscuro",
            "noctis-sereno",
            "noctis-uva",
            "noctis-viola",
        ],
        default: "intellij-dark",
        description: "Chrome palette: IntelliJ, Catppuccin, Monokai, Ayu, Dracula, Tokyo Night, or Noctis variants",
        category: OptionCategory::Appearance,
    },
    OptionSpec {
        name: "breadcrumb-enabled",
        label: "Show breadcrumb",
        kind: OptionKind::Bool,
        choices: &[],
        default: "true",
        description: "Show the path breadcrumb under the tab bar",
        category: OptionCategory::Appearance,
    },
    OptionSpec {
        name: "minimap-enabled",
        label: "Show minimap",
        kind: OptionKind::Bool,
        choices: &[],
        default: "false",
        description: "Show the code minimap instead of the scrollbar",
        category: OptionCategory::Appearance,
    },
    OptionSpec {
        name: "number",
        label: "Line numbers",
        kind: OptionKind::Bool,
        choices: &[],
        default: "true",
        description: "Show line numbers in the gutter",
        category: OptionCategory::Editing,
    },
    OptionSpec {
        name: "scrolloff",
        label: "Scroll offset",
        kind: OptionKind::Int,
        choices: &[],
        default: "3",
        description: "Lines of context kept around the cursor while scrolling",
        category: OptionCategory::Editing,
    },
    OptionSpec {
        name: "mouse-support",
        label: "Mouse support",
        kind: OptionKind::Bool,
        choices: &[],
        default: "true",
        description: "Click, drag, and wheel support in the editor pane",
        category: OptionCategory::Editing,
    },
    OptionSpec {
        name: "auto-save",
        label: "Auto-save",
        kind: OptionKind::Bool,
        choices: &[],
        default: "false",
        description: "Write modified buffers to disk every 30 seconds (opt-in)",
        category: OptionCategory::Behavior,
    },
    OptionSpec {
        name: "agent.provider",
        label: "Agent provider",
        kind: OptionKind::Enum,
        choices: &["openai", "deepseek", "kimi", "zai"],
        default: "openai",
        description: "LLM provider for the built-in agent",
        category: OptionCategory::Behavior,
    },
    OptionSpec {
        name: "plugins-enabled",
        label: "Enable plugins",
        kind: OptionKind::Bool,
        choices: &[],
        default: "true",
        description: "Load plugins from disk at startup (bundled and user directories)",
        category: OptionCategory::Behavior,
    },
    OptionSpec {
        name: "plugins-root",
        label: "Plugins directory",
        kind: OptionKind::Str,
        choices: &[],
        // Empty means unset: the fallback (~/.config/magma/plugins) is
        // computed at use time because $HOME is not a compile-time constant.
        default: "",
        description: "Directory plugins are installed to and loaded from (empty uses ~/.config/magma/plugins)",
        category: OptionCategory::Behavior,
    },
    // Managed by the per-plugin toggles on the Plugins settings page; shown
    // as a raw row nowhere.
    OptionSpec {
        name: "plugins-disabled",
        label: "Disabled plugins",
        kind: OptionKind::Str,
        choices: &[],
        default: "",
        description: "Plugin names that stay off until the next startup (comma-separated)",
        category: OptionCategory::Behavior,
    },
];

/// The option's current value: user-set, else the registry default.
pub fn effective(editor: &Editor, spec: &OptionSpec) -> String {
    editor
        .options
        .get(spec.name)
        .cloned()
        .unwrap_or_else(|| spec.default.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn specs_are_unique_and_documented() {
        let mut names: Vec<&str> = OPTION_SPECS.iter().map(|s| s.name).collect();
        let count = names.len();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), count, "duplicate option name in the registry");
        for spec in OPTION_SPECS {
            assert!(
                !spec.description.is_empty(),
                "{} needs a description",
                spec.name
            );
            // Str options may default to "" (unset); their description
            // documents the computed fallback.
            if spec.kind != OptionKind::Str {
                assert!(!spec.default.is_empty(), "{} needs a default", spec.name);
            }
            if spec.kind == OptionKind::Enum {
                assert!(
                    spec.choices.contains(&spec.default),
                    "{} default must be a choice",
                    spec.name
                );
            }
        }
    }

    #[test]
    fn effective_falls_back_to_default() {
        let ed = Editor::new(Box::new(crate::storage::disk::DiskFileSystem::new()));
        let autosave = OPTION_SPECS.iter().find(|s| s.name == "auto-save").unwrap();
        assert_eq!(effective(&ed, autosave), "false");
    }
}
