//! The Recycle Bin as the window uses it (`docs/ai-library-plan.md` 5.4, D-046): the plan card
//! asks whether the source's drive has one, step 8 of a split's write puts the original in it, and
//! undo's removals go to it. The system's bin is `mdshared_ui::platform::recycle`; everything here
//! goes through [`RecycleBin`], so a test, or a shot, is given a bin of its own and the system's
//! is never touched by one.

use mdedit_core::runs;
use std::path::Path;

/// What the window asks of a bin.
pub trait RecycleBin {
    /// Why `path` cannot go in, in the plan card's words; `None` when it can.
    fn refusal(&self, path: &Path) -> Option<String>;
    /// Puts the file in the bin; `Err` says why it did not go, and it is then where it was.
    fn recycle(&mut self, path: &Path) -> Result<(), String>;
    /// Opens the bin in the file manager; false where that cannot be done.
    fn open(&self) -> bool;
    /// Whether [`RecycleBin::open`] does anything here: the undo question offers the button only
    /// then.
    fn can_open(&self) -> bool;
}

/// The system's Recycle Bin. A test window never has it.
#[cfg_attr(test, allow(dead_code))]
pub struct SystemBin;

impl RecycleBin for SystemBin {
    fn refusal(&self, path: &Path) -> Option<String> {
        crate::platform::recycle_refusal(path).map(|r| r.text())
    }

    fn recycle(&mut self, path: &Path) -> Result<(), String> {
        crate::platform::recycle(path)
    }

    fn open(&self) -> bool {
        crate::platform::open_recycle_bin()
    }

    fn can_open(&self) -> bool {
        cfg!(windows)
    }
}

/// What the window holds while its bin is lent out (undo holds the window and the bin at once):
/// a bin that takes nothing.
pub struct Taken;

impl RecycleBin for Taken {
    fn refusal(&self, _path: &Path) -> Option<String> {
        Some(crate::plan_ui::recycle_unavailable("it is in use"))
    }

    fn recycle(&mut self, _path: &Path) -> Result<(), String> {
        Err("it is in use".to_string())
    }

    fn open(&self) -> bool {
        false
    }

    fn can_open(&self) -> bool {
        false
    }
}

/// The bin a test window has: none at all, so no test can reach the system's.
#[cfg(test)]
pub struct NoBin;

#[cfg(test)]
impl RecycleBin for NoBin {
    fn refusal(&self, _path: &Path) -> Option<String> {
        Some(crate::plan_ui::recycle_unavailable("a test has none"))
    }

    fn recycle(&mut self, _path: &Path) -> Result<(), String> {
        Err("a test has none".to_string())
    }

    fn open(&self) -> bool {
        false
    }

    fn can_open(&self) -> bool {
        false
    }
}

/// A test's own bin: a folder the files are moved into, the calls it was given, and what it
/// says about a drive or a file when told to.
#[cfg(test)]
pub struct FolderBin {
    pub into: std::path::PathBuf,
    pub calls: std::rc::Rc<std::cell::RefCell<Vec<std::path::PathBuf>>>,
    pub refusal: Option<String>,
    pub refuse: Option<String>,
}

#[cfg(test)]
impl FolderBin {
    pub fn new(into: &Path) -> FolderBin {
        std::fs::create_dir_all(into).expect("the test's bin");
        FolderBin {
            into: into.to_path_buf(),
            calls: Default::default(),
            refusal: None,
            refuse: None,
        }
    }

    /// Where `path` is once it is in this bin.
    pub fn held(into: &Path, path: &Path) -> std::path::PathBuf {
        into.join(path.file_name().unwrap_or_default())
    }
}

#[cfg(test)]
impl RecycleBin for FolderBin {
    fn refusal(&self, _path: &Path) -> Option<String> {
        self.refusal.clone()
    }

    fn recycle(&mut self, path: &Path) -> Result<(), String> {
        self.calls.borrow_mut().push(path.to_path_buf());
        if let Some(why) = &self.refuse {
            return Err(why.clone());
        }
        std::fs::rename(path, FolderBin::held(&self.into, path)).map_err(|e| e.to_string())
    }

    fn open(&self) -> bool {
        false
    }

    fn can_open(&self) -> bool {
        true
    }
}

/// Step 8's bin: the window's, as `runs` asks for one.
pub struct Through<'a>(pub &'a mut dyn RecycleBin);

impl runs::Bin for Through<'_> {
    fn recycle(&mut self, path: &Path) -> Result<(), String> {
        self.0.recycle(path)
    }
}

/// Undo's bin (4.5, rule 3; D-071): the Recycle Bin where the file's drive has one, and a plain
/// delete where it has none, which is what undo did before the bin was built and all such a drive
/// offers. A file the bin refuses is kept, and the report says why.
pub struct ForUndo<'a>(pub &'a mut dyn RecycleBin);

impl runs::Bin for ForUndo<'_> {
    fn recycle(&mut self, path: &Path) -> Result<(), String> {
        if self.0.refusal(path).is_some() {
            return std::fs::remove_file(path).map_err(|e| e.to_string());
        }
        self.0.recycle(path)
    }
}
