//! Dired display formatting and directory reading.

use std::collections::HashMap;
use std::path::Path;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use super::dired::{DiredEntry, MarkType, SortField};

/// Number of header lines above the first entry.
pub const HEADER_LINES: usize = 4;

fn format_size(bytes: u64) -> String {
    match bytes {
        b if b < 1_024 => format!("{b}B"),
        b if b < 1_048_576 => format!("{}K", b / 1_024),
        b if b < 1_073_741_824 => format!("{}M", b / 1_048_576),
        b => format!("{}G", b / 1_073_741_824),
    }
}

#[cfg(unix)]
fn format_perms(is_dir: bool, is_symlink: bool, mode: u32) -> String {
    let type_char = if is_dir { 'd' } else if is_symlink { 'l' } else { '-' };
    let bits = [
        (0o400, 'r'), (0o200, 'w'), (0o100, 'x'),
        (0o040, 'r'), (0o020, 'w'), (0o010, 'x'),
        (0o004, 'r'), (0o002, 'w'), (0o001, 'x'),
    ];
    let rwx: String = bits.iter().map(|(bit, ch)| if mode & bit != 0 { *ch } else { '-' }).collect();
    format!("{type_char}{rwx}")
}

#[cfg(not(unix))]
fn format_perms(is_dir: bool, _is_symlink: bool, _mode: u32) -> String {
    if is_dir { "d---------".to_string() } else { "----------".to_string() }
}

fn format_modified(time: std::time::SystemTime) -> (String, u64) {
    let Ok(dur) = time.duration_since(std::time::UNIX_EPOCH) else {
        return ("?".to_string(), 0);
    };
    let secs = dur.as_secs();
    let s = secs % 60;
    let m = (secs / 60) % 60;
    let h = (secs / 3600) % 24;
    let days = secs / 86400;
    let year = 1970 + days / 365;
    let day_of_year = days % 365;
    let month = (day_of_year / 30).min(11);
    let day = day_of_year % 30 + 1;
    let months = ["Jan","Feb","Mar","Apr","May","Jun","Jul","Aug","Sep","Oct","Nov","Dec"];
    let out = format!("{} {:>2} {:02}:{:02}:{:02} {}", months[month as usize], day, h, m, s, year);
    (out, secs)
}

pub fn mark_char(mt: MarkType) -> &'static str {
    match mt {
        MarkType::Delete => "D",
        MarkType::Copy   => "C",
        MarkType::Move   => "M",
    }
}

pub fn is_executable(perms: &str) -> bool {
    perms.as_bytes().get(3) == Some(&b'x')
}

/// Read `dir`, returning unsorted entries (including `.` and `..`).
pub fn read_dir(dir: &Path) -> Result<Vec<DiredEntry>, String> {
    let mut entries: Vec<DiredEntry> = Vec::new();

    for special in [".", ".."] {
        let path = if special == "." { dir.to_path_buf() } else {
            dir.parent().unwrap_or(dir).to_path_buf()
        };
        if let Ok(meta) = std::fs::metadata(&path) {
            #[cfg(unix)]
            let mode = meta.permissions().mode();
            #[cfg(not(unix))]
            let mode = 0u32;
            let (modified, modified_raw) = meta.modified().map(format_modified).unwrap_or_else(|_| ("?".to_string(), 0));
            entries.push(DiredEntry {
                name: special.to_string(),
                is_dir: true,
                is_symlink: false,
                size: meta.len(),
                perms: format_perms(true, false, mode),
                modified,
                modified_raw,
            });
        }
    }

    let read = std::fs::read_dir(dir).map_err(|e| format!("Cannot read directory: {e}"))?;
    let rest: Vec<DiredEntry> = read
        .filter_map(|r| r.ok())
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let meta = entry.metadata().ok()?;
            let symlink_meta = std::fs::symlink_metadata(entry.path()).ok()?;
            let is_symlink = symlink_meta.file_type().is_symlink();
            let is_dir = meta.is_dir();
            #[cfg(unix)]
            let mode = symlink_meta.permissions().mode();
            #[cfg(not(unix))]
            let mode = 0u32;
            let (modified, modified_raw) = meta.modified().map(format_modified).unwrap_or_else(|_| ("?".to_string(), 0));
            Some(DiredEntry {
                perms: format_perms(is_dir, is_symlink, mode),
                size: meta.len(),
                modified,
                modified_raw,
                name,
                is_dir,
                is_symlink,
            })
        })
        .collect();

    entries.extend(rest);
    Ok(entries)
}

fn sort_entries<'a>(entries: &mut [&'a DiredEntry], field: SortField, reverse: bool) {
    entries.sort_by(|a, b| {
        let cmp = match field {
            SortField::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            SortField::Size => a.size.cmp(&b.size),
            SortField::Date => a.modified_raw.cmp(&b.modified_raw),
        };
        if reverse { cmp.reverse() } else { cmp }
    });
}

fn sort_char(field: SortField, reverse: bool) -> &'static str {
    match (field, reverse) {
        (SortField::Name, false) => "N",
        (SortField::Name, true)  => "N↑",
        (SortField::Size, false) => "S",
        (SortField::Size, true)  => "S↑",
        (SortField::Date, false) => "D",
        (SortField::Date, true)  => "D↑",
    }
}

/// Build sorted, filtered entry view for display.
pub fn view_entries<'a>(
    entries: &'a [DiredEntry],
    filter: Option<&str>,
    show_hidden: bool,
    field: SortField,
    reverse: bool,
) -> Vec<&'a DiredEntry> {
    let mut filtered: Vec<&DiredEntry> = entries
        .iter()
        .filter(|e| {
            if e.name == "." || e.name == ".." { return true; }
            if !show_hidden && e.name.starts_with('.') { return false; }
            if let Some(pat) = filter {
                if !pat.is_empty() && !e.name.to_lowercase().contains(&pat.to_lowercase()) {
                    return false;
                }
            }
            true
        })
        .collect();

    let mut special = Vec::new();
    let mut rest = Vec::new();
    for e in filtered.drain(..) {
        if e.name == "." || e.name == ".." {
            special.push(e);
        } else {
            rest.push(e);
        }
    }

    sort_entries(&mut rest, field, reverse);
    special.sort_by(|a, b| {
        let a_is_parent = a.name == "..";
        let b_is_parent = b.name == "..";
        b_is_parent.cmp(&a_is_parent)
    });

    let mut result = special;
    result.extend(rest);
    result
}

/// Build the full text content for the dired buffer.
pub fn build_display(
    dir: &Path,
    entries: &[DiredEntry],
    marks: &HashMap<String, MarkType>,
    sort_field: SortField,
    sort_reverse: bool,
    filter_pattern: Option<&str>,
    show_hidden: bool,
) -> String {
    let dir_str = dir.to_string_lossy();
    let view = view_entries(entries, filter_pattern, show_hidden, sort_field, sort_reverse);
    let sort_info = sort_char(sort_field, sort_reverse);
    let filter_info = filter_pattern.filter(|p| !p.is_empty());
    let has_active_filter = filter_info.is_some();

    let header_name = if let Some(pat) = filter_info {
        format!("Name (filter: {pat} sort: {sort_info})")
    } else {
        format!("Name (sort: {sort_info})")
    };

    let mut out = format!(
        "{dir_str}\n\n  {:<10}  {:>6}  {:<21}  {}\n  {}\n",
        "Perms", "Size", "Modified", &header_name,
        "──────────  ──────  ─────────────────────  ──────────────────────────────",
    );
    for entry in &view {
        let mt = marks.get(&entry.name);
        let mark = if let Some(t) = mt {
            format!("[{}]", mark_char(*t))
        } else {
            "   ".to_string()
        };
        let size = if entry.is_dir { "<DIR>".to_string() } else { format_size(entry.size) };
        let display_name = if entry.is_dir {
            format!("{}/", entry.name)
        } else if entry.is_symlink {
            format!("{}@", entry.name)
        } else {
            entry.name.clone()
        };
        out.push_str(&format!(
            "{mark} {:<10}  {:>6}  {:<21}  {}\n",
            &entry.perms[..entry.perms.len().min(10)],
            size,
            &entry.modified[..entry.modified.len().min(21)],
            display_name,
        ));
    }

    if !marks.is_empty() {
        out.push('\n');
        let mut counts: HashMap<MarkType, usize> = HashMap::new();
        for (_, mt) in marks {
            *counts.entry(*mt).or_default() += 1;
        }
        let mut parts: Vec<String> = Vec::new();
        for (mt, cnt) in [(MarkType::Delete, "delete"), (MarkType::Copy, "copy"), (MarkType::Move, "move")] {
            if let Some(n) = counts.get(&mt) {
                parts.push(format!("{cnt}:{n}"));
            }
        }
        out.push_str(&format!("  Marks — {}\n", parts.join("  ")));
    }

    if has_active_filter {
        out.push_str("  [filter active — clear with :Dfilter ]\n");
    }

    out
}
