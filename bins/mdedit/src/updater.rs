//! Updating mdedit in place: `mdshared_app::updater` with this application's name and manifest,
//! and `mdshared_fetch` as the transport. The mechanism, the threads, the messages and the bar
//! are the shared crates'; the viewer's `docs/UPDATE.md` describes the whole flow.

use mdshared_app::updater as shared;
use std::time::Duration;

pub use shared::{
    Event, Files, NOT_INSTALLED, Stage, Updates, at_launch_label, check_due, spawn_at_launch,
    spawn_check, spawn_install, unix_now,
};

/// mdedit as the updater knows it: the executable and, on Windows, the two programs beside it:
/// the engine for the preview pane's formulas and diagrams (`mdview-engine.exe`, built from the
/// viewer at the revision mdedit pins) and the AI engine (`mdedit-ai.exe`). No installer entry
/// yet.
///
/// Windows reads `mdedit-update-2.txt`, the format-2 manifest that can list `mdedit-ai`;
/// `mdedit-update.txt` stays format 1 for the builds up to 1.0.6, which refuse any key they do not
/// know (tools/write-manifest.sh writes both). Elsewhere the AI engine is inside the AppImage or
/// the bundle, and the manifests read there are unchanged.
pub const APP: shared::App = shared::App {
    name: "mdedit",
    version: env!("CARGO_PKG_VERSION"),
    manifest_url: MANIFEST_URL,
    manifest_env: "MDEDIT_UPDATE_MANIFEST",
    engine: Some("mdview-engine"),
    extras: &["mdedit-ai"],
    uninstall_key: None,
};

/// The manifest this build reads (see [`APP`]).
pub const MANIFEST_URL: &str = if cfg!(windows) {
    "https://github.com/Kaan88/markdown-editor-rs/releases/latest/download/mdedit-update-2.txt"
} else {
    "https://github.com/Kaan88/markdown-editor-rs/releases/latest/download/mdedit-update.txt"
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

/// True when a program this copy ships beside its executable is not there: the preview pane's
/// engine or the AI engine. A copy that updated itself from 1.0.6 or earlier has no `mdedit-ai`,
/// since those builds read the format-1 manifest, which cannot list it.
pub fn missing_parts(files: &Files) -> bool {
    files
        .engine
        .iter()
        .chain(&files.extras)
        .any(|p| !p.exists())
}

/// At launch, when the look for a newer version is not due (the reader turned it off) and a
/// program beside the executable is missing: fetches the manifest and puts in place what it lists
/// of this very version ([`shared::restore_engine`]). Nothing is shown; an offer it may hold is
/// left for a look the reader asks for. When the look is due, `spawn_at_launch` does the same.
pub fn spawn_restore(files: Files) {
    if !missing_parts(&files) {
        return;
    }
    let _ = std::thread::Builder::new()
        .name("mdedit-restore".into())
        .spawn(move || {
            let Some(platform) = mdshared_app::update::Platform::current() else {
                return;
            };
            let bytes = fetch(
                &APP.manifest_url(),
                mdshared_app::update::MAX_MANIFEST_BYTES,
                Duration::from_secs(30),
            );
            if let Ok(bytes) = bytes {
                _ = shared::restore_engine(&APP, &files, &bytes, platform, &fetch);
            }
        });
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
        // Windows reads the format-2 manifest, which lists the AI engine; the others keep theirs.
        assert_eq!(APP.extras, ["mdedit-ai"]);
        assert_eq!(
            MANIFEST_URL.ends_with("/mdedit-update-2.txt"),
            cfg!(windows),
            "{MANIFEST_URL}"
        );
    }

    #[test]
    fn a_missing_engine_or_ai_engine_is_noticed() {
        let dir = std::env::temp_dir().join(format!("mdedit-parts-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = |name: &str| {
            let p = dir.join(name);
            std::fs::write(&p, name).unwrap();
            p
        };
        let mut files = Files {
            app: file("mdedit.exe"),
            engine: Some(file("mdview-engine.exe")),
            extras: vec![file("mdedit-ai.exe")],
        };
        assert!(!missing_parts(&files));
        // A copy updated from 1.0.6: the AI engine was never installed.
        files.extras = vec![dir.join("not-there.exe")];
        assert!(missing_parts(&files));
        files.extras.clear();
        files.engine = Some(dir.join("not-there.exe"));
        assert!(missing_parts(&files));
        _ = std::fs::remove_dir_all(&dir);
    }
}
