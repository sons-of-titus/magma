//! Commit and Debug tool-window sync: one pass per refresh over the live
//! VC and debug state, projecting the models the panels render.

use slint::{ModelRc, VecModel};

use crate::sync::STATUS_TTL;
use crate::{AppState, AppWindow, ChangeRow, TabItem, projection};
use kernel::render::tabs;
use kernel::state::Editor;

pub(super) fn sync_tabs(app: &AppWindow, state: &mut AppState) {
    let ed = &state.editor;
    let focused = ed.view_tree.focused_window();
    let tab_models: Vec<TabItem> = tabs::collect_tabs(ed)
        .into_iter()
        .map(|t| {
            let (icon, icon_color, icon_path, icon_image) = match t.icon {
                Some(kernel::icons::IconKind::Glyph { glyph, color }) => (
                    glyph.to_string(),
                    color
                        .map(|(r, g, b)| slint::Color::from_rgb_u8(r, g, b))
                        .unwrap_or(slint::Color::from_rgb_u8(166, 173, 200)),
                    String::new(),
                    slint::Image::default(),
                ),
                Some(kernel::icons::IconKind::Svg { path }) => {
                    let p = path.to_string_lossy().into_owned();
                    let img = match state.icon_images.get(&p) {
                        Some(img) => img.clone(),
                        None => {
                            let img = slint::Image::load_from_path(std::path::Path::new(&p))
                                .unwrap_or_default();
                            state.icon_images.insert(p.clone(), img.clone());
                            img
                        }
                    };
                    (
                        String::new(),
                        slint::Color::from_rgb_u8(166, 173, 200),
                        p,
                        img,
                    )
                }
                None => (
                    String::new(),
                    slint::Color::from_rgb_u8(166, 173, 200),
                    String::new(),
                    slint::Image::default(),
                ),
            };
            TabItem {
                title: t.label.into(),
                active: focused == Some(t.window_id) && ed.db.active_tab.is_none(),
                modified: t.modified,
                icon: icon.into(),
                icon_color,
                icon_path: icon_path.into(),
                icon_image,
            }
        })
        .chain(ed.db.tabs.iter().map(|tab| {
            let title = if tab.structure {
                format!("{} [structure]", tab.title)
            } else {
                tab.title.clone()
            };
            let icon = if matches!(tab.kind, kernel::db::DataTabKind::Table { .. }) {
                "\u{F04EB}"
            } else {
                "\u{F018D}"
            };
            TabItem {
                title: title.into(),
                active: ed.db.active_tab == Some(tab.id),
                modified: !tab.pending.is_empty(),
                icon: icon.into(),
                icon_color: slint::Color::from_rgb_u8(137, 180, 250),
                icon_path: slint::SharedString::default(),
                icon_image: slint::Image::default(),
            }
        }))
        .collect();
    state.m_tabs.set(tab_models, |m| app.set_tabs(m));
}

pub(super) fn sync_commit(app: &AppWindow, ed: &mut Editor, last_message: &str) {
    let status = ed.vc.last_status.clone();
    app.set_vc_has_repo(status.is_some());
    if let Some(status) = status {
        app.set_vc_repo_label(
            format!("{} · {}", status.backend_name, branch_or(&status.branch)).into(),
        );
        app.set_vc_branch(status.branch.as_str().into());
        app.set_vc_has_staging(status.has_staging);
        app.set_staged_changes(ModelRc::new(VecModel::from(
            status
                .staged
                .iter()
                .map(|f| change_row(f, true, "staged"))
                .collect::<Vec<_>>(),
        )));
        app.set_unstaged_changes(ModelRc::new(VecModel::from(
            status
                .unstaged
                .iter()
                .map(|f| change_row(f, false, "unstaged"))
                .collect::<Vec<_>>(),
        )));
        app.set_untracked_changes(ModelRc::new(VecModel::from(
            status
                .untracked
                .iter()
                .map(|p| untracked_row(p))
                .collect::<Vec<_>>(),
        )));
        app.set_conflict_changes(ModelRc::new(VecModel::from(
            status
                .conflicts
                .iter()
                .map(|p| conflict_row(p))
                .collect::<Vec<_>>(),
        )));
    } else {
        app.set_vc_repo_label("".into());
        app.set_vc_branch("".into());
        app.set_staged_changes(ModelRc::new(VecModel::from(Vec::<ChangeRow>::new())));
        app.set_unstaged_changes(ModelRc::new(VecModel::from(Vec::<ChangeRow>::new())));
        app.set_untracked_changes(ModelRc::new(VecModel::from(Vec::<ChangeRow>::new())));
        app.set_conflict_changes(ModelRc::new(VecModel::from(Vec::<ChangeRow>::new())));
    }
    let branches: Vec<slint::SharedString> =
        ed.vc.branches.iter().cloned().map(Into::into).collect();
    app.set_vc_branches(ModelRc::new(VecModel::from(branches)));
    app.set_vc_busy(ed.vc.pending_ops > 0);

    // A recent repository failure surfaces in the panel, where the action
    // that failed lives; the status bar keeps its own TTL copy.
    let (message, error) = projection::status_message(ed, last_message, STATUS_TTL);
    let banner = if error && !message.is_empty() {
        message
    } else {
        String::new()
    };
    app.set_vc_error(banner.as_str().into());
}

pub(super) fn branch_or(branch: &str) -> &str {
    if branch.is_empty() {
        "(no branch)"
    } else {
        branch
    }
}

pub(super) fn change_row(file: &kernel::vc::FileStatus, staged: bool, kind: &str) -> ChangeRow {
    let path = &file.path;
    ChangeRow {
        path: path.clone().into(),
        name: path.rsplit('/').next().unwrap_or(path).into(),
        dir: match path.rfind('/') {
            Some(i) => path[..i].into(),
            None => "".into(),
        },
        code: file.code.to_string().into(),
        kind: kind.into(),
        staged: staged.into(),
    }
}

pub(super) fn untracked_row(path: &str) -> ChangeRow {
    ChangeRow {
        path: path.into(),
        name: path.rsplit('/').next().unwrap_or(path).into(),
        dir: match path.rfind('/') {
            Some(i) => path[..i].into(),
            None => "".into(),
        },
        code: "?".into(),
        kind: "untracked".into(),
        staged: false.into(),
    }
}

pub(super) fn conflict_row(path: &str) -> ChangeRow {
    ChangeRow {
        path: path.into(),
        name: path.rsplit('/').next().unwrap_or(path).into(),
        dir: match path.rfind('/') {
            Some(i) => path[..i].into(),
            None => "".into(),
        },
        code: "!".into(),
        kind: "conflict".into(),
        staged: false.into(),
    }
}

/// Debug tool window: one pass over the active session, plus the shell-side
/// variables-expansion set.  A new scope reference (fresh stop) re-opens the
/// scope group and collapses everything else.
pub(super) fn sync_debug(app: &AppWindow, state: &mut AppState) {
    let ed = &state.editor;
    let snapshot = projection::debug_snapshot(ed, &state.debug_expanded);
    state.debug_ref_slots = snapshot.ref_slots.clone();

    if let Some(session) = ed.debug.active_session()
        && session.scope_ref != state.debug_scope_seen
    {
        state.debug_scope_seen = session.scope_ref;
        state.debug_expanded.clear();
        if let Some(r) = session.scope_ref {
            state.debug_expanded.insert(r);
        }
    }

    app.set_debug_phase(snapshot.phase.as_str().into());
    app.set_debug_status(snapshot.status_text.as_str().into());
    app.set_debug_adapter(snapshot.adapter.as_str().into());
    let can = snapshot.can();
    app.set_debug_can_resume(can.resume);
    app.set_debug_can_pause(can.pause);
    app.set_debug_can_step(can.step);
    app.set_debug_can_stop(can.stop);
    app.set_debug_can_restart(can.restart);
    app.set_debug_frames(ModelRc::new(VecModel::from(snapshot.frames)));
    app.set_debug_vars(ModelRc::new(VecModel::from(snapshot.variables)));
    app.set_debug_watches(ModelRc::new(VecModel::from(snapshot.watches)));
    app.set_debug_breakpoints(ModelRc::new(VecModel::from(snapshot.breakpoints)));
    app.set_debug_console(ModelRc::new(VecModel::from(snapshot.console)));
    app.set_debug_console_total(snapshot.console_total as i32);
}
