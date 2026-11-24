//! File-backed storage for secrets (agent API keys, OAuth tokens, saved
//! data-source passwords).  The secrets file sits next to settings.json,
//! holds only secrets, and is written with owner-only permissions; the
//! settings file never sees a secret.  Every call is graceful when storage
//! is unavailable — the secret then simply stays session-only.

use logging::debug;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const SECRET_PREFIX: &str = "agent.api-key.";
const FILE_NAME: &str = "secrets.json";

/// `MAGMA_SECRETS=off` routes every secret through the session-only path:
/// tests would otherwise touch the developer's real secrets file.
fn disabled() -> bool {
    std::env::var("MAGMA_SECRETS").as_deref() == Ok("off")
}

fn file_path() -> Option<PathBuf> {
    if disabled() {
        return None;
    }
    Some(super::settings::config_dir().join(FILE_NAME))
}

/// API keys may come from the environment: `agent.api-key.openai` maps to
/// `MAGMA_API_KEY_OPENAI`.  The environment wins over the stored file, so
/// a shell-exported key takes precedence without editing any file.
fn env_override(account: &str) -> Option<String> {
    let provider = account.strip_prefix(SECRET_PREFIX)?;
    let var = format!("MAGMA_API_KEY_{}", provider.to_uppercase().replace('-', "_"));
    std::env::var(&var).ok().filter(|v| !v.is_empty())
}

pub fn load_key(account: &str) -> Option<String> {
    if let Some(secret) = env_override(account) {
        return Some(secret);
    }
    let path = file_path()?;
    read_file(&path).remove(account)
}

/// Returns false when storage is off — the secret then stays session-only
/// and must not be listed as persisted.
pub fn save_key(account: &str, secret: &str) -> bool {
    let Some(path) = file_path() else {
        return false;
    };
    let mut stored = read_file(&path);
    stored.insert(account.to_string(), secret.to_string());
    write_file(&path, &stored);
    true
}

pub fn delete_key(account: &str) {
    let Some(path) = file_path() else {
        return;
    };
    let mut stored = read_file(&path);
    if stored.remove(account).is_some() {
        write_file(&path, &stored);
    }
}

/// All stored secrets under `dir` — the dir-scoped form settings
/// persistence uses, so tests can work in a temporary directory.
pub(crate) fn load_all(dir: &Path) -> BTreeMap<String, String> {
    read_file(&dir.join(FILE_NAME))
}

pub(crate) fn save_all(dir: &Path, secrets: &BTreeMap<String, String>) {
    write_file(&dir.join(FILE_NAME), secrets);
}

fn read_file(path: &Path) -> BTreeMap<String, String> {
    match std::fs::read_to_string(path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(e) => {
            if e.kind() != std::io::ErrorKind::NotFound {
                debug!("secrets: could not read {}: {e}", path.display());
            }
            BTreeMap::new()
        }
    }
}

fn write_file(path: &Path, secrets: &BTreeMap<String, String>) {
    match serde_json::to_string_pretty(secrets) {
        Ok(content) => super::settings::write_private(path, &content),
        Err(e) => debug!("secrets: could not serialize: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Serializes the env-var flips below; the vars are process-global and
    // kernel tests run in parallel threads.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    const ACCOUNT: &str = "agent.api-key.zctestprovider";

    #[test]
    fn round_trip_through_config_dir() {
        let _env = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("magma-secrets-home-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        unsafe { std::env::set_var("MAGMA_CONFIG_DIR", &dir) };

        assert!(save_key(ACCOUNT, "sk-test"));
        assert_eq!(load_key(ACCOUNT).as_deref(), Some("sk-test"));
        let raw = std::fs::read_to_string(dir.join(FILE_NAME)).unwrap();
        assert!(raw.contains("sk-test"));

        delete_key(ACCOUNT);
        assert_eq!(load_key(ACCOUNT), None);
        let raw = std::fs::read_to_string(dir.join(FILE_NAME)).unwrap();
        assert!(!raw.contains(ACCOUNT));

        unsafe { std::env::remove_var("MAGMA_CONFIG_DIR") };
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn env_var_overrides_the_stored_key() {
        let _env = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { std::env::set_var("MAGMA_API_KEY_ZCTESTPROVIDER", "sk-env") };
        assert_eq!(load_key(ACCOUNT).as_deref(), Some("sk-env"));
        unsafe { std::env::remove_var("MAGMA_API_KEY_ZCTESTPROVIDER") };
    }

    #[test]
    fn off_is_session_only() {
        let _env = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { std::env::set_var("MAGMA_SECRETS", "off") };
        assert!(!save_key(ACCOUNT, "hunter2"));
        assert_eq!(load_key(ACCOUNT), None);
        delete_key(ACCOUNT);
        unsafe { std::env::remove_var("MAGMA_SECRETS") };
    }
}
