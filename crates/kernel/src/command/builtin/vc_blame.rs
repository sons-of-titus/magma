//! Git blame annotations (`vc-blame`).
//!
//! Blame shells out on the maintenance lane — a large repository takes
//! hundreds of milliseconds, far too long to block a keystroke — and lands
//! via `Vc(Event::BlameReady)`.  Results become `LinePrefix` decorations;
//! a generation counter drops runs that a toggle superseded.

use crate::command::CommandResult;
use crate::state::Editor;

use super::helpers::*;
use super::vcs::vc_working_dir;

/// Annotate the current buffer with per-line blame information rendered as
/// `LinePrefix` text decorations (F-24).  Toggles: a second invocation
/// clears the layer.
///
pub(crate) fn vc_blame(editor: &mut Editor) -> CommandResult {
    let buf_id = get_current_buffer_id(editor);
    let path = editor
        .buffers
        .get(buf_id)
        .and_then(|a| a.lock().unwrap_or_else(|e| e.into_inner()).path.clone())
        .ok_or_else(|| "current buffer has no file path".to_string())?;

    let dir = vc_working_dir(editor);
    let (backend_idx, root) = match editor.vc.detect_for(&dir) {
        Some(v) => v,
        None => return Err("No version-control repository found".to_string()),
    };
    editor.vc.active_backend = Some(backend_idx);
    editor.vc.active_backend_name = Some(editor.vc.backends[backend_idx].name().to_string());
    editor.vc.repo_root = Some(root.clone());

    let backend = editor.vc.active_backend_name.clone().unwrap();
    // Toggle off if already annotated.
    let already = editor
        .view_of_buffer(buf_id)
        .map(|v| v.decor_count_layer("vc-blame") > 0)
        .unwrap_or(false);
    if already {
        editor.vc.blame_gen += 1;
        if let Some(view) = editor.view_of_buffer_mut(buf_id) {
            view.decor_clear_layer("vc-blame");
        }
        editor.frame_dirty = true;
        return Ok(());
    }

    editor.vc.blame_gen += 1;
    let generation = editor.vc.blame_gen;
    let Some(bg) = editor.background.clone() else {
        let result = crate::vc::run_op(&root, &backend, &crate::vc::VcRemoteOp::Blame(path));
        return apply_blame(editor, buf_id, generation, result);
    };
    let sender = bg.sender.clone();
    bg.spawn_blocking(move || {
        let result = crate::vc::run_op(&root, &backend, &crate::vc::VcRemoteOp::Blame(path));
        logging::LogErr::log_warn(sender.send(crate::runtime::BackgroundEvent::Vc(
            crate::vc::Event::BlameReady {
                buf_id,
                generation,
                result,
            },
        )));
    });
    Ok(())
}

/// Apply a finished blame run: stale generations and buffers whose file has
/// changed identity are dropped, everything else becomes prefix decorations.
pub(crate) fn apply_blame(
    editor: &mut Editor,
    buf_id: usize,
    generation: u64,
    result: Result<String, String>,
) -> CommandResult {
    if editor.vc.blame_gen != generation {
        return Ok(());
    }
    if editor.buffers.get(buf_id).is_none() {
        return Ok(());
    }
    let lines = match result {
        Ok(raw) => crate::vc::parse_blame_tsv(&raw),
        Err(e) => return Err(e),
    };
    if let Some(view) = editor.view_of_buffer_mut(buf_id) {
        view.decor_clear_layer("vc-blame");
    }
    for (i, bl) in lines.into_iter().enumerate() {
        let annotation = bl.annotation();
        if annotation.is_empty() {
            continue;
        }
        if let Some(view) = editor.view_of_buffer_mut(buf_id) {
            view.decor_set_prefix("vc-blame", i, annotation, "comment-face".to_string());
        }
    }
    editor.frame_dirty = true;
    Ok(())
}

/// Re-run blame for a saved file that is currently annotated, so the
/// annotations track the lines just committed to disk instead of going
/// stale until the next manual toggle.
pub(crate) fn refresh_blame_after_save(editor: &mut Editor, path: &str) {
    let Some(buf_id) = editor
        .buffers
        .iter()
        .find(|(_, a)| a.lock().unwrap_or_else(|e| e.into_inner()).path.as_deref() == Some(path))
        .map(|(k, _)| k)
    else {
        return;
    };
    let annotated = editor
        .view_of_buffer(buf_id)
        .map(|v| v.decor_count_layer("vc-blame") > 0)
        .unwrap_or(false);
    if !annotated {
        return;
    }
    let dir = vc_working_dir(editor);
    let Some((backend_idx, root)) = editor.vc.detect_for(&dir) else {
        return;
    };
    editor.vc.active_backend = Some(backend_idx);
    editor.vc.active_backend_name = Some(editor.vc.backends[backend_idx].name().to_string());
    editor.vc.repo_root = Some(root.clone());
    let backend = editor.vc.active_backend_name.clone().unwrap();
    let Some(bg) = editor.background.clone() else {
        return;
    };
    editor.vc.blame_gen += 1;
    let generation = editor.vc.blame_gen;
    let path = path.to_string();
    let sender = bg.sender.clone();
    bg.spawn_blocking(move || {
        let result = crate::vc::run_op(&root, &backend, &crate::vc::VcRemoteOp::Blame(path));
        logging::LogErr::log_warn(sender.send(crate::runtime::BackgroundEvent::Vc(
            crate::vc::Event::BlameReady {
                buf_id,
                generation,
                result,
            },
        )));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn editor() -> Editor {
        let mut ed = Editor::new(Box::new(crate::storage::disk::DiskFileSystem::new()));
        super::super::register_builtin_commands(&mut ed);
        ed
    }

    #[test]
    fn stale_generation_drops_the_run() {
        let mut ed = editor();
        let key = ed.create_buffer_from_str("blamed", "one\ntwo\n");
        ed.vc.blame_gen = 5;
        let tsv = "abc12345\tAlice\t2026-10-25\ndef67890\tBob\t2026-10-24\n";
        apply_blame(&mut ed, key, 4, Ok(tsv.to_string())).unwrap();
        assert_eq!(
            0,
            ed.view_of_buffer(key)
                .map(|v| v.decor_count_layer("vc-blame"))
                .unwrap_or(0),
            "a run from a superseded generation must not paint"
        );
    }

    #[test]
    fn current_generation_becomes_prefix_decorations() {
        let mut ed = editor();
        let key = ed.create_buffer_from_str("blamed", "one\ntwo\n");
        ed.vc.blame_gen = 7;
        let tsv = "abc12345\tAlice\t2026-10-25\n\t\t\n";
        apply_blame(&mut ed, key, 7, Ok(tsv.to_string())).unwrap();
        let view = ed.view_of_buffer(key).unwrap();
        assert_eq!(
            view.decor_count_layer("vc-blame"),
            1,
            "empty annotations skipped"
        );
        let decors = view.decoration_layers.get("vc-blame").unwrap();
        match &decors[0] {
            crate::text_engine::Decoration::LinePrefix { line, text, .. } => {
                assert_eq!(*line, 0);
                assert_eq!(text, "Alice 2026-10-25");
            }
            other => panic!("expected LinePrefix, got {other:?}"),
        }
        assert!(ed.frame_dirty, "the frame must be marked for repaint");
    }

    #[test]
    fn missing_buffer_is_dropped_quietly() {
        let mut ed = editor();
        ed.vc.blame_gen = 3;
        let tsv = "abc\tAlice\t2026-10-25\n";
        // 99 never exists; must not panic and must not paint anything.
        apply_blame(&mut ed, 99, 3, Ok(tsv.to_string())).unwrap();
        assert!(ed.view_of_buffer(99).is_none());
    }

    #[test]
    fn error_result_surfaces_as_command_error() {
        let mut ed = editor();
        let key = ed.create_buffer_from_str("blamed", "one\n");
        ed.vc.blame_gen = 2;
        let err = apply_blame(&mut ed, key, 2, Err("git blew up".into()));
        assert_eq!(err.unwrap_err(), "git blew up");
    }
}
