#[macro_use]
mod helpers;
use scripting;

// Picker sources: plugins register search surfaces; the query event
// collects entries through (picker/push-items) and they land in the
// kernel cache the frontend projects.
#[test]
fn picker_source_collects_entries_for_the_query() {
    janet_test!(ed, {
        scripting::eval(r#"(picker/register-source "zz-src" "ZZ Picker" "type")"#);
        scripting::eval(
            r#"(event/on "picker-source-query"
                 (fn [ev]
                   (when (= "zz-src" (get ev :source))
                     (def q (string/ascii-lower (string/trim (or (get ev :query) ""))))
                     (def out @[])
                     (each name ["Alpha" "Beta" "Alpine"]
                       (when (or (= 0 (length q)) (string/has-prefix? q (string/ascii-lower name)))
                         (array/push out {:title name :detail "fn" :meta "f:1"
                                          :payload (string "/proj/f.rs" "\x1f" 3)})))
                     (picker/push-items out))))"#,
        );

        // The kernel command refuses unknown ids and queues known ones.
        assert!(
            scripting::eval_result(r#"(editor/run-command "gui-picker" "no-such-source")"#)
                .is_err(),
            "unknown source is a command error"
        );
        // Headless: there is no shell to open the picker, so the query
        // event is what the collector answers to.
        scripting::eval_result(
            r#"(do (event/emit "picker-source-query" {"source" "zz-src" "query" ""}) true)"#,
        )
        .unwrap();
        ed.events.drain_and_dispatch();

        assert_eq!(ed.picker_state.items.len(), 3, "empty query lists everything");
        assert_eq!(ed.picker_state.items[0].title, "Alpha");
        assert_eq!(
            ed.picker_state.items[0].payload,
            format!("/proj/f.rs\u{1f}3"),
            "path payloads open the file at the line"
        );

        scripting::eval_result(
            r#"(do (event/emit "picker-source-query" {"source" "zz-src" "query" "al"}) true)"#,
        )
        .unwrap();
        ed.events.drain_and_dispatch();
        let titles: Vec<&str> =
            ed.picker_state.items.iter().map(|i| i.title.as_str()).collect();
        assert_eq!(titles, vec!["Alpha", "Alpine"], "the query filters");
    });
}

// The built-in sources ship with the editor: symbols in file and tags.
#[test]
fn builtin_sources_are_registered_at_startup() {
    janet_test!(_ed, {
        let ids = scripting::eval_result(
            r#"(string/join (map |(get $ :id) (picker/sources)) ",")"#,
        )
        .unwrap_or_default();
        assert!(ids.contains("document-symbols"), "file symbols source: {ids}");
        assert!(ids.contains("tags"), "tags source: {ids}");
    });
}

// Unloading the registering plugin removes its sources.
#[test]
fn plugin_unload_removes_its_picker_sources() {
    janet_test!(ed, {
        let root = std::env::temp_dir().join(format!("magma_psrc_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let dir = root.join("zz-psrc");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("main.janet"),
            r#"(picker/register-source "zz-plugin-src" "Z" "z")"#,
        )
        .unwrap();
        scripting::eval(&format!(
            r#"(plugin/register "zz-psrc" "1.0" "{}" "main.janet" @[] @[] @[:editor-access])"#,
            dir.to_str().unwrap().replace('\\', "/")
        ));
        scripting::eval_result(r#"(plugin/enable "zz-psrc")"#).unwrap();
        assert!(ed.picker_sources.get("zz-plugin-src").is_some());
        scripting::eval_result(r#"(plugin/unload "zz-psrc")"#).unwrap();
        assert!(
            ed.picker_sources.get("zz-plugin-src").is_none(),
            "teardown removes the source"
        );
        std::fs::remove_dir_all(&root).ok();
    });
}

// Status-bar segments: plugins own a pill by id, empty text removes it,
// and the LSP attach layer posts server state on the boot it reports.
#[test]
fn status_segments_upsert_and_remove_by_id() {
    janet_test!(ed, {
        scripting::eval(r#"(status/set-segment "zz-a" "one" "info")"#);
        scripting::eval(r#"(status/set-segment "zz-b" "two" "warn")"#);
        assert_eq!(ed.status_segments.len(), 2);
        scripting::eval(r#"(status/set-segment "zz-a" "uno" "error")"#);
        assert_eq!(ed.status_segments.len(), 2, "update, not append");
        assert_eq!(ed.status_segments[0].text, "uno");
        assert_eq!(ed.status_segments[0].kind, "error");
        scripting::eval(r#"(status/set-segment "zz-a" "")"#);
        assert_eq!(ed.status_segments.len(), 1, "empty text removes the pill");
    });
}
