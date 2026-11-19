//! The manifests the release scripts write (tools/write-manifest.sh, tools/release.ps1) must read
//! as installable releases with the build that reads each, and every file they list must have the
//! hash they state. Each of these folders is checked when it exists, and skipped when it does not:
//!
//! * `target/release-files-all/`   the release workflow and tools/release-all.sh
//! * `target/dist-cross/`          tools/release-cross.sh (Windows only)
//! * `target/dist/<version>/`      tools\release.ps1 on Windows (Windows only)
//!
//! `mdedit-update.txt` (format 1) is what mdedit up to 1.0.6 reads on Windows and Linux, and 1.1 on
//! Linux: it is read here with 1.0.6's shared updater (`mdshared-app-106`) and with this one.
//! `mdedit-update-2.txt` (format 2) is what 1.1 reads on Windows: it must list the AI engine, and
//! 1.0.6's reader must refuse it (that is why there are two). Both platforms are checked whatever
//! this runs on.

use mdshared_app::update::{Platform, UpdateManifest, Version, newer, verify_sha256_file};
use mdshared_app_106::update as old;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn folders() -> Vec<PathBuf> {
    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target");
    let out = vec![
        target.join("release-files-all"),
        target.join("dist-cross"),
        target.join("dist").join(env!("CARGO_PKG_VERSION")),
    ];
    out.into_iter()
        .filter(|d| d.join("mdedit-update.txt").is_file())
        .collect()
}

/// `key: value` lines of a manifest.
fn keys(text: &str) -> HashMap<String, String> {
    text.lines()
        .filter_map(|l| l.split_once(": "))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

/// Every file a manifest lists is beside it, with the stated hash; how many.
fn files_match(dir: &Path, text: &str) -> usize {
    let k = keys(text);
    let mut checked = 0;
    for (key, hash) in k.iter().filter(|(k, _)| k.ends_with(".sha256")) {
        let entry = key.trim_end_matches(".sha256");
        let url = k
            .get(&format!("{entry}.url"))
            .unwrap_or_else(|| panic!("{key} has no .url"));
        let file = dir.join(url.rsplit('/').next().unwrap());
        assert!(file.is_file(), "{} is listed but missing", file.display());
        assert!(
            verify_sha256_file(&file, hash).unwrap(),
            "{} does not match {key}",
            file.display()
        );
        checked += 1;
    }
    checked
}

#[test]
fn the_release_manifests_are_read_and_their_files_match() {
    for dir in folders() {
        let one = std::fs::read(dir.join("mdedit-update.txt")).unwrap();
        let text = String::from_utf8(one.clone()).expect("the manifest is UTF-8");
        let version = keys(&text)["version"].clone();
        let d = dir.display();
        assert!(text.starts_with("manifest: 1\n"), "{d}: format 1");
        assert!(files_match(&dir, &text) >= 1, "{d}: no file hashes");

        // Format 1, as mdedit 1.0.6 reads it.
        let m = old::UpdateManifest::parse(&one).unwrap_or_else(|e| panic!("{d}: 1.0.6: {e}"));
        assert_eq!(m.version.to_string(), version);
        let older = old::Version::parse("1.0.6").unwrap();
        let windows = m.asset(old::Platform::WindowsX64).is_some();
        if windows {
            let a = old::newer(&older, &m, old::Platform::WindowsX64).expect("1.0.6 is offered");
            assert!(
                a.url.ends_with(&format!("/v{version}/{}", a.name)),
                "{d}: {}",
                a.url
            );
            let e = a.engine.expect("the Windows entry lists the engine");
            assert!(
                e.url.ends_with(&format!("/v{version}/mdview-engine.exe")),
                "{d}: {}",
                e.url
            );
        }
        if m.asset(old::Platform::LinuxX64).is_some() {
            assert!(
                old::newer(&older, &m, old::Platform::LinuxX64).is_some(),
                "{d}: Linux"
            );
        }

        // Format 1, as this build reads it on Linux.
        let m = UpdateManifest::parse(&one).unwrap_or_else(|e| panic!("{d}: {e}"));
        let older = Version::parse("1.0.6").unwrap();
        if m.asset(Platform::LinuxX64).is_some() {
            let a = newer(&older, &m, Platform::LinuxX64).expect("Linux is offered");
            assert!(
                a.url.ends_with(&format!("/v{version}/{}", a.name)),
                "{d}: {}",
                a.url
            );
        }

        // Format 2, as this build reads it on Windows.
        let two_path = dir.join("mdedit-update-2.txt");
        if !windows {
            continue;
        }
        let two = std::fs::read(&two_path)
            .unwrap_or_else(|_| panic!("{d}: Windows files but no mdedit-update-2.txt"));
        let text2 = String::from_utf8(two.clone()).unwrap();
        assert!(text2.starts_with("manifest: 2\n"), "{d}: format 2");
        assert!(
            files_match(&dir, &text2) >= 3,
            "{d}: format 2 lists exe, engine, AI engine"
        );
        let m = UpdateManifest::parse(&two).unwrap_or_else(|e| panic!("{d}: format 2: {e}"));
        assert_eq!(m.version.to_string(), version);
        let a = newer(&older, &m, Platform::WindowsX64).expect("Windows is offered");
        assert!(a.engine.is_some(), "{d}: format 2 lists the engine");
        let ai = a.extra("mdedit-ai").expect("format 2 lists mdedit-ai");
        assert!(
            ai.url.ends_with(&format!("/v{version}/mdedit-ai.exe")),
            "{d}: {}",
            ai.url
        );
        // 1.0.6 refuses it: it must keep reading format 1.
        assert!(
            old::UpdateManifest::parse(&two).is_err(),
            "{d}: 1.0.6 reads format 2"
        );
        // A manifest at the current version offers nothing.
        let same = Version::parse(&version).unwrap();
        assert!(newer(&same, &m, Platform::WindowsX64).is_none());
    }
}
