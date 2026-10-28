//! Editor state — the central `Editor` struct and all sub-modules.

pub mod mode;
pub mod persist;
pub mod views;
pub use font;
pub mod completion;
pub mod cursor;
pub mod editing;
pub mod gutter;
pub mod io;
pub mod minimap;
pub mod options;
pub mod overlay;
pub mod search;
pub mod snippet;
pub mod visual;

use std::sync::{Arc, Mutex};

use crate::agent::AgentManager;
use crate::clipboard::Clipboard;
use crate::command::CommandRegistry;
use crate::debug::DebugManager;
use crate::event::EventBus;
use crate::extension::ExtensionRegistry;
use crate::keymap::KeymapManager;
use crate::render::view_tree::ViewTree;
use crate::runtime::BackgroundHandle;
use crate::scheduler::WorkScheduler;
use crate::script_runtime::ScriptRuntime;
use crate::semantic::{LanguageRegistry, SemanticEngine};
use crate::storage::FileSystem;
use crate::task::TaskScheduler;
use crate::text_engine::{Buffer, BufferView};
pub use id;

pub use crate::input::event::{MouseButton, MouseEventKind};
pub use crate::project::{
    Dependency, DependencyKind, Module, Project, ProjectManager, ProjectMember, Workspace,
};
pub use completion::CompletionState;
pub use cursor::{ExtraCursor, MultiCursorState};
pub use editing::EditingState;
pub use font::{ContextFont, FontConfig};
pub use gutter::{FoldIcons, GutterCell, GutterRegistry};
pub use io::IoState;
pub use mode::{EditorMode, SearchDirection, Selection};
pub use overlay::{LayoutSnapshot, Overlay};
pub use search::SearchState;
pub use snippet::SnippetState;

/// Tracks mouse press/drag state across frames.
#[derive(Debug, Clone)]
pub struct MouseState {
    pub pressed: bool,
    pub button: MouseButton,
    pub press_x: u16,
    pub press_y: u16,
    pub last_x: u16,
    pub last_y: u16,
    pub drag_active: bool,
    /// Multi-click detection: time and cell of the previous press, and the
    /// running count (1 = single, 2 = double, 3+ = line select).
    pub last_click: Option<(std::time::Instant, u16, u16)>,
    pub click_count: u32,
}

impl Default for MouseState {
    fn default() -> Self {
        MouseState {
            pressed: false,
            button: MouseButton::Left,
            press_x: 0,
            press_y: 0,
            last_x: 0,
            last_y: 0,
            drag_active: false,
            last_click: None,
            click_count: 0,
        }
    }
}

/// A hover card: code info shown near the pointer after it rests on a word.
#[derive(Debug, Clone)]
pub struct HoverCard {
    /// The word under the pointer when the card was created.
    pub word: String,
    /// Pointer position in pixels (GUI layer coordinates).
    pub px: f32,
    pub py: f32,
    /// When the pointer settled on this word — the card appears after a
    /// short dwell so quick sweeps never flash panels.
    pub since: std::time::Instant,
    /// Ready-to-render lines: declaration kind/name, definition sites,
    /// diagnostics on the line.
    pub lines: Vec<String>,
}

/// Severity of a transient status-bar message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageKind {
    Info,
    Error,
}

/// One transient status-bar message.  The GUI renders it for a few seconds
/// and then drops it; `*Messages*` is the durable log.
#[derive(Debug, Clone)]
pub struct StatusMessage {
    pub kind: MessageKind,
    pub text: String,
    pub at: std::time::Instant,
}

/// The single source of truth for all editor state.
/// All access is guarded by `Arc<RwLock<Editor>>`.
pub struct Editor {
    pub buffers: slab::Slab<Arc<Mutex<Buffer>>>,
    /// Per-window view state (indexed by buffer slab key).
    pub views: std::collections::HashMap<usize, BufferView>,
    /// Split-window view state: a window showing a buffer that another
    /// window also shows gets its own view fork, keyed by (window id,
    /// buffer slab key).  Buffers shown by a single window live in `views`.
    pub window_views: std::collections::HashMap<(crate::state::id::WindowId, usize), BufferView>,
    pub view_tree: ViewTree,
    pub events: EventBus,
    pub commands: CommandRegistry,
    pub keymaps: KeymapManager,
    pub fs: Box<dyn FileSystem>,
    pub running: bool,
    pub editor_mode: EditorMode,
    pub selection: Option<Selection>,
    pub plugin_state: std::collections::HashMap<String, String>,
    pub next_buffer_id: u64,
    pub next_window_id: u64,
    /// Named registers (vim `"a`…); the unnamed register lives in `editing`.
    pub registers: std::collections::HashMap<String, String>,
    pub marks: std::collections::HashMap<char, usize>,
    pub command_history: Vec<String>,
    /// Search dialog / repeat-search / grep pipeline state (R-24).
    pub search: SearchState,
    /// Yank/put, macro, change-list, and last-motion state (R-24).
    pub editing: EditingState,
    pub clipboard: Clipboard,
    pub alternate_buffer: Option<usize>,
    pub background: Option<BackgroundHandle>,
    pub options: std::collections::HashMap<String, String>,
    pub theme: std::collections::HashMap<String, (u8, u8, u8)>,
    pub quickfix_list: Vec<crate::task::QuickfixEntry>,
    pub quickfix_index: usize,
    /// Which list `:cn`/`:cp` walk: mirrored from the focused tool window by
    /// the GUI (commands only see the editor).  Falls back to build problems.
    pub quickfix_focus: crate::task::QuickfixList,
    pub vc: crate::vc::VcState,
    pub faces: std::collections::HashMap<String, crate::render::surface::Style>,
    pub scope_faces: Vec<(String, String)>,
    pub cursor_shape: String,
    /// Pointer-hover code info card (GUI only; None while hidden).
    pub hover: Option<HoverCard>,
    /// Invalidation flag for the GUI: every mutating entry point (key
    /// dispatch, background event, mouse, chrome action) sets this, and the
    /// frame that observes it repaints and clears it.  Replaces polling all
    /// buffers for a render signature (rework.md §7).
    pub frame_dirty: bool,
    /// Incremental tree-sitter parse trees, keyed by buffer slab
    /// (docs/core-redesign.md §7.2).  Each state records the document
    /// revision its tree was parsed at.
    pub ts_trees: std::collections::HashMap<usize, crate::semantic::ts_registry::TsTreeState>,
    pub ts_languages: std::collections::HashMap<usize, String>,
    pub saved_layouts:
        std::collections::HashMap<String, (Vec<crate::render::view_tree::Pane>, usize)>,
    pub mark_ring: Vec<(String, usize)>,
    pub module_paths: Vec<String>,
    pub font_changed: bool,
    pub font_config: FontConfig,
    pub overlays: Vec<Overlay>,
    pub next_overlay_id: usize,
    pub last_layout: LayoutSnapshot,
    pub mouse_state: MouseState,
    pub on_input_fn: Option<String>,
    pub input_consumed: bool,
    pub status_message: Option<StatusMessage>,
    /// Snapshot of the most recently finished task: (name, combined output).
    /// The output panel renders this; `quickfix_list` carries the errors.
    pub last_task: Option<(String, String)>,
    /// Quick-fix picker over server code actions (None while closed).
    pub quickfix_menu: Option<crate::state::overlay::QuickfixMenu>,

    // ── Sub-structs ─────────────────────────────────────────────────
    pub snippet: SnippetState,
    pub multi_cursor: MultiCursorState,
    pub completion: CompletionState,
    pub project_manager: ProjectManager,
    pub io: IoState,
    pub gutter: GutterRegistry,
    pub semantic: SemanticEngine,
    pub language_registry: LanguageRegistry,
    pub lsp: crate::semantic::lsp::LspRegistry,
    pub index: crate::index::IndexStore,
    /// Active file-icon pack; the Janet-installed default is snapshotted
    /// here so the `icon-pack` option can switch back to it.
    pub icons: crate::icons::IconPack,
    pub default_icons: crate::icons::IconPack,
    /// Minimap sampled-line caches keyed by buffer slab key (rework.md §7).
    pub minimap_cache: std::collections::HashMap<usize, crate::state::minimap::MinimapCache>,
    /// Per-row grapheme→cell layouts, keyed by (buffer, line start, tab
    /// width) and versioned by buffer revision (R-12).
    pub row_layouts: crate::render::row_layout::RowLayoutCache,
    pub task_scheduler: TaskScheduler,
    pub debug: DebugManager,
    pub scheduler: WorkScheduler,
    pub extension_registry: ExtensionRegistry,
    pub agent: AgentManager,
    pub db: crate::db::DbState,
    pub runtime: Option<Box<dyn ScriptRuntime>>,
}

impl Editor {
    pub fn new(fs: Box<dyn FileSystem>) -> Self {
        let mut theme = std::collections::HashMap::new();
        theme.insert("bg".into(), (30, 30, 46));
        theme.insert("fg".into(), (205, 214, 244));
        theme.insert("line-num".into(), (108, 112, 134));
        theme.insert("line-num-bg".into(), (30, 30, 46));
        theme.insert("line-num-current".into(), (205, 214, 244));
        theme.insert("status-bg".into(), (24, 24, 37));
        theme.insert("status-fg".into(), (205, 214, 244));
        theme.insert("diag-error".into(), (243, 139, 168));
        theme.insert("diag-warn".into(), (250, 179, 135));
        theme.insert("highlight-bg".into(), (49, 50, 68));
        theme.insert("highlight-fg".into(), (203, 166, 247));
        theme.insert("selection-bg".into(), (137, 180, 250));
        theme.insert("selection-fg".into(), (30, 30, 46));
        theme.insert("gui-bg".into(), (30, 30, 46));
        theme.insert("terminal-bg".into(), (0, 0, 0));
        theme.insert("terminal-fg".into(), (0, 255, 0));
        theme.insert("diff-add".into(), (166, 227, 161));
        theme.insert("diff-del".into(), (243, 139, 168));
        theme.insert("diff-hunk".into(), (137, 180, 250));

        Editor {
            buffers: slab::Slab::new(),
            views: std::collections::HashMap::new(),
            window_views: std::collections::HashMap::new(),
            view_tree: ViewTree::new(),
            events: EventBus::new(),
            commands: CommandRegistry::new(),
            keymaps: KeymapManager::new(),
            fs,
            running: true,
            editor_mode: EditorMode::default(),
            selection: None,
            plugin_state: std::collections::HashMap::new(),
            next_buffer_id: 1,
            next_window_id: 1,
            registers: std::collections::HashMap::new(),
            marks: std::collections::HashMap::new(),
            command_history: Vec::new(),
            search: crate::state::search::SearchState {
                pattern: None,
                forward: true,
                ..Default::default()
            },
            editing: EditingState::default(),
            clipboard: Clipboard::new(),
            alternate_buffer: None,
            background: None,
            options: std::collections::HashMap::new(),
            theme,
            quickfix_list: Vec::new(),
            quickfix_index: 0,
            quickfix_focus: crate::task::QuickfixList::Problems,
            vc: crate::vc::VcState::new(),
            faces: std::collections::HashMap::new(),
            scope_faces: Vec::new(),
            cursor_shape: "beam".to_string(),
            hover: None,
            frame_dirty: true,
            ts_trees: std::collections::HashMap::new(),
            ts_languages: std::collections::HashMap::new(),
            saved_layouts: std::collections::HashMap::new(),
            mark_ring: Vec::new(),
            font_changed: false,
            font_config: FontConfig::default(),
            overlays: Vec::new(),
            next_overlay_id: 1,
            last_layout: LayoutSnapshot::default(),
            mouse_state: MouseState::default(),
            on_input_fn: None,
            input_consumed: false,
            status_message: None,
            last_task: None,
            quickfix_menu: None,
            module_paths: {
                let home = std::env::var("HOME").unwrap_or_default();
                if home.is_empty() {
                    vec![]
                } else {
                    vec![format!("{}/.config/magma/plugins", home)]
                }
            },
            runtime: None,
            snippet: SnippetState::default(),
            multi_cursor: MultiCursorState::default(),
            completion: CompletionState::default(),
            project_manager: ProjectManager::default(),
            io: IoState::default(),
            gutter: GutterRegistry::default(),
            semantic: SemanticEngine::new(),
            language_registry: LanguageRegistry::new(),
            lsp: crate::semantic::lsp::LspRegistry::default(),
            index: crate::index::IndexStore::default(),
            icons: crate::icons::IconPack::empty("default"),
            default_icons: crate::icons::IconPack::empty("default"),
            minimap_cache: std::collections::HashMap::new(),
            row_layouts: crate::render::row_layout::RowLayoutCache::default(),
            task_scheduler: TaskScheduler::new(),
            debug: DebugManager::new(),
            scheduler: WorkScheduler::new(),
            extension_registry: ExtensionRegistry::new(),
            agent: AgentManager::new(),
            db: Default::default(),
        }
    }

    /// Create a new empty buffer + view, insert both, return slab key.
    pub fn create_buffer(&mut self, name: &str) -> usize {
        let id = self.allocate_buffer_id();
        let buf = crate::text_engine::Buffer::new(crate::state::id::BufferId(id), name);
        let arc = Arc::new(Mutex::new(buf));
        let key = self.buffers.insert(arc.clone());
        self.views
            .insert(key, crate::text_engine::BufferView::new(arc));
        key
    }

    /// Create a new buffer + view with initial content.
    pub fn create_buffer_from_str(&mut self, name: &str, content: &str) -> usize {
        let id = self.allocate_buffer_id();
        let buf =
            crate::text_engine::Buffer::from_string(crate::state::id::BufferId(id), name, content);
        let arc = Arc::new(Mutex::new(buf));
        let key = self.buffers.insert(arc.clone());
        self.views
            .insert(key, crate::text_engine::BufferView::new(arc));
        key
    }

    /// Get the focused buffer's slab key (0 if none).
    pub fn focused_buffer_key(&self) -> usize {
        self.view_tree
            .focused_window()
            .and_then(|wid| self.view_tree.buffer(wid))
            .unwrap_or(0)
    }

    /// Look up a theme color by key, falling back to a hardcoded default.
    pub fn theme_color(&self, key: &str) -> (u8, u8, u8) {
        self.theme.get(key).copied().unwrap_or(match key {
            "bg" => (30, 30, 46),
            "fg" => (205, 214, 244),
            "line-num" => (108, 112, 134),
            "line-num-bg" => (30, 30, 46),
            "line-num-current" => (205, 214, 244),
            "status-bg" => (24, 24, 37),
            "status-fg" => (205, 214, 244),
            "diag-error" => (243, 139, 168),
            "diag-warn" => (250, 179, 135),
            "highlight-bg" => (49, 50, 68),
            "highlight-fg" => (203, 166, 247),
            "exec-line-bg" => (68, 84, 106),
            "exec-line-fg" => (137, 180, 250),
            "selection-bg" => (137, 180, 250),
            "selection-fg" => (30, 30, 46),
            "gui-bg" => (30, 30, 46),
            "cursor" => (180, 190, 254),
            "diff-add" => (166, 227, 161),
            "diff-del" => (243, 139, 168),
            "diff-hunk" => (137, 180, 250),
            _ => (200, 200, 200),
        })
    }

    pub fn mode_name(&self) -> String {
        self.editor_mode.name.to_ascii_uppercase()
    }

    pub fn selection_range(&self, cursor: usize) -> Option<(usize, usize)> {
        let sel = self.selection.as_ref()?;
        if sel.is_line() {
            let buf_id = crate::input::focused_buffer_id(self);
            if let Some(arc) = self.buffers.get(buf_id) {
                let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
                let a_line = sel.anchor;
                let c_line = buf.slice(0, cursor).chars().filter(|&c| c == '\n').count();
                let (first, last) = (a_line.min(c_line), a_line.max(c_line));
                let start = buf.line_start_offset(first).unwrap_or(0);
                let end = buf
                    .line_start_offset(last)
                    .map(|o| {
                        let text = buf.slice(o, buf.len());
                        let line_text = text.split('\n').next().unwrap_or("");
                        o + line_text.len() + 1
                    })
                    .unwrap_or(buf.len());
                return Some((start, end.min(buf.len())));
            }
        }
        Some((sel.anchor.min(cursor), sel.anchor.max(cursor) + 1))
    }

    pub fn allocate_buffer_id(&mut self) -> u64 {
        let id = self.next_buffer_id;
        self.next_buffer_id += 1;
        id
    }

    pub fn allocate_window_id(&mut self) -> u64 {
        let id = self.next_window_id;
        self.next_window_id += 1;
        id
    }

    pub fn resolve_face_style(&self, face: &str) -> Option<crate::render::surface::Style> {
        self.faces.get(face).copied()
    }

    /// Names of buffers quit must protect: modified and writable.  Read-only
    /// buffers hold generated content (shell/grep output), not user work.
    pub fn dirty_buffer_names(&self) -> Vec<String> {
        self.buffers
            .iter()
            .filter_map(|(key, arc)| {
                let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
                let saving = self.io.saves_in_flight.contains_key(&key);
                ((buf.modified() || saving) && !buf.read_only).then(|| buf.name.clone())
            })
            .collect()
    }

    /// Surface a transient status message.  This is the single sink for
    /// user-facing feedback — command failures land here instead of being
    /// discarded.
    pub fn show_message(&mut self, kind: MessageKind, text: impl Into<String>) {
        self.status_message = Some(StatusMessage {
            kind,
            text: text.into(),
            at: std::time::Instant::now(),
        });
    }
}
