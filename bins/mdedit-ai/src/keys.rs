//! API keys, in the system's credential store: Windows Credential Manager, the macOS Keychain,
//! the Secret Service on Linux. Each is kept under the service "mdedit" with the slot as its
//! user name ("claude-api", "openai:openrouter"), so it shows in Credential Manager as mdedit's.
//!
//! `MDEDIT_AI_KEY`, when set, is used for every slot instead and nothing is stored: for tests and
//! for running without a credential store.

use crate::proto::Fail;

const SERVICE: &str = "mdedit";

fn entry(slot: &str) -> Result<keyring::Entry, Fail> {
    keyring::Entry::new(SERVICE, &target(slot)).map_err(store_fail)
}

/// The prefix of a custom endpoint's slot, `openai:custom:<url>`.
const CUSTOM: &str = "openai:custom:";
/// The shortest key taken: shorter text is not a key, and could not be hidden from messages.
pub const KEY_MIN: usize = 4;

/// The user name a slot's key is stored under. A custom endpoint's URL becomes the hex SHA-256 of
/// it: Windows compares credential names without case, and URL paths can differ only in case;
/// it also keeps the name short whatever the URL's length.
pub fn target(slot: &str) -> String {
    use sha2::Digest;
    match slot.strip_prefix(CUSTOM) {
        Some(url) => {
            let digest = sha2::Sha256::digest(url.as_bytes());
            let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
            format!("{CUSTOM}{hex}")
        }
        None => slot.to_string(),
    }
}

fn store_fail(e: keyring::Error) -> Fail {
    if matches!(e, keyring::Error::NoDefaultStore) && cfg!(target_os = "linux") {
        return Fail::new(
            "keystore",
            "No credential store is running to keep the key in. Start one (GNOME Keyring or KWallet's Secret Service) and try again.",
        );
    }
    Fail::new(
        "keystore",
        format!("The system's credential store could not be used: {e}"),
    )
}

fn override_key() -> Option<String> {
    std::env::var("MDEDIT_AI_KEY").ok()
}

/// The stored key, or `None` when there is none.
pub fn get(slot: &str) -> Result<Option<String>, Fail> {
    if let Some(k) = override_key() {
        return Ok((!k.is_empty()).then_some(k));
    }
    match entry(slot)?.get_password() {
        Ok(k) => Ok(Some(k)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(store_fail(e)),
    }
}

pub fn set(slot: &str, key: &str) -> Result<(), Fail> {
    if override_key().is_some() {
        return Ok(());
    }
    let key = key.trim();
    if key.is_empty() {
        return clear(slot);
    }
    if key.chars().count() < KEY_MIN {
        return Err(Fail::new(
            "bad_request",
            "This is too short to be an API key.",
        ));
    }
    entry(slot)?.set_password(key).map_err(store_fail)
}

pub fn clear(slot: &str) -> Result<(), Fail> {
    if override_key().is_some() {
        return Ok(());
    }
    match entry(slot)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(store_fail(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Audit 1, round 2: paths that differ only in case get different credentials on Windows,
    /// and every name is short and in lower case.
    #[test]
    fn custom_endpoints_have_names_apart() {
        let a = target("openai:custom:https://h.example/Tenant/v1");
        let b = target("openai:custom:https://h.example/tenant/v1");
        assert_ne!(a.to_lowercase(), b.to_lowercase());
        for t in [&a, &b] {
            assert_eq!(t, &t.to_lowercase());
            assert_eq!(t.len(), CUSTOM.len() + 64);
        }
        let long = format!("openai:custom:https://h.example/{}", "x".repeat(2000));
        assert_eq!(target(&long).len(), CUSTOM.len() + 64);
        assert_eq!(target("claude-api"), "claude-api");
        assert_eq!(target("openai:groq"), "openai:groq");
    }
}
