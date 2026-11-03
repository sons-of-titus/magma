//! Completion popup — trigger, refilter, navigation, accept.

use crate::kernel::input;
use crate::kernel::state::Editor;
use crate::kernel::state::completion::CompletionItem;
use crate::kernel::storage::disk::DiskFileSystem;
use crate::tests::helpers::make_editor;

fn editor_with(text: &str) -> Editor {
    let mut ed = make_editor();
    let key = ed.create_buffer_from_str("words.txt", text);
    if let Some(win) = ed.view_tree.focused_window_mut() {
        win.buffer_id = Some(key);
    }
    // Type at the end, like a user at the bottom of a file.
    ed.view_of_buffer_mut(key).unwrap().set_cursor(text.len());
    ed
}

fn type_key(ed: &mut Editor, key: &str) {
    input::dispatch_key(ed, key);
}

#[test]
fn typing_two_word_chars_opens_the_popup() {
    let mut ed = editor_with("done door dormant\n");
    type_key(&mut ed, "d");
    assert!(
        !ed.completion.visible,
        "one char is too short for the auto popup"
    );

    type_key(&mut ed, "o");
    assert!(ed.completion.visible, "two word chars must open the popup");
    assert_eq!(ed.completion.prefix, "do");
    assert_eq!(
        ed.completion
            .items
            .iter()
            .map(|i| i.label.clone())
            .collect::<Vec<_>>(),
        vec!["done", "door", "dormant"]
    );
}

#[test]
fn typing_refilters_the_popup_in_place() {
    let mut ed = editor_with("done door dormant\n");
    type_key(&mut ed, "d");
    type_key(&mut ed, "o");
    type_key(&mut ed, "n");
    assert!(ed.completion.visible);
    assert_eq!(
        ed.completion
            .items
            .iter()
            .map(|i| i.label.clone())
            .collect::<Vec<_>>(),
        vec!["done"]
    );
}

#[test]
fn popup_dismisses_when_nothing_matches() {
    let mut ed = editor_with("done door\n");
    type_key(&mut ed, "d");
    type_key(&mut ed, "o");
    assert!(ed.completion.visible);
    type_key(&mut ed, "z");
    assert!(!ed.completion.visible, "no candidates must close the popup");
}

#[test]
fn non_word_input_dismisses_and_inserts() {
    let mut ed = editor_with("done door\n");
    type_key(&mut ed, "d");
    type_key(&mut ed, "o");
    assert!(ed.completion.visible);

    type_key(&mut ed, "(");
    assert!(!ed.completion.visible);
    let key = ed
        .view_tree
        .focused_window()
        .and_then(|w| ed.view_tree.buffer(w))
        .unwrap();
    let text = ed.buffers.get(key).unwrap().lock().unwrap().slice(0, 20);
    assert_eq!(text, "done door\ndo(", "the paren must still insert");
}

#[test]
fn popup_navigation_and_accept() {
    let mut ed = editor_with("dog done\n");
    type_key(&mut ed, "d");
    type_key(&mut ed, "o");
    assert_eq!(
        ed.completion
            .items
            .iter()
            .map(|i| i.label.clone())
            .collect::<Vec<_>>(),
        vec!["dog", "done"]
    );
    assert_eq!(ed.completion.idx, 0);

    type_key(&mut ed, "down");
    assert_eq!(ed.completion.idx, 1, "down moves the selection");
    type_key(&mut ed, "up");
    assert_eq!(ed.completion.idx, 0, "up moves back");

    type_key(&mut ed, "tab");
    assert!(!ed.completion.visible, "accept closes the popup");
    let key = ed
        .view_tree
        .focused_window()
        .and_then(|w| ed.view_tree.buffer(w))
        .unwrap();
    let text = ed.buffers.get(key).unwrap().lock().unwrap().slice(0, 20);
    assert!(
        text.starts_with("dog done\ndog"),
        "accept inserts the remaining suffix, got: {text:?}"
    );
}

#[test]
fn escape_dismisses_without_inserting() {
    let mut ed = editor_with("done door\n");
    type_key(&mut ed, "d");
    type_key(&mut ed, "o");
    assert!(ed.completion.visible);

    type_key(&mut ed, "esc");
    assert!(!ed.completion.visible);
    let key = ed
        .view_tree
        .focused_window()
        .and_then(|w| ed.view_tree.buffer(w))
        .unwrap();
    let text = ed.buffers.get(key).unwrap().lock().unwrap().slice(0, 20);
    assert_eq!(text, "done door\ndo", "esc must not alter the text");
}

#[test]
fn ctrl_space_triggers_at_any_prefix_length() {
    let mut ed = editor_with("done door\n");
    // Move cursor between 'd' and 'o' in "done" is awkward; instead put the
    // cursor after the single char 'd' of "door" via insert.
    type_key(&mut ed, "d");
    assert!(!ed.completion.visible);

    // ctrl-space routes here through the keymap (janet-loaded); the
    // command itself is the contract the key resolves to.
    let r = crate::kernel::command::execute_command(
        &mut ed,
        "completion-trigger",
        &std::collections::HashMap::new(),
    );
    r.unwrap();
    assert!(
        ed.completion.visible,
        "completion-trigger must open the popup"
    );
    assert_eq!(ed.completion.prefix, "d");
    assert_eq!(
        ed.completion
            .items
            .iter()
            .map(|i| i.label.clone())
            .collect::<Vec<_>>(),
        vec!["done", "door"]
    );
}

#[test]
fn word_prefix_len_measures_the_current_word() {
    let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
    let key = ed.create_buffer_from_str("t", "hello world\n");
    if let Some(win) = ed.view_tree.focused_window_mut() {
        win.buffer_id = Some(key);
    }
    ed.view_of_buffer_mut(key).unwrap().set_cursor(8); // after "hello_wo".len().."hello wo"
    assert_eq!(crate::kernel::completion_words::word_prefix_len(&ed), 2);
    ed.view_of_buffer_mut(key).unwrap().set_cursor(6); // after the space
    assert_eq!(crate::kernel::completion_words::word_prefix_len(&ed), 0);
}

// ── Revision-keyed word cache (rework: no per-keystroke document copy) ──

#[test]
fn word_cache_hits_while_the_revision_is_current_and_rebuilds_after_an_edit() {
    let mut ed = editor_with("alpha alphabet\n");
    let key = crate::tests::helpers::focused_key(&ed);

    type_key(&mut ed, "a");
    type_key(&mut ed, "l");
    assert!(ed.completion.visible);
    let cached = ed
        .completion
        .word_cache
        .get(&key)
        .expect("inline scan caches the word set");
    assert_eq!(
        cached.revision,
        ed.buffers.get(key).unwrap().lock().unwrap().revision()
    );
    assert!(cached.words.contains("alphabet"));

    // An edit moves the revision; the next trigger must rebuild, not serve
    // the stale set.
    let stale = cached.revision;
    ed.view_of_buffer_mut(key).unwrap().insert(0, "zeta ");
    type_key(&mut ed, "p");
    type_key(&mut ed, "h");
    let cached = ed
        .completion
        .word_cache
        .get(&key)
        .expect("re-scan after edit");
    assert_ne!(
        cached.revision, stale,
        "the cache must track the new revision"
    );
    assert!(
        cached.words.contains("zeta"),
        "new text participates in candidates"
    );
}

#[test]
fn stale_background_word_results_are_discarded() {
    let mut ed = editor_with("word one\n");
    let key = crate::tests::helpers::focused_key(&ed);
    let stale_rev = ed.buffers.get(key).unwrap().lock().unwrap().revision();

    // The buffer moves on while the scan is in flight.
    ed.view_of_buffer_mut(key).unwrap().insert(0, "new ");
    assert_ne!(
        ed.buffers.get(key).unwrap().lock().unwrap().revision(),
        stale_rev
    );

    // The scan result for the obsolete revision must be dropped.
    crate::kernel::runtime::process_background_event(
        &mut ed,
        crate::kernel::runtime::BackgroundEvent::BufferWords {
            buf_key: key,
            revision: stale_rev,
            words: vec!["stale".into()],
        },
    );
    assert!(
        !ed.completion.word_cache.contains_key(&key),
        "a result for an obsolete revision must never land"
    );

    // A result matching the current revision lands.
    let rev = ed.buffers.get(key).unwrap().lock().unwrap().revision();
    crate::kernel::runtime::process_background_event(
        &mut ed,
        crate::kernel::runtime::BackgroundEvent::BufferWords {
            buf_key: key,
            revision: rev,
            words: vec!["fresh".into()],
        },
    );
    assert!(ed.completion.word_cache.contains_key(&key));
}

// ── LSP-driven completion ────────────────────────────────────────────

use crate::kernel::completion::{accept, dismiss};
use crate::kernel::completion_lsp::{merge_lsp_items, parse_lsp_completion_items};

#[test]
fn lsp_items_parse_from_array_and_list() {
    let arr = parse_lsp_completion_items(
        r#"[{"label":"dog","kind":3,"insertText":"doggo","detail":"fn(dog)"}]"#,
    );
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0].label, "dog");
    assert_eq!(arr[0].insert, "doggo");
    assert_eq!(arr[0].kind, "fn");
    assert_eq!(arr[0].detail, "fn(dog)");

    let list =
        parse_lsp_completion_items(r#"{"isIncomplete":false,"items":[{"label":"door","kind":6}]}"#);
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].insert, "door", "no insertText: insert = label");
    assert_eq!(list[0].kind, "var");

    assert!(parse_lsp_completion_items("not json").is_empty());
}

#[test]
fn lsp_items_lead_the_popup_and_drop_duplicates() {
    let mut ed = editor_with("done\n");
    input::dispatch_key(&mut ed, "d");
    input::dispatch_key(&mut ed, "o");
    assert!(ed.completion.visible);

    merge_lsp_items(
        &mut ed,
        vec![
            CompletionItem {
                label: "dog".into(),
                insert: "dog".into(),
                kind: "fn".into(),
                ..Default::default()
            },
            CompletionItem {
                label: "done".into(),
                insert: "done".into(),
                kind: "var".into(),
                ..Default::default()
            },
        ],
    );
    let labels: Vec<&str> = ed
        .completion
        .items
        .iter()
        .map(|i| i.label.as_str())
        .collect();
    assert_eq!(
        labels.first(),
        Some(&"dog"),
        "server items keep their order and lead"
    );
    assert_eq!(
        labels.iter().filter(|l| **l == "done").count(),
        1,
        "word duplicate dropped"
    );
    // The user's highlighted word ("done") survives the merge, so the
    // selection follows it instead of being yanked back to the top.
    assert_eq!(ed.completion.idx, 1);
    dismiss(&mut ed);
}

#[test]
fn merge_rejects_items_that_no_longer_match_the_prefix() {
    let mut ed = editor_with("done\n");
    input::dispatch_key(&mut ed, "d");
    input::dispatch_key(&mut ed, "o");
    merge_lsp_items(
        &mut ed,
        vec![CompletionItem {
            label: "zebra".into(),
            ..Default::default()
        }],
    );
    assert!(
        ed.completion
            .items
            .iter()
            .all(|i| i.label.starts_with("do")),
        "stale-prefix items are filtered on merge"
    );
    dismiss(&mut ed);
}

#[test]
fn accept_types_the_server_insert_text() {
    let mut ed = editor_with("done\n");
    input::dispatch_key(&mut ed, "d");
    input::dispatch_key(&mut ed, "o");
    merge_lsp_items(
        &mut ed,
        vec![CompletionItem {
            label: "document".into(),
            insert: "document_changes".into(),
            kind: "fn".into(),
            ..Default::default()
        }],
    );
    ed.completion.idx = 0;
    accept(&mut ed);
    assert_eq!(
        ed.buffers
            .get(crate::tests::helpers::focused_key(&ed))
            .unwrap()
            .lock()
            .unwrap()
            .slice(0, 32),
        "done\ndocument_changes",
        "accept inserts insertText, not the label"
    );
}

#[test]
fn accept_replaces_prefix_when_insert_does_not_extend_it() {
    let mut ed = editor_with("done\n");
    input::dispatch_key(&mut ed, "d");
    input::dispatch_key(&mut ed, "o");
    merge_lsp_items(
        &mut ed,
        vec![CompletionItem {
            label: "do_thing".into(),
            insert: "do_thing()".into(),
            kind: "snip".into(),
            ..Default::default()
        }],
    );
    ed.completion.idx = 0;
    accept(&mut ed);
    assert_eq!(
        ed.buffers
            .get(crate::tests::helpers::focused_key(&ed))
            .unwrap()
            .lock()
            .unwrap()
            .slice(0, 32),
        "done\ndo_thing()",
        "an insert that does not extend the prefix replaces it wholesale"
    );
}

// ── Rich LSP payload: kinds, signatures, snippets, text edits ────────

const RUST_ANALYZER_PAYLOAD: &str = r#"[
    {
        "label": "push",
        "kind": 2,
        "detail": "fn push(&mut self, value: T)",
        "documentation": { "kind": "markdown", "value": "Appends a value." },
        "filterText": "push",
        "insertTextFormat": 2,
        "textEdit": {
            "range": { "start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 2} },
            "newText": "push(${1:value})"
        }
    },
    {
        "label": "Vec",
        "kind": 7,
        "labelDetails": { "description": "alloc::vec" }
    }
]"#;

#[test]
fn lsp_items_carry_kind_signature_docs_and_snippet_format() {
    let items = parse_lsp_completion_items(RUST_ANALYZER_PAYLOAD);
    assert_eq!(items.len(), 2);

    let push = &items[0];
    assert_eq!(push.label, "push");
    assert_eq!(push.kind, "method");
    assert_eq!(push.detail, "fn push(&mut self, value: T)");
    assert_eq!(push.docs, "Appends a value.");
    assert!(push.is_snippet, "insertTextFormat 2 marks a snippet");
    assert_eq!(push.insert, "push(${1:value})", "textEdit.newText wins");
    assert_eq!(push.filter, "push");

    let vec_item = &items[1];
    assert_eq!(vec_item.kind, "class");
    assert_eq!(
        vec_item.detail, "alloc::vec",
        "labelDetails.description backs an empty detail"
    );
    assert!(!vec_item.is_snippet);
    assert_eq!(vec_item.insert, "Vec");
}

#[test]
fn server_items_keep_their_metadata_and_word_duplicates_drop() {
    let mut ed = editor_with("done door\n");
    input::dispatch_key(&mut ed, "d");
    input::dispatch_key(&mut ed, "o");
    assert!(ed.completion.visible);

    merge_lsp_items(
        &mut ed,
        vec![CompletionItem {
            kind: "field".into(),
            detail: "door: Doorway".into(),
            ..CompletionItem::word("door")
        }],
    );
    let door = ed
        .completion
        .items
        .iter()
        .find(|i| i.label == "door")
        .expect("door survives the merge");
    assert_eq!(
        door.kind, "field",
        "the server copy wins with its kind attached"
    );
    assert_eq!(door.detail, "door: Doorway");
    assert_eq!(
        ed.completion
            .items
            .iter()
            .filter(|i| i.label == "door")
            .count(),
        1,
        "the bare word duplicate drops"
    );
    dismiss(&mut ed);
}

#[test]
fn accepting_a_snippet_completion_expands_tabstops() {
    let mut ed = editor_with("pu");
    ed.completion.visible = true;
    ed.completion.prefix = "pu".into();
    ed.completion.items = vec![CompletionItem {
        is_snippet: true,
        insert: "push(${1:value})$0".into(),
        ..CompletionItem::word("push")
    }];
    ed.completion.idx = 0;

    accept(&mut ed);
    let key = crate::tests::helpers::focused_key(&ed);
    let text = ed.buffers.get(key).unwrap().lock().unwrap().slice(0, 40);
    assert_eq!(text, "push(value)", "no $ marker survives accept");
    assert!(ed.snippet.active, "tab-stop cycling is live");
    assert_eq!(ed.snippet.tabstops.len(), 2);
    assert_eq!(
        ed.view_of_buffer(key).unwrap().cursor_offset(),
        "push(".len(),
        "the cursor parks on the first tab stop"
    );
    let stop = &ed.snippet.tabstops[0];
    assert_eq!(
        &text[stop.offset..stop.offset + stop.text_len],
        "value",
        "tab-stop offsets point into the buffer"
    );
}

#[test]
fn accepting_a_textedit_completion_replaces_the_prefix_wholesale() {
    let mut ed = editor_with("pos");
    ed.completion.visible = true;
    ed.completion.prefix = "pos".into();
    ed.completion.items = vec![CompletionItem {
        kind: "field".into(),
        detail: "player.position: Vec2".into(),
        insert: "player.position".into(),
        ..CompletionItem::word("position")
    }];
    ed.completion.idx = 0;

    accept(&mut ed);
    let key = crate::tests::helpers::focused_key(&ed);
    let text = ed.buffers.get(key).unwrap().lock().unwrap().slice(0, 40);
    assert_eq!(text, "player.position");
    assert_eq!(
        ed.view_of_buffer(key).unwrap().cursor_offset(),
        "player.position".len(),
        "the cursor lands after the replaced text"
    );
}

#[test]
fn merge_filters_on_filter_text_case_insensitively() {
    let mut ed = editor_with("done\n");
    input::dispatch_key(&mut ed, "d");
    input::dispatch_key(&mut ed, "o");
    merge_lsp_items(
        &mut ed,
        vec![
            CompletionItem {
                label: "Doorway".into(),
                filter: "doorway".into(),
                ..Default::default()
            },
            CompletionItem {
                label: "zebra".into(),
                ..Default::default()
            },
        ],
    );
    let labels: Vec<&str> = ed
        .completion
        .items
        .iter()
        .map(|i| i.label.as_str())
        .collect();
    assert!(
        labels.contains(&"Doorway"),
        "case differences do not drop items"
    );
    assert!(!labels.contains(&"zebra"), "unrelated items are filtered");
    dismiss(&mut ed);
}

#[test]
fn sql_console_completion_suggests_alias_columns() {
    let mut ed = make_editor();
    // A connected SQLite source with one table.
    let dir = std::env::temp_dir().join(format!(
        "magma-compl-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("c.sqlite");
    let conn = ::db::connect(
        &::db::DataSource {
            id: 0,
            name: "c".into(),
            kind: db::DriverKind::Sqlite,
            host: path.to_string_lossy().to_string(),
            port: 0,
            database: String::new(),
            user: String::new(),
            ssl_mode: Default::default(),
            trust_cert: false,
            save_password: false,
            url: None,
        },
        None,
    )
    .unwrap();
    conn.execute_script(
        "CREATE TABLE users (id INTEGER PRIMARY KEY, email TEXT);",
        10,
    )
    .unwrap();
    execute_source_add_and_connect(&mut ed, &path);

    // A *sql* console buffer with an aliased FROM.
    let key = ed.create_buffer_from_str("*sql: c*", "SELECT u. FROM users u");
    if let Some(win) = ed.view_tree.focused_window_mut() {
        win.buffer_id = Some(key);
    }
    // Cursor right behind the dot after the alias.
    ed.view_of_buffer_mut(key).unwrap().set_cursor("SELECT u.".len());
    ed.db.consoles.insert(ed.db.active.unwrap(), key);

    crate::kernel::completion::trigger(&mut ed);
    let labels: Vec<String> = ed
        .completion
        .items
        .iter()
        .map(|i| i.label.clone())
        .collect();
    assert!(
        labels.contains(&"id".to_string()) && labels.contains(&"email".to_string()),
        "alias columns suggested: {labels:?}"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn sql_console_completion_suggests_tables_after_from() {
    let mut ed = make_editor();
    let dir = std::env::temp_dir().join(format!(
        "magma-compl2-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("c.sqlite");
    let conn = ::db::connect(
        &::db::DataSource {
            id: 0,
            name: "c".into(),
            kind: db::DriverKind::Sqlite,
            host: path.to_string_lossy().to_string(),
            port: 0,
            database: String::new(),
            user: String::new(),
            ssl_mode: Default::default(),
            trust_cert: false,
            save_password: false,
            url: None,
        },
        None,
    )
    .unwrap();
    conn.execute_script(
        "CREATE TABLE orders (id INTEGER PRIMARY KEY); CREATE TABLE payments (id INTEGER);",
        10,
    )
    .unwrap();
    execute_source_add_and_connect(&mut ed, &path);

    let key = ed.create_buffer_from_str("*sql: c*", "SELECT * FROM ord");
    if let Some(win) = ed.view_tree.focused_window_mut() {
        win.buffer_id = Some(key);
    }
    ed.view_of_buffer_mut(key).unwrap().set_cursor("SELECT * FROM ord".len());
    ed.db.consoles.insert(ed.db.active.unwrap(), key);

    crate::kernel::completion::trigger(&mut ed);
    let labels: Vec<String> = ed
        .completion
        .items
        .iter()
        .map(|i| i.label.clone())
        .collect();
    assert!(
        labels.contains(&"orders".to_string()),
        "table suggested after FROM: {labels:?}"
    );
    let _ = std::fs::remove_dir_all(dir);
}

fn execute_source_add_and_connect(ed: &mut Editor, path: &std::path::Path) {
    use kernel::command::args::ArgValue;
    use std::collections::HashMap;
    let mut args = HashMap::new();
    args.insert("name".to_string(), ArgValue::String("c".into()));
    args.insert("kind".to_string(), ArgValue::String("sqlite".into()));
    args.insert(
        "host".to_string(),
        ArgValue::Path(path.to_string_lossy().to_string()),
    );
    crate::kernel::command::execute_command(ed, "db-source-add", &args).unwrap();
    let id = ed.db.active.unwrap();
    let mut args = HashMap::new();
    args.insert("source".to_string(), ArgValue::Integer(id as i64));
    crate::kernel::command::execute_command(ed, "db-connect", &args).unwrap();
}
