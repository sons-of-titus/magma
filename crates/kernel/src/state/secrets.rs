//! OS-keychain storage for secrets (agent API keys).  The platform keychain
//! encrypts at rest; secrets never reach the settings file.  Every call is
//! graceful on platforms without a key service — the secret then simply
//! stays session-only.

use logging::debug;

const SERVICE: &str = "magma";

/// `MAGMA_KEYCHAIN=off` routes every secret through the session-only path:
/// tests would otherwise touch the developer's real login keychain, and a
/// container has no key service at all.
fn keychain_disabled() -> bool {
    std::env::var("MAGMA_KEYCHAIN").as_deref() == Ok("off")
}

fn entry(account: &str) -> Option<keyring::Entry> {
    if keychain_disabled() {
        return None;
    }
    keyring::Entry::new(SERVICE, account).ok()
}

pub fn load_key(account: &str) -> Option<String> {
    let entry = entry(account)?;
    match entry.get_password() {
        Ok(secret) => Some(secret),
        Err(keyring::Error::NoEntry) => None,
        Err(e) => {
            debug!("keychain: could not read '{account}': {e}");
            None
        }
    }
}

/// Returns false when the platform keychain refused (the secret then stays
/// session-only and must not be listed as persisted).
pub fn save_key(account: &str, secret: &str) -> bool {
    let Some(entry) = entry(account) else {
        return false;
    };
    match entry.set_password(secret) {
        Ok(()) => true,
        Err(e) => {
            debug!("keychain: could not store '{account}': {e}");
            false
        }
    }
}

pub fn delete_key(account: &str) {
    if let Some(entry) = entry(account)
        && let Err(e) = entry.delete_credential()
        && !matches!(e, keyring::Error::NoEntry)
    {
        debug!("keychain: could not delete '{account}': {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keychain_off_is_session_only() {
        // No other kernel test touches the keyring, so flipping the env
        // here cannot race a concurrent reader.
        unsafe { std::env::set_var("MAGMA_KEYCHAIN", "off") };
        assert!(!save_key("db-source-test", "hunter2"));
        assert_eq!(load_key("db-source-test"), None);
        delete_key("db-source-test");
        unsafe { std::env::remove_var("MAGMA_KEYCHAIN") };
    }
}
