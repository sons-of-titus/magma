//! Agent tool-window wiring: sessions, transcript expansion, composer.

use std::cell::RefCell;
use std::rc::Rc;

use slint::ComponentHandle;

use super::settings;
use crate::{AppState, AppWindow, projection, sync::refresh};

pub(super) fn install(app: &AppWindow, state: Rc<RefCell<AppState>>) {
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_agent_toggle(move || {
            let mut s = state.borrow_mut();
            s.agent_open = !s.agent_open;
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
            match kernel::agent::loop_::start_session(&mut s.editor, &prompt) {
                Ok(id) => {
                    s.message = format!("agent session {id} started");
                    s.agent_open = true;
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
        app.on_agent_provider_changed(move |provider| {
            let mut s = state.borrow_mut();
            let provider = provider.to_string();
            s.editor
                .options
                .insert("agent.provider".into(), provider.clone());
            // Keep the model selector meaningful: jump to the new provider's
            // default when the current model is not one of its models.
            let models = kernel::agent::llm::models_for_provider(&provider);
            let model = s
                .editor
                .options
                .get("agent.model")
                .cloned()
                .unwrap_or_default();
            if !models.is_empty() && !models.contains(&model.as_str()) {
                let default = kernel::agent::llm::provider_by_name(&provider)
                    .map(|p| p.default_model)
                    .unwrap_or(models[0]);
                s.editor
                    .options
                    .insert("agent.model".into(), default.into());
            }
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_agent_open_settings(move || {
            let mut s = state.borrow_mut();
            settings::open(&weak, &mut s, settings::CATEGORY_AGENT);
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
