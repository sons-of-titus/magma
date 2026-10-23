//! Run-configuration commands — launch, stop, and select the active config.

use crate::kernel::state::Editor;

pub(super) fn register(editor: &mut Editor) {
    let cmds = &mut editor.commands;

    cmds.register_fn("run-config",
        "Launch a run configuration (default: the active one)",
        vec![crate::kernel::command::args::ArgSpec::optional(
            "name", crate::kernel::command::args::ArgType::String,
            crate::kernel::command::args::ArgValue::String(String::new()),
        )],
        |editor, args| {
            let name = match args.get("name").and_then(|a| a.as_string()) {
                Some(n) if !n.is_empty() => n.to_string(),
                _ => editor.project_manager.run_configs.active.clone()
                    .ok_or_else(|| "no run configuration selected".to_string())?,
            };
            let Some(bg) = editor.background.clone() else {
                return Err("run-config: no background runtime".to_string());
            };
            let id = editor.project_manager.launch_config(&name, &mut editor.task_scheduler, &bg)?;
            crate::kernel::run_config::announce_launch(editor, id);
            Ok(())
        },
    );

    cmds.register_fn("stop-config",
        "Stop the running task of a run configuration (default: the active one)",
        vec![crate::kernel::command::args::ArgSpec::optional(
            "name", crate::kernel::command::args::ArgType::String,
            crate::kernel::command::args::ArgValue::String(String::new()),
        )],
        |editor, args| {
            let name = match args.get("name").and_then(|a| a.as_string()) {
                Some(n) if !n.is_empty() => n.to_string(),
                _ => editor.project_manager.run_configs.active.clone()
                    .ok_or_else(|| "no run configuration selected".to_string())?,
            };
            editor.project_manager.run_configs.stop(&name, &mut editor.task_scheduler)
        },
    );

    cmds.register_fn("select-config",
        "Make a run configuration the active one and persist the choice",
        vec![crate::kernel::command::args::ArgSpec::new(
            "name", crate::kernel::command::args::ArgType::String,
        )],
        |editor, args| {
            let name = args.get("name").and_then(|a| a.as_string())
                .ok_or_else(|| "name required".to_string())?;
            if editor.project_manager.run_configs.named(&name).is_none() {
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
            crate::kernel::command::args::ArgSpec::optional(
                "name", crate::kernel::command::args::ArgType::String,
                crate::kernel::command::args::ArgValue::String(String::new()),
            ),
            crate::kernel::command::args::ArgSpec::optional(
                "adapter", crate::kernel::command::args::ArgType::String,
                crate::kernel::command::args::ArgValue::String(String::new()),
            ),
        ],
        |editor, args| {
            use crate::kernel::project::{detect, toolchain};

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
            let adapter = match override_adapter {
                Some(a) => a.to_string(),
                None => detect::adapter_candidates(&language).iter()
                    .find(|candidate| toolchain::find(candidate).is_some())
                    .map(|candidate| candidate.to_string())
                    .ok_or_else(|| format!(
                        "no debug adapter for {language}: tried {} — install one or run :dbg-start <adapter>",
                        detect::adapter_candidates(&language).join(", ")))?,
            };

            let root = editor.project_manager.project.root.clone()
                .ok_or_else(|| "no project open".to_string())?;
            crate::kernel::debug::spawn_session(editor, &adapter, Some(cfg.to_launch_args(&root)))?;
            editor.show_message(
                crate::kernel::state::MessageKind::Info,
                format!("Debugging '{name}' with {adapter}"),
            );
            Ok(())
        },
    );
}
