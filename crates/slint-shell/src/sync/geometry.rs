//! Refresh plumbing shared with `main.rs`: the model re-projection gate,
//! the editor grid sizing, and the pane revision signature.

use slint::{ModelRc, VecModel};

use kernel::state::Editor;

use crate::{AppWindow, logical_size};

/// Re-projects a model-valued property only when the row data actually
/// changed: replacing the model every refresh dirties Slint's scene graph
/// and forces a full re-layout even when nothing moved.
pub(crate) struct ModelGate<T> {
    last: Vec<T>,
}

impl<T: PartialEq + Clone + 'static> ModelGate<T> {
    pub(crate) fn new() -> Self {
        ModelGate { last: Vec::new() }
    }

    pub(crate) fn set(&mut self, next: Vec<T>, set: impl FnOnce(ModelRc<T>)) {
        if self.last == next {
            return;
        }
        self.last = next.clone();
        set(ModelRc::new(VecModel::from(next)));
    }
}

/// Editor grid from the actual laid-out pane, so the image, the surface, and
/// click hit-testing all agree with what is on screen.  Before the first
/// layout pass the element has no size yet; fall back to the window size
/// minus the fixed chrome (menu, tabs, breadcrumb, bottom bar, status).
pub(crate) fn editor_grid(app: &AppWindow, cell_w: u32, cell_h: u32) -> (u32, u32) {
    let mut w = app.get_editor_view_width() as f32;
    let mut h = app.get_editor_view_height() as f32;
    // Before the first real layout pass the view reports its minimum size.
    // Deriving the viewport scroll from that postage stamp sticks once the
    // pane reaches its real size — the cursor-follow rules only guarantee
    // the cursor stays inside, never that the view re-centers — so treat
    // unbelievable sizes the same as "no size yet".
    if w < 200.0 || h < 100.0 {
        let (mut lw, mut lh) = logical_size(app);
        // A not-yet-mapped window reports a stub size; sizing the surface
        // from it would freeze the viewport scroll at a postage stamp.
        // The window's preferred size is the honest guess until the real
        // layout lands.
        if lw < 300.0 || lh < 200.0 {
            lw = 1280.0;
            lh = 800.0;
        }
        w = (lw - 304.0).max(80.0);
        h = (lh - 132.0).max(40.0);
    }
    (
        (w / cell_w.max(1) as f32).floor().max(1.0) as u32,
        (h / cell_h.max(1) as f32).floor().max(1.0) as u32,
    )
}

pub(crate) fn revision_of(editor: &Editor) -> u64 {
    editor
        .buffers
        .iter()
        .map(|(_, arc)| arc.lock().unwrap_or_else(|e| e.into_inner()).revision())
        .sum()
}

