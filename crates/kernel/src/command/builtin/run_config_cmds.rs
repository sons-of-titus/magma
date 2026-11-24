//! Run-configuration commands — launch, stop, and select the active config.

use crate::state::Editor;

/// Which debug adapter serves `language`, in override order: the explicit
/// `:dbg-start` argument, the user's `<lang>-debug-adapter` option, the
/// project toolchain's `:debug` declaration, then the built-in candidates.
/// All names resolve through `toolchain::find_extended`, so an adapter
/// outside `$PATH` still spawns; an unresolved explicit name passes
/// through verbatim.
pub(crate) fn resolve_adapter(
    editor: &Editor,
    language: &str,
    override_adapter: Option<&str>,
) -> Result<String, String> {
    use crate::project::{detect, toolchain};

    let root = editor
        .project_manager
        .project
        .root
        .clone()
        .ok_or_else(|| "no project open".to_string())?;
    let toolchain_adapter = editor
        .toolchains
        .detect(&root)
        .and_then(|t| t.debug_adapter.clone());
    let option_adapter = editor
        .options
        .get(&format!("{language}-debug-adapter"))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && s != "nil");
    let resolved = match override_adapter {
        Some(a) => Some(
            toolchain::find_extended(a)
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_else(|| a.to_string()),
        ),
        None => option_adapter
            .as_deref()
            .and_then(toolchain::find_extended)
            .or_else(|| detect::find_adapter(language))
            .or_else(|| toolchain_adapter.as_deref().and_then(toolchain::find_extended))
            .map(|path| path.to_string_lossy().into_owned()),
    };
    resolved.ok_or_else(|| {
        format!(
            "no debug adapter for {language}: tried {} — install one or run :dbg-start <adapter>",
            detect::adapter_candidates(language).join(", ")
        )
    })
}

/// Launch the detection-seeded `build`/`test` config for the project,
/// with an error that names the detected language when there is none.
fn launch_preset(editor: &mut Editor, name: &str) -> Result<(), String> {
    let language = editor.project_manager.project.language.clone();
    if editor.project_manager.run_configs.named(name).is_none() {
        return Err(if language.is_empty() {
            format!("no {name} task detected — open a project with a known build system")
        } else {
            format!("no {name} task for this project (detected: {language})")
        });
    }
    let Some(bg) = editor.background.clone() else {
        return Err("no background runtime".to_string());
    };
    let id = editor
        .project_manager
        .launch_config(name, &mut editor.task_scheduler, &bg)?;
    crate::run_config::announce_launch(editor, id);
    Ok(())
}

pub(super) fn register(editor: &mut Editor) {
    let cmds = &mut editor.commands;

    // Toolbar Build/Test dispatch here: the detection-seeded configs are
    // named "build"/"test", so the buttons run the project's real task
    // instead of a hard-coded `make`.  A project with no detected task
    // gets a sentence, not a foreign build system's failure.
    cmds.register_fn("task-build", "Run the project's build task", vec![], |editor, _args| {
        launch_preset(editor, "build")
    });
    cmds.register_fn("task-test", "Run the project's tests", vec![], |editor, _args| {
        launch_preset(editor, "test")
    });

    cmds.register_fn(
        "run-config",
        "Launch a run configuration (default: the active one)",
        vec![crate::command::args::ArgSpec::optional(
            "name",
            crate::command::args::ArgType::String,
            crate::command::args::ArgValue::String(String::new()),
        )],
        |editor, args| {
            let name = match args.get("name").and_then(|a| a.as_string()) {
                Some(n) if !n.is_empty() => n.to_string(),
                _ => editor
                    .project_manager
                    .run_configs
                    .active
                    .clone()
                    .ok_or_else(|| "no run configuration selected".to_string())?,
            };
            let Some(bg) = editor.background.clone() else {
                return Err("run-config: no background runtime".to_string());
            };
            let id =
                editor
                    .project_manager
                    .launch_config(&name, &mut editor.task_scheduler, &bg)?;
            crate::run_config::announce_launch(editor, id);
            Ok(())
        },
    );

    cmds.register_fn(
        "stop-config",
        "Stop the running task of a run configuration (default: the active one)",
        vec![crate::command::args::ArgSpec::optional(
            "name",
            crate::command::args::ArgType::String,
            crate::command::args::ArgValue::String(String::new()),
        )],
        |editor, args| {
            let name = match args.get("name").and_then(|a| a.as_string()) {
                Some(n) if !n.is_empty() => n.to_string(),
                _ => editor
                    .project_manager
                    .run_configs
                    .active
                    .clone()
                    .ok_or_else(|| "no run configuration selected".to_string())?,
            };
            editor
                .project_manager
                .run_configs
                .stop(&name, &mut editor.task_scheduler)
        },
    );

    cmds.register_fn(
        "select-config",
        "Make a run configuration the active one and persist the choice",
        vec![crate::command::args::ArgSpec::new(
            "name",
            crate::command::args::ArgType::String,
        )],
        |editor, args| {
            let name = args
                .get("name")
                .and_then(|a| a.as_string())
                .ok_or_else(|| "name required".to_string())?;
            if editor.project_manager.run_configs.named(name).is_none() {
                return Err(format!("no run configuration named '{name}'"));
            }
            editor.project_manager.run_configs.active = Some(name.to_string());
            let Some(root) = editor.project_manager.project.root.clone() else {
                return Err("no project open".to_string());
            };
            editor.project_manager.run_configs.save(&root)
        },
    );

    cmds.register_fn("debug-config",
        "Start a debug session from a run configuration (default: the active one)",
        vec![
            crate::command::args::ArgSpec::optional(
                "name", crate::command::args::ArgType::String,
                crate::command::args::ArgValue::String(String::new()),
            ),
            crate::command::args::ArgSpec::optional(
                "adapter", crate::command::args::ArgType::String,
                crate::command::args::ArgValue::String(String::new()),
            ),
        ],
        |editor, args| {
            let name = match args.get("name").and_then(|a| a.as_string()) {
                Some(n) if !n.is_empty() => n.to_string(),
                _ => editor.project_manager.run_configs.active.clone()
                    .ok_or_else(|| "no run configuration selected".to_string())?,
            };
            let cfg = editor.project_manager.run_configs.named(&name)
                .ok_or_else(|| format!("no run configuration named '{name}'"))?
                .clone();
            if !cfg.debuggable {
                return Err(format!("run configuration '{name}' is not marked debuggable"));
            }
            let override_adapter = args.get("adapter").and_then(|a| a.as_string())
                .filter(|a| !a.is_empty());
            let language = editor.project_manager.project.language.clone();
            let adapter = resolve_adapter(editor, &language, override_adapter)?;
            let root = editor.project_manager.project.root.clone()
                .ok_or_else(|| "no project open".to_string())?;

            crate::debug::spawn_session(editor, &adapter, Some(cfg.to_launch_args(&root)))?;
            editor.show_message(
                crate::state::MessageKind::Info,
                format!("Debugging '{name}' with {adapter}"),
            );
            Ok(())
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn editor_with_project(language: &str) -> Editor {
        let mut ed = Editor::new(Box::new(crate::storage::disk::DiskFileSystem::new()));
        super::super::register_builtin_commands(&mut ed);
        ed.project_manager.project.language = language.to_string();
        ed.project_manager.project.root =
            Some(std::env::temp_dir().join("magma_adapter_tests"));
        ed
    }

    #[test]
    fn unknown_language_has_no_adapter() {
        let ed = editor_with_project("cobol");
        assert!(
            resolve_adapter(&ed, "cobol", None).is_err(),
            "no candidates and no option must error, not guess"
        );
    }

    #[test]
    fn user_option_beats_the_candidate_table() {
        // "sh" exists on every PATH and is not a python candidate, so a
        // hit here can only come from the option.
        let mut ed = editor_with_project("python");
        ed.options
            .insert("python-debug-adapter".to_string(), "sh".to_string());
        let adapter = resolve_adapter(&ed, "python", None).unwrap();
        assert!(
            adapter.ends_with("/sh"),
            "the option resolves first: got {adapter}"
        );
    }

    #[test]
    fn nil_or_empty_option_falls_through_to_candidates() {
        for value in ["nil", "", "   "] {
            let mut ed = editor_with_project("cobol");
            ed.options
                .insert("cobol-debug-adapter".to_string(), value.to_string());
            assert!(
                resolve_adapter(&ed, "cobol", None).is_err(),
                "placeholder option {value:?} must behave like no option"
            );
        }
    }

    #[test]
    fn explicit_override_passes_through_when_unresolvable() {
        let ed = editor_with_project("cobol");
        let adapter = resolve_adapter(&ed, "cobol", Some("some-custom-adapter")).unwrap();
        assert_eq!(adapter, "some-custom-adapter");
    }
}
