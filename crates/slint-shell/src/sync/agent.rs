//! Agent tool-window sync for the refresh pass: transcript, connections,
//! onboarding state, and composer controls.

use slint::{ModelRc, VecModel};

use crate::{AppState, AppWindow, projection};

pub(super) fn sync(app: &AppWindow, state: &mut AppState) {
    let attachment_labels = projection::attachment_labels(state);
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
    app.set_agent_phase(projection::agent_phase(ed, state.agent_active).into());
    app.set_agent_phase_detail(projection::agent_phase_detail(ed, state.agent_active).into());
    app.set_agent_approval_summary(
        projection::agent_approval_summary(ed, state.agent_active).into(),
    );
    app.set_agent_files(ModelRc::new(VecModel::from(
        projection::agent_files(ed, state.agent_active)
            .into_iter()
            .map(Into::into)
            .collect::<Vec<slint::SharedString>>(),
    )));

    // Model catalogs for connected providers refresh lazily; the dock being
    // open is the trigger (the fetch is guarded against duplication).
    if state.agent_open {
        kernel::agent::connections::ensure_catalogs(ed);
    }

    // ── Connections + onboarding ─────────────────────────────────────────
    app.set_agent_connected(ed.agent_connections.any_connected());
    app.set_agent_provider_cards(ModelRc::new(VecModel::from(
        projection::provider_cards(ed),
    )));
    let (auth_active, auth) = projection::auth_panel(ed);
    app.set_agent_auth_active(auth_active);
    app.set_agent_auth(auth);

    // ── Composer ─────────────────────────────────────────────────────────
    app.set_agent_attachments(ModelRc::new(VecModel::from(
        attachment_labels
            .into_iter()
            .map(Into::into)
            .collect::<Vec<slint::SharedString>>(),
    )));
    app.set_agent_provider_choice(projection::active_provider_choice(ed));
    app.set_agent_provider_choices(ModelRc::new(VecModel::from(
        projection::provider_choices(ed),
    )));
    app.set_agent_model_choice(projection::active_model_choice(ed));
    app.set_agent_model_choices(ModelRc::new(VecModel::from(projection::model_choices(
        ed,
        &state.agent_model_search,
    ))));
    app.set_agent_reasoning_supported(projection::reasoning_supported(ed));
    app.set_agent_reasoning_label(projection::reasoning_label(ed).into());
    app.set_agent_reasoning_choices(ModelRc::new(VecModel::from(
        projection::reasoning_choices(ed),
    )));
    app.set_agent_send_blocked_reason(projection::send_blocked_reason(ed).into());

    // ── Dialog sync ────────────────────────────────────────────────────────
    // The unsaved-changes dialog projects the live dirty set: buffers that
    // finish saving while it is open drop off the list.
    app.set_quit_confirm_visible(state.quit_confirm_visible);
    if state.quit_confirm_visible {
        let dirty = ed.dirty_buffer_names();
        let n = dirty.len();
        app.set_quit_confirm_message(if n == 1 {
            "Save changes to this file before quitting?".into()
        } else {
            format!("Save changes to these {n} files before quitting?").into()
        });
        app.set_quit_confirm_files(dirty.join("\n").into());
    }

    // ── Install offers ─────────────────────────────────────────────────
    // One pending offer projects as a modal; the answer routes back
    // through `offer:accept` / `offer:decline`.
    let offer = ed.offers.front();
    app.set_offer_visible(offer.is_some());
    if let Some(offer) = offer {
        app.set_offer_title(offer.title.as_str().into());
        app.set_offer_body(offer.body.as_str().into());
        app.set_offer_accept_label(match offer.kind.as_str() {
            "plugin" => "Enable",
            _ => "Install",
        }.into());
    }
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
        app.set_settings_plugin_rows(ModelRc::new(VecModel::from(projection::plugin_rows(ed))));
    }
}
