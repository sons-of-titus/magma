//! Template rendering — copies a template's source tree into the
//! destination, substituting `{{variable}}` placeholders in file contents
//! and path segments.
//!
//! Everything a plugin declares is untrusted input: values never touch a
//! shell, symlinks are refused, every rendered path segment is re-checked,
//! and the destination may not escape or contain the template source.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use super::{GenerationSummary, InputKind, ProjectTemplate};

/// Validate user input against the template's declared inputs.  Extra
/// undeclared keys are dropped with a warning-free pass (templates may rely
/// on the implicit `project_name`), missing required keys are an error.
pub fn validate_inputs(
    template: &ProjectTemplate,
    vars: &HashMap<String, String>,
) -> Result<(), String> {
    for input in &template.variables {
        let value = vars.get(&input.key);
        match value {
            None if input.required && input.default.is_none() => {
                return Err(format!(
                    "template {}: missing required input {:?}",
                    template.id, input.key
                ));
            }
            Some(value) if value.is_empty() && input.required && input.default.is_none() => {
                return Err(format!(
                    "template {}: missing required input {:?}",
                    template.id, input.key
                ));
            }
            Some(value) => check_value(template, input, value)?,
            None => {}
        }
    }
    Ok(())
}

fn check_value(
    template: &ProjectTemplate,
    input: &super::TemplateInput,
    value: &str,
) -> Result<(), String> {
    match &input.kind {
        InputKind::Choice(options) => {
            if !options.iter().any(|o| o == value) {
                return Err(format!(
                    "template {}: input {} must be one of: {}",
                    template.id,
                    input.key,
                    options.join(", ")
                ));
            }
        }
        InputKind::ProjectName => {
            if !valid_project_name(value) {
                return Err(format!(
                    "template {}: input {} must be letters, digits, '-' or '_'",
                    template.id, input.key
                ));
            }
        }
        InputKind::Boolean => {
            if value != "true" && value != "false" {
                return Err(format!(
                    "template {}: input {} must be true or false",
                    template.id, input.key
                ));
            }
        }
        InputKind::Text => {}
    }
    Ok(())
}

pub fn valid_project_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Generate the project.  `project_name` is also exposed to substitutions
/// as the `project_name` variable.
pub fn generate(
    template: &ProjectTemplate,
    dest: &Path,
    project_name: &str,
    vars: &HashMap<String, String>,
    cancel: &AtomicBool,
) -> Result<GenerationSummary, String> {
    if !valid_project_name(project_name) {
        return Err(format!(
            "invalid project name {project_name:?}: letters, digits, '-' and '_' only"
        ));
    }
    if template
        .variables
        .iter()
        .any(|i| i.key == "project_name" && i.kind != InputKind::ProjectName)
    {
        return Err(format!(
            "template {}: a variable named project_name must use the project-name type",
            template.id
        ));
    }

    let mut vars = vars.clone();
    // The project name doubles as the project_name template input, so
    // validation sees every variable the renderer will.
    vars.insert("project_name".to_string(), project_name.to_string());
    validate_inputs(template, &vars)?;

    // Declared defaults materialize here so rendering never sees a
    // declared-but-absent variable.
    for input in &template.variables {
        if !vars.contains_key(&input.key)
            && let Some(default) = &input.default
        {
            vars.insert(input.key.clone(), default.clone());
        }
    }

    let dest_pre_existed = dest.exists();
    check_destination(template, dest)?;
    std::fs::create_dir_all(dest).map_err(|e| format!("mkdir {}: {e}", dest.display()))?;

    // Everything the walk creates, in creation order — a failed or
    // cancelled generation rolls back so the user never finds a
    // half-rendered project on disk.
    let mut walk = Walk {
        template,
        vars: &vars,
        cancel,
        files: Vec::new(),
        created: vec![dest.to_path_buf()],
    };
    let walk_result = walk.run(&template.source.clone(), dest, "", 0);
    if let Err(e) = walk_result {
        for path in walk.created.iter().rev() {
            if path.is_dir() {
                let _ = std::fs::remove_dir(path);
            } else {
                let _ = std::fs::remove_file(path);
            }
        }
        if !dest_pre_existed {
            let _ = std::fs::remove_dir(dest);
        }
        return Err(e);
    }
    let mut files = walk.files;
    files.sort();
    Ok(GenerationSummary {
        template_id: template.id.clone(),
        name: project_name.to_string(),
        dest: dest.to_path_buf(),
        files,
    })
}

/// The destination must be absolute, absent or an empty directory, and
/// outside the template source (so generation can never write into the
/// plugin package or delete-from it).
fn check_destination(template: &ProjectTemplate, dest: &Path) -> Result<(), String> {
    if !dest.is_absolute() {
        return Err(format!(
            "destination must be an absolute path: {}",
            dest.display()
        ));
    }
    if dest.components().count() <= 1 {
        return Err("refusing to use a filesystem root as destination".to_string());
    }
    let source = std::fs::canonicalize(&template.source)
        .map_err(|e| format!("template source {}: {e}", template.source.display()))?;
    // The destination itself usually does not exist yet — and even when a
    // path prefix is, it may differ lexically through symlinks (macOS
    // /var → /private/var) — so reconstruct the would-be canonical path
    // from the deepest existing ancestor before comparing.  The relative
    // suffix comes off the raw path, before canonicalization changes the
    // prefix out from under strip_prefix.
    let existing_raw = deepest_existing(dest);
    let suffix = dest.strip_prefix(&existing_raw).unwrap_or(dest);
    let existing = std::fs::canonicalize(&existing_raw).unwrap_or(existing_raw);
    let resolved = existing.join(suffix);
    if resolved.starts_with(&source) || source.starts_with(&resolved) {
        return Err(format!(
            "destination {} overlaps the template source {}",
            dest.display(),
            source.display()
        ));
    }
    match std::fs::symlink_metadata(dest) {
        Err(_) => Ok(()),
        Ok(meta) if meta.is_dir() => {
            let empty = std::fs::read_dir(dest)
                .map(|mut d| d.next().is_none())
                .unwrap_or(false);
            if empty {
                Ok(())
            } else {
                Err(format!("destination {} is not empty", dest.display()))
            }
        }
        Ok(_) => Err(format!(
            "destination {} exists and is not a directory",
            dest.display()
        )),
    }
}

/// The nearest existing ancestor of `path`, including `path` itself.
fn deepest_existing(path: &Path) -> PathBuf {
    let mut current = path.to_path_buf();
    loop {
        if current.exists() {
            return current;
        }
        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => return current,
        }
    }
}

const MAX_TEMPLATE_FILES: usize = 20_000;
const MAX_TEMPLATE_DEPTH: usize = 32;

/// One generation's walk state: the inputs every level needs plus the
/// creation log for rollback.
struct Walk<'a> {
    template: &'a ProjectTemplate,
    vars: &'a HashMap<String, String>,
    cancel: &'a AtomicBool,
    files: Vec<String>,
    created: Vec<PathBuf>,
}

impl Walk<'_> {
    fn run(
        &mut self,
        src: &Path,
        dest: &Path,
        rel_prefix: &str,
        depth: usize,
    ) -> Result<(), String> {
        let id = &self.template.id;
        if depth > MAX_TEMPLATE_DEPTH {
            return Err(format!(
                "template {id}: directory nesting deeper than {MAX_TEMPLATE_DEPTH}"
            ));
        }
        if self.files.len() > MAX_TEMPLATE_FILES {
            return Err(format!(
                "template {id}: more than {MAX_TEMPLATE_FILES} files"
            ));
        }
        if self.cancel.load(Ordering::Relaxed) {
            return Err("project creation cancelled".to_string());
        }
        let mut entries: Vec<PathBuf> = std::fs::read_dir(src)
            .map_err(|e| format!("template {id}: read {}: {e}", src.display()))?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .collect();
        entries.sort();
        for entry in entries {
            let name = entry
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            // Reject symlinks before anything else: they are the classic
            // way a hostile template writes outside the destination.
            let meta = std::fs::symlink_metadata(&entry)
                .map_err(|e| format!("template {id}: stat {}: {e}", entry.display()))?;
            if meta.file_type().is_symlink() {
                return Err(format!(
                    "template {id}: symlinks are not allowed (found {})",
                    entry.display()
                ));
            }
            // The template's own metadata file is not project content.
            if meta.is_file() && depth == 0 && name == "template.janet" {
                continue;
            }
            let rendered = render_segment(self.template, &name, self.vars)?;
            let target = dest.join(&rendered);
            let rel = if rel_prefix.is_empty() {
                rendered
            } else {
                format!("{rel_prefix}/{rendered}")
            };
            if meta.is_dir() {
                std::fs::create_dir_all(&target)
                    .map_err(|e| format!("mkdir {}: {e}", target.display()))?;
                self.created.push(target.clone());
                self.run(&entry, &target, &rel, depth + 1)?;
            } else if meta.is_file() {
                let bytes =
                    std::fs::read(&entry).map_err(|e| format!("read {}: {e}", entry.display()))?;
                let content = match String::from_utf8(bytes) {
                    Ok(text) => render_string(self.template, &text, self.vars)?.into_bytes(),
                    Err(raw) => raw.into_bytes(),
                };
                std::fs::write(&target, &content)
                    .map_err(|e| format!("write {}: {e}", target.display()))?;
                self.created.push(target);
                self.files.push(rel);
            }
        }
        Ok(())
    }
}

/// Count the files a template would copy (registration-time summary).
pub fn count_files(dir: &Path) -> usize {
    let mut count = 0;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let Ok(meta) = std::fs::symlink_metadata(entry.path()) else {
                continue;
            };
            if meta.is_symlink() {
                continue;
            }
            if meta.is_dir() {
                count += count_files(&entry.path());
            } else if meta.is_file() && entry.file_name().to_string_lossy() != "template.janet" {
                count += 1;
            }
        }
    }
    count
}

/// Render one path segment.  Segments come from `read_dir`, so any `/`,
/// `\`, or traversal a variable value introduces is a rejected attempt to
/// escape the destination.
fn render_segment(
    template: &ProjectTemplate,
    segment: &str,
    vars: &HashMap<String, String>,
) -> Result<String, String> {
    let rendered = render_string(template, segment, vars)?;
    if rendered.is_empty()
        || rendered == "."
        || rendered == ".."
        || rendered.contains('/')
        || rendered.contains('\\')
        || rendered.contains('\0')
    {
        return Err(format!(
            "template {}: variable substitution turns path segment {segment:?} into the invalid name {rendered:?}",
            template.id
        ));
    }
    Ok(rendered)
}

/// Single-pass `{{key}}` substitution.  Unknown keys and unclosed
/// placeholders are errors — silently emitting raw braces hides typos.
pub fn render_string(
    template: &ProjectTemplate,
    text: &str,
    vars: &HashMap<String, String>,
) -> Result<String, String> {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find("}}") else {
            return Err(format!(
                "template {}: unclosed {{{{ placeholder",
                template.id
            ));
        };
        let key = after[..end].trim();
        let Some(value) = vars.get(key) else {
            return Err(format!(
                "template {}: reference to unknown variable {{{{{key}}}}}",
                template.id
            ));
        };
        out.push_str(value);
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    Ok(out)
}
