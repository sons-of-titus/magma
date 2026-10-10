//! Icon packs — file/folder icon resolution for the GUI.
//!
//! The active pack lives in `Editor.icons`; the Janet-owned default pack is
//! snapshotted in `Editor.default_icons` when `icons/set-pack` installs it.
//! VS Code icon-theme packs load from disk via `icons/load-pack` (see
//! `vscode.rs`).  Rendering reads the pack; it never holds per-extension
//! knowledge itself.

pub mod vscode;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// One icon: either a glyph in the bundled icon font, or an SVG file
/// (relative paths resolved against the pack directory).
#[derive(Debug, Clone, PartialEq)]
pub enum IconKind {
    Glyph {
        glyph: char,
        color: Option<(u8, u8, u8)>,
    },
    Svg {
        path: PathBuf,
    },
}

impl IconKind {
    pub fn glyph(glyph: char) -> Self {
        IconKind::Glyph { glyph, color: None }
    }
}

/// Directories searched when the `icon-pack` option (or `icons/load-pack`)
/// names a pack without a path.
pub fn pack_search_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let home = std::env::var("HOME").unwrap_or_default();
    if !home.is_empty() {
        dirs.push(PathBuf::from(format!("{home}/.config/magma/icon-packs")));
    }
    dirs.push(PathBuf::from(".magma/icon-packs"));
    dirs
}

#[derive(Debug, Clone, Default)]
pub struct IconPack {
    pub name: String,
    /// Exact file names: "Makefile", "Dockerfile", "LICENSE".
    pub file_names: HashMap<String, IconKind>,
    /// Extensions without the leading dot; keys may be compound
    /// ("spec.ts") — lookup prefers the longest match.
    pub extensions: HashMap<String, IconKind>,
    pub default_file: Option<IconKind>,
    pub folder: Option<IconKind>,
    pub folder_open: Option<IconKind>,
}

impl IconPack {
    pub fn empty(name: &str) -> Self {
        IconPack {
            name: name.to_string(),
            ..Default::default()
        }
    }

    /// Exact file name first, then longest-suffix extension match
    /// ("app.spec.ts" tries "spec.ts" before "ts"), then the pack default.
    pub fn lookup(&self, file_name: &str) -> Option<&IconKind> {
        if let Some(icon) = self.file_names.get(file_name) {
            return Some(icon);
        }
        let lower = file_name.to_lowercase();
        for (i, _) in lower.match_indices('.') {
            if let Some(icon) = self.extensions.get(&lower[i + 1..]) {
                return Some(icon);
            }
        }
        self.default_file.as_ref()
    }

    pub fn folder_icon(&self, open: bool) -> Option<&IconKind> {
        if open {
            self.folder_open.as_ref().or(self.folder.as_ref())
        } else {
            self.folder.as_ref()
        }
    }
}

/// Non-mutating option follower, run each frame next to `sync_theme`.
/// When the `icon-pack` option names a pack other than the active one,
/// load it; "default" restores the Janet-installed default pack.
///
/// A pack that fails to load clears the option and surfaces one message —
/// the alternative is re-reading a broken file every frame.
pub fn sync_option(ed: &mut crate::kernel::state::Editor) {
    let Some(want) = ed.options.get("icon-pack").cloned() else {
        return;
    };
    if want == ed.icons.name {
        return;
    }
    if want == "default" {
        ed.icons = ed.default_icons.clone();
        return;
    }
    match find_and_load(Path::new(&want)) {
        Ok(pack) => ed.icons = pack,
        Err(e) => {
            ed.options.remove("icon-pack");
            ed.show_message(
                crate::kernel::state::MessageKind::Error,
                format!("icon-pack {want:?} not loaded: {e}"),
            );
        }
    }
}

/// Resolve a pack name or path and build its `IconPack`.
/// Bare names are searched in `pack_search_dirs`.
pub fn find_and_load(name_or_path: &Path) -> Result<IconPack, String> {
    let candidates: Vec<PathBuf> = if name_or_path.is_absolute() || name_or_path.exists() {
        vec![name_or_path.to_path_buf()]
    } else {
        pack_search_dirs()
            .into_iter()
            .map(|dir| dir.join(name_or_path))
            .collect()
    };
    for candidate in candidates {
        if !candidate.exists() {
            continue;
        }
        return vscode::load_pack(&candidate);
    }
    Err(format!(
        "not found (searched icon-pack dirs and {name_or_path:?})"
    ))
}
