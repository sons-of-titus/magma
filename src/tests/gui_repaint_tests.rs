//! Repaint scheduling — the shell must go quiet when nothing changed.
//!
//! Regression tests for the idle-CPU bugs: a pending atlas upload (welcome
//! surface), a stale blink deadline (terminal / normal modes), and the
//! per-frame window-title command each held `request_repaint()` open on
//! every frame, keeping the UI thread painting at display rate forever.

#![cfg(feature = "gui")]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui;

use crate::kernel::render::gui::GuiApp;
use crate::kernel::state::mode::EditorMode;
use crate::tests::helpers::make_editor;

/// Counts egui repaint requests that fire immediately (delay == 0).
fn immediate_repaint_counter(ctx: &egui::Context) -> Arc<AtomicUsize> {
    let count = Arc::new(AtomicUsize::new(0));
    let observed = count.clone();
    ctx.set_request_repaint_callback(move |info| {
        if info.delay == Duration::ZERO {
            observed.fetch_add(1, Ordering::Relaxed);
        }
    });
    count
}

/// One full egui pass plus the shell's repaint scheduling for it.
///
/// egui only reports an immediate request when it lowers the pending
/// repaint delay, and a pass resets that bookkeeping — so every cycle is
/// self-contained: an immediate request in this cycle raises the counter,
/// an idle schedule raises nothing.
fn schedule_cycle(app: &mut GuiApp, ctx: &egui::Context, draw_shell: bool) {
    if draw_shell {
        let mut out = ctx.run_ui(egui::RawInput::default(), |root| {
            let shell_ctx = root.ctx().clone();
            app.editor_shell(&shell_ctx, root);
        });
        out.textures_delta.clear();
    } else {
        app.schedule_repaint(ctx);
        let mut out = ctx.run_ui(egui::RawInput::default(), |_| {});
        out.textures_delta.clear();
    }
}

/// N scheduling cycles with the blink deadline pinned fresh, so a slow
/// test thread cannot turn a due blink into a false repaint count.
fn quiet_cycles(app: &mut GuiApp, ctx: &egui::Context, immediate: &AtomicUsize, n: usize, draw_shell: bool) -> usize {
    let before = immediate.load(Ordering::Relaxed);
    for _ in 0..n {
        app.last_blink = Instant::now();
        schedule_cycle(app, ctx, draw_shell);
    }
    immediate.load(Ordering::Relaxed) - before
}

#[test]
fn welcome_surface_with_pending_atlas_upload_stays_idle() {
    let mut app = GuiApp::new(make_editor());
    let ctx = egui::Context::default();
    let immediate = immediate_repaint_counter(&ctx);

    // Fresh app state: atlas upload pending, welcome surface, text mode.
    app.editor_open = false;
    // Two settle passes: the first paints the initial signature, the second
    // consumes egui's built-in "each request repaints twice" follow-up.
    schedule_cycle(&mut app, &ctx, false);
    schedule_cycle(&mut app, &ctx, false);

    assert_eq!(quiet_cycles(&mut app, &ctx, &immediate, 3, false), 0,
        "a pending atlas upload must not wake the welcome surface frame after frame");
}

#[test]
fn stale_blink_deadline_does_not_latch_in_non_text_modes() {
    let mut app = GuiApp::new(make_editor());
    let ctx = egui::Context::default();
    let immediate = immediate_repaint_counter(&ctx);

    app.editor.editor_mode = EditorMode::new("terminal", false);
    app.editor_open = true;
    app.atlas_dirty = false;
    schedule_cycle(&mut app, &ctx, false);
    schedule_cycle(&mut app, &ctx, false);

    // A deadline that went stale while the mode did not accept text must
    // stay stale without holding repaint_now open.
    app.last_blink = Instant::now() - Duration::from_millis(1000);
    assert_eq!(quiet_cycles(&mut app, &ctx, &immediate, 3, false), 0,
        "a frozen blink deadline in terminal mode must not request repaints forever");
}

#[test]
fn due_blink_in_text_mode_repaints_once_per_flip() {
    let mut app = GuiApp::new(make_editor());
    let ctx = egui::Context::default();
    let immediate = immediate_repaint_counter(&ctx);

    app.editor_open = true;
    app.atlas_dirty = false;
    schedule_cycle(&mut app, &ctx, false);
    schedule_cycle(&mut app, &ctx, false);

    app.last_blink = Instant::now() - Duration::from_millis(1000);
    schedule_cycle(&mut app, &ctx, false);
    assert!(immediate.load(Ordering::Relaxed) >= 1,
        "the flip cycle must repaint so the new cursor phase shows immediately");

    quiet_cycles(&mut app, &ctx, &immediate, 1, false);
    let settled = immediate.load(Ordering::Relaxed);
    quiet_cycles(&mut app, &ctx, &immediate, 1, false);
    assert_eq!(immediate.load(Ordering::Relaxed), settled,
        "a settled blink must not keep scheduling immediate repaints");
}

#[test]
fn idle_editor_shell_leaves_the_frame_clean_in_text_mode() {
    let mut app = GuiApp::new(make_editor());
    let ctx = egui::Context::default();

    app.editor_open = true;
    app.atlas_dirty = false;
    // An idle shell pass — no keys, no events, no chrome actions — must not
    // dirty the frame; a slow debug pass may still flip the blink, which is
    // the heartbeat's job, not the shell's.
    for _ in 0..3 {
        schedule_cycle(&mut app, &ctx, true);
        assert!(!app.editor.frame_dirty,
            "an idle editor shell in text mode must not dirty the frame");
    }
}

#[test]
fn idle_editor_shell_leaves_the_frame_clean_in_terminal_mode() {
    let mut app = GuiApp::new(make_editor());
    let ctx = egui::Context::default();

    app.editor.editor_mode = EditorMode::new("terminal", false);
    app.editor_open = true;
    app.atlas_dirty = false;
    for _ in 0..3 {
        schedule_cycle(&mut app, &ctx, true);
        assert!(!app.editor.frame_dirty,
            "an idle editor shell in terminal mode must not dirty the frame");
    }
}
