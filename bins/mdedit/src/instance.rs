//! One window per settings folder. The window that has the folder holds a lock on a file beside
//! the settings for as long as it runs; a launch that cannot take it hands its files to that
//! window, waiting for it to listen if it is still starting (audit 2: two launches close
//! together, as Explorer makes for several files, each ran a window on the same session, and
//! each exit dropped the other's kept drafts).

use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// How long a launch waits for a window that holds the folder to start listening.
pub const WAIT: Duration = Duration::from_secs(10);

/// What a launch is to do.
#[derive(Debug)]
pub enum Claim {
    /// This launch has the folder: the lock is held while the file is open. `None` when no lock
    /// file could be made (a folder that cannot be written): as before, with no lock.
    Own(Option<File>),
    /// The window that has the folder took the files.
    Handed,
    /// Another window holds the folder but took nothing in time (hung, or busy for long): this
    /// one runs without the session, so it cannot destroy that window's.
    Busy,
}

/// The lock file of a settings file.
pub fn lock_path(settings: &Path) -> PathBuf {
    let mut p = settings.as_os_str().to_owned();
    p.push(".lock");
    PathBuf::from(p)
}

/// Takes the folder for this launch, or hands over with `forward` (true when a running window
/// took the files). A running window is tried first: one from before these locks holds none.
pub fn claim(settings: &Path, mut forward: impl FnMut() -> bool, wait: Duration) -> Claim {
    if forward() {
        return Claim::Handed;
    }
    let path = lock_path(settings);
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let Ok(file) = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)
    else {
        return Claim::Own(None);
    };
    let end = Instant::now() + wait;
    loop {
        match file.try_lock() {
            Ok(()) => return Claim::Own(Some(file)),
            Err(TryLockError::Error(_)) => return Claim::Own(None),
            Err(TryLockError::WouldBlock) => {}
        }
        // Held: the window that has it may not listen yet.
        if forward() {
            return Claim::Handed;
        }
        if Instant::now() >= end {
            return Claim::Busy;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("mdedit-instance-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn the_second_launch_waits_for_the_first_to_listen_and_hands_over() {
        let d = dir("two");
        let settings = d.join("settings.ini");
        let first = claim(&settings, || false, WAIT);
        assert!(matches!(first, Claim::Own(Some(_))));
        // The first is still starting for a while, then listens.
        let mut tries = 0;
        let second = claim(
            &settings,
            || {
                tries += 1;
                tries > 3
            },
            WAIT,
        );
        assert!(matches!(second, Claim::Handed), "{second:?}");
        assert!(tries > 3);
        drop(first);
        // Once it is gone the folder is free.
        assert!(matches!(
            claim(&settings, || false, WAIT),
            Claim::Own(Some(_))
        ));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_window_that_never_listens_leaves_the_next_launch_without_the_session() {
        let d = dir("busy");
        let settings = d.join("settings.ini");
        let _first = claim(&settings, || false, WAIT);
        let second = claim(&settings, || false, Duration::from_millis(200));
        assert!(matches!(second, Claim::Busy), "{second:?}");
        let _ = std::fs::remove_dir_all(&d);
    }
}
