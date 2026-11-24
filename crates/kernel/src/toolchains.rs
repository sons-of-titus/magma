//! Plugin-registered toolchains — declared build systems with detection
//! markers and named actions (build, test, run, …).
//!
//! A toolchain declaration doubles as a project-type detector: its marker
//! files recognize existing projects the same way `project::detect`'s
//! built-in table does, and its actions seed run configurations.  Declaring
//! a project type never requires the toolchain binary to be installed —
//! availability is checked separately through `project::toolchain::find`.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use crate::run_config::RunConfig;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolchainAction {
    pub program: String,
    pub args: Vec<String>,
    /// Project-relative working directory; None means the project root.
    pub cwd: Option<String>,
    pub env: HashMap<String, String>,
    /// Whether a debug session can launch from this action.
    pub debuggable: bool,
}

#[derive(Debug, Clone)]
pub struct ToolchainInfo {
    /// Namespaced id, `<plugin>.<name>` (e.g. `"rust.cargo"`).
    pub id: String,
    /// The language projects of this toolchain detect as.
    pub language: String,
    /// Marker files (relative to a project root) recognizing this
    /// toolchain's project type.
    pub detect_markers: Vec<String>,
    /// Named actions in registration order (BTreeMap keyed by action name
    /// keeps iteration deterministic; the names are conventional:
    /// build, test, run, clean, format, lint, …).
    pub actions: BTreeMap<String, ToolchainAction>,
    /// Debug adapter id this project type debugs with, consulted by the
    /// `debug-config` command when the built-in candidates do not apply.
    pub debug_adapter: Option<String>,
    /// The plugin that registered this toolchain.
    pub owner: String,
}

impl ToolchainInfo {
    /// True when at least one marker file exists at `root`.
    pub fn matches_root(&self, root: &Path) -> bool {
        !self.detect_markers.is_empty() && self.detect_markers.iter().any(|m| root.join(m).exists())
    }
}

/// Registration-order registry: earlier registrations win detection ties,
/// and plugin load order is the documented precedence.
#[derive(Debug, Default)]
pub struct ToolchainRegistry {
    toolchains: Vec<ToolchainInfo>,
}

impl ToolchainRegistry {
    pub fn register(&mut self, info: ToolchainInfo) -> Result<(), String> {
        if !valid_id(&info.id) {
            return Err(format!(
                "toolchain id {:?} is invalid: ids are `<plugin>.<name>` in lowercase letters, digits, '.', '-' and '_'",
                info.id
            ));
        }
        if info.language.is_empty() {
            return Err(format!("toolchain {}: :language is required", info.id));
        }
        if info.detect_markers.is_empty() {
            return Err(format!(
                "toolchain {}: at least one :detect marker file is required",
                info.id
            ));
        }
        if info.actions.is_empty() {
            return Err(format!(
                "toolchain {}: at least one :action is required",
                info.id
            ));
        }
        for (name, action) in &info.actions {
            if action.program.is_empty() {
                return Err(format!(
                    "toolchain {}: action {:?} has no :program",
                    info.id, name
                ));
            }
        }
        if let Some(existing) = self.get(&info.id)
            && existing.owner != info.owner
        {
            return Err(format!(
                "toolchain id {} is already registered by plugin {}",
                info.id, existing.owner
            ));
        }
        match self.toolchains.iter_mut().find(|t| t.id == info.id) {
            Some(slot) => *slot = info,
            None => self.toolchains.push(info),
        }
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&ToolchainInfo> {
        self.toolchains.iter().find(|t| t.id == id)
    }

    pub fn list(&self) -> &[ToolchainInfo] {
        &self.toolchains
    }

    pub fn count(&self) -> usize {
        self.toolchains.len()
    }

    /// The first registered toolchain whose markers exist at `root`.
    /// Built-in detection (project::detect) takes precedence over this —
    /// callers consult it only when the static table did not match, or to
    /// merge extra actions for an already-detected language.
    pub fn detect(&self, root: &Path) -> Option<&ToolchainInfo> {
        self.toolchains.iter().find(|t| t.matches_root(root))
    }

    /// Remove every toolchain registered by `owner`; returns how many.
    pub fn remove_owner(&mut self, owner: &str) -> usize {
        let before = self.toolchains.len();
        self.toolchains.retain(|t| t.owner != owner);
        before - self.toolchains.len()
    }
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '-' | '_'))
        && id.contains('.')
}

/// Detect the project type at `root` — built-in markers first, then
/// registered toolchains in registration order — and seed run
/// configurations from saved config, built-in defaults, and toolchain
/// actions filling the gaps.  Returns the detected language, if any.
pub fn detect_and_seed(
    root: &Path,
    toolchains: &[ToolchainInfo],
    configs: &mut crate::run_config::RunConfigManager,
) -> Option<String> {
    let static_language = crate::project::detect::detect_language(root);
    let toolchain = toolchains.iter().find(|t| t.matches_root(root));
    let language = static_language
        .map(str::to_string)
        .or_else(|| toolchain.map(|t| t.language.clone()));
    if let Some(lang) = &language {
        configs.load_or_defaults(root, lang);
    }
    if let Some(tc) = toolchain {
        merge_run_configs(&mut configs.configs, tc, configs.loaded_from_disk);
    }
    language
}

/// Merge a toolchain's actions into a project's run configurations without
/// clobbering anything: saved configs keep priority, and toolchain actions
/// fill the gaps (format, lint, clean, …).  One exception: a toolchain may
/// mark a built-in default debuggable — the static defaults cannot know
/// which languages have adapters.  Saved user edits are never touched.
pub fn merge_run_configs(
    configs: &mut Vec<RunConfig>,
    toolchain: &ToolchainInfo,
    saved_from_disk: bool,
) {
    for (name, action) in &toolchain.actions {
        if let Some(existing) = configs.iter_mut().find(|c| &c.name == name) {
            if !saved_from_disk && action.debuggable {
                existing.debuggable = true;
            }
            continue;
        }
        configs.push(RunConfig {
            name: name.clone(),
            command: action.program.clone(),
            args: action.args.clone(),
            cwd: action.cwd.clone(),
            env: action.env.clone(),
            build_before_run: None,
            debuggable: action.debuggable,
        });
    }
}
