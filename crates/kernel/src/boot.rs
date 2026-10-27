//! Shared startup for the interactive and headless binaries.
//!
//! Both frontends need the same editor: commands registered, history and
//! marks restored, the background pool wired, crash recovery armed, and the
//! Janet runtime initialized on the thread that will own it.  None of this
//! is presentation, so it lives in the kernel where the server can reuse it.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;

use crate::command::builtin;
use crate::runtime::{BackgroundEvent, BackgroundHandle};
use crate::script_runtime::ScriptRuntime;
use crate::state::Editor;
use crate::state::persist;
use crate::storage::DiskFileSystem;

/// Which startup behaviors to run.  The GUI is interactive and reopens the
/// last project; the headless server is neither.
#[derive(Debug, Clone, Copy)]
pub struct BootOptions {
    pub interactive: bool,
    pub reopen_last_project: bool,
}

pub struct Boot {
    pub editor: Editor,
    pub runtime: Arc<tokio::runtime::Runtime>,
    pub background: BackgroundHandle,
    pub bg_receiver: mpsc::UnboundedReceiver<BackgroundEvent>,
}

pub fn bootstrap(
    options: BootOptions,
    script_runtime: Option<Box<dyn ScriptRuntime>>,
) -> std::io::Result<Boot> {
    let mut editor = Editor::new(Box::new(DiskFileSystem::new()));

    builtin::register_builtin_commands(&mut editor);
    editor.command_history = persist::load_command_history();
    for m in persist::load_marks() {
        editor.marks.insert(m.name, m.byte_offset);
    }
    editor.project_manager.recent_projects = persist::load_recent_projects();

    let runtime = Arc::new(tokio::runtime::Runtime::new()?);
    let (bg_sender, bg_receiver) = mpsc::unbounded_channel();
    let background = BackgroundHandle::new(runtime.clone(), bg_sender);

    editor.background = Some(background.clone());
    // Crash recovery (R-16): this run's snapshot dir; leftover snapshots
    // from a previous crash resurface as read-only *recovered* buffers.
    editor.io.crash_dir = Some(crate::storage::crash_recovery::new_session_dir());
    crate::storage::crash_recovery::recover_into(&mut editor);

    {
        let bg = background.clone();
        runtime.spawn(async move {
            auto_save_heartbeat(bg).await;
        });
    }

    if let Some(mut rt) = script_runtime {
        rt.init(&mut editor);
        editor.runtime = Some(rt);
    }

    {
        use crate::event::keys;
        use crate::event::payload::*;
        editor
            .events
            .emit_typed(keys::events::EDITOR_READY, EmptyPayload);
        editor.dispatch_events_scoped();
    }

    // Reopen the last project in interactive mode: JetBrains restores the
    // working set rather than dropping you on a blank scratch buffer.
    if options.reopen_last_project
        && let Some(path) =
            persist::most_recent_existing_project(&editor.project_manager.recent_projects)
    {
        let mut args = std::collections::HashMap::new();
        args.insert(
            "path".to_string(),
            crate::command::args::ArgValue::Path(path),
        );
        if let Err(e) = crate::command::execute_command(&mut editor, "open-project", &args) {
            warn!("Could not reopen last project: {e}");
        }
    }

    Ok(Boot {
        editor,
        runtime,
        background,
        bg_receiver,
    })
}

/// Shut the pool down with a deadline: a lane-starved blocking task can
/// otherwise block the runtime's destructor forever.
pub fn shutdown(runtime: Arc<tokio::runtime::Runtime>) {
    if let Ok(rt) = Arc::try_unwrap(runtime) {
        rt.shutdown_timeout(Duration::from_secs(2));
    }
}

/// Opt-in auto-save heartbeat.  The heartbeat owns no editor state: it only
/// nudges the foreground loop, which snapshots dirty buffers and hands the
/// write to the background pool (rework.md §1: background threads never
/// touch the editor).
async fn auto_save_heartbeat(bg: BackgroundHandle) {
    let mut interval = tokio::time::interval(Duration::from_secs(30));
    loop {
        interval.tick().await;
        bg.send_event(BackgroundEvent::AutoSaveTick);
    }
}
