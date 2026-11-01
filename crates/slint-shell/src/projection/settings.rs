//! Settings page projections from the kernel option registry.

use kernel::state::Editor;
use kernel::state::options::{OPTION_SPECS, OptionCategory, OptionKind};
use slint::{ModelRc, SharedString, VecModel};

use crate::{PluginRow, SettingsRow};

/// Registry-backed rows for one settings page.  The Agent and Plugins pages
/// select by name prefix rather than registry category; the Advanced page
/// lists custom options (never secrets).
pub fn settings_rows(ed: &Editor, category: i32, filter: &str) -> Vec<SettingsRow> {
    let filter = filter.trim().to_lowercase();
    let want = |name: &str, desc: &str| {
        filter.is_empty()
            || name.to_lowercase().contains(&filter)
            || desc.to_lowercase().contains(&filter)
    };

    let from_spec = |spec: &kernel::state::options::OptionSpec| {
        let value = kernel::state::options::effective(ed, spec);
        let int_value: i32 = value.parse().unwrap_or(0);
        SettingsRow {
            // Users read labels; the registry key stays available (meta row,
            // scripts) and is what the controls write back to.
            name: spec.label.into(),
            key: spec.name.into(),
            description: spec.description.into(),
            kind: match spec.kind {
                OptionKind::Bool => "bool",
                OptionKind::Int => "int",
                OptionKind::Enum => "enum",
                OptionKind::Str => "str",
            }
            .into(),
            value: value.into(),
            int_value,
            choices: ModelRc::new(VecModel::from(
                spec.choices
                    .iter()
                    .map(|c| SharedString::from(*c))
                    .collect::<Vec<_>>(),
            )),
            modified: ed.options.contains_key(spec.name),
        }
    };

    let mut rows: Vec<SettingsRow> = Vec::new();
    match category {
        // Appearance and Editing add their font rows directly in the view.
        0 => {
            rows.extend(
                OPTION_SPECS
                    .iter()
                    .filter(|s| s.category == OptionCategory::Appearance)
                    .map(from_spec),
            );
        }
        1 => {
            rows.extend(
                OPTION_SPECS
                    .iter()
                    .filter(|s| s.category == OptionCategory::Editing)
                    .map(from_spec),
            );
        }
        // Plugins options live on the Plugins page, `agent.*` on the Agent
        // page; `plugins-disabled` is backed by the per-plugin toggles and
        // shown as a raw row nowhere.
        2 => {
            rows.extend(
                OPTION_SPECS
                    .iter()
                    .filter(|s| {
                        s.category == OptionCategory::Behavior
                            && !s.name.starts_with("agent.")
                            && !s.name.starts_with("plugins-")
                    })
                    .map(from_spec),
            );
        }
        3 => {
            rows.extend(
                OPTION_SPECS
                    .iter()
                    .filter(|s| s.name.starts_with("plugins-") && s.name != "plugins-disabled")
                    .map(from_spec),
            );
        }
        4 => {
            rows.extend(
                OPTION_SPECS
                    .iter()
                    .filter(|s| s.name.starts_with("agent."))
                    .map(from_spec),
            );
        }
        _ => {
            // Advanced: every option that the registry does not know about.
            let registered = |n: &str| {
                OPTION_SPECS.iter().any(|s| s.name == n)
                    || n.starts_with("agent.api-key")
                    || n.starts_with("font.")
            };
            let mut names: Vec<&String> = ed.options.keys().filter(|n| !registered(n)).collect();
            names.sort();
            rows.extend(names.into_iter().map(|name| SettingsRow {
                name: name.clone().into(),
                key: name.clone().into(),
                description: "custom option".into(),
                kind: "str".into(),
                value: ed.options.get(name).cloned().unwrap_or_default().into(),
                int_value: 0,
                choices: ModelRc::new(VecModel::from(Vec::<SharedString>::new())),
                modified: true,
            }));
        }
    }
    rows.retain(|r| {
        want(r.name.as_str(), r.description.as_str())
            || (!r.key.is_empty() && want(r.key.as_str(), ""))
    });
    rows
}

/// One row per registered plugin for the Plugins page.  `enabled` mirrors
/// the `plugins-disabled` option — the toggle writes it, startup reads it.
pub fn plugin_rows(ed: &Editor) -> Vec<PluginRow> {
    let disabled = disabled_plugins(ed);
    ed.plugin_registry
        .list()
        .iter()
        .map(|p| PluginRow {
            name: p.name.clone().into(),
            version: p.version.clone().into(),
            entry: p.entry_point.clone().into(),
            loaded: p.loaded,
            enabled: !disabled.iter().any(|n| n == &p.name),
            capabilities: ed
                .extension_registry
                .get(&p.name)
                .map(|m| {
                    m.capabilities
                        .iter()
                        .map(|c| c.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default()
                .into(),
        })
        .collect()
}

/// The comma-separated `plugins-disabled` option, parsed.
pub fn disabled_plugins(ed: &Editor) -> Vec<String> {
    ed.options
        .get("plugins-disabled")
        .map(|v| {
            v.split(',')
                .map(str::trim)
                .filter(|n| !n.is_empty())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}
