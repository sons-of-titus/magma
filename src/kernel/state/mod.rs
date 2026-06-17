//! Editor state — the central `Editor` struct and all sub-modules.

pub mod id;
pub mod mode;
pub mod persist;
pub mod font;
pub mod snippet;
pub mod cursor;
pub mod visual;
pub mod completion;
pub mod io;
pub mod gutter;
pub mod overlay;

use std::sync::{Arc, Mutex, RwLock};

use crate::kernel::text_engine::{Buffer, BufferView};
use crate::kernel::clipboard::Clipboard;
use crate::kernel::command::CommandRegistry;
use crate::kernel::event::EventBus;
use crate::kernel::storage::FileSystem;
use crate::kernel::keymap::KeymapManager;
use crate::kernel::runtime::BackgroundHandle;
use crate::kernel::scripting::ScriptRuntime;
use crate::kernel::render::view_tree::ViewTree;
use crate::kernel::debug::DebugManager;
use crate::kernel::semantic::SemanticEngine;
use crate::kernel::task::TaskScheduler;

pub use mode::{EditorMode, Minibuffer, Selection, SearchDirection};
pub use font::{ContextFont, FontConfig};
pub use cursor::{ExtraCursor, MultiCursorState};
pub use visual::BlockVisualState;
pub use completion::CompletionState;
pub use snippet::SnippetState;
pub use crate::kernel::project::{
    Project, ProjectManager, ProjectMember, Workspace,
    Module, Dependency, DependencyKind, BuildKind, BuildTarget, Config,
};
pub use io::IoState;
pub use gutter::{FoldIcons, GutterCell, GutterRegistry};
pub use overlay::{Overlay, LayoutSnapshot};

/// The single source of truth for all editor state.
/// All access is guarded by `Arc<RwLock<Editor>>`.
pub struct Editor {
    pub buffers: slab::Slab<Arc<Mutex<Buffer>>>,
    /// Per-window view state (indexed by buffer slab key).
    pub views: std::collections::HashMap<usize, BufferView>,
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
    pub yanked_text: Option<String>,
    pub registers: std::collections::HashMap<String, String>,
    pub marks: std::collections::HashMap<char, usize>,
    pub last_insert_pos: usize,
    pub command_history: Vec<String>,
    pub last_find: Option<(char, bool, bool)>,
    pub search_pattern: Option<String>,
    pub search_forward: bool,
    pub recording_macro: Option<String>,
    pub macros: std::collections::HashMap<String, Vec<String>>,
    pub insert_pending_register: bool,
    pub clipboard: Clipboard,
    pub alternate_buffer: Option<usize>,
    pub last_visual: Option<(usize, usize, bool)>,
    pub background: Option<BackgroundHandle>,
    pub options: std::collections::HashMap<String, String>,
    pub change_list: Vec<usize>,
    pub change_list_idx: usize,
    pub theme: std::collections::HashMap<String, (u8, u8, u8)>,
    pub quickfix_list: Vec<crate::kernel::task::QuickfixEntry>,
    pub quickfix_index: usize,
    pub dired: crate::kernel::vc::dired::DiredState,
    pub vc: crate::kernel::vc::VcState,
    pub faces: std::collections::HashMap<String, crate::kernel::render::surface::Style>,
    pub scope_faces: Vec<(String, String)>,
    pub cursor_shape: String,
    pub ts_trees: std::collections::HashMap<usize, Vec<u8>>,
    pub ts_languages: std::collections::HashMap<usize, String>,
    pub saved_layouts: std::collections::HashMap<String, (Vec<crate::kernel::render::view_tree::Pane>, usize)>,
    pub mark_ring: Vec<(String, usize)>,
    pub module_paths: Vec<String>,
    pub font_changed: bool,
    pub font_config: FontConfig,
    pub modeline_fn: Option<String>,
    pub modeline_rendered: String,
    pub overlays: Vec<Overlay>,
    pub next_overlay_id: usize,
    pub tab_bar_enabled: bool,
    pub last_layout: LayoutSnapshot,
    pub on_input_fn: Option<String>,
    pub input_consumed: bool,

    // ── Sub-structs ─────────────────────────────────────────────────
    pub snippet: SnippetState,
    pub multi_cursor: MultiCursorState,
    pub block_visual: BlockVisualState,
    pub completion: CompletionState,
    pub project_manager: ProjectManager,
    pub io: IoState,
    pub gutter: GutterRegistry,
    pub semantic: SemanticEngine,
    pub task_scheduler: TaskScheduler,
    pub debug: DebugManager,
    pub runtime: Option<Box<dyn ScriptRuntime>>,
}

impl Editor {
    pub fn new(fs: Box<dyn FileSystem>) -> Self {
        let mut theme = std::collections::HashMap::new();
        theme.insert("bg".into(),               (30,  30,  46));
        theme.insert("fg".into(),               (205, 214, 244));
        theme.insert("line-num".into(),         (108, 112, 134));
        theme.insert("line-num-bg".into(),      (30,  30,  46));
        theme.insert("line-num-current".into(), (205, 214, 244));
        theme.insert("status-bg".into(),        (24,  24,  37));
        theme.insert("status-fg".into(),        (205, 214, 244));
        theme.insert("diag-error".into(),       (243, 139, 168));
        theme.insert("diag-warn".into(),        (250, 179, 135));
        theme.insert("highlight-bg".into(),     (49,  50,  68));
        theme.insert("highlight-fg".into(),     (203, 166, 247));
        theme.insert("selection-bg".into(),     (137, 180, 250));
        theme.insert("selection-fg".into(),     (30,  30,  46));
        theme.insert("gui-bg".into(),           (30,  30,  46));
        theme.insert("terminal-bg".into(),      (0,   0,   0));
        theme.insert("terminal-fg".into(),      (0,   255, 0));

        Editor {
            buffers: slab::Slab::new(),
            views: std::collections::HashMap::new(),
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
            yanked_text: None,
            registers: std::collections::HashMap::new(),
            marks: std::collections::HashMap::new(),
            last_insert_pos: 0,
            command_history: Vec::new(),
            last_find: None,
            search_pattern: None,
            search_forward: true,
            recording_macro: None,
            macros: std::collections::HashMap::new(),
            insert_pending_register: false,
            clipboard: Clipboard::new(),
            alternate_buffer: None,
            last_visual: None,
            background: None,
            options: std::collections::HashMap::new(),
            change_list: Vec::new(),
            change_list_idx: 0,
            theme,
            quickfix_list: Vec::new(),
            quickfix_index: 0,
            dired: crate::kernel::vc::dired::DiredState::new(),
            vc: crate::kernel::vc::VcState::new(),
            faces: std::collections::HashMap::new(),
            scope_faces: Vec::new(),
            cursor_shape: "block".to_string(),
            ts_trees: std::collections::HashMap::new(),
            ts_languages: std::collections::HashMap::new(),
            saved_layouts: std::collections::HashMap::new(),
            mark_ring: Vec::new(),
            font_changed: false,
            font_config: FontConfig::default(),
            modeline_fn: None,
            modeline_rendered: String::new(),
            overlays: Vec::new(),
            next_overlay_id: 1,
            tab_bar_enabled: false,
            last_layout: LayoutSnapshot::default(),
            on_input_fn: None,
            input_consumed: false,
            module_paths: {
                let home = std::env::var("HOME").unwrap_or_default();
                if home.is_empty() { vec![] }
                else { vec![format!("{}/.config/magma/plugins", home)] }
            },
            runtime: None,
            snippet: SnippetState::default(),
            multi_cursor: MultiCursorState::default(),
            block_visual: BlockVisualState::default(),
            completion: CompletionState::default(),
            project_manager: ProjectManager::default(),
            io: IoState::default(),
            gutter: GutterRegistry::default(),
            semantic: SemanticEngine::new(),
            task_scheduler: TaskScheduler::new(),
            debug: DebugManager::new(),
        }
    }

    /// Create a new empty buffer + view, insert both, return slab key.
    pub fn create_buffer(&mut self, name: &str) -> usize {
        let id = self.allocate_buffer_id();
        let buf = crate::kernel::text_engine::Buffer::new(crate::kernel::state::id::BufferId(id), name);
        let arc = Arc::new(Mutex::new(buf));
        let key = self.buffers.insert(arc.clone());
        self.views.insert(key, crate::kernel::text_engine::BufferView::new(arc));
        key
    }

    /// Create a new buffer + view with initial content.
    pub fn create_buffer_from_str(&mut self, name: &str, content: &str) -> usize {
        let id = self.allocate_buffer_id();
        let buf = crate::kernel::text_engine::Buffer::from_string(crate::kernel::state::id::BufferId(id), name, content);
        let arc = Arc::new(Mutex::new(buf));
        let key = self.buffers.insert(arc.clone());
        self.views.insert(key, crate::kernel::text_engine::BufferView::new(arc));
        key
    }

    /// Get the focused buffer's slab key (0 if none).
    pub fn focused_buffer_key(&self) -> usize {
        self.view_tree.focused_window()
            .and_then(|wid| self.view_tree.buffer(wid))
            .unwrap_or(0)
    }

    /// Get the focused window's view.
    pub fn focused_view(&self) -> Option<&crate::kernel::text_engine::BufferView> {
        let key = self.focused_buffer_key();
        self.views.get(&key)
    }

    /// Get the focused window's view (mutable).
    pub fn focused_view_mut(&mut self) -> Option<&mut crate::kernel::text_engine::BufferView> {
        let key = self.view_tree.focused_window()
            .and_then(|wid| self.view_tree.buffer(wid))?;
        self.views.get_mut(&key)
    }

    /// Look up a theme color by key, falling back to a hardcoded default.
    pub fn theme_color(&self, key: &str) -> (u8, u8, u8) {
        self.theme.get(key).copied().unwrap_or(match key {
            "bg"               => (30,  30,  46),
            "fg"               => (205, 214, 244),
            "line-num"         => (108, 112, 134),
            "line-num-bg"      => (30,  30,  46),
            "line-num-current" => (205, 214, 244),
            "status-bg"        => (24,  24,  37),
            "status-fg"        => (205, 214, 244),
            "diag-error"       => (243, 139, 168),
            "diag-warn"        => (250, 179, 135),
            "highlight-bg"     => (49,  50,  68),
            "highlight-fg"     => (203, 166, 247),
            "selection-bg"     => (137, 180, 250),
            "selection-fg"     => (30,  30,  46),
            "gui-bg"           => (30,  30,  46),
            _                  => (200, 200, 200),
        })
    }

    pub fn vim_mode_name(&self) -> String {
        self.editor_mode.name.to_ascii_uppercase()
    }

    pub fn selection_range(&self, cursor: usize) -> Option<(usize, usize)> {
        let sel = self.selection.as_ref()?;
        if sel.is_line() {
            let buf_id = crate::kernel::input::focused_buffer_id(self);
            if let Some(arc) = self.buffers.get(buf_id) {
                let buf = arc.lock().unwrap();
                let a_line = sel.anchor;
                let c_line = buf.slice(0, cursor).chars().filter(|&c| c == '\n').count();
                let (first, last) = (a_line.min(c_line), a_line.max(c_line));
                let start = buf.line_start_offset(first).unwrap_or(0);
                let end = buf.line_start_offset(last).map(|o| {
                    let text = buf.slice(o, buf.len());
                    let line_text = text.split('\n').next().unwrap_or("");
                    o + line_text.len() + 1
                }).unwrap_or(buf.len());
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

    pub fn resolve_face_style(&self, face: &str) -> Option<crate::kernel::render::surface::Style> {
        self.faces.get(face).copied()
    }
}

/// Thread-safe handle to the editor
pub type EditorHandle = Arc<RwLock<Editor>>;
