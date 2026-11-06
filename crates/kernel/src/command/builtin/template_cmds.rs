//! Project creation from templates — the `new-project` command drives the
//! background generation engine; the New Project wizard is its main caller.

use crate::command::args::{ArgSpec, ArgType, ArgValue};
use crate::runtime::Lane;
use crate::state::Editor;
use std::collections::HashMap;

pub fn register(editor: &mut Editor) {
    editor.commands.register_fn(
        "new-project",
        "Create a project from a registered template",
        vec![
            ArgSpec::new("template", ArgType::String),
            ArgSpec::new("dest", ArgType::Path),
            ArgSpec::new("name", ArgType::String),
            ArgSpec::optional("vars", ArgType::String, ArgValue::String("{}".into())),
            ArgSpec::optional("open", ArgType::Boolean, ArgValue::Boolean(true)),
        ],
        |ed, args| {
            let template_id = arg_str(args, "template")
                .ok_or("new-project: template id required")?
                .to_string();
            let dest = arg_str(args, "dest")
                .ok_or("new-project: destination path required")?
                .to_string();
            let name = arg_str(args, "name")
                .ok_or("new-project: project name required")?
                .to_string();
            let vars_text = arg_str(args, "vars").unwrap_or("{}");
            let open = args.get("open").and_then(|v| v.as_bool()).unwrap_or(true);

            let mut vars: HashMap<String, String> = serde_json::from_str(vars_text)
                .map_err(|e| format!("new-project: vars must be a JSON object: {e}"))?;
            for (k, v) in &vars {
                if k.is_empty() {
                    return Err("new-project: variable names must not be empty".into());
                }
                if v.chars().any(|c| c == '\0') {
                    return Err(format!("new-project: variable {k:?} contains NUL"));
                }
            }
            vars.remove("project_name");

            let template = ed
                .templates
                .get(&template_id)
                .cloned()
                .ok_or_else(|| format!("unknown template: {template_id}"))?;
            let dest = crate::command::builtin::project_cmds::expand_tilde(&dest);
            let dest_path = std::path::PathBuf::from(&dest);

            if ed.template_jobs.is_generating() {
                return Err("a project is already being created".into());
            }
            // The project name doubles as the project_name template input.
            let mut validate_vars = vars.clone();
            validate_vars.insert("project_name".to_string(), name.clone());
            crate::templates::generate::validate_inputs(&template, &validate_vars)?;

            // Fail fast on a destination the wizard could never use; the
            // engine re-checks inside the background job (races, roots).
            match std::fs::symlink_metadata(&dest_path) {
                Ok(meta) if meta.is_dir() => {
                    let empty = std::fs::read_dir(&dest_path)
                        .map(|mut d| d.next().is_none())
                        .unwrap_or(false);
                    if !empty {
                        return Err(format!("destination {} is not empty", dest_path.display()));
                    }
                }
                Ok(_) => {
                    return Err(format!(
                        "destination {} exists and is not a directory",
                        dest_path.display()
                    ));
                }
                Err(e) => { logging::error!("template_cmds.rs:78: {e}"); {} }
            }
            if !crate::templates::generate::valid_project_name(&name) {
                return Err(format!(
                    "invalid project name {name:?}: letters, digits, '-' and '_' only"
                ));
            }

            let (request_id, cancel) =
                ed.template_jobs
                    .begin(&template.id, &name, dest_path.clone(), open);
            let Some(bg) = ed.background.clone() else {
                ed.template_jobs.generating = None;
                return Err("no background runtime available".into());
            };
            let sender = bg.sender.clone();
            let dest_for_msg = dest_path.display().to_string();
            let name_for_msg = name.clone();
            bg.spawn_on(Lane::Maintenance, Some("project-generate"), move || {
                let result = crate::templates::generate::generate(
                    &template, &dest_path, &name, &vars, &cancel,
                );
                logging::LogErr::log_warn(sender.send(crate::runtime::BackgroundEvent::Templates(
                    crate::templates::TemplateEvent::Generated {
                        request_id,
                        open,
                        result,
                    },
                )));
            });
            ed.show_message(
                crate::state::MessageKind::Info,
                &format!("Creating {} in {}", name_for_msg, dest_for_msg),
            );
            Ok(())
        },
    );

    editor.commands.register_fn(
        "new-project-cancel",
        "Cancel the in-flight project generation",
        vec![],
        |ed, _args| {
            if ed.template_jobs.cancel() {
                Ok(())
            } else {
                Err("no project generation in flight".into())
            }
        },
    );
}

fn arg_str<'a>(args: &'a HashMap<String, ArgValue>, key: &str) -> Option<&'a str> {
    args.get(key).and_then(|v| v.as_string())
}
