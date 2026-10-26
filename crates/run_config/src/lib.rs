//! Run configurations — named, editable launch definitions scoped to a
//! project.  A config spawns a program with arguments, a working directory,
//! and an environment; `build_before_run` names another config to complete
//! first.
//!
//! This crate owns the config shape and the detection-seeded defaults.  The
//! manager that loads/saves them, launches tasks, and chains a build step
//! lives in the editor and needs the task scheduler and the background
//! handle.

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunConfig {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    /// Working directory; relative paths resolve against the project root.
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Name of another config to run to completion first; this config only
    /// launches when that one exits 0.
    #[serde(default)]
    pub build_before_run: Option<String>,
    /// Whether a debug session can launch from this config.
    #[serde(default)]
    pub debuggable: bool,
}

impl RunConfig {
    /// The DAP `launch` request arguments for this configuration, with a
    /// relative `cwd` resolved against the project `root`.
    pub fn to_launch_args(&self, root: &Path) -> serde_json::Value {
        serde_json::json!({
            "request": "launch",
            "program": self.command,
            "args": self.args,
            "cwd": resolve_cwd(self, root).to_string_lossy(),
            "name": self.name,
        })
    }
}

/// Resolve a config's working directory against the project root.  An
/// absent or blank `cwd` means the root itself; an absolute `cwd` is used
/// as-is.
pub fn resolve_cwd(cfg: &RunConfig, root: &Path) -> PathBuf {
    match cfg.cwd.as_deref().map(str::trim) {
        None | Some("") => root.to_path_buf(),
        Some(cwd) => {
            let p = Path::new(cwd);
            if p.is_absolute() { p.to_path_buf() } else { root.join(p) }
        }
    }
}

fn mk(name: &str, command: &str, args: &[&str], debuggable: bool) -> RunConfig {
    RunConfig {
        name: name.to_string(),
        command: command.to_string(),
        args: args.iter().map(|s| s.to_string()).collect(),
        cwd: None,
        env: HashMap::new(),
        build_before_run: None,
        debuggable,
    }
}

/// Default build/test/run configs for a detected language.  Commands that
/// build themselves (`cargo run`, `npm start`, `make run`) do not chain a
/// build-first step.
pub fn defaults_for(language: &str) -> Vec<RunConfig> {
    match language {
        "rust" => vec![
            mk("build", "cargo", &["build"], false),
            mk("test", "cargo", &["test"], false),
            mk("run", "cargo", &["run"], true),
        ],
        "go" => vec![
            mk("build", "go", &["build", "./..."], false),
            mk("test", "go", &["test", "./..."], false),
            mk("run", "go", &["run", "."], false),
        ],
        "node" => vec![
            mk("build", "npm", &["run", "build"], false),
            mk("test", "npm", &["test"], false),
            mk("run", "npm", &["start"], false),
        ],
        "make" => vec![
            mk("build", "make", &[], false),
            mk("test", "make", &["test"], false),
            mk("run", "make", &["run"], false),
        ],
        "python" => vec![
            mk("test", "python", &["-m", "pytest"], false),
            mk("run", "python", &["-m", "main"], false),
        ],
        "cmake" => vec![
            mk("build", "cmake", &["--build", "build"], false),
            mk("test", "ctest", &["--test-dir", "build"], false),
        ],
        "maven" => vec![
            mk("build", "mvn", &["compile"], false),
            mk("test", "mvn", &["test"], false),
            mk("run", "mvn", &["exec:java"], false),
        ],
        "gradle" => vec![
            mk("build", "gradle", &["build"], false),
            mk("test", "gradle", &["test"], false),
            mk("run", "gradle", &["run"], false),
        ],
        _ => Vec::new(),
    }
}
