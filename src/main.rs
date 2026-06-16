use std::sync::{Arc, RwLock};
use std::time::Duration;

use magma::kernel::event::keys;
use magma::kernel::event::payload::*;

use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;

use magma::kernel::text_engine::Buffer;
use magma::kernel::command::builtin;
use magma::kernel::storage::DiskFileSystem;
use magma::kernel::render::frame::render_frame;
use magma::kernel::render::surface::Surface;
use magma::kernel::render::tui::TuiRenderer;
use magma::kernel::input::event::InputEvent;
use magma::kernel::render::RenderTrait;
use magma::kernel::runtime::{process_background_event, BackgroundEvent, BackgroundHandle};
use magma::kernel::state::id::BufferId;
use magma::kernel::state::persist;
#[cfg(feature = "janet")]
use magma::kernel::scripting::ScriptRuntime;
use magma::kernel::state::Editor;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let _force_tui = args.iter().any(|a| a == "--tui");
    let force_gui = args.iter().any(|a| a == "--gui");
    let server_mode = args.iter().any(|a| a == "--server");
    let connect_mode = args.iter().any(|a| a == "--connect");

    let editor = Arc::new(RwLock::new(Editor::new(Box::new(DiskFileSystem::new()))));

    {
        let mut ed = editor.write().unwrap();
        builtin::register_builtin_commands(&mut ed);
        ed.command_history = persist::load_command_history();
        for m in persist::load_marks() {
            ed.marks.insert(m.name, m.byte_offset);
        }
    }

    {
        let mut ed = editor.write().unwrap();
        let id = ed.allocate_buffer_id();
        let mut buffer = Buffer::new(BufferId(id), "*scratch*");
        buffer.major_mode = magma::kernel::text_engine::MajorMode::Prog;
        let arc = std::sync::Arc::new(std::sync::Mutex::new(buffer));
        let entry = ed.buffers.vacant_entry();
        let buf_key = entry.key();
        entry.insert(arc.clone());
        ed.views.insert(buf_key, magma::kernel::text_engine::BufferView::new(arc));

        if let Some(win) = ed.view_tree.focused_window_mut() {
            win.buffer_id = Some(buf_key);
        }
    }

    let runtime = Arc::new(tokio::runtime::Runtime::new()?);
    let (bg_sender, bg_receiver) = mpsc::unbounded_channel();
    let bg_handle = BackgroundHandle::new(runtime.clone(), bg_sender.clone());

    {
        let mut ed = editor.write().unwrap();
        ed.background = Some(bg_handle.clone());
    }

    {
        let editor_ref = editor.clone();
        let bg = bg_handle.clone();
        runtime.spawn(async move {
            auto_save_loop(editor_ref, bg).await;
        });
    }

    let _file_watcher = {
        match magma::kernel::storage::watcher::FileWatcher::new(bg_sender.clone()) {
            Ok(mut fw) => {
                let ed = editor.read().unwrap();
                for (_, arc) in ed.buffers.iter() {
                    let buf = arc.lock().unwrap();
                    if let Some(ref path) = buf.path {
                        fw.watch(path);
                    }
                }
                drop(ed);
                Some(fw)
            }
            Err(e) => {
                eprintln!("Warning: could not start file watcher: {e}");
                None
            }
        }
    };

    #[cfg(feature = "janet")]
    {
        let mut ed = editor.write().unwrap();
        let mut rt = magma::kernel::scripting::JanetRuntime::new();
        rt.init(&mut *ed as *mut _);
        ed.runtime = Some(Box::new(rt));
    }

    {
        let mut ed = editor.write().unwrap();
        ed.events.emit_typed(keys::events::EDITOR_READY, EmptyPayload);
        ed.events.drain_and_dispatch();
    }

    if server_mode {
        let addr = args.iter()
            .skip_while(|a| a != &"--server")
            .nth(1)
            .map(|s| s.as_str())
            .unwrap_or("127.0.0.1:7890");
        eprintln!("Starting Magma server on {addr} (headless)");
        return runtime.block_on(async move {
            magma::kernel::server::run_server(editor, addr).await
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

    #[cfg(feature = "gui")]
    let use_gui = !_force_tui;
    #[cfg(not(feature = "gui"))]
    let use_gui = false;
    let _ = force_gui;

    if use_gui {
        run_gui(editor, bg_receiver)
    } else {
        run_tui(editor, bg_receiver)
    }
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

fn persist_state(editor: &Editor) {
    persist::save_command_history(&editor.command_history);
    let persisted: Vec<persist::PersistedMark> = editor.marks.iter()
        .map(|(&name, &byte_offset)| persist::PersistedMark {
            name,
            buffer_id: 0,
            byte_offset,
        })
        .collect();
    persist::save_marks(&persisted);
}

async fn auto_save_loop(editor: Arc<RwLock<Editor>>, bg: BackgroundHandle) {
    let mut interval = tokio::time::interval(Duration::from_secs(30));
    loop {
        interval.tick().await;

        let dirty: Vec<(usize, String, String)> = {
            let ed = match editor.read() {
                Ok(g) => g,
                Err(_) => continue,
            };
            ed.buffers.iter()
                .filter(|(_, arc)| arc.lock().unwrap().modified())
                .filter_map(|(key, arc)| {
                    let buf = arc.lock().unwrap();
                    buf.path.clone().map(|p| (key, p, buf.slice(0, buf.len())))
                })
                .collect()
        };

        for (key, path, content) in dirty {
            let editor = editor.clone();
            bg.spawn_blocking(move || {
                let result = (|| -> Result<(), String> {
                    if let Some(parent) = std::path::Path::new(&path).parent() {
                        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                    }
                    std::fs::write(&path, &content).map_err(|e| e.to_string())?;
                    Ok(())
                })();
                if result.is_ok()
                    && let Ok(mut ed) = editor.write()
                        && let Some(arc) = ed.buffers.get_mut(key) {
                            arc.lock().unwrap().mark_saved();
                        }
            });
        }
    }
}

#[cfg(feature = "gui")]
fn run_gui(
    editor: Arc<RwLock<Editor>>,
    bg_receiver: mpsc::UnboundedReceiver<BackgroundEvent>,
) -> Result<(), Box<dyn std::error::Error>> {
    use eframe::egui;
    use magma::kernel::render::gui::GuiApp;

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Magma")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([400.0, 300.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Magma",
        options,
        Box::new(|_cc| {
            Ok(Box::new(GuiApp::new_with_bg(editor, bg_receiver)))
        }),
    )
    .map_err(|e| format!("GUI error: {e}").into())
}

#[cfg(not(feature = "gui"))]
fn run_gui(
    _editor: Arc<RwLock<Editor>>,
    _bg_receiver: mpsc::UnboundedReceiver<BackgroundEvent>,
) -> Result<(), Box<dyn std::error::Error>> {
    drop(_bg_receiver);
    eprintln!("Compiled without the 'gui' feature. Use --tui or recompile with --features gui.");
    Ok(())
}

// ── Terminal mode ─────────────────────────────────────────────────────────

fn run_tui(
    editor: Arc<RwLock<Editor>>,
    mut bg_receiver: mpsc::UnboundedReceiver<BackgroundEvent>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut renderer = TuiRenderer::new()?;
    let (width, height) = renderer.dimensions();
    let mut surface = Surface::new(width, height);

    loop {
        {
            let ed = editor.read().unwrap();
            if !ed.running {
                break;
            }
        }

        while let Ok(event) = bg_receiver.try_recv() {
            let mut ed = editor.write().unwrap();
            process_background_event(&mut ed, event);
        }

        if let Some(event) = renderer.poll_event() {
            match event {
                InputEvent::Resize(w, h) => {
                    surface = Surface::new(w, h);
                    let mut ed = editor.write().unwrap();
                    ed.view_tree.resize(w, h);
                }
                InputEvent::Key(key) => {
                    let mut ed = editor.write().unwrap();

                    if key == "ctrl-q" {
                        ed.running = false;
                        continue;
                    }

                    magma::kernel::input::dispatch_key(&mut ed, &key);
                }
                InputEvent::Mouse(m) => {
                    let mut ed = editor.write().unwrap();
                    magma::kernel::input::mouse::dispatch_mouse(&mut ed, &surface, m.x, m.y);
                }
            }
        }

        {
            let ed = editor.read().unwrap();
            render_frame(&ed, &mut surface);
        }

        {
            let mut ed = editor.write().unwrap();
            #[cfg(feature = "janet")]
            magma::kernel::scripting::set_surface_ptr(&mut surface as *mut _);

            ed.events.emit_typed(keys::events::RENDER_FRAME, EmptyPayload);
            if ed.tab_bar_enabled {
                ed.events.emit_typed(keys::events::RENDER_TAB_BAR, EmptyPayload);
            }
            ed.events.drain_and_dispatch();

            if let Some(ref fn_name) = ed.modeline_fn.clone() {
                let expr = format!("({fn_name})");
                let result = ed.runtime.as_mut()
                    .map(|rt| rt.eval(&expr))
                    .unwrap_or_default();
                ed.modeline_rendered = result;
            } else {
                ed.modeline_rendered.clear();
            }

            #[cfg(feature = "janet")]
            magma::kernel::scripting::clear_surface_ptr();
        }

        renderer.draw(&surface);
    }

    {
        let ed = editor.read().unwrap();
        persist_state(&ed);
    }
    renderer.close();
    Ok(())
}
