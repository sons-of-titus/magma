//! Signature help — request on call punctuation, parse, dismiss, and the
//! background-event landing.

use crate::kernel::input;
use crate::kernel::runtime::process_background_event;
use crate::kernel::runtime::BackgroundEvent;
use crate::kernel::state::Editor;
use crate::tests::helpers::make_editor;

const SIGNATURE_PAYLOAD: &str = r#"{
    "activeSignature": 0,
    "activeParameter": 1,
    "signatures": [
        { "label": "fn insert(&mut self, offset: usize, text: &str)",
          "parameters": [{ "label": [21, 34] }, { "label": [36, 46] }] }
    ]
}"#;

fn editor_at_call_open() -> Editor {
    let mut ed = make_editor();
    let key = ed.create_buffer_from_str("call.rs", "foo.bar(\n");
    if let Some(win) = ed.view_tree.focused_window_mut() {
        win.buffer_id = Some(key);
    }
    // Cursor right after the open paren.
    ed.view_of_buffer_mut(key).unwrap().set_cursor(8);
    ed
}

fn deliver(ed: &mut Editor, json: &str, revision: u64) {
    let key = crate::tests::helpers::focused_key(ed);
    process_background_event(
        ed,
        BackgroundEvent::Lsp(crate::kernel::semantic::Event::SignatureHelp {
            path: "rust".into(),
            json: json.into(),
            buf_key: Some(key),
            revision,
        }),
    );
}

#[test]
fn a_fresh_response_populates_the_signature_popup() {
    let mut ed = editor_at_call_open();
    let revision = ed
        .buffers
        .get(crate::tests::helpers::focused_key(&ed))
        .unwrap()
        .lock()
        .unwrap()
        .revision();
    deliver(&mut ed, SIGNATURE_PAYLOAD, revision);

    assert!(ed.signature.visible);
    let sig = ed.signature.active().expect("one signature");
    assert!(sig.label.starts_with("fn insert("));
    assert_eq!(ed.signature.active_parameter, 1);
    let (start, end) = sig.params[1];
    assert_eq!(&sig.label[start..end], "text: &str");
}

#[test]
fn a_stale_response_never_lands() {
    let mut ed = editor_at_call_open();
    let key = crate::tests::helpers::focused_key(&ed);
    let stale = ed.buffers.get(key).unwrap().lock().unwrap().revision();
    // The buffer moves on while the response is in flight.
    ed.view_of_buffer_mut(key).unwrap().insert(0, "let x = ");
    deliver(&mut ed, SIGNATURE_PAYLOAD, stale);
    assert!(!ed.signature.visible, "a superseded revision must be dropped");
}

#[test]
fn a_null_response_hides_the_popup() {
    let mut ed = editor_at_call_open();
    let revision = ed
        .buffers
        .get(crate::tests::helpers::focused_key(&ed))
        .unwrap()
        .lock()
        .unwrap()
        .revision();
    deliver(&mut ed, SIGNATURE_PAYLOAD, revision);
    assert!(ed.signature.visible);

    // The server says there is no signature here (comment, string, …).
    deliver(&mut ed, "null", revision);
    assert!(!ed.signature.visible);
}

#[test]
fn close_paren_dismisses_and_open_paren_with_no_server_stays_hidden() {
    let mut ed = editor_at_call_open();
    let revision = ed
        .buffers
        .get(crate::tests::helpers::focused_key(&ed))
        .unwrap()
        .lock()
        .unwrap()
        .revision();
    deliver(&mut ed, SIGNATURE_PAYLOAD, revision);
    assert!(ed.signature.visible);

    input::dispatch_key(&mut ed, ")");
    assert!(!ed.signature.visible, ") closes the popup");
    let key = crate::tests::helpers::focused_key(&ed);
    let text = ed.buffers.get(key).unwrap().lock().unwrap().slice(0, 20);
    assert_eq!(text, "foo.bar()\n", ") still inserts");

    // No server attached: `(` inserts and the popup stays hidden.
    input::dispatch_key(&mut ed, "(");
    assert!(!ed.signature.visible);
    let text = ed.buffers.get(key).unwrap().lock().unwrap().slice(0, 20);
    assert_eq!(text, "foo.bar()(\n", "( still inserts");
}

#[test]
fn a_response_for_an_unfocused_buffer_is_ignored() {
    let mut ed = editor_at_call_open();
    let other = ed.create_buffer_from_str("other.rs", "x\n");
    let revision = ed
        .buffers
        .get(other)
        .unwrap()
        .lock()
        .unwrap()
        .revision();
    process_background_event(
        &mut ed,
        BackgroundEvent::Lsp(crate::kernel::semantic::Event::SignatureHelp {
            path: "rust".into(),
            json: SIGNATURE_PAYLOAD.into(),
            buf_key: Some(other),
            revision,
        }),
    );
    assert!(
        !ed.signature.visible,
        "background responses must not open the popup over the focused buffer"
    );
}
