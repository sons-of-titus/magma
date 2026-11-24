//! Persistent user settings (`settings.json` in the config dir).  Options
//! under secret prefixes are handed to the secrets file
//! (`super::secrets`), never written here.

use std::path::{Path, PathBuf};

const SETTINGS_FILE: &str = "settings.json";
/// Options under these prefixes carry secrets: they are stored in the
/// secrets file, never in the settings file.
const SECRET_PREFIX: &str = "agent.api-key.";
const OAUTH_PREFIX: &str = "agent.oauth.";

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct PersistedSettings {
    #[serde(default)]
    options: std::collections::HashMap<String, String>,
    #[serde(default)]
    data_sources: Vec<crate::storage::workspace::PersistedDataSource>,
}

fn is_secret_option(name: &str) -> bool {
    name.starts_with(SECRET_PREFIX) || name.starts_with(OAUTH_PREFIX)
}

/// The user config directory (~/.config/magma), creating it if needed.
pub fn config_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("MAGMA_CONFIG_DIR") {
        logging::LogErr::log_err(std::fs::create_dir_all(&dir));
        return PathBuf::from(dir);
    }
    let home = std::env::var("HOME").unwrap_or_default();
    let dir = if home.is_empty() {
        PathBuf::from("~/.config/magma")
    } else {
        PathBuf::from(home).join(".config").join("magma")
    };
    logging::LogErr::log_err(std::fs::create_dir_all(&dir));
    dir
}

/// Merge persisted settings and stored secrets into the editor's options.
/// Runs before the Janet user config so `init.janet` can override.
pub fn load_settings(ed: &mut super::Editor) {
    load_settings_from(&config_dir(), ed);
}

pub fn load_settings_from(dir: &Path, ed: &mut super::Editor) {
    let Ok(content) = std::fs::read_to_string(dir.join(SETTINGS_FILE)) else {
        return;
    };
    let Ok(settings) = serde_json::from_str::<PersistedSettings>(&content) else {
        return;
    };
    for (name, value) in settings.options {
        ed.options.insert(name, value);
    }
    for (account, secret) in super::secrets::load_all(dir) {
        if is_secret_option(&account) {
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

/// Write current options to the settings file and sync secrets to the
/// secrets file.
pub fn save_settings(ed: &super::Editor) {
    save_settings_to(&config_dir(), ed);
}

pub fn save_settings_to(dir: &Path, ed: &super::Editor) {
    logging::LogErr::log_err(std::fs::create_dir_all(dir));
    let mut file_options = std::collections::HashMap::new();
    // Rewriting the whole file: data-source passwords and anything else
    // already stored survive untouched, dropped agent secrets fall away.
    let mut stored_secrets = super::secrets::load_all(dir);
    stored_secrets.retain(|account, _| !is_secret_option(account));
    for (name, value) in &ed.options {
        if is_secret_option(name) {
            if !value.is_empty() {
                stored_secrets.insert(name.clone(), value.clone());
            }
            continue;
        }
        file_options.insert(name.clone(), value.clone());
    }
    super::secrets::save_all(dir, &stored_secrets);
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
        data_sources,
    };
    if let Ok(content) = serde_json::to_string_pretty(&settings) {
        write_private(&dir.join(SETTINGS_FILE), &content);
    }
}

/// Secrets sit in the same directory; keep both files owner-only where the
/// OS supports permission bits.
pub(crate) fn write_private(path: &Path, content: &str) {
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
            logging::LogErr::log_err(file.write_all(content.as_bytes()));
            return;
        }
    }
    if let Err(e) = std::fs::write(path, content) {
        logging::error!("cannot write {}: {e}", path.display());
    }
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
    fn secrets_live_in_their_own_file_and_round_trip() {
        const TEST_ACCOUNT: &str = "agent.api-key.zctestprovider";
        let dir = std::env::temp_dir().join(format!("magma-secrets-file-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let ed = editor_with_options(&[(TEST_ACCOUNT, "sk-test"), ("agent.provider", "openai")]);
        save_settings_to(&dir, &ed);

        let raw = std::fs::read_to_string(dir.join(SETTINGS_FILE)).unwrap();
        assert!(!raw.contains("sk-test"), "secret value must not leak into settings");
        assert!(!raw.contains(TEST_ACCOUNT), "secret account must not leak into settings");
        let secrets = std::fs::read_to_string(dir.join("secrets.json")).unwrap();
        assert!(secrets.contains("sk-test"));

        let mut restored = editor_with_options(&[]);
        load_settings_from(&dir, &mut restored);
        assert_eq!(
            restored.options.get(TEST_ACCOUNT).map(String::as_str),
            Some("sk-test")
        );
        assert_eq!(
            restored.options.get("agent.provider").map(String::as_str),
            Some("openai")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn removing_a_secret_option_drops_it_from_the_secrets_file() {
        const TEST_ACCOUNT: &str = "agent.oauth.zctestprovider";
        let dir =
            std::env::temp_dir().join(format!("magma-secrets-drop-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let ed = editor_with_options(&[(TEST_ACCOUNT, "tok")]);
        save_settings_to(&dir, &ed);

        let ed = editor_with_options(&[]);
        save_settings_to(&dir, &ed);
        let secrets = super::super::secrets::load_all(&dir);
        assert!(!secrets.contains_key(TEST_ACCOUNT));
        let _ = std::fs::remove_dir_all(&dir);
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
