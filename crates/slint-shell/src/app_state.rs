//! The shell's per-window state: everything the projection pass reads and
//! the action callbacks mutate, one `RefCell` away from every callback.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use kernel::project::tree::FileNode;
use kernel::render::gpu_atlas::GpuGlyphAtlas;
use kernel::render::surface::Surface;
use kernel::runtime::BackgroundEvent;
use kernel::state::Editor;
use kernel::workspace::WorkspaceRuntime;

use crate::actions::picker;
use crate::sync;
use crate::{CompletionEntry, FindRow, GitHistoryRow, MenuItem, OutputLine, ProblemRow, ProgressTask, TabItem, TemplateItem, TemplateVar, TreeItem, WelcomeWindow};
use crate::context;

use crate::new_project;

pub(crate) struct AppState {
    pub(crate) editor: Editor,
    pub(crate) bg_receiver: tokio::sync::mpsc::UnboundedReceiver<BackgroundEvent>,
    pub(crate) atlas: GpuGlyphAtlas,
    pub(crate) atlas_ppx: f32,
    pub(crate) cell_w: u32,
    pub(crate) cell_h: u32,
    /// Empty leading above the glyphs in a row; the caret starts below it.
    pub(crate) caret_inset: f32,
    /// Beam height in logical px: cap line to baseline, measured from the
    /// atlas like `caret_inset` so the beam ends where the ink ends.
    pub(crate) caret_height: f32,
    pub(crate) surface: Surface,
    pub(crate) terminal_grid: (usize, u32, u32),
    pub(crate) surface_sig: u64,
    pub(crate) gutter_sig: u64,
    pub(crate) image_pane_key: u64,
    pub(crate) rendered_revision: u64,
    pub(crate) message: String,
    pub(crate) workspace: WorkspaceRuntime,
    pub(crate) tree_nodes: Vec<FileNode>,
    pub(crate) dirs: HashSet<String>,
    pub(crate) expanded: HashSet<String>,
    pub(crate) tree_epoch: u64,
    pub(crate) tree_root: Option<PathBuf>,
    pub(crate) rail: i32,
    pub(crate) sidebar_open: bool,
    /// Bottom dock height; the top-edge drag handle edits it.
    pub(crate) bottom_height: f32,
    /// Output pane follows the newest lines as they land.
    pub(crate) output_follow: bool,
    /// Per-tool dock widths (project, commit, debug); the drag handle edits
    /// the active one.
    pub(crate) sidebar_widths: [f32; 3],
    /// DAP variablesReferences the user opened in the variables tree.
    /// The Database tool window on the right, beneath the Agent dock.
    pub(crate) db_open: bool,
    pub(crate) db_height: f32,
    pub(crate) db_form_open: bool,
    pub(crate) db_form_note: String,
    pub(crate) db_expanded: HashSet<usize>,
    pub(crate) debug_expanded: HashSet<i64>,
    /// The scope reference those expansions belong to; a new stop resets.
    pub(crate) debug_scope_seen: Option<i64>,
    /// Slot table from the last debug sync: UI row ids → DAP ids.
    pub(crate) debug_ref_slots: Vec<i64>,
    /// Chrome syncs from the first frame; only the editor surface waits for
    /// the window's real size — the pre-map stub sizes would otherwise
    /// derive a viewport scroll that never unwinds.
    pub(crate) window_ready: bool,
    pub(crate) last_logical_width: f32,
    /// The launcher window; visible exactly while no project is open.
    pub(crate) welcome: slint::Weak<WelcomeWindow>,
    pub(crate) launcher_visible: bool,
    pub(crate) theme_name: String,
    pub(crate) app_font_name: String,
    pub(crate) app_font_size: f32,
    pub(crate) menu_title: Option<&'static str>,
    pub(crate) menu_x: f32,
    pub(crate) menu_visible: bool,
    pub(crate) bottom: i32,
    pub(crate) agent_open: bool,
    /// Pinned session for the agent dock; `None` follows the newest.
    pub(crate) agent_active: Option<u64>,
    /// Transcript entries the user folded open, keyed by (session, index).
    pub(crate) agent_expanded: HashSet<(u64, usize)>,
    pub(crate) agent_dock_width: f32,
    pub(crate) last_px: f32,
    pub(crate) last_py: f32,
    pub(crate) hovering: bool,
    /// The active picker surface (Go to File/Symbol, buffers, palette,
    /// prompts).  `None` while hidden.
    pub(crate) picker: picker::PickerKind,
    pub(crate) picker_query: String,
    pub(crate) picker_selected: usize,
    /// The full (unfiltered) picker entries: title, detail, meta, payload.
    pub(crate) picker_items: Vec<picker::PickerItem>,
    /// Indices into `picker_items` the visible rows show, in row order.
    pub(crate) visible_items: Vec<usize>,
    /// Find bar state: mode, and the match readout text.
    pub(crate) find_visible: bool,
    pub(crate) find_replace_visible: bool,
    pub(crate) find_query: String,
    pub(crate) find_replacement: String,
    pub(crate) find_match_text: String,
    pub(crate) find_has_error: bool,
    /// The open context menu and the object it targets (tab index or tree
    /// path), so its commands know what to act on.
    pub(crate) context_menu: Option<context::ContextTarget>,
    /// The path a tree context menu was raised on.
    pub(crate) context_path: String,
    /// The unsaved-changes dialog raised over a refused quit.
    pub(crate) quit_confirm_visible: bool,
    /// New Project wizard overlay state.
    pub(crate) new_project: new_project::NewProjectState,
    /// Save was chosen in that dialog; quit once the dirty set drains
    /// (async writes keep buffers dirty until the write-confirmed events
    /// land).
    pub(crate) quit_when_clean: bool,
    pub(crate) settings_visible: bool,
    pub(crate) settings_category: i32,
    pub(crate) settings_search: String,
    /// Options/page changed and the settings rows need re-projection.
    pub(crate) settings_rows_dirty: bool,
    pub(crate) terminal_scroll: usize,
    pub(crate) git_history: Vec<GitHistoryRow>,
    pub(crate) wheel_y: f32,
    pub(crate) wheel_x: f32,
    /// Signature gates so idle refreshes skip re-blitting textures.
    pub(crate) minimap_sig: u64,
    pub(crate) terminal_sig: u64,
    /// Decoded file-type icons, keyed by SVG path.
    pub(crate) icon_images: HashMap<String, slint::Image>,
    pub(crate) m_tabs: sync::ModelGate<TabItem>,
    pub(crate) m_tree: sync::ModelGate<TreeItem>,
    pub(crate) m_crumbs: sync::ModelGate<slint::SharedString>,
    pub(crate) m_find_rows: sync::ModelGate<FindRow>,
    pub(crate) m_output_lines: sync::ModelGate<OutputLine>,
    pub(crate) m_problems: sync::ModelGate<ProblemRow>,
    pub(crate) m_progress: sync::ModelGate<ProgressTask>,
    pub(crate) m_recents: sync::ModelGate<slint::SharedString>,
    pub(crate) m_sessions: sync::ModelGate<slint::SharedString>,
    pub(crate) m_git_history: sync::ModelGate<GitHistoryRow>,
    pub(crate) m_menu: sync::ModelGate<MenuItem>,
    pub(crate) m_new_project_templates: sync::ModelGate<TemplateItem>,
    pub(crate) m_new_project_vars: sync::ModelGate<TemplateVar>,
    pub(crate) m_new_project_languages: sync::ModelGate<slint::SharedString>,
    pub(crate) m_hover_lines: sync::ModelGate<slint::SharedString>,
    pub(crate) m_completion_entries: sync::ModelGate<CompletionEntry>,
    pub(crate) m_quickfix_items: sync::ModelGate<slint::SharedString>,
}


/// macOS dark-mode detection for the default theme.  `defaults read` fails
/// (non-zero) when the key is unset, which is the light mode case.
#[cfg(target_os = "macos")]
pub(crate) fn system_prefers_dark() -> bool {
    std::process::Command::new("defaults")
        .args(["read", "-g", "AppleInterfaceStyle"])
        .output()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .trim()
                .eq_ignore_ascii_case("dark")
        })
        .unwrap_or(false)
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn system_prefers_dark() -> bool {
    std::env::var("COLORFGBG")
        .ok()
        .and_then(|v| v.split(';').next_back()?.parse::<u8>().ok())
        .map(|bg| bg == 0 || bg == 8)
        .unwrap_or(false)
}

