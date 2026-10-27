//! IME preedit tests (R-15): composition events surface through the GUI
//! input collector, and a commit inserts like any text input.

use eframe::egui;

use crate::tests::helpers;

#[test]
fn ime_preedit_and_commit_reach_the_collect_surface() {
    let ctx = egui::Context::default();
    ctx.begin_pass(egui::RawInput {
        events: vec![
            egui::Event::Ime(egui::ImeEvent::Preedit {
                text: "こん".to_string(),
                active_range_chars: Some(0..2),
            }),
            egui::Event::Ime(egui::ImeEvent::Commit("こんにちは".to_string())),
        ],
        ..Default::default()
    });
    let mut wheel = 0.0;
    let mut wheel_x = 0.0;
    let (_keys, _mice, ime) =
        crate::render::gui_input::collect(&ctx, 16.0, &mut wheel, &mut wheel_x);
    assert_eq!(ime, vec![
        crate::render::gui_input::ImeInput::Preedit {
            text: "こん".to_string(),
            cursor_char: Some(0),
        },
        crate::render::gui_input::ImeInput::Commit("こんにちは".to_string()),
    ]);
}

#[test]
fn empty_preedit_clears_the_composition() {
    let ctx = egui::Context::default();
    ctx.begin_pass(egui::RawInput {
        events: vec![
            egui::Event::Ime(egui::ImeEvent::Preedit {
                text: "に".to_string(),
                active_range_chars: None,
            }),
            egui::Event::Ime(egui::ImeEvent::Preedit {
                text: String::new(),
                active_range_chars: None,
            }),
        ],
        ..Default::default()
    });
    let mut wheel = 0.0;
    let mut wheel_x = 0.0;
    let (_keys, _mice, ime) =
        crate::render::gui_input::collect(&ctx, 16.0, &mut wheel, &mut wheel_x);
    assert_eq!(ime.len(), 2);
    assert_eq!(ime[1].clone(), crate::render::gui_input::ImeInput::Preedit {
        text: String::new(),
        cursor_char: None,
    }, "the empty preedit must arrive so the shell clears its overlay");
}

#[test]
fn ime_commit_inserts_into_the_buffer_like_text_input() {
    let mut ed = helpers::make_editor();
    ed.editor_mode = magma::kernel::state::EditorMode::new("insert", true);
    magma::kernel::input::dispatch_key(&mut ed, "こんにちは");
    assert_eq!(helpers::buf_text(&ed), "こんにちは");
}
