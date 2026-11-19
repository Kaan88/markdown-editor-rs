//! API keys, in the system's credential store: Windows Credential Manager, the macOS Keychain,
//! the Secret Service on Linux. Each is kept under the service "mdedit" with the slot as its
//! user name ("claude-api", "openai:openrouter"), so it shows in Credential Manager as mdedit's.
//!
//! `MDEDIT_AI_KEY`, when set, is used for every slot instead and nothing is stored: for tests and
//! for running without a credential store.

use crate::proto::Fail;

const SERVICE: &str = "mdedit";

fn entry(slot: &str) -> Result<keyring::Entry, Fail> {
    keyring::Entry::new(SERVICE, slot).map_err(store_fail)
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
