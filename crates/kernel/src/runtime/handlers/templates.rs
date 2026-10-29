//! Project-template background-event handlers — a finished generation lands
//! here: the job clears, the result is stored for the wizard, and a
//! successful creation may open the project right away.

use crate::state::{Editor, MessageKind};

use super::super::BackgroundEvent;

pub(super) fn handle(ed: &mut Editor, event: BackgroundEvent) {
    match event {
        BackgroundEvent::Templates(crate::templates::TemplateEvent::Generated {
            request_id,
            open,
            result,
        }) => handle_generated(ed, request_id, open, result),
        _ => {}
    }
}

fn handle_generated(
    ed: &mut Editor,
    request_id: u64,
    open: bool,
    result: Result<crate::templates::GenerationSummary, String>,
) {
    // A stale job id means a cancel raced a completion; either way the slot
    // only clears for the job that owns it.
    if ed
        .template_jobs
        .generating
        .as_ref()
        .is_some_and(|job| job.request_id == request_id)
    {
        ed.template_jobs.generating = None;
    }
    match result {
        Ok(summary) => {
            ed.template_jobs.last_result = Some(Ok(summary.clone()));
            info!(
                "project created: {} from template {} ({} files)",
                summary.dest.display(),
                summary.template_id,
                summary.files.len()
            );
            if open {
                let mut args = std::collections::HashMap::new();
                args.insert(
                    "path".to_string(),
                    crate::command::args::ArgValue::Path(
                        summary.dest.to_string_lossy().into_owned(),
                    ),
                );
                if let Err(e) = crate::command::execute_command(ed, "open-project", &args) {
                    ed.show_message(
                        MessageKind::Error,
                        &format!("project created but could not be opened: {e}"),
                    );
                }
            } else {
                ed.show_message(
                    MessageKind::Info,
                    &format!("created {} ({} files)", summary.name, summary.files.len()),
                );
            }
        }
        Err(e) => {
            ed.template_jobs.last_result = Some(Err(e.clone()));
            ed.show_message(MessageKind::Error, &e);
        }
    }
}
