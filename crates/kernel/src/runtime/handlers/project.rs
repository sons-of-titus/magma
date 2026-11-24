//! Project background-event handlers — find-files results, project-wide
//! search batches, and off-thread task detection results.

use crate::state::Editor;

use super::super::BackgroundEvent;

pub(super) fn handle(ed: &mut Editor, event: BackgroundEvent) {
    match event {
        BackgroundEvent::Project(event) => handle_project_event(ed, event),
        BackgroundEvent::TasksDetected {
            generation,
            language,
            configs,
        } => handle_tasks_detected(ed, generation, language, configs),
        _ => {}
    }
}

/// Apply off-thread language/run-config detection.  A result from a
/// superseded project open is dropped: the new project's own detection will
/// land shortly.
fn handle_tasks_detected(
    ed: &mut Editor,
    generation: u64,
    language: Option<String>,
    configs: crate::run_config::RunConfigManager,
) {
    if generation != ed.project_manager.project.generation {
        return;
    }
    ed.project_manager.project.detecting_tasks = false;
    ed.project_manager.project.language = language.unwrap_or_default();
    ed.project_manager.run_configs = configs;
    ed.project_manager.sync_active_to_registry();
    // The toolchain-install layer listens for this to fetch a missing
    // toolchain the manifest pinned; nothing fires without one.
    let mut data = std::collections::HashMap::new();
    data.insert("language".to_string(), ed.project_manager.project.language.clone());
    ed.events.emit("tasks-detected", data);
    if ed.project_manager.project.language.is_empty() {
        ed.show_message(
            crate::state::MessageKind::Info,
            "No recognized build system here (Cargo, go, npm, Make, Python, CMake, Maven, Gradle) — create run configurations with the toolbar gear",
        );
    }
}

fn handle_project_event(ed: &mut Editor, event: crate::project::Event) {
    use crate::project::Event;
    match event {
        Event::FileSearchListed { pattern, files } => {
            crate::command::builtin::buffer::present_file_search(ed, &pattern, files);
        }
        Event::GrepResultsBatch {
            generation,
            entries,
        } if generation == ed.search.generation => {
            ed.search.results.extend(entries);
            ed.search
                .results
                .truncate(crate::task::QUICKFIX_MAX_RESULTS);
        }
        Event::GrepDone { generation } if generation == ed.search.generation => {
            ed.search.index = 0;
        }
        // Stale generations fall through here: a newer search superseded them.
        _ => {}
    }
}
