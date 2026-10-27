//! Project background-event handlers — find-files results and project-wide
//! search batches.

use crate::state::Editor;

use super::super::BackgroundEvent;

pub(super) fn handle(ed: &mut Editor, event: BackgroundEvent) {
    match event {
        BackgroundEvent::Project(event) => handle_project_event(ed, event),
        _ => {}
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
