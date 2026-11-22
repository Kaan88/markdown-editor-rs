//! The Windows exe carries a version resource (build.rs), which is where Hub reads the version of
//! a copy it did not install, and what Explorer's Details tab shows. Read here from the built
//! exe's bytes, so the test needs no Windows API and no new dependency: the resource's strings
//! are UTF-16, each `key`, a terminating zero, padding to four bytes, then the `value`.

#![cfg(windows)]

fn utf16(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

/// The value stored under `key` in the exe's string table, when there is one.
fn string_value(exe: &[u8], key: &str) -> Option<String> {
    let mut needle = utf16(key);
    needle.extend([0, 0]);
    let at = exe.windows(needle.len()).position(|w| w == needle)?;
    let mut i = at + needle.len();
    while exe.get(i..i + 2) == Some(&[0, 0]) {
        i += 2;
    }
    let units: Vec<u16> = exe[i..]
        .windows(2)
        .step_by(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .take_while(|&u| u != 0)
        .collect();
    Some(String::from_utf16_lossy(&units))
}

#[test]
fn the_exe_carries_its_version() {
    let exe = std::fs::read(env!("CARGO_BIN_EXE_mdedit")).unwrap();
    let version = env!("CARGO_PKG_VERSION");
    for key in ["FileVersion", "ProductVersion"] {
        assert_eq!(string_value(&exe, key).as_deref(), Some(version), "{key}");
    }
    assert_eq!(string_value(&exe, "ProductName").as_deref(), Some("mdedit"));
    assert_eq!(
        string_value(&exe, "OriginalFilename").as_deref(),
        Some("mdedit.exe")
    );
}
