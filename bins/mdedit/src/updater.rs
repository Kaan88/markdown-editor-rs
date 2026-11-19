//! Updating mdedit in place: `mdshared_app::updater` with this application's name and manifest,
//! and `mdshared_fetch` as the transport. The mechanism, the threads, the messages and the bar
//! are the shared crates'; the viewer's `docs/UPDATE.md` describes the whole flow.

use mdshared_app::updater as shared;
use std::time::Duration;

pub use shared::{
    Event, Files, NOT_INSTALLED, Stage, Updates, at_launch_label, check_due, spawn_install,
    unix_now,
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

/// The manifest of this very version, for a repair: the one in the release of its own tag, which
/// a newer release does not replace (`MDEDIT_UPDATE_MANIFEST` overrides it, as it does the look).
/// The latest release's manifest is of another version once a newer one is out, and a repair
/// takes nothing from that (audit 1 round 5).
pub fn repair_manifest_url() -> String {
    if let Ok(url) = std::env::var(APP.manifest_env) {
        return url;
    }
    let latest = APP.manifest_url();
    let name = latest.rsplit('/').next().unwrap_or_default();
    format!(
        "https://github.com/Kaan88/markdown-editor-rs/releases/download/v{}/{name}",
        APP.version
    )
}

/// One repair at a time: the launch's and a look the reader asks for would write the same files.
static REPAIRING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Puts in place what is missing beside the executable (see [`missing_parts`]), from this
/// version's own manifest ([`repair_manifest_url`]). `Ok(true)` when something was put in place,
/// `Ok(false)` when nothing was missing, a repair is already running or the release lists none
/// of it; `Err` when a download, its hash or the write failed: nothing is in place then, and the
/// next start or look tries again.
pub fn repair(files: &Files) -> Result<bool, String> {
    use std::sync::atomic::Ordering;
    if !missing_parts(files) {
        return Ok(false);
    }
    let Some(platform) = mdshared_app::update::Platform::current() else {
        return Ok(false);
    };
    if REPAIRING.swap(true, Ordering::AcqRel) {
        return Ok(false);
    }
    let result = fetch(
        &repair_manifest_url(),
        mdshared_app::update::MAX_MANIFEST_BYTES,
        Duration::from_secs(30),
    )
    .and_then(|bytes| shared::restore_engine(&APP, files, &bytes, platform, &fetch));
    REPAIRING.store(false, Ordering::Release);
    result
}

/// What [`spawn_look`] posts.
#[derive(Debug)]
pub enum Answer {
    /// The look for a newer version.
    Update(Event),
    /// A repair ran ([`repair`]).
    Repaired(Result<bool, String>),
}

/// On its own thread: at launch (`launch`), clears what an earlier update left behind; when a
/// program beside the executable is missing, repairs it ([`repair`]) and posts how that went;
/// then, when `look` says so, looks for a newer version (`Some(manual)`) and posts the answer.
/// `files` is `None` for a copy that cannot update itself: then only the look runs.
pub fn spawn_look(
    files: Option<Files>,
    launch: bool,
    look: Option<bool>,
    post: impl Fn(Answer) + Send + 'static,
) {
    let _ = std::thread::Builder::new()
        .name("mdedit-update".into())
        .spawn(move || {
            if let Some(files) = &files {
                if launch {
                    shared::clean_up(files);
                }
                if missing_parts(files) {
                    post(Answer::Repaired(repair(files)));
                }
            }
            if let Some(manual) = look {
                let result = shared::check(&APP, &fetch);
                post(Answer::Update(Event::Checked { manual, result }));
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

    /// Audit 1 round 5: a repair reads the manifest of this very version, in the release of its
    /// own tag, which stays when a newer release is out; the look reads the latest.
    #[test]
    fn a_repair_reads_the_manifest_of_its_own_version() {
        if std::env::var(APP.manifest_env).is_ok() {
            return;
        }
        let url = repair_manifest_url();
        let latest = APP.manifest_url();
        let name = latest.rsplit('/').next().unwrap();
        assert_eq!(
            url,
            format!(
                "https://github.com/Kaan88/markdown-editor-rs/releases/download/v{}/{name}",
                env!("CARGO_PKG_VERSION")
            )
        );
        assert!(latest.contains("/releases/latest/download/"));
        // Nothing missing: nothing is fetched.
        let dir = std::env::temp_dir().join(format!("mdedit-repair-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let app = dir.join("mdedit.exe");
        std::fs::write(&app, "x").unwrap();
        let files = Files {
            app: app.clone(),
            engine: None,
            extras: vec![app],
        };
        assert_eq!(repair(&files), Ok(false));
        let _ = std::fs::remove_dir_all(&dir);
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
