//! Directory editor — dired-style buffer that lists a directory and supports
//! file operations (open, navigate, mark/delete, rename, copy, mkdir).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

/// A single entry in a dired listing.
#[derive(Debug, Clone)]
pub struct DiredEntry {
    pub name: String,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub size: u64,
    pub perms: String,
    pub modified: String,
}

/// Active dired session state.
#[derive(Debug, Default)]
pub struct DiredState {
    /// Absolute path of the displayed directory.
    pub dir: PathBuf,
    /// Sorted entries for the current directory.
    pub entries: Vec<DiredEntry>,
    /// Files marked for deletion.
    pub marks: HashSet<String>,
    /// Slab key of the `*dired*` buffer, if currently open.
    pub buf_key: Option<usize>,
    /// Whether a dired buffer is currently active.
    pub active: bool,
}

/// Number of header lines above the first entry (0-indexed).
pub const HEADER_LINES: usize = 4;

// ── Formatting helpers ────────────────────────────────────────────────────────

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

fn format_modified(time: std::time::SystemTime) -> String {
    let Ok(dur) = time.duration_since(std::time::UNIX_EPOCH) else {
        return "?".to_string();
    };
    let secs = dur.as_secs();
    // Minimal UTC breakdown (no external crate)
    let s = secs % 60;
    let m = (secs / 60) % 60;
    let h = (secs / 3600) % 24;
    let days = secs / 86400;
    // Approx year/month/day from epoch days
    let year = 1970 + days / 365;
    let day_of_year = days % 365;
    let month = (day_of_year / 30).min(11);
    let day = day_of_year % 30 + 1;
    let months = ["Jan","Feb","Mar","Apr","May","Jun","Jul","Aug","Sep","Oct","Nov","Dec"];
    format!("{} {:>2} {:02}:{:02}:{:02} {}", months[month as usize], day, h, m, s, year)
}

// ── Directory reading ─────────────────────────────────────────────────────────

/// Read `dir`, returning entries sorted: `.` and `..` first, dirs before files,
/// then alphabetically within each group.
pub fn read_dir(dir: &Path) -> Result<Vec<DiredEntry>, String> {
    let mut entries: Vec<DiredEntry> = Vec::new();

    // Always add . and ..
    for special in [".", ".."] {
        let path = if special == "." { dir.to_path_buf() } else {
            dir.parent().unwrap_or(dir).to_path_buf()
        };
        if let Ok(meta) = std::fs::metadata(&path) {
            #[cfg(unix)]
            let mode = meta.permissions().mode();
            #[cfg(not(unix))]
            let mode = 0u32;
            let modified = meta.modified().map(format_modified).unwrap_or_else(|_| "?".to_string());
            entries.push(DiredEntry {
                name: special.to_string(),
                is_dir: true,
                is_symlink: false,
                size: meta.len(),
                perms: format_perms(true, false, mode),
                modified,
            });
        }
    }

    let read = std::fs::read_dir(dir).map_err(|e| format!("Cannot read directory: {e}"))?;
    let mut rest: Vec<DiredEntry> = read
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
            let modified = meta.modified().map(format_modified).unwrap_or_else(|_| "?".to_string());
            Some(DiredEntry {
                perms: format_perms(is_dir, is_symlink, mode),
                size: meta.len(),
                modified,
                name,
                is_dir,
                is_symlink,
            })
        })
        .collect();

    rest.sort_by(|a, b| {
        b.is_dir.cmp(&a.is_dir).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    entries.extend(rest);
    Ok(entries)
}

// ── Display ───────────────────────────────────────────────────────────────────

/// Build the full text content for the dired buffer.
pub fn build_display(dir: &Path, entries: &[DiredEntry], marks: &HashSet<String>) -> String {
    let dir_str = dir.to_string_lossy();
    let mut out = format!(
        "{dir_str}\n\n  {:<10}  {:>6}  {:<21}  {}\n  {}\n",
        "Perms", "Size", "Modified", "Name",
        "──────────  ──────  ─────────────────────  ──────────────────────────",
    );
    for entry in entries {
        let mark = if marks.contains(&entry.name) { "[D]" } else { "   " };
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
    out
}

// ── State operations ──────────────────────────────────────────────────────────

impl DiredState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Return the entry for display line `line` (0-indexed), or `None`.
    pub fn entry_at_line(&self, line: usize) -> Option<&DiredEntry> {
        let idx = line.checked_sub(HEADER_LINES)?;
        self.entries.get(idx)
    }

    /// Full path for a named entry in the current directory.
    pub fn full_path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    /// Reload entries from disk and return updated display text.
    pub fn reload(&mut self) -> Result<String, String> {
        self.entries = read_dir(&self.dir.clone())?;
        Ok(build_display(&self.dir, &self.entries, &self.marks))
    }
}
