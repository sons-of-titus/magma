//! Database tool-window sync: one pass per refresh over the live DbState.

use slint::ModelRc;

use slint::VecModel;

use crate::{AppState, AppWindow, projection};

pub(super) fn sync(app: &AppWindow, state: &mut AppState) {
    let ed = &mut state.editor;
    app.set_db_explorer_width(state.db_explorer_width);
    app.set_db_sources(ModelRc::new(VecModel::from(
        projection::db_sources(ed)
            .into_iter()
            .map(Into::into)
            .collect::<Vec<slint::SharedString>>(),
    )));
    app.set_db_active_source(projection::db_active_source_name(ed).into());
    let active_id = ed.db.active;
    let connected = active_id.map(|id| ed.db.is_connected(id)).unwrap_or(false);
    app.set_db_connected(connected);
    app.set_db_connected_label(projection::db_connected_label(ed).into());
    app.set_db_busy(projection::db_busy(ed));
    app.set_db_querying(projection::db_querying(ed));
    app.set_db_open(state.db_open);
    app.set_db_height(state.db_height);
    app.set_db_tree_filter(state.db_tree_filter.as_str().into());
    app.set_db_tree_filter_active(state.db_tree_filter_active);
    app.set_db_tree(ModelRc::new(VecModel::from(projection::db_tree(
        ed,
        &state.db_tree_filter,
    ))));
    let widths = active_id
        .and_then(|_| ed.db.active_tab)
        .and_then(|tab| state.db_col_widths.get(&tab).cloned())
        .unwrap_or_default();
    app.set_db_tabs(ModelRc::new(VecModel::from(projection::db_tabs(ed))));
    app.set_db_grid_title(projection::db_grid_title(ed).into());
    app.set_db_columns(ModelRc::new(VecModel::from(projection::db_columns(
        ed,
        &widths,
    ))));
    app.set_db_rows(ModelRc::new(VecModel::from(projection::db_rows(ed))));
    let (status, is_error) = projection::db_status(ed);
    app.set_db_status(status.into());
    app.set_db_status_error(is_error);
    app.set_db_truncated(projection::db_truncated(ed));
    app.set_db_row_count(projection::db_row_count(ed).into());
    app.set_db_elapsed(projection::db_elapsed(ed).into());
    app.set_db_page_info(projection::db_page_info(ed).into());
    app.set_db_can_edit(projection::db_can_edit(ed));
    app.set_db_pending_count(projection::db_pending_count(ed));
    app.set_db_has_tab(projection::db_has_tab(ed));
    app.set_db_filter_text(
        ed.db
            .active_tab()
            .map(|t| t.filter.clone())
            .unwrap_or_default()
            .into(),
    );

    // Connection dialog
    dialog_sync(app, state);
}

fn dialog_sync(app: &AppWindow, state: &mut AppState) {
    use slint::SharedString;
    let (testing, token) = state
        .db_dialog
        .as_ref()
        .map(|d| (d.testing, d.test_token))
        .unwrap_or((false, 0));
    if testing {
        let (note, _) = dialog_note(state, token);
        if !note.is_empty()
            && let Some(d) = &mut state.db_dialog
        {
            d.testing = false;
        }
    }
    let Some(d) = &mut state.db_dialog else {
        app.set_db_dialog_visible(false);
        return;
    };
    app.set_db_dialog_visible(true);
    app.set_db_dialog_title(
        if d.editing.is_some() {
            "Data Source Properties"
        } else {
            "Add Data Source"
        }
        .into(),
    );
    app.set_db_dialog_kinds(ModelRc::new(VecModel::from(
        [
            db::DriverKind::Sqlite,
            db::DriverKind::Postgres,
            db::DriverKind::MySql,
            db::DriverKind::MariaDb,
            db::DriverKind::SqlServer,
        ]
        .iter()
        .map(|k| SharedString::from(k.label()))
        .collect::<Vec<_>>(),
    )));
    app.set_db_dialog_ssl_choices(ModelRc::new(VecModel::from(
        ["disabled", "prefer", "require"]
            .iter()
            .map(|s| SharedString::from(*s))
            .collect::<Vec<_>>(),
    )));
    app.set_db_dialog_kind_index(d.kind_index as i32);
    app.set_db_dialog_embedded(d.kind_index == 0);
    // Hydrate the editable fields only once, when the dialog opens, so
    // subsequent refreshes don't clobber in-progress typing.
    if d.needs_hydration {
        app.set_db_dialog_name(d.name.as_str().into());
        app.set_db_dialog_host(d.host.as_str().into());
        app.set_db_dialog_port(d.port.as_str().into());
        app.set_db_dialog_database(d.database.as_str().into());
        app.set_db_dialog_user(d.user.as_str().into());
        app.set_db_dialog_password(d.password.as_str().into());
        app.set_db_dialog_save_password(d.save_password);
        app.set_db_dialog_trust_cert(d.trust_cert);
        app.set_db_dialog_url(d.url.as_str().into());
        d.needs_hydration = false;
    }
    if d.port_push {
        app.set_db_dialog_port(d.port.as_str().into());
        d.port_push = false;
    }
    app.set_db_dialog_ssl_index(d.ssl_index as i32);
    app.set_db_dialog_url_mode(d.url_mode);
    app.set_db_dialog_testing(d.testing);
    let token = d.test_token;
    let (note, is_error) = dialog_note(state, token);
    app.set_db_dialog_note(note.into());
    app.set_db_dialog_note_error(is_error);
}

/// The dialog's note line: probe result for this dialog's token, or
/// the last command error.
fn dialog_note(state: &AppState, token: u64) -> (String, bool) {
    if let Some(result) = &state.editor.db.last_test
        && (state.editor.db.test_token == token || token == 0)
    {
        return match result {
            Ok(label) => (label.clone(), false),
            Err(e) => (e.clone(), true),
        };
    }
    (String::new(), false)
}

