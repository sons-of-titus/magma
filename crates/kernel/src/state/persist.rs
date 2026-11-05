/// Persistent state — command history, marks, recent projects, and user
/// settings.  History/marks/projects live in the state dir (`state_dir()`),
/// user settings in the config dir (`config_dir()`); secrets go to the OS
/// keychain (`super::secrets`), never to disk.
use std::path::PathBuf;

/// Return the Magma state directory (~/.magma), creating it if needed.
pub fn state_dir() -> PathBuf {
    let dir = dirs_data_dir().join("magma");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn dirs_data_dir() -> PathBuf {
    // Follow XDG or fall back to ~/.magma
    if let Ok(dir) = std::env::var("MAGMA_DIR") {
        return PathBuf::from(dir);
    }
    xdg_data_home()
        .or_else(home_fallback)
        .unwrap_or_else(|| PathBuf::from("~/.magma"))
}

#[cfg(target_os = "macos")]
fn home_fallback() -> Option<PathBuf> {
    std::env::var("HOME")
        .ok()
        .map(|h| PathBuf::from(h).join("Library").join("Application Support"))
}

#[cfg(not(target_os = "macos"))]
fn home_fallback() -> Option<PathBuf> {
    std::env::var("HOME")
        .ok()
        .map(|h| PathBuf::from(h).join(".local").join("share"))
}

fn xdg_data_home() -> Option<PathBuf> {
    std::env::var("XDG_DATA_HOME").ok().map(PathBuf::from)
}

// ── Command-line history ─────────────────────────────────────────────

const HISTORY_FILE: &str = "history";

/// Load command history from `~/.magma/history`. Returns an empty vec on
/// first run or if the file is missing/corrupt.
pub fn load_command_history() -> Vec<String> {
    let path = state_dir().join(HISTORY_FILE);
    match std::fs::read_to_string(&path) {
        Ok(content) => content
            .lines()
            .map(|l| l.to_string())
            .filter(|l| !l.is_empty())
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Save command history to `~/.magma/history` (last 500 entries).
pub fn save_command_history(history: &[String]) {
    let path = state_dir().join(HISTORY_FILE);
    let max = history.len().min(500);
    let content = history[history.len() - max..].join("\n");
    let _ = std::fs::write(&path, &content);
}

// ── Marks persistence ────────────────────────────────────────────────

const MARKS_FILE: &str = "marks.json";

/// Persistent representation of a named mark.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PersistedMark {
    pub name: char,
    pub buffer_id: u64,
    pub byte_offset: usize,
}

/// Load persisted marks from `~/.magma/marks.json`.
pub fn load_marks() -> Vec<PersistedMark> {
    let path = state_dir().join(MARKS_FILE);
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

/// Save marks to `~/.magma/marks.json`.
pub fn save_marks(marks: &[PersistedMark]) {
    let path = state_dir().join(MARKS_FILE);
    if let Ok(content) = serde_json::to_string(marks) {
        let _ = std::fs::write(&path, &content);
    }
}

// ── Session state ────────────────────────────────────────────────────

/// Persist the editor's session state (command history + marks).
/// Called once when the GUI exits.
pub fn save_session(editor: &super::Editor) {
    save_command_history(&editor.command_history);
    let marks: Vec<PersistedMark> = editor
        .marks
        .iter()
        .map(|(&name, &byte_offset)| PersistedMark {
            name,
            buffer_id: 0,
            byte_offset,
        })
        .collect();
    save_marks(&marks);
}

// ── Recent projects ──────────────────────────────────────────────────

const RECENT_PROJECTS_FILE: &str = "projects";

/// Load recent project paths (MRU first) from the state directory.
pub fn load_recent_projects() -> Vec<String> {
    let path = state_dir().join(RECENT_PROJECTS_FILE);
    std::fs::read_to_string(&path)
        .map(|content| {
            content
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// Save recent project paths (MRU first) to the state directory.
pub fn save_recent_projects(projects: &[String]) {
    let path = state_dir().join(RECENT_PROJECTS_FILE);
    let _ = std::fs::write(&path, projects.join("\n"));
}

/// The most recently opened project that still exists on disk — the one to
/// reopen at startup, skipping any paths that have since been moved or deleted.
pub fn most_recent_existing_project(projects: &[String]) -> Option<String> {
    projects
        .iter()
        .find(|p| std::path::Path::new(p).is_dir())
        .cloned()
}

// ── Settings ─────────────────────────────────────────────────────────

const SETTINGS_FILE: &str = "settings.json";
/// Options under these prefixes carry secrets: they are stored in the OS
/// keychain, never in the settings file.
const SECRET_PREFIX: &str = "agent.api-key.";
const OAUTH_PREFIX: &str = "agent.oauth.";

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct PersistedSettings {
    #[serde(default)]
    options: std::collections::HashMap<String, String>,
    /// Non-secret manifest: which providers hold a keychain entry.
    #[serde(default)]
    keychain_keys: Vec<String>,
    /// Providers holding a stored OAuth token (device flows).
    #[serde(default)]
    keychain_oauth: Vec<String>,
    #[serde(default)]
    data_sources: Vec<crate::storage::workspace::PersistedDataSource>,
}

/// The user config directory (~/.config/magma), creating it if needed.
pub fn config_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("MAGMA_CONFIG_DIR") {
        let _ = std::fs::create_dir_all(&dir);
        return PathBuf::from(dir);
    }
    let home = std::env::var("HOME").unwrap_or_default();
    let dir = if home.is_empty() {
        PathBuf::from("~/.config/magma")
    } else {
        PathBuf::from(home).join(".config").join("magma")
    };
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Merge persisted settings into the editor's options and restore keychain
/// secrets.  Runs before the Janet user config so `init.janet` can override.
pub fn load_settings(ed: &mut super::Editor) {
    load_settings_from(&config_dir(), ed);
}

pub fn load_settings_from(dir: &std::path::Path, ed: &mut super::Editor) {
    let Ok(content) = std::fs::read_to_string(dir.join(SETTINGS_FILE)) else {
        return;
    };
    let Ok(settings) = serde_json::from_str::<PersistedSettings>(&content) else {
        return;
    };
    for (name, value) in settings.options {
        ed.options.insert(name, value);
    }
    for provider in settings.keychain_keys {
        let account = format!("{SECRET_PREFIX}{provider}");
        if let Some(secret) = super::secrets::load_key(&account) {
            ed.options.insert(account, secret);
        }
    }
    for provider in settings.keychain_oauth {
        let account = format!("{OAUTH_PREFIX}{provider}");
        if let Some(secret) = super::secrets::load_key(&account) {
            ed.options.insert(account, secret);
        }
    }
    // Restored credentials become connections; the model catalogs refresh
    // lazily once the Agent panel is visible.
    ed.agent_connections.sync_from_options(&ed.options);
    if !settings.data_sources.is_empty() {
        ed.db.sources = settings
            .data_sources
            .iter()
            .filter_map(|src| {
                let kind = db::DriverKind::parse(&src.kind)?;
                let host = if src.host.is_empty() {
                    src.path.clone().unwrap_or_default()
                } else {
                    src.host.clone()
                };
                Some(db::DataSource {
                    id: src.id,
                    name: src.name.clone(),
                    kind,
                    host,
                    port: if src.port == 0 {
                        kind.default_port()
                    } else {
                        src.port
                    },
                    database: src.database.clone(),
                    user: src.user.clone(),
                    ssl_mode: db::SslMode::parse(&src.ssl_mode).unwrap_or_default(),
                    trust_cert: src.trust_cert,
                    save_password: src.save_password,
                    url: src.url.clone(),
                })
            })
            .collect();
        ed.db.next_id_after_restore();
    }
}

/// Write current options to the settings file and sync API keys with the OS
/// keychain.
pub fn save_settings(ed: &super::Editor) {
    save_settings_to(&config_dir(), ed);
}

pub fn save_settings_to(dir: &std::path::Path, ed: &super::Editor) {
    let _ = std::fs::create_dir_all(dir);
    let mut file_options = std::collections::HashMap::new();
    let mut keychain_keys: Vec<String> = Vec::new();
    let mut keychain_oauth: Vec<String> = Vec::new();
    for (name, value) in &ed.options {
        if let Some(provider) = name.strip_prefix(SECRET_PREFIX) {
            if !value.is_empty() && super::secrets::save_key(name, value) {
                keychain_keys.push(provider.to_string());
            }
            continue;
        }
        if let Some(provider) = name.strip_prefix(OAUTH_PREFIX) {
            if !value.is_empty() && super::secrets::save_key(name, value) {
                keychain_oauth.push(provider.to_string());
            }
            continue;
        }
        file_options.insert(name.clone(), value.clone());
    }
    if let Ok(content) = std::fs::read_to_string(dir.join(SETTINGS_FILE))
        && let Ok(old) = serde_json::from_str::<PersistedSettings>(&content)
    {
        for provider in old.keychain_keys {
            if !keychain_keys.contains(&provider) {
                super::secrets::delete_key(&format!("{SECRET_PREFIX}{provider}"));
            }
        }
        for provider in old.keychain_oauth {
            if !keychain_oauth.contains(&provider) {
                super::secrets::delete_key(&format!("{OAUTH_PREFIX}{provider}"));
            }
        }
    }
    keychain_keys.sort();
    keychain_oauth.sort();
    let data_sources = ed
        .db
        .sources
        .iter()
        .map(|src| crate::storage::workspace::PersistedDataSource {
            id: src.id,
            name: src.name.clone(),
            kind: src.kind.as_str().to_string(),
            host: src.host.clone(),
            port: src.port,
            database: src.database.clone(),
            user: src.user.clone(),
            ssl_mode: src.ssl_mode.as_str().to_string(),
            trust_cert: src.trust_cert,
            save_password: src.save_password,
            url: src.url.clone(),
            path: None,
        })
        .collect();
    let settings = PersistedSettings {
        options: file_options,
        keychain_keys,
        keychain_oauth,
        data_sources,
    };
    if let Ok(content) = serde_json::to_string_pretty(&settings) {
        write_private(&dir.join(SETTINGS_FILE), &content);
    }
}

/// The settings file sits next to keychain-backed secrets; keep it
/// owner-only where the OS supports permission bits.
fn write_private(path: &std::path::Path, content: &str) {
    #[cfg(unix)]
    {
        use std::io::Write as _;
        use std::os::unix::fs::OpenOptionsExt;
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)
        {
            let _ = file.write_all(content.as_bytes());
            return;
        }
    }
    let _ = std::fs::write(path, content);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn editor_with_options(pairs: &[(&str, &str)]) -> super::super::Editor {
        let mut ed =
            super::super::Editor::new(Box::new(crate::storage::disk::DiskFileSystem::new()));
        for (name, value) in pairs {
            ed.options.insert(name.to_string(), value.to_string());
        }
        ed
    }

    #[test]
    fn settings_round_trip_without_secrets() {
        let dir = std::env::temp_dir().join(format!("magma-settings-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let ed = editor_with_options(&[("gui-theme", "dracula"), ("scrolloff", "5")]);
        save_settings_to(&dir, &ed);

        let mut restored = editor_with_options(&[]);
        load_settings_from(&dir, &mut restored);
        assert_eq!(
            restored.options.get("gui-theme").map(String::as_str),
            Some("dracula")
        );
        assert_eq!(
            restored.options.get("scrolloff").map(String::as_str),
            Some("5")
        );

        let raw = std::fs::read_to_string(dir.join(SETTINGS_FILE)).unwrap();
        assert!(
            !raw.contains("api-key"),
            "secret accounts must not leak into the file"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn secrets_never_land_in_the_file_and_manifest_tracks_providers() {
        // A provider name no real key is stored under — the keychain entry
        // this test creates must be its own and gets deleted below.
        const TEST_ACCOUNT: &str = "agent.api-key.zctestprovider";
        let dir = std::env::temp_dir().join(format!("magma-secrets-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let ed = editor_with_options(&[(TEST_ACCOUNT, "sk-test"), ("agent.provider", "openai")]);
        save_settings_to(&dir, &ed);

        let raw = std::fs::read_to_string(dir.join(SETTINGS_FILE)).unwrap();
        assert!(!raw.contains("sk-test"));
        let parsed: PersistedSettings = serde_json::from_str(&raw).unwrap();
        // Manifest only lists providers the keychain actually accepted; on a
        // machine without a key service the key stays session-only.
        let accepted = parsed.keychain_keys.contains(&"zctestprovider".to_string())
            && super::super::secrets::load_key(TEST_ACCOUNT).is_some();

        let mut restored = editor_with_options(&[]);
        load_settings_from(&dir, &mut restored);
        if accepted {
            assert_eq!(
                restored.options.get(TEST_ACCOUNT).map(String::as_str),
                Some("sk-test")
            );
        } else {
            assert!(!restored.options.contains_key(TEST_ACCOUNT));
        }
        let _ = std::fs::remove_dir_all(&dir);
        super::super::secrets::delete_key(TEST_ACCOUNT);
    }

    #[test]
    fn settings_round_trip_persists_data_sources() {
        let dir = std::env::temp_dir().join(format!("magma-db-settings-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut ed = editor_with_options(&[]);
        ed.db.add_source(db::DataSource {
            id: 0,
            name: "local-pg".into(),
            kind: db::DriverKind::Postgres,
            host: "localhost".into(),
            port: 0,
            database: "app".into(),
            user: "postgres".into(),
            ssl_mode: db::SslMode::Prefer,
            trust_cert: true,
            save_password: true,
            url: None,
        });
        save_settings_to(&dir, &ed);

        let mut restored = editor_with_options(&[]);
        load_settings_from(&dir, &mut restored);
        assert_eq!(restored.db.sources.len(), 1);
        assert_eq!(restored.db.sources[0].name, "local-pg");
        assert_eq!(restored.db.sources[0].port, 5432);
        assert!(restored.db.sources[0].trust_cert);
        assert_eq!(restored.db.active, Some(restored.db.sources[0].id));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
