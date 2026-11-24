//! Per-subsystem background-event handlers (R-22).
//!
//! The dispatch below is the single exhaustive match over `BackgroundEvent`;
//! each arm is a one-liner into the subsystem module that owns the logic.
//! `Custom` stays here because it has no subsystem.

mod agent;
mod dap;
mod db;
mod fs;
mod index;
mod install;
mod lsp;
mod net;
mod process;
mod project;
mod session;
mod task;
mod templates;
mod vc;

use crate::state::Editor;

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
        | BackgroundEvent::AutoSaveTick
        | BackgroundEvent::AutoSaveWritten { .. } => fs::handle(ed, event),

        // ── LSP ──────────────────────────────────────────────────────────
        BackgroundEvent::Lsp(_) => lsp::handle(ed, event),

        // ── Managed downloads ────────────────────────────────────────────
        BackgroundEvent::InstallFinished { .. } => install::handle(ed, event),

        // ── Debug Adapter Protocol ───────────────────────────────────────
        BackgroundEvent::Dap(_) => dap::handle(ed, event),

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

        // ── Index ────────────────────────────────────────────────────────
        BackgroundEvent::Index(_) => index::handle(ed, event),

        // ── Project search ───────────────────────────────────────────────
        BackgroundEvent::Project(_) | BackgroundEvent::TasksDetected { .. } => {
            project::handle(ed, event)
        }

        // ── Session restore ──────────────────────────────────────────────
        BackgroundEvent::SessionBufferLoaded { .. }
        | BackgroundEvent::SessionRestoreDone { .. } => session::handle(ed, event),

        // ── Tasks ────────────────────────────────────────────────────────
        BackgroundEvent::WorkProgress { .. }
        | BackgroundEvent::WorkCompleted { .. }
        | BackgroundEvent::WorkFailed { .. }
        | BackgroundEvent::BufferWords { .. }
        | BackgroundEvent::TsTreeReady { .. }
        | BackgroundEvent::Task(_) => task::handle(ed, event),

        // ── Version control ──────────────────────────────────────────────
        BackgroundEvent::Vc(_) => vc::handle(ed, event),

        // ── Project templates ────────────────────────────────────────────
        BackgroundEvent::Templates(_) => templates::handle(ed, event),

        // ── Agent ────────────────────────────────────────────────────────
        BackgroundEvent::AgentThought { .. }
        | BackgroundEvent::AgentAction { .. }
        | BackgroundEvent::AgentStepResult { .. }
        | BackgroundEvent::AgentLlmReply { .. }
        | BackgroundEvent::AgentResult { .. }
        | BackgroundEvent::AgentError { .. }
        | BackgroundEvent::AgentAuthInstruction { .. }
        | BackgroundEvent::AgentAuthResult { .. }
        | BackgroundEvent::AgentModels { .. } => agent::handle(ed, event),
        BackgroundEvent::Db(_) => db::handle(ed, event),

        // ── Free-form ────────────────────────────────────────────────────
        BackgroundEvent::Custom(name, payload) => {
            let mut data = std::collections::HashMap::new();
            data.insert("payload".into(), payload);
            ed.events.emit(&name, data);
        }
    }
}
