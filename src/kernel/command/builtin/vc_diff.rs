//! The side-by-side editor diff — JetBrains' two-pane change view.
//!
//! Opening a file's diff splits the editor area into two linked panes: the
//! left shows the file at HEAD, the right the working tree.  Changed lines
//! carry a colour and a gutter bar on both sides, the two halves scroll
//! together, and per-hunk stage/discard lives on the right (worktree) pane.
//! The pair stands in for the whole editor layout and reads as a single tab
//! (`{file}:HEAD->TREE`); the tabs it concealed return when it closes.
//!
//! Everything is derived from the zero-context patch: its `@@` headers give
//! both the changed-line marks and the hunk the cursor addresses, and the
//! same patch text is what `git apply` consumes when staging a hunk.

use crate::kernel::command::CommandResult;
use crate::kernel::command::args::{ArgSpec, ArgType};
use crate::kernel::render::gutter_providers::JanetProvider;
use crate::kernel::render::surface::Style;
use crate::kernel::render::view_tree::{LayoutConstraint, SplitDirection};
use crate::kernel::runtime::BackgroundEvent;
use crate::kernel::state::id::WindowId;
use crate::kernel::state::{Editor, MessageKind};
use crate::kernel::vc::{DiffData, DiffView, HiddenPane, VcRemoteOp, backends::split_hunks};

use super::vc_hunks::{Bar, HunkRange, changed_lines, hunk_covers, line_spans, parse_hunks};
use super::vcs::vc_spawn_op;

/// Gutter column that carries the diff bars.
const GUTTER: &str = ":vc-diff";
/// Highlight layer for changed-line colouring.
const LAYER: &str = "vc-diff";
/// Changed-line faces: a red band on the HEAD side, a green band on the
/// worktree side.
const DEL_FACE: &str = "diff-del-face";
const ADD_FACE: &str = "diff-add-face";

pub(super) fn register(editor: &mut Editor) {
    let cmds = &mut editor.commands;

    cmds.register_fn("vc-open-diff",
        "Open a file at HEAD and in the working tree, side by side, with per-hunk actions",
        vec![ArgSpec::new("path", ArgType::Path)],
        |editor, args| {
            let path = args.get("path").and_then(|a| a.as_string())
                .map(str::to_string).filter(|s| !s.is_empty())
                .ok_or_else(|| "a path is required".to_string())?;
            spawn_diff(editor, path)
        });

    cmds.register_fn("vc-stage-hunk",
        "Stage the hunk under the cursor in the right-hand diff pane",
        vec![],
        |editor, _args| hunk_op(editor, true));

    cmds.register_fn("vc-discard-hunk",
        "Discard the hunk under the cursor (rewrites the worktree)",
        vec![],
        |editor, _args| hunk_op(editor, false));

    cmds.register_fn("vc-close-diff",
        "Close the side-by-side diff and restore the previous editor layout",
        vec![],
        |editor, _args| { close_diff(editor); Ok(()) });

    cmds.register_fn("vc-fetch",
        "Fetch from the remote; output lands in *vc-fetch*",
        vec![],
        |editor, _args| vc_spawn_op(editor, VcRemoteOp::Fetch, Some("*vc-fetch*"), true));

    cmds.register_fn("vc-pull",
        "Pull from the remote; output lands in *vc-pull*",
        vec![],
        |editor, _args| vc_spawn_op(editor, VcRemoteOp::Pull, Some("*vc-pull*"), true));

    cmds.register_fn("vc-take-ours",
        "Resolve the conflict by keeping your side, then stage it",
        vec![ArgSpec::new("path", ArgType::Path)],
        |editor, args| resolve(editor, args, true));

    cmds.register_fn("vc-take-theirs",
        "Resolve the conflict by keeping the incoming side, then stage it",
        vec![ArgSpec::new("path", ArgType::Path)],
        |editor, args| resolve(editor, args, false));
}

// ── Fetching ─────────────────────────────────────────────────────────────────

/// Fetch the HEAD blob, the working-tree content, and the zero-context patch
/// off the interactive path; `Vc(DiffReady)` opens the two panes.
pub(crate) fn spawn_diff(editor: &mut Editor, path: String) -> CommandResult {
    let (backend, root) = active(editor)?;
    if let Some(bg) = editor.background.clone() {
        let sender = bg.sender.clone();
        bg.spawn_blocking(move || {
            let result = fetch_bundle(&root, &backend, &path);
            let _ = sender.send(BackgroundEvent::Vc(
                crate::kernel::vc::Event::DiffReady { path, result }));
        });
    } else {
        match fetch_bundle(&root, &backend, &path) {
            Ok(data) => open_diff(editor, &path, data),
            Err(e) => editor.show_message(MessageKind::Error, e),
        }
    }
    Ok(())
}

fn fetch_bundle(root: &std::path::Path, backend: &str, path: &str) -> Result<DiffData, String> {
    let patch = crate::kernel::vc::run_op(root, backend, &VcRemoteOp::HunkDiff(path.to_string()))
        .unwrap_or_default();
    let head = crate::kernel::vc::run_op(
        root, backend, &VcRemoteOp::CatFile { rev: "HEAD".into(), path: path.to_string() })
        .unwrap_or_default();
    // A new (untracked) file has no HEAD blob; a deleted file has no worktree
    // content.  Both are legitimate — the pane just renders empty.
    let work = std::fs::read_to_string(root.join(path)).unwrap_or_default();
    Ok(DiffData { head, work, patch })
}

/// Reload the diff view in place when a repository op changed the worktree.
pub(crate) fn reload_diff(editor: &mut Editor) {
    let Some(view) = editor.vc.diff_view.clone() else { return };
    if editor.view_tree.pane(view.left).is_none() || editor.view_tree.pane(view.right).is_none() {
        close_diff(editor);
        return;
    }
    let _ = spawn_diff(editor, view.path);
}

// ── The two panes ────────────────────────────────────────────────────────────

/// Show `path`'s HEAD and working-tree versions in two linked panes.
pub(crate) fn open_diff(editor: &mut Editor, path: &str, data: DiffData) {
    let (_left, _right, left_buf, right_buf, created) = ensure_pair(editor, path);
    let short = path.rsplit('/').next().unwrap_or(path);

    // The panes hold generated snapshots, not the file itself, so their
    // syntax comes from the source path rather than a `path` of their own.
    set_language_path(editor, left_buf, path);
    set_language_path(editor, right_buf, path);

    write_side(editor, left_buf, &format!("{short}:HEAD"), &data.head);
    write_side(editor, right_buf, &format!("{short}:Working Tree"), &data.work);

    let hunks = parse_hunks(&data.patch);
    mark_side(editor, left_buf, &data.head, &hunks, Bar::Del);
    mark_side(editor, right_buf, &data.work, &hunks, Bar::Add);

    bind_hunk_keys(editor, right_buf);

    if let Some(view) = editor.vc.diff_view.as_mut() {
        view.path = path.to_string();
        view.patch = data.patch.clone();
    }
    // A reload of an already-open pair keeps the user's focus and scroll; only
    // a freshly created pair is aligned at the top and focused.
    if created
        && let Some(view) = &editor.vc.diff_view {
            let (left, right) = (view.left, view.right);
            editor.view_tree.focus(right);
            for (id, buf) in [(right, right_buf), (left, left_buf)] {
                if let Some(p) = editor.view_tree.window_mut(id) {
                    p.scroll_offset = 0;
                    p.scroll_left = 0;
                    p.scroll_pinned = true;
                    p.scroll_left_pinned = true;
                    p.scroll_buf = Some(buf);
                }
            }
        }
}

/// Reuse the existing panes, or stand the two linked halves in for the whole
/// editor area.  Returns the two pane ids, the two buffer keys, and whether
/// the pair was created by this call.
fn ensure_pair(editor: &mut Editor, path: &str) -> (WindowId, WindowId, usize, usize, bool) {
    if let Some(view) = &editor.vc.diff_view
        && editor.view_tree.pane(view.left).is_some()
        && editor.view_tree.pane(view.right).is_some() {
            return (view.left, view.right, view.left_buf, view.right_buf, false);
    }

    // Conceal the user's whole layout — buffer, constraint and weight — so the
    // diff reads as a single tab and closing puts everything back untouched.
    let hidden: Vec<HiddenPane> = editor.view_tree.panes().iter()
        .map(|p| HiddenPane {
            id: p.id, buffer_id: p.buffer_id, layout: p.layout, weight: p.weight,
        })
        .collect();
    let left = editor.view_tree.focused_window().unwrap_or(WindowId(1));
    // A bufferless Tab pane neither renders nor hit-tests, so the two halves
    // tile the editor area alone.
    for pane in editor.view_tree.panes_mut() {
        pane.buffer_id = None;
        pane.layout = LayoutConstraint::Tab;
    }
    if let Some(p) = editor.view_tree.window_mut(left) {
        p.layout = LayoutConstraint::Split(SplitDirection::Horizontal);
    }
    let right = editor.view_tree.split_horizontal(left)
        .expect("splitting always yields a pane");

    let left_buf = editor.create_buffer("(HEAD)");
    let right_buf = editor.create_buffer("(Working Tree)");
    if let Some(p) = editor.view_tree.window_mut(left) { p.buffer_id = Some(left_buf); }
    if let Some(p) = editor.view_tree.window_mut(right) {
        p.buffer_id = Some(right_buf);
        p.layout = LayoutConstraint::Split(SplitDirection::Horizontal);
    }
    // One link id ties the two halves' scrolling together.
    let link = left_buf as u32;
    if let Some(p) = editor.view_tree.window_mut(left) { p.scroll_link = Some(link); }
    if let Some(p) = editor.view_tree.window_mut(right) { p.scroll_link = Some(link); }

    editor.vc.diff_view = Some(DiffView {
        left, right, left_buf, right_buf, path: path.to_string(),
        patch: String::new(), hidden,
    });
    (left, right, left_buf, right_buf, true)
}

/// Point a diff pane's syntax at the source file, so the highlighter colours
/// it as that language without the pane claiming a file of its own.
fn set_language_path(editor: &mut Editor, key: usize, path: &str) {
    if let Some(arc) = editor.buffers.get_mut(key) {
        arc.lock().unwrap_or_else(|e| e.into_inner()).language_path = Some(path.to_string());
    }
}

/// Write `text` into a read-only diff pane with a proper tab label.
fn write_side(editor: &mut Editor, key: usize, name: &str, text: &str) {
    {
        let arc = editor.buffers.get_mut(key).expect("diff pane buffer");
        let mut buf = arc.lock().unwrap_or_else(|e| e.into_inner());
        let len = buf.len();
        if len > 0 { buf.delete(0, len); }
        buf.insert(0, text);
        buf.name = name.to_string();
        buf.read_only = true;
        buf.ephemeral = true;
        buf.header_line = Some(name.to_string());
    }
    if let Some(view) = editor.view_of_buffer_mut(key) {
        view.set_cursor(0);
    }
}

/// Colour the changed lines and give them gutter bars.
fn mark_side(editor: &mut Editor, key: usize, text: &str, hunks: &[HunkRange], bar: Bar) {
    let left = matches!(bar, Bar::Del);
    let face = if left { DEL_FACE } else { ADD_FACE };
    let lines = changed_lines(hunks, left);
    let spans = line_spans(text);
    let ranges: Vec<(usize, usize, String)> = lines.iter()
        .filter_map(|l| spans.get(*l).map(|(s, e)| (*s, *e, face.to_string())))
        .collect();
    if let Some(view) = editor.view_of_buffer_mut(key) {
        view.set_highlights_layer(LAYER, ranges);
    }
    register_faces(editor);
    let bg = editor.theme_color("line-num-bg");
    let fg = editor.theme_color(if left { "diff-del" } else { "diff-add" });
    editor.gutter.add_provider(Box::new(JanetProvider {
        provider_name: GUTTER.to_string(),
        fn_name: None,
    }));
    let cells = lines.iter()
        .map(|l| (*l, "▎".to_string(), Style { fg, bg, ..Default::default() }))
        .collect();
    editor.gutter.set_provider_cells(GUTTER, key, cells);
}

/// Register the two changed-line faces: a red band on the HEAD side, a green
/// band on the worktree side, mirroring the gutter bars.
fn register_faces(editor: &mut Editor) {
    let bg = editor.theme_color("bg");
    let fg = editor.theme_color("fg");
    let band = |c| Style { fg, bg: blend(c, bg, 0.30), ..Default::default() };
    editor.faces.insert(DEL_FACE.into(), band(editor.theme_color("diff-del")));
    editor.faces.insert(ADD_FACE.into(), band(editor.theme_color("diff-add")));
}

/// Blend `over` onto `base`: `amount` 1.0 is pure `over`, 0.0 is pure `base`.
fn blend(over: (u8, u8, u8), base: (u8, u8, u8), amount: f32) -> (u8, u8, u8) {
    let mix = |o: u8, b: u8| (o as f32 * amount + b as f32 * (1.0 - amount)).round() as u8;
    (mix(over.0, base.0), mix(over.1, base.1), mix(over.2, base.2))
}

fn bind_hunk_keys(editor: &mut Editor, key: usize) {
    let k = key as u64;
    editor.keymaps.set_buffer_local(k, "return", "vc-stage-hunk");
    editor.keymaps.set_buffer_local(k, "s",      "vc-stage-hunk");
    editor.keymaps.set_buffer_local(k, "x",      "vc-discard-hunk");
    editor.keymaps.set_buffer_local(k, "q",      "vc-close-diff");
}

// ── Closing ──────────────────────────────────────────────────────────────────

/// Close the diff view: hand the concealed layout back and drop both panes.
pub fn close_diff(editor: &mut Editor) {
    let Some(view) = editor.vc.diff_view.take() else { return };
    // Restore every concealed pane before dropping the diff buffers, so no
    // pane is left pointing at one of them.
    for h in &view.hidden {
        if let Some(pane) = editor.view_tree.window_mut(h.id) {
            pane.buffer_id = h.buffer_id;
            pane.layout = h.layout;
            pane.weight = h.weight;
            pane.scroll_link = None;
        }
    }
    editor.view_tree.close(view.right);
    editor.drop_buffer(view.right_buf);
    editor.drop_buffer(view.left_buf);
    editor.view_tree.focus(view.left);
}

// ── Hunk actions ─────────────────────────────────────────────────────────────

/// Stage or discard the hunk under the cursor in the worktree pane.
fn hunk_op(editor: &mut Editor, stage: bool) -> CommandResult {
    let view = editor.vc.diff_view.clone()
        .ok_or_else(|| "No diff view open".to_string())?;
    let key = view.right_buf;
    let (cursor_line, patch) = {
        let cursor = editor.view_of_buffer(key).map(|v| v.cursor_offset()).unwrap_or(0);
        let arc = editor.buffers.get(key).ok_or_else(|| "No diff view open".to_string())?;
        let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
        let line = buf.slice(0, cursor).chars().filter(|&c| c == '\n').count();
        (line, view.patch.clone())
    };
    let hunks = parse_hunks(&patch);
    let index = hunks.iter().position(|h| hunk_covers(h, cursor_line))
        .ok_or_else(|| "No hunk on this line".to_string())?;
    let single = split_hunks(&patch).get(index).cloned()
        .ok_or_else(|| "Hunk is stale — reopen the diff".to_string())?;

    let op = if stage { VcRemoteOp::StagePatch(single) } else { VcRemoteOp::DiscardPatch(single) };
    vc_spawn_op(editor, op, None, false)?;
    if !stage {
        editor.show_message(MessageKind::Info, "hunk discarded");
    }
    Ok(())
}

fn resolve(editor: &mut Editor, args: &std::collections::HashMap<String, crate::kernel::command::args::ArgValue>, ours: bool) -> CommandResult {
    let path = args.get("path").and_then(|a| a.as_string())
        .map(str::to_string).filter(|s| !s.is_empty())
        .ok_or_else(|| "a path is required".to_string())?;
    let op = if ours { VcRemoteOp::ResolveOurs(path) } else { VcRemoteOp::ResolveTheirs(path) };
    vc_spawn_op(editor, op, None, true)
}

// ── shared ───────────────────────────────────────────────────────────────────

fn active(editor: &Editor) -> Result<(String, std::path::PathBuf), String> {
    match (editor.vc.active_backend_name.clone(), editor.vc.repo_root.clone()) {
        (Some(b), Some(r)) => Ok((b, r)),
        _ => Err("No active VC backend".to_string()),
    }
}
