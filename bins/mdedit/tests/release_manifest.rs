//! The manifests the release scripts write (tools/write-manifest.sh, tools/release.ps1) must read
//! as installable releases, and every file they list must have the hash they state. Each of
//! these folders is checked when it exists, and skipped when it does not:
//!
//! * `target/release-files-all/`   tools/release-all.sh (Windows and Linux)
//! * `target/dist-cross/`          tools/release-cross.sh (Windows only)
//! * `target/dist/<version>/`      tools\release.ps1 on Windows (Windows only)
//!
//! Runs on Windows and Linux. The platform this test runs on is offered the release when the
//! manifest has an entry for it.

use mdshared_app::update::{Version, verify_sha256_file};
use mdshared_app::updater::{self as shared, App};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const APP: App = App {
    name: "mdedit",
    version: env!("CARGO_PKG_VERSION"),
    manifest_url: "https://github.com/Kaan88/markdown-editor-rs/releases/latest/download/mdedit-update.txt",
    manifest_env: "MDEDIT_UPDATE_MANIFEST",
    engine: None,
    uninstall_key: None,
};

fn folders() -> Vec<PathBuf> {
    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target");
    let mut out = vec![target.join("release-files-all"), target.join("dist-cross")];
    out.push(target.join("dist").join(env!("CARGO_PKG_VERSION")));
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

#[test]
fn the_release_manifests_are_read_and_their_files_match() {
    let current = if cfg!(windows) {
        "windows-x86_64"
    } else {
        "linux-x86_64"
    };
    for dir in folders() {
        let bytes = std::fs::read(dir.join("mdedit-update.txt")).unwrap();
        let text = String::from_utf8(bytes.clone()).expect("the manifest is UTF-8");
        let k = keys(&text);
        let version = k.get("version").expect("a version");
        let older = Version::parse("0.0.1").unwrap();

        // The platform this runs on is offered the release when the manifest lists it.
        let offer = shared::offer(&APP, &bytes, &older).expect("the manifest parses");
        match k.get(&format!("{current}.asset")) {
            Some(asset) => {
                let o = offer.unwrap_or_else(|| panic!("{}: no offer for {current}", dir.display()));
                assert_eq!(o.version.to_string(), *version);
                assert!(
                    o.app.url.ends_with(&format!("/v{version}/{asset}")),
                    "{}: url {} does not end in v{version}/{asset}",
                    dir.display(),
                    o.app.url
                );
                assert!(o.engine.is_none(), "mdedit has no engine file");
            }
            None => assert!(offer.is_none(), "{}: offered a platform it does not list", dir.display()),
        }
        // A manifest at the current version offers nothing.
        let same = Version::parse(version).unwrap();
        assert!(shared::offer(&APP, &bytes, &same).unwrap().is_none());

        // Every file the manifest lists is beside it, with the stated hash.
        let mut checked = 0;
        for (key, hash) in k.iter().filter(|(k, _)| k.ends_with(".sha256")) {
            let platform = key.trim_end_matches(".sha256");
            let asset = k
                .get(&format!("{platform}.asset"))
                .unwrap_or_else(|| panic!("{key} has no .asset"));
            let file = dir.join(asset);
            assert!(file.is_file(), "{} is listed but missing", file.display());
            assert!(
                verify_sha256_file(&file, hash).unwrap(),
                "{} does not match {key}",
                file.display()
            );
            checked += 1;
        }
        assert!(checked >= 1, "{}: no file hashes in the manifest", dir.display());
    }
}
