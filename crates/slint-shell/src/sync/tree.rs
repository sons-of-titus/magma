//! Project-tree projection: editor file list → sidebar rows.
//!
//! Rebuilds the node tree when the walk epoch advances and flattens the
//! visible rows through the expanded set.  Expansion state survives
//! incremental walk batches; a new project root starts fully expanded.

use std::collections::HashSet;

use kernel::project::tree::{build, collect_dirs};

use crate::{AppState, AppWindow, projection};

pub(crate) fn sync_tree(app: &AppWindow, state: &mut AppState) {
    let project = &state.editor.project_manager.project;
    let root = project.root.clone();
    if state.tree_epoch != project.files_epoch || state.tree_root != root {
        let new_root = state.tree_root != root;
        state.tree_epoch = project.files_epoch;
        state.tree_root = root;
        let mut all: Vec<String> = project.files.clone();
        all.extend(project.ignored_files.iter().cloned());
        let ignored: HashSet<String> = project.ignored_files.clone();
        state.tree_nodes = build(&all, &ignored);
        let mut dirs = HashSet::new();
        collect_dirs(&state.tree_nodes, &mut dirs);
        if new_root {
            state.expanded = dirs.clone();
        } else {
            // Incremental walk batches rebuild the tree repeatedly; only
            // directories first seen in this batch open automatically, so
            // the user's expansions survive the load.
            for dir in &dirs {
                if !state.dirs.contains(dir) {
                    state.expanded.insert(dir.clone());
                }
            }
        }
        state.dirs = dirs;
    }

    let (title, empty) = match state.rail {
        1 => ("Commit", ""),
        2 => ("Debugger", ""),
        _ => (
            "Project",
            if !state.tree_nodes.is_empty() {
                "No matching files"
            } else if project.indexing {
                "Indexing files…"
            } else {
                "No project open. Use File › Open Folder…"
            },
        ),
    };
    app.set_tree_title(title.into());
    app.set_tree_empty(empty.into());

    let rows = if state.rail == 0 {
        projection::tree_rows(&state.editor, &state.tree_nodes, &state.expanded)
    } else {
        Vec::new()
    };
    state.m_tree.set(rows, |m| app.set_tree(m));
    app.set_selected_path(projection::current_path(&state.editor).into());
}
