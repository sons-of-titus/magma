//! Database tool-window sync: one pass per refresh over the live DbState.

use slint::{ModelRc, VecModel};

use crate::{AppState, AppWindow, projection};

pub(super) fn sync(app: &AppWindow, state: &mut AppState) {
    let ed = &mut state.editor;
    app.set_db_sources(ModelRc::new(VecModel::from(
        projection::db_sources(ed)
            .into_iter()
            .map(Into::into)
            .collect::<Vec<slint::SharedString>>(),
    )));
    app.set_db_active_source(projection::db_active_source_name(ed).into());
    let connected = ed
        .db
        .active
        .map(|id| ed.db.is_connected(id))
        .unwrap_or(false);
    app.set_db_connected(connected);
    app.set_db_connected_label(projection::db_connected_label(ed).into());
    app.set_db_busy(ed.db.busy_metadata || ed.db.busy_query);
    app.set_db_open(state.db_open);
    app.set_db_height(state.db_height);
    app.set_db_form_open(state.db_form_open);
    app.set_db_form_note(state.db_form_note.as_str().into());
    app.set_db_tree(ModelRc::new(VecModel::from(projection::db_tree(
        ed,
        &state.db_expanded,
    ))));
    app.set_db_columns(ModelRc::new(VecModel::from(projection::db_columns(ed))));
    app.set_db_rows(ModelRc::new(VecModel::from(projection::db_rows(ed))));
    let (status, is_error) = projection::db_status(ed);
    app.set_db_status(status.into());
    app.set_db_status_error(is_error);
    app.set_db_truncated(
        ed.db
            .last_result
            .as_ref()
            .map(|r| r.truncated)
            .unwrap_or(false),
    );
    app.set_db_row_count(projection::db_row_count(ed).into());
    app.set_db_elapsed(projection::db_elapsed(ed).into());
}
