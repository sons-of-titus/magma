// Interactive-path latency probes (R-7 follow-up to the user report:
// typing degraded and TAB blocked on large buffers).
//
// These run the REAL pipeline — Janet keymap loaded, insert-mode keys,
// the syntax/completion hooks — against a 100 KB buffer, and gate the
// per-keystroke cost.  The pre-fix behavior (inline word-set rebuild per
// keystroke, full tree-sitter re-highlight per keystroke) fails these.

use kernel::input;
use scripting;
#[macro_use] mod helpers;

const KEYS: usize = 30;

#[test]
fn typing_stays_fast_on_a_100kb_buffer() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor();
    scripting::init(&mut ed);
    let _scope = scripting::EditorScope::new(&mut ed);

    let body = "fn alpha_one() { let beta_two = three(); }\n".repeat(100_000 / 44);
    let key = ed.create_buffer_from_str("big.rs", &body);
    if let Some(win) = ed.view_tree.focused_window_mut() {
        win.buffer_id = Some(key);
    }
    let _ = scripting::eval("(editor/run-command \"set-major-mode\" \"rust-mode\")");
    // Insert mode, mid-document cursor.
    ed.editor_mode = kernel::state::EditorMode::new("insert", true);
    if let Some(view) = ed.view_of_buffer_mut(key) {
        view.set_cursor(body.len() / 2);
    }

    let mut samples: Vec<u128> = Vec::new();
    for i in 0..KEYS {
        let t0 = std::time::Instant::now();
        input::dispatch_key(&mut ed, if i % 5 == 4 { "return" } else { "x" });
        samples.push(t0.elapsed().as_millis());
    }
    samples.sort_unstable();
    let p50 = samples[samples.len() / 2];
    let p100 = samples[samples.len() - 1];
    println!("typing on 100 KB: p50 {p50} ms, max {p100} ms over {KEYS} keys");
    assert!(p50 <= 10, "typing p50 {p50} ms — the keystroke path regressed");
    assert!(p100 <= 100, "typing max {p100} ms — an O(document) pass is back on the keystroke path");
}

#[test]
fn indent_line_on_a_100kb_buffer_does_not_block() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor();
    scripting::init(&mut ed);
    let _scope = scripting::EditorScope::new(&mut ed);

    let body = "fn alpha_one() { let beta_two = three(); }\n".repeat(100_000 / 44);
    let key = ed.create_buffer_from_str("big.rs", &body);
    if let Some(win) = ed.view_tree.focused_window_mut() {
        win.buffer_id = Some(key);
    }
    let _ = scripting::eval("(editor/run-command \"set-major-mode\" \"rust-mode\")");
    if let Some(view) = ed.view_of_buffer_mut(key) {
        view.set_cursor(body.len() / 2);
    }

    let t0 = std::time::Instant::now();
    for _ in 0..10 {
        input::dispatch_key(&mut ed, "tab");
    }
    let elapsed = t0.elapsed().as_millis();
    println!("10 × tab on 100 KB: {elapsed} ms total");
    assert!(elapsed <= 500,
        "TAB took {elapsed} ms for 10 presses — it is blocking again");
}
