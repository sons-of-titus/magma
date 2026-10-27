//! Agent tool-window + dialog/settings sync for the refresh pass.

use slint::{ModelRc, VecModel};

use crate::{AppState, AppWindow, projection};

pub(super) fn sync(app: &AppWindow, state: &mut AppState) {
    let ed = &mut state.editor;
    app.set_agent_open(state.agent_open);
    app.set_agent_entries(ModelRc::new(VecModel::from(projection::agent_entries(
        ed,
        state.agent_active,
        &state.agent_expanded,
    ))));
    app.set_agent_sessions(ModelRc::new(VecModel::from(
        projection::agent_session_chips(ed, state.agent_active),
    )));
    app.set_agent_running(projection::agent_running(ed));
    app.set_agent_turns_left(projection::agent_turns_left(ed, state.agent_active));
    app.set_agent_dock_width(state.agent_dock_width);
    app.set_agent_configured(kernel::agent::llm::LlmConfig::is_configured(&ed.options));
    app.set_agent_providers(ModelRc::new(VecModel::from(
        kernel::agent::llm::PROVIDER_NAMES
            .iter()
            .map(|p| slint::SharedString::from(*p))
            .collect::<Vec<_>>(),
    )));
    // Sync a provider chosen in the dock back into the options (mid-chat
    // switch keeps the running session's history).
    let app_prov = app.get_agent_provider().to_string();
    if !app_prov.trim().is_empty()
        && ed.options.get("agent.provider").map(|s| s.as_str()) != Some(app_prov.as_str())
    {
        ed.options.insert("agent.provider".into(), app_prov.clone());
    }
    let prov = ed
        .options
        .get("agent.provider")
        .cloned()
        .unwrap_or_else(|| "openai".into());
    app.set_agent_provider(prov.clone().into());
    // Same for the model: LlmConfig is re-read on every turn.
    let app_model = app.get_agent_model().to_string();
    if !app_model.trim().is_empty()
        && ed.options.get("agent.model").map(|s| s.as_str()) != Some(app_model.as_str())
    {
        ed.options.insert("agent.model".into(), app_model.clone());
    }
    let model_now = ed
        .options
        .get("agent.model")
        .cloned()
        .unwrap_or_else(|| "gpt-4o-mini".into());
    app.set_agent_model(model_now.into());
    let models: Vec<slint::SharedString> = kernel::agent::llm::models_for_provider(&prov)
        .iter()
        .map(|m| (*m).into())
        .collect();
    app.set_agent_models(ModelRc::new(VecModel::from(models))); // ── Dialog sync ────────────────────────────────────────────────────────
    app.set_dialog_visible(state.dialog_visible);
    app.set_dialog_title(state.dialog_title.as_str().into());
    // settings state is in-out: read user changes back
    state.settings_visible = app.get_settings_visible();
    state.settings_category = app.get_settings_category();
    // Re-project rows only when something relevant changed: rebuilding the
    // model mid-keystroke would clobber in-progress text edits.
    if state.settings_visible && state.settings_rows_dirty {
        state.settings_rows_dirty = false;
        app.set_settings_rows(ModelRc::new(VecModel::from(projection::settings_rows(
            ed,
            state.settings_category,
            &state.settings_search,
        ))));
    }
}
