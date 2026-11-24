#[macro_use]
mod helpers;
use scripting;

// `tr` reads stdin and writes stdout, so it doubles as a deterministic
// fake formatter: [:lower:] → [:upper:] visibly rewrites the text.
const UPPER: &str = r#"(formatter/register "zzf" {:command "tr" :args @["[:lower:]" "[:upper:]"]})"#;

#[test]
fn registered_formatter_runs_over_text() {
    janet_test!(_ed, {
        assert_eq!(scripting::eval(UPPER), "ok");
        let got =
            scripting::eval_result(r#"(formatter/format "zzf" "abc def")"#).unwrap_or_default();
        assert_eq!(got, r#""ABC DEF""#);
    });
}

#[test]
fn failing_formatter_keeps_text_and_reports_once() {
    janet_test!(ed, {
        // /usr/bin/false exits non-zero without producing stdout.
        assert_eq!(
            scripting::eval(r#"(formatter/register "zzfail" {:command "false" :args @[]})"#),
            "ok"
        );
        let got = scripting::eval_result(r#"(formatter/format "zzfail" "keep me")"#)
            .unwrap_or_default();
        assert_eq!(got, r#""keep me""#, "a failed format leaves the text alone");
        let detail = ed
            .status_message
            .as_ref()
            .map(|m| m.text.clone())
            .unwrap_or_default();
        assert!(
            detail.contains("failed"),
            "the failure must be user-visible: {detail}"
        );
    });
}

#[test]
fn missing_formatter_binary_is_reported_not_silent() {
    janet_test!(ed, {
        assert_eq!(
            scripting::eval(
                r#"(formatter/register "zznone" {:command "zz-no-such-formatter" :args @[]})"#
            ),
            "ok"
        );
        let got = scripting::eval_result(r#"(formatter/format "zznone" "untouched")"#)
            .unwrap_or_default();
        assert_eq!(got, r#""untouched""#);
        let detail = ed
            .status_message
            .as_ref()
            .map(|m| m.text.clone())
            .unwrap_or_default();
        assert!(
            detail.contains("not found") && detail.contains("zz-no-such-formatter"),
            "missing formatter names the binary: {detail}"
        );
    });
}

#[test]
fn registration_defines_the_format_command() {
    janet_test!(ed, {
        assert_eq!(scripting::eval(UPPER), "ok");
        assert!(
            ed.commands.exists("zzf-format-buffer"),
            "formatter/register defines <lang>-format-buffer"
        );
    });
}

#[test]
fn format_on_save_respects_the_option() {
    janet_test!(ed, {
        assert_eq!(scripting::eval(UPPER), "ok");
        assert_eq!(
            scripting::eval(r#"(major-mode/define "zzf-mode" {:parent "prog"})"#),
            "ok"
        );
        let buf = ed.create_buffer_from_str("a.zzf", "abc\n");
        if let Some(win) = ed.view_tree.focused_window_mut() {
            win.buffer_id = Some(buf);
        }
        // Mode lands through the same path a plugin language uses.
        scripting::eval(r#"(put *extension-mode-map* "zzf" "zzf-mode")"#);
        scripting::eval_result(&format!(
            r#"(do (event/emit "buffer-created" {{"buffer-id" "{buf}" "path" "/proj/a.zzf"}}) true)"#
        ))
        .unwrap();
        ed.dispatch_events_scoped();

        // Option unset: the save hook must not rewrite the buffer.
        scripting::eval_result(&format!(
            r#"(do (event/emit "buffer-before-save" {{"buffer-id" "{buf}" "path" "/proj/a.zzf"}}) true)"#
        ))
        .unwrap();
        ed.dispatch_events_scoped();
        let text = ed
            .buffers
            .get(buf)
            .unwrap()
            .lock()
            .unwrap()
            .slice(0, 3);
        assert_eq!(text, "abc", "no format-on-save option, no rewrite");

        // Option set: the same event now formats.
        scripting::eval(r#"(option/set "zzf-format-on-save" "true")"#);
        scripting::eval_result(&format!(
            r#"(do (event/emit "buffer-before-save" {{"buffer-id" "{buf}" "path" "/proj/a.zzf"}}) true)"#
        ))
        .unwrap();
        ed.dispatch_events_scoped();
        let text = ed
            .buffers
            .get(buf)
            .unwrap()
            .lock()
            .unwrap()
            .slice(0, 3);
        assert_eq!(text, "ABC", "format-on-save rewrote the buffer");
    });
}

// The save command must snapshot buffer content AFTER the before-save
// hooks run — the old order captured first and silently wrote pre-format
// text to disk while the buffer showed the formatted version.
#[test]
fn before_save_mutations_are_what_gets_written() {
    janet_test!(ed, {
        let tmp = std::env::temp_dir().join(format!(
            "magma_fmt_save_{}.txt",
            std::process::id()
        ));
        let path = tmp.to_str().unwrap().replace('\\', "/");
        let key = ed.view_tree
            .focused_window()
            .and_then(|wid| ed.view_tree.buffer(wid))
            .unwrap();
        ed.buffers.get(key).unwrap().lock().unwrap().path = Some(path.clone());
        scripting::eval(
            r#"(event/on "buffer-before-save"
                 (fn [_]
                   (def b (buffer/current))
                   (buffer/insert b (buffer/len b) "!")
                   nil))"#,
        );
        scripting::eval_result(r#"(editor/run-command "save-buffer")"#).unwrap();
        let written = std::fs::read_to_string(&tmp).unwrap_or_default();
        // The helper editor's buffer is empty; the hook appends "!" and the
        // write must contain the hook's output, not the pre-hook snapshot.
        assert_eq!(written, "!", "the hook's mutation must be what gets written");
        std::fs::remove_file(&tmp).ok();
    });
}
