//! Reading a file into a [`Buffer`]: streamed in [`READ_CHUNK`](crate::limits::READ_CHUNK)
//! pieces, decoded, counted for line endings and fed to a rope builder in one pass, so the file's
//! bytes are never held whole beside the rope. Runs on a worker thread in the shell; nothing here
//! touches a window.

use crate::buffer::Buffer;
use crate::encoding::{Encoding, sniff_bom};
use crate::eol::EolCounts;
use crate::limits::{MAX_FILE_BYTES, PROGRESS_STEP, READ_CHUNK, TAB_WIDTH};
use mdshared_app::files::{FileStamp, stamp};
use ropey::RopeBuilder;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

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
        LoadOptions {
            legacy: encoding_rs::WINDOWS_1252,
            max_bytes: MAX_FILE_BYTES,
        }
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
        match self {
            LoadError::NotFound => write!(f, "The file does not exist."),
            LoadError::IsDirectory => write!(f, "This is a folder, not a file."),
            LoadError::TooLarge { bytes, max } => write!(
                f,
                "The file is too large to open ({}; the limit is {}).",
                size_text(*bytes),
                size_text(*max)
            ),
            LoadError::StillChanging => {
                write!(f, "The file kept changing while it was being read.")
            }
            LoadError::Cancelled => write!(f, "Loading was cancelled."),
            LoadError::Io(msg) => write!(f, "The file could not be read: {msg}."),
        }
    }
}

impl std::error::Error for LoadError {}

fn size_text(bytes: u64) -> String {
    const MIB: f64 = (1u64 << 20) as f64;
    if bytes >= 1 << 30 {
        format!("{:.1} GB", bytes as f64 / (1u64 << 30) as f64)
    } else if bytes >= 1 << 20 {
        format!("{:.1} MB", bytes as f64 / MIB)
    } else if bytes >= 1 << 10 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} bytes")
    }
}

// ---------------------------------------------------------------------------------------------
// The sink: everything that is computed on the decoded text.

struct Sink {
    builder: RopeBuilder,
    eol: EolCounts,
    has_nul: bool,
    lossy: bool,
    longest: usize,
    /// Characters of the line in progress (a tab counts `TAB_WIDTH`).
    cur: usize,
    /// The last byte seen was a CR: an LF next belongs to the same break.
    after_cr: bool,
}

/// Characters in a UTF-8 slice, a tab counting `TAB_WIDTH`.
fn line_chars(bytes: &[u8]) -> usize {
    bytes
        .iter()
        .map(|&b| ((b & 0xC0) != 0x80) as usize + (TAB_WIDTH - 1) * (b == b'\t') as usize)
        .sum()
}

impl Sink {
    fn new() -> Sink {
        Sink {
            builder: RopeBuilder::new(),
            eol: EolCounts::default(),
            has_nul: false,
            lossy: false,
            longest: 0,
            cur: 0,
            after_cr: false,
        }
    }

    /// The next piece of decoded text.
    fn push(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let bytes = text.as_bytes();
        self.builder.append(text);
        self.eol.scan(bytes);
        if !self.has_nul {
            self.has_nul = memchr::memchr(0, bytes).is_some();
        }
        self.track_lines(bytes);
    }

    /// One U+FFFD for input that could not be decoded.
    fn replacement(&mut self) {
        self.lossy = true;
        self.push("\u{FFFD}");
    }

    fn track_lines(&mut self, mut b: &[u8]) {
        if self.after_cr {
            self.after_cr = false;
            if b[0] == b'\n' {
                b = &b[1..];
            }
        }
        while !b.is_empty() {
            match memchr::memchr2(b'\n', b'\r', b) {
                Some(j) => {
                    let seg = &b[..j];
                    // A line that starts and ends inside this piece cannot beat the longest
                    // when even four characters per byte would not.
                    if self.cur != 0 || seg.len().saturating_mul(4) > self.longest {
                        self.longest = self.longest.max(self.cur + line_chars(seg));
                    }
                    self.cur = 0;
                    if b[j] == b'\r' {
                        if j + 1 < b.len() {
                            b = if b[j + 1] == b'\n' {
                                &b[j + 2..]
                            } else {
                                &b[j + 1..]
                            };
                        } else {
                            self.after_cr = true;
                            b = &b[j + 1..];
                        }
                    } else {
                        b = &b[j + 1..];
                    }
                }
                None => {
                    self.cur += line_chars(b);
                    break;
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Decoders.

/// Width of the UTF-8 sequence a lead byte starts; 1 for bytes that start none.
fn utf8_width(lead: u8) -> usize {
    match lead {
        0xC2..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xF4 => 4,
        _ => 1,
    }
}

/// How many bytes at the end of `data` are the start of a sequence that is not complete yet.
fn incomplete_tail(data: &[u8]) -> usize {
    let n = data.len();
    for back in 1..=3.min(n) {
        let b = data[n - back];
        if (b & 0xC0) != 0x80 {
            return if utf8_width(b) > back { back } else { 0 };
        }
    }
    0
}

/// Input that is not valid UTF-8, met while the file may still turn out to be in a legacy code
/// page.
struct NotUtf8;

/// UTF-8 decoding with a carry for a sequence cut by the end of a piece. `strict` stops at the
/// first invalid byte (no byte order mark: the file is then read again as a legacy code page);
/// otherwise invalid input becomes U+FFFD.
struct Utf8 {
    carry: [u8; 4],
    carry_len: usize,
    strict: bool,
}

impl Utf8 {
    fn new(strict: bool) -> Utf8 {
        Utf8 {
            carry: [0; 4],
            carry_len: 0,
            strict,
        }
    }

    fn feed(&mut self, mut data: &[u8], sink: &mut Sink) -> Result<(), NotUtf8> {
        if self.carry_len > 0 {
            let need = utf8_width(self.carry[0]) - self.carry_len;
            let take = need.min(data.len());
            let mut tmp = [0u8; 4];
            tmp[..self.carry_len].copy_from_slice(&self.carry[..self.carry_len]);
            tmp[self.carry_len..self.carry_len + take].copy_from_slice(&data[..take]);
            let n = self.carry_len + take;
            if take < need {
                self.carry = tmp;
                self.carry_len = n;
                return Ok(());
            }
            let carried = self.carry_len;
            self.carry_len = 0;
            match std::str::from_utf8(&tmp[..n]) {
                Ok(s) => {
                    sink.push(s);
                    data = &data[take..];
                }
                Err(e) => {
                    if self.strict {
                        return Err(NotUtf8);
                    }
                    sink.replacement();
                    let bad = e.error_len().unwrap_or(n);
                    data = &data[bad.saturating_sub(carried).min(take)..];
                }
            }
        }
        let tail = incomplete_tail(data);
        let (body, rest) = data.split_at(data.len() - tail);
        let mut body = body;
        loop {
            match std::str::from_utf8(body) {
                Ok(s) => {
                    sink.push(s);
                    break;
                }
                Err(e) => {
                    if self.strict {
                        return Err(NotUtf8);
                    }
                    let v = e.valid_up_to();
                    if let Ok(s) = std::str::from_utf8(&body[..v]) {
                        sink.push(s);
                    }
                    sink.replacement();
                    let bad = e.error_len().unwrap_or(body.len() - v);
                    body = &body[v + bad..];
                }
            }
        }
        self.carry[..tail].copy_from_slice(rest);
        self.carry_len = tail;
        Ok(())
    }

    /// The end of the input: a sequence still incomplete is invalid.
    fn finish(&mut self, sink: &mut Sink) -> Result<(), NotUtf8> {
        if self.carry_len > 0 {
            if self.strict {
                return Err(NotUtf8);
            }
            self.carry_len = 0;
            sink.replacement();
        }
        Ok(())
    }
}

/// `encoding_rs` decoding (UTF-16 or a legacy code page), without byte order mark handling.
struct Rs {
    decoder: encoding_rs::Decoder,
    out: String,
}

impl Rs {
    fn new(encoding: &'static encoding_rs::Encoding) -> Rs {
        Rs {
            decoder: encoding.new_decoder_without_bom_handling(),
            out: String::new(),
        }
    }

    fn feed(&mut self, mut data: &[u8], last: bool, sink: &mut Sink) {
        loop {
            self.out.clear();
            let need = self
                .decoder
                .max_utf8_buffer_length(data.len())
                .unwrap_or(data.len().saturating_mul(3) + 16);
            self.out.reserve(need);
            let (result, read, had_errors) =
                self.decoder.decode_to_string(data, &mut self.out, last);
            if had_errors {
                sink.lossy = true;
            }
            sink.push(&self.out);
            data = &data[read..];
            if result == encoding_rs::CoderResult::InputEmpty {
                break;
            }
        }
    }
}

enum Decoder {
    Utf8(Utf8),
    Rs(Rs),
}

impl Decoder {
    fn feed(&mut self, data: &[u8], sink: &mut Sink) -> Result<(), NotUtf8> {
        match self {
            Decoder::Utf8(d) => d.feed(data, sink),
            Decoder::Rs(d) => {
                d.feed(data, false, sink);
                Ok(())
            }
        }
    }

    fn finish(&mut self, sink: &mut Sink) -> Result<(), NotUtf8> {
        match self {
            Decoder::Utf8(d) => d.finish(sink),
            Decoder::Rs(d) => {
                d.feed(&[], true, sink);
                Ok(())
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// One pass over a reader.

/// What to do with a file that has no byte order mark.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Detect {
    /// Try UTF-8; stop at the first invalid byte so the caller can start again as legacy.
    Utf8First,
    /// Read it as the legacy code page.
    Legacy,
}

enum Pass {
    Done(Decoded),
    /// Not valid UTF-8 and no byte order mark: read again with [`Detect::Legacy`].
    NotUtf8,
}

struct Decoded {
    sink: Sink,
    encoding: Encoding,
}

/// Reads one piece, retrying when interrupted.
fn read_some<R: Read + ?Sized>(reader: &mut R, buf: &mut [u8]) -> Result<usize, LoadError> {
    loop {
        match reader.read(buf) {
            Ok(n) => return Ok(n),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(LoadError::Io(e.to_string())),
        }
    }
}

struct Progress<'a> {
    total: Option<u64>,
    report: &'a dyn Fn(u64, u64),
    max_bytes: u64,
    done: u64,
    reported: u64,
}

fn decode<R: Read + ?Sized>(
    reader: &mut R,
    progress: &mut Progress<'_>,
    opts: &LoadOptions,
    cancel: &AtomicBool,
    detect: Detect,
) -> Result<Pass, LoadError> {
    let check = |cancel: &AtomicBool| {
        if cancel.load(Ordering::Relaxed) {
            Err(LoadError::Cancelled)
        } else {
            Ok(())
        }
    };
    let mut buf = vec![0u8; READ_CHUNK];
    let mut sink = Sink::new();
    progress.done = 0;
    progress.reported = 0;

    // The first bytes decide the encoding: three are enough for any byte order mark.
    let mut head: Vec<u8> = Vec::with_capacity(8);
    let mut eof = false;
    while head.len() < 3 {
        check(cancel)?;
        let n = read_some(reader, &mut buf[..3 - head.len()])?;
        if n == 0 {
            eof = true;
            break;
        }
        head.extend_from_slice(&buf[..n]);
        progress.done += n as u64;
    }
    let bom = if detect == Detect::Legacy {
        None
    } else {
        sniff_bom(&head)
    };
    let (encoding, mut decoder, skip) = match bom {
        Some(enc @ Encoding::Utf8Bom) => (enc, Decoder::Utf8(Utf8::new(false)), enc.bom_len()),
        Some(enc @ Encoding::Utf16Le) => (
            enc,
            Decoder::Rs(Rs::new(encoding_rs::UTF_16LE)),
            enc.bom_len(),
        ),
        Some(enc @ Encoding::Utf16Be) => (
            enc,
            Decoder::Rs(Rs::new(encoding_rs::UTF_16BE)),
            enc.bom_len(),
        ),
        _ => match detect {
            Detect::Utf8First => (Encoding::Utf8, Decoder::Utf8(Utf8::new(true)), 0),
            Detect::Legacy => (
                Encoding::Legacy(opts.legacy),
                Decoder::Rs(Rs::new(opts.legacy)),
                0,
            ),
        },
    };
    if decoder
        .feed(&head[skip.min(head.len())..], &mut sink)
        .is_err()
    {
        return Ok(Pass::NotUtf8);
    }

    loop {
        // A file shorter than the head has been read whole.
        if eof {
            break;
        }
        check(cancel)?;
        let n = read_some(reader, &mut buf)?;
        if n == 0 {
            break;
        }
        progress.done += n as u64;
        if progress.done > progress.max_bytes {
            return Err(LoadError::TooLarge {
                bytes: progress.done,
                max: progress.max_bytes,
            });
        }
        if decoder.feed(&buf[..n], &mut sink).is_err() {
            return Ok(Pass::NotUtf8);
        }
        if progress.done - progress.reported >= PROGRESS_STEP {
            progress.reported = progress.done;
            (progress.report)(progress.done, progress.total.unwrap_or(0));
        }
    }
    if decoder.finish(&mut sink).is_err() {
        return Ok(Pass::NotUtf8);
    }
    Ok(Pass::Done(Decoded { sink, encoding }))
}

/// A reader that can be started again from its beginning.
trait Source: Read {
    fn rewind(&mut self) -> Result<(), LoadError>;
}

/// Both passes: UTF-8 first, then the legacy code page if the bytes were not UTF-8.
fn decode_with_fallback<S: Source>(
    source: &mut S,
    progress: &mut Progress<'_>,
    opts: &LoadOptions,
    cancel: &AtomicBool,
) -> Result<Decoded, LoadError> {
    match decode(source, progress, opts, cancel, Detect::Utf8First)? {
        Pass::Done(d) => Ok(d),
        Pass::NotUtf8 => {
            source.rewind()?;
            match decode(source, progress, opts, cancel, Detect::Legacy)? {
                Pass::Done(d) => Ok(d),
                Pass::NotUtf8 => unreachable!("legacy decoding never asks for another pass"),
            }
        }
    }
}

fn finish(decoded: Decoded, file_bytes: u64, stamp: FileStamp, started: Instant) -> Loaded {
    let Decoded { mut sink, encoding } = decoded;
    sink.eol.finish();
    let longest = sink.longest.max(sink.cur);
    let rope = sink.builder.finish();
    Loaded {
        buffer: Buffer::from_rope(rope),
        info: FileInfo {
            encoding,
            eol: sink.eol,
            lossy: sink.lossy,
            has_nul: sink.has_nul,
            file_bytes,
            stamp,
            longest_line_chars: longest,
            elapsed: started.elapsed(),
        },
    }
}

fn io_error(e: &std::io::Error) -> LoadError {
    if e.kind() == std::io::ErrorKind::NotFound {
        LoadError::NotFound
    } else {
        LoadError::Io(e.to_string())
    }
}

/// A file read from its start again by seeking.
struct FileSource(std::fs::File);

impl Read for FileSource {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(out)
    }
}

impl Source for FileSource {
    fn rewind(&mut self) -> Result<(), LoadError> {
        self.0
            .seek(SeekFrom::Start(0))
            .map(|_| ())
            .map_err(|e| LoadError::Io(e.to_string()))
    }
}

/// Reads `path`. `progress(done, total)` is called at most every
/// [`PROGRESS_STEP`](crate::limits::PROGRESS_STEP) bytes; `cancel` is checked between chunks.
/// Retries up to three times while the file's stamp changes during the read.
pub fn load_path(
    path: &Path,
    opts: &LoadOptions,
    progress: &dyn Fn(u64, u64),
    cancel: &AtomicBool,
) -> Result<Loaded, LoadError> {
    let started = Instant::now();
    for _attempt in 0..3 {
        let meta = std::fs::metadata(path).map_err(|e| io_error(&e))?;
        if meta.is_dir() {
            return Err(LoadError::IsDirectory);
        }
        let size = meta.len();
        if size > opts.max_bytes {
            return Err(LoadError::TooLarge {
                bytes: size,
                max: opts.max_bytes,
            });
        }
        let before = stamp(path);
        if before == FileStamp::Missing {
            return Err(LoadError::NotFound);
        }
        let mut source = FileSource(std::fs::File::open(path).map_err(|e| io_error(&e))?);
        let mut prog = Progress {
            total: Some(size),
            report: progress,
            max_bytes: opts.max_bytes,
            done: 0,
            reported: 0,
        };
        let decoded = decode_with_fallback(&mut source, &mut prog, opts, cancel)?;
        if stamp(path) != before {
            continue;
        }
        return Ok(finish(decoded, size, before, started));
    }
    Err(LoadError::StillChanging)
}

/// Keeps the bytes read in the first pass, to hand them out again when it is rewound.
struct Recorder<'a> {
    inner: &'a mut dyn Read,
    kept: Vec<u8>,
    recording: bool,
    replay: Option<std::io::Cursor<Vec<u8>>>,
}

impl Read for Recorder<'_> {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        if let Some(cursor) = &mut self.replay {
            let n = cursor.read(out)?;
            if n > 0 {
                return Ok(n);
            }
            self.replay = None;
        }
        let n = self.inner.read(out)?;
        if self.recording {
            self.kept.extend_from_slice(&out[..n]);
        }
        Ok(n)
    }
}

impl Source for Recorder<'_> {
    fn rewind(&mut self) -> Result<(), LoadError> {
        self.recording = false;
        self.replay = Some(std::io::Cursor::new(std::mem::take(&mut self.kept)));
        Ok(())
    }
}

/// The same decoding as [`load_path`] over any reader, which may return pieces of any size
/// (tests feed one to seven bytes at a time). `total` is the size if known, for progress.
/// `stamp` is `FileStamp::Missing` and `elapsed` the decode time.
///
/// A reader cannot be rewound, so the bytes read are kept until the load ends, to be able to
/// start again as a legacy code page; `load_path` seeks back in the file instead and keeps
/// nothing. `file_bytes` is the number of bytes read.
pub fn load_reader(
    reader: &mut dyn Read,
    total: Option<u64>,
    opts: &LoadOptions,
    progress: &dyn Fn(u64, u64),
    cancel: &AtomicBool,
) -> Result<Loaded, LoadError> {
    let started = Instant::now();
    let mut prog = Progress {
        total,
        report: progress,
        max_bytes: opts.max_bytes,
        done: 0,
        reported: 0,
    };
    let mut source = Recorder {
        inner: reader,
        kept: Vec::new(),
        recording: true,
        replay: None,
    };
    let decoded = decode_with_fallback(&mut source, &mut prog, opts, cancel)?;
    Ok(finish(decoded, prog.done, FileStamp::Missing, started))
}

/// [`load_reader`] over a byte slice, which cannot fail.
pub fn load_bytes(bytes: &[u8], opts: &LoadOptions) -> Loaded {
    let mut reader = bytes;
    let opts = &LoadOptions {
        max_bytes: u64::MAX,
        ..*opts
    };
    let cancel = AtomicBool::new(false);
    match load_reader(
        &mut reader,
        Some(bytes.len() as u64),
        opts,
        &|_, _| {},
        &cancel,
    ) {
        Ok(loaded) => loaded,
        Err(_) => unreachable!("a byte slice does not fail and nothing sets the flag"),
    }
}
