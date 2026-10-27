use std::path::Path;

/// A throwaway git repo with one committed file.  Returns (root, file path).
fn scratch_git_repo(tag: &str) -> (std::path::PathBuf, String) {
    let root = std::env::temp_dir().join(format!(
        "magma-vc-f23-{tag}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let file = root.join("work.txt");
    std::fs::write(&file, "line1\nline2\nline3\nline4\nline5\nline6\n").unwrap();
    run_git(&root, &["init", "-q"]);
    run_git(
        &root,
        &["-c", "user.email=t@t", "-c", "user.name=t", "add", "."],
    );
    run_git(
        &root,
        &[
            "-c",
            "user.email=t@t",
            "-c",
            "user.name=t",
            "commit",
            "-q",
            "-m",
            "initial",
        ],
    );
    (root, file.to_string_lossy().into_owned())
}

fn run_git(root: &Path, args: &[&str]) -> String {
    crate::kernel::vc::run_cmd(root, "git", args).unwrap_or_default()
}

// ── Side-by-side editor diff ─────────────────────────────────────────────────

#[test]
fn vc_open_diff_opens_two_linked_read_only_panes() {
    let git_ok = crate::kernel::vc::run_cmd(Path::new("."), "git", &["--version"]).is_ok();
    if !git_ok {
        eprintln!("git unavailable; skipping diff test");
        return;
    }
    let (root, file) = scratch_git_repo("diff");
    std::fs::write(&file, "line1\nCHANGED2\nline3\nline4\nline5\nline6\n").unwrap();

    let mut ed = crate::tests::helpers::make_editor();
    let focused = crate::tests::helpers::focused_key(&ed);
    ed.buffers.get(focused).unwrap().lock().unwrap().path = Some(file);

    // `vc` detects the repo and syncs state (no background pool → synchronous).
    crate::tests::helpers::run(&mut ed, "vc");
    crate::kernel::command::builtin::vc_diff::spawn_diff(&mut ed, "work.txt".to_string()).unwrap();

    let view = ed.vc.diff_view.clone().expect("the diff view must open");
    assert!(
        ed.view_tree.pane(view.left).is_some() && ed.view_tree.pane(view.right).is_some(),
        "both diff panes must exist"
    );
    assert_eq!(ed.view_tree.panes().len(), 2, "the editor is split in two");

    // Left = HEAD, right = working tree, both read-only.
    let left = ed.buffers.get(view.left_buf).unwrap().lock().unwrap();
    let (left_text, left_ro) = (left.slice(0, left.len()), left.read_only);
    drop(left);
    let right = ed.buffers.get(view.right_buf).unwrap().lock().unwrap();
    let (right_text, right_ro) = (right.slice(0, right.len()), right.read_only);
    drop(right);
    assert!(left_ro && right_ro, "both diff panes are read-only");
    assert!(
        left_text.contains("line2"),
        "the left pane shows the committed version"
    );
    assert!(
        right_text.contains("CHANGED2"),
        "the right pane shows the working tree"
    );

    // The two halves scroll together.
    let link = ed.view_tree.pane(view.left).unwrap().scroll_link;
    assert!(link.is_some());
    assert_eq!(
        ed.view_tree.pane(view.right).unwrap().scroll_link,
        link,
        "both halves share one scroll link"
    );

    // Hunk actions are buffer-local to the worktree pane — no global layer.
    assert_eq!(
        ed.keymaps
            .resolve_for_buffer("x", Some(view.right_buf as u64))
            .as_deref(),
        Some("vc-discard-hunk")
    );
    assert!(
        !ed.keymaps.is_layer_active("vc"),
        "opening the diff must not activate a global vc layer"
    );
    assert!(
        ed.gutter
            .sign_cache
            .keys()
            .any(|(name, buf, _)| name == ":vc-diff" && *buf == view.right_buf),
        "the worktree pane carries gutter marks"
    );

    // Closing restores the pre-split layout and drops nothing else.
    crate::kernel::command::builtin::vc_diff::close_diff(&mut ed);
    assert!(ed.vc.diff_view.is_none(), "the diff view is closed");
    assert_eq!(
        ed.view_tree.panes().len(),
        1,
        "back to a single editor pane"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn vc_open_diff_is_one_tab_and_hands_other_tabs_back() {
    let git_ok = crate::kernel::vc::run_cmd(Path::new("."), "git", &["--version"]).is_ok();
    if !git_ok {
        eprintln!("git unavailable; skipping diff-tab test");
        return;
    }
    let (root, file) = scratch_git_repo("difftab");
    std::fs::write(&file, "line1\nCHANGED2\nline3\nline4\nline5\nline6\n").unwrap();

    let mut ed = crate::tests::helpers::make_editor();
    let focused = crate::tests::helpers::focused_key(&ed);
    ed.buffers.get(focused).unwrap().lock().unwrap().path = Some(file);

    // A second file open in its own tab, so the diff has something to conceal.
    let other = ed.create_buffer_from_str("other.txt", "keep me\n");
    ed.view_tree.open_tab(Some(other));

    crate::tests::helpers::run(&mut ed, "vc");
    crate::kernel::command::builtin::vc_diff::spawn_diff(&mut ed, "work.txt".to_string()).unwrap();

    let view = ed.vc.diff_view.clone().expect("diff view");
    // Only the two diff halves carry a buffer; the other tab is concealed.
    let showing: Vec<_> = ed
        .view_tree
        .panes()
        .iter()
        .filter(|p| p.buffer_id.is_some())
        .map(|p| p.id)
        .collect();
    assert_eq!(showing.len(), 2, "only the two diff halves render");
    assert!(
        ed.view_tree
            .panes()
            .iter()
            .any(|p| p.id != view.left && p.id != view.right && p.buffer_id.is_none()),
        "the other tab is concealed, not dropped"
    );

    // The pair reads as one tab, labelled `{file}:HEAD->TREE`.
    let tabs = crate::kernel::render::tabs::collect_tabs(&ed);
    assert_eq!(tabs.len(), 1, "the diff is a single tab");
    assert_eq!(tabs[0].label, "work.txt:HEAD->TREE");

    // Closing hands the concealed tab back, buffer and all.
    crate::kernel::command::builtin::vc_diff::close_diff(&mut ed);
    assert!(
        ed.view_tree
            .panes()
            .iter()
            .any(|p| p.buffer_id == Some(other)),
        "the concealed tab returns on close"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn vc_open_diff_highlights_changed_lines() {
    let git_ok = crate::kernel::vc::run_cmd(Path::new("."), "git", &["--version"]).is_ok();
    if !git_ok {
        eprintln!("git unavailable; skipping diff-highlight test");
        return;
    }
    let (root, file) = scratch_git_repo("diffhl");
    std::fs::write(&file, "line1\nCHANGED2\nline3\nline4\nline5\nline6\n").unwrap();

    let mut ed = crate::tests::helpers::make_editor();
    let focused = crate::tests::helpers::focused_key(&ed);
    ed.buffers.get(focused).unwrap().lock().unwrap().path = Some(file);

    crate::tests::helpers::run(&mut ed, "vc");
    crate::kernel::command::builtin::vc_diff::spawn_diff(&mut ed, "work.txt".to_string()).unwrap();
    let view = ed.vc.diff_view.clone().expect("diff view");

    for (buf, needle, face) in [
        (view.right_buf, "CHANGED2", "diff-add-face"),
        (view.left_buf, "line2", "diff-del-face"),
    ] {
        let text = {
            let arc = ed.buffers.get(buf).unwrap();
            let b = arc.lock().unwrap_or_else(|e| e.into_inner());
            b.slice(0, b.len())
        };
        let at = text.find(needle).expect("needle in pane") as usize;
        let bview = ed.views.get(&buf).expect("diff pane view");
        let layer = bview
            .highlight_layers
            .get("vc-diff")
            .unwrap_or_else(|| panic!("the vc-diff highlight layer must exist"));
        assert!(
            layer
                .iter()
                .any(|(s, e, f)| *s <= at && at < *e && f == face),
            "the changed line at byte {at} must be covered by {face}: {layer:?}"
        );
        assert!(
            ed.faces.contains_key(face),
            "{face} must be a registered face"
        );
    }

    // The band reaches the painted surface: the changed row's cells carry the
    // face, so the line reads as removed/added, not plain text.
    ed.view_tree.resize(120, 20);
    let mut surface = crate::kernel::render::surface::Surface::new(120, 20);
    crate::kernel::render::frame::render_frame(&ed, &mut surface);
    for (pane_id, face) in [(view.left, "diff-del-face"), (view.right, "diff-add-face")] {
        let pane = ed.view_tree.pane(pane_id).unwrap();
        let style = *ed.faces.get(face).unwrap();
        let painted = (pane.x..pane.x + pane.width)
            .flat_map(|x| (pane.y..pane.y + pane.height).map(move |y| (x, y)))
            .any(|(x, y)| {
                surface
                    .cell(x, y)
                    .map(|c| c.style == style)
                    .unwrap_or(false)
            });
        assert!(painted, "{face} must paint the changed row on the surface");
    }

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn vc_diff_panes_take_syntax_from_the_source_path() {
    let git_ok = crate::kernel::vc::run_cmd(Path::new("."), "git", &["--version"]).is_ok();
    if !git_ok {
        eprintln!("git unavailable; skipping diff-syntax test");
        return;
    }
    let (root, file) = scratch_git_repo("diffsyn");
    std::fs::write(&file, "line1\nCHANGED2\nline3\nline4\nline5\nline6\n").unwrap();

    let mut ed = crate::tests::helpers::make_editor();
    let focused = crate::tests::helpers::focused_key(&ed);
    ed.buffers.get(focused).unwrap().lock().unwrap().path = Some(file);

    crate::tests::helpers::run(&mut ed, "vc");
    crate::kernel::command::builtin::vc_diff::spawn_diff(&mut ed, "work.txt".to_string()).unwrap();
    let view = ed.vc.diff_view.clone().expect("diff view");

    for buf in [view.left_buf, view.right_buf] {
        let arc = ed.buffers.get(buf).unwrap();
        let b = arc.lock().unwrap_or_else(|e| e.into_inner());
        assert!(
            b.path.is_none(),
            "a diff pane must not claim the file — :w would overwrite it"
        );
        assert_eq!(
            b.language_path.as_deref(),
            Some("work.txt"),
            "the pane's syntax must be inferred from the source path"
        );
    }

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn vc_diff_renders_when_the_two_sides_differ_in_length() {
    let git_ok = crate::kernel::vc::run_cmd(Path::new("."), "git", &["--version"]).is_ok();
    if !git_ok {
        eprintln!("git unavailable; skipping diff-length test");
        return;
    }
    let (root, file) = scratch_git_repo("difflen");
    // The working tree grows far past HEAD, so the linked scroll mirrors a
    // top line the shorter HEAD pane does not have.
    let long: String = (0..120).map(|i| format!("line {i}\n")).collect();
    std::fs::write(&file, &long).unwrap();

    let mut ed = crate::tests::helpers::make_editor();
    let focused = crate::tests::helpers::focused_key(&ed);
    ed.buffers.get(focused).unwrap().lock().unwrap().path = Some(file);

    crate::tests::helpers::run(&mut ed, "vc");
    crate::kernel::command::builtin::vc_diff::spawn_diff(&mut ed, "work.txt".to_string()).unwrap();
    let view = ed.vc.diff_view.clone().expect("diff view");

    let end = {
        let arc = ed.buffers.get(view.right_buf).unwrap();
        let b = arc.lock().unwrap_or_else(|e| e.into_inner());
        b.len()
    };
    ed.views.get_mut(&view.right_buf).unwrap().set_cursor(end);
    ed.view_tree.resize(80, 6);
    crate::kernel::render::frame::sync_viewport_scroll(&mut ed);

    // Rendering the pinned, shorter pane must not underflow its line count.
    let mut surface = crate::kernel::render::surface::Surface::new(80, 6);
    crate::kernel::render::frame::render_frame(&ed, &mut surface);

    let _ = std::fs::remove_dir_all(&root);
}
