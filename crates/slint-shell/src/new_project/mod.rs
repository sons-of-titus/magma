//! New Project wizard — state, actions, and projection for the
//! NewProjectView overlay.  The registry and generation live in the kernel
//! (`kernel::templates`); this module only browses, validates, and submits.

use std::collections::HashMap;

use kernel::command::args::ArgValue;
use kernel::command::execute_command;
use kernel::templates::InputKind;

mod validation;
use validation::{VarValue, validate};

use crate::{AppState, AppWindow, TemplateItem, TemplateVar, sync};

#[derive(Default)]
pub struct NewProjectState {
    pub visible: bool,
    /// Whether the form currently validates; synced from the last refresh
    /// so the keyboard path can gate Return without touching the window.
    pub can_create: bool,
    /// Index into the filtered template list.
    selected: usize,
    /// Template ids in display order (filtered by the search text).
    filtered: Vec<String>,
    /// The selected template's inputs with user values.
    vars: Vec<VarValue>,
    /// toolchain id → availability message; "" means available.  PATH
    /// lookups are cached per id so refreshes never rescan.
    availability: HashMap<String, String>,
    /// Failure from the last create attempt, shown until the next action.
    note: String,
}

pub fn open(weak: &slint::Weak<AppWindow>, s: &mut AppState) {
    if s.new_project.visible {
        return;
    }
    s.new_project = NewProjectState::default();
    s.new_project.visible = true;
    // Destination default: beside the open project, else the home folder.
    let dest = s
        .editor
        .project_manager
        .project
        .root
        .as_ref()
        .and_then(|r| r.parent().map(|p| p.to_string_lossy().into_owned()))
        .or_else(|| std::env::var("HOME").ok())
        .unwrap_or_default();
    if let Some(app) = weak.upgrade() {
        app.set_new_project_search("".into());
        app.set_new_project_name("".into());
        app.set_new_project_dest(dest.into());
    }
    s.editor.frame_dirty = true;
}

pub fn close(s: &mut AppState) {
    s.new_project.visible = false;
    s.editor.frame_dirty = true;
}

/// Route a `new-project:*` command from the shared dispatcher.
pub fn command(weak: &slint::Weak<AppWindow>, s: &mut AppState, cmd: &str) {
    let Some(app) = weak.upgrade() else {
        return;
    };
    let rest = cmd.strip_prefix("new-project:").unwrap_or(cmd);
    match rest {
        "close" => close(s),
        "create" => create(&app, s),
        "browse" => browse(&app, s),
        "search-changed" => s.new_project.selected = 0,
        "name-changed" | "dest-changed" => {}
        _ => {
            if let Some(index) = rest.strip_prefix("select:") {
                s.new_project.selected = index.parse().unwrap_or(0);
            } else if let Some(delta) = rest.strip_prefix("move:") {
                let delta: i32 = delta.parse().unwrap_or(0);
                s.new_project.selected = (s.new_project.selected as i32 + delta).max(0) as usize;
            } else if let Some(pair) = rest.strip_prefix("var:")
                && let Some((key, value)) = pair.split_once('=')
            {
                for var in &mut s.new_project.vars {
                    if var.key == key {
                        var.value = value.to_string();
                    }
                }
            }
        }
    }
    s.editor.frame_dirty = true;
    sync::refresh(weak, s);
}

fn browse(app: &AppWindow, _s: &mut AppState) {
    if let Some(dir) = rfd::FileDialog::new().pick_folder() {
        app.set_new_project_dest(dir.to_string_lossy().into_owned().into());
    }
}

fn create(app: &AppWindow, s: &mut AppState) {
    let template_id = s.new_project.filtered.get(s.new_project.selected).cloned();
    let Some(template_id) = template_id else {
        return;
    };
    let name = app.get_new_project_name().to_string();
    let dest_parent = app.get_new_project_dest().to_string();
    let dest = std::path::Path::new(&dest_parent).join(&name);
    let mut vars = HashMap::new();
    for var in &s.new_project.vars {
        if !var.value.is_empty() {
            vars.insert(var.key.clone(), var.value.clone());
        }
    }
    let vars_json = serde_json::to_string(&vars).unwrap_or_else(|_| "{}".into());

    let mut args = HashMap::new();
    args.insert("template".to_string(), ArgValue::String(template_id));
    args.insert(
        "dest".to_string(),
        ArgValue::Path(dest.to_string_lossy().into_owned()),
    );
    args.insert("name".to_string(), ArgValue::String(name));
    args.insert("vars".to_string(), ArgValue::String(vars_json));
    match execute_command(&mut s.editor, "new-project", &args) {
        Ok(()) => s.new_project.note.clear(),
        Err(e) => s.new_project.note = e,
    }
}

/// Push the wizard's full state into the view.  Called from `sync::refresh`.
pub fn sync(app: &AppWindow, s: &mut AppState) {
    let visible = s.new_project.visible;
    app.set_new_project_visible(visible);
    if !visible {
        return;
    }
    let search = app.get_new_project_search().to_string();
    let needle = search.to_lowercase();
    let name = app.get_new_project_name().to_string();
    let dest = app.get_new_project_dest().to_string();

    let editor = &mut s.editor;
    // Cloned: the wizard holds templates across the mutable availability
    // lookup below, and the structs are tiny.
    let filtered: Vec<kernel::templates::ProjectTemplate> = editor
        .templates
        .list()
        .into_iter()
        .filter(|t| {
            needle.is_empty()
                || t.id.contains(&needle)
                || t.name.to_lowercase().contains(&needle)
                || t.category.to_lowercase().contains(&needle)
                || t.description.to_lowercase().contains(&needle)
                || t.language.to_lowercase().contains(&needle)
        })
        .cloned()
        .collect();

    if s.new_project
        .filtered
        .iter()
        .ne(filtered.iter().map(|t| &t.id))
    {
        s.new_project.filtered = filtered.iter().map(|t| t.id.clone()).collect();
    }
    let selected = s.new_project.selected.min(filtered.len().saturating_sub(1));
    s.new_project.selected = selected;

    let selected_template = filtered.get(selected);

    // Rebuild the variable form when the selection changes, carrying over
    // values the user already typed for shared keys.
    if let Some(t) = selected_template {
        let current_keys: Vec<String> = s.new_project.vars.iter().map(|v| v.key.clone()).collect();
        let form_matches = t.variables.iter().map(|i| &i.key).eq(current_keys.iter());
        if !form_matches {
            s.new_project.vars = t
                .variables
                .iter()
                .map(|input| VarValue {
                    key: input.key.clone(),
                    kind: match input.kind {
                        InputKind::Text => "text".to_string(),
                        InputKind::ProjectName => "project-name".to_string(),
                        InputKind::Choice(_) => "choice".to_string(),
                        InputKind::Boolean => "boolean".to_string(),
                    },
                    value: input.default.clone().unwrap_or_default(),
                    description: input.description.clone(),
                    required: input.required,
                })
                .collect();
        }
    } else {
        s.new_project.vars.clear();
    }

    let (can_create, reason) = match selected_template {
        None => (false, "Select a template".to_string()),
        Some(_) => validate(&name, &dest, &s.new_project.vars),
    };

    let generating = editor.template_jobs.is_generating();
    let reason = if generating { String::new() } else { reason };

    let items: Vec<TemplateItem> = filtered
        .iter()
        .enumerate()
        .map(|(i, t)| template_item(t, i == selected))
        .collect();
    s.m_new_project_templates
        .set(items, |m| app.set_new_project_templates(m));

    let mut detail = selected_template
        .map(|t| template_item(t, true))
        .unwrap_or_else(empty_item);
    // Only the detail pane surfaces toolchain availability, and only the
    // selected template needs the check — PATH lookups are cached per id.
    if let Some(t) = selected_template
        && let Some(id) = t.toolchain.as_deref()
    {
        detail.toolchain_status =
            toolchain_status(editor, &mut s.new_project.availability, id).into();
    }
    app.set_new_project_detail(detail);

    let vars: Vec<TemplateVar> = s
        .new_project
        .vars
        .iter()
        .map(|v| TemplateVar {
            key: v.key.clone().into(),
            label: v.key.replace('_', " ").into(),
            description: v.description.clone().into(),
            kind: v.kind.clone().into(),
            value: v.value.clone().into(),
            choices: match selected_template
                .and_then(|t| t.variables.iter().find(|i| i.key == v.key))
            {
                Some(input) => match &input.kind {
                    InputKind::Choice(c) => {
                        slint::ModelRc::from(std::rc::Rc::new(slint::VecModel::from(
                            c.iter()
                                .map(|x| slint::SharedString::from(x.as_str()))
                                .collect::<Vec<_>>(),
                        )))
                    }
                    _ => slint::ModelRc::default(),
                },
                None => slint::ModelRc::default(),
            },
            required: v.required,
        })
        .collect();
    s.m_new_project_vars
        .set(vars, |m| app.set_new_project_vars(m));

    let can_create = can_create && !generating;
    s.new_project.can_create = can_create;
    app.set_new_project_can_create(can_create);
    app.set_new_project_create_reason(reason.into());
    app.set_new_project_note(s.new_project.note.clone().into());
    app.set_new_project_generating(generating);

    // A finished generation resolves the wizard: success closes it (the
    // editor window takes over), failure keeps it open with the error.
    if !generating {
        match &editor.template_jobs.last_result {
            Some(Ok(_)) => s.new_project.visible = false,
            Some(Err(e)) => s.new_project.note = e.clone(),
            None => {}
        }
    }
}

fn template_item(t: &kernel::templates::ProjectTemplate, selected: bool) -> TemplateItem {
    TemplateItem {
        id: t.id.clone().into(),
        name: t.name.clone().into(),
        description: t.description.clone().into(),
        category: t.category.clone().into(),
        language: t.language.clone().into(),
        plugin: t.owner.clone().into(),
        toolchain: t.toolchain.clone().unwrap_or_default().into(),
        toolchain_status: "".into(),
        file_count: t.file_count as i32,
        selected,
    }
}

fn empty_item() -> TemplateItem {
    TemplateItem {
        id: "".into(),
        name: "".into(),
        description: "".into(),
        category: "".into(),
        language: "".into(),
        plugin: "".into(),
        toolchain: "".into(),
        toolchain_status: "".into(),
        file_count: 0,
        selected: false,
    }
}

/// Missing-toolchain message for a declared toolchain id, cached per id.
/// Detection is deliberately separate from execution: a template stays
/// creatable, the warning explains what the user is missing.
fn toolchain_status(
    editor: &mut kernel::state::Editor,
    cache: &mut HashMap<String, String>,
    id: &str,
) -> String {
    if let Some(cached) = cache.get(id) {
        return cached.clone();
    }
    let message = match editor.toolchains.get(id).cloned() {
        None => format!("toolchain {id} is not registered"),
        Some(tc) => match tc.actions.iter().next() {
            None => String::new(),
            Some((_, action)) => {
                if kernel::project::toolchain::find(&action.program).is_some() {
                    String::new()
                } else {
                    format!(
                        "{} is not installed — the project can still be created, but build, test, and run will not work until it is",
                        action.program
                    )
                }
            }
        },
    };
    cache.insert(id.to_string(), message.clone());
    message
}
