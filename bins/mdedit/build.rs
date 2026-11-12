//! Embeds the application icon on Windows when the SDK's resource compiler can be found. Without
//! it the build still succeeds; the exe just has no icon.

use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=../../assets/mdedit.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows")
        || std::env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc")
    {
        return;
    }
    let Some(rc) = find_rc() else {
        println!("cargo:warning=rc.exe not found; mdedit.exe is built without its icon");
        return;
    };
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let icon = manifest.join("../../assets/mdedit.ico");
    let script = out.join("mdedit.rc");
    let res = out.join("mdedit.res");
    // Resource 1 is the icon Explorer and the taskbar show.
    let text = format!(
        "1 ICON \"{}\"\n",
        icon.display().to_string().replace('\\', "/")
    );
    if std::fs::write(&script, text).is_err() {
        return;
    }
    let ok = Command::new(rc)
        .arg("/nologo")
        .arg("/fo")
        .arg(&res)
        .arg(&script)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if ok {
        println!("cargo:rustc-link-arg-bins={}", res.display());
    } else {
        println!("cargo:warning=rc.exe failed; mdedit.exe is built without its icon");
    }
}

/// `rc.exe` from PATH, or the newest x64 one under the Windows 10/11 SDK.
fn find_rc() -> Option<PathBuf> {
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            let candidate = dir.join("rc.exe");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    let base = PathBuf::from(std::env::var_os("ProgramFiles(x86)")?).join("Windows Kits/10/bin");
    let mut versions: Vec<PathBuf> = std::fs::read_dir(base)
        .ok()?
        .flatten()
        .map(|e| e.path().join("x64/rc.exe"))
        .filter(|p| p.is_file())
        .collect();
    versions.sort();
    versions.pop()
}
