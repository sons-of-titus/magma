//! Debug session → Debug panel models.
//!
//! Pure projection: one pass over the active session per refresh.  The
//! expanded-variables set lives in the shell (session UI state), keyed by DAP
//! `variablesReference`; everything else reads live from the session model.

use kernel::state::Editor;

use crate::{ConsoleRow, DebugBpRow, DebugFrameRow, DebugVarRow, DebugWatchRow};

/// How deep the flattened variables tree may go — adapters happily recurse
/// through container types; the panel shows the first few levels and the
/// user expands the rest deliberately.
const MAX_VAR_DEPTH: u32 = 6;

pub enum Phase {
    NoSession,
    Starting,
    Running,
    Paused,
}

impl Phase {
    pub fn as_str(&self) -> &'static str {
        match self {
            Phase::NoSession => "none",
            Phase::Starting => "starting",
            Phase::Running => "running",
            Phase::Paused => "paused",
        }
    }
}

pub struct DebugSnapshot {
    pub phase: Phase,
    /// "Paused · main.rs:12" / "Running" / "Starting…" / "" (no session).
    pub status_text: String,
    pub adapter: String,
    /// True when a live adapter connection backs the session.
    pub connected: bool,
    pub frames: Vec<DebugFrameRow>,
    pub variables: Vec<DebugVarRow>,
    /// Slot table: row `var_ref`/`frame_id` values index into this to get
    /// the real 64-bit DAP id.
    pub ref_slots: Vec<i64>,
    pub watches: Vec<DebugWatchRow>,
    pub breakpoints: Vec<DebugBpRow>,
    pub console: Vec<ConsoleRow>,
    /// Console line count before the tail cut — drives the "earlier output
    /// trimmed" note when the cap hits.
    pub console_total: usize,
}

impl DebugSnapshot {
    /// Which execution actions are valid right now.
    pub fn can(&self) -> Can {
        let paused = matches!(self.phase, Phase::Paused);
        let running = matches!(self.phase, Phase::Running);
        Can {
            resume: self.connected && paused,
            pause: self.connected && running,
            step: self.connected && paused,
            stop: self.connected,
            restart: self.connected,
        }
    }
}

/// Which execution actions are valid right now.
pub struct Can {
    pub resume: bool,
    pub pause: bool,
    pub step: bool,
    pub stop: bool,
    pub restart: bool,
}

pub fn snapshot(ed: &Editor, expanded: &std::collections::HashSet<i64>) -> DebugSnapshot {
    let Some(session) = ed.debug.active_session() else {
        return DebugSnapshot {
            phase: Phase::NoSession,
            status_text: String::new(),
            adapter: String::new(),
            connected: false,
            frames: Vec::new(),
            variables: Vec::new(),
            ref_slots: Vec::new(),
            watches: Vec::new(),
            breakpoints: all_breakpoints(ed),
            console: Vec::new(),
            console_total: 0,
        };
    };

    let connected = ed.debug.connections.contains_key(&session.id);
    let phase = if !connected {
        Phase::Starting
    } else if session.stopped {
        Phase::Paused
    } else {
        Phase::Running
    };
    let status_text = match phase {
        Phase::Starting => "Starting…".to_string(),
        Phase::Running => "Running".to_string(),
        Phase::Paused => match &session.stopped_line {
            Some((path, line)) => {
                format!("Paused \u{2009}\u{b7}\u{2009}{}:{line}", short_name(path))
            }
            None => "Paused".to_string(),
        },
        Phase::NoSession => String::new(),
    };

    let selected = session.current_frame().map(|f| f.id);
    // Slint integers are i32 while DAP ids routinely exceed that, so rows
    // carry a slot index into `ref_slots`, rebuilt on every sync.
    let mut ref_slots: Vec<i64> = Vec::new();
    let frames = session
        .frames
        .iter()
        .map(|f| {
            let slot = ref_slots.len() as i32;
            ref_slots.push(f.id as i64);
            DebugFrameRow {
                name: f.name.clone().into(),
                location: f
                    .source
                    .as_deref()
                    .map(short_name)
                    .unwrap_or_default()
                    .into(),
                line_no: f.line as i32,
                frame_id: slot,
                is_top: Some(f.id) == session.frames.first().map(|t| t.id),
                is_selected: selected == Some(f.id),
                has_location: f.source.is_some(),
            }
        })
        .collect();

    let mut variables = Vec::new();
    if let Some(r) = session.scope_ref {
        let open = expanded.contains(&r);
        let label = session.scope_name.clone().unwrap_or_else(|| "Scope".into());
        let count = session
            .variable_scopes
            .get(&r)
            .map(|v| v.len())
            .unwrap_or(0);
        let slot = ref_slots.len() as i32;
        ref_slots.push(r);
        variables.push(DebugVarRow {
            depth: 0,
            name: label.into(),
            value: "".into(),
            type_name: "".into(),
            var_ref: slot,
            has_children: true,
            is_open: open,
            is_scope: true,
            count: count as i32,
        });
        if open && let Some(children) = session.variable_scopes.get(&r) {
            let owned: Vec<_> = children.clone();
            flatten_vars(
                &mut variables,
                &owned,
                &session.variable_scopes,
                1,
                expanded,
                &mut ref_slots,
            );
        }
    }

    let watches = ed
        .debug
        .watches
        .iter()
        .enumerate()
        .map(|(i, expr)| {
            let raw = session
                .watch_results
                .get(i)
                .and_then(|r| r.clone())
                .unwrap_or_default();
            // A leading '!' marks an evaluation failure; the row shows the
            // message without the marker.
            let (result, state) = if let Some(msg) = raw.strip_prefix('!') {
                (msg.to_string(), "error")
            } else if session.watch_results.get(i).is_some() {
                (raw, "ok")
            } else {
                (String::new(), "pending")
            };
            DebugWatchRow {
                expr: expr.clone().into(),
                result: result.into(),
                state: state.into(),
            }
        })
        .collect();

    let console_total = session.console.len();
    let start = console_total.saturating_sub(200);
    let console = session.console[start..]
        .iter()
        .map(|line| ConsoleRow {
            text: line.clone().into(),
            kind: if line.starts_with("! ") {
                "error".into()
            } else if line.starts_with("= ") {
                "result".into()
            } else {
                "out".into()
            },
        })
        .collect();

    DebugSnapshot {
        phase,
        status_text,
        adapter: session.adapter.clone(),
        connected,
        frames,
        variables,
        ref_slots,
        watches,
        breakpoints: all_breakpoints(ed),
        console,
        console_total,
    }
}

fn flatten_vars(
    out: &mut Vec<DebugVarRow>,
    vars: &[kernel::debug::Variable],
    scopes: &std::collections::HashMap<i64, Vec<kernel::debug::Variable>>,
    depth: u32,
    expanded: &std::collections::HashSet<i64>,
    ref_slots: &mut Vec<i64>,
) {
    if depth > MAX_VAR_DEPTH {
        return;
    }
    for v in vars {
        let has_children = v.variables_reference != 0;
        let is_open = has_children && expanded.contains(&v.variables_reference);
        let slot = ref_slots.len() as i32;
        ref_slots.push(v.variables_reference);
        out.push(DebugVarRow {
            depth: depth as i32,
            name: v.name.clone().into(),
            value: v.value.clone().into(),
            type_name: v.type_name.clone().unwrap_or_default().into(),
            var_ref: slot,
            has_children,
            is_open,
            is_scope: false,
            count: scopes
                .get(&v.variables_reference)
                .map(|c| c.len() as i32)
                .unwrap_or(0),
        });
        if is_open && let Some(children) = scopes.get(&v.variables_reference) {
            let owned: Vec<_> = children.clone();
            flatten_vars(out, &owned, scopes, depth + 1, expanded, ref_slots);
        }
    }
}

fn all_breakpoints(ed: &Editor) -> Vec<DebugBpRow> {
    ed.debug
        .all_breakpoints()
        .into_iter()
        .map(|bp| DebugBpRow {
            file: bp.file.clone().into(),
            file_label: short_name(&bp.file).into(),
            line_no: bp.line as i32,
            enabled: bp.enabled,
            verified: bp.verified,
            condition: bp.condition.clone().unwrap_or_default().into(),
        })
        .collect()
}

fn short_name(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_string()
}
