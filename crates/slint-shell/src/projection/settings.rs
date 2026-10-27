//! Settings page projections from the kernel option registry.

use kernel::state::Editor;
use kernel::state::options::{OPTION_SPECS, OptionCategory, OptionKind};
use slint::{ModelRc, SharedString, VecModel};

use crate::SettingsRow;

/// Registry-backed rows for one settings page.  `agent.provider` lives on
/// the Agent page rather than Behavior; the Advanced page lists custom
/// options (never secrets).
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
            name: spec.name.into(),
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
        2 => {
            rows.extend(
                OPTION_SPECS
                    .iter()
                    .filter(|s| {
                        s.category == OptionCategory::Behavior && !s.name.starts_with("agent.")
                    })
                    .map(from_spec),
            );
        }
        3 => {
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
                OPTION_SPECS.iter().any(|s| s.name == n) || n.starts_with("agent.api-key")
            };
            let mut names: Vec<&String> = ed.options.keys().filter(|n| !registered(n)).collect();
            names.sort();
            rows.extend(names.into_iter().map(|name| SettingsRow {
                name: name.clone().into(),
                description: "custom option".into(),
                kind: "str".into(),
                value: ed.options.get(name).cloned().unwrap_or_default().into(),
                int_value: 0,
                choices: ModelRc::new(VecModel::from(Vec::<SharedString>::new())),
                modified: true,
            }));
        }
    }
    rows.retain(|r| want(r.name.as_str(), r.description.as_str()));
    rows
}
