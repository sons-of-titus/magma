//! Agent tool-window wiring: sessions, transcript expansion, composer,
//! provider connections, and authentication flows.

use std::cell::RefCell;
use std::rc::Rc;

use slint::ComponentHandle;

use crate::app_state::AgentAttachment;
use crate::{AppState, AppWindow, projection, sync::refresh};

const ATTACH_BODY_LIMIT: usize = 12_000;

pub(super) fn install(app: &AppWindow, state: Rc<RefCell<AppState>>) {
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_agent_toggle(move || {
            let mut s = state.borrow_mut();
            s.agent_open = !s.agent_open;
            if s.agent_open {
                s.db_open = false;
            }
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_agent_submit(move || {
            let prompt = weak
                .upgrade()
                .map(|a| a.get_agent_prompt().to_string())
                .unwrap_or_default();
            if prompt.trim().is_empty() {
                return;
            }
            if let Some(app) = weak.upgrade() {
                app.set_agent_prompt("".into());
            }
            let mut s = state.borrow_mut();
            // Staged context attachments travel ahead of the prompt text.
            let mut composed = String::new();
            for att in std::mem::take(&mut s.agent_attachments) {
                composed.push_str(&format!("--- {} ---\n{}\n\n", att.label, att.body));
            }
            composed.push_str(&prompt);
            match kernel::agent::loop_::start_session(&mut s.editor, &composed) {
                Ok(id) => {
                    s.message = format!("agent session {id} started");
                    s.agent_open = true;
                    s.db_open = false;
                    s.agent_manage_open = false;
                    // Follow the fresh session rather than a pinned old one.
                    s.agent_active = None;
                }
                Err(e) => s.message = e,
            }
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_agent_stop(move || {
            let mut s = state.borrow_mut();
            if let Some(id) = projection::running_session(&s.editor) {
                s.editor.agent.request_stop(id);
                s.message = "agent will stop after its current step".into();
            }
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_agent_select_session(move |id| {
            let mut s = state.borrow_mut();
            s.agent_active = Some(id.max(0) as u64);
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_agent_toggle_expanded(move |idx| {
            let mut s = state.borrow_mut();
            let Some(id) = projection::active_session_id(&s.editor, s.agent_active) else {
                return;
            };
            let key = (id, idx.max(0) as usize);
            if !s.agent_expanded.remove(&key) {
                s.agent_expanded.insert(key);
            }
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_agent_approve(move || {
            let mut s = state.borrow_mut();
            if !kernel::agent::loop_::resolve_approval(&mut s.editor, true) {
                s.message = "no agent action is waiting for approval".into();
            }
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_agent_decline(move || {
            let mut s = state.borrow_mut();
            if !kernel::agent::loop_::resolve_approval(&mut s.editor, false) {
                s.message = "no agent action is waiting for approval".into();
            }
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_agent_open_file(move |path| {
            let mut s = state.borrow_mut();
            let mut args = std::collections::HashMap::new();
            args.insert(
                "path".to_string(),
                kernel::command::args::ArgValue::Path(path.to_string()),
            );
            match kernel::command::execute_command(&mut s.editor, "open-file", &args) {
                Ok(()) => s.message.clear(),
                Err(e) => s.message = e,
            }
            refresh(&weak, &mut s);
        });
    }

    // ── Provider connections ─────────────────────────────────────────────
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_agent_connect_provider(move |id| {
            let mut s = state.borrow_mut();
            match kernel::agent::connections::begin_auth(&mut s.editor, &id) {
                Ok(()) => s.message.clear(),
                Err(e) => s.message = e,
            }
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_agent_disconnect_provider(move |id| {
            let mut s = state.borrow_mut();
            kernel::agent::connections::disconnect(&mut s.editor, &id);
            s.message = format!("{id} disconnected");
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_agent_submit_key(move |key| {
            let mut s = state.borrow_mut();
            match kernel::agent::connections::submit_api_key(&mut s.editor, &key) {
                Ok(()) => {
                    s.message.clear();
                    kernel::state::persist::save_settings(&s.editor);
                }
                Err(e) => s.message = e,
            }
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_agent_open_url(move |url| {
            open_in_browser(&url);
            let mut s = state.borrow_mut();
            s.message.clear();
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_agent_copy_code(move || {
            let mut s = state.borrow_mut();
            if let Some(pending) = &s.editor.agent_connections.pending {
                if let kernel::agent::AuthInstruction::DeviceCode { user_code, .. } =
                    &pending.instruction
                {
                    s.editor.clipboard.set_text(user_code);
                    s.message = "verification code copied".into();
                }
            }
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_agent_onboarding_back(move || {
            let mut s = state.borrow_mut();
            kernel::agent::connections::cancel_auth(&mut s.editor);
            refresh(&weak, &mut s);
        });
    }

    // ── Composer controls ────────────────────────────────────────────────
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_agent_provider_selected(move |id| {
            let mut s = state.borrow_mut();
            s.editor
                .options
                .insert("agent.provider".into(), id.to_string());
            // Model and reasoning must re-validate against the new catalog;
            // drop stale choices so the projections pick fresh defaults.
            s.editor.options.remove("agent.model");
            s.editor.options.remove("agent.reasoning");
            if let Some(first) =
                kernel::agent::connections::selected_model_of(&s.editor).map(|m| m.id.clone())
            {
                s.editor.options.insert("agent.model".into(), first);
            }
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_agent_model_changed(move |id| {
            let mut s = state.borrow_mut();
            s.editor
                .options
                .insert("agent.model".into(), id.to_string());
            s.editor.options.remove("agent.reasoning");
            kernel::state::persist::save_settings(&s.editor);
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_agent_reasoning_changed(move |label| {
            let mut s = state.borrow_mut();
            // The reasoning picker's ids are display labels; map back to the
            // level id the catalog issued.
            if let Some(level) = kernel::agent::connections::selected_model_of(&s.editor)
                .map(|m| {
                    m.reasoning
                        .levels()
                        .iter()
                        .find(|l| l.label == label.as_str())
                        .cloned()
                })
                .flatten()
            {
                s.editor
                    .options
                    .insert("agent.reasoning".into(), level.id);
                kernel::state::persist::save_settings(&s.editor);
            }
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_agent_attach_active_buffer(move || {
            let mut s = state.borrow_mut();
            if let Some(att) = active_buffer_attachment(&mut s) {
                s.agent_attachments.push(att);
            }
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_agent_attach_file(move || {
            let Some(path) = rfd::FileDialog::new().pick_file() else {
                return;
            };
            let mut s = state.borrow_mut();
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| path.to_string_lossy().to_string());
                    s.agent_attachments.push(AgentAttachment {
                        label: name,
                        body: truncate_body(&content),
                    });
                }
                Err(e) => s.message = format!("could not read {}: {e}", path.display()),
            }
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_agent_attach_remove(move |idx| {
            let mut s = state.borrow_mut();
            let idx = idx.max(0) as usize;
            if idx < s.agent_attachments.len() {
                s.agent_attachments.remove(idx);
            }
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_agent_manage_providers(move || {
            let mut s = state.borrow_mut();
            s.agent_manage_open = true;
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_agent_resized(move |delta| {
            let mut s = state.borrow_mut();
            s.agent_dock_width = (s.agent_dock_width + delta).clamp(280.0, 640.0);
            refresh(&weak, &mut s);
        });
    }
}

/// Buffer content for a focused buffer, truncated for prompt use.
fn active_buffer_attachment(s: &mut AppState) -> Option<AgentAttachment> {
    let key = projection::focused_key(&s.editor)?;
    let buf = s.editor.buffers.get(key)?;
    let buf = buf.lock().unwrap_or_else(|e| e.into_inner());
    let text: String = buf.slice(0, buf.len().min(ATTACH_BODY_LIMIT));
    if text.trim().is_empty() {
        return None;
    }
    Some(AgentAttachment {
        label: buf.name.clone(),
        body: text,
    })
}

fn truncate_body(content: &str) -> String {
    if content.chars().count() <= ATTACH_BODY_LIMIT {
        return content.to_string();
    }
    let cut: String = content.chars().take(ATTACH_BODY_LIMIT).collect();
    format!("{cut}\n…[truncated]")
}

fn open_in_browser(url: &str) {
    #[cfg(target_os = "macos")]
    logging::LogErr::log_err(std::process::Command::new("open").arg(url).spawn());
    #[cfg(target_os = "linux")]
    logging::LogErr::log_err(std::process::Command::new("xdg-open").arg(url).spawn());
    #[cfg(target_os = "windows")]
    logging::LogErr::log_err(std::process::Command::new("cmd")
        .args(["/c", "start", "", url])
        .spawn());
}
