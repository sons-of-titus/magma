use std::collections::HashMap;
use std::path::PathBuf;

use crate::kernel::vc::dired::{
    self, build_display, mark_char, read_dir, view_entries, DiredEntry, DiredState,
    MarkType, SortField, HEADER_LINES,
};

fn make_entry(name: &str, is_dir: bool) -> DiredEntry {
    DiredEntry {
        name: name.to_string(),
        is_dir,
        is_symlink: false,
        size: 100,
        perms: if is_dir { "drwxr-xr-x".to_string() } else { "-rw-r--r--".to_string() },
        modified: "Jun 17 12:34:56 2026".to_string(),
        modified_raw: 1_700_000_000,
    }
}

fn make_entry_full(
    name: &str, is_dir: bool, is_symlink: bool, size: u64, perms: &str, modified_raw: u64,
) -> DiredEntry {
    DiredEntry {
        name: name.to_string(),
        is_dir,
        is_symlink,
        size,
        perms: perms.to_string(),
        modified: "Jun 17 12:34:56 2026".to_string(),
        modified_raw,
    }
}

#[test]
fn dired_state_new_defaults() {
    let ds = DiredState::new();
    assert!(ds.dir.as_os_str().is_empty());
    assert!(ds.entries.is_empty());
    assert!(ds.marks.is_empty());
    assert!(ds.buf_key.is_none());
    assert!(!ds.active);
    assert_eq!(ds.sort_field, SortField::Name);
    assert!(!ds.sort_reverse);
    assert!(!ds.show_hidden);
    assert!(ds.filter_pattern.is_none());
}

#[test]
fn dired_mark_char_returns_correct_chars() {
    assert_eq!(mark_char(MarkType::Delete), "D");
    assert_eq!(mark_char(MarkType::Copy), "C");
    assert_eq!(mark_char(MarkType::Move), "M");
}

#[test]
fn dired_toggle_mark_round_trip() {
    let mut ds = DiredState::new();
    ds.entries.push(make_entry("foo.txt", false));

    ds.toggle_mark("foo.txt", MarkType::Delete);
    assert_eq!(ds.marks.get("foo.txt"), Some(&MarkType::Delete));

    ds.toggle_mark("foo.txt", MarkType::Delete);
    assert!(ds.marks.get("foo.txt").is_none());
}

#[test]
fn dired_toggle_mark_different_types() {
    let mut ds = DiredState::new();
    ds.entries.push(make_entry("a.txt", false));

    ds.toggle_mark("a.txt", MarkType::Delete);
    ds.toggle_mark("a.txt", MarkType::Copy);
    assert_eq!(ds.marks.get("a.txt"), Some(&MarkType::Copy));
}

#[test]
fn dired_toggle_mark_ignores_dot_entries() {
    let mut ds = DiredState::new();
    ds.toggle_mark(".", MarkType::Delete);
    ds.toggle_mark("..", MarkType::Delete);
    assert!(ds.marks.is_empty());
}

#[test]
fn dired_invert_marks_inverts_all() {
    let mut ds = DiredState::new();
    ds.entries.push(make_entry(".", true));
    ds.entries.push(make_entry("..", true));
    ds.entries.push(make_entry("a.txt", false));
    ds.entries.push(make_entry("b.txt", false));
    ds.toggle_mark("a.txt", MarkType::Delete);

    ds.invert_marks();
    assert!(ds.marks.get("a.txt").is_none());
    assert_eq!(ds.marks.get("b.txt"), Some(&MarkType::Delete));
}

#[test]
fn dired_mark_counts_all_types() {
    let mut ds = DiredState::new();
    ds.toggle_mark("a.txt", MarkType::Delete);
    ds.toggle_mark("b.txt", MarkType::Copy);
    ds.toggle_mark("c.txt", MarkType::Move);

    let counts = ds.mark_counts();
    assert_eq!(counts[0], (MarkType::Delete, 1));
    assert_eq!(counts[1], (MarkType::Copy, 1));
    assert_eq!(counts[2], (MarkType::Move, 1));
}

#[test]
fn dired_entry_at_line_accounts_for_header() {
    let mut ds = DiredState::new();
    ds.entries.push(make_entry("a.txt", false));
    ds.entries.push(make_entry("b.txt", false));

    assert!(ds.entry_at_line(0).is_none());
    assert!(ds.entry_at_line(HEADER_LINES - 1).is_none());
    let e = ds.entry_at_line(HEADER_LINES);
    assert!(e.is_some());
    assert_eq!(e.unwrap().name, "a.txt");
}

#[test]
fn dired_full_path_joins_dir_and_name() {
    let mut ds = DiredState::new();
    ds.dir = PathBuf::from("/tmp");
    assert_eq!(ds.full_path("test.txt"), PathBuf::from("/tmp/test.txt"));
}

#[test]
fn dired_build_display_contains_header() {
    let dir = PathBuf::from("/tmp");
    let entries = vec![make_entry("a.txt", false)];
    let marks = HashMap::new();

    let text = build_display(&dir, &entries, &marks,
        SortField::Name, false, None, false);
    assert!(text.contains("/tmp"));
    assert!(text.contains("Perms"));
    assert!(text.contains("Name"));
    assert!(text.contains("a.txt"));
}

#[test]
fn dired_build_display_shows_marks() {
    let dir = PathBuf::from("/tmp");
    let entry = make_entry("a.txt", false);
    let entries = vec![entry];
    let mut marks = HashMap::new();
    marks.insert("a.txt".to_string(), MarkType::Delete);

    let text = build_display(&dir, &entries, &marks,
        SortField::Name, false, None, false);
    assert!(text.contains("[D]"));
}

#[test]
fn dired_build_display_shows_status_line() {
    let dir = PathBuf::from("/tmp");
    let entries = vec![make_entry("a.txt", false)];
    let mut marks = HashMap::new();
    marks.insert("a.txt".to_string(), MarkType::Delete);
    marks.insert("b.txt".to_string(), MarkType::Copy);

    let text = build_display(&dir, &entries, &marks,
        SortField::Name, false, None, false);
    assert!(text.contains("Marks"));
    assert!(text.contains("delete:1"));
    assert!(text.contains("copy:1"));
}

#[test]
fn dired_build_display_shows_filter_indicator() {
    let dir = PathBuf::from("/tmp");
    let entries = vec![make_entry("a.txt", false)];
    let marks = HashMap::new();

    let text = build_display(&dir, &entries, &marks,
        SortField::Name, false, Some("filter"), false);
    assert!(text.contains("filter"));
}

#[test]
fn dired_build_display_shows_sort_indicator() {
    let dir = PathBuf::from("/tmp");
    let entries = vec![make_entry("a.txt", false)];
    let marks = HashMap::new();

    let text = build_display(&dir, &entries, &marks,
        SortField::Size, true, None, false);
    assert!(text.contains("S"));
}

#[test]
fn dired_view_entries_filters_by_pattern() {
    let entries = vec![
        make_entry("foo.txt", false),
        make_entry("bar.txt", false),
        make_entry("baz.rs", false),
    ];

    let view = view_entries(&entries, Some("foo"), false, SortField::Name, false);
    assert_eq!(view.len(), 1);
    assert_eq!(view[0].name, "foo.txt");
}

#[test]
fn dired_view_entries_hides_dotfiles() {
    let entries = vec![
        make_entry("visible.txt", false),
        make_entry(".hidden", false),
    ];

    let view = view_entries(&entries, None, false, SortField::Name, false);
    assert_eq!(view.len(), 1);
    assert_eq!(view[0].name, "visible.txt");
}

#[test]
fn dired_view_entries_shows_dotfiles_when_enabled() {
    let entries = vec![
        make_entry("visible.txt", false),
        make_entry(".hidden", false),
    ];

    let view = view_entries(&entries, None, true, SortField::Name, false);
    assert_eq!(view.len(), 2);
    assert!(view.iter().any(|e| e.name == ".hidden"));
}

#[test]
fn dired_view_entries_passes_through_entries_without_dot() {
    let entries = vec![
        make_entry("f.txt", false),
    ];

    let view = view_entries(&entries, None, false, SortField::Name, false);
    assert_eq!(view.len(), 1);
    assert_eq!(view[0].name, "f.txt");
}

#[test]
fn dired_read_dir_returns_dot_entries() {
    let tmp = std::env::temp_dir().join("magma_dired_test_read");
    std::fs::create_dir_all(&tmp).unwrap();

    let result = read_dir(&tmp);
    assert!(result.is_ok());
    let entries = result.unwrap();
    assert!(entries.iter().any(|e| e.name == "."));
    assert!(entries.iter().any(|e| e.name == ".."));

    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn dired_read_dir_fails_on_nonexistent_dir() {
    let result = read_dir(&PathBuf::from("/nonexistent_magma_dir_12345"));
    assert!(result.is_err());
}

#[test]
fn dired_entry_at_line_returns_none_for_empty_entries() {
    let ds = DiredState::new();
    assert!(ds.entry_at_line(HEADER_LINES).is_none());
    assert!(ds.entry_at_line(999).is_none());
}

#[test]
fn dired_reload_populates_entries() {
    let tmp = std::env::temp_dir().join("magma_dired_test_reload");
    std::fs::create_dir_all(&tmp).unwrap();
    std::fs::write(tmp.join("test.txt"), "hello").unwrap();

    let mut ds = DiredState::new();
    ds.dir = tmp.clone();
    let result = ds.reload();
    assert!(result.is_ok());
    assert!(!ds.entries.is_empty());
    assert!(ds.entries.iter().any(|e| e.name == "test.txt"));

    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn dired_visible_entries_respects_filters() {
    let mut ds = DiredState::new();
    ds.entries.push(make_entry("foo.txt", false));
    ds.entries.push(make_entry("bar.txt", false));
    ds.filter_pattern = Some("foo".to_string());

    let visible = ds.visible_entries();
    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0].name, "foo.txt");
}

#[test]
fn dired_is_executable_detects_owner_x_bit() {
    assert!(dired::is_executable("-rwxr--r--"));
    assert!(!dired::is_executable("-rw-r--r--"));
}

#[test]
fn dired_sort_by_size() {
    let entries = vec![
        make_entry_full("large.txt", false, false, 9999, "-rw-r--r--", 0),
        make_entry_full("small.txt", false, false, 100, "-rw-r--r--", 0),
    ];
    let view = view_entries(&entries, None, false, SortField::Size, false);
    assert_eq!(view[0].name, "small.txt");
    assert_eq!(view[1].name, "large.txt");

    let rev = view_entries(&entries, None, false, SortField::Size, true);
    assert_eq!(rev[0].name, "large.txt");
    assert_eq!(rev[1].name, "small.txt");
}

#[test]
fn dired_sort_by_date() {
    let entries = vec![
        make_entry_full("new.txt", false, false, 100, "-rw-r--r--", 9999),
        make_entry_full("old.txt", false, false, 100, "-rw-r--r--", 100),
    ];
    let view = view_entries(&entries, None, false, SortField::Date, false);
    assert_eq!(view[0].name, "old.txt");
    assert_eq!(view[1].name, "new.txt");
}
