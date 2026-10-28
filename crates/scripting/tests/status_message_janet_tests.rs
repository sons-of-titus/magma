#[macro_use]
mod helpers;
// editor/echo — Janet-side transient status + durable log.

use kernel::state::MessageKind;
use scripting;

#[test]
fn echo_defaults_to_info_and_logs() {
    janet_test!(ed, {
        let r = scripting::eval("(editor/echo \"indexing done\")");
        assert_eq!(r, "ok");
        let msg = ed.status_message.expect("echo must surface a message");
        assert_eq!(msg.kind, MessageKind::Info);
        assert_eq!(msg.text, "indexing done");

        let messages = ed
            .buffers
            .iter()
            .find(|(_, arc)| arc.lock().unwrap().name == "*Messages*")
            .map(|(_, arc)| {
                let buf = arc.lock().unwrap();
                buf.slice(0, buf.len())
            })
            .unwrap_or_default();
        assert!(messages.contains("indexing done"), "got: {messages}");
    });
}

#[test]
fn echo_error_kind_is_distinct() {
    janet_test!(ed, {
        let r = scripting::eval("(editor/echo \"Unknown command: frob\" \"error\")");
        assert_eq!(r, "ok");
        let msg = ed.status_message.expect("echo must surface a message");
        assert_eq!(msg.kind, MessageKind::Error);
        assert_eq!(msg.text, "Unknown command: frob");
    });
}
