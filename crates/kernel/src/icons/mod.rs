//! Icon packs — the editor-facing side.
//!
//! The pack model (`IconKind`, `IconPack`, lookup order), the pack search
//! dirs, and the VS Code manifest loader live in the `icons` crate.  What
//! stays here is the part that touches editor state: following the
//! `icon-pack` option, swapping `Editor.icons`, and restoring the
//! Janet-installed default pack in `Editor.default_icons`.

pub use icons::{IconKind, IconPack, find_and_load, pack_search_dirs};
pub use icons::vscode;

use std::path::Path;

use crate::state::{Editor, MessageKind};

/// Non-mutating option follower, run each frame next to `sync_theme`.
/// When the `icon-pack` option names a pack other than the active one,
/// load it; "default" restores the Janet-installed default pack.
///
/// A pack that fails to load clears the option and surfaces one message —
/// the alternative is re-reading a broken file every frame.
pub fn sync_option(ed: &mut Editor) {
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
                MessageKind::Error,
                format!("icon-pack {want:?} not loaded: {e}"),
            );
        }
    }
}
