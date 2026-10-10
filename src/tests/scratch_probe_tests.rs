#![cfg(feature = "janet")]
use crate::kernel::scripting;

#[test]
fn probe_syntax_event_path() {
    janet_test!(ed, {
        let key = ed.create_buffer_from_str("main.rs", "fn main() { let x = true; }");
        if let Some(arc) = ed.buffers.get(key) {
            arc.lock().unwrap().path = Some("/tmp/probe/main.rs".to_string());
        }
        for expr in [
            format!(r#"(string (file-extension (buffer/path {key})))"#),
            format!(r#"(string (length (get *syntax-rules* "rust")))"#),
        ] {
            let r = scripting::eval_result(&expr);
            println!("EXPR: {expr}\n  -> {:?}", r);
        }
        // Fire the real event, exactly as buffer-created emission does.
        ed.events.emit("buffer-created", {
            let mut m = std::collections::HashMap::new();
            m.insert("buffer-id".to_string(), key.to_string());
            m.insert("revision-before".to_string(), "0".to_string());
            m.insert("revision-after".to_string(), "0".to_string());
            m.insert("edits".to_string(), String::new());
            m
        });
        ed.dispatch_events_scoped();
        let view = ed.view_of_buffer(key).expect("view");
        println!("LAYERS: {:?}", view.highlight_layers.keys().collect::<Vec<_>>());
        println!("SYNTAX RANGES: {:?}", view.highlight_layers.get("syntax").map(|v| v.len()));
    });
}
