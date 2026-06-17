//! Plugin Capability System (Phase 10).
//!
//! Extensions declare required capabilities in an `ExtensionManifest`.
//! The `ExtensionRegistry` stores manifests and enforces them when Janet C
//! functions are called while a named extension is the active context.

use std::collections::HashMap;

/// Permissions an extension may request.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Capability {
    /// Read and write the local filesystem (`fs/*` functions).
    Filesystem,
    /// Send HTTP requests and open TCP connections (`net/*` functions).
    Network,
    /// Spawn external processes (`process/spawn`, `task/spawn` shell commands).
    ProcessExecution,
    /// Read and modify editor state (buffers, cursor, commands, events, …).
    EditorAccess,
}

impl Capability {
    pub fn as_str(&self) -> &'static str {
        match self {
            Capability::Filesystem       => "filesystem",
            Capability::Network          => "network",
            Capability::ProcessExecution => "process-execution",
            Capability::EditorAccess     => "editor-access",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "filesystem"        => Some(Capability::Filesystem),
            "network"           => Some(Capability::Network),
            "process-execution" => Some(Capability::ProcessExecution),
            "editor-access"     => Some(Capability::EditorAccess),
            _ => None,
        }
    }
}

/// A registered extension's metadata and declared capabilities.
#[derive(Debug, Clone)]
pub struct ExtensionManifest {
    pub name: String,
    pub version: String,
    pub capabilities: Vec<Capability>,
    pub entry_point: String,
}

impl ExtensionManifest {
    pub fn new(
        name: impl Into<String>,
        version: impl Into<String>,
        capabilities: Vec<Capability>,
        entry_point: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            capabilities,
            entry_point: entry_point.into(),
        }
    }

    pub fn has_capability(&self, cap: &Capability) -> bool {
        self.capabilities.contains(cap)
    }
}

/// Tracks all registered extensions and their declared capabilities.
///
/// The registry is the authority for what any named extension is allowed to
/// do.  When a Janet C function requires a capability, it calls
/// `scripting::extension_api::check_capability`, which reads the current
/// extension name from a thread-local and consults this registry.
#[derive(Default)]
pub struct ExtensionRegistry {
    extensions: HashMap<String, ExtensionManifest>,
}

impl ExtensionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a manifest, replacing any previous registration for the same name.
    pub fn register(&mut self, manifest: ExtensionManifest) {
        self.extensions.insert(manifest.name.clone(), manifest);
    }

    /// Unregister an extension by name.
    pub fn unregister(&mut self, name: &str) {
        self.extensions.remove(name);
    }

    /// Return a reference to the manifest for `name`, if registered.
    pub fn get(&self, name: &str) -> Option<&ExtensionManifest> {
        self.extensions.get(name)
    }

    /// Return all registered extension manifests as a `Vec`.
    pub fn list(&self) -> Vec<&ExtensionManifest> {
        let mut v: Vec<_> = self.extensions.values().collect();
        v.sort_by(|a, b| a.name.cmp(&b.name));
        v
    }

    /// Return true if extension `name` has declared `capability`.
    ///
    /// Returns `false` for unregistered names so that extensions that declare
    /// themselves but forget to call `extension/declare` before using a
    /// privileged API are denied.  Core code (no active extension) always
    /// passes — the caller (`check_capability`) handles the `None` case.
    pub fn has_capability(&self, name: &str, capability: &Capability) -> bool {
        self.extensions
            .get(name)
            .map_or(false, |m| m.has_capability(capability))
    }

    pub fn count(&self) -> usize {
        self.extensions.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_registry_is_empty() {
        assert_eq!(ExtensionRegistry::new().count(), 0);
    }

    #[test]
    fn register_and_get() {
        let mut reg = ExtensionRegistry::new();
        reg.register(ExtensionManifest::new(
            "test-ext", "1.0",
            vec![Capability::Filesystem],
            "test.janet",
        ));
        assert!(reg.get("test-ext").is_some());
    }

    #[test]
    fn has_capability_true_when_declared() {
        let mut reg = ExtensionRegistry::new();
        reg.register(ExtensionManifest::new(
            "ext", "1.0", vec![Capability::Network], "e.janet",
        ));
        assert!(reg.has_capability("ext", &Capability::Network));
        assert!(!reg.has_capability("ext", &Capability::Filesystem));
    }

    #[test]
    fn has_capability_false_for_unknown() {
        let reg = ExtensionRegistry::new();
        assert!(!reg.has_capability("unknown", &Capability::EditorAccess));
    }

    #[test]
    fn capability_round_trip() {
        for cap in [
            Capability::Filesystem,
            Capability::Network,
            Capability::ProcessExecution,
            Capability::EditorAccess,
        ] {
            let s = cap.as_str();
            assert_eq!(Capability::from_str(s), Some(cap));
        }
    }
}
