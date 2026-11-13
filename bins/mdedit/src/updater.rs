//! Updating mdedit in place: `mdshared_app::updater` with this application's name and manifest,
//! and `mdshared_fetch` as the transport. The mechanism, the threads, the messages and the bar
//! are the shared crates'; the viewer's `docs/UPDATE.md` describes the whole flow.

use mdshared_app::updater as shared;
use std::time::Duration;

pub use shared::{
    Event, Files, NOT_INSTALLED, Stage, Updates, at_launch_label, check_due, spawn_at_launch,
    spawn_check, spawn_install, unix_now,
};

/// mdedit as the updater knows it: one executable, and no installer entry yet.
pub const APP: shared::App = shared::App {
    name: "mdedit",
    version: env!("CARGO_PKG_VERSION"),
    manifest_url: "https://github.com/Kaan88/markdown-editor-rs/releases/latest/download/mdedit-update.txt",
    manifest_env: "MDEDIT_UPDATE_MANIFEST",
    engine: None,
    uninstall_key: None,
};

const USER_AGENT: &str = "mdedit";

/// The transport the application uses.
pub fn fetch(url: &str, max_bytes: usize, timeout: Duration) -> Result<Vec<u8>, String> {
    mdshared_fetch::fetch_file(USER_AGENT, url, max_bytes, timeout)
}

/// This process's executable, when this copy can update itself (not a cargo build's output).
pub fn installed_files() -> Option<Files> {
    shared::installed_files(&APP)
}

/// `--check-update`: looks for a newer version as the application would and prints the answer.
pub fn check_and_print() -> i32 {
    eprintln!(
        "mdedit {}; manifest {}",
        APP.current_version(),
        APP.manifest_url()
    );
    match shared::check(&APP, &fetch) {
        Ok(Some(offer)) => {
            eprintln!("mdedit {} is available", offer.version);
            eprintln!("  {}", offer.app.url);
            0
        }
        Ok(None) => {
            eprintln!("nothing newer");
            0
        }
        Err(e) => {
            eprintln!("the look failed: {e}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_application_is_described_to_the_updater() {
        // The package version must read as a version, or every look would panic.
        assert_eq!(APP.current_version().to_string(), env!("CARGO_PKG_VERSION"));
        assert!(APP.manifest_url.starts_with("https://"));
        // A cargo build never updates itself.
        assert!(installed_files().is_none());
    }
}
