//! A run that writes notes: its record, the order of the write, and undo
//! (`docs/ai-library-plan.md` 4.3, 4.4, 4.5).
//!
//! Nothing here can overwrite, move, rename or delete a file it did not write. A note and a
//! picture are created with [`mdshared_app::files::create_new`]: a name that is taken is refused
//! before a byte is written, and the bytes reach the name by a rename that never replaces a file.
//! A folder is only ever created, inside the library folder or the group's folder. The one
//! deletion is undo's, and it goes only to what the record says this run created (`created`, set
//! after the fact, never from the plan), only inside the library folder or the home the group's
//! library file declares now, only a regular file with the extension a run writes, only while
//! it is byte for byte the one it wrote, and only when no library file lists it any more.
//!
//! The order is the one of 4.3, and every prefix of it is a state that can be lived with: the
//! record first, then folders, pictures, notes, the read-back, the links, the closed record, and
//! the Recycle Bin last (not built: phase T). Nothing is linked before it exists, and the source
//! document is never touched.

use mdshared_app::files::create_new;
use mdshared_app::library::{self, Parsed};
use mdshared_app::paths::same_path;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// The folder of the records, inside the library folder. The reader takes only the `.md` files
/// directly in the library folder, so nothing in here is ever read as a group.
pub const RUNS_DIR: &str = ".runs";

/// The newest records kept, and the days kept when they are more.
pub const KEEP_RUNS: usize = 20;
pub const KEEP_DAYS: u64 = 90;

/// A record bigger than this is not one of ours.
const RECORD_MAX: u64 = 4 * 1024 * 1024;

/// The record format this program writes and reads. A record of a later version is not read, not
/// acted on, and never deleted: it is shown as written by a newer version.
pub const RECORD_VERSION: u32 = 1;

/// The most of one file undo reads to compare it with its recorded digest, whatever the record
/// says about it. A note or a source of more is not checked, so it is not removed and undo is not
/// run against it: a path in a record is not allowed to make the window read without end.
pub const DIGEST_MAX_BYTES: u64 = 64 * 1024 * 1024;

/// The most library files undo reads to find out whether one still lists a note.
const LISTING_MAX_FILES: usize = 4096;

/// The pictures a run copies: the only extensions undo removes besides a note's `.md`.
const PICTURE_EXTENSIONS: [&str; 10] = [
    "png", "jpg", "jpeg", "gif", "webp", "bmp", "svg", "avif", "tif", "tiff",
];

// ---- time -----------------------------------------------------------------------------------------

/// Seconds since 1970 and `2026-11-21T14:03:11Z`, in UTC.
pub fn now_utc() -> (u64, String) {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    (secs, utc(secs))
}

/// `YYYY-MM-DDTHH:MM:SSZ` for seconds since 1970, UTC.
pub fn utc(secs: u64) -> String {
    let (y, m, d, hh, mm, ss) = civil(secs);
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

/// The day number and the seconds of the day, as a civil date and time, UTC.
fn civil(secs: u64) -> (i64, u32, u32, u64, u64, u64) {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d, rem / 3600, rem / 60 % 60, rem % 60)
}

/// The seconds since 1970 of a `YYYY-MM-DDTHH:MM:SSZ` stamp, when it reads as one.
fn secs_of(stamp: &str) -> Option<u64> {
    let b = stamp.as_bytes();
    if b.len() != 20
        || b.get(4) != Some(&b'-')
        || b.get(7) != Some(&b'-')
        || b.get(10) != Some(&b'T')
        || b.get(13) != Some(&b':')
        || b.get(16) != Some(&b':')
        || b.get(19) != Some(&b'Z')
    {
        return None;
    }
    let num = |from: usize, to: usize| -> Option<u64> { stamp.get(from..to)?.parse().ok() };
    let (y, m, d) = (num(0, 4)? as i64, num(5, 7)? as u32, num(8, 10)? as u32);
    let (hh, mm, ss) = (num(11, 13)?, num(14, 16)?, num(17, 19)?);
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) || hh > 23 || mm > 59 || ss > 60 {
        return None;
    }
    let days = days_from_civil(y, m, d);
    if days < 0 {
        return None;
    }
    Some(days as u64 * 86_400 + hh * 3600 + mm * 60 + ss)
}

/// Days since 1970 of a civil date, UTC.
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = y - i64::from(m <= 2);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = i64::from(if m > 2 { m - 3 } else { m + 9 });
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The name a record has at `secs`: `2026-11-21T14-03-11Z-split`. A colon is not a character
/// Windows allows in a file name, so the clock is written with hyphens. The `started` field
/// inside keeps the ISO stamp.
fn stamp(secs: u64, kind: Kind) -> String {
    let name = utc(secs).replace(':', "-");
    let stem = name.strip_suffix('Z').unwrap_or(&name);
    format!("{stem}Z-{}", kind.as_str())
}

// ---- the record -----------------------------------------------------------------------------------

/// What a run was: the one kind there is so far.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Split,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Split => "split",
        }
    }

    fn parse(text: &str) -> Option<Kind> {
        match text {
            "split" => Some(Kind::Split),
            _ => None,
        }
    }
}

/// Where a run is in the order of 4.3.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    /// The record is made; nothing else was done.
    Writing,
    /// The notes are written, but a library file could not be updated.
    LinksPending,
    /// Everything was done and the record was closed.
    Done,
    /// The run was cancelled between files.
    Stopped,
    /// Undo ran.
    Undone,
}

impl State {
    pub fn as_str(self) -> &'static str {
        match self {
            State::Writing => "writing",
            State::LinksPending => "links pending",
            State::Done => "done",
            State::Stopped => "stopped",
            State::Undone => "undone",
        }
    }

    fn parse(text: &str) -> Option<State> {
        match text {
            "writing" => Some(State::Writing),
            "links pending" => Some(State::LinksPending),
            "done" => Some(State::Done),
            "stopped" => Some(State::Stopped),
            "undone" => Some(State::Undone),
            _ => None,
        }
    }

    /// A run that can be undone, or finished.
    pub fn live(self) -> bool {
        !matches!(self, State::Undone)
    }
}

/// What was sent to the AI, when anything was. A split at headings sends nothing.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Sent {
    pub connection: String,
    pub model: String,
    pub bytes: u64,
    pub requests: u64,
}

/// The document that was split: its path, its length and its digest, so step 8 and undo can tell
/// whether it is still the file that was read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    pub path: PathBuf,
    pub len: u64,
    pub sha256: String,
    /// Undo refuses while this is true and the file is not back. Nothing sets it before phase T.
    pub recycled: bool,
}

/// The group the notes went to: its name, its library file, whether the run made it, and its
/// folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    /// The name the owner sees, which is what a new library file's heading holds.
    pub name: String,
    pub file: PathBuf,
    /// In a [`Request`]: the file is to be made. In a [`Record`]: this run made it, which is set
    /// only after the file was created, so a file that was there is never marked.
    pub created: bool,
    pub home: PathBuf,
}

/// One note the record holds: where it went, what it holds, and the line that lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteRec {
    pub path: PathBuf,
    /// This run created the file. False for a note that was there already with the same bytes
    /// (it is linked, and undo takes its line out, but never the file) and for one that was not
    /// written. Set after the write; a record with no such field says false.
    pub created: bool,
    pub sha256: String,
    /// The lines of the source, 1-based and inclusive.
    pub ranges: Vec<(usize, usize)>,
    pub heading_shift: u8,
    pub title_added: bool,
    pub sections: Vec<String>,
    /// The link line as it was written into the library file.
    pub line: String,
}

/// One picture the run copied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PictureRec {
    pub path: PathBuf,
    /// This run created the file: see [`NoteRec::created`].
    pub created: bool,
    pub sha256: String,
}

/// One run's record, in memory with absolute paths.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub version: u32,
    pub kind: Kind,
    pub started: String,
    /// Not written: what `started` means as a number, for the list's age.
    pub started_secs: u64,
    pub state: State,
    pub instruction: String,
    pub sent: Option<Sent>,
    pub source: Source,
    pub group: Group,
    /// The folders this run made, parents first: not the folders its notes use, only the ones
    /// that were not there when it started.
    pub folders: Vec<PathBuf>,
    pub notes: Vec<NoteRec>,
    pub pictures: Vec<PictureRec>,
    /// The section headings the run wrote into a library file, outermost first, so undo can take
    /// them out when nothing is under them.
    pub sections_added: Vec<Vec<String>>,
}

/// A record read from disk.
#[derive(Clone, Debug)]
pub struct Run {
    pub record: Record,
    pub path: PathBuf,
    pub library: PathBuf,
}

// ---- paths in a record -----------------------------------------------------------------------------

fn stored(path: &Path, library: &Path) -> String {
    stored_on(path, library, cfg!(windows))
}

fn resolved(text: &str, library: &Path) -> PathBuf {
    resolved_on(text, library, cfg!(windows))
}

/// A record is written with `/` between the parts of a path, so a library carried between
/// machines reads the same everywhere. Only Windows separates with `\`: everywhere else a
/// backslash is an ordinary character of a file's name, and turning it into a separator would
/// make the record name a different file. `windows` is the platform, taken as an argument so a
/// test can try both on either (and it is why the record's paths broke on Linux and a Mac).
fn stored_on(path: &Path, library: &Path, windows: bool) -> String {
    let shown = path.strip_prefix(library).unwrap_or(path);
    let text = shown.to_string_lossy();
    if windows {
        text.replace('\\', "/")
    } else {
        text.into_owned()
    }
}

/// What [`stored_on`] wrote, read back on the platform that is reading it.
fn resolved_on(text: &str, library: &Path, windows: bool) -> PathBuf {
    let p = if windows {
        PathBuf::from(text.replace('/', "\\"))
    } else {
        PathBuf::from(text)
    };
    if p.is_absolute() || library.as_os_str().is_empty() {
        p
    } else {
        library.join(p)
    }
}

// ---- JSON ------------------------------------------------------------------------------------------

/// A small JSON value, enough for a record and no more. Nothing here recurses without a depth
/// bound, so a hostile file cannot exhaust the stack; the release build aborts on a panic, so
/// every index is checked.
enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(items) => items.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    fn str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }

    fn num(&self) -> Option<f64> {
        match self {
            Json::Num(n) if n.is_finite() => Some(*n),
            _ => None,
        }
    }

    fn uint(&self) -> Option<u64> {
        let n = self.num()?;
        (n >= 0.0 && n <= u64::MAX as f64).then_some(n as u64)
    }

    fn usize(&self) -> Option<usize> {
        let n = self.uint()?;
        usize::try_from(n).ok()
    }

    fn bool(&self) -> Option<bool> {
        match self {
            Json::Bool(b) => Some(*b),
            _ => None,
        }
    }

    fn arr(&self) -> Option<&[Json]> {
        match self {
            Json::Arr(items) => Some(items),
            _ => None,
        }
    }
}

const JSON_DEPTH: u8 = 32;

struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn new(text: &'a str) -> Reader<'a> {
        Reader {
            b: text.as_bytes(),
            at: 0,
        }
    }

    fn skip(&mut self) {
        while let Some(&c) = self.b.get(self.at) {
            if c == b' ' || c == b'\t' || c == b'\r' || c == b'\n' {
                self.at += 1;
            } else {
                break;
            }
        }
    }

    fn eat(&mut self, c: u8) -> bool {
        self.skip();
        if self.b.get(self.at) == Some(&c) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    fn word(&mut self, word: &str) -> bool {
        self.skip();
        let end = self.at.saturating_add(word.len());
        if self.b.get(self.at..end) == Some(word.as_bytes()) {
            self.at = end;
            true
        } else {
            false
        }
    }

    fn value(&mut self, depth: u8) -> Option<Json> {
        if depth == 0 {
            return None;
        }
        self.skip();
        match *self.b.get(self.at)? {
            b'n' => self.word("null").then_some(Json::Null),
            b't' => self.word("true").then_some(Json::Bool(true)),
            b'f' => self.word("false").then_some(Json::Bool(false)),
            b'"' => self.string().map(Json::Str),
            b'[' => {
                self.at += 1;
                let mut items = Vec::new();
                self.skip();
                if self.eat(b']') {
                    return Some(Json::Arr(items));
                }
                loop {
                    items.push(self.value(depth - 1)?);
                    if self.eat(b']') {
                        return Some(Json::Arr(items));
                    }
                    if !self.eat(b',') {
                        return None;
                    }
                }
            }
            b'{' => {
                self.at += 1;
                let mut items: Vec<(String, Json)> = Vec::new();
                self.skip();
                if self.eat(b'}') {
                    return Some(Json::Obj(items));
                }
                loop {
                    self.skip();
                    if self.b.get(self.at) != Some(&b'"') {
                        return None;
                    }
                    let key = self.string()?;
                    if !self.eat(b':') {
                        return None;
                    }
                    let value = self.value(depth - 1)?;
                    items.push((key, value));
                    if self.eat(b'}') {
                        return Some(Json::Obj(items));
                    }
                    if !self.eat(b',') {
                        return None;
                    }
                }
            }
            _ => self.number(),
        }
    }

    fn number(&mut self) -> Option<Json> {
        self.skip();
        let start = self.at;
        if self.b.get(self.at) == Some(&b'-') {
            self.at += 1;
        }
        let mut digits = false;
        while self.b.get(self.at).is_some_and(|c| c.is_ascii_digit()) {
            self.at += 1;
            digits = true;
        }
        if self.b.get(self.at) == Some(&b'.') {
            self.at += 1;
            while self.b.get(self.at).is_some_and(|c| c.is_ascii_digit()) {
                self.at += 1;
            }
        }
        if matches!(self.b.get(self.at), Some(b'e' | b'E')) {
            self.at += 1;
            if matches!(self.b.get(self.at), Some(b'+' | b'-')) {
                self.at += 1;
            }
            while self.b.get(self.at).is_some_and(|c| c.is_ascii_digit()) {
                self.at += 1;
            }
        }
        if !digits {
            return None;
        }
        let text = std::str::from_utf8(self.b.get(start..self.at)?).ok()?;
        text.parse().ok().map(Json::Num)
    }

    fn string(&mut self) -> Option<String> {
        if self.b.get(self.at) != Some(&b'"') {
            return None;
        }
        self.at += 1;
        let mut out = String::new();
        loop {
            let c = *self.b.get(self.at)?;
            self.at += 1;
            match c {
                b'"' => return Some(out),
                b'\\' => {
                    let e = *self.b.get(self.at)?;
                    self.at += 1;
                    match e {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let hi = self.hex4()?;
                            let ch = if (0xD800..0xDC00).contains(&hi) {
                                // A surrogate pair, when the second half is there.
                                let save = self.at;
                                if self.b.get(self.at) == Some(&b'\\')
                                    && self.b.get(self.at + 1) == Some(&b'u')
                                {
                                    self.at += 2;
                                    match self.hex4() {
                                        Some(lo) if (0xDC00..0xE000).contains(&lo) => {
                                            char::from_u32(
                                                0x1_0000 + ((hi - 0xD800) << 10) + (lo - 0xDC00),
                                            )
                                            .unwrap_or('\u{FFFD}')
                                        }
                                        _ => {
                                            self.at = save;
                                            '\u{FFFD}'
                                        }
                                    }
                                } else {
                                    '\u{FFFD}'
                                }
                            } else {
                                char::from_u32(hi).unwrap_or('\u{FFFD}')
                            };
                            out.push(ch);
                        }
                        _ => return None,
                    }
                }
                c if c < 0x20 => return None,
                c if c < 0x80 => out.push(c as char),
                _ => {
                    // A UTF-8 character's bytes, from its first byte's length.
                    let len = if c < 0xE0 {
                        2
                    } else if c < 0xF0 {
                        3
                    } else {
                        4
                    };
                    let from = self.at - 1;
                    let end = from + len;
                    let bytes = self.b.get(from..end)?;
                    out.push_str(std::str::from_utf8(bytes).ok()?);
                    self.at = end;
                }
            }
        }
    }

    fn hex4(&mut self) -> Option<u32> {
        let mut n = 0u32;
        for _ in 0..4 {
            let c = *self.b.get(self.at)?;
            self.at += 1;
            n = n * 16 + (c as char).to_digit(16)?;
        }
        Some(n)
    }
}

fn parse_json(text: &str) -> Option<Json> {
    let mut r = Reader::new(text);
    let value = r.value(JSON_DEPTH)?;
    r.skip();
    Some(value)
}

fn esc(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

// ---- the record's files ----------------------------------------------------------------------------

impl Record {
    /// The JSON this record is written as, with the library folder's paths relative to it.
    pub fn to_json(&self, library: &Path) -> String {
        let mut s = String::new();
        s.push_str("{\n");
        s.push_str(&format!("  \"version\": {},\n", self.version));
        s.push_str(&format!("  \"kind\": \"{}\",\n", esc(self.kind.as_str())));
        s.push_str(&format!("  \"started\": \"{}\",\n", esc(&self.started)));
        s.push_str(&format!("  \"state\": \"{}\",\n", esc(self.state.as_str())));
        s.push_str(&format!(
            "  \"instruction\": \"{}\",\n",
            esc(&self.instruction)
        ));
        match &self.sent {
            Some(sent) => s.push_str(&format!(
                "  \"sent\": {{\"connection\": \"{}\", \"model\": \"{}\", \"bytes\": {}, \"requests\": {}}},\n",
                esc(&sent.connection),
                esc(&sent.model),
                sent.bytes,
                sent.requests
            )),
            None => s.push_str("  \"sent\": null,\n"),
        }
        s.push_str(&format!(
            "  \"source\": {{\"path\": \"{}\", \"len\": {}, \"sha256\": \"{}\", \"recycled\": {}}},\n",
            esc(&stored(&self.source.path, library)),
            self.source.len,
            esc(&self.source.sha256),
            self.source.recycled
        ));
        s.push_str(&format!(
            "  \"group\": {{\"name\": \"{}\", \"file\": \"{}\", \"created\": {}, \"home\": \"{}\"}},\n",
            esc(&self.group.name),
            esc(&stored(&self.group.file, library)),
            self.group.created,
            esc(&stored(&self.group.home, library))
        ));
        let folders: Vec<String> = self
            .folders
            .iter()
            .map(|f| format!("\"{}\"", esc(&stored(f, library))))
            .collect();
        s.push_str(&format!("  \"folders\": [{}],\n", folders.join(", ")));
        s.push_str("  \"notes\": [");
        for (i, n) in self.notes.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            let ranges: Vec<String> = n
                .ranges
                .iter()
                .map(|(a, b)| format!("[{a}, {b}]"))
                .collect();
            let sections: Vec<String> = n
                .sections
                .iter()
                .map(|x| format!("\"{}\"", esc(x)))
                .collect();
            s.push_str(&format!(
                "\n    {{\"path\": \"{}\", \"created\": {}, \"sha256\": \"{}\", \"ranges\": [{}], \"heading_shift\": {}, \"title_added\": {}, \"sections\": [{}], \"line\": \"{}\"}}",
                esc(&stored(&n.path, library)),
                n.created,
                esc(&n.sha256),
                ranges.join(", "),
                n.heading_shift,
                n.title_added,
                sections.join(", "),
                esc(&n.line)
            ));
        }
        if !self.notes.is_empty() {
            s.push_str("\n  ");
        }
        s.push_str("],\n");
        let headings: Vec<String> = self
            .sections_added
            .iter()
            .map(|path| {
                let names: Vec<String> = path.iter().map(|x| format!("\"{}\"", esc(x))).collect();
                format!("[{}]", names.join(", "))
            })
            .collect();
        s.push_str(&format!(
            "  \"sections_added\": [{}],\n",
            headings.join(", ")
        ));
        let pictures: Vec<String> = self
            .pictures
            .iter()
            .map(|p| {
                format!(
                    "{{\"path\": \"{}\", \"created\": {}, \"sha256\": \"{}\"}}",
                    esc(&stored(&p.path, library)),
                    p.created,
                    esc(&p.sha256)
                )
            })
            .collect();
        s.push_str(&format!("  \"pictures\": [{}]\n", pictures.join(", ")));
        s.push_str("}\n");
        s
    }

    /// The record a JSON text holds, its paths resolved against `library`; `None` when it is not
    /// a record this program wrote.
    pub fn from_json(text: &str, library: &Path) -> Option<Record> {
        let json = parse_json(text)?;
        let version = u32::try_from(json.get("version")?.uint()?).ok()?;
        if version > RECORD_VERSION {
            return None;
        }
        let kind = Kind::parse(json.get("kind")?.str()?)?;
        let started = json.get("started")?.str()?.to_string();
        let started_secs = secs_of(&started).unwrap_or(0);
        let state = State::parse(json.get("state")?.str()?)?;
        let instruction = json
            .get("instruction")
            .and_then(Json::str)
            .unwrap_or_default()
            .to_string();
        let sent = json.get("sent").and_then(|s| {
            Some(Sent {
                connection: s.get("connection")?.str()?.to_string(),
                model: s
                    .get("model")
                    .and_then(Json::str)
                    .unwrap_or_default()
                    .to_string(),
                bytes: s.get("bytes").and_then(Json::uint).unwrap_or(0),
                requests: s.get("requests").and_then(Json::uint).unwrap_or(0),
            })
        });
        let src = json.get("source")?;
        let source = Source {
            path: resolved(src.get("path")?.str()?, library),
            len: src.get("len")?.uint()?,
            sha256: src
                .get("sha256")
                .and_then(Json::str)
                .unwrap_or_default()
                .to_string(),
            recycled: src.get("recycled").and_then(Json::bool).unwrap_or(false),
        };
        let g = json.get("group")?;
        let file = resolved(g.get("file")?.str()?, library);
        let group = Group {
            name: g
                .get("name")
                .and_then(Json::str)
                .map(str::to_string)
                .unwrap_or_else(|| {
                    file.file_stem()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_default()
                }),
            file,
            created: g.get("created").and_then(Json::bool).unwrap_or(false),
            home: resolved(
                g.get("home").and_then(Json::str).unwrap_or_default(),
                library,
            ),
        };
        let folders = json
            .get("folders")
            .and_then(Json::arr)
            .map(|a| {
                a.iter()
                    .filter_map(Json::str)
                    .map(|s| resolved(s, library))
                    .collect()
            })
            .unwrap_or_default();
        let notes = json
            .get("notes")
            .and_then(Json::arr)
            .map(|a| {
                a.iter()
                    .filter_map(|n| {
                        let path = resolved(n.get("path")?.str()?, library);
                        let ranges = n
                            .get("ranges")
                            .and_then(Json::arr)
                            .map(|r| {
                                r.iter()
                                    .filter_map(|pair| {
                                        let pair = pair.arr()?;
                                        let a = pair.first()?.usize()?;
                                        let b = pair.get(1)?.usize()?;
                                        (a >= 1 && a <= b).then_some((a, b))
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        let sections = n
                            .get("sections")
                            .and_then(Json::arr)
                            .map(|s| s.iter().filter_map(Json::str).map(str::to_string).collect())
                            .unwrap_or_default();
                        Some(NoteRec {
                            path,
                            created: n.get("created").and_then(Json::bool).unwrap_or(false),
                            sha256: n
                                .get("sha256")
                                .and_then(Json::str)
                                .unwrap_or_default()
                                .to_string(),
                            ranges,
                            heading_shift: n
                                .get("heading_shift")
                                .and_then(Json::uint)
                                .unwrap_or(0)
                                .min(8) as u8,
                            title_added: n.get("title_added").and_then(Json::bool).unwrap_or(false),
                            sections,
                            line: n
                                .get("line")
                                .and_then(Json::str)
                                .unwrap_or_default()
                                .to_string(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let sections_added = json
            .get("sections_added")
            .and_then(Json::arr)
            .map(|a| {
                a.iter()
                    .filter_map(|path| {
                        let names: Vec<String> = path
                            .arr()?
                            .iter()
                            .filter_map(Json::str)
                            .map(str::to_string)
                            .collect();
                        (!names.is_empty()).then_some(names)
                    })
                    .take(64)
                    .collect()
            })
            .unwrap_or_default();
        let pictures = json
            .get("pictures")
            .and_then(Json::arr)
            .map(|a| {
                a.iter()
                    .filter_map(|p| {
                        Some(PictureRec {
                            path: resolved(p.get("path")?.str()?, library),
                            created: p.get("created").and_then(Json::bool).unwrap_or(false),
                            sha256: p
                                .get("sha256")
                                .and_then(Json::str)
                                .unwrap_or_default()
                                .to_string(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        Some(Record {
            version,
            kind,
            started,
            started_secs,
            state,
            instruction,
            sent,
            source,
            group,
            folders,
            notes,
            pictures,
            sections_added,
        })
    }

    /// Writes the record as a new file in `<library>/.runs`, and gives its path. A name that is
    /// taken is another run's record: the next number is used, and no file is ever replaced.
    pub fn create(&self, library: &Path) -> io::Result<PathBuf> {
        let dir = library.join(RUNS_DIR);
        std::fs::create_dir_all(&dir)?;
        let stem = stamp(self.started_secs, self.kind);
        let text = self.to_json(library);
        for n in 1..=99 {
            let name = if n == 1 {
                format!("{stem}.json")
            } else {
                format!("{stem}-{n}.json")
            };
            let path = dir.join(name);
            match create_new(&path, text.as_bytes()) {
                Ok(()) => return Ok(path),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "the runs folder holds that name already",
        ))
    }

    /// Writes the record again in place. The file is the run's own, named when step 1 made it,
    /// so this replaces it with the same run's newer state.
    pub fn close(&self, path: &Path, library: &Path) -> io::Result<()> {
        mdshared_app::settings::write_atomic(path, &self.to_json(library))
    }

    /// Appends one line to the `<same name>.done` beside the record.
    pub fn note_done(&self, path: &Path, line: &str) {
        let done = path.with_extension("done");
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&done)
        {
            let _ = writeln!(file, "{} {line}", utc(now_utc().0));
        }
    }

    /// The group's name as the list shows it: its library file's stem.
    pub fn group_name(&self) -> String {
        self.group
            .file
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    /// The source's name.
    pub fn source_name(&self) -> String {
        self.source
            .path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
    }
}

/// Reads one record. `None` when the file is not a record of ours; nothing here can panic.
pub fn load(path: &Path, library: &Path) -> Option<Record> {
    inspect(path, library).ok()
}

/// A record file this version could not use: another version wrote it, or it is not a record.
/// It is listed and never acted on, and never deleted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unread {
    pub path: PathBuf,
    /// A later version of this program wrote it: a version this one does not know, or a kind or
    /// a state it has no word for.
    pub newer: bool,
    /// When it was written, from its name (`2026-11-21T14:03:11Z`), or empty.
    pub started: String,
}

/// Reads one record; `Err(newer)` when it cannot be used, saying whether a later version wrote it.
fn inspect(path: &Path, library: &Path) -> Result<Record, bool> {
    let meta = std::fs::metadata(path).map_err(|_| false)?;
    if !meta.is_file() || meta.len() > RECORD_MAX {
        return Err(false);
    }
    let text = std::fs::read_to_string(path).map_err(|_| false)?;
    match Record::from_json(&text, library) {
        Some(record) => Ok(record),
        None => Err(written_by_newer(&text)),
    }
}

/// Whether a text that is not a record of this version is one of a later version: valid JSON
/// whose version is past [`RECORD_VERSION`], or whose kind or state this version has no word for.
fn written_by_newer(text: &str) -> bool {
    let Some(json) = parse_json(text) else {
        return false;
    };
    let version = json.get("version").and_then(Json::uint);
    version.is_some_and(|v| v > u64::from(RECORD_VERSION))
        || (version.is_some()
            && (json
                .get("kind")
                .and_then(Json::str)
                .is_some_and(|k| Kind::parse(k).is_none())
                || json
                    .get("state")
                    .and_then(Json::str)
                    .is_some_and(|s| State::parse(s).is_none())))
}

/// The time a record's file name holds: `2026-11-21T14-03-11Z-split.json`, as an ISO stamp.
fn name_stamp(path: &Path) -> String {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let parts = (|| {
        let head = stem.get(..20)?;
        Some(format!(
            "{}:{}:{}",
            head.get(..13)?,
            head.get(14..16)?,
            head.get(17..)?
        ))
    })();
    parts
        .filter(|iso| secs_of(iso).is_some())
        .unwrap_or_default()
}

/// Every record in `library/.runs`, newest first. The folder is read only when this is asked
/// for: never at start.
pub fn list(library: &Path) -> Vec<Run> {
    list_all(library).0
}

/// [`list`], and with it the record files this version cannot use, newest first by the time in
/// their names, so a window can say that they are there.
pub fn list_all(library: &Path) -> (Vec<Run>, Vec<Unread>) {
    let dir = library.join(RUNS_DIR);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return (Vec::new(), Vec::new());
    };
    let mut runs: Vec<Run> = Vec::new();
    let mut unread: Vec<Unread> = Vec::new();
    for path in entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("json"))
        })
    {
        match inspect(&path, library) {
            Ok(record) => runs.push(Run {
                record,
                path,
                library: library.to_path_buf(),
            }),
            Err(newer) => unread.push(Unread {
                started: name_stamp(&path),
                path,
                newer,
            }),
        }
    }
    runs.sort_by(|a, b| b.record.started.cmp(&a.record.started));
    unread.sort_by(|a, b| b.started.cmp(&a.started).then_with(|| a.path.cmp(&b.path)));
    (runs, unread)
}

/// Deletes the records older than the newest [`KEEP_RUNS`] and older than [`KEEP_DAYS`]. Their
/// notes and their `.done` files beside them go only when the record goes. A file that does not
/// load as a record of this version is not counted among the newest and is never deleted: it may
/// be the record of a later version, and its undo with it.
pub fn prune(library: &Path, now: u64) {
    let dir = library.join(RUNS_DIR);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    let mut all: Vec<(u64, PathBuf)> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("json"))
        })
        .filter_map(|p| {
            let record = load(&p, library)?;
            Some((record.started_secs, p))
        })
        .collect();
    all.sort_by_key(|(secs, _)| std::cmp::Reverse(*secs));
    for (i, (secs, path)) in all.into_iter().enumerate() {
        let old = i >= KEEP_RUNS && (now.saturating_sub(secs)) > KEEP_DAYS * 86_400;
        if old {
            let _ = std::fs::remove_file(&path);
            let _ = std::fs::remove_file(path.with_extension("done"));
        }
    }
}

// ---- what to write --------------------------------------------------------------------------------

/// One note the run writes: the exact bytes, where they go, and the line that lists it.
#[derive(Clone, Debug)]
pub struct NewNote {
    pub path: PathBuf,
    pub text: String,
    /// The same note as the plan made it from the source's lines. Step 5 compares the file with
    /// this: in the program it is the same text as `text`; a writer that wrote anything else is
    /// caught here.
    pub from_source: String,
    pub title: String,
    /// The note's path as the library file links it: relative to the library when it is inside.
    pub link_path: PathBuf,
    pub sections: Vec<String>,
    pub ranges: Vec<(usize, usize)>,
    pub heading_shift: u8,
    pub title_added: bool,
    /// The file was there with these bytes when the plan was made (3.9).
    pub already: bool,
}

/// One picture the run copies.
#[derive(Clone, Debug)]
pub struct NewPicture {
    pub source: PathBuf,
    /// The folder it goes in; its name is made from the source's own name.
    pub folder: PathBuf,
}

/// Everything a write does, worked out before step 1.
#[derive(Clone, Debug)]
pub struct Request {
    pub library: PathBuf,
    pub source: PathBuf,
    pub instruction: String,
    pub sent: Option<Sent>,
    pub group: Group,
    /// The folders to make, parents first, all of them inside the library or the group's folder.
    pub folders: Vec<PathBuf>,
    pub notes: Vec<NewNote>,
    pub pictures: Vec<NewPicture>,
}

/// A link as the library file holds it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    pub path: PathBuf,
    pub title: String,
    pub sections: Vec<String>,
    pub line: String,
}

impl Link {
    pub fn new(note: &NewNote) -> Link {
        Link {
            path: note.link_path.clone(),
            title: note.title.clone(),
            sections: note.sections.clone(),
            line: library::link_line(&note.title, &note.link_path),
        }
    }
}

/// A link undo takes out: the line as it was recorded, so a line the owner edited is kept.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkRemoval {
    pub path: PathBuf,
    pub sections: Vec<String>,
    pub line: String,
}

/// What the notes and the library write through: the window's own gate, which refuses a library
/// file that is open with unsaved changes, re-reads it, checks the result and writes it at once.
pub trait Gate {
    /// Adds every link of one library file in one atomic write.
    fn add_links(&mut self, file: &Path, links: &[Link]) -> Result<(), String>;

    /// Takes every removal out of one library file in one atomic write, and then the section
    /// headings the run wrote that have nothing under them.
    fn remove_links(
        &mut self,
        file: &Path,
        links: &[LinkRemoval],
        headings: &[Vec<String>],
    ) -> Result<(), String>;
}

/// Why one file was left out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    pub path: PathBuf,
    pub why: String,
}

impl Failure {
    fn new(path: &Path, why: impl Into<String>) -> Failure {
        Failure {
            path: path.to_path_buf(),
            why: why.into(),
        }
    }
}

/// What a write did.
#[derive(Clone, Debug, Default)]
pub struct Report {
    pub record: Option<Record>,
    pub record_path: Option<PathBuf>,
    /// Step 1 failed: nothing at all was done.
    pub start_error: Option<String>,
    pub notes_written: Vec<PathBuf>,
    pub notes_present: Vec<PathBuf>,
    pub pictures_written: Vec<PathBuf>,
    pub note_failures: Vec<Failure>,
    pub picture_failures: Vec<Failure>,
    pub check_failures: Vec<Failure>,
    pub folder_failures: Vec<Failure>,
    /// The group's library file could not be made.
    pub group_failure: Option<Failure>,
    /// The library files that could not be updated, and why.
    pub link_failures: Vec<Failure>,
    /// The library files that were.
    pub linked: Vec<PathBuf>,
    /// Step 7 was reached.
    pub closed: bool,
    pub close_error: Option<String>,
}

impl Report {
    /// How many notes are on disk: written now, or there already.
    pub fn notes(&self) -> usize {
        self.notes_written.len() + self.notes_present.len()
    }

    /// Whether the run counts as done, even with a file or two left out.
    pub fn state(&self) -> Option<State> {
        self.record.as_ref().map(|r| r.state)
    }
}

// ---- the write -----------------------------------------------------------------------------------

/// Splits `req` into its files, in the order of 4.3, with the clock read now.
pub fn write(req: &Request, gate: &mut dyn Gate) -> Report {
    let (secs, started) = now_utc();
    write_at(req, gate, secs, &started)
}

/// [`write`] with the clock given, so a test can know the record's name.
pub fn write_at(req: &Request, gate: &mut dyn Gate, secs: u64, started: &str) -> Report {
    let mut report = Report::default();
    // The source as it is now: its length and digest go in the record, and step 8 compares the
    // file with them before it would move it anywhere.
    let source = match std::fs::read(&req.source) {
        Ok(bytes) => Source {
            path: req.source.clone(),
            len: bytes.len() as u64,
            sha256: mdshared_app::update::sha256_hex(&bytes),
            recycled: false,
        },
        Err(e) => {
            report.start_error = Some(format!("The run could not start: {e}"));
            return report;
        }
    };
    if let Some(why) = outside(req) {
        report.start_error = Some(format!("The run could not start: {why}"));
        return report;
    }
    // 1. The record, state "writing", holding the whole plan. What the run did is not in it yet:
    // no folder, no group file and no note is marked as made by this run until it was made, so a
    // run that stops here leaves a record whose undo touches nothing it did not make.
    let mut record = Record {
        version: RECORD_VERSION,
        kind: Kind::Split,
        started: started.to_string(),
        started_secs: secs,
        state: State::Writing,
        instruction: req.instruction.clone(),
        sent: req.sent.clone(),
        source,
        group: Group {
            created: false,
            ..req.group.clone()
        },
        folders: Vec::new(),
        notes: req
            .notes
            .iter()
            .map(|n| NoteRec {
                path: n.path.clone(),
                created: false,
                // The digest of the bytes this note will hold, known before they are written.
                sha256: mdshared_app::update::sha256_hex(n.text.as_bytes()),
                ranges: n.ranges.clone(),
                heading_shift: n.heading_shift,
                title_added: n.title_added,
                sections: n.sections.clone(),
                line: library::link_line(&n.title, &n.link_path),
            })
            .collect(),
        pictures: Vec::new(),
        sections_added: Vec::new(),
    };
    let record_path = match record.create(&req.library) {
        Ok(path) => path,
        Err(e) => {
            report.start_error = Some(format!("The run could not start: {e}"));
            return report;
        }
    };
    report.record_path = Some(record_path.clone());
    record.note_done(&record_path, "started");

    // 2. Folders. A folder that cannot be made fails the notes that needed it, at 4.
    for folder in &req.folders {
        if !inside_any(folder, req) {
            report.folder_failures.push(Failure::new(
                folder,
                "it is outside the library and the group's folder",
            ));
            continue;
        }
        // The folders that are not there yet, deepest first: the ones this run makes. A folder
        // that was there is never in the record, so undo never removes it.
        let mut missing: Vec<PathBuf> = Vec::new();
        let mut at = Some(folder.as_path());
        while let Some(dir) = at {
            if dir.symlink_metadata().is_ok() || !inside_any(dir, req) {
                break;
            }
            missing.push(dir.to_path_buf());
            at = dir.parent();
        }
        let made = std::fs::create_dir_all(folder);
        for dir in missing.into_iter().rev() {
            if dir.is_dir() && !record.folders.iter().any(|f| same_path(f, &dir)) {
                record.folders.push(dir);
            }
        }
        if let Err(e) = made {
            report
                .folder_failures
                .push(Failure::new(folder, e.to_string()));
        }
    }

    // 3. Pictures: each copied to a new file, never over one.
    for picture in &req.pictures {
        let bytes = match std::fs::read(&picture.source) {
            Ok(bytes) => bytes,
            Err(e) => {
                report
                    .picture_failures
                    .push(Failure::new(&picture.source, e.to_string()));
                continue;
            }
        };
        let stem = picture
            .source
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "picture".to_string());
        let ext = picture
            .source
            .extension()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        match create_copy(&picture.folder, &stem, &ext, &bytes) {
            Ok((path, true)) => {
                record.pictures.push(PictureRec {
                    path: path.clone(),
                    created: true,
                    sha256: mdshared_app::update::sha256_hex(&bytes),
                });
                report.pictures_written.push(path.clone());
                record.note_done(&record_path, &format!("picture {path:?}"));
            }
            Ok((path, false)) => {
                // The same bytes are already there: the picture counts as copied, and the file
                // is not written again. It is not this run's file, so undo leaves it.
                record.pictures.push(PictureRec {
                    path: path.clone(),
                    created: false,
                    sha256: mdshared_app::update::sha256_hex(&bytes),
                });
                report.pictures_written.push(path);
            }
            Err(e) => {
                report
                    .picture_failures
                    .push(Failure::new(&picture.source, e.to_string()));
            }
        }
    }

    // 4. Notes: each created at the name the plan gave it, never over a file.
    let mut written: Vec<usize> = Vec::new();
    for (i, note) in req.notes.iter().enumerate() {
        if !inside_any(&note.path, req) {
            report.note_failures.push(Failure::new(
                &note.path,
                "it is outside the library and the group's folder",
            ));
            continue;
        }
        match create_new(&note.path, note.text.as_bytes()) {
            Ok(()) => {
                written.push(i);
                if let Some(rec) = record.notes.get_mut(i) {
                    rec.created = true;
                }
                report.notes_written.push(note.path.clone());
                record.note_done(&record_path, &format!("note {}", shown(&note.path)));
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                // 3.9: the same name with the same bytes is the note, already there. A file
                // with other bytes is never touched, and the note is reported.
                match std::fs::read(&note.path) {
                    Ok(bytes) if bytes == note.text.as_bytes() => {
                        written.push(i);
                        report.notes_present.push(note.path.clone());
                    }
                    Ok(_) => report.note_failures.push(Failure::new(
                        &note.path,
                        "a file of that name is there with other contents",
                    )),
                    Err(e) => report
                        .note_failures
                        .push(Failure::new(&note.path, e.to_string())),
                }
            }
            Err(e) => report
                .note_failures
                .push(Failure::new(&note.path, e.to_string())),
        }
    }

    // 5. The read-back: each note read again and compared with the lines it was made from. A
    // note that does not read back is neither linked nor counted, and would stop the Recycle
    // Bin step (not built: phase T).
    let mut sound: Vec<usize> = Vec::new();
    for &i in &written {
        let note = &req.notes[i];
        match verify(&note.path, &note.from_source) {
            Ok(()) => sound.push(i),
            Err(why) => {
                report.check_failures.push(Failure::new(&note.path, why));
                record.note_done(&record_path, &format!("check failed {}", shown(&note.path)));
            }
        }
    }

    // What the run has made so far goes into the record now, before any library file is touched:
    // a run that stops after this point can be undone, and undo knows exactly which folders and
    // notes are its own. A failure to write it again is reported by step 7, which tries once more.
    let _ = record.close(&record_path, &req.library);

    // 6. The library: the group's file, then each file's links in one atomic write.
    if !sound.is_empty() {
        let group_ready = if req.group.created {
            match create_group(req) {
                Ok(()) => {
                    record.group.created = true;
                    let _ = record.close(&record_path, &req.library);
                    record.note_done(&record_path, &format!("group {}", shown(&req.group.file)));
                    true
                }
                Err(e) => {
                    report.group_failure = Some(Failure::new(&req.group.file, e.to_string()));
                    false
                }
            }
        } else {
            true
        };
        if group_ready {
            let links: Vec<Link> = sound.iter().map(|&i| Link::new(&req.notes[i])).collect();
            // The headings the file does not have yet are the run's own, for undo.
            let before = std::fs::read_to_string(&req.group.file).ok().map(|text| {
                library::parse(&text, req.group.file.parent().unwrap_or(Path::new("")))
            });
            let mut added: Vec<Vec<String>> = Vec::new();
            for note in sound.iter().map(|&i| &req.notes[i]) {
                for n in 1..=note.sections.len() {
                    let path: Vec<String> = note.sections[..n].to_vec();
                    let known = before
                        .as_ref()
                        .is_some_and(|p| p.sections.iter().any(|s| sections_eq(s, &path)));
                    if !known && !added.iter().any(|s| sections_eq(s, &path)) {
                        added.push(path);
                    }
                }
            }
            match gate.add_links(&req.group.file, &links) {
                Ok(()) => {
                    record.sections_added = added;
                    report.linked.push(req.group.file.clone());
                    record.note_done(
                        &record_path,
                        &format!("links {} in {}", links.len(), shown(&req.group.file)),
                    );
                }
                Err(why) => report
                    .link_failures
                    .push(Failure::new(&req.group.file, why)),
            }
        }
    }

    // 7. The record is closed. A run that could not update a library file says so.
    record.state = if report.link_failures.is_empty() && report.group_failure.is_none() {
        State::Done
    } else {
        State::LinksPending
    };
    match record.close(&record_path, &req.library) {
        Ok(()) => {
            report.closed = true;
            record.note_done(&record_path, record.state.as_str());
        }
        Err(e) => report.close_error = Some(e.to_string()),
    }
    // What the run wrote, when it knows: the notes that were sound and linked keep the digest
    // of their bytes; a note that failed keeps the digest it was planned with.
    if report.closed {
        prune(&req.library, secs);
    }
    report.record = Some(record);
    report
}

/// The text a note must hold, read from disk; `Err` says it does not read back the same.
pub fn verify(path: &Path, expected: &str) -> Result<(), String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    if bytes == expected.as_bytes() {
        Ok(())
    } else {
        Err("does not read back the same".to_string())
    }
}

/// Writes `bytes` as `stem.ext`, or `stem 2.ext` when that is taken, and never over a file. The
/// second value is false when a file with exactly these bytes was there already.
fn create_copy(folder: &Path, stem: &str, ext: &str, bytes: &[u8]) -> io::Result<(PathBuf, bool)> {
    for n in 1..=999 {
        let name = if ext.is_empty() {
            if n == 1 {
                stem.to_string()
            } else {
                format!("{stem} {n}")
            }
        } else if n == 1 {
            format!("{stem}.{ext}")
        } else {
            format!("{stem} {n}.{ext}")
        };
        let path = folder.join(name);
        match create_new(&path, bytes) {
            Ok(()) => return Ok((path, true)),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                if std::fs::read(&path).is_ok_and(|old| old == bytes) {
                    return Ok((path, false));
                }
            }
            Err(e) => return Err(e),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "every name for it is taken",
    ))
}

/// Makes the group's library file, refusing when the library already holds as many library files
/// as the reader takes. The file is created exclusively: an existing file is never replaced.
fn create_group(req: &Request) -> io::Result<()> {
    if let Ok(entries) = std::fs::read_dir(&req.library) {
        let count = entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .is_some_and(|x| x.eq_ignore_ascii_case("md"))
            })
            .count();
        if count >= library::MAX_FILES {
            return Err(io::Error::other(format!(
                "The library already has {} files.",
                library::MAX_FILES
            )));
        }
    }
    let name = req.group.name.trim();
    let home = written_home(&req.group.home, &req.library);
    let text = library::set_group_home(&format!("# {name}\n"), &home)
        .ok_or_else(|| io::Error::other("the group has no name"))?;
    let read = library::parse(&text, &req.library);
    if read.name.as_deref() != Some(name) || read.home.is_none() {
        return Err(io::Error::other(
            "the file would not read back as its notes",
        ));
    }
    create_new(&req.group.file, text.as_bytes())
}

/// A folder as a library file writes it: relative to the library when it is inside it, so the
/// library can be moved as a whole.
fn written_home(home: &Path, library: &Path) -> PathBuf {
    match home.strip_prefix(library) {
        Ok(inside) if !inside.as_os_str().is_empty() => inside.to_path_buf(),
        _ => home.to_path_buf(),
    }
}

impl Group {
    /// The name the library file is made from: its file name without `.md`.
    pub fn file_stem(&self) -> String {
        self.file
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
    }
}

fn shown(path: &Path) -> String {
    path.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// Whether `path` is in the library folder or in the group's folder. Nothing is written outside
/// those, and undo removes nothing outside them either: the one rule, in one place.
///
/// No library folder, or no home, is nothing: an empty path is not a folder that holds
/// everything. A path is compared as the disk compares names (part by part, in the platform's
/// case), after the disk has been asked where its links lead.
fn inside(library: &Path, home: &Path, path: &Path) -> bool {
    if library.as_os_str().is_empty() {
        return false;
    }
    let path = mdshared_app::paths::canonical_or_absolute(path);
    let library = mdshared_app::paths::canonical_or_absolute(library);
    if under(&path, &library) && !same_path(&path, &library) {
        return true;
    }
    if home.as_os_str().is_empty() {
        return false;
    }
    under(&path, &mdshared_app::paths::canonical_or_absolute(home))
}

/// Whether `path` is `base` or lies below it, part by part, with the case rule of
/// [`same_path`]. No disk is asked.
fn under(path: &Path, base: &Path) -> bool {
    let mut parts = path.components();
    base.components().all(|b| {
        parts
            .next()
            .is_some_and(|p| same_path(Path::new(p.as_os_str()), Path::new(b.as_os_str())))
    })
}

/// [`inside`] for a request's two folders.
fn inside_any(path: &Path, req: &Request) -> bool {
    inside(&req.library, &req.group.home, path)
}

/// Why the request must not be written, or `None`.
fn outside(req: &Request) -> Option<String> {
    let home = &req.group.home;
    for folder in &req.folders {
        if !inside_any(folder, req) {
            return Some(format!(
                "{} is outside the library and the group's folder.",
                folder.display()
            ));
        }
    }
    for note in &req.notes {
        if !inside_any(&note.path, req) {
            return Some(format!(
                "{} is outside the library and the group's folder.",
                note.path.display()
            ));
        }
    }
    for picture in &req.pictures {
        if !inside_any(&picture.folder, req) {
            return Some(format!(
                "{} is outside the library and the group's folder.",
                picture.folder.display()
            ));
        }
    }
    if !inside_any(&req.group.file, req) && home != &req.library {
        return Some(format!(
            "{} is outside the library folder.",
            req.group.file.display()
        ));
    }
    None
}

/// The library text with every link added, in the order given, one call of the library's own
/// writer per link. A note under no section goes first, so that it lands before the first
/// section heading rather than at the end of the file under the last one. Undo removes them in
/// the reverse order.
pub fn with_links_added(text: &str, links: &[Link]) -> String {
    let mut out = text.to_string();
    let mut ordered: Vec<&Link> = links.iter().filter(|l| l.sections.is_empty()).collect();
    ordered.extend(links.iter().filter(|l| !l.sections.is_empty()));
    for link in ordered {
        out = library::add_note(&out, &link.sections, &link.title, &link.path);
    }
    out
}

/// Whether `new` is exactly `old`'s notes plus `links`, each under its section: the check step 6
/// makes before an atomic write. A count is not enough. `base` resolves a link's written path the
/// way the library file does.
pub fn check_links_added(old: &Parsed, new: &Parsed, links: &[Link], base: &Path) -> bool {
    let same = |e: &library::Entry, l: &Link| {
        let target = link_target(&l.path, base);
        same_path(&e.path, &target)
            && sections_eq(&e.sections, &l.sections)
            && e.title.trim() == l.title.trim().replace(['[', ']'], "").trim()
    };
    if new.entries.len() != old.entries.len() + links.len() {
        return false;
    }
    // Every note the file had is still there, once per occurrence.
    let mut left: Vec<&library::Entry> = new.entries.iter().collect();
    for e in &old.entries {
        let at = left.iter().position(|n| {
            same_path(&n.path, &e.path) && n.sections == e.sections && n.title == e.title
        });
        match at {
            Some(i) => {
                left.remove(i);
            }
            None => return false,
        }
    }
    // And each planned link is among what is left, under its section.
    for l in links {
        let at = left.iter().position(|n| same(n, l));
        match at {
            Some(i) => {
                left.remove(i);
            }
            None => return false,
        }
    }
    left.is_empty()
}

/// The library text with every recorded link taken out: only a line that is still exactly the
/// recorded line under the recorded section goes. `None` when not one of them was found.
pub fn with_links_removed(text: &str, links: &[LinkRemoval], base: &Path) -> Option<String> {
    let mut out = text.to_string();
    let mut removed = false;
    for link in links {
        let parsed = library::parse(&out, base);
        let target = link_target(&link.path, base);
        let at = parsed.entries.iter().find(|e| {
            same_path(&e.path, &target)
                && sections_eq(&e.sections, &link.sections)
                && out
                    .lines()
                    .nth(e.line.saturating_sub(1))
                    .is_some_and(|line| line.trim_end() == link.line.trim_end())
        });
        if let Some(e) = at
            && let Some(new) = library::remove_note(&out, e.line)
        {
            out = new;
            removed = true;
        }
    }
    removed.then_some(out)
}

/// Whether `new` is `old` without exactly the removals: the check step 2 of undo makes.
pub fn check_links_removed(
    old: &Parsed,
    new: &Parsed,
    removals: &[LinkRemoval],
    base: &Path,
) -> bool {
    if new.entries.len() + removals.len() != old.entries.len() {
        return false;
    }
    let mut left: Vec<&library::Entry> = old.entries.iter().collect();
    for e in &new.entries {
        let at = left.iter().position(|n| {
            same_path(&n.path, &e.path) && n.sections == e.sections && n.title == e.title
        });
        match at {
            Some(i) => {
                left.remove(i);
            }
            None => return false,
        }
    }
    // What is left is the removals, each of them, under its section.
    for l in removals {
        let target = link_target(&l.path, base);
        let at = left
            .iter()
            .position(|n| same_path(&n.path, &target) && sections_eq(&n.sections, &l.sections));
        match at {
            Some(i) => {
                left.remove(i);
            }
            None => return false,
        }
    }
    left.is_empty()
}

/// The library text with the section headings the run wrote taken out, when nothing is under
/// them: deepest first, and only a heading whose path and place are still there.
pub fn with_headings_removed(text: &str, headings: &[Vec<String>], base: &Path) -> Option<String> {
    let mut out = text.to_string();
    let mut changed = false;
    let mut ordered = headings.to_vec();
    ordered.sort_by_key(|h| std::cmp::Reverse(h.len()));
    for heading in ordered {
        let parsed = library::parse(&out, base);
        let under = parsed
            .entries
            .iter()
            .any(|e| sections_eq(&e.sections[..e.sections.len().min(heading.len())], &heading));
        if under {
            continue;
        }
        let Some(line) = library::section_line(&out, &heading) else {
            continue;
        };
        let Some(text_line) = out.lines().nth(line.saturating_sub(1)) else {
            continue;
        };
        if !text_line.trim_start().starts_with('#') {
            continue;
        }
        let mut lines: Vec<&str> = out.lines().collect();
        if line == 0 || line > lines.len() {
            continue;
        }
        lines.remove(line - 1);
        // The file's own line ending, found the way `mdshared_app::library`'s writers find it,
        // so a CRLF file stays CRLF and a file that ended with no newline keeps none.
        let eol = if out.contains("\r\n") { "\r\n" } else { "\n" };
        let mut new = lines.join(eol);
        if out.ends_with('\n') {
            new.push_str(eol);
        }
        out = new;
        changed = true;
    }
    changed.then_some(out)
}

fn sections_eq(a: &[String], b: &[String]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|(x, y)| x.trim().eq_ignore_ascii_case(y.trim()))
}

/// A link's path as the library file resolves it.
fn link_target(path: &Path, base: &Path) -> PathBuf {
    resolved(&path.to_string_lossy(), base)
}

// ---- undo ------------------------------------------------------------------------------------------

/// What undo would do to one run, worked out from the record and the disk as they are now.
#[derive(Clone, Debug, Default)]
pub struct UndoPlan {
    /// Why undo cannot run at all.
    pub refusal: Option<String>,
    /// The links to take out, per library file.
    pub links: Vec<(PathBuf, Vec<LinkRemoval>)>,
    /// Lines that are gone already: nothing to do.
    pub links_gone: usize,
    /// Notes whose line the owner edited or moved since: the line is kept, and so is the note.
    pub links_kept: Vec<PathBuf>,
    /// Notes that stay because a library line still lists them after this run's lines are out.
    pub held: Vec<PathBuf>,
    /// Files the run found there already, with the bytes it would have written: they are linked,
    /// undo takes their lines out, and the files are not its own to remove.
    pub before: Vec<PathBuf>,
    /// The files this run wrote, with the digest they had: removed only while it matches.
    pub notes: Vec<PathBuf>,
    pub pictures: Vec<PathBuf>,
    /// Files changed since: kept, and listed.
    pub kept: Vec<PathBuf>,
    /// The notes among them, and the pictures, so the question can name each kind (5.9).
    pub notes_kept: Vec<PathBuf>,
    pub pictures_kept: Vec<PathBuf>,
    pub gone: Vec<PathBuf>,
    /// The group's file, when the run made it.
    pub group_file: Option<PathBuf>,
    /// The folders the run made, deepest first.
    pub folders: Vec<PathBuf>,
}

impl UndoPlan {
    /// Whether anything would happen.
    pub fn acts(&self) -> bool {
        self.refusal.is_none()
            && (!self.notes.is_empty()
                || !self.pictures.is_empty()
                || !self.links.is_empty()
                || self.group_file.is_some()
                || !self.folders.is_empty())
    }

    /// The question's sentence: what goes, and what stays.
    pub fn sentence(&self, group: &str) -> String {
        if let Some(why) = &self.refusal {
            return why.clone();
        }
        let mut goes: Vec<String> = Vec::new();
        if !self.notes.is_empty() {
            goes.push(count(self.notes.len(), "note", "notes"));
        }
        if !self.pictures.is_empty() {
            goes.push(count(self.pictures.len(), "picture", "pictures"));
        }
        let mut s = if goes.is_empty() && self.links.is_empty() {
            "Nothing of this run is left to remove.".to_string()
        } else {
            let mut s;
            if goes.is_empty() {
                s = "Removes the lines this run added".to_string();
            } else {
                s = format!("Removes {} this run wrote", goes.join(" and "));
            }
            if !self.links.is_empty() {
                let lines: usize = self.links.iter().map(|(_, l)| l.len()).sum();
                if goes.is_empty() {
                    s.push_str(&format!(": {lines} in {group}"));
                } else {
                    s.push_str(&format!(", their {lines} lines in {group}"));
                }
            }
            if !self.folders.is_empty() {
                s.push_str(&format!(
                    ", and {} if {} empty",
                    count(self.folders.len(), "folder", "folders"),
                    if self.folders.len() == 1 {
                        "it is"
                    } else {
                        "they are"
                    }
                ));
            }
            s.push('.');
            s
        };
        if !self.kept.is_empty() {
            let mut said: Vec<String> = Vec::new();
            if !self.notes_kept.is_empty() {
                said.push(format!(
                    "{} changed since",
                    count(self.notes_kept.len(), "note was", "notes were")
                ));
            }
            if !self.pictures_kept.is_empty() {
                said.push(format!(
                    "{} changed since",
                    count(self.pictures_kept.len(), "picture was", "pictures were")
                ));
            }
            s.push_str(&format!(
                " {} and {} kept.",
                said.join(", "),
                if self.kept.len() == 1 { "is" } else { "are" }
            ));
        }
        if !self.links_kept.is_empty() {
            s.push_str(&format!(
                " {} edited or moved since and {} kept, with {}.",
                count(self.links_kept.len(), "line was", "lines were"),
                if self.links_kept.len() == 1 {
                    "is"
                } else {
                    "are"
                },
                if self.links_kept.len() == 1 {
                    "its note"
                } else {
                    "their notes"
                }
            ));
        }
        let other: Vec<&PathBuf> = self
            .held
            .iter()
            .filter(|h| !self.links_kept.iter().any(|k| same_path(k, h)))
            .collect();
        if !other.is_empty() {
            s.push_str(&if other.len() == 1 {
                " 1 note stays, because another line in the library lists it.".to_string()
            } else {
                format!(
                    " {} notes stay, because other lines in the library list them.",
                    other.len()
                )
            });
        }
        if !self.before.is_empty() {
            s.push_str(&format!(
                " {} there before this run and {} kept.",
                count(self.before.len(), "file was", "files were"),
                if self.before.len() == 1 { "is" } else { "are" }
            ));
        }
        s
    }
}

fn count(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {many}")
    }
}

/// The text of a regular file of at most `max` bytes, read whole and no further; `None` when it
/// is not a regular file, is bigger, or cannot be read. With `follow` a link is followed, as it
/// is for a source document; without it a link is not a regular file, which is what undo wants
/// of anything it is about to delete.
fn read_regular(path: &Path, max: u64, follow: bool) -> Option<Vec<u8>> {
    use std::io::Read;
    let meta = if follow {
        std::fs::metadata(path)
    } else {
        std::fs::symlink_metadata(path)
    }
    .ok()?;
    if !meta.is_file() || meta.len() > max {
        return None;
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(max.saturating_add(1))
        .read_to_end(&mut bytes)
        .ok()?;
    (bytes.len() as u64 <= max).then_some(bytes)
}

fn digest_of(path: &Path) -> Option<String> {
    let bytes = read_regular(path, DIGEST_MAX_BYTES, false)?;
    Some(mdshared_app::update::sha256_hex(&bytes))
}

fn has_parent_dir(path: &Path) -> bool {
    path.components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
}

fn has_extension(path: &Path, allowed: &[&str]) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| allowed.iter().any(|a| e.eq_ignore_ascii_case(a)))
}

/// Where the document a run was cut from stands now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SourceState {
    /// The file has the recorded length and digest.
    Same,
    Gone,
    /// There is a file, and it is not the recorded one.
    Changed,
    /// It is the recorded length, and too large to be compared.
    TooLarge,
}

fn source_state(source: &Source) -> SourceState {
    let Ok(meta) = std::fs::metadata(&source.path) else {
        return SourceState::Gone;
    };
    if !meta.is_file() || meta.len() != source.len {
        return SourceState::Changed;
    }
    if source.len > DIGEST_MAX_BYTES {
        return SourceState::TooLarge;
    }
    match read_regular(&source.path, DIGEST_MAX_BYTES, true) {
        Some(bytes)
            if mdshared_app::update::sha256_hex(&bytes).eq_ignore_ascii_case(&source.sha256) =>
        {
            SourceState::Same
        }
        Some(_) => SourceState::Changed,
        None => SourceState::Gone,
    }
}

/// Why undo does not run when the original is not the file the run was cut from. The notes may
/// be the only copy of its text, and undo is the one thing here that deletes.
fn source_refusal(record: &Record, state: SourceState) -> String {
    let name = record.source_name();
    if record.source.recycled {
        return format!("Put \u{2018}{name}\u{2019} back from the Recycle Bin first, then undo.");
    }
    let what = match state {
        SourceState::Gone => format!(
            "\u{2018}{name}\u{2019}, the file these notes were cut from, is not where it was"
        ),
        SourceState::Changed => {
            format!("\u{2018}{name}\u{2019} is not the file these notes were cut from any more")
        }
        _ => format!("\u{2018}{name}\u{2019} is too large to be checked against this run"),
    };
    format!(
        "{what}, so these notes may be the only copy of its text. Undo will not remove them. \
         Put the original back, or delete the notes yourself if you do not need them."
    )
}

// ---- what a record may reach -------------------------------------------------------------------

/// What a record may name, and so what undo and finish may touch for it: the library folder, and
/// the home the group's library file declares now. A record is a file on disk like any other, so
/// it is not given more than write access to the library folder gives: a path in it is never
/// taken as it stands, and the home is read from the library file and not from the record.
struct Reach {
    library: PathBuf,
    home: Option<PathBuf>,
}

impl Reach {
    /// The reach of `record` in `library`, or the sentence that says which path of the record is
    /// outside it, so that none of the record is acted on.
    fn of(record: &Record, library: &Path) -> Result<Reach, String> {
        if !library.is_absolute() {
            return Err("The library folder is not known, so this run is not undone.".to_string());
        }
        let reach = Reach {
            library: library.to_path_buf(),
            home: declared_home(&record.group.file, library),
        };
        let outside = |path: &Path| {
            format!(
                "{} is outside the library and the group's folder, so this run is not undone.",
                path.display()
            )
        };
        let foreign = |path: &Path| {
            format!(
                "{} is not a file a run writes, so this run is not undone.",
                path.display()
            )
        };
        let file = record.group.file.as_path();
        if !reach.is_library_file(file) {
            return Err(outside(file));
        }
        for folder in &record.folders {
            if !reach.allows(folder) {
                return Err(outside(folder));
            }
        }
        for note in &record.notes {
            if !reach.allows(&note.path) {
                return Err(outside(&note.path));
            }
            if !has_extension(&note.path, &["md"]) {
                return Err(foreign(&note.path));
            }
        }
        for picture in &record.pictures {
            if !reach.allows(&picture.path) {
                return Err(outside(&picture.path));
            }
            if !has_extension(&picture.path, &PICTURE_EXTENSIONS) {
                return Err(foreign(&picture.path));
            }
        }
        Ok(reach)
    }

    /// A path with no `..`, inside the library folder (not the folder itself) or inside the home.
    fn allows(&self, path: &Path) -> bool {
        !has_parent_dir(path)
            && inside(
                &self.library,
                self.home.as_deref().unwrap_or(Path::new("")),
                path,
            )
    }

    /// A `.md` file directly in the library folder: where a group's library file is, and the one
    /// place undo writes besides the files a run made.
    fn is_library_file(&self, path: &Path) -> bool {
        !has_parent_dir(path)
            && has_extension(path, &["md"])
            && path.parent().is_some_and(|dir| {
                same_path(
                    &mdshared_app::paths::canonical_or_absolute(dir),
                    &mdshared_app::paths::canonical_or_absolute(&self.library),
                )
            })
    }
}

/// The folder the group's library file names with `home:` as it reads now; `None` when the file
/// is not there, names none, names one with a `..` in it, or names the library folder or a folder
/// that holds it.
fn declared_home(group_file: &Path, library: &Path) -> Option<PathBuf> {
    let bytes = read_regular(group_file, library::MAX_FILE_BYTES as u64, true)?;
    let text = mdshared_app::files::decode(&bytes).0.into_owned();
    let home = library::parse(&text, library).home?;
    if home.as_os_str().is_empty() || has_parent_dir(&home) {
        return None;
    }
    let held = under(
        &mdshared_app::paths::canonical_or_absolute(library),
        &mdshared_app::paths::canonical_or_absolute(&home),
    );
    (!held).then_some(home)
}

// ---- what the library lists --------------------------------------------------------------------

/// One note line of one library file, as the file holds it now.
struct Listing {
    file: PathBuf,
    path: PathBuf,
    /// The name of the file path leads to, in lower case: the cheap test before the real one.
    name: String,
    sections: Vec<String>,
    /// The line's text, as it is in the file.
    line: String,
}

/// Every note line of every library file, read from the files and not from what the window has
/// in memory.
struct Listed {
    /// False when the library could not be read in full, so that what is not listed may only be
    /// what was not seen.
    complete: bool,
    entries: Vec<Listing>,
}

/// Reads the library folder again: every `.md` file directly in it, as the reader takes them, a
/// file of more than the reader's cap as the reader takes it (it lists nothing). The reader keeps
/// at most [`library::MAX_FILES`] files; this looks at more, so a note listed by the 513th file is
/// still seen, and says it is not complete past [`LISTING_MAX_FILES`].
fn list_library(library: &Path) -> Listed {
    let mut listed = Listed {
        complete: true,
        entries: Vec::new(),
    };
    let Ok(dir) = std::fs::read_dir(library) else {
        listed.complete = false;
        return listed;
    };
    let mut files: Vec<PathBuf> = dir
        .flatten()
        .map(|e| e.path())
        .filter(|p| has_extension(p, &["md"]) && p.is_file())
        .collect();
    files.sort();
    if files.len() > LISTING_MAX_FILES {
        listed.complete = false;
        files.truncate(LISTING_MAX_FILES);
    }
    for file in files {
        let too_large =
            std::fs::metadata(&file).is_ok_and(|m| m.len() > library::MAX_FILE_BYTES as u64);
        if too_large {
            continue;
        }
        let Some(bytes) = read_regular(&file, library::MAX_FILE_BYTES as u64, true) else {
            listed.complete = false;
            continue;
        };
        let text = mdshared_app::files::decode(&bytes).0.into_owned();
        let lines: Vec<&str> = text.lines().collect();
        for e in library::parse(&text, library).entries {
            listed.entries.push(Listing {
                name: lower_name(&e.path),
                line: lines
                    .get(e.line.saturating_sub(1))
                    .copied()
                    .unwrap_or_default()
                    .to_string(),
                file: file.clone(),
                path: e.path,
                sections: e.sections,
            });
        }
    }
    listed
}

fn lower_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

/// The lines of listed that name path.
fn named_in<'a>(listed: &'a Listed, path: &Path) -> Vec<&'a Listing> {
    let name = lower_name(path);
    listed
        .entries
        .iter()
        .filter(|l| l.name == name && same_file(&l.path, path))
        .collect()
}

/// Whether two paths are one file: spelled alike, or, when their names are alike, leading to the
/// same place once the disk is asked. A mistake in the first direction keeps a note; one in the
/// other would delete a note a line still lists, so the disk is asked when in doubt.
fn same_file(a: &Path, b: &Path) -> bool {
    if same_path(a, b) {
        return true;
    }
    match (a.file_name(), b.file_name()) {
        (Some(x), Some(y)) if same_path(Path::new(x), Path::new(y)) => same_path(
            &mdshared_app::paths::canonical_or_absolute(a),
            &mdshared_app::paths::canonical_or_absolute(b),
        ),
        _ => false,
    }
}

/// Works out what undo would do, reading the disk and never writing it.
pub fn prepare(record: &Record, library: &Path) -> UndoPlan {
    plan_of(record, library).0
}

fn plan_of(record: &Record, library: &Path) -> (UndoPlan, Option<Reach>) {
    let mut plan = UndoPlan::default();
    if record.state == State::Undone {
        plan.refusal = Some("This run is undone already.".to_string());
        return (plan, None);
    }
    // Every path the record names must be one a write could have made, and is checked against
    // the library folder and the home the group's library file declares now, never against the
    // record's own word (4.1, 6.2). One that is not makes it a record this program did not
    // write: none of it is acted on, and nothing is removed.
    let reach = match Reach::of(record, library) {
        Ok(reach) => reach,
        Err(why) => {
            plan.refusal = Some(why);
            return (plan, None);
        }
    };
    // The original. Undo is the one thing that deletes, so it does not run unless the notes can
    // not be the only copy: the original is where it was, with the length and the digest it had.
    // A source in the Recycle Bin must be back for the same reason.
    let state = source_state(&record.source);
    if state != SourceState::Same {
        plan.refusal = Some(source_refusal(record, state));
        return (plan, None);
    }
    let listed = list_library(library);
    if !listed.complete {
        plan.refusal = Some(
            "The library could not be read in full, so there is no telling which notes a line \
             still lists. Nothing was removed."
                .to_string(),
        );
        return (plan, None);
    }
    // Each recorded note: its line, then its file.
    let mut removals: Vec<LinkRemoval> = Vec::new();
    for note in &record.notes {
        let named = named_in(&listed, &note.path);
        let in_group: Vec<&&Listing> = named
            .iter()
            .filter(|l| same_path(&l.file, &record.group.file))
            .collect();
        let exact = in_group.iter().any(|l| {
            sections_eq(&l.sections, &note.sections) && l.line.trim_end() == note.line.trim_end()
        });
        // The file first: a note changed since is kept, and so is its line.
        let mut file_goes = false;
        if !note.created {
            plan.before.push(note.path.clone());
        } else if note.path.symlink_metadata().is_err() {
            plan.gone.push(note.path.clone());
        } else if digest_of(&note.path).is_some_and(|d| d.eq_ignore_ascii_case(&note.sha256)) {
            file_goes = true;
        } else {
            plan.kept.push(note.path.clone());
            plan.notes_kept.push(note.path.clone());
            continue;
        }
        if exact {
            removals.push(LinkRemoval {
                path: note.path.clone(),
                sections: note.sections.clone(),
                line: note.line.clone(),
            });
        } else if in_group.is_empty() {
            // Not in the file: the line is gone, or the notes were never linked.
            plan.links_gone += 1;
        } else {
            // The line is there and is not the recorded one, or is under another section: the
            // owner edited or moved it, and it is kept.
            plan.links_kept.push(note.path.clone());
        }
        // Whatever still lists the note after the removals keeps it: the line of the owner's,
        // a line in another library file, a second line of the same file.
        let left = named.len() - usize::from(exact);
        if file_goes {
            if left == 0 {
                plan.notes.push(note.path.clone());
            } else {
                plan.held.push(note.path.clone());
            }
        }
    }
    for picture in &record.pictures {
        if !picture.created {
            plan.before.push(picture.path.clone());
        } else if picture.path.symlink_metadata().is_err() {
            plan.gone.push(picture.path.clone());
        } else if digest_of(&picture.path).is_some_and(|d| d.eq_ignore_ascii_case(&picture.sha256))
        {
            plan.pictures.push(picture.path.clone());
        } else {
            plan.kept.push(picture.path.clone());
            plan.pictures_kept.push(picture.path.clone());
        }
    }
    if !removals.is_empty() {
        plan.links = vec![(record.group.file.clone(), removals)];
    }
    // The group's file when this run made it, and the folders this run made, deepest first.
    if record.group.created && record.group.file.is_file() {
        plan.group_file = Some(record.group.file.clone());
    }
    let mut folders = record.folders.clone();
    folders.sort_by_key(|f| std::cmp::Reverse(f.components().count()));
    plan.folders = folders;
    (plan, Some(reach))
}

/// What undo did.
#[derive(Clone, Debug, Default)]
pub struct UndoReport {
    pub refusal: Option<String>,
    pub plan: Option<UndoPlan>,
    pub removed_notes: Vec<PathBuf>,
    pub removed_pictures: Vec<PathBuf>,
    pub removed_links: usize,
    pub kept: Vec<PathBuf>,
    pub links_kept: Vec<PathBuf>,
    /// Notes this run did not create: they were there already, and stay.
    pub before: Vec<PathBuf>,
    /// Notes kept because a library file still lists them, whether the line could not be taken
    /// out, the owner edited or moved it, or another file lists the note too.
    pub held_back: Vec<PathBuf>,
    pub group_removed: bool,
    pub group_kept: bool,
    pub folders_removed: Vec<PathBuf>,
    pub folders_kept: Vec<PathBuf>,
    /// Library files that could not be updated, and why.
    pub failures: Vec<Failure>,
    /// The record was marked "undone".
    pub marked: bool,
    pub mark_error: Option<String>,
}

/// The library text with the run's lines taken out and then the section headings the run wrote
/// that have nothing under them: both removals in one text, `Some` when either changed
/// anything. The one composition every gate uses, the window's and the tests', so that what the
/// tests check is what the program does.
pub fn with_removals(
    text: &str,
    links: &[LinkRemoval],
    headings: &[Vec<String>],
    base: &Path,
) -> Option<String> {
    let without = with_links_removed(text, links, base);
    let swept = with_headings_removed(without.as_deref().unwrap_or(text), headings, base);
    swept.or(without)
}

/// Undoes one run: the links, then the files, then the group's file, then the folders, and the
/// record is marked "undone" and kept. A run whose links could not be taken out is not undone:
/// the notes those lines name stay, and the record says "links pending" so it can be tried again.
/// A note is removed only when, after the links step, no library file lists it: the library is
/// read again for that, so whatever kept a line (an edit, a move, a refusal) keeps its note.
pub fn undo(path: &Path, record: &Record, library: &Path, gate: &mut dyn Gate) -> UndoReport {
    let mut report = UndoReport::default();
    let (plan, reach) = plan_of(record, library);
    let Some(reach) = reach.filter(|_| plan.refusal.is_none()) else {
        report.refusal = plan.refusal.clone();
        report.plan = Some(plan);
        return report;
    };
    // 2. The links first, so the pane never shows a link to a file that is gone.
    for (file, removals) in &plan.links {
        let headings = if same_path(file, &record.group.file) {
            record.sections_added.as_slice()
        } else {
            &[]
        };
        match gate.remove_links(file, removals, headings) {
            Ok(()) => report.removed_links += removals.len(),
            Err(why) => report.failures.push(Failure::new(file, why)),
        }
    }
    // 3. The library again, from the files: a note is removed only if no library file lists it
    //    now. The lines the links step could not take out, a line moved or edited since the
    //    plan was made and a second library file that lists the note all keep it here.
    let listed = list_library(library);
    for held in &plan.held {
        report.held_back.push(held.clone());
    }
    let created_note = |p: &Path| {
        record
            .notes
            .iter()
            .find(|n| n.created && same_path(&n.path, p))
    };
    for note in &plan.notes {
        let still_listed = !listed.complete || !named_in(&listed, note).is_empty();
        if still_listed {
            report.held_back.push(note.clone());
            continue;
        }
        let matches = created_note(note).is_some_and(|n| {
            reach.allows(note)
                && has_extension(note, &["md"])
                && digest_of(note).is_some_and(|d| d.eq_ignore_ascii_case(&n.sha256))
        });
        if matches && std::fs::remove_file(note).is_ok() {
            report.removed_notes.push(note.clone());
        } else {
            report.kept.push(note.clone());
        }
    }
    for picture in &plan.pictures {
        let matches = record
            .pictures
            .iter()
            .find(|p| p.created && same_path(&p.path, picture))
            .is_some_and(|p| {
                reach.allows(picture)
                    && has_extension(picture, &PICTURE_EXTENSIONS)
                    && digest_of(picture).is_some_and(|d| d.eq_ignore_ascii_case(&p.sha256))
            });
        if matches && std::fs::remove_file(picture).is_ok() {
            report.removed_pictures.push(picture.clone());
        } else {
            report.kept.push(picture.clone());
        }
    }
    // 4. The group's library file, only when this run made it and nothing but its heading and
    //    its `home:` line is left in it.
    if let Some(file) = &plan.group_file {
        if reach.is_library_file(file)
            && file.symlink_metadata().is_ok_and(|m| m.is_file())
            && only_heading_and_home(file)
        {
            if std::fs::remove_file(file).is_ok() {
                report.group_removed = true;
            } else {
                report.group_kept = true;
            }
        } else {
            report.group_kept = true;
        }
    }
    // 5. The folders this run made, only while empty, and never a link to one.
    for folder in &plan.folders {
        let real = folder
            .symlink_metadata()
            .is_ok_and(|m| m.is_dir() && !m.file_type().is_symlink());
        if real && reach.allows(folder) && std::fs::remove_dir(folder).is_ok() {
            report.folders_removed.push(folder.clone());
        } else {
            report.folders_kept.push(folder.clone());
        }
    }
    // 6. The record is marked and stays in the list, but only when its links went: a run whose
    //    lines are still in a library file is not undone, and undo can be pressed again.
    let mut marked = record.clone();
    marked.state = if report.failures.is_empty() {
        State::Undone
    } else {
        State::LinksPending
    };
    match marked.close(path, library) {
        Ok(()) => report.marked = true,
        Err(e) => report.mark_error = Some(e.to_string()),
    }
    report.kept.extend(plan.kept.iter().cloned());
    report.links_kept = plan.links_kept.clone();
    report.before = plan.before.clone();
    report.plan = Some(plan);
    report
}

/// Whether a library file holds nothing but its group's heading and its `home:` line.
fn only_heading_and_home(path: &Path) -> bool {
    let Some(bytes) = read_regular(path, library::MAX_FILE_BYTES as u64, false) else {
        return false;
    };
    let text = mdshared_app::files::decode(&bytes).0.into_owned();
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    lines.len() == 2
        && lines[0].starts_with("# ")
        && lines[1]
            .split_once(':')
            .is_some_and(|(k, _)| k.trim().eq_ignore_ascii_case("home"))
}

// ---- finishing -----------------------------------------------------------------------------

/// What Finish did: the links it added, and the record it closed.
#[derive(Clone, Debug, Default)]
pub struct FinishReport {
    pub added: usize,
    pub failures: Vec<Failure>,
    pub state: Option<State>,
    pub error: Option<String>,
}

/// Finishes a run that stopped before its links were written: every recorded note that is on
/// disk with the right bytes is linked, and the record is closed. A note that is not there
/// cannot be written again from the record (it holds ranges, not text), and is left out.
pub fn finish(path: &Path, record: &Record, library: &Path, gate: &mut dyn Gate) -> FinishReport {
    let mut report = FinishReport::default();
    if !record.state.live() {
        report.error = Some("This run is undone already.".to_string());
        return report;
    }
    // Finish writes into the group's library file, so it reaches no further than undo does.
    if let Err(why) = Reach::of(record, library) {
        report.error = Some(why);
        return report;
    }
    let present = read_regular(&record.group.file, library::MAX_FILE_BYTES as u64, true)
        .map(|b| library::parse(&mdshared_app::files::decode(&b).0, library));
    let mut links: Vec<Link> = Vec::new();
    for note in &record.notes {
        if !digest_of(&note.path).is_some_and(|d| d.eq_ignore_ascii_case(&note.sha256)) {
            continue;
        }
        let already = present.as_ref().is_some_and(|p| {
            p.entries
                .iter()
                .any(|e| same_path(&e.path, &note.path) && sections_eq(&e.sections, &note.sections))
        });
        if already {
            continue;
        }
        // The link is the recorded line, with the path as the line spells it: the same text a
        // write of the run would have put in the file, so that undo finds it as the run's own.
        let (title, written) = link_parts(&note.line);
        let path = if title.is_empty() || written.is_empty() {
            note.path.clone()
        } else {
            PathBuf::from(written)
        };
        links.push(Link {
            path,
            title,
            sections: note.sections.clone(),
            line: note.line.clone(),
        });
    }
    if !links.is_empty() {
        match gate.add_links(&record.group.file, &links) {
            Ok(()) => report.added = links.len(),
            Err(why) => report.failures.push(Failure::new(&record.group.file, why)),
        }
    }
    let mut closed = record.clone();
    closed.state = if report.failures.is_empty() {
        State::Done
    } else {
        State::LinksPending
    };
    match closed.close(path, library) {
        Ok(()) => report.state = Some(closed.state),
        Err(e) => report.error = Some(e.to_string()),
    }
    report
}

/// A recorded link line as its title and its path text: `- [Title](<path>)`.
fn link_parts(line: &str) -> (String, String) {
    let t = line.trim().trim_start_matches("- ").trim();
    if let Some(rest) = t.strip_prefix('[')
        && let Some((title, after)) = rest.split_once("](")
        && let Some(path) = after.strip_suffix(')')
    {
        let path = path.trim();
        let path = path
            .strip_prefix('<')
            .and_then(|p| p.strip_suffix('>'))
            .unwrap_or(path);
        return (title.to_string(), path.replace("%20", " "));
    }
    (String::new(), t.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT: AtomicUsize = AtomicUsize::new(0);

    fn scratch(tag: &str) -> PathBuf {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("mdedit-runs-{tag}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a scratch folder");
        dir
    }

    fn write_file(path: &Path, text: &str) {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).expect("its folder");
        }
        std::fs::write(path, text).expect("a file");
    }

    /// The window's gate, without a window: the same library writers, the same checks, one
    /// atomic write. A test can make it refuse.
    struct TestGate {
        refusals: Vec<PathBuf>,
        added: Vec<(PathBuf, usize)>,
        removed: Vec<(PathBuf, usize)>,
    }

    impl TestGate {
        fn new() -> TestGate {
            TestGate {
                refusals: Vec::new(),
                added: Vec::new(),
                removed: Vec::new(),
            }
        }
    }

    impl Gate for TestGate {
        fn add_links(&mut self, file: &Path, links: &[Link]) -> Result<(), String> {
            if self.refusals.iter().any(|p| same_path(p, file)) {
                return Err("The group's library file has unsaved changes.".to_string());
            }
            let text = std::fs::read_to_string(file).map_err(|e| e.to_string())?;
            let base = file.parent().unwrap_or(Path::new(""));
            let new = with_links_added(&text, links);
            if !check_links_added(
                &library::parse(&text, base),
                &library::parse(&new, base),
                links,
                base,
            ) {
                return Err(format!(
                    "the file would not read back as its notes: old {:?} new {:?} links {:?} text {:?}",
                    library::parse(&text, base)
                        .entries
                        .iter()
                        .map(|e| (&e.path, &e.sections, &e.title))
                        .collect::<Vec<_>>(),
                    library::parse(&new, base)
                        .entries
                        .iter()
                        .map(|e| (&e.path, &e.sections, &e.title))
                        .collect::<Vec<_>>(),
                    links
                        .iter()
                        .map(|l| (&l.path, &l.sections, &l.title))
                        .collect::<Vec<_>>(),
                    new
                ));
            }
            mdshared_app::settings::write_atomic(file, &new).map_err(|e| e.to_string())?;
            self.added.push((file.to_path_buf(), links.len()));
            Ok(())
        }

        fn remove_links(
            &mut self,
            file: &Path,
            links: &[LinkRemoval],
            headings: &[Vec<String>],
        ) -> Result<(), String> {
            if self.refusals.iter().any(|p| same_path(p, file)) {
                return Err("The group's library file has unsaved changes.".to_string());
            }
            let text = std::fs::read_to_string(file).map_err(|e| e.to_string())?;
            let base = file.parent().unwrap_or(Path::new(""));
            let new = with_removals(&text, links, headings, base)
                .ok_or_else(|| "the file was not changed".to_string())?;
            if !check_links_removed(
                &library::parse(&text, base),
                &library::parse(&new, base),
                links,
                base,
            ) {
                return Err("the file would not read back as its notes".to_string());
            }
            mdshared_app::settings::write_atomic(file, &new).map_err(|e| e.to_string())?;
            self.removed.push((file.to_path_buf(), links.len()));
            Ok(())
        }
    }

    /// A plan of three notes in `library/Trip`, and a new group `Trip.md`.
    fn plan(library: &Path, source: &Path, notes: &[(&str, &str, &[&str])]) -> Request {
        let home = library.join("Trip");
        let mut items = Vec::new();
        for (i, (name, text, sections)) in notes.iter().enumerate() {
            let sections: Vec<String> = sections.iter().map(|s| s.to_string()).collect();
            let folder = library::note_folder(&home, &sections);
            let path = folder.join(format!("{name}.md"));
            items.push(NewNote {
                link_path: path
                    .strip_prefix(library)
                    .map(Path::to_path_buf)
                    .unwrap_or(path.clone()),
                path,
                text: text.to_string(),
                from_source: text.to_string(),
                title: name.to_string(),
                sections,
                ranges: vec![(i + 1, i + 1)],
                heading_shift: 0,
                title_added: false,
                already: false,
            });
        }
        let mut folders: Vec<PathBuf> = items
            .iter()
            .filter_map(|n| n.path.parent().map(Path::to_path_buf))
            .collect();
        folders.push(home.clone());
        folders.sort_by_key(|f| f.components().count());
        folders.dedup();
        Request {
            library: library.to_path_buf(),
            source: source.to_path_buf(),
            instruction: String::new(),
            sent: None,
            group: Group {
                name: "Trip".to_string(),
                file: library.join("Trip.md"),
                created: true,
                home,
            },
            folders,
            notes: items,
            pictures: Vec::new(),
        }
    }

    fn source(dir: &Path) -> PathBuf {
        let path = dir.join("Trip journal.md");
        write_file(&path, "# Japan\n\none\n\n## Asakusa\n\ntwo\n");
        path
    }

    fn read(path: &Path) -> String {
        std::fs::read_to_string(path).unwrap_or_default()
    }

    #[test]
    fn a_write_makes_the_folders_the_notes_the_links_and_the_record() {
        let dir = scratch("ok");
        let library = dir.join("library");
        std::fs::create_dir_all(&library).expect("the library");
        let source = source(&dir);
        let req = plan(
            &library,
            &source,
            &[
                ("Asakusa", "# Asakusa\n", &["Tokyo"]),
                ("Nara", "# Nara\n", &[]),
            ],
        );
        let mut gate = TestGate::new();
        let report = write_at(
            &req,
            &mut gate,
            secs_of("2026-11-21T14:03:11Z").expect("the stamp"),
            "2026-11-21T14:03:11Z",
        );
        assert_eq!(report.start_error, None);
        assert_eq!(report.notes(), 2);
        assert!(
            report.note_failures.is_empty(),
            "{:?}",
            report.note_failures
        );
        assert!(
            report.check_failures.is_empty(),
            "{:?}",
            report.check_failures
        );
        assert!(
            report.link_failures.is_empty(),
            "{:?}",
            report.link_failures
        );
        assert_eq!(report.state(), Some(State::Done));
        assert_eq!(read(&library.join("Trip/Tokyo/Asakusa.md")), "# Asakusa\n");
        let group = read(&library.join("Trip.md"));
        assert!(group.starts_with("# Trip\n"), "{group}");
        assert!(group.contains("home:"), "{group}");
        assert!(
            group.contains("- [Asakusa](Trip/Tokyo/Asakusa.md)"),
            "{group}"
        );
        // The record, its done file, and the digest of what was written.
        let record_path = report.record_path.expect("a record");
        assert!(record_path.ends_with("2026-11-21T14-03-11Z-split.json"));
        let text = read(&record_path);
        assert!(text.contains("\"state\": \"done\""), "{text}");
        assert!(text.contains("\"kind\": \"split\""), "{text}");
        let loaded = load(&record_path, &library).expect("the record reads");
        assert_eq!(loaded.notes.len(), 2);
        assert_eq!(
            loaded.notes[0].sha256,
            mdshared_app::update::sha256_hex(b"# Asakusa\n")
        );
        assert_eq!(loaded.state, State::Done);
        assert_eq!(
            loaded.source.sha256,
            mdshared_app::update::sha256_hex(&std::fs::read(&source).expect("the source"))
        );
        assert!(record_path.with_extension("done").is_file());
        // The source is untouched.
        assert_eq!(read(&source), "# Japan\n\none\n\n## Asakusa\n\ntwo\n");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn step_one_leaves_nothing_at_all_when_the_record_cannot_be_written() {
        let dir = scratch("step1");
        let library = dir.join("library");
        std::fs::create_dir_all(&library).expect("the library");
        // A file where the records' folder would go.
        write_file(&library.join(RUNS_DIR), "not a folder");
        let source = source(&dir);
        let req = plan(&library, &source, &[("Asakusa", "# Asakusa\n", &[])]);
        let mut gate = TestGate::new();
        let report = write_at(&req, &mut gate, 1, "2026-11-21T14:03:11Z");
        let why = report.start_error.clone().expect("the run did not start");
        assert!(why.starts_with("The run could not start: "), "{why}");
        // Nothing: no folder, no note, no group, no link.
        let names: Vec<String> = std::fs::read_dir(&library)
            .expect("the library")
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec![RUNS_DIR], "{names:?}");
        assert!(!library.join("Trip").exists());
        assert!(gate.added.is_empty());
        assert_eq!(read(&source), "# Japan\n\none\n\n## Asakusa\n\ntwo\n");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn step_two_leaves_the_notes_that_needed_a_folder_it_could_not_make() {
        let dir = scratch("step2");
        let library = dir.join("library");
        std::fs::create_dir_all(&library).expect("the library");
        // The group's folder is a file: no folder can be made inside it.
        write_file(&library.join("Trip"), "in the way");
        let source = source(&dir);
        let req = plan(&library, &source, &[("Asakusa", "# Asakusa\n", &["Tokyo"])]);
        let mut gate = TestGate::new();
        let report = write_at(&req, &mut gate, 2, "2026-11-21T14:03:11Z");
        assert!(!report.folder_failures.is_empty());
        assert_eq!(report.notes(), 0);
        assert_eq!(report.note_failures.len(), 1);
        assert!(report.linked.is_empty(), "nothing is linked");
        // What is left: the file that was in the way, and a record saying what failed.
        assert_eq!(read(&library.join("Trip")), "in the way");
        let record = read(&report.record_path.clone().expect("a record"));
        assert!(record.contains("\"state\": \"done\""), "{record}");
        // The library file was made and holds no link.
        let group = read(&library.join("Trip.md"));
        assert!(!group.contains("Asakusa"), "{group}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn step_three_leaves_the_picture_out_and_writes_the_note_anyway() {
        let dir = scratch("step3");
        let library = dir.join("library");
        std::fs::create_dir_all(&library).expect("the library");
        let source = source(&dir);
        let mut req = plan(&library, &source, &[("Asakusa", "# Asakusa\n", &[])]);
        req.pictures.push(NewPicture {
            source: dir.join("missing.png"),
            folder: library.join("Trip"),
        });
        let mut gate = TestGate::new();
        let report = write_at(&req, &mut gate, 3, "2026-11-21T14:03:11Z");
        assert_eq!(report.picture_failures.len(), 1);
        assert!(!report.notes_written.is_empty());
        assert!(library.join("Trip/Asakusa.md").is_file());
        assert!(!library.join("Trip/missing.png").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn step_four_writes_the_others_and_never_replaces_the_file_that_was_there() {
        let dir = scratch("step4");
        let library = dir.join("library");
        std::fs::create_dir_all(&library).expect("the library");
        let source = source(&dir);
        let req = plan(
            &library,
            &source,
            &[("One", "# One\n", &[]), ("Two", "# Two\n", &[])],
        );
        // A file of the same name, holding the owner's words.
        write_file(&library.join("Trip/One.md"), "# Mine\n");
        let mut gate = TestGate::new();
        let report = write_at(&req, &mut gate, 4, "2026-11-21T14:03:11Z");
        assert_eq!(report.note_failures.len(), 1);
        assert!(report.note_failures[0].why.contains("other contents"));
        assert_eq!(
            read(&library.join("Trip/One.md")),
            "# Mine\n",
            "never replaced"
        );
        assert_eq!(read(&library.join("Trip/Two.md")), "# Two\n");
        // Only the note that was written is linked.
        let group = read(&library.join("Trip.md"));
        assert!(!group.contains("One"), "{group}");
        assert!(group.contains("Two"), "{group}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn step_five_reports_a_note_that_does_not_read_back_and_leaves_the_file() {
        let dir = scratch("step5");
        let library = dir.join("library");
        std::fs::create_dir_all(&library).expect("the library");
        let source = source(&dir);
        let mut req = plan(&library, &source, &[("One", "# Written\n", &[])]);
        // The text that was written and the lines it was made from do not agree: the check
        // catches what a broken writer would do.
        req.notes[0].from_source = "# Made from the source\n".to_string();
        let mut gate = TestGate::new();
        let report = write_at(&req, &mut gate, 5, "2026-11-21T14:03:11Z");
        assert_eq!(report.check_failures.len(), 1);
        assert_eq!(report.check_failures[0].why, "does not read back the same");
        // What is left: the note as it was written, unlinked.
        assert_eq!(read(&library.join("Trip/One.md")), "# Written\n");
        let group = read(&library.join("Trip.md"));
        assert!(!group.contains("[One]"), "{group}");
        assert!(gate.added.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn step_six_leaves_the_notes_unlinked_and_the_record_says_links_pending() {
        let dir = scratch("step6");
        let library = dir.join("library");
        std::fs::create_dir_all(&library).expect("the library");
        let source = source(&dir);
        let req = plan(
            &library,
            &source,
            &[("One", "# One\n", &[]), ("Two", "# Two\n", &[])],
        );
        let mut gate = TestGate::new();
        gate.refusals.push(library.join("Trip.md"));
        let report = write_at(&req, &mut gate, 6, "2026-11-21T14:03:11Z");
        assert_eq!(report.state(), Some(State::LinksPending));
        assert_eq!(report.link_failures.len(), 1);
        // The notes are there, and the library file names neither.
        assert!(library.join("Trip/One.md").is_file());
        assert!(library.join("Trip/Two.md").is_file());
        let group = read(&library.join("Trip.md"));
        assert!(!group.contains("[One]"), "{group}");
        assert!(!group.contains("[Two]"), "{group}");
        // Finish links them, and closes the record.
        let record_path = report.record_path.clone().expect("a record");
        let record = load(&record_path, &library).expect("the record");
        let mut gate = TestGate::new();
        let done = finish(&record_path, &record, &library, &mut gate);
        assert_eq!(done.added, 2, "{:?}", done.failures);
        assert_eq!(done.state, Some(State::Done));
        let group = read(&library.join("Trip.md"));
        assert!(group.contains("[One]"), "{group}");
        let again = load(&record_path, &library).expect("the record");
        assert_eq!(again.state, State::Done);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn step_seven_leaves_the_run_whole_and_its_undo_refused_for_the_reason() {
        let dir = scratch("step7");
        let library = dir.join("library");
        std::fs::create_dir_all(&library).expect("the library");
        let runs = library.join(RUNS_DIR);
        std::fs::create_dir_all(&runs).expect("the runs folder");
        // A folder where the record's temporary name will be: the record can be made, and not
        // written again.
        std::fs::create_dir_all(runs.join("2026-11-21T14-03-11Z-split.json.tmp"))
            .expect("a folder in the way");
        let source = source(&dir);
        let req = plan(&library, &source, &[("One", "# One\n", &[])]);
        let mut gate = TestGate::new();
        let secs = secs_of("2026-11-21T14:03:11Z").expect("the stamp");
        let report = write_at(&req, &mut gate, secs, "2026-11-21T14:03:11Z");
        assert!(report.close_error.is_some(), "the close failed");
        assert!(!report.closed);
        // The run still counts: the note, the link, and the record at "writing".
        assert!(library.join("Trip/One.md").is_file());
        assert!(read(&library.join("Trip.md")).contains("[One]"));
        let record_path = report.record_path.clone().expect("a record");
        let record = load(&record_path, &library).expect("the record");
        assert_eq!(record.state, State::Writing);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_source_in_the_recycle_bin_that_is_not_back_refuses_undo_and_keeps_every_note() {
        let dir = scratch("undo-source");
        let library = dir.join("library");
        std::fs::create_dir_all(&library).expect("the library");
        let source = source(&dir);
        let req = plan(&library, &source, &[("One", "# One\n", &[])]);
        let mut gate = TestGate::new();
        let report = write_at(&req, &mut gate, 7, "2026-11-21T14:03:11Z");
        let record_path = report.record_path.clone().expect("a record");
        let mut record = load(&record_path, &library).expect("the record");
        // As if phase T had put the source away.
        record.source.recycled = true;
        record.source.sha256 = mdshared_app::update::sha256_hex(b"another file");
        let plan = prepare(&record, &library);
        let why = plan.refusal.clone().expect("refused");
        assert_eq!(
            why,
            "Put \u{2018}Trip journal.md\u{2019} back from the Recycle Bin first, then undo."
        );
        assert!(plan.notes.is_empty(), "nothing would be removed");
        let mut gate = TestGate::new();
        let done = undo(&record_path, &record, &library, &mut gate);
        assert!(done.refusal.is_some());
        assert!(library.join("Trip/One.md").is_file(), "the note stays");
        assert!(read(&library.join("Trip.md")).contains("[One]"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn undo_takes_the_links_out_first_then_the_notes_the_group_and_the_folders() {
        let dir = scratch("undo");
        let library = dir.join("library");
        std::fs::create_dir_all(&library).expect("the library");
        let source = source(&dir);
        let req = plan(
            &library,
            &source,
            &[("One", "# One\n", &["Tokyo"]), ("Two", "# Two\n", &[])],
        );
        let mut gate = TestGate::new();
        let report = write_at(&req, &mut gate, 8, "2026-11-21T14:03:11Z");
        assert_eq!(report.state(), Some(State::Done));
        let record_path = report.record_path.clone().expect("a record");
        let record = load(&record_path, &library).expect("the record");
        let mut gate = TestGate::new();
        let done = undo(&record_path, &record, &library, &mut gate);
        assert_eq!(done.refusal, None);
        assert_eq!(done.removed_notes.len(), 2);
        assert_eq!(done.removed_links, 2);
        assert!(done.group_removed);
        assert_eq!(done.folders_removed.len(), 2, "{:?}", done.folders_removed);
        // Everything the run made is gone; the source is where it was.
        assert!(!library.join("Trip").exists());
        assert!(!library.join("Trip.md").exists());
        assert!(source.is_file());
        // The record is marked and stays.
        let again = load(&record_path, &library).expect("the record stays");
        assert_eq!(again.state, State::Undone);
        assert!(prepare(&again, &library).refusal.is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn undo_keeps_a_note_the_owner_changed_and_a_link_the_owner_edited() {
        let dir = scratch("undo-kept");
        let library = dir.join("library");
        std::fs::create_dir_all(&library).expect("the library");
        let source = source(&dir);
        let req = plan(
            &library,
            &source,
            &[("One", "# One\n", &[]), ("Two", "# Two\n", &[])],
        );
        let mut gate = TestGate::new();
        let report = write_at(&req, &mut gate, 9, "2026-11-21T14:03:11Z");
        let record_path = report.record_path.clone().expect("a record");
        // The owner changes one note and edits the other's line in the library file.
        write_file(&library.join("Trip/One.md"), "# One, changed\n");
        let group = read(&library.join("Trip.md"));
        let edited = group.replace("[Two]", "[Two, renamed]");
        write_file(&library.join("Trip.md"), &edited);
        let record = load(&record_path, &library).expect("the record");
        let plan = prepare(&record, &library);
        assert_eq!(plan.kept.len(), 1, "{:?}", plan.kept);
        assert_eq!(plan.notes_kept.len(), 1, "{:?}", plan.notes_kept);
        assert_eq!(plan.links_kept.len(), 1, "{:?}", plan.links_kept);
        let words = plan.sentence("Trip");
        assert!(
            words.contains("1 note was changed since and is kept."),
            "{words}"
        );
        assert!(
            words.contains("1 line was edited or moved since and is kept, with its note."),
            "{words}"
        );
        let mut gate = TestGate::new();
        let done = undo(&record_path, &record, &library, &mut gate);
        // The line the owner edited is kept, and a note a library line still lists is kept with
        // it: no line is left naming a file that undo removed.
        assert!(done.removed_notes.is_empty(), "{:?}", done.removed_notes);
        assert_eq!(read(&library.join("Trip/Two.md")), "# Two\n");
        assert_eq!(read(&library.join("Trip/One.md")), "# One, changed\n");
        let group = read(&library.join("Trip.md"));
        assert!(
            group.contains("[One]"),
            "the kept note's line stays: {group}"
        );
        assert!(group.contains("[Two, renamed]"), "{group}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `prepare` reads the disk and `undo` reads it again, which looks like the same question
    /// asked twice until the file changes between them: the editor asks, the owner saves over a
    /// note while the question is on screen, and the answer is already stale. The second hash is
    /// the one that decides, so a note written to between the two calls is kept.
    #[test]
    fn a_note_changed_after_undo_was_worked_out_is_still_kept() {
        let dir = scratch("undo-raced");
        let library = dir.join("library");
        std::fs::create_dir_all(&library).expect("the library");
        let source = source(&dir);
        let req = plan(
            &library,
            &source,
            &[("One", "# One\n", &[]), ("Two", "# Two\n", &[])],
        );
        let mut gate = TestGate::new();
        let report = write_at(&req, &mut gate, 9, "2026-11-21T14:03:11Z");
        let record_path = report.record_path.clone().expect("a record");
        let record = load(&record_path, &library).expect("the record");
        // Worked out while both notes are untouched: undo would remove both.
        let plan = prepare(&record, &library);
        assert_eq!(plan.notes.len(), 2, "{:?}", plan.notes);
        assert!(plan.kept.is_empty(), "{:?}", plan.kept);
        // The owner saves over one of them before pressing Undo.
        let one = library.join("Trip").join("One.md");
        write_file(&one, "# One, saved while the question was up\n");
        let mut gate = TestGate::new();
        let done = undo(&record_path, &record, &library, &mut gate);
        assert_eq!(
            done.removed_notes,
            vec![library.join("Trip/Two.md")],
            "only the note that is still ours goes"
        );
        assert_eq!(
            read(&one),
            "# One, saved while the question was up\n",
            "the note written between the two reads is kept, whatever the plan said"
        );
        assert!(
            done.kept.iter().any(|p| same_path(p, &one)),
            "{:?}",
            done.kept
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn undo_leaves_a_group_file_that_holds_more_than_its_heading_and_its_home() {
        let dir = scratch("undo-group");
        let library = dir.join("library");
        std::fs::create_dir_all(&library).expect("the library");
        let source = source(&dir);
        let req = plan(&library, &source, &[("One", "# One\n", &[])]);
        let mut gate = TestGate::new();
        let report = write_at(&req, &mut gate, 10, "2026-11-21T14:03:11Z");
        let record_path = report.record_path.clone().expect("a record");
        // The owner writes something of their own into the group's file.
        let group = format!("{}Mine to keep.\n", read(&library.join("Trip.md")));
        write_file(&library.join("Trip.md"), &group);
        let record = load(&record_path, &library).expect("the record");
        let mut gate = TestGate::new();
        let done = undo(&record_path, &record, &library, &mut gate);
        assert!(done.group_kept && !done.group_removed);
        let group = read(&library.join("Trip.md"));
        assert!(group.contains("Mine to keep."), "{group}");
        assert!(!group.contains("[One]"), "{group}");
        // The folder held no file of the owner's own, so it went.
        assert!(done.folders_removed.contains(&library.join("Trip")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A record is a file in `.runs`, and undo deletes and writes what it names. One that names
    /// a path outside the library folder and the group's folder is not a record this program
    /// wrote (the write refuses those, 4.1 and 6.2), so undo acts on none of it.
    #[test]
    fn undo_removes_nothing_when_the_record_names_a_path_outside_the_library_and_the_groups_folder()
    {
        let dir = scratch("undo-outside");
        let library = dir.join("library");
        std::fs::create_dir_all(&library).expect("the library");
        let source = source(&dir);
        let req = plan(&library, &source, &[("One", "# One\n", &[])]);
        let mut gate = TestGate::new();
        let report = write_at(&req, &mut gate, 13, "2026-11-21T14:03:11Z");
        let record_path = report.record_path.clone().expect("a record");
        let good = load(&record_path, &library).expect("the record");
        // The owner's own files, with the digests a hostile record would carry: what it takes is
        // the path, not the hash.
        let mine = dir.join("My own notes.md");
        write_file(&mine, "words I wrote\n");
        let own_group = dir.join("My own group.md");
        write_file(&own_group, "# Mine\nhome: Mine\n");
        let outside_folder = dir.join("work");
        std::fs::create_dir_all(&outside_folder).expect("the folder");
        let digest = |text: &str| mdshared_app::update::sha256_hex(text.as_bytes());
        // The record a hostile `.runs` file holds: the owner's own note, with its real digest.
        let mut note = good.clone();
        note.notes[0].path = mine.clone();
        note.notes[0].sha256 = digest("words I wrote\n");
        // Undo refuses, and removes neither the owner's file nor the run's own note, which it
        // would otherwise take.
        let mut gate = TestGate::new();
        let done = undo(&record_path, &note, &library, &mut gate);
        assert!(done.refusal.is_some());
        assert!(gate.removed.is_empty(), "{:?}", gate.removed);
        assert_eq!(
            read(&mine),
            "words I wrote\n",
            "the owner's file is untouched"
        );
        assert_eq!(
            read(&library.join("Trip/One.md")),
            "# One\n",
            "and the run's own note is not removed either"
        );
        assert!(read(&library.join("Trip.md")).contains("[One]"));
        assert_eq!(
            load(&record_path, &library).expect("the record").state,
            State::Done
        );
        // The same refusal for a picture, a folder and the group's library file, and nothing is
        // planned for any of them either: the confinement is decided before any removal.
        let mut picture = good.clone();
        picture.pictures.push(PictureRec {
            path: mine.clone(),
            created: true,
            sha256: digest("words I wrote\n"),
        });
        let mut folder = good.clone();
        folder.folders = vec![outside_folder.clone()];
        let mut group = good.clone();
        group.group.file = own_group.clone();
        for record in [&note, &picture, &folder, &group] {
            let plan = prepare(record, &library);
            let why = plan.refusal.clone().expect("refused");
            assert!(
                why.ends_with(
                    "is outside the library and the group's folder, so this run is not undone."
                ),
                "{why}"
            );
            assert!(plan.notes.is_empty(), "{:?}", plan.notes);
            assert!(plan.pictures.is_empty(), "{:?}", plan.pictures);
            assert!(plan.folders.is_empty(), "{:?}", plan.folders);
            assert!(plan.links.is_empty(), "{:?}", plan.links);
            assert!(plan.group_file.is_none(), "{:?}", plan.group_file);
        }
        assert_eq!(read(&own_group), "# Mine\nhome: Mine\n");
        assert!(outside_folder.is_dir());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A library file the owner's editor wrote with CRLF comes back with CRLF: undo's own
    /// removal of a section heading keeps the file's endings, as every writer in
    /// `mdshared_app::library` does.
    #[test]
    fn undo_keeps_the_line_endings_of_a_crlf_library_file() {
        let dir = scratch("undo-crlf");
        let library = dir.join("library");
        std::fs::create_dir_all(&library).expect("the library");
        let source = source(&dir);
        // A group that is there already, so undo keeps its file and takes the heading out of it.
        write_file(&library.join("Trip.md"), "# Trip\nhome: Trip\n");
        let mut req = plan(&library, &source, &[("One", "# One\n", &["Tokyo"])]);
        req.group.created = false;
        let mut gate = TestGate::new();
        let report = write_at(&req, &mut gate, 14, "2026-11-21T14:03:11Z");
        let record_path = report.record_path.clone().expect("a record");
        let lf = read(&library.join("Trip.md"));
        assert!(lf.contains("\n\n## Tokyo\n"), "{lf:?}");
        // The file with the endings the owner's editor gives it.
        write_file(&library.join("Trip.md"), &lf.replace('\n', "\r\n"));
        let record = load(&record_path, &library).expect("the record");
        let mut gate = TestGate::new();
        let done = undo(&record_path, &record, &library, &mut gate);
        assert_eq!(done.removed_links, 1);
        assert_eq!(done.removed_notes.len(), 1);
        let after = String::from_utf8(
            std::fs::read(library.join("Trip.md")).expect("the group's file stays"),
        )
        .expect("utf-8");
        assert!(
            !after.replace("\r\n", "").contains('\n'),
            "every line keeps CRLF: {after:?}"
        );
        assert!(after.starts_with("# Trip\r\nhome: Trip\r\n"), "{after:?}");
        assert!(!after.contains("## Tokyo"), "{after:?}");
        assert!(!after.contains("[One]"), "{after:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The group's library file is open with unsaved changes, so the gate refuses to take the
    /// lines out. The notes must stay, because their lines are still in the file, and the record
    /// must not say "undone": the press can be made again.
    #[test]
    fn an_undo_whose_links_could_not_be_taken_out_keeps_the_notes_and_is_not_undone() {
        let dir = scratch("undo-refused");
        let library = dir.join("library");
        std::fs::create_dir_all(&library).expect("the library");
        let source = source(&dir);
        let req = plan(
            &library,
            &source,
            &[("One", "# One\n", &[]), ("Two", "# Two\n", &[])],
        );
        let mut gate = TestGate::new();
        let report = write_at(&req, &mut gate, 12, "2026-11-21T14:03:11Z");
        let record_path = report.record_path.clone().expect("a record");
        let record = load(&record_path, &library).expect("the record");
        let mut gate = TestGate::new();
        gate.refusals.push(library.join("Trip.md"));
        let done = undo(&record_path, &record, &library, &mut gate);
        assert_eq!(done.removed_links, 0);
        assert!(done.removed_notes.is_empty(), "{:?}", done.removed_notes);
        assert_eq!(done.held_back.len(), 2, "{:?}", done.held_back);
        assert_eq!(done.failures.len(), 1, "{:?}", done.failures);
        // Exactly what is left: both notes, both lines, and a record that can be tried again.
        assert_eq!(read(&library.join("Trip/One.md")), "# One\n");
        assert_eq!(read(&library.join("Trip/Two.md")), "# Two\n");
        let group = read(&library.join("Trip.md"));
        assert!(group.contains("[One]"), "{group}");
        assert!(group.contains("[Two]"), "{group}");
        let again = load(&record_path, &library).expect("the record stays");
        assert_eq!(again.state, State::LinksPending);
        assert!(again.state.live(), "Undo is offered again");
        // With the file saved or closed, the same press finishes the job.
        let mut gate = TestGate::new();
        let done = undo(&record_path, &again, &library, &mut gate);
        assert!(done.failures.is_empty(), "{:?}", done.failures);
        assert_eq!(done.removed_links, 2);
        assert_eq!(done.removed_notes.len(), 2);
        assert!(done.marked);
        assert!(!library.join("Trip/One.md").exists());
        assert!(!library.join("Trip/Two.md").exists());
        assert_eq!(
            load(&record_path, &library)
                .expect("the record stays")
                .state,
            State::Undone
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_record_reads_back_with_its_paths_and_survives_a_hostile_file() {
        let dir = scratch("json");
        let library = dir.join("library");
        std::fs::create_dir_all(&library).expect("the library");
        let record = Record {
            version: 1,
            kind: Kind::Split,
            started: "2026-11-21T14:03:11Z".to_string(),
            started_secs: secs_of("2026-11-21T14:03:11Z").expect("the stamp"),
            state: State::Done,
            instruction: "one note per chapter".to_string(),
            sent: None,
            source: Source {
                path: dir.join("A \"quoted\" \\ file.md"),
                len: 12,
                sha256: "abc".to_string(),
                recycled: false,
            },
            group: Group {
                name: "Group".to_string(),
                file: library.join("Group.md"),
                created: true,
                home: library.join("Group"),
            },
            folders: vec![library.join("Group").join("Tokyo")],
            notes: vec![NoteRec {
                path: library.join("Group").join("Tokyo").join("One.md"),
                created: true,
                sha256: "def".to_string(),
                ranges: vec![(1, 5), (7, 9)],
                heading_shift: 1,
                title_added: false,
                sections: vec!["Tokyo".to_string()],
                line: "- [One](Group/Tokyo/One.md)".to_string(),
            }],
            pictures: Vec::new(),
            sections_added: vec![vec!["Tokyo".to_string()]],
        };
        let text = record.to_json(&library);
        let back = Record::from_json(&text, &library).expect("it reads");
        // The source's name holds a quote and a backslash. On Windows a backslash separates
        // the parts of a path; everywhere else it is a character of the name and must survive
        // the round trip as one.
        assert_eq!(back, record);
        assert!(text.contains("\"path\": \"Group/Tokyo/One.md\""), "{text}");
        // Nothing that is not a record, and no depth that can end the process.
        assert!(Record::from_json("", &library).is_none());
        assert!(Record::from_json("{}", &library).is_none());
        assert!(Record::from_json("[1,2,3]", &library).is_none());
        let deep = "[".repeat(10_000) + &"]".repeat(10_000);
        assert!(Record::from_json(&deep, &library).is_none());
        assert!(Record::from_json("{\"version\":1,\"kind\":\"split\"", &library).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_path_written_into_a_record_reads_back_as_the_same_path() {
        // What a record writes it must read back, on the platform that wrote it, for a name
        // holding whatever that platform allows in one. Both are tried here whatever this
        // machine is, because the fault this pins only showed on Linux and a Mac.
        for windows in [true, false] {
            let library = PathBuf::from(if windows { "C:\\library" } else { "/library" });
            let sep = if windows { "\\" } else { "/" };
            let group = PathBuf::from(format!("{}{sep}Group", library.display()));
            // On Windows `\` separates the parts, so a name cannot hold one; everywhere else
            // it is an ordinary character, and writing it as `/` names a different file.
            let names: &[&str] = if windows {
                &["One.md", "A 'quoted' file.md"]
            } else {
                &["One.md", "A \\ file.md"]
            };
            for name in names {
                let path = PathBuf::from(format!("{}{sep}{name}", group.display()));
                let text = stored_on(&path, &library, windows);
                assert_eq!(resolved_on(&text, &library, windows), path, "from {text}");
            }
            // The parts are written with `/` between them whatever the platform separates
            // with, so a library carried to another machine reads the same there.
            let one = PathBuf::from(format!("{}{sep}One.md", group.display()));
            assert_eq!(stored_on(&one, &library, windows), "Group/One.md");
        }
        // The text itself, because a `Path` on Windows reads `/` and `\` alike and would hide
        // this: off Windows a backslash in a name is written as it stands.
        assert_eq!(
            stored_on(
                Path::new("/library/Group/A \\ file.md"),
                Path::new("/library"),
                false
            ),
            "Group/A \\ file.md"
        );
    }

    #[test]
    fn the_clock_writes_iso_and_a_name_windows_takes() {
        let (_, stamp_text) = now_utc();
        assert_eq!(stamp_text.len(), 20, "{stamp_text}");
        assert!(stamp_text.ends_with('Z'));
        assert_eq!(utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(utc(1_800_000_000), "2027-01-15T08:00:00Z");
        assert_eq!(secs_of("2027-01-15T08:00:00Z"), Some(1_800_000_000));
        assert_eq!(secs_of("not a stamp"), None);
        let name = stamp(1_800_000_000, Kind::Split);
        assert_eq!(name, "2027-01-15T08-00-00Z-split");
        assert!(!name.contains(':'), "{name}");
    }

    #[test]
    fn a_newer_record_sorts_first_and_the_old_ones_go_after_a_run() {
        let dir = scratch("list");
        let library = dir.join("library");
        std::fs::create_dir_all(&library).expect("the library");
        let source = source(&dir);
        let req = plan(&library, &source, &[("One", "# One\n", &[])]);
        let mut gate = TestGate::new();
        let a = write_at(&req, &mut gate, 1, "2026-11-21T14:00:01Z");
        let b = write_at(&req, &mut gate, 2, "2026-11-21T14:00:02Z");
        let runs = list(&library);
        assert_eq!(runs.len(), 2);
        assert_eq!(
            runs[0].path,
            b.record_path.clone().expect("the second record")
        );
        assert_eq!(runs[1].path, a.record_path.expect("the first record"));
        // Pruning keeps the newest twenty and everything of the last ninety days.
        let base = runs[0].record.clone();
        let mut paths = Vec::new();
        for secs in 1..=21u64 {
            let old = Record {
                started: utc(secs),
                started_secs: secs,
                ..base.clone()
            };
            paths.push(old.create(&library).expect("an old record"));
        }
        let now = 200 * 86_400;
        prune(&library, now);
        for path in &paths[..3] {
            assert!(!path.exists(), "the three oldest went");
        }
        for path in &paths[3..] {
            assert!(path.exists(), "the newest twenty stayed");
        }
        assert!(runs[0].path.exists(), "the new ones stayed");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_link_is_added_under_its_section_and_read_back_exactly() {
        let base = if cfg!(windows) {
            Path::new(r"C:\lib")
        } else {
            Path::new("/lib")
        };
        let text = "# Group\n\nhome: Group\n";
        let link = Link {
            path: PathBuf::from("Group/Tokyo/One.md"),
            title: "One".to_string(),
            sections: vec!["Tokyo".to_string()],
            line: "- [One](Group/Tokyo/One.md)".to_string(),
        };
        let new = with_links_added(text, std::slice::from_ref(&link));
        assert!(new.contains("## Tokyo"), "{new}");
        assert!(new.contains(&link.line), "{new}");
        let old_p = library::parse(text, base);
        let new_p = library::parse(&new, base);
        assert!(check_links_added(
            &old_p,
            &new_p,
            std::slice::from_ref(&link),
            base,
        ));
        // A line that was edited is not removed.
        let edited = new.replace("[One]", "[One, edited]");
        let removal = LinkRemoval {
            path: link.path.clone(),
            sections: link.sections.clone(),
            line: link.line.clone(),
        };
        assert_eq!(
            with_links_removed(&edited, std::slice::from_ref(&removal), base),
            None
        );
        let removed =
            with_links_removed(&new, std::slice::from_ref(&removal), base).expect("removed");
        assert!(!removed.contains("[One]"), "{removed}");
    }

    #[test]
    fn nothing_is_written_outside_the_library_or_the_groups_folder() {
        let dir = scratch("outside");
        let library = dir.join("library");
        std::fs::create_dir_all(&library).expect("the library");
        let source = source(&dir);
        let mut req = plan(&library, &source, &[("One", "# One\n", &[])]);
        req.notes[0].path = dir.join("Somewhere else.md");
        let mut gate = TestGate::new();
        let report = write_at(&req, &mut gate, 11, "2026-11-21T14:03:11Z");
        let why = report.start_error.expect("refused");
        assert!(why.contains("outside the library"), "{why}");
        assert!(!dir.join("Somewhere else.md").exists());
        assert!(!library.join(RUNS_DIR).exists(), "not even a record");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A path with `..` in it is not taken as it stands, even where folding it would land inside
    /// the library: a run never writes one, so a record that holds one is not a record of a run.
    #[test]
    fn a_record_path_with_dots_in_it_is_refused_even_when_it_folds_to_a_path_inside() {
        let dir = scratch("dots-inside");
        let library = dir.join("library");
        write_file(&library.join("Trip/One.md"), "# One\n");
        let reach = Reach {
            library: library.clone(),
            home: None,
        };
        assert!(reach.allows(&library.join("Trip/One.md")));
        assert!(
            !reach.allows(&library.join("Trip/Other/../One.md")),
            "it names the note, and still it is refused"
        );
        assert!(!reach.allows(&library.join("..").join("library").join("Trip/One.md")));
        assert!(
            !reach.allows(&library),
            "the library folder itself is not a path of a run"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// File names compare as the disk compares them: `C:\Notes` and `c:\notes` are one folder on
    /// Windows and macOS, so a record that spells the library in another case is not refused for
    /// it, and `Trip` is not `Trips`.
    #[test]
    fn a_path_is_under_a_folder_part_by_part_in_the_case_the_disk_uses() {
        assert!(under(Path::new("/lib/Trip/One.md"), Path::new("/lib")));
        assert!(under(Path::new("/lib"), Path::new("/lib")));
        assert!(!under(Path::new("/library/One.md"), Path::new("/lib")));
        assert!(!under(Path::new("/lib"), Path::new("/lib/Trip")));
        if mdshared_app::paths::NAMES_IGNORE_CASE {
            assert!(under(Path::new("/LIB/trip/One.md"), Path::new("/lib/Trip")));
        } else {
            assert!(!under(
                Path::new("/LIB/trip/One.md"),
                Path::new("/lib/Trip")
            ));
        }
    }
}
