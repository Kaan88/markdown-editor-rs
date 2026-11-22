//! A stderr that is gone is not a reason to die. The release build aborts on a panic, and
//! `std`'s `eprintln!` panics when the write fails ("failed printing to stderr", a closed pipe:
//! os error 232 on Windows, EPIPE elsewhere), so one line of `--trace` after the reader went
//! away used to end mdedit. Found on 2026-11-22 when a shell closed a test run's pipe.

use std::process::{Command, Stdio};

#[test]
fn a_run_whose_stderr_is_closed_still_finishes() {
    let out = std::env::temp_dir().join(format!("mdedit-closed-stderr-{}", std::process::id()));
    std::fs::create_dir_all(&out).unwrap();
    let settings = out.join("settings.ini");
    std::fs::write(&settings, "").unwrap();
    let sample = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/sample.rs");
    let mut child = Command::new(env!("CARGO_BIN_EXE_mdedit"))
        .args([
            "--behind",
            "--no-session",
            "--trace",
            "--size",
            "640x400",
            "--shot",
        ])
        .arg(out.join("shot.png"))
        .arg(sample)
        .env("MDEDIT_SETTINGS", &settings)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("mdedit starts");
    // Close the reading end before mdedit has written anything: every trace line it prints
    // from here on goes to a pipe nobody reads.
    drop(child.stderr.take());
    let status = child.wait().expect("mdedit runs");
    let _ = std::fs::remove_dir_all(&out);
    assert!(
        status.success(),
        "mdedit ended with {status} when its stderr was closed"
    );
}
