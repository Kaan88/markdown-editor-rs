//! Reading a file into a [`Buffer`]: streamed in [`READ_CHUNK`](crate::limits::READ_CHUNK)
//! pieces, decoded, counted for line endings and fed to a rope builder in one pass, so the file's
//! bytes are never held whole beside the rope. Runs on a worker thread in the shell; nothing here
//! touches a window.

use crate::buffer::Buffer;
use crate::encoding::Encoding;
use crate::eol::EolCounts;
use mdshared_app::files::FileStamp;
use std::io::Read;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

/// What the caller decides about a load.
#[derive(Clone, Copy, Debug)]
pub struct LoadOptions {
    /// The code page for files that are not valid UTF-8 and have no byte order mark. The shell
    /// passes the system's (`GetACP`, through `encoding::legacy_for_code_page`); tests pass
    /// windows-1254 explicitly.
    pub legacy: &'static encoding_rs::Encoding,
    /// Files larger than this on disk are refused.
    pub max_bytes: u64,
}

impl Default for LoadOptions {
    /// windows-1252 and [`MAX_FILE_BYTES`](crate::limits::MAX_FILE_BYTES).
    fn default() -> LoadOptions {
        todo!()
    }
}

/// Facts about the file that was read, kept beside its buffer.
#[derive(Clone, Debug)]
pub struct FileInfo {
    pub encoding: Encoding,
    pub eol: EolCounts,
    /// Some input could not be decoded and was replaced by U+FFFD (invalid UTF-8 after a UTF-8
    /// byte order mark, unpaired UTF-16 surrogates, an odd UTF-16 byte count). Such a file stays
    /// read-only once editing exists, so it is never saved repaired.
    pub lossy: bool,
    /// The file holds U+0000 characters (shown as control pictures).
    pub has_nul: bool,
    /// Size on disk, in bytes.
    pub file_bytes: u64,
    /// The stamp taken before reading; a load that saw the stamp change while reading retries.
    pub stamp: FileStamp,
    /// The most characters any line has (line break excluded, a tab counted as
    /// `TAB_WIDTH` characters), for the horizontal scroll range before lines are shaped.
    pub longest_line_chars: usize,
    /// Wall time of the load, for `--trace`.
    pub elapsed: Duration,
}

pub struct Loaded {
    pub buffer: Buffer,
    pub info: FileInfo,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoadError {
    NotFound,
    IsDirectory,
    TooLarge {
        bytes: u64,
        max: u64,
    },
    /// The file kept changing over three attempts.
    StillChanging,
    /// The `cancel` flag was set.
    Cancelled,
    Io(String),
}

impl std::fmt::Display for LoadError {
    /// One sentence for the text area of a tab that could not be opened.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let _ = f;
        todo!()
    }
}

impl std::error::Error for LoadError {}

/// Reads `path`. `progress(done, total)` is called at most every
/// [`PROGRESS_STEP`](crate::limits::PROGRESS_STEP) bytes; `cancel` is checked between chunks.
/// Retries up to three times while the file's stamp changes during the read.
pub fn load_path(
    path: &Path,
    opts: &LoadOptions,
    progress: &dyn Fn(u64, u64),
    cancel: &AtomicBool,
) -> Result<Loaded, LoadError> {
    let _ = (path, opts, progress, cancel);
    todo!()
}

/// The same decoding as [`load_path`] over any reader, which may return pieces of any size
/// (tests feed one to seven bytes at a time). `total` is the size if known, for progress.
/// `stamp` is `FileStamp::Missing` and `elapsed` the decode time.
pub fn load_reader(
    reader: &mut dyn Read,
    total: Option<u64>,
    opts: &LoadOptions,
    progress: &dyn Fn(u64, u64),
    cancel: &AtomicBool,
) -> Result<Loaded, LoadError> {
    let _ = (reader, total, opts, progress, cancel);
    todo!()
}

/// [`load_reader`] over a byte slice, which cannot fail.
pub fn load_bytes(bytes: &[u8], opts: &LoadOptions) -> Loaded {
    let _ = (bytes, opts);
    todo!()
}
