use std::sync::Arc;
use std::time::Duration;

use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;

use magma::kernel::text_engine::Buffer;
use magma::kernel::command::builtin;
use magma::kernel::storage::DiskFileSystem;
use magma::kernel::runtime::{BackgroundEvent, BackgroundHandle};
use magma::kernel::state::id::BufferId;
use magma::kernel::state::persist;
#[cfg(feature = "janet")]
use magma::kernel::scripting::ScriptRuntime;
use magma::kernel::state::Editor;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let server_mode = args.iter().any(|a| a == "--server");
    let connect_mode = args.iter().any(|a| a == "--connect");

    let mut editor = Editor::new(Box::new(DiskFileSystem::new()));

    builtin::register_builtin_commands(&mut editor);
    editor.command_history = persist::load_command_history();
    for m in persist::load_marks() {
        editor.marks.insert(m.name, m.byte_offset);
    }
    editor.project_manager.recent_projects = persist::load_recent_projects();

    let id = editor.allocate_buffer_id();
    let mut buffer = Buffer::new(BufferId(id), "*scratch*");
    buffer.major_mode = magma::kernel::text_engine::MajorMode::Prog;
    let arc = std::sync::Arc::new(std::sync::Mutex::new(buffer));
    let entry = editor.buffers.vacant_entry();
    let buf_key = entry.key();
    entry.insert(arc.clone());
    editor.views.insert(buf_key, magma::kernel::text_engine::BufferView::new(arc));

    if let Some(win) = editor.view_tree.focused_window_mut() {
        win.buffer_id = Some(buf_key);
    }

    let runtime = Arc::new(tokio::runtime::Runtime::new()?);
    let (bg_sender, bg_receiver) = mpsc::unbounded_channel();
    let bg_handle = BackgroundHandle::new(runtime.clone(), bg_sender.clone());

    editor.background = Some(bg_handle.clone());

    {
        let bg = bg_handle.clone();
        runtime.spawn(async move {
            auto_save_heartbeat(bg).await;
        });
    }

    #[cfg(feature = "janet")]
    if !server_mode {
        // The GUI loop owns the editor on the main thread; the Janet VM is
        // initialized there once and never leaves this thread.
        let mut rt = magma::kernel::scripting::JanetRuntime::new();
        rt.init(&mut editor);
        editor.runtime = Some(Box::new(rt));
    }

    {
        use magma::kernel::event::keys;
        use magma::kernel::event::payload::*;
        editor.events.emit_typed(keys::events::EDITOR_READY, EmptyPayload);
        editor.dispatch_events_scoped();
    }

    // Reopen the last project in interactive mode: JetBrains restores the
    // working set rather than dropping you on a blank scratch buffer.
    if !server_mode && !connect_mode
        && let Some(path) = persist::most_recent_existing_project(&editor.project_manager.recent_projects) {
            let mut args = std::collections::HashMap::new();
            args.insert("path".to_string(),
                magma::kernel::command::args::ArgValue::Path(path));
            if let Err(e) = magma::kernel::command::execute_command(&mut editor, "open-project", &args) {
                eprintln!("Could not reopen last project: {e}");
            }
    }

    if server_mode {
        let addr = args.iter()
            .skip_while(|a| a != &"--server")
            .nth(1)
            .map(|s| s.as_str())
            .unwrap_or("127.0.0.1:7890");
        eprintln!("Starting Magma server on {addr} (headless)");
        return runtime.block_on(async move {
            magma::kernel::server::run_server(editor, bg_receiver, addr).await
        });
    }

    if connect_mode {
        let addr = args.iter()
            .skip_while(|a| a != &"--connect")
            .nth(1)
            .map(|s| s.as_str())
            .unwrap_or("127.0.0.1:7890");
        eprintln!("Connecting to Magma server at {addr}");
        return runtime.block_on(async move {
            client_connect(addr).await
        });
    }

    let result = run_gui(editor, bg_receiver);

    // A lane-starved blocking task can block the runtime's destructor
    // forever; shut down with a deadline instead of hanging at exit.
    drop(result);
    match std::sync::Arc::try_unwrap(runtime) {
        Ok(rt) => {
            rt.shutdown_timeout(std::time::Duration::from_secs(2));
        }
        Err(rt) => {
            // Handles still alive (embedded editor not yet dropped):
            // signal the workers and let the Arc teardown finish it.
            let _ = rt;
        }
    }
    Ok(())
}

async fn client_connect(addr: &str) -> Result<(), Box<dyn std::error::Error>> {
    use tokio::io::{AsyncBufReadExt, BufReader};
    use tokio::net::TcpStream;

    let stream = TcpStream::connect(addr).await?;
    let (reader, mut writer) = stream.into_split();
    let mut stdout = tokio::io::stdout();
    let mut stdin = BufReader::new(tokio::io::stdin()).lines();
    let mut server_lines = BufReader::new(reader).lines();

    loop {
        tokio::select! {
            line = stdin.next_line() => {
                match line? {
                    Some(text) if !text.trim().is_empty() => {
                        writer.write_all(text.as_bytes()).await?;
                        writer.write_all(b"\n").await?;
                    }
                    Some(_) => {}
                    None => break,
                }
            }
            resp = server_lines.next_line() => {
                match resp? {
                    Some(line) => {
                        stdout.write_all(format!("{line}\n").as_bytes()).await?;
                        stdout.flush().await?;
                    }
                    None => {
                        eprintln!("Server disconnected");
                        break;
                    }
                }
            }
        }
    }
    Ok(())
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

#[cfg(feature = "gui")]
fn run_gui(
    editor: magma::kernel::state::Editor,
    bg_receiver: mpsc::UnboundedReceiver<BackgroundEvent>,
) -> Result<(), Box<dyn std::error::Error>> {
    use eframe::egui;
    use magma::kernel::render::gui::GuiApp;

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Magma")
            .with_inner_size([800.0, 640.0])
            .with_min_inner_size([640.0, 480.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Magma",
        options,
        Box::new(|_cc| {
            Ok(Box::new(GuiApp::new_with_bg(editor, bg_receiver)))
        }),
    )
    .map_err(|e| -> Box<dyn std::error::Error> { format!("GUI error: {e}").into() })?;
    Ok(())
}

#[cfg(not(feature = "gui"))]
fn run_gui(
    _editor: magma::kernel::state::Editor,
    _bg_receiver: mpsc::UnboundedReceiver<BackgroundEvent>,
) -> Result<(), Box<dyn std::error::Error>> {
    let _ = _bg_receiver;
    eprintln!("Magma requires the 'gui' feature for interactive use; recompile with --features gui, or run --server for headless mode.");
    Ok(())
}
