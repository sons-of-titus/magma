#[macro_use]
mod helpers;
use kernel::state::{HoverCard, HoverLine};
use scripting;

fn card_with_word(word: &str) -> HoverCard {
    HoverCard {
        word: word.to_string(),
        buf_key: 0,
        offset: 0,
        px: 0.0,
        py: 0.0,
        since: std::time::Instant::now(),
        lines: Vec::new(),
    }
}

// ── C function registration ─────────────────────────────────────────────

#[test]
fn hover_attach_c_function_registered() {
    janet_test!(ed, {
        ed.hover = Some(card_with_word("foo"));
        let result = scripting::eval("(hover/attach \"foo\" \"code\" \"https://example.com\")");
        assert_eq!(result, "ok", "hover/attach should be registered");
        let lines = &ed.hover.as_ref().unwrap().lines;
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], HoverLine {
            kind: "code".into(),
            text: "https://example.com".into(),
        });
    });
}

// ── Observable card state after Janet runs ──────────────────────────────

#[test]
fn hover_attach_requires_matching_word() {
    janet_test!(ed, {
        ed.hover = Some(card_with_word("foo"));
        let result = scripting::eval("(hover/attach \"bar\" \"code\" \"x\")");
        assert_eq!(result, "ok");
        assert!(
            ed.hover.as_ref().unwrap().lines.is_empty(),
            "a mismatched word must not attach"
        );
    });
}

#[test]
fn hover_attach_without_card_is_a_noop() {
    janet_test!(ed, {
        assert!(ed.hover.is_none());
        let result = scripting::eval_result("(hover/attach \"bar\" \"code\" \"x\")");
        assert_eq!(result.unwrap(), "false", "no card on screen → false");
        assert!(ed.hover.is_none());
    });
}

#[test]
fn hover_attach_appends_in_order() {
    janet_test!(ed, {
        ed.hover = Some(card_with_word("dup"));
        scripting::eval("(hover/attach \"dup\" \"muted\" \"first\")");
        scripting::eval("(hover/attach \"dup\" \"text\" \"second\")");
        let lines = &ed.hover.as_ref().unwrap().lines;
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text, "first");
        assert_eq!(lines[1].text, "second");
        assert_eq!(lines[1].kind, "text");
    });
}

// ── Markdown plugin hover augmentation, end to end ──────────────────────

#[test]
fn markdown_hover_handler_attaches_link_and_heading() {
    janet_test!(ed, {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../plugins/markdown/hover.janet");
        let src = std::fs::read_to_string(path).expect("read markdown hover.janet");
        let result = scripting::eval(&src);
        assert_eq!(result, "ok", "markdown hover.janet should evaluate");

        // Link label under the pointer → the target attaches as a code line.
        ed.hover = Some(card_with_word("spec"));
        ed.events.emit_typed(
            kernel::event::keys::events::HOVER_CARD,
            kernel::event::payload::HoverCardPayload {
                word: "spec".into(),
                line_text: "read the [spec](https://example.io/x) first".into(),
                line: "3".into(),
                path: "notes.md".into(),
            },
        );
        ed.dispatch_events_scoped();
        let lines = &ed.hover.as_ref().unwrap().lines;
        assert!(
            lines.iter().any(|l| l.kind == "code" && l.text.contains("https://example.io/x")),
            "link target line attached, got {lines:?}"
        );

        // Hovered line is a heading → the heading text attaches.
        ed.hover = Some(card_with_word("Setup"));
        ed.events.emit_typed(
            kernel::event::keys::events::HOVER_CARD,
            kernel::event::payload::HoverCardPayload {
                word: "Setup".into(),
                line_text: "## Setup instructions".into(),
                line: "0".into(),
                path: "notes.md".into(),
            },
        );
        ed.dispatch_events_scoped();
        let lines = &ed.hover.as_ref().unwrap().lines;
        assert!(
            lines.iter().any(|l| l.kind == "heading" && l.text.contains("Setup instructions")),
            "heading line attached, got {lines:?}"
        );

        // A plain prose line attaches nothing.
        ed.hover = Some(card_with_word("word"));
        ed.events.emit_typed(
            kernel::event::keys::events::HOVER_CARD,
            kernel::event::payload::HoverCardPayload {
                word: "word".into(),
                line_text: "just some prose with word in it".into(),
                line: "7".into(),
                path: "notes.md".into(),
            },
        );
        ed.dispatch_events_scoped();
        assert!(
            ed.hover.as_ref().unwrap().lines.is_empty(),
            "plain prose must not attach markdown context"
        );
    });
}
