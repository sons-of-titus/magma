#[macro_use]
mod helpers;
use scripting;

// ── registration and expansion ───────────────────────────────────────────────

#[test]
fn registered_snippet_expands_in_its_language() {
    janet_test!(ed, {
        assert_eq!(
            scripting::eval(
                r#"(snippet/register "zzfn" "fn ${1:name}() {\n    ${0}\n}" "zzlang" "test fn")"#
            ),
            "ok"
        );
        // Put the buffer in the language's mode so resolution sees "zzlang".
        scripting::eval(r#"(major-mode/define "zzlang-mode" {:parent "prog"})"#);
        let buf = ed.create_buffer_from_str("a.zzlang", "zzfn");
        if let Some(win) = ed.view_tree.focused_window_mut() {
            win.buffer_id = Some(buf);
        }
        scripting::eval(r#"(put *extension-mode-map* "zzlang" "zzlang-mode")"#);
        scripting::eval_result(&format!(
            r#"(do (event/emit "buffer-created" {{"buffer-id" "{buf}" "path" "/proj/a.zzlang"}}) true)"#
        ))
        .unwrap();
        ed.dispatch_events_scoped();

        // Cursor sits after the trigger word (the command expands what is
        // behind it), exactly as typing does.
        if let Some(view) = ed.views.get_mut(&buf) {
            view.set_cursor(4);
        }
        scripting::eval_result(r#"(editor/run-command "snippet-expand")"#).unwrap();
        let text = ed
            .buffers
            .get(buf)
            .unwrap()
            .lock()
            .unwrap()
            .slice(0, 30);
        assert!(
            text.starts_with("fn "),
            "the trigger expands into the body: {text:?}"
        );
        assert!(
            !text.contains("${"),
            "tab-stop markers never land in the buffer: {text:?}"
        );
        assert!(
            ed.snippet.active,
            "expansion arms the tab-stop cycle"
        );
    });
}

#[test]
fn snippets_are_language_scoped() {
    janet_test!(ed, {
        scripting::eval(r#"(snippet/register "zzboth" "scoped" "zz-scoped" nil)"#);
        scripting::eval(r#"(snippet/register "zzboth" "global" nil nil)"#);
        // Scoped wins where it applies; other languages fall back to global.
        let scoped = scripting::eval_result(
            r#"(get (get (snippet/list "zz-scoped") 0) :body)"#,
        )
        .unwrap_or_default();
        assert_eq!(scoped, r#""scoped""#, "language-scoped entry listed");
        let global_body = scripting::eval_result(
            r#"(get (get (snippet/list "zz-other") 0) :body)"#,
        )
        .unwrap_or_default();
        assert_eq!(
            global_body, r#""global""#,
            "other languages see the global entry"
        );
    });
}

#[test]
fn snippet_remove_drops_the_registration() {
    janet_test!(_ed, {
        scripting::eval(r#"(snippet/register "zzgone" "body" "zz-gone" nil)"#);
        let listed = scripting::eval_result(r#"(length (snippet/list "zz-gone"))"#)
            .unwrap_or_default();
        assert_eq!(listed, "1");
        assert_eq!(
            scripting::eval_result(r#"(snippet/remove "zzgone" "zz-gone")"#).unwrap_or_default(),
            ":ok"
        );
        let listed = scripting::eval_result(r#"(length (snippet/list "zz-gone"))"#)
            .unwrap_or_default();
        assert_eq!(listed, "0");
    });
}

// ── plugin lifecycle ─────────────────────────────────────────────────────────

// plugin/define loads a sibling snippets.janet; unloading the plugin's
// language removes its snippets with it.
#[test]
fn plugin_snippets_load_with_the_manifest_and_die_with_the_plugin() {
    janet_test!(ed, {
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        scripting::eval(&format!(
            r#"(do (plugin/discover "{}") (plugin/enable "rust"))"#,
            repo.join("plugins").to_str().unwrap().replace('\\', "/")
        ));
        let rust_snippets =
            scripting::eval_result(r#"(length (snippet/list "rust"))"#).unwrap_or_default();
        assert!(
            rust_snippets != "0" && rust_snippets != "nil",
            "the rust pack loads through the manifest: {rust_snippets}"
        );

        scripting::eval_result(r#"(plugin/unload "rust")"#).unwrap();
        ed.dispatch_events_scoped();
        let after = scripting::eval_result(r#"(length (snippet/list "rust"))"#)
            .unwrap_or_default();
        assert_eq!(after, "0", "unloading the plugin removes its snippets");
    });
}
