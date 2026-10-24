//! Per-subsystem background-event handlers (R-22).
//!
//! The dispatch below is the single exhaustive match over `BackgroundEvent`;
//! each arm is a one-liner into the subsystem module that owns the logic.
//! `Custom` stays here because it has no subsystem.

mod agent;
mod dap;
mod fs;
mod lsp;
mod net;
mod process;
mod task;
mod vc;

use crate::kernel::state::Editor;

use super::BackgroundEvent;

pub(super) fn dispatch(ed: &mut Editor, event: BackgroundEvent) {
    match event {
        // ── Filesystem / storage ─────────────────────────────────────────
        BackgroundEvent::FileReadResult { .. }
        | BackgroundEvent::FileWriteResult { .. }
        | BackgroundEvent::FileChanged { .. }
        | BackgroundEvent::BufferReloaded { .. }
        | BackgroundEvent::SaveWritten { .. }
        | BackgroundEvent::SaveFailed { .. }
        | BackgroundEvent::FileSearchListed { .. }
        | BackgroundEvent::AutoSaveTick
        | BackgroundEvent::AutoSaveWritten { .. } => fs::handle(ed, event),

        // ── LSP ──────────────────────────────────────────────────────────
        BackgroundEvent::LspDiagnostics { .. }
        | BackgroundEvent::LspCompletion { .. }
        | BackgroundEvent::LspResponse { .. }
        | BackgroundEvent::LspHover { .. }
        | BackgroundEvent::LspDefinition { .. }
        | BackgroundEvent::LspCodeActions { .. }
        | BackgroundEvent::LspCompletionItems { .. }
        | BackgroundEvent::LspRenameResult { .. }
        | BackgroundEvent::LspProgress { .. }
        | BackgroundEvent::LspServerExited { .. }
        | BackgroundEvent::LspStarted { .. }
        | BackgroundEvent::LspApplyEdit { .. } => lsp::handle(ed, event),

        // ── Debug Adapter Protocol ───────────────────────────────────────
        BackgroundEvent::DapStarted { .. }
        | BackgroundEvent::DapStopped { .. }
        | BackgroundEvent::DapOutput { .. }
        | BackgroundEvent::DapTerminated { .. }
        | BackgroundEvent::DapResponse { .. }
        | BackgroundEvent::DapCommandResult { .. } => dap::handle(ed, event),

        // ── Processes / terminals ────────────────────────────────────────
        BackgroundEvent::TerminalOutput { .. }
        | BackgroundEvent::TerminalExited { .. }
        | BackgroundEvent::ProcessOutput { .. }
        | BackgroundEvent::ProcessExited { .. }
        | BackgroundEvent::TaskReady { .. } => process::handle(ed, event),

        // ── Network ──────────────────────────────────────────────────────
        BackgroundEvent::HttpResponse { .. }
        | BackgroundEvent::HttpError { .. }
        | BackgroundEvent::TcpConnected { .. }
        | BackgroundEvent::TcpData { .. }
        | BackgroundEvent::TcpClosed { .. }
        | BackgroundEvent::TcpError { .. }
        | BackgroundEvent::TcpClientConnected { .. }
        | BackgroundEvent::TcpClientData { .. }
        | BackgroundEvent::TcpClientDisconnected { .. } => net::handle(ed, event),

        // ── Tasks / index / search ───────────────────────────────────────
        BackgroundEvent::FileIndexed { .. }
        | BackgroundEvent::GrepResultsBatch { .. }
        | BackgroundEvent::GrepDone { .. }
        | BackgroundEvent::WorkProgress { .. }
        | BackgroundEvent::WorkCompleted { .. }
        | BackgroundEvent::WorkFailed { .. }
        | BackgroundEvent::BufferWords { .. }
        | BackgroundEvent::IndexRecordArrived(_)
        | BackgroundEvent::TsTreeReady { .. }
        | BackgroundEvent::TaskOutputLine { .. }
        | BackgroundEvent::TaskRunCompleted { .. } => task::handle(ed, event),

        // ── Version control ──────────────────────────────────────────────
        BackgroundEvent::VcOpDone { .. }
        | BackgroundEvent::VcStatusFetched { .. } => vc::handle(ed, event),

        // ── Agent ────────────────────────────────────────────────────────
        BackgroundEvent::AgentThought { .. }
        | BackgroundEvent::AgentAction { .. }
        | BackgroundEvent::AgentStepResult { .. }
        | BackgroundEvent::AgentLlmReply { .. }
        | BackgroundEvent::AgentResult { .. }
        | BackgroundEvent::AgentError { .. } => agent::handle(ed, event),

        // ── Free-form ────────────────────────────────────────────────────
        BackgroundEvent::Custom(name, payload) if name == "vc-hunks-diff" => {
            // The hunk buffer needs its own renderer (patches stashed on
            // VcState), so this Custom event is claimed by the VC layer.
            if let Err(e) = crate::kernel::command::builtin::vc_ops::build_hunks_buf(ed, &payload) {
                ed.show_message(crate::kernel::state::MessageKind::Error, e);
            }
        }
        BackgroundEvent::Custom(name, payload) => {
            let mut data = std::collections::HashMap::new();
            data.insert("payload".into(), payload);
            ed.events.emit(&name, data);
        }
    }
}
