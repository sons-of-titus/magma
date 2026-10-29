//! Project templates — the registry of plugin-contributed project
//! generators and the state of in-flight generations.
//!
//! Plugins declare templates through the `template/register` Janet API (see
//! `crates/scripting/src/template_api.rs`); the registry here is pure Rust
//! state so the New Project wizard and the `new-project` command can both
//! read it without a scripting runtime.  The rendering engine lives in
//! `generate.rs`.

pub mod generate;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// The kind of one user input a template asks for.  All values travel as
/// strings; the kind drives validation and the wizard's editor widget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputKind {
    Text,
    /// A safe directory/file name component: `[A-Za-z0-9_-]+`.
    ProjectName,
    /// One of the listed options, validated exactly.
    Choice(Vec<String>),
    /// Rendered as `"true"`/`"false"`.
    Boolean,
}

#[derive(Debug, Clone)]
pub struct TemplateInput {
    pub key: String,
    pub kind: InputKind,
    pub required: bool,
    pub default: Option<String>,
    pub description: String,
}

/// A registered project template.  `source` is an absolute directory of
/// files copied into every generated project; `{{variable}}` placeholders in
/// file contents and path segments are substituted at generation time.
#[derive(Debug, Clone)]
pub struct ProjectTemplate {
    /// Namespaced id, `<plugin>.<name>` (e.g. `"rust.binary"`).
    pub id: String,
    pub name: String,
    pub description: String,
    /// Free-form grouping label for the wizard ("application", "library", …).
    pub category: String,
    pub language: String,
    /// Absolute directory the template files are copied from.
    pub source: PathBuf,
    pub variables: Vec<TemplateInput>,
    /// Toolchain id this template's projects build with; the wizard shows
    /// its availability (and, through the toolchain, the debug adapter)
    /// before creating anything.
    pub toolchain: Option<String>,
    /// Files the template copies, counted at registration so the wizard can
    /// show a summary without touching the filesystem.
    pub file_count: usize,
    /// The plugin that registered this template; unloading the plugin
    /// removes its templates.
    pub owner: String,
}

#[derive(Debug, Default)]
pub struct TemplateRegistry {
    templates: HashMap<String, ProjectTemplate>,
}

impl TemplateRegistry {
    pub fn register(&mut self, template: ProjectTemplate) -> Result<(), String> {
        validate_id(&template.id)?;
        if template.name.is_empty() {
            return Err(format!("template {}: :name is required", template.id));
        }
        if !template.source.is_dir() {
            return Err(format!(
                "template {}: source directory {} does not exist",
                template.id,
                template.source.display()
            ));
        }
        for input in &template.variables {
            validate_input_key(&template.id, input)?;
        }
        if let Some(existing) = self.templates.get(&template.id)
            && existing.owner != template.owner
        {
            return Err(format!(
                "template id {} is already registered by plugin {}",
                template.id, existing.owner
            ));
        }
        let mut template = template;
        template.file_count = generate::count_files(&template.source);
        self.templates.insert(template.id.clone(), template);
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&ProjectTemplate> {
        self.templates.get(id)
    }

    /// All templates sorted by (category, id) — the wizard's display order.
    pub fn list(&self) -> Vec<&ProjectTemplate> {
        let mut out: Vec<&ProjectTemplate> = self.templates.values().collect();
        out.sort_by(|a, b| (&a.category, &a.id).cmp(&(&b.category, &b.id)));
        out
    }

    pub fn count(&self) -> usize {
        self.templates.len()
    }

    /// Remove every template registered by `owner`; returns how many.
    pub fn remove_owner(&mut self, owner: &str) -> usize {
        let before = self.templates.len();
        self.templates.retain(|_, t| t.owner != owner);
        before - self.templates.len()
    }
}

fn validate_id(id: &str) -> Result<(), String> {
    let valid = !id.is_empty()
        && id.len() <= 128
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '-' | '_'))
        && id.contains('.');
    if valid {
        Ok(())
    } else {
        Err(format!(
            "template id {id:?} is invalid: ids are `<plugin>.<name>` in lowercase letters, digits, '.', '-' and '_'"
        ))
    }
}

fn validate_input_key(template_id: &str, input: &TemplateInput) -> Result<(), String> {
    let valid = !input.key.is_empty()
        && input
            .key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_');
    if valid {
        Ok(())
    } else {
        Err(format!(
            "template {template_id}: input key {:?} is invalid (letters, digits, '_' only)",
            input.key
        ))
    }
}

// ── In-flight generations ────────────────────────────────────────────────────

/// State of one generation run.  `cancel` is shared with the background
/// closure; the wizard cancels through `new-project-cancel`.
#[derive(Debug)]
pub struct GenerationJob {
    pub request_id: u64,
    pub template_id: String,
    pub name: String,
    pub dest: PathBuf,
    pub open_on_success: bool,
    pub cancel: Arc<AtomicBool>,
}

#[derive(Debug, Default)]
pub struct TemplateJobs {
    pub generating: Option<GenerationJob>,
    pub last_result: Option<Result<GenerationSummary, String>>,
    next_request_id: u64,
}

impl TemplateJobs {
    /// Start tracking a generation; returns the request id and the cancel
    /// flag to hand to the background closure.
    pub fn begin(
        &mut self,
        template_id: &str,
        name: &str,
        dest: PathBuf,
        open_on_success: bool,
    ) -> (u64, Arc<AtomicBool>) {
        self.next_request_id += 1;
        let cancel = Arc::new(AtomicBool::new(false));
        self.generating = Some(GenerationJob {
            request_id: self.next_request_id,
            template_id: template_id.to_string(),
            name: name.to_string(),
            dest,
            open_on_success,
            cancel: Arc::clone(&cancel),
        });
        self.last_result = None;
        (self.next_request_id, cancel)
    }

    pub fn is_generating(&self) -> bool {
        self.generating.is_some()
    }

    pub fn cancel(&mut self) -> bool {
        match &self.generating {
            Some(job) => {
                job.cancel.store(true, Ordering::Relaxed);
                true
            }
            None => false,
        }
    }
}

/// What a successful generation produced.
#[derive(Debug, Clone)]
pub struct GenerationSummary {
    pub template_id: String,
    pub name: String,
    pub dest: PathBuf,
    /// Files created, relative to `dest`, sorted.
    pub files: Vec<String>,
}

/// Background events for the template subsystem.
#[derive(Debug, Clone)]
pub enum TemplateEvent {
    Generated {
        request_id: u64,
        open: bool,
        result: Result<GenerationSummary, String>,
    },
}
