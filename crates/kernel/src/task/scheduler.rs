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
    /// When non-empty, `command` spawns directly with these arguments
    /// instead of through `sh -c`.
    pub args: Vec<String>,
    /// Working directory for the child process.
    pub cwd: Option<std::path::PathBuf>,
    pub environment: HashMap<String, String>,
    pub dependencies: Vec<TaskId>,
    pub status: TaskStatus,
    pub output: Option<TaskOutput>,
}

/// Where a task's streamed output lands: the buffer that shows the run as
/// it progresses, and whether finishing it feeds the quickfix list (`make`).
#[derive(Debug, Clone)]
pub struct OutputSink {
    pub buffer_key: usize,
    pub parse_quickfix: bool,
    /// The command that produced this output — input to the tool-output
    /// diagnostic parser.
    pub command: String,
}

/// Orchestrates task definitions, dependency resolution, and concurrent execution.
#[derive(Default)]
pub struct TaskScheduler {
    pub tasks: HashMap<TaskId, Task>,
    /// Name → id mapping for named tasks.
    pub named: HashMap<String, TaskId>,
    /// Kill handles for running children — `cancel` must actually stop
    /// the process, not only flip a status flag (rework.md §6).
    pub running: HashMap<TaskId, std::sync::Arc<std::sync::Mutex<Option<std::process::Child>>>>,
    /// Tasks whose output streams into a dedicated output buffer.
    pub output_sinks: HashMap<TaskId, OutputSink>,
    pub next_id: u64,
}

impl TaskScheduler {
    pub fn new() -> Self {
        TaskScheduler {
            tasks: HashMap::new(),
            named: HashMap::new(),
            running: HashMap::new(),
            output_sinks: HashMap::new(),
            next_id: 1,
        }
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
        self.tasks.insert(
            id,
            Task {
                id,
                name,
                command,
                args: Vec::new(),
                cwd: None,
                environment,
                dependencies,
                status: TaskStatus::Pending,
                output: None,
            },
        );
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
            self.tasks
                .get(dep_id)
                .map(|t| t.status == TaskStatus::Completed)
                .unwrap_or(true)
        })
    }

    /// Spawn the task on the background thread pool.  A finished task may be
    /// launched again (status resets to Pending); only a Running task
    /// refuses.  The background thread sends
    /// `BackgroundEvent::Task(task::Event::RunCompleted)` when done.
    pub fn launch(
        &mut self,
        id: TaskId,
        bg: &crate::runtime::BackgroundHandle,
    ) -> Result<(), String> {
        let task = self
            .tasks
            .get_mut(&id)
            .ok_or_else(|| format!("task {id} not found"))?;
        match task.status {
            TaskStatus::Running => {
                return Err(format!("task {id} is already running"));
            }
            _ => {
                task.status = TaskStatus::Pending;
                task.output = None;
            }
        }
        task.status = TaskStatus::Running;

        let command = task.command.clone();
        let args = task.args.clone();
        let cwd = task.cwd.clone();
        let environment = task.environment.clone();
        let sender = bg.sender.clone();
        let child_slot: std::sync::Arc<std::sync::Mutex<Option<std::process::Child>>> =
            std::sync::Arc::new(std::sync::Mutex::new(None));
        self.running.insert(id, child_slot.clone());

        bg.spawn_blocking(move || {
            let start = Instant::now();
            let mut cmd = if args.is_empty() {
                let mut c = std::process::Command::new("sh");
                c.arg("-c").arg(&command);
                c
            } else {
                let mut c = std::process::Command::new(&command);
                c.args(&args);
                c
            };
            if let Some(cwd) = cwd {
                cmd.current_dir(cwd);
            }
            for (k, v) in &environment {
                cmd.env(k, v);
            }
            cmd.stdout(std::process::Stdio::piped());
            cmd.stderr(std::process::Stdio::piped());
            // Fresh process group: cancelling a task takes its whole
            // subtree (sh -c children, make grandchildren) down with it.
            crate::util::proctree::own_process_group(&mut cmd);

            let (exit_code, stdout, stderr) = match cmd.spawn() {
                Ok(child) => {
                    *child_slot.lock().unwrap_or_else(|e| e.into_inner()) = Some(child);
                    // Stream output line by line — long builds show
                    // progress as they run instead of at completion.
                    fn stream_lines<R: std::io::Read + Send + 'static>(
                        stream: Option<R>,
                        stream_name: &'static str,
                        id: u64,
                        sender: tokio::sync::mpsc::UnboundedSender<crate::runtime::BackgroundEvent>,
                    ) -> std::thread::JoinHandle<String> {
                        let Some(s) = stream else {
                            return std::thread::spawn(String::new);
                        };
                        std::thread::spawn(move || {
                            use std::io::BufRead;
                            let mut sink = String::new();
                            for line in std::io::BufReader::new(s).lines().map_while(Result::ok) {
                                sink.push_str(&line);
                                sink.push('\n');
                                let _ = sender.send(crate::runtime::BackgroundEvent::Task(
                                    super::Event::OutputLine {
                                        id,
                                        stream: stream_name.into(),
                                        line,
                                    },
                                ));
                            }
                            sink
                        })
                    }
                    // Both pipes drain concurrently: reading stdout to EOF
                    // first deadlocks against a child that fills its stderr
                    // pipe (the OS buffer is ~64 KiB) while producing no
                    // stdout.
                    let mut slot = child_slot.lock().unwrap_or_else(|e| e.into_inner());
                    let (out, err) = if let Some(child) = slot.as_mut() {
                        (child.stdout.take(), child.stderr.take())
                    } else {
                        (None, None)
                    };
                    drop(slot);
                    let out_handle = stream_lines(out, "stdout", id, sender.clone());
                    let err_handle = stream_lines(err, "stderr", id, sender.clone());
                    let out = out_handle.join().unwrap_or_default();
                    let err = err_handle.join().unwrap_or_default();
                    let code = child_slot
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .as_mut()
                        .and_then(|c| c.wait().ok())
                        .and_then(|s| s.code())
                        .unwrap_or(-1);
                    (code, out, err)
                }
                Err(e) => (-1, String::new(), e.to_string()),
            };
            let duration_ms = start.elapsed().as_millis() as u64;
            let _ = sender.send(crate::runtime::BackgroundEvent::Task(
                super::Event::RunCompleted {
                    id,
                    exit_code,
                    stdout,
                    stderr,
                    duration_ms,
                },
            ));
        });

        Ok(())
    }

    /// Run a task by name; returns its ID.  Errors if the name is unknown or
    /// if dependencies are not yet satisfied.
    pub fn run_by_name(
        &mut self,
        name: &str,
        bg: &crate::runtime::BackgroundHandle,
    ) -> Result<TaskId, String> {
        let id = self
            .task_by_name(name)
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
        bg: &crate::runtime::BackgroundHandle,
    ) -> Result<(), String> {
        if !self.dependencies_satisfied(id) {
            return Err(format!("task {id} has unsatisfied dependencies"));
        }
        self.launch(id, bg)
    }

    /// Cancel a task: kill its process and mark it Cancelled.  The
    /// background thread's completion event lands on a non-Running task
    /// and is ignored by the completion handler.
    pub fn cancel(&mut self, id: TaskId) {
        if let Some(slot) = self.running.get(&id)
            && let Ok(mut guard) = slot.lock()
            && let Some(child) = guard.as_mut()
        {
            crate::util::proctree::kill_tree_hard(child.id() as i32);
            let _ = child.kill();
        }
        self.running.remove(&id);
        if let Some(task) = self.tasks.get_mut(&id)
            && matches!(task.status, TaskStatus::Running | TaskStatus::Pending)
        {
            task.status = TaskStatus::Cancelled;
        }
    }

    /// Called from `process_background_event` when a task exits with code 0.
    pub fn mark_completed(&mut self, id: TaskId, output: TaskOutput) {
        if let Some(task) = self.tasks.get_mut(&id)
            && task.status == TaskStatus::Running
        {
            task.status = TaskStatus::Completed;
            task.output = Some(output);
        }
    }

    /// Called from `process_background_event` when a task exits with non-zero code.
    pub fn mark_failed(&mut self, id: TaskId, error: String, output: TaskOutput) {
        if let Some(task) = self.tasks.get_mut(&id)
            && task.status == TaskStatus::Running
        {
            task.status = TaskStatus::Failed(error);
            task.output = Some(output);
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
        self.tasks
            .values()
            .filter(|t| t.status == TaskStatus::Pending && self.dependencies_satisfied(t.id))
            .map(|t| t.id)
            .collect()
    }
}
