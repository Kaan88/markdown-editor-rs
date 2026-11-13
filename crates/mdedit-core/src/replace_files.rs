//! Replace across files: a plan, then an apply, then (if wanted) a revert. All three are headless
//! and run on a worker thread; each takes a `cancel` flag, looked at between files, and a
//! progress callback `(done, total)` in files.
//!
//! **Plan** ([`plan`]) walks a folder (or takes a list of files), reads each file once with the
//! editor's loader (so it is decoded as it would be opened), counts the matches of the query in
//! it and keeps a few lines of before and after for a preview. Folders and files are chosen as
//! find in files chooses them (`search::walk_files`: the same order, the same skipped names) and
//! a file is skipped for the same reasons (too large, binary, unreadable) and for one more: text
//! that would not be written back unchanged. Nothing is written. A plan remembers, per file, the
//! length and hash of the bytes it saw.
//!
//! **Apply** ([`apply`]) takes the plan and the entries the user left ticked. For each file it
//! reads the bytes again and refuses (skips, with a reason) a file that no longer has the length
//! and hash the plan recorded; it decodes with the encoding the plan recorded, replaces with
//! `replace::replace_all` (the same expansion of `$1`, `${name}` and the same handling of line
//! breaks as the single-file replace), and writes through `save::save_path`, so it is a temporary
//! file beside the target and a swap, never a truncated file, and the same encoding, byte order
//! mark and line breaks come back. A file the encoding cannot hold the result of is not written
//! (a failure). Files open in the editor with unsaved changes must be listed in
//! `ApplyOptions::skip` (and `PlanOptions` takes the same list), because the file on disk is not
//! what the user sees: they are never touched.
//!
//! **Revert** ([`revert`]) restores the files an apply changed, from the original bytes the
//! report keeps in memory (up to [`REPLACE_REVERT_BYTES`] in all) or from the backup files, and
//! only those that still hold exactly the bytes apply wrote.

use crate::buffer::Buffer;
use crate::encoding::Encoding;
use crate::eol::EolCounts;
use crate::find::{Query, find_all};
use crate::hash::hash128;
use crate::limits::{
    REPLACE_BACKUPS, REPLACE_FILE_BYTES, REPLACE_MATCHES_PER_FILE, REPLACE_PLAN_BYTES,
    REPLACE_PLAN_FILES, REPLACE_PREVIEW_LINES, REPLACE_PREVIEW_PART, REPLACE_REVERT_BYTES,
    REPLACE_SKIPPED_KEPT, REPLACE_WALK_FILES,
};
use crate::load::{LoadError, LoadOptions, load_reader};
use crate::replace::{expand, replace_all};
use crate::save::{SaveError, save_bytes, save_path};
use crate::search::walk_files;
use crate::{History, Selection};
use mdshared_app::files::{FileStamp, stamp};
use std::collections::HashSet;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// Where the files come from.
#[derive(Clone, Copy, Debug)]
pub enum Source<'a> {
    /// A folder, walked recursively like find in files, or a single file.
    Tree(&'a Path),
    /// These files, in this order (each once).
    Files(&'a [PathBuf]),
}

#[derive(Clone, Copy, Debug)]
pub struct PlanOptions {
    /// How files are decoded: the system's code page for files that are neither UTF-8 nor
    /// marked. `max_bytes` and `forced` are not used (see `max_file_bytes`).
    pub load: LoadOptions,
    /// Larger files are skipped (`REPLACE_FILE_BYTES`, the cap of find in files).
    pub max_file_bytes: u64,
    /// The plan stops (`Plan::truncated`) when it holds this many files, or the files in it
    /// this many bytes.
    pub max_files: usize,
    pub max_bytes: u64,
    /// Names starting with a dot are skipped (a tree only).
    pub skip_hidden: bool,
}

impl Default for PlanOptions {
    fn default() -> PlanOptions {
        PlanOptions {
            load: LoadOptions::default(),
            max_file_bytes: REPLACE_FILE_BYTES,
            max_files: REPLACE_PLAN_FILES,
            max_bytes: REPLACE_PLAN_BYTES,
            skip_hidden: true,
        }
    }
}

/// One match shown as it is and as it would become: the text around it on its line, cut to a
/// few dozen characters each side and to `REPLACE_PREVIEW_PART` bytes for the match and for its
/// replacement. Line breaks inside a match show as `↵`, other control characters as spaces.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preview {
    /// The line the match starts on, 0-based.
    pub line: usize,
    pub before: String,
    pub after: String,
}

/// A file the replacement would change.
#[derive(Clone, Debug)]
pub struct FilePlan {
    pub path: PathBuf,
    pub encoding: Encoding,
    pub eol: EolCounts,
    /// Matches in the file; at most `REPLACE_MATCHES_PER_FILE` (see `capped`).
    pub matches: usize,
    /// There are more matches than were counted (or the search gave up); applying still
    /// replaces them all, or refuses the file when it gives up again.
    pub capped: bool,
    /// The first `REPLACE_PREVIEW_LINES` matches.
    pub preview: Vec<Preview>,
    /// The length and hash of the bytes the plan was made from.
    pub bytes: u64,
    pub hash: u128,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Totals {
    /// Files in the plan, matches in them (a lower bound where a file is `capped`), their bytes.
    pub files: usize,
    pub matches: usize,
    pub bytes: u64,
    /// Files read and searched, those without a match included.
    pub searched: usize,
}

#[derive(Clone, Debug)]
pub struct Plan {
    query: Query,
    replacement: String,
    load: LoadOptions,
    /// The files with at least one match, in walk order.
    pub files: Vec<FilePlan>,
    pub totals: Totals,
    /// Files left out and why, at most `REPLACE_SKIPPED_KEPT` of them; `skipped_count` is how
    /// many there were.
    pub skipped: Vec<(PathBuf, String)>,
    pub skipped_count: usize,
    /// The plan stopped at a limit (files in the plan, their bytes, or files walked).
    pub truncated: bool,
    pub cancelled: bool,
}

impl Plan {
    pub fn query(&self) -> &Query {
        &self.query
    }

    pub fn replacement(&self) -> &str {
        &self.replacement
    }

    /// The indexes of every file in the plan, for "apply to all".
    pub fn all(&self) -> Vec<usize> {
        (0..self.files.len()).collect()
    }
}

fn kib(bytes: u64) -> String {
    if bytes >= 1 << 20 {
        format!("{} MiB", bytes >> 20)
    } else {
        format!("{} KiB", bytes.div_ceil(1024))
    }
}

/// The bytes of a file and the stamp seen before reading; `Err` is the reason it could not be
/// read as one consistent piece. At most `max` bytes are read; a longer file is an error.
fn read_file(path: &Path, max: u64) -> Result<(Vec<u8>, FileStamp), String> {
    let before = stamp(path);
    let file = std::fs::File::open(path).map_err(|e| format!("cannot be read: {e}"))?;
    let size = file.metadata().map_or(0, |m| m.len());
    if size > max {
        return Err(format!("larger than {}", kib(max)));
    }
    let mut bytes = Vec::with_capacity(size as usize);
    file.take(max.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|e| format!("cannot be read: {e}"))?;
    if bytes.len() as u64 > max {
        return Err(format!("larger than {}", kib(max)));
    }
    if stamp(path) != before {
        return Err("changed while it was read".to_string());
    }
    Ok((bytes, before))
}

/// Whether `path` is one of `skip` (compared as given, and by canonical path when that works).
struct SkipSet {
    exact: HashSet<PathBuf>,
    canonical: HashSet<PathBuf>,
}

impl SkipSet {
    fn new(paths: &[PathBuf]) -> SkipSet {
        SkipSet {
            exact: paths.iter().cloned().collect(),
            canonical: paths
                .iter()
                .filter_map(|p| std::fs::canonicalize(p).ok())
                .collect(),
        }
    }

    fn has(&self, path: &Path) -> bool {
        if self.exact.is_empty() && self.canonical.is_empty() {
            return false;
        }
        self.exact.contains(path)
            || std::fs::canonicalize(path).is_ok_and(|c| self.canonical.contains(&c))
    }
}

/// Text for a preview: control characters as spaces (line breaks as `↵`), cut to `max` bytes at
/// a char boundary with `…`.
fn clean(text: &str, max: usize) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if out.len() + c.len_utf8() > max {
            out.push('…');
            break;
        }
        match c {
            '\n' | '\r' => {
                if out.ends_with('↵') && c == '\n' && text.contains("\r\n") {
                    continue;
                }
                out.push('↵');
            }
            c if c.is_control() => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

fn text_of(buf: &Buffer, range: std::ops::Range<usize>) -> String {
    let mut s = String::new();
    buf.text(range, &mut s);
    s
}

/// Context kept each side of a match in a preview, in bytes.
const PREVIEW_LEAD: usize = 48;

fn preview_of(
    buf: &Buffer,
    query: &Query,
    replacement: &str,
    range: std::ops::Range<usize>,
) -> Preview {
    let line = buf.line_of(range.start);
    let start_line = buf.line_range(line);
    let end_line = buf.line_range(buf.line_of(range.end));
    let mut from = range
        .start
        .saturating_sub(PREVIEW_LEAD)
        .max(start_line.start);
    while !buf.is_char_boundary(from) {
        from += 1;
    }
    let to = range
        .end
        .saturating_add(PREVIEW_LEAD)
        .min(end_line.end.max(range.end));
    let to = buf.floor_char(to);
    let prefix = clean(&text_of(buf, from..range.start), usize::MAX);
    let matched = text_of(buf, range.clone());
    let suffix = clean(&text_of(buf, range.end..to.max(range.end)), usize::MAX);
    let new = expand(query, &matched, replacement);
    let part = REPLACE_PREVIEW_PART;
    Preview {
        line,
        before: format!("{prefix}{}{suffix}", clean(&matched, part)),
        after: format!("{prefix}{}{suffix}", clean(&new, part)),
    }
}

enum Planned {
    File(Box<FilePlan>),
    NoMatch,
    Skip(String),
    Cancelled,
}

fn plan_file(
    path: &Path,
    query: &Query,
    replacement: &str,
    opts: &PlanOptions,
    cancel: &AtomicBool,
) -> Planned {
    let (bytes, _) = match read_file(path, opts.max_file_bytes) {
        Ok(r) => r,
        Err(why) => return Planned::Skip(why),
    };
    let load = LoadOptions {
        max_bytes: u64::MAX,
        forced: None,
        ..opts.load
    };
    let mut reader = &bytes[..];
    let loaded = match load_reader(
        &mut reader,
        Some(bytes.len() as u64),
        &load,
        &|_, _| {},
        cancel,
    ) {
        Ok(l) => l,
        Err(LoadError::Cancelled) => return Planned::Cancelled,
        Err(e) => return Planned::Skip(e.to_string()),
    };
    if loaded.info.has_nul {
        return Planned::Skip("binary file (contains NUL characters)".to_string());
    }
    let found = find_all(
        loaded.buffer.rope(),
        query,
        REPLACE_MATCHES_PER_FILE,
        cancel,
    );
    if found.cancelled {
        return Planned::Cancelled;
    }
    if found.ranges.is_empty() {
        return Planned::NoMatch;
    }
    if loaded.info.lossy {
        return Planned::Skip(
            "some bytes could not be read, so the file could not be written back unchanged"
                .to_string(),
        );
    }
    let preview = found
        .ranges
        .iter()
        .take(REPLACE_PREVIEW_LINES)
        .map(|r| preview_of(&loaded.buffer, query, replacement, r.clone()))
        .collect();
    Planned::File(Box::new(FilePlan {
        path: path.to_path_buf(),
        encoding: loaded.info.encoding,
        eol: loaded.info.eol,
        matches: found.ranges.len(),
        capped: found.capped,
        preview,
        bytes: bytes.len() as u64,
        hash: hash128(&bytes),
    }))
}

/// Finds what replacing `query` by `replacement` (a template: `$1`, `${name}` and `$$` when the
/// query is a regular expression) would change. `skip` lists files that must not be planned:
/// the paths of documents open in the editor with unsaved changes (they are reported as
/// skipped). `progress(done, total)` is called after each file. Reads every file once; writes
/// nothing. Stops early with `cancelled` when `cancel` is set.
pub fn plan(
    source: Source<'_>,
    query: &Query,
    replacement: &str,
    opts: &PlanOptions,
    skip: &[PathBuf],
    cancel: &AtomicBool,
    progress: &dyn Fn(usize, usize),
) -> Plan {
    let mut out = Plan {
        query: query.clone(),
        replacement: replacement.to_string(),
        load: opts.load,
        files: Vec::new(),
        totals: Totals::default(),
        skipped: Vec::new(),
        skipped_count: 0,
        truncated: false,
        cancelled: false,
    };
    let mut paths: Vec<PathBuf> = Vec::new();
    match source {
        Source::Tree(root) => {
            let finished = walk_files(root, opts.skip_hidden, cancel, &mut |p| {
                if paths.len() >= REPLACE_WALK_FILES {
                    out.truncated = true;
                    return false;
                }
                paths.push(p.to_path_buf());
                true
            });
            out.cancelled = !finished && !out.truncated;
        }
        Source::Files(list) => {
            let mut seen = HashSet::new();
            for p in list.iter().take(REPLACE_WALK_FILES) {
                if seen.insert(p) {
                    paths.push(p.clone());
                }
            }
            out.truncated = list.len() > REPLACE_WALK_FILES;
            out.cancelled = cancel.load(Ordering::Relaxed);
        }
    }
    let skip = SkipSet::new(skip);
    let total = paths.len();
    let skipped = |out: &mut Plan, path: &Path, why: String| {
        out.skipped_count += 1;
        if out.skipped.len() < REPLACE_SKIPPED_KEPT {
            out.skipped.push((path.to_path_buf(), why));
        }
    };
    for (done, path) in paths.iter().enumerate() {
        if out.cancelled || cancel.load(Ordering::Relaxed) {
            out.cancelled = true;
            break;
        }
        if out.files.len() >= opts.max_files || out.totals.bytes >= opts.max_bytes {
            out.truncated = true;
            break;
        }
        if skip.has(path) {
            skipped(
                &mut out,
                path,
                "open in the editor with unsaved changes".into(),
            );
        } else {
            match plan_file(path, query, replacement, opts, cancel) {
                Planned::File(f) => {
                    out.totals.searched += 1;
                    out.totals.files += 1;
                    out.totals.matches += f.matches;
                    out.totals.bytes += f.bytes;
                    out.files.push(*f);
                }
                Planned::NoMatch => out.totals.searched += 1,
                Planned::Skip(why) => skipped(&mut out, path, why),
                Planned::Cancelled => {
                    out.cancelled = true;
                    break;
                }
            }
        }
        progress(done + 1, total);
    }
    out
}

// ---- apply

#[derive(Clone, Copy, Debug)]
pub struct ApplyOptions<'a> {
    /// Before a file is changed, its original bytes are written to `<name>.bak` beside it, or
    /// `<name>.bak1`, `.bak2`, .. when that name is taken (an existing file is never
    /// overwritten); with `REPLACE_BACKUPS` names taken the file is not changed.
    pub backup: bool,
    /// Paths that must not be changed: documents open in the editor with unsaved changes (the
    /// file on disk is not what the user sees there). Checked again here, so a document edited
    /// after the plan was made is left alone.
    pub skip: &'a [PathBuf],
    /// The original bytes of changed files are kept in memory for [`revert`] up to this many in
    /// all (`REPLACE_REVERT_BYTES`); past it a file is revertible only from its backup.
    pub keep_bytes: u64,
}

impl Default for ApplyOptions<'_> {
    fn default() -> ApplyOptions<'static> {
        ApplyOptions {
            backup: false,
            skip: &[],
            keep_bytes: REPLACE_REVERT_BYTES,
        }
    }
}

/// A file apply changed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Changed {
    pub path: PathBuf,
    pub replacements: usize,
    pub old_bytes: u64,
    pub new_bytes: u64,
    /// The backup written, if one was asked for.
    pub backup: Option<PathBuf>,
}

/// What it takes to put one changed file back: the hash and length of the bytes before and of
/// the bytes apply wrote, and the original bytes (in memory, or at `backup`).
#[derive(Clone, Debug)]
pub struct Revert {
    pub path: PathBuf,
    pub original_hash: u128,
    pub original_len: u64,
    pub written_hash: u128,
    pub written_len: u64,
    pub original: Option<Vec<u8>>,
    pub backup: Option<PathBuf>,
}

#[derive(Clone, Debug, Default)]
pub struct Report {
    pub changed: Vec<Changed>,
    /// Replacements made in all.
    pub replacements: usize,
    /// Files left alone, and why: changed since the plan, open with unsaved changes, nothing to
    /// change any more.
    pub skipped: Vec<(PathBuf, String)>,
    /// Files that could not be changed, and why: read-only, a character the encoding cannot
    /// hold, an I/O error. Their content is as it was (and the backup of such a file is
    /// removed).
    pub failures: Vec<(PathBuf, String)>,
    /// Stopped by `cancel`: the files in `changed` were written whole, the rest not touched.
    pub cancelled: bool,
    /// For [`revert`], one for each file in `changed`.
    pub reverts: Vec<Revert>,
}

impl Report {
    /// Files [`revert`] can restore (their original bytes were kept, or a backup was made).
    pub fn revertible(&self) -> usize {
        self.reverts
            .iter()
            .filter(|r| r.original.is_some() || r.backup.is_some())
            .count()
    }
}

enum Fail {
    Skip(String),
    Fail(String),
}

fn save_error(e: SaveError) -> Fail {
    match e {
        SaveError::ChangedOnDisk => Fail::Skip("changed since the search".to_string()),
        e => Fail::Fail(e.to_string()),
    }
}

/// Writes `bytes` to a new `<name>.bak`, `.bak1`, .. beside `path`; never overwrites.
fn make_backup(path: &Path, bytes: &[u8]) -> Result<PathBuf, String> {
    let name = path.file_name().unwrap_or_default().to_os_string();
    for n in 0..=REPLACE_BACKUPS {
        let mut file_name = name.clone();
        if n == 0 {
            file_name.push(".bak");
        } else {
            file_name.push(format!(".bak{n}"));
        }
        let candidate = path.with_file_name(file_name);
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate);
        match file {
            Ok(mut f) => {
                let done = f.write_all(bytes).and_then(|()| f.sync_all());
                if let Err(e) = done {
                    drop(f);
                    let _ = std::fs::remove_file(&candidate);
                    return Err(format!("the backup could not be written: {e}"));
                }
                return Ok(candidate);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("the backup could not be created: {e}")),
        }
    }
    Err("all backup names are taken".to_string())
}

fn apply_file(
    plan: &Plan,
    fp: &FilePlan,
    opts: &ApplyOptions<'_>,
    kept: &mut u64,
) -> Result<(Changed, Revert), Fail> {
    let path = &fp.path;
    if std::fs::metadata(path).is_ok_and(|m| m.len() != fp.bytes) {
        return Err(Fail::Skip("changed since the search".to_string()));
    }
    let (bytes, before) = read_file(path, fp.bytes).map_err(|why| {
        if why.starts_with("larger") || why.starts_with("changed") {
            Fail::Skip("changed since the search".to_string())
        } else {
            Fail::Fail(why)
        }
    })?;
    if bytes.len() as u64 != fp.bytes || hash128(&bytes) != fp.hash {
        return Err(Fail::Skip("changed since the search".to_string()));
    }
    let load = LoadOptions {
        max_bytes: u64::MAX,
        forced: Some(fp.encoding),
        ..plan.load
    };
    let never = AtomicBool::new(false);
    let mut reader = &bytes[..];
    let loaded = load_reader(&mut reader, Some(fp.bytes), &load, &|_, _| {}, &never)
        .map_err(|e| Fail::Fail(e.to_string()))?;
    if loaded.info.lossy || loaded.info.has_nul || loaded.info.encoding != fp.encoding {
        return Err(Fail::Fail(
            "the file could not be read back the way it was planned".to_string(),
        ));
    }
    let mut buf = loaded.buffer;
    let mut hist = History::new();
    let (outcome, count) = replace_all(
        &mut buf,
        &mut hist,
        Selection::at(0),
        &plan.query,
        &plan.replacement,
        loaded.info.eol.dominant(),
        None,
        &never,
    );
    if count == 0 {
        return Err(Fail::Skip(
            "nothing to replace any more (or too much work to replace safely)".to_string(),
        ));
    }
    if !fp.capped && count != fp.matches {
        return Err(Fail::Fail(format!(
            "{count} matches now, {} in the plan",
            fp.matches
        )));
    }
    if outcome.change.is_none() {
        return Err(Fail::Skip("the replacement equals the text".to_string()));
    }
    // Nothing is written until the new text is known to fit the encoding.
    if let Some((ch, offset)) = crate::save::first_unencodable(buf.rope(), fp.encoding) {
        let line = buf.line_of(offset);
        return Err(Fail::Fail(format!(
            "the character '{ch}' (U+{:04X}) near line {} cannot be stored in the file's {} encoding",
            ch as u32,
            line + 1,
            fp.encoding.label()
        )));
    }
    if std::fs::metadata(path).is_ok_and(|m| m.permissions().readonly()) {
        return Err(Fail::Fail(SaveError::ReadOnly.to_string()));
    }
    let backup = if opts.backup {
        Some(make_backup(path, &bytes).map_err(Fail::Fail)?)
    } else {
        None
    };
    let saved = match save_path(path, buf.rope(), fp.encoding, Some(&before), &never) {
        Ok(s) => s,
        Err(e) => {
            if let Some(b) = &backup {
                let _ = std::fs::remove_file(b);
            }
            return Err(save_error(e));
        }
    };
    let written_hash = std::fs::read(path).map_or(0, |b| hash128(&b));
    let keep = *kept + fp.bytes <= opts.keep_bytes;
    if keep {
        *kept += fp.bytes;
    }
    let changed = Changed {
        path: path.clone(),
        replacements: count,
        old_bytes: fp.bytes,
        new_bytes: saved.bytes,
        backup: backup.clone(),
    };
    let revert = Revert {
        path: path.clone(),
        original_hash: fp.hash,
        original_len: fp.bytes,
        written_hash,
        written_len: saved.bytes,
        original: keep.then_some(bytes),
        backup,
    };
    Ok((changed, revert))
}

/// Applies the entries `chosen` (indexes into `plan.files`; each at most once, in plan order) of
/// a plan. See the module doc for what each file goes through. `cancel` is looked at before each
/// file; a file being written is finished. `progress(done, total)` follows each file.
pub fn apply(
    plan: &Plan,
    chosen: &[usize],
    opts: &ApplyOptions<'_>,
    cancel: &AtomicBool,
    progress: &dyn Fn(usize, usize),
) -> Report {
    let mut report = Report::default();
    let mut picked: Vec<usize> = chosen
        .iter()
        .copied()
        .filter(|&i| i < plan.files.len())
        .collect();
    picked.sort_unstable();
    picked.dedup();
    let skip = SkipSet::new(opts.skip);
    let total = picked.len();
    let mut kept = 0u64;
    for (done, &i) in picked.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            report.cancelled = true;
            break;
        }
        let fp = &plan.files[i];
        let result = if skip.has(&fp.path) {
            Err(Fail::Skip(
                "open in the editor with unsaved changes".to_string(),
            ))
        } else {
            apply_file(plan, fp, opts, &mut kept)
        };
        match result {
            Ok((changed, revert)) => {
                report.replacements += changed.replacements;
                report.changed.push(changed);
                report.reverts.push(revert);
            }
            Err(Fail::Skip(why)) => report.skipped.push((fp.path.clone(), why)),
            Err(Fail::Fail(why)) => report.failures.push((fp.path.clone(), why)),
        }
        progress(done + 1, total);
    }
    report
}

// ---- revert

#[derive(Clone, Debug, Default)]
pub struct RevertReport {
    pub restored: Vec<PathBuf>,
    /// Left as they are: changed since apply, or the original is gone.
    pub skipped: Vec<(PathBuf, String)>,
    pub failures: Vec<(PathBuf, String)>,
    pub cancelled: bool,
}

fn revert_file(r: &Revert) -> Result<(), Fail> {
    let (current, before) = read_file(&r.path, r.written_len)
        .map_err(|_| Fail::Skip("changed since the replacement".to_string()))?;
    if current.len() as u64 != r.written_len || hash128(&current) != r.written_hash {
        return Err(Fail::Skip("changed since the replacement".to_string()));
    }
    let original = match (&r.original, &r.backup) {
        (Some(bytes), _) => std::borrow::Cow::Borrowed(&bytes[..]),
        (None, Some(backup)) => match std::fs::read(backup) {
            Ok(b) if b.len() as u64 == r.original_len && hash128(&b) == r.original_hash => {
                std::borrow::Cow::Owned(b)
            }
            _ => {
                return Err(Fail::Skip(
                    "the backup is missing or was changed".to_string(),
                ));
            }
        },
        (None, None) => {
            return Err(Fail::Skip(
                "the original was not kept (too large, no backup)".to_string(),
            ));
        }
    };
    save_bytes(&r.path, &original, Some(&before)).map_err(save_error)?;
    Ok(())
}

/// Puts back the files `report` changed, last changed first: a file is restored only when it
/// still holds exactly the bytes apply wrote (same length and hash); the original comes from
/// memory, else from its backup (checked against the recorded hash). Backups are left in place.
pub fn revert(
    report: &Report,
    cancel: &AtomicBool,
    progress: &dyn Fn(usize, usize),
) -> RevertReport {
    let mut out = RevertReport::default();
    let total = report.reverts.len();
    for (done, r) in report.reverts.iter().rev().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            out.cancelled = true;
            break;
        }
        match revert_file(r) {
            Ok(()) => out.restored.push(r.path.clone()),
            Err(Fail::Skip(why)) => out.skipped.push((r.path.clone(), why)),
            Err(Fail::Fail(why)) => out.failures.push((r.path.clone(), why)),
        }
        progress(done + 1, total);
    }
    out
}

#[cfg(test)]
mod tests;
