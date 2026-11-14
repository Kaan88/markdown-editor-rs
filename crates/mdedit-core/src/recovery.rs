//! The recovery copy of an unsaved document: one file that holds its text and what is needed to
//! put it back after a crash (the file it came from, its encoding and line ending, and the
//! file's size and time when it was read). Headless: the shell decides when to write and run
//! the thread; this module only knows the format.
//!
//! ```text
//! mdedit-recovery 1
//! path=<the file, escaped; empty for a document that has none>
//! untitled=<its number, 0 for a file>
//! encoding=UTF-8
//! eol=Unix (LF)
//! disk=<size>/<seconds>.<nanoseconds>     (omitted when there was no file to compare with)
//! bytes=<length of the text>
//! ---
//! <the text, UTF-8, as it is in the buffer>
//! ```
//!
//! A file is read back only when the text after the header has exactly `bytes` bytes, so a copy
//! cut short (a power loss between the data and the rename on a file system that reorders them)
//! is dropped and never offered.

use crate::Encoding;
use mdshared_app::files::FileStamp;
use mdshared_app::settings::{decode_path, encode_path};
use ropey::Rope;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

/// The first line of a recovery file.
pub const MAGIC: &str = "mdedit-recovery 1";
/// The extension of a recovery file; the temporary name while one is written adds `.tmp`.
pub const EXTENSION: &str = "rec";
/// A header longer than this is not one.
const HEADER_MAX: usize = 64 * 1024;
const SEPARATOR: &str = "---\n";

/// What a recovery file says about its text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Header {
    /// The file the text came from; `None` for a document that has no file.
    pub path: Option<PathBuf>,
    /// The number in the name of a document without a file ("Untitled 2"), else 0.
    pub untitled: u32,
    /// The encoding the document saves in, as [`Encoding::label`] names it.
    pub encoding: String,
    /// The line ending, as [`crate::LineEnding::label`] names it (for the record).
    pub eol: String,
    /// The size and modification time of the file when it was read, when it had been.
    pub disk: Option<(u64, Duration)>,
    /// The length of the text, in bytes.
    pub bytes: u64,
}

impl Header {
    /// The disk stamp the header holds, as the loader makes them.
    pub fn stamp(&self) -> Option<FileStamp> {
        let (size, since) = self.disk?;
        Some(FileStamp::Present {
            modified: Some(UNIX_EPOCH + since),
            size,
        })
    }

    /// Records `stamp` as the file's (nothing for a missing file or one without a time).
    pub fn set_stamp(&mut self, stamp: &FileStamp) {
        self.disk = match stamp {
            FileStamp::Present {
                modified: Some(m),
                size,
            } => m.duration_since(UNIX_EPOCH).ok().map(|d| (*size, d)),
            _ => None,
        };
    }

    /// The text of the header, ending with the separator line.
    pub fn encode(&self) -> String {
        let path = self.path.as_deref().map(encode_path).unwrap_or_default();
        let mut s = format!(
            "{MAGIC}\npath={path}\nuntitled={}\nencoding={}\neol={}\n",
            self.untitled,
            one_line(&self.encoding),
            one_line(&self.eol)
        );
        if let Some((size, d)) = self.disk {
            s.push_str(&format!(
                "disk={size}/{}.{:09}\n",
                d.as_secs(),
                d.subsec_nanos()
            ));
        }
        s.push_str(&format!("bytes={}\n{SEPARATOR}", self.bytes));
        s
    }

    /// Reads a header from the start of `data`; returns it and the length of the header's text.
    pub fn parse(data: &[u8]) -> Option<(Header, usize)> {
        let end = find(data, SEPARATOR.as_bytes())? + SEPARATOR.len();
        let head = std::str::from_utf8(&data[..end - SEPARATOR.len()]).ok()?;
        let mut lines = head.split('\n');
        if lines.next()? != MAGIC {
            return None;
        }
        let mut h = Header {
            path: None,
            untitled: 0,
            encoding: String::new(),
            eol: String::new(),
            disk: None,
            bytes: 0,
        };
        let mut have_bytes = false;
        for line in lines.filter(|l| !l.is_empty()) {
            let (key, value) = line.split_once('=')?;
            match key {
                "path" => h.path = (!value.is_empty()).then(|| decode_path(value)),
                "untitled" => h.untitled = value.parse().ok()?,
                "encoding" => h.encoding = value.to_string(),
                "eol" => h.eol = value.to_string(),
                "disk" => {
                    let (size, time) = value.split_once('/')?;
                    let (secs, nanos) = time.split_once('.')?;
                    let nanos: u32 = nanos.parse().ok().filter(|n| *n < 1_000_000_000)?;
                    h.disk = Some((size.parse().ok()?, Duration::new(secs.parse().ok()?, nanos)));
                }
                "bytes" => {
                    h.bytes = value.parse().ok()?;
                    have_bytes = true;
                }
                // A key from a newer version is skipped.
                _ => {}
            }
        }
        have_bytes.then_some((h, end))
    }
}

fn one_line(s: &str) -> String {
    s.replace(['\n', '\r'], " ")
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    // The separator is at the start of a line: after the last header line's break.
    hay.windows(needle.len())
        .enumerate()
        .find(|(i, w)| *w == needle && *i > 0 && hay[i - 1] == b'\n')
        .map(|(i, _)| i)
}

/// The encoding a label names: the ones [`Encoding::label`] gives, and any code page the
/// `encoding_rs` labels know.
pub fn encoding_from_label(label: &str) -> Option<Encoding> {
    Some(match label {
        "UTF-8" => Encoding::Utf8,
        "UTF-8 with BOM" => Encoding::Utf8Bom,
        "UTF-16 LE" => Encoding::Utf16Le,
        "UTF-16 BE" => Encoding::Utf16Be,
        other => Encoding::Legacy(encoding_rs::Encoding::for_label(other.as_bytes())?),
    })
}

/// Writes the recovery file `name` in `dir` (made if needed): the header and `text`, to a
/// temporary name, synced, then renamed over the old copy, so a copy is whole or absent.
/// `header.bytes` is set from `text`.
pub fn write(dir: &Path, name: &str, header: &Header, text: &Rope) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let mut header = header.clone();
    header.bytes = text.len_bytes() as u64;
    let (tmp, path) = (dir.join(format!("{name}.tmp")), dir.join(name));
    let result = (|| {
        let mut w = io::BufWriter::new(std::fs::File::create(&tmp)?);
        w.write_all(header.encode().as_bytes())?;
        for chunk in text.chunks() {
            w.write_all(chunk.as_bytes())?;
        }
        w.into_inner().map_err(|e| e.into_error())?.sync_all()?;
        std::fs::rename(&tmp, &path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// The header of the recovery file at `path`, when it is a whole one (its text has the length
/// the header says). Reads only the start of the file.
pub fn read_header(path: &Path) -> Option<Header> {
    let mut file = std::fs::File::open(path).ok()?;
    let total = file.metadata().ok()?.len();
    let mut head = Vec::new();
    (&mut file)
        .take(HEADER_MAX as u64)
        .read_to_end(&mut head)
        .ok()?;
    let (h, len) = Header::parse(&head)?;
    (total == len as u64 + h.bytes).then_some(h)
}

/// The header and the text of the recovery file at `path`; `None` unless it is whole and its
/// text is UTF-8.
pub fn read(path: &Path) -> Option<(Header, String)> {
    let data = std::fs::read(path).ok()?;
    let (h, len) = Header::parse(&data[..data.len().min(HEADER_MAX)])?;
    if data.len() as u64 != len as u64 + h.bytes {
        return None;
    }
    let text = String::from_utf8(data[len..].to_vec()).ok()?;
    Some((h, text))
}

/// Whether `name` is the name of a recovery file.
pub fn is_recovery_name(name: &str) -> bool {
    name.strip_suffix(EXTENSION)
        .is_some_and(|stem| stem.ends_with('.') && stem.len() > 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header() -> Header {
        Header {
            path: Some(PathBuf::from("/tmp/a b/notes.md")),
            untitled: 0,
            encoding: "Windows-1254".into(),
            eol: "Windows (CRLF)".into(),
            disk: Some((1234, Duration::new(1_700_000_000, 5))),
            bytes: 0,
        }
    }

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("mdedit-rec-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn a_header_round_trips() {
        let mut h = header();
        h.bytes = 42;
        let text = h.encode();
        let (back, len) = Header::parse(text.as_bytes()).unwrap();
        assert_eq!((back, len), (h, text.len()));
        // No file, no stamp; a path with a line break in it survives.
        let h = Header {
            path: Some(PathBuf::from("a\nb")),
            untitled: 3,
            disk: None,
            ..header()
        };
        assert_eq!(Header::parse(h.encode().as_bytes()).unwrap().0, h);
        let h = Header { path: None, ..h };
        assert_eq!(Header::parse(h.encode().as_bytes()).unwrap().0.path, None);
    }

    #[test]
    fn damaged_headers_are_refused() {
        assert!(Header::parse(b"").is_none());
        assert!(Header::parse(b"mdedit-recovery 2\nbytes=1\n---\n").is_none());
        assert!(Header::parse(b"mdedit-recovery 1\npath=\n---\n").is_none());
        assert!(Header::parse(b"mdedit-recovery 1\nbytes=x\n---\n").is_none());
        assert!(Header::parse(b"mdedit-recovery 1\nbytes=1\ndisk=1/2\n---\n").is_none());
        // The separator must start a line: a text that holds "---" cannot end the header early.
        assert!(Header::parse(b"mdedit-recovery 1\nbytes=1").is_none());
        // A key from a newer version is skipped.
        assert!(Header::parse(b"mdedit-recovery 1\nfuture=1\nbytes=1\n---\n").is_some());
    }

    #[test]
    fn a_copy_is_written_and_read_back_whole() {
        let d = dir("whole");
        let text = Rope::from_str("one\r\ntwo\n---\nthree \u{e9}");
        write(&d, "1.rec", &header(), &text).unwrap();
        assert_eq!(
            std::fs::read_dir(&d).unwrap().count(),
            1,
            "no temporary file stays"
        );
        let (h, body) = read(&d.join("1.rec")).unwrap();
        assert_eq!(body, text.to_string());
        assert_eq!(h.bytes, text.len_bytes() as u64);
        assert_eq!(h.path, header().path);
        assert_eq!(read_header(&d.join("1.rec")), Some(h.clone()));
        assert_eq!(
            h.stamp(),
            Some(FileStamp::Present {
                modified: Some(UNIX_EPOCH + Duration::new(1_700_000_000, 5)),
                size: 1234
            })
        );
        // Written again over the old copy.
        write(&d, "1.rec", &header(), &Rope::from_str("x")).unwrap();
        assert_eq!(read(&d.join("1.rec")).unwrap().1, "x");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_cut_short_or_longer_copy_is_dropped() {
        let d = dir("cut");
        write(&d, "1.rec", &header(), &Rope::from_str("some text")).unwrap();
        let full = std::fs::read(d.join("1.rec")).unwrap();
        std::fs::write(d.join("cut.rec"), &full[..full.len() - 1]).unwrap();
        assert!(read(&d.join("cut.rec")).is_none() && read_header(&d.join("cut.rec")).is_none());
        let mut longer = full.clone();
        longer.push(b'!');
        std::fs::write(d.join("long.rec"), longer).unwrap();
        assert!(read(&d.join("long.rec")).is_none());
        std::fs::write(d.join("junk.rec"), b"hello").unwrap();
        assert!(read(&d.join("junk.rec")).is_none());
        assert!(read(&d.join("none.rec")).is_none());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn encodings_come_back_from_their_labels() {
        for e in [
            Encoding::Utf8,
            Encoding::Utf8Bom,
            Encoding::Utf16Le,
            Encoding::Utf16Be,
            Encoding::Legacy(encoding_rs::WINDOWS_1254),
            Encoding::Legacy(encoding_rs::SHIFT_JIS),
        ] {
            assert_eq!(encoding_from_label(&e.label()), Some(e), "{}", e.label());
        }
        assert_eq!(encoding_from_label("nonsense"), None);
    }

    #[test]
    fn recovery_names() {
        assert!(is_recovery_name("a-1.rec"));
        assert!(
            !is_recovery_name("a-1.rec.tmp")
                && !is_recovery_name("rec")
                && !is_recovery_name(".rec")
        );
        assert!(!is_recovery_name("1.txt"));
    }
}
