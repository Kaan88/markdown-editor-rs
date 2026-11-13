//! Embeds the application icon on Windows when a resource compiler can be found: the SDK's
//! `rc.exe`, or, when cross-compiling from Linux, LLVM's `llvm-rc` (from the `MD_LLVM_RC`
//! variable or PATH). Without either the build still succeeds; the exe just has no icon.

use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=../../assets/mdedit.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows")
        || std::env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc")
    {
        return;
    }
    let Some((rc, llvm)) = find_rc() else {
        println!("cargo:warning=rc.exe and llvm-rc not found; mdedit.exe is built without its icon");
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
    let mut command = Command::new(rc);
    if llvm {
        // llvm-rc: the script has no #include, so it needs no preprocessor (and no clang).
        // `--` ends the options: llvm-rc takes an absolute input path like /cache/... for one.
        command.arg("/FO").arg(&res).arg("-no-preprocess").arg("--").arg(&script);
    } else {
        command.arg("/nologo").arg("/fo").arg(&res).arg(&script);
    }
    let ok = command.status().map(|s| s.success()).unwrap_or(false);
    if ok {
        println!("cargo:rustc-link-arg-bins={}", res.display());
    } else {
        println!("cargo:warning=the resource compiler failed; mdedit.exe is built without its icon");
    }
}

/// The resource compiler and whether it is LLVM's: `rc.exe` from PATH, the newest x64 one under
/// the Windows 10/11 SDK, then `llvm-rc` (`MD_LLVM_RC`, else PATH).
fn find_rc() -> Option<(PathBuf, bool)> {
    let on_path = |name: &str| {
        let paths = std::env::var_os("PATH")?;
        std::env::split_paths(&paths)
            .map(|dir| dir.join(name))
            .find(|p| p.is_file())
    };
    if let Some(rc) = on_path("rc.exe") {
        return Some((rc, false));
    }
    if let Some(base) = std::env::var_os("ProgramFiles(x86)") {
        let base = PathBuf::from(base).join("Windows Kits/10/bin");
        let mut versions: Vec<PathBuf> = std::fs::read_dir(base)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path().join("x64/rc.exe"))
            .filter(|p| p.is_file())
            .collect();
        versions.sort();
        if let Some(rc) = versions.pop() {
            return Some((rc, false));
        }
    }
    println!("cargo:rerun-if-env-changed=MD_LLVM_RC");
    if let Some(rc) = std::env::var_os("MD_LLVM_RC").map(PathBuf::from) {
        return rc.is_file().then_some((rc, true));
    }
    on_path("llvm-rc").map(|rc| (rc, true))
}
