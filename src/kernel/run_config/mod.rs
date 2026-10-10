//! Run configurations — named, editable launch definitions scoped to a
//! project.  A config spawns a program with arguments, a working directory,
//! and an environment through the TaskScheduler (streaming, exit codes,
//! cancellation); `build_before_run` chains another config first and only
//! launches on success.  Configs persist per project under
//! `<root>/.magma/run-configs.json`; detection seeds in-memory defaults for
//! recognized marker files.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub mod defaults;

use crate::kernel::event::keys;
use crate::kernel::event::payload::TaskStartedPayload;
use crate::kernel::runtime::BackgroundHandle;
use crate::kernel::state::{Editor, MessageKind};
use crate::kernel::task::TaskScheduler;

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

#[derive(Debug, Clone, Default)]
pub struct RunConfigManager {
    pub configs: Vec<RunConfig>,
    pub active: Option<String>,
    /// (task id of the in-flight build step, config waiting to launch).
    pub pending: Option<(u64, String)>,
}

impl RunConfigManager {
    pub fn named(&self, name: &str) -> Option<&RunConfig> {
        self.configs.iter().find(|c| c.name == name)
    }

    pub fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.configs.iter().map(|c| c.name.clone()).collect();
        names.sort();
        names
    }

    pub fn upsert(&mut self, config: RunConfig) {
        match self.configs.iter_mut().find(|c| c.name == config.name) {
            Some(slot) => *slot = config,
            None => self.configs.push(config),
        }
    }

    pub fn remove(&mut self, name: &str) {
        self.configs.retain(|c| c.name != name);
        if self.active.as_deref() == Some(name) {
            self.active = self.configs.first().map(|c| c.name.clone());
        }
    }

    fn config_file(root: &Path) -> PathBuf {
        root.join(".magma").join("run-configs.json")
    }

    /// Read `<root>/.magma/run-configs.json` into the store; false when the
    /// file is missing or unreadable.
    pub fn load(&mut self, root: &Path) -> bool {
        let Ok(text) = std::fs::read_to_string(Self::config_file(root)) else { return false };
        #[derive(Deserialize)]
        struct File {
            #[serde(default)]
            configs: Vec<RunConfig>,
            #[serde(default)]
            active: Option<String>,
        }
        let Ok(file) = serde_json::from_str::<File>(&text) else { return false };
        self.configs = file.configs;
        self.active = file.active.filter(|a| self.named(a).is_some());
        !self.configs.is_empty()
    }

    pub fn save(&self, root: &Path) -> Result<(), String> {
        let path = Self::config_file(root);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("create .magma: {e}"))?;
        }
        #[derive(Serialize)]
        struct File<'a> {
            configs: &'a [RunConfig],
            active: &'a Option<String>,
        }
        let text = serde_json::to_string_pretty(&File {
            configs: &self.configs,
            active: &self.active,
        }).map_err(|e| e.to_string())?;
        std::fs::write(&path, text + "\n").map_err(|e| format!("write {}: {e}", path.display()))
    }

    /// Load the project's saved configs, or seed in-memory defaults for the
    /// detected language when none exist.  Defaults are not written until a
    /// user edit saves them.
    pub fn load_or_defaults(&mut self, root: &Path, language: &str) {
        if self.load(root) {
            return;
        }
        self.configs = defaults_for(language);
        self.active = self.configs.first().map(|c| c.name.clone());
    }

    /// Launch a config by name; with `build_before_run`, launches that config
    /// first and queues this one to start when the build exits 0.
    pub fn launch(
        &mut self,
        name: &str,
        scheduler: &mut TaskScheduler,
        bg: &BackgroundHandle,
        root: &Path,
    ) -> Result<u64, String> {
        let cfg = self.named(name)
            .ok_or_else(|| format!("no run configuration named '{name}'"))?
            .clone();
        if let Some(dep_name) = cfg.build_before_run.clone() {
            if self.pending.is_some() {
                return Err("another run is still waiting for its build".to_string());
            }
            if dep_name == cfg.name {
                return Err(format!("run configuration '{}' cannot build itself", cfg.name));
            }
            let dep = self.named(&dep_name)
                .ok_or_else(|| format!("build-first config '{dep_name}' not found"))?
                .clone();
            let dep_id = Self::launch_task(&dep, scheduler, bg, root)?;
            self.pending = Some((dep_id, cfg.name));
            Ok(dep_id)
        } else {
            Self::launch_task(&cfg, scheduler, bg, root)
        }
    }

    /// Launch a config's own task without its build-first dependency — the
    /// continuation path once that dependency already succeeded.
    pub fn launch_direct(
        &mut self,
        name: &str,
        scheduler: &mut TaskScheduler,
        bg: &BackgroundHandle,
        root: &Path,
    ) -> Result<u64, String> {
        let cfg = self.named(name)
            .ok_or_else(|| format!("no run configuration named '{name}'"))?
            .clone();
        Self::launch_task(&cfg, scheduler, bg, root)
    }

    /// The task name for a config is the config name, so task events, the
    /// Output pane, and `stop-config` all key off one string.
    fn launch_task(
        cfg: &RunConfig,
        scheduler: &mut TaskScheduler,
        bg: &BackgroundHandle,
        root: &Path,
    ) -> Result<u64, String> {
        if cfg.command.trim().is_empty() {
            return Err(format!("run configuration '{}' has no command", cfg.name));
        }
        let cwd = resolve_cwd(cfg, root);
        let id = scheduler.define(cfg.name.clone(), cfg.command.clone(), cfg.env.clone(), vec![]);
        let task = scheduler.tasks.get_mut(&id)
            .ok_or_else(|| format!("task {id} not found"))?;
        task.args = cfg.args.clone();
        task.cwd = Some(cwd);
        scheduler.run_by_name(&cfg.name, bg)
    }

    /// Stop the running task for a config.
    pub fn stop(&mut self, name: &str, scheduler: &mut TaskScheduler) -> Result<(), String> {
        let id = scheduler.task_by_name(name)
            .ok_or_else(|| format!("no run configuration named '{name}'"))?;
        self.pending = None;
        scheduler.cancel(id);
        Ok(())
    }
}

fn resolve_cwd(cfg: &RunConfig, root: &Path) -> PathBuf {
    match cfg.cwd.as_deref().map(str::trim) {
        None | Some("") => root.to_path_buf(),
        Some(cwd) => {
            let p = Path::new(cwd);
            if p.is_absolute() { p.to_path_buf() } else { root.join(p) }
        }
    }
}

pub use defaults::defaults_for;

/// Create or update a config from the editor dialog: validates, renames when
/// `original` differs, makes the saved config active, and persists to the
/// project's `.magma` store.
pub fn upsert_config(
    ed: &mut Editor,
    original: Option<String>,
    cfg: RunConfig,
) -> Result<String, String> {
    if cfg.name.trim().is_empty() {
        return Err("run configuration needs a name".to_string());
    }
    if cfg.command.trim().is_empty() {
        return Err(format!("run configuration '{}' needs a command", cfg.name));
    }
    if let Some(original) = original
        && original != cfg.name {
            ed.project_manager.run_configs.remove(&original);
        }
    let name = cfg.name.clone();
    ed.project_manager.run_configs.upsert(cfg);
    ed.project_manager.run_configs.active = Some(name.clone());
    save_active(ed)
}

fn save_active(ed: &mut Editor) -> Result<String, String> {
    let Some(root) = ed.project_manager.project.root.clone() else {
        return Err("no project open".to_string());
    };
    let name = ed.project_manager.run_configs.active.clone()
        .unwrap_or_default();
    ed.project_manager.run_configs.save(&root)?;
    Ok(name)
}

/// TaskRunCompleted continuation: when the finished task was a build step,
/// launch the config waiting on it (only on success).
pub fn continue_pending(ed: &mut Editor, task_id: u64, exit_code: i32) {
    let Some((dep_id, name)) = ed.project_manager.run_configs.pending.clone() else { return };
    if dep_id != task_id {
        return;
    }
    ed.project_manager.run_configs.pending = None;
    if exit_code != 0 {
        ed.show_message(MessageKind::Error,
            format!("'{name}' not started: build exited with {exit_code}"));
        return;
    }
    let Some(bg) = ed.background.clone() else { return };
    let Some(root) = ed.project_manager.project.root.clone() else { return };
    match ed.project_manager.run_configs.launch_direct(&name, &mut ed.task_scheduler, &bg, &root) {
        Ok(id) => announce_launch(ed, id),
        Err(e) => ed.show_message(MessageKind::Error, e),
    }
}

/// Register a launched task with the work scheduler and announce it.  The
/// announced name is the launched task's — with `build_before_run` that is
/// the build step, not the config the user asked for.
pub fn announce_launch(ed: &mut Editor, id: u64) {
    let name = ed.task_scheduler.tasks.get(&id)
        .map(|t| t.name.clone())
        .unwrap_or_default();
    ed.scheduler.register_task(id, &name);
    ed.events.emit_typed(keys::events::TASK_STARTED, TaskStartedPayload {
        id: id.to_string(),
        name,
    });
}
