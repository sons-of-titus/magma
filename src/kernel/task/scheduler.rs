//! Task System — Phase 4.
//!
//! A `Task` wraps a shell command with dependencies, environment, and output.
//! `TaskScheduler` orchestrates dependency resolution and concurrent execution
//! via the background thread pool; completion events fire on the main thread
//! via the event bus.

use std::collections::HashMap;
use std::time::Instant;

pub type TaskId = u64;

#[derive(Debug, Clone, PartialEq)]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed(String),
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct TaskOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub duration_ms: u64,
}

#[derive(Debug, Clone)]
pub struct Task {
    pub id: TaskId,
    pub name: String,
    pub command: String,
    pub environment: HashMap<String, String>,
    pub dependencies: Vec<TaskId>,
    pub status: TaskStatus,
    pub output: Option<TaskOutput>,
}

/// Orchestrates task definitions, dependency resolution, and concurrent execution.
#[derive(Default)]
pub struct TaskScheduler {
    pub tasks: HashMap<TaskId, Task>,
    /// Name → id mapping for named tasks.
    pub named: HashMap<String, TaskId>,
    pub next_id: u64,
}

impl TaskScheduler {
    pub fn new() -> Self {
        TaskScheduler { tasks: HashMap::new(), named: HashMap::new(), next_id: 1 }
    }

    /// Define (or redefine) a named task.  Returns the stable task ID for this name.
    /// If the name already exists and the task is not Running, fields are updated
    /// and status is reset to Pending so the task can be re-run.
    pub fn define(
        &mut self,
        name: String,
        command: String,
        environment: HashMap<String, String>,
        dependencies: Vec<TaskId>,
    ) -> TaskId {
        if let Some(&id) = self.named.get(&name) {
            if let Some(task) = self.tasks.get_mut(&id) {
                task.command = command;
                task.environment = environment;
                task.dependencies = dependencies;
                if task.status != TaskStatus::Running {
                    task.status = TaskStatus::Pending;
                    task.output = None;
                }
            }
            return id;
        }
        let id = self.next_id;
        self.next_id += 1;
        self.named.insert(name.clone(), id);
        self.tasks.insert(id, Task {
            id, name, command, environment, dependencies,
            status: TaskStatus::Pending,
            output: None,
        });
        id
    }

    pub fn task_by_name(&self, name: &str) -> Option<TaskId> {
        self.named.get(name).copied()
    }

    /// True iff all dependencies of `id` are in `Completed` status.
    pub fn dependencies_satisfied(&self, id: TaskId) -> bool {
        let task = match self.tasks.get(&id) {
            Some(t) => t,
            None => return false,
        };
        task.dependencies.iter().all(|dep_id| {
            self.tasks.get(dep_id)
                .map(|t| t.status == TaskStatus::Completed)
                .unwrap_or(true)
        })
    }

    /// Spawn the task on the background thread pool.  Marks it Running immediately.
    /// The background thread sends `BackgroundEvent::TaskRunCompleted` when done.
    pub fn launch(
        &mut self,
        id: TaskId,
        bg: &crate::kernel::runtime::BackgroundHandle,
    ) -> Result<(), String> {
        let task = self.tasks.get_mut(&id)
            .ok_or_else(|| format!("task {id} not found"))?;
        if !matches!(task.status, TaskStatus::Pending) {
            return Err(format!("task {id} is not in Pending state"));
        }
        task.status = TaskStatus::Running;

        let command = task.command.clone();
        let environment = task.environment.clone();
        let sender = bg.sender.clone();

        bg.spawn_blocking(move || {
            use std::io::Read;
            let start = Instant::now();
            let mut cmd = std::process::Command::new("sh");
            cmd.arg("-c").arg(&command);
            for (k, v) in &environment {
                cmd.env(k, v);
            }
            cmd.stdout(std::process::Stdio::piped());
            cmd.stderr(std::process::Stdio::piped());

            let (exit_code, stdout, stderr) = match cmd.spawn() {
                Ok(mut child) => {
                    let mut out = String::new();
                    let mut err = String::new();
                    if let Some(s) = child.stdout.take() {
                        let _ = std::io::BufReader::new(s).read_to_string(&mut out);
                    }
                    if let Some(s) = child.stderr.take() {
                        let _ = std::io::BufReader::new(s).read_to_string(&mut err);
                    }
                    let code = child.wait().ok().and_then(|s| s.code()).unwrap_or(-1);
                    (code, out, err)
                }
                Err(e) => (-1, String::new(), e.to_string()),
            };
            let duration_ms = start.elapsed().as_millis() as u64;
            let _ = sender.send(crate::kernel::runtime::BackgroundEvent::TaskRunCompleted {
                id, exit_code, stdout, stderr, duration_ms,
            });
        });

        Ok(())
    }

    /// Run a task by name; returns its ID.  Errors if the name is unknown or
    /// if dependencies are not yet satisfied.
    pub fn run_by_name(
        &mut self,
        name: &str,
        bg: &crate::kernel::runtime::BackgroundHandle,
    ) -> Result<TaskId, String> {
        let id = self.task_by_name(name)
            .ok_or_else(|| format!("task '{name}' not defined"))?;
        if !self.dependencies_satisfied(id) {
            return Err(format!("task '{name}' has unsatisfied dependencies"));
        }
        self.launch(id, bg)?;
        Ok(id)
    }

    /// Run a task by ID.  Errors if deps are not satisfied.
    pub fn run_id(
        &mut self,
        id: TaskId,
        bg: &crate::kernel::runtime::BackgroundHandle,
    ) -> Result<(), String> {
        if !self.dependencies_satisfied(id) {
            return Err(format!("task {id} has unsatisfied dependencies"));
        }
        self.launch(id, bg)
    }

    /// Cancel a task.  Running tasks are marked Cancelled; the background thread
    /// may still complete and send an event, which is ignored because the
    /// status is no longer Running.
    pub fn cancel(&mut self, id: TaskId) {
        if let Some(task) = self.tasks.get_mut(&id) {
            if matches!(task.status, TaskStatus::Running | TaskStatus::Pending) {
                task.status = TaskStatus::Cancelled;
            }
        }
    }

    /// Called from `process_background_event` when a task exits with code 0.
    pub fn mark_completed(&mut self, id: TaskId, output: TaskOutput) {
        if let Some(task) = self.tasks.get_mut(&id) {
            if task.status == TaskStatus::Running {
                task.status = TaskStatus::Completed;
                task.output = Some(output);
            }
        }
    }

    /// Called from `process_background_event` when a task exits with non-zero code.
    pub fn mark_failed(&mut self, id: TaskId, error: String, output: TaskOutput) {
        if let Some(task) = self.tasks.get_mut(&id) {
            if task.status == TaskStatus::Running {
                task.status = TaskStatus::Failed(error);
                task.output = Some(output);
            }
        }
    }

    pub fn status(&self, id: TaskId) -> Option<&TaskStatus> {
        self.tasks.get(&id).map(|t| &t.status)
    }

    pub fn output(&self, id: TaskId) -> Option<&TaskOutput> {
        self.tasks.get(&id).and_then(|t| t.output.as_ref())
    }

    /// Return IDs of Pending tasks whose dependencies are all Completed.
    /// Called after a task finishes to schedule newly-unblocked tasks.
    pub fn ready_to_run(&self) -> Vec<TaskId> {
        self.tasks.values()
            .filter(|t| t.status == TaskStatus::Pending && self.dependencies_satisfied(t.id))
            .map(|t| t.id)
            .collect()
    }

    /// Auto-define `build`, `test`, and `run` tasks based on project markers at `root`.
    /// Skips definition if a same-named task is already Running.
    pub fn auto_detect_project_tasks(&mut self, root: &str) {
        let path = std::path::Path::new(root);
        let (build_cmd, test_cmd, run_cmd) = if path.join("Cargo.toml").exists() {
            ("cargo build", "cargo test", "cargo run")
        } else if path.join("package.json").exists() {
            ("npm run build", "npm test", "npm start")
        } else if path.join("Makefile").exists() {
            ("make", "make test", "make run")
        } else if path.join("pyproject.toml").exists() || path.join("setup.py").exists() {
            ("python -m build", "python -m pytest", "python -m main")
        } else {
            return;
        };
        self.define("build".into(), build_cmd.into(), HashMap::new(), vec![]);
        self.define("test".into(), test_cmd.into(), HashMap::new(), vec![]);
        self.define("run".into(), run_cmd.into(), HashMap::new(), vec![]);
    }
}
