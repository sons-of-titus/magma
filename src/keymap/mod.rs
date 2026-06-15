/// Layered keymap system.
/// Resolution order: buffer-local → active mode layers (LIFO) → global.
///
/// # Layer lifecycle
/// Layers live in a registry (`layers`) and are never deleted — their bindings
/// survive push/pop cycles.  `push_layer` activates a layer; `pop_layer`
/// deactivates it.  This means you can pre-populate an "insert" layer's
/// bindings before the first `push_layer("insert")` call, and those bindings
/// will still be present when the layer is re-activated later.
use std::collections::HashMap;

pub type KeySequence  = String;
pub type CommandName  = String;

#[derive(Debug, Clone)]
struct KeymapLayer {
    bindings: HashMap<KeySequence, CommandName>,
}

#[derive(Debug)]
pub struct KeymapManager {
    /// Global fallback bindings (lowest priority).
    global: KeymapLayer,
    /// Registry of all known layers.  Entries are never removed so bindings
    /// survive deactivation.
    layers: HashMap<String, KeymapLayer>,
    /// Currently active layer names, in push order.  Resolution iterates this
    /// in reverse (last-pushed = highest priority).
    active: Vec<String>,
    /// Per-buffer bindings (highest priority of all).
    buffer_local: HashMap<u64, KeymapLayer>,
}

impl Default for KeymapManager {
    fn default() -> Self {
        Self::new()
    }
}

impl KeymapManager {
    pub fn new() -> Self {
        KeymapManager {
            global: KeymapLayer { bindings: HashMap::new() },
            layers: HashMap::new(),
            active: Vec::new(),
            buffer_local: HashMap::new(),
        }
    }

    // ── Binding setters ───────────────────────────────────────────────────

    /// Bind `key` → `command` in the global layer.
    pub fn set(&mut self, key: &str, command: &str) {
        self.global.bindings.insert(key.into(), command.into());
    }

    /// Bind `key` → `command` in the named layer (creates it if absent).
    /// Passing `"global"` is equivalent to `set`.
    pub fn set_layer(&mut self, layer: &str, key: &str, command: &str) {
        if layer == "global" {
            self.set(key, command);
            return;
        }
        self.layers
            .entry(layer.to_string())
            .or_insert_with(|| KeymapLayer { bindings: HashMap::new() })
            .bindings
            .insert(key.into(), command.into());
    }

    pub fn set_buffer_local(&mut self, buffer_id: u64, key: &str, command: &str) {
        self.buffer_local
            .entry(buffer_id)
            .or_insert_with(|| KeymapLayer { bindings: HashMap::new() })
            .bindings
            .insert(key.into(), command.into());
    }

    pub fn unset(&mut self, key: &str) {
        self.global.bindings.remove(key);
    }

    pub fn unset_layer(&mut self, layer: &str, key: &str) {
        if layer == "global" {
            self.unset(key);
        } else if let Some(l) = self.layers.get_mut(layer) {
            l.bindings.remove(key);
        }
    }

    // ── Layer activation ──────────────────────────────────────────────────

    /// Activate `layer` (add to the active stack).  If the layer is already
    /// active this is a no-op.  Creates the layer in the registry if absent.
    pub fn push_layer(&mut self, layer: &str) {
        self.layers
            .entry(layer.to_string())
            .or_insert_with(|| KeymapLayer { bindings: HashMap::new() });
        if !self.active.contains(&layer.to_string()) {
            self.active.push(layer.to_string());
        }
    }

    /// Deactivate `layer` without removing its bindings from the registry.
    pub fn pop_layer(&mut self, layer: &str) {
        self.active.retain(|n| n != layer);
    }

    /// Returns `true` if `layer` is currently active.
    pub fn is_layer_active(&self, layer: &str) -> bool {
        self.active.iter().any(|n| n == layer)
    }

    // ── Resolution ────────────────────────────────────────────────────────

    pub fn resolve(&self, key: &str) -> Option<CommandName> {
        self.resolve_for_buffer(key, None)
    }

    /// Resolve `key` with full priority chain:
    /// buffer-local > active layers (LIFO) > global.
    pub fn resolve_for_buffer(&self, key: &str, buffer_id: Option<u64>) -> Option<CommandName> {
        if let Some(bid) = buffer_id
            && let Some(layer) = self.buffer_local.get(&bid)
                && let Some(cmd) = layer.bindings.get(key) {
                    return Some(cmd.clone());
                }

        for name in self.active.iter().rev() {
            if let Some(layer) = self.layers.get(name)
                && let Some(cmd) = layer.bindings.get(key) {
                    return Some(cmd.clone());
                }
        }

        self.global.bindings.get(key).cloned()
    }

    // ── Introspection ─────────────────────────────────────────────────────

    pub fn list_layer(&self, layer: &str) -> Vec<(String, String)> {
        if layer == "global" {
            self.global.bindings.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
        } else if let Some(l) = self.layers.get(layer) {
            l.bindings.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
        } else {
            Vec::new()
        }
    }

    pub fn list_all(&self) -> HashMap<String, Vec<(String, String)>> {
        let mut result = HashMap::new();
        result.insert("global".into(), self.list_layer("global"));
        for name in self.layers.keys() {
            result.insert(name.clone(), self.list_layer(name));
        }
        result
    }

    pub fn active_layers(&self) -> Vec<String> {
        let mut out = vec!["global".into()];
        out.extend(self.active.iter().cloned());
        out
    }

    pub fn describe(&self, key: &str) -> Option<CommandName> {
        self.resolve(key)
    }
}



