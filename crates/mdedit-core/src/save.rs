//! Writing a buffer back to a file without losing a byte that was not edited.
//!
//! The rope keeps every line break as the file had it and the encoding is known from the load, so
//! saving is: encode the text in that encoding (byte order mark first where it has one) and
//! replace the file safely. Nothing is normalised: no line ending is changed, no final newline
//! is added. The text goes to a temporary file beside the target, which is flushed and synced
//! and then swapped in (`ReplaceFileW` on Windows, so the target keeps its attributes, ACL,
//! creation time and alternate streams; a rename elsewhere), so a failed save leaves the target
//! as it was.
//!
//! The one `unsafe` block of the crate is the `ReplaceFileW` call in `sys`.

use crate::encoding::Encoding;
use mdshared_app::files::{FileStamp, stamp};
use ropey::Rope;
use std::fs::OpenOptions;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// How many bytes are written between two looks at the `cancel` flag.
const CANCEL_STEP: usize = 64 * 1024;

#[derive(Debug)]
pub enum SaveError {
    /// A character the encoding cannot hold. Nothing was written to the target. `line` is
    /// 0-based and `col` counts characters from the line start.
    Unencodable {
        ch: char,
        line: usize,
        col: usize,
    },
    /// The file on disk is not the one that was loaded (its stamp differs from `expected`).
    ChangedOnDisk,
    /// The target is read-only (attribute or permission).
    ReadOnly,
    Cancelled,
    Io(String),
}

impl std::fmt::Display for SaveError {
    /// One sentence a user can read.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SaveError::Unencodable { ch, line, col } => write!(
                f,
                "The character '{ch}' (U+{:04X}) at line {}, column {} cannot be stored in this \
                 file's encoding.",
                *ch as u32,
                line + 1,
                col + 1
            ),
            SaveError::ChangedOnDisk => {
                write!(f, "The file was changed on disk after it was opened.")
            }
            SaveError::ReadOnly => write!(f, "The file is read-only."),
            SaveError::Cancelled => write!(f, "Saving was cancelled."),
            SaveError::Io(msg) => write!(f, "The file could not be saved: {msg}."),
        }
    }
}

impl std::error::Error for SaveError {}

impl From<std::io::Error> for SaveError {
    fn from(e: std::io::Error) -> SaveError {
        SaveError::Io(e.to_string())
    }
}

/// What a successful save leaves behind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Saved {
    /// The stamp of the file as written, to compare against on the next save.
    pub stamp: FileStamp,
    /// Bytes written, byte order mark included.
    pub bytes: u64,
}

/// Why an encode stopped; `Unmappable` carries the character and its byte offset in the text.
enum Stop {
    Unmappable { ch: char, offset: usize },
    Cancelled,
    Io(std::io::Error),
}

impl From<std::io::Error> for Stop {
    fn from(e: std::io::Error) -> Stop {
        Stop::Io(e)
    }
}

/// Counts bytes and looks at the cancel flag every [`CANCEL_STEP`] of them.
struct Pacer<'a> {
    cancel: &'a AtomicBool,
    since: usize,
}

impl Pacer<'_> {
    fn advance(&mut self, bytes: usize) -> Result<(), Stop> {
        self.since += bytes;
        if self.since >= CANCEL_STEP {
            self.since = 0;
            if self.cancel.load(Ordering::Relaxed) {
                return Err(Stop::Cancelled);
            }
        }
        Ok(())
    }
}

fn encode_inner(
    rope: &Rope,
    encoding: Encoding,
    out: &mut dyn Write,
    cancel: &AtomicBool,
) -> Result<u64, Stop> {
    if cancel.load(Ordering::Relaxed) {
        return Err(Stop::Cancelled);
    }
    let mut pacer = Pacer { cancel, since: 0 };
    let mut written: u64 = 0;
    match encoding {
        Encoding::Utf8 | Encoding::Utf8Bom => {
            if encoding == Encoding::Utf8Bom {
                out.write_all(&[0xEF, 0xBB, 0xBF])?;
                written += 3;
            }
            for chunk in rope.chunks() {
                out.write_all(chunk.as_bytes())?;
                written += chunk.len() as u64;
                pacer.advance(chunk.len())?;
            }
        }
        Encoding::Utf16Le | Encoding::Utf16Be => {
            let le = encoding == Encoding::Utf16Le;
            let mut buf: Vec<u8> = Vec::with_capacity(CANCEL_STEP + 8);
            buf.extend_from_slice(if le { &[0xFF, 0xFE] } else { &[0xFE, 0xFF] });
            // A `&str` chunk is whole characters (ropey never cuts one), so no carry is needed.
            for chunk in rope.chunks() {
                for c in chunk.chars() {
                    let mut units = [0u16; 2];
                    for unit in c.encode_utf16(&mut units) {
                        buf.extend_from_slice(&if le {
                            unit.to_le_bytes()
                        } else {
                            unit.to_be_bytes()
                        });
                    }
                }
                if buf.len() >= CANCEL_STEP {
                    out.write_all(&buf)?;
                    written += buf.len() as u64;
                    pacer.advance(buf.len())?;
                    buf.clear();
                }
            }
            out.write_all(&buf)?;
            written += buf.len() as u64;
        }
        Encoding::Legacy(enc) => {
            let mut encoder = enc.new_encoder();
            let mut buf: Vec<u8> = Vec::new();
            let mut base = 0usize;
            for chunk in rope.chunks() {
                let mut src = chunk;
                loop {
                    buf.clear();
                    let need = encoder
                        .max_buffer_length_from_utf8_without_replacement(src.len())
                        .unwrap_or(src.len().saturating_mul(4) + 16);
                    buf.reserve(need);
                    let (result, read) =
                        encoder.encode_from_utf8_to_vec_without_replacement(src, &mut buf, false);
                    out.write_all(&buf)?;
                    written += buf.len() as u64;
                    match result {
                        encoding_rs::EncoderResult::InputEmpty => {
                            base += src.len();
                            break;
                        }
                        encoding_rs::EncoderResult::OutputFull => {
                            base += read;
                            src = &src[read..];
                        }
                        encoding_rs::EncoderResult::Unmappable(ch) => {
                            return Err(Stop::Unmappable {
                                ch,
                                offset: base + read - ch.len_utf8(),
                            });
                        }
                    }
                }
                pacer.advance(chunk.len())?;
            }
            // The final empty call with `last` set flushes a stateful encoder (ISO-2022-JP).
            buf.clear();
            buf.reserve(16);
            let (result, _) =
                encoder.encode_from_utf8_to_vec_without_replacement("", &mut buf, true);
            debug_assert_eq!(result, encoding_rs::EncoderResult::InputEmpty);
            out.write_all(&buf)?;
            written += buf.len() as u64;
        }
    }
    Ok(written)
}

/// 0-based line and character column of a byte offset, lines broken as the buffer breaks them.
fn line_col(rope: &Rope, offset: usize) -> (usize, usize) {
    let line = rope.byte_to_line(offset);
    let col = rope.byte_to_char(offset) - rope.line_to_char(line);
    (line, col)
}

fn stop_to_error(rope: &Rope, stop: Stop) -> SaveError {
    match stop {
        Stop::Unmappable { ch, offset } => {
            let (line, col) = line_col(rope, offset);
            SaveError::Unencodable { ch, line, col }
        }
        Stop::Cancelled => SaveError::Cancelled,
        Stop::Io(e) => SaveError::Io(e.to_string()),
    }
}

/// Streams the rope's text to `out` in `encoding`, byte order mark included, and returns the
/// bytes written. Checks `cancel` at least every 64 KiB. A character the encoding cannot hold
/// stops it with [`SaveError::Unencodable`]; `out` may then hold the text before it.
pub fn encode_to(
    rope: &Rope,
    encoding: Encoding,
    out: &mut dyn Write,
    cancel: &AtomicBool,
) -> Result<u64, SaveError> {
    encode_inner(rope, encoding, out, cancel).map_err(|stop| stop_to_error(rope, stop))
}

/// The first character `encoding` cannot hold, with its byte offset in the text, if any (for
/// asking before a save). The UTF encodings hold every character: always `None`.
pub fn first_unencodable(rope: &Rope, encoding: Encoding) -> Option<(char, usize)> {
    if !matches!(encoding, Encoding::Legacy(_)) {
        return None;
    }
    let never = AtomicBool::new(false);
    match encode_inner(rope, encoding, &mut std::io::sink(), &never) {
        Err(Stop::Unmappable { ch, offset }) => Some((ch, offset)),
        _ => None,
    }
}

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// A name beside `target` that cannot collide with another save or another process.
fn temp_path(target: &Path) -> PathBuf {
    let mut name = target.file_name().unwrap_or_default().to_os_string();
    name.push(format!(
        ".~mdedit-{}-{}.tmp",
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    target.with_file_name(name)
}

/// Writes the text to `temp`, flushed and synced.
fn write_temp(
    temp: &Path,
    rope: &Rope,
    encoding: Encoding,
    cancel: &AtomicBool,
) -> Result<u64, SaveError> {
    let file = OpenOptions::new().write(true).create_new(true).open(temp)?;
    let mut writer = BufWriter::with_capacity(CANCEL_STEP, file);
    let bytes = encode_to(rope, encoding, &mut writer, cancel)?;
    writer.flush()?;
    let file = writer.into_inner().map_err(|e| e.into_error())?;
    file.sync_all()?;
    Ok(bytes)
}

/// Puts `temp` in place of `target`, which keeps its attributes where the system allows.
fn replace(target: &Path, temp: &Path) -> Result<(), SaveError> {
    let exists = target.exists();
    #[cfg(windows)]
    if exists {
        match sys::replace_file(target, temp) {
            sys::Replace::Done => return Ok(()),
            // Network shares and some file systems cannot do it: a rename still can.
            sys::Replace::Unsupported => {}
            sys::Replace::Failed(e) => return Err(e.into()),
        }
    }
    #[cfg(not(windows))]
    if exists {
        let perms = std::fs::metadata(target)?.permissions();
        std::fs::set_permissions(temp, perms)?;
    }
    let _ = exists;
    std::fs::rename(temp, target)?;
    Ok(())
}

/// Writes the text to `path`, safely (see the module doc). `expected` is the stamp the caller
/// last saw for the file; `None` skips the check (Save As, or an explicit overwrite). A file
/// that was there and is now missing counts as changed; one missing at load and still missing
/// does not.
pub fn save_path(
    path: &Path,
    rope: &Rope,
    encoding: Encoding,
    expected: Option<&FileStamp>,
    cancel: &AtomicBool,
) -> Result<Saved, SaveError> {
    if let Some(expected) = expected
        && stamp(path) != *expected
    {
        return Err(SaveError::ChangedOnDisk);
    }
    if let Ok(meta) = std::fs::metadata(path)
        && meta.permissions().readonly()
    {
        return Err(SaveError::ReadOnly);
    }
    let temp = temp_path(path);
    let result = write_temp(&temp, rope, encoding, cancel).and_then(|bytes| {
        replace(path, &temp)?;
        Ok(bytes)
    });
    match result {
        Ok(bytes) => Ok(Saved {
            stamp: stamp(path),
            bytes,
        }),
        Err(e) => {
            let _ = std::fs::remove_file(&temp);
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(windows)]
mod sys {
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;
    use windows_sys::Win32::Foundation::{
        ERROR_CALL_NOT_IMPLEMENTED, ERROR_INVALID_FUNCTION, ERROR_INVALID_PARAMETER,
        ERROR_NOT_SUPPORTED,
    };
    use windows_sys::Win32::Storage::FileSystem::{REPLACEFILE_IGNORE_MERGE_ERRORS, ReplaceFileW};

    pub enum Replace {
        Done,
        /// The file system does not support `ReplaceFileW` (as opposed to refusing this call).
        Unsupported,
        Failed(std::io::Error),
    }

    fn wide(path: &Path) -> Vec<u16> {
        path.as_os_str().encode_wide().chain(Some(0)).collect()
    }

    /// Replaces `target` with `temp`, keeping the target's attributes, ACL, creation time and
    /// alternate streams.
    pub fn replace_file(target: &Path, temp: &Path) -> Replace {
        let target = wide(target);
        let temp = wide(temp);
        // SAFETY: both paths are NUL-terminated UTF-16 buffers that live until the call returns;
        // the backup name, the exclude and the reserved arguments are documented as nullable.
        let ok = unsafe {
            ReplaceFileW(
                target.as_ptr(),
                temp.as_ptr(),
                std::ptr::null(),
                REPLACEFILE_IGNORE_MERGE_ERRORS,
                std::ptr::null(),
                std::ptr::null(),
            )
        };
        if ok != 0 {
            return Replace::Done;
        }
        let err = std::io::Error::last_os_error();
        match err.raw_os_error().map(|code| code as u32) {
            Some(
                ERROR_INVALID_FUNCTION
                | ERROR_NOT_SUPPORTED
                | ERROR_CALL_NOT_IMPLEMENTED
                | ERROR_INVALID_PARAMETER,
            ) => Replace::Unsupported,
            _ => Replace::Failed(err),
        }
    }
}
