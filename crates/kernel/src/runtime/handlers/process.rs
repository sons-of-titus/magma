//! Process and terminal background-event handlers — PTY output, process
//! lifecycle, and stored-task readiness.

use crate::state::Editor;
use crate::event::keys;
use crate::event::payload::*;

use super::super::BackgroundEvent;

pub(super) fn handle(ed: &mut Editor, event: BackgroundEvent) {
    match event {
        BackgroundEvent::TerminalOutput { terminal_id, data } => {
            ed.io.terminals.apply_output(terminal_id, &data);
        }
        BackgroundEvent::TerminalExited { terminal_id, exit_code } => {
            if exit_code != 0 {
                ed.show_message(crate::state::MessageKind::Error,
                    format!("terminal exited with code {exit_code}"));
            }
            ed.io.terminals.close(terminal_id);
            if ed.io.terminals.focused.is_none() && ed.editor_mode.name == "terminal" {
                crate::terminal::set_focused(ed, None);
            }
        }
        BackgroundEvent::ProcessOutput { id, line, stream } => {
            ed.events.emit_typed(keys::events::PROCESS_OUTPUT, ProcessOutputPayload {
                id: id.to_string(),
                line,
                stream,
            });
        }
        BackgroundEvent::ProcessExited { id, exit_code, cmd } => {
            // Finished processes leave the list — it tracks live processes,
            // and a graveyard of exited entries taught nothing.
            ed.io.processes.remove(&id);
            if exit_code == 0 {
                ed.scheduler.complete_process(id);
            } else {
                ed.scheduler.fail_process(id, format!("exit code {exit_code}"));
            }
            ed.events.emit_typed(keys::events::PROCESS_EXIT, ProcessExitPayload {
                id: id.to_string(),
                exit_code: exit_code.to_string(),
                cmd,
            });
        }
        BackgroundEvent::TaskReady { id } => {
            let mut rt = ed.runtime.take();
            if let Some(rt) = &mut rt {
                rt.execute_stored_task(ed, id);
            }
            ed.runtime = rt;
        }
        _ => {}
    }
}
