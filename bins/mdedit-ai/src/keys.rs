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

/// The stored key, or `None` when there is none. An empty slot (a provider that takes no key)
/// has none, and the credential store is not asked, so a local server works where no store runs.
pub fn get(slot: &str) -> Result<Option<String>, Fail> {
    if slot.is_empty() {
        return Ok(None);
    }
    if let Some(k) = override_key() {
        return usable(k);
    }
    match entry(slot)?.get_password() {
        Ok(k) => usable(k),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(store_fail(e)),
    }
}

/// A key as found, under the one rule every key follows wherever it comes from: shorter than
/// [`KEY_MIN`] it is never sent, since it could not be hidden from messages (audit 1 round 3, #4).
fn usable(k: String) -> Result<Option<String>, Fail> {
    match k.trim().chars().count() {
        0 => Ok(None),
        n if n < KEY_MIN => Err(Fail::new(
            "auth",
            "The stored key is too short to be an API key. Enter it again in Settings › AI.",
        )),
        _ => Ok(Some(k)),
    }
}

pub fn set(slot: &str, key: &str) -> Result<(), Fail> {
    let key = key.trim();
    if !key.is_empty() && key.chars().count() < KEY_MIN {
        return Err(Fail::new(
            "bad_request",
            "This is too short to be an API key.",
        ));
    }
    if override_key().is_some() {
        return Ok(());
    }
    if key.is_empty() {
        return clear(slot);
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

    /// A provider that takes no key (an empty slot) has none, and the store is not asked: a
    /// local server works where no credential store runs.
    #[test]
    fn an_empty_slot_has_no_key_and_asks_no_store() {
        assert!(matches!(get(""), Ok(None)));
    }

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

    /// Audit 1 round 3, #4: a key too short to hide is refused wherever it comes from.
    #[test]
    fn a_short_key_is_never_used() {
        assert!(usable("abc".into()).is_err());
        assert!(usable(" ab ".into()).is_err());
        assert_eq!(usable(String::new()).unwrap(), None);
        assert_eq!(usable("abcd".into()).unwrap().as_deref(), Some("abcd"));
        assert!(set("claude-api", "xyz").is_err());
    }
}
