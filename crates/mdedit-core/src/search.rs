//! Search results by line: the matches of a [`Query`] in one buffer, and in every file of a folder
//! tree. Matching is `find::find_all`; this module adds the line of each match and the short text
//! the results list shows for it, and walks folders. The shell runs `in_tree` on a worker thread
//! and gets its events through the `sink` it passes.
//!
//! The snippet of a hit is the line the match starts on, without its line break and without its
//! leading spaces and tabs. A line of more than [`SNIPPET_MAX`] bytes is cut to a window that
//! starts at most 40 bytes before the match (at a char boundary, moving forward) and holds at
//! most `SNIPPET_MAX` bytes (ending at a char boundary or at the line's end). Only the window
//! is read from the rope, so a line of 100 MB costs no more than a short one. Tabs and other
//! control characters become spaces, as many as they have bytes, so that byte offsets into the
//! snippet stay as exact as the ones in the text.

use crate::buffer::Buffer;
use crate::find::{Query, find_all};
use crate::load::{LoadError, LoadOptions, load_path};
use ropey::Rope;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// A snippet holds at most this many bytes.
pub const SNIPPET_MAX: usize = 240;

/// A window starts at most this many bytes before the match.
const LEAD_MAX: usize = 40;

/// One match, shown with the line it is on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hit {
    /// The match in the text, byte offsets.
    pub range: Range<usize>,
    /// The line the match starts on, 0-based.
    pub line: usize,
    /// The text shown for it: the line the match starts on without its line break, with
    /// leading spaces and tabs removed, cut to at most `SNIPPET_MAX` bytes at a char boundary
    /// around the match, tabs replaced by single spaces and other control characters by as many
    /// spaces as they have bytes.
    pub snippet: String,
    /// Where the match is in `snippet`, byte offsets (an empty range at the right place when
    /// the match itself is empty; clipped to the snippet when the match runs past the line or
    /// the cut).
    pub mark: Range<usize>,
}

fn is_boundary(rope: &Rope, i: usize) -> bool {
    i >= rope.len_bytes() || (rope.byte(i) & 0xC0) != 0x80
}

/// Makes hits for matches given in buffer order; remembers the last line's facts so that many
/// matches on one line scan its leading blanks once.
struct Snipper<'a> {
    buf: &'a Buffer,
    /// Line, its text range without break, and where its text starts after leading blanks.
    cache: Option<(usize, Range<usize>, usize)>,
}

impl<'a> Snipper<'a> {
    fn new(buf: &'a Buffer) -> Snipper<'a> {
        Snipper { buf, cache: None }
    }

    fn hit(&mut self, range: Range<usize>) -> Hit {
        let buf = self.buf;
        let rope = buf.rope();
        let line = buf.line_of(range.start);
        let (text, content) = match &self.cache {
            Some((l, t, c)) if *l == line => (t.clone(), *c),
            _ => {
                let t = buf.line_range(line);
                let blanks = rope
                    .byte_slice(t.clone())
                    .bytes()
                    .take_while(|&b| b == b' ' || b == b'\t')
                    .count();
                let c = t.start + blanks;
                self.cache = Some((line, t.clone(), c));
                (t, c)
            }
        };
        let end = text.end;
        let (mut w0, mut w1) = (content, end);
        if end - content > SNIPPET_MAX {
            let rel = range.start.saturating_sub(content);
            w0 = content + rel.saturating_sub(LEAD_MAX);
            while !is_boundary(rope, w0) {
                w0 += 1;
            }
            w1 = end.min(w0 + SNIPPET_MAX);
            while !is_boundary(rope, w1) {
                w1 -= 1;
            }
        }
        let mut snippet = String::with_capacity(w1 - w0);
        for chunk in rope.byte_slice(w0..w1).chunks() {
            for c in chunk.chars() {
                if c.is_control() {
                    for _ in 0..c.len_utf8() {
                        snippet.push(' ');
                    }
                } else {
                    snippet.push(c);
                }
            }
        }
        let start = range.start.clamp(w0, w1) - w0;
        let stop = range.end.clamp(w0, w1) - w0;
        Hit {
            range,
            line,
            snippet,
            mark: start..stop.max(start),
        }
    }
}

/// The matches of `query` in a buffer, at most `cap`. The bool is true when there were more.
/// A match that spans lines is one hit on its first line.
pub fn in_buffer(buf: &Buffer, query: &Query, cap: usize, cancel: &AtomicBool) -> (Vec<Hit>, bool) {
    let m = find_all(buf.rope(), query, cap.saturating_add(1), cancel);
    let more = m.ranges.len() > cap || m.capped;
    let mut snipper = Snipper::new(buf);
    let hits = m
        .ranges
        .into_iter()
        .take(cap)
        .map(|r| snipper.hit(r))
        .collect();
    (hits, more)
}

/// What a search of a folder leaves out and where it stops.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TreeOptions {
    /// Files larger than this are skipped.
    pub max_file_bytes: u64,
    /// The search stops after this many hits in all.
    pub max_hits: usize,
    /// At most this many hits are kept of one file (the search goes on with the next file).
    pub max_hits_per_file: usize,
    /// Files and folders whose name starts with a dot are skipped.
    pub skip_hidden: bool,
}

impl Default for TreeOptions {
    fn default() -> TreeOptions {
        TreeOptions {
            max_file_bytes: 16 << 20,
            max_hits: 20_000,
            max_hits_per_file: 2_000,
            skip_hidden: true,
        }
    }
}

/// What a tree search reports as it goes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// A file with at least one hit. `more` is true when the file had more hits than were kept.
    File {
        path: PathBuf,
        hits: Vec<Hit>,
        more: bool,
    },
    /// Sent every so often (every 64 files): files looked at so far.
    Progress { files: usize },
    /// The end: files looked at (read and searched), files skipped (too large, binary,
    /// unreadable), whether it stopped at `max_hits`, whether it was cancelled.
    Done {
        files: usize,
        skipped: usize,
        truncated: bool,
        cancelled: bool,
    },
}

/// Folders never searched.
const SKIPPED_DIRS: [&str; 3] = [".git", "node_modules", "target"];

struct Walk<'a> {
    query: &'a Query,
    load: &'a LoadOptions,
    opts: &'a TreeOptions,
    cancel: &'a AtomicBool,
    sink: &'a mut dyn FnMut(Event),
    files: usize,
    skipped: usize,
    hits: usize,
    truncated: bool,
    cancelled: bool,
}

impl Walk<'_> {
    fn stopped(&self) -> bool {
        self.truncated || self.cancelled
    }

    fn dir(&mut self, dir: &Path) {
        let Ok(read) = std::fs::read_dir(dir) else {
            return;
        };
        let mut files: Vec<(String, PathBuf)> = Vec::new();
        let mut dirs: Vec<(String, PathBuf)> = Vec::new();
        for entry in read.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if self.opts.skip_hidden && name.starts_with('.') {
                continue;
            }
            let path = entry.path();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let (is_dir, is_file) = if kind.is_symlink() {
                match std::fs::metadata(&path) {
                    // A link to a folder is not followed; one to a file is searched.
                    Ok(m) => (false, m.is_file()),
                    Err(_) => continue,
                }
            } else {
                (kind.is_dir(), kind.is_file())
            };
            let key = format!("{}\0{}", name.to_ascii_lowercase(), name);
            if is_dir {
                if !SKIPPED_DIRS.contains(&name.as_str()) {
                    dirs.push((key, path));
                }
            } else if is_file {
                files.push((key, path));
            }
        }
        files.sort();
        dirs.sort();
        for (_, path) in files {
            self.file(&path);
            if self.stopped() {
                return;
            }
        }
        for (_, path) in dirs {
            self.dir(&path);
            if self.stopped() {
                return;
            }
        }
    }

    fn file(&mut self, path: &Path) {
        if self.cancel.load(Ordering::Relaxed) {
            self.cancelled = true;
            return;
        }
        if self.hits >= self.opts.max_hits {
            self.truncated = true;
            return;
        }
        match std::fs::metadata(path) {
            Ok(m) if m.len() <= self.opts.max_file_bytes => {}
            _ => {
                self.skipped += 1;
                return;
            }
        }
        let loaded = match load_path(path, self.load, &|_, _| {}, self.cancel) {
            Ok(l) => l,
            Err(LoadError::Cancelled) => {
                self.cancelled = true;
                return;
            }
            Err(_) => {
                self.skipped += 1;
                return;
            }
        };
        if loaded.info.has_nul {
            self.skipped += 1;
            return;
        }
        self.files += 1;
        let cap = self
            .opts
            .max_hits_per_file
            .min(self.opts.max_hits - self.hits);
        let (hits, more) = in_buffer(&loaded.buffer, self.query, cap, self.cancel);
        drop(loaded);
        if self.cancel.load(Ordering::Relaxed) {
            self.cancelled = true;
        }
        self.hits += hits.len();
        if !hits.is_empty() {
            (self.sink)(Event::File {
                path: path.to_path_buf(),
                hits,
                more,
            });
        }
        if self.files.is_multiple_of(64) {
            (self.sink)(Event::Progress { files: self.files });
        }
        if self.hits >= self.opts.max_hits {
            self.truncated = true;
        }
    }
}

/// Searches every file under `root` (a folder, searched recursively; or a single file), in a
/// stable order: within a folder, files first then subfolders, each sorted by name ignoring
/// ASCII case. Symbolic links to folders are not followed. Folders named `.git`, `node_modules`
/// and `target` are skipped always; names starting with a dot are skipped when
/// `opts.skip_hidden`. A file is skipped when it is larger than `opts.max_file_bytes`, cannot
/// be read, or is binary (`FileInfo::has_nul`). Each file is read with `load_path` and
/// `load` (the encoding detection the editor uses). `sink` is called on the calling thread
/// for every event; the last one is always `Event::Done`. `cancel` is looked at between files
/// and passed to the load and the find.
///
/// `Done.truncated` is set as soon as the hits kept reach `opts.max_hits`, even when nothing
/// more would have been found.
pub fn in_tree(
    root: &Path,
    query: &Query,
    load: &LoadOptions,
    opts: &TreeOptions,
    cancel: &AtomicBool,
    sink: &mut dyn FnMut(Event),
) {
    let mut walk = Walk {
        query,
        load,
        opts,
        cancel,
        sink,
        files: 0,
        skipped: 0,
        hits: 0,
        truncated: false,
        cancelled: cancel.load(Ordering::Relaxed),
    };
    if !walk.cancelled {
        match std::fs::metadata(root) {
            Ok(m) if m.is_dir() => walk.dir(root),
            Ok(m) if m.is_file() => walk.file(root),
            _ => {}
        }
    }
    let done = Event::Done {
        files: walk.files,
        skipped: walk.skipped,
        truncated: walk.truncated,
        cancelled: walk.cancelled,
    };
    (walk.sink)(done);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::find::FindOptions;
    use std::fs;

    fn no_cancel() -> AtomicBool {
        AtomicBool::new(false)
    }

    fn q(text: &str, o: FindOptions) -> Query {
        Query::new(text, o).unwrap()
    }

    fn plain(text: &str) -> Query {
        q(text, FindOptions::default())
    }

    fn run(text: &str, query: &Query, cap: usize) -> (Vec<Hit>, bool) {
        in_buffer(&Buffer::from_text(text), query, cap, &no_cancel())
    }

    #[test]
    fn plain_hit() {
        let (h, more) = run("hello world\nfoo needle bar\n", &plain("needle"), 10);
        assert!(!more);
        assert_eq!(
            h,
            vec![Hit {
                range: 16..22,
                line: 1,
                snippet: "foo needle bar".into(),
                mark: 4..10
            }]
        );
    }

    #[test]
    fn match_case_and_whole_word() {
        let t = "Cat cat concat cat.";
        let (h, _) = run(t, &plain("cat"), 10);
        assert_eq!(h.len(), 4);
        let mc = FindOptions {
            match_case: true,
            ..Default::default()
        };
        let (h, _) = run(t, &q("Cat", mc), 10);
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].mark, 0..3);
        let ww = FindOptions {
            whole_word: true,
            ..Default::default()
        };
        let (h, _) = run(t, &q("cat", ww), 10);
        let marks: Vec<_> = h.iter().map(|h| h.mark.clone()).collect();
        assert_eq!(marks, vec![0..3, 4..7, 15..18]);
    }

    #[test]
    fn regex_hits() {
        let re = FindOptions {
            regex: true,
            ..Default::default()
        };
        let (h, _) = run(
            "a\n  fn one()\nlet fn_x\n\tfn two",
            &q(r"^\s*fn \w+", re),
            10,
        );
        assert_eq!(h.len(), 2);
        assert_eq!((h[0].line, h[0].snippet.as_str()), (1, "fn one()"));
        assert_eq!(h[0].mark, 0..6);
        assert_eq!((h[1].line, h[1].snippet.as_str()), (3, "fn two"));
        assert_eq!(h[1].mark, 0..6);
        // Empty matches are not reported by find_all.
        let (h, more) = run("abc", &q("x*", re), 10);
        assert!(h.is_empty() && !more);
    }

    #[test]
    fn empty_range_mark() {
        let b = Buffer::from_text("\t ab\ncd");
        let mut s = Snipper::new(&b);
        let h = s.hit(3..3);
        assert_eq!((h.line, h.snippet.as_str(), h.mark), (0, "ab", 1..1));
        let h = s.hit(1..1);
        assert_eq!(h.mark, 0..0);
        let h = s.hit(7..7);
        assert_eq!((h.line, h.snippet.as_str(), h.mark), (1, "cd", 2..2));
    }

    #[test]
    fn several_on_one_line() {
        let (h, _) = run("ab ab ab\nx", &plain("ab"), 10);
        assert_eq!(h.len(), 3);
        assert!(h.iter().all(|h| h.line == 0 && h.snippet == "ab ab ab"));
        let marks: Vec<_> = h.iter().map(|h| h.mark.clone()).collect();
        assert_eq!(marks, vec![0..2, 3..5, 6..8]);
    }

    #[test]
    fn match_across_lines() {
        let (h, _) = run("xxa\nbyy\nz", &plain("a\nb"), 10);
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].range, 2..5);
        assert_eq!(
            (h[0].line, h[0].snippet.as_str(), h[0].mark.clone()),
            (0, "xxa", 2..3)
        );
        let (h, _) = run("xxa\r\nbyy", &plain("a\nb"), 10);
        assert_eq!(h[0].range, 2..6);
        assert_eq!((h[0].snippet.as_str(), h[0].mark.clone()), ("xxa", 2..3));
    }

    #[test]
    fn crlf_and_cr_lines() {
        let (h, _) = run("one\r\ntwo needle\r\nthree", &plain("needle"), 10);
        assert_eq!(
            (h[0].line, h[0].snippet.as_str(), h[0].mark.clone()),
            (1, "two needle", 4..10)
        );
        let (h, _) = run("one\rtwo needle\rthree", &plain("needle"), 10);
        assert_eq!((h[0].line, h[0].snippet.as_str()), (1, "two needle"));
    }

    #[test]
    fn cap_and_more() {
        let t = "a a a a a";
        let (h, more) = run(t, &plain("a"), 3);
        assert_eq!((h.len(), more), (3, true));
        let (h, more) = run(t, &plain("a"), 5);
        assert_eq!((h.len(), more), (5, false));
        let (h, more) = run(t, &plain("a"), 10);
        assert_eq!((h.len(), more), (5, false));
        let (h, more) = run(t, &plain("a"), 0);
        assert_eq!((h.len(), more), (0, true));
        let (h, more) = run(t, &plain("zzz"), 0);
        assert_eq!((h.len(), more), (0, false));
    }

    #[test]
    fn leading_blanks_stripped() {
        let (h, _) = run("a\n \t  needle x\n\t\tneedle", &plain("needle"), 10);
        assert_eq!(
            (h[0].line, h[0].snippet.as_str(), h[0].mark.clone()),
            (1, "needle x", 0..6)
        );
        assert_eq!((h[1].snippet.as_str(), h[1].mark.clone()), ("needle", 0..6));
        // A match inside the blanks is clipped to the start.
        let (h, _) = run("\t\t  x", &plain("\t "), 10);
        assert_eq!((h[0].snippet.as_str(), h[0].mark.clone()), ("x", 0..0));
        // A line of only blanks has an empty snippet.
        let (h, _) = run("   \t", &plain(" "), 10);
        assert_eq!((h[0].snippet.as_str(), h[0].mark.clone()), ("", 0..0));
    }

    fn long_line(len: usize, at: &[usize]) -> String {
        let mut v = vec![b'x'; len];
        for &p in at {
            v[p..p + 6].copy_from_slice(b"needle");
        }
        String::from_utf8(v).unwrap()
    }

    #[test]
    fn long_line_start_middle_end() {
        let t = long_line(1000, &[0, 500, 994]);
        let (h, _) = run(&t, &plain("needle"), 10);
        assert_eq!(h.len(), 3);
        assert_eq!((h[0].snippet.len(), h[0].mark.clone()), (240, 0..6));
        assert_eq!(h[0].snippet, t[0..240]);
        assert_eq!((h[1].snippet.len(), h[1].mark.clone()), (240, 40..46));
        assert_eq!(h[1].snippet, t[460..700]);
        assert_eq!((h[2].snippet.len(), h[2].mark.clone()), (46, 40..46));
        assert_eq!(h[2].snippet, t[954..1000]);
        // A match starting within the first 40 bytes starts the window at 0.
        let t = long_line(1000, &[30]);
        let (h, _) = run(&t, &plain("needle"), 10);
        assert_eq!((h[0].snippet.len(), h[0].mark.clone()), (240, 30..36));
    }

    #[test]
    fn long_line_after_blanks() {
        let mut t = String::from("\t  ");
        t.push_str(&"y".repeat(300));
        t.push_str("needle");
        t.push_str(&"y".repeat(10));
        let (h, _) = run(&t, &plain("needle"), 10);
        assert_eq!(h[0].snippet.len(), 56);
        assert_eq!(h[0].mark, 40..46);
        assert_eq!(h[0].snippet, t[3 + 260..]);
    }

    #[test]
    fn long_line_match_runs_past_cut() {
        let t = "z".repeat(400);
        let (h, _) = run(&t, &plain(&"z".repeat(300)), 10);
        assert_eq!(h[0].range, 0..300);
        assert_eq!((h[0].snippet.len(), h[0].mark.clone()), (240, 0..240));
    }

    #[test]
    fn multibyte_cut() {
        let mut t = "日".repeat(100);
        t.push_str("needle");
        t.push_str(&"😀".repeat(100));
        let (h, _) = run(&t, &plain("needle"), 10);
        // The window would start at 260, inside a character: it moves forward to 261. It would
        // end at 501, inside an emoji: it moves back to 498.
        assert_eq!(h[0].snippet, t[261..498]);
        assert_eq!(h[0].mark, 39..45);
    }

    #[test]
    fn multibyte_cuts_always_on_boundaries() {
        for k in 0..60 {
            for tail in ["😀ş", "日本", "İıÇğ"] {
                let mut t = "x".repeat(k);
                t.push_str(&"日ş".repeat(60));
                t.push_str("needle");
                t.push_str(&tail.repeat(100));
                t.push_str("\nnext");
                let (h, _) = run(&t, &plain("needle"), 10);
                let h = &h[0];
                assert!(h.snippet.len() <= SNIPPET_MAX);
                assert_eq!(&h.snippet[h.mark.clone()], "needle", "k={k} tail={tail}");
                assert!(t.contains(&h.snippet));
                assert!(h.mark.start <= LEAD_MAX);
            }
        }
    }

    #[test]
    fn turkish_text_short_line() {
        let (h, _) = run("Çalışma ĞÜŞ İstanbul ışık", &plain("istanbul"), 10);
        assert_eq!(h[0].snippet, "Çalışma ĞÜŞ İstanbul ışık");
        assert_eq!(&h[0].snippet[h[0].mark.clone()], "İstanbul");
    }

    #[test]
    fn control_characters() {
        let (h, _) = run("a\u{1}b\tneedle\u{85}x\u{7f}", &plain("needle"), 10);
        // U+0085 has two bytes: two spaces.
        assert_eq!(h[0].snippet, "a b needle  x ");
        assert_eq!(h[0].mark, 4..10);
        assert_eq!(&h[0].snippet[h[0].mark.clone()], "needle");
        let (h, _) = run("a\tb\tc", &plain("c"), 10);
        assert_eq!((h[0].snippet.as_str(), h[0].mark.clone()), ("a b c", 4..5));
    }

    #[test]
    fn five_megabyte_line() {
        let mut t = "x".repeat(5_000_000);
        t.push_str("needle");
        t.push_str("tail");
        let b = Buffer::from_text(&t);
        let start = std::time::Instant::now();
        let (h, more) = in_buffer(&b, &plain("needle"), 10, &no_cancel());
        let took = start.elapsed();
        assert!(!more);
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].line, 0);
        assert_eq!(h[0].range, 5_000_000..5_000_006);
        assert_eq!(h[0].snippet, format!("{}needletail", "x".repeat(40)));
        assert_eq!(h[0].mark, 40..46);
        assert!(took.as_secs() < 5, "took {took:?}");
    }

    // ---- in_tree ----

    struct TmpDir(PathBuf);

    impl TmpDir {
        fn new(tag: &str) -> TmpDir {
            static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let p = std::env::temp_dir().join(format!(
                "mdedit-search-{tag}-{}-{}",
                std::process::id(),
                N.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = fs::remove_dir_all(&p);
            fs::create_dir_all(&p).unwrap();
            TmpDir(p)
        }

        fn write(&self, rel: &str, bytes: &[u8]) {
            let p = self.0.join(rel);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, bytes).unwrap();
        }
    }

    impl Drop for TmpDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn load_opts() -> LoadOptions {
        LoadOptions {
            legacy: encoding_rs::WINDOWS_1254,
            ..LoadOptions::default()
        }
    }

    fn tree(root: &Path, query: &Query, opts: &TreeOptions) -> Vec<Event> {
        let mut ev = Vec::new();
        in_tree(root, query, &load_opts(), opts, &no_cancel(), &mut |e| {
            ev.push(e)
        });
        ev
    }

    fn make_tree(max_big: usize) -> TmpDir {
        let t = TmpDir::new("tree");
        t.write("a.txt", b"needle one\nplain\n  needle two\n");
        t.write("B.txt", b"a Needle\r\n");
        t.write("c.txt", b"nothing here\n");
        t.write("empty.txt", b"");
        t.write(".hidden.txt", b"needle hidden\n");
        let mut bin = b"needle\0".to_vec();
        bin.extend_from_slice(b"more bytes");
        t.write("bin.dat", &bin);
        let mut big = b"needle ".to_vec();
        big.resize(max_big, b'.');
        t.write("big.txt", &big);
        // Windows-1254: "Çalışma needle ğüş" with ı=0xFD, ş=0xFE, ğ=0xF0, Ç=0xC7, ü=0xFC.
        t.write("tr1254.txt", b"\xC7al\xFD\xFEma needle \xF0\xFC\xFE\n");
        let mut u16 = vec![0xFF, 0xFE];
        for unit in "wide needle \u{65E5}\n".encode_utf16() {
            u16.extend_from_slice(&unit.to_le_bytes());
        }
        t.write("u16.txt", &u16);
        t.write(".git/x.txt", b"needle\n");
        t.write("node_modules/x.txt", b"needle\n");
        t.write("target/x.txt", b"needle\n");
        t.write("zeta/last.txt", b"needle last\n");
        t.write("Mid/inner.txt", b"needle inner\n");
        t.write("sub/z.txt", b"needle z\n");
        t.write("sub/Alpha/q.txt", b"needle q\n");
        t.write("sub/Alpha/.hid/r.txt", b"needle r\n");
        t
    }

    fn rel(root: &Path, e: &Event) -> Option<String> {
        match e {
            Event::File { path, .. } => Some(
                path.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/"),
            ),
            _ => None,
        }
    }

    #[test]
    fn tree_order_and_counts() {
        let t = make_tree(2000);
        let opts = TreeOptions {
            max_file_bytes: 1000,
            ..TreeOptions::default()
        };
        let ev = tree(&t.0, &plain("needle"), &opts);
        let paths: Vec<String> = ev.iter().filter_map(|e| rel(&t.0, e)).collect();
        assert_eq!(
            paths,
            [
                "a.txt",
                "B.txt",
                "tr1254.txt",
                "u16.txt",
                "Mid/inner.txt",
                "sub/z.txt",
                "sub/Alpha/q.txt",
                "zeta/last.txt"
            ]
        );
        // Read and searched: a, B, c, empty, tr1254, u16, inner, z, q, last. Skipped: bin, big.
        assert_eq!(
            ev.last().unwrap(),
            &Event::Done {
                files: 10,
                skipped: 2,
                truncated: false,
                cancelled: false
            }
        );
        assert_eq!(ev.len(), 9);
        let Event::File { hits, more, .. } = &ev[0] else {
            panic!()
        };
        assert!(!more);
        assert_eq!(hits.len(), 2);
        assert_eq!(
            (hits[0].line, hits[0].snippet.as_str(), hits[0].mark.clone()),
            (0, "needle one", 0..6)
        );
        assert_eq!(
            (hits[1].line, hits[1].snippet.as_str(), hits[1].mark.clone()),
            (2, "needle two", 0..6)
        );
        let Event::File { hits, .. } = &ev[1] else {
            panic!()
        };
        assert_eq!(
            (hits[0].snippet.as_str(), hits[0].mark.clone()),
            ("a Needle", 2..8)
        );
        let Event::File { hits, .. } = &ev[2] else {
            panic!()
        };
        assert_eq!(hits[0].snippet, "Çalışma needle ğüş");
        assert_eq!(&hits[0].snippet[hits[0].mark.clone()], "needle");
        let Event::File { hits, .. } = &ev[3] else {
            panic!()
        };
        assert_eq!(hits[0].snippet, "wide needle \u{65E5}");
        assert_eq!(&hits[0].snippet[hits[0].mark.clone()], "needle");
    }

    #[test]
    fn tree_hidden_included_when_asked() {
        let t = make_tree(2000);
        let opts = TreeOptions {
            max_file_bytes: 1000,
            skip_hidden: false,
            ..TreeOptions::default()
        };
        let ev = tree(&t.0, &plain("needle"), &opts);
        let paths: Vec<String> = ev.iter().filter_map(|e| rel(&t.0, e)).collect();
        assert_eq!(paths[0], ".hidden.txt");
        assert_eq!(paths[1], "a.txt");
        assert!(paths.contains(&"sub/Alpha/.hid/r.txt".to_string()));
        // .git, node_modules and target stay out.
        assert!(!paths.iter().any(|p| p.starts_with(".git")
            || p.starts_with("node_modules")
            || p.starts_with("target")));
        assert_eq!(paths.len(), 10);
    }

    #[test]
    fn tree_max_hits_and_per_file() {
        let t = make_tree(2000);
        let base = TreeOptions {
            max_file_bytes: 1000,
            ..TreeOptions::default()
        };
        let o = TreeOptions {
            max_hits: 3,
            ..base
        };
        let ev = tree(&t.0, &plain("needle"), &o);
        let paths: Vec<String> = ev.iter().filter_map(|e| rel(&t.0, e)).collect();
        assert_eq!(paths, ["a.txt", "B.txt"]);
        assert_eq!(
            ev.last().unwrap(),
            &Event::Done {
                files: 2,
                skipped: 0,
                truncated: true,
                cancelled: false
            }
        );
        // The limit falls inside a file: that file is cut and says there was more.
        let o = TreeOptions {
            max_hits: 1,
            ..base
        };
        let ev = tree(&t.0, &plain("needle"), &o);
        let Event::File { hits, more, .. } = &ev[0] else {
            panic!()
        };
        assert_eq!((hits.len(), *more), (1, true));
        assert_eq!(ev.len(), 2);
        assert!(matches!(
            ev[1],
            Event::Done {
                truncated: true,
                ..
            }
        ));

        let o = TreeOptions {
            max_hits_per_file: 1,
            ..base
        };
        let ev = tree(&t.0, &plain("needle"), &o);
        let Event::File { hits, more, .. } = &ev[0] else {
            panic!()
        };
        assert_eq!((hits.len(), *more), (1, true));
        let Event::File { hits, more, .. } = &ev[1] else {
            panic!()
        };
        assert_eq!((hits.len(), *more), (1, false));
        let paths: Vec<String> = ev.iter().filter_map(|e| rel(&t.0, e)).collect();
        assert_eq!(paths.len(), 8);
        assert!(matches!(
            ev.last().unwrap(),
            Event::Done {
                truncated: false,
                ..
            }
        ));
    }

    #[test]
    fn tree_cancel_before_start() {
        let t = make_tree(2000);
        let cancel = AtomicBool::new(true);
        let mut ev = Vec::new();
        let (l, o) = (load_opts(), TreeOptions::default());
        in_tree(&t.0, &plain("needle"), &l, &o, &cancel, &mut |e| ev.push(e));
        assert_eq!(
            ev,
            [Event::Done {
                files: 0,
                skipped: 0,
                truncated: false,
                cancelled: true
            }]
        );
    }

    #[test]
    fn tree_cancel_from_sink() {
        let t = make_tree(2000);
        let cancel = AtomicBool::new(false);
        let mut ev = Vec::new();
        let (l, o) = (load_opts(), TreeOptions::default());
        in_tree(&t.0, &plain("needle"), &l, &o, &cancel, &mut |e| {
            if matches!(e, Event::File { .. }) {
                cancel.store(true, Ordering::Relaxed);
            }
            ev.push(e);
        });
        assert_eq!(ev.len(), 2);
        assert!(matches!(ev[0], Event::File { .. }));
        assert_eq!(
            ev[1],
            Event::Done {
                files: 1,
                skipped: 0,
                truncated: false,
                cancelled: true
            }
        );
    }

    #[test]
    fn tree_single_file_and_missing_root() {
        let t = make_tree(2000);
        let one = t.0.join("sub").join("z.txt");
        let ev = tree(&one, &plain("needle"), &TreeOptions::default());
        assert_eq!(ev.len(), 2);
        assert!(
            matches!(&ev[0], Event::File { path, hits, .. } if *path == one && hits.len() == 1)
        );
        assert_eq!(
            ev[1],
            Event::Done {
                files: 1,
                skipped: 0,
                truncated: false,
                cancelled: false
            }
        );
        // A hidden file named as the root is searched.
        let hid = t.0.join(".hidden.txt");
        let ev = tree(&hid, &plain("needle"), &TreeOptions::default());
        assert_eq!(ev.len(), 2);
        let ev = tree(&t.0.join("nope"), &plain("needle"), &TreeOptions::default());
        assert_eq!(
            ev,
            [Event::Done {
                files: 0,
                skipped: 0,
                truncated: false,
                cancelled: false
            }]
        );
    }

    #[test]
    fn tree_progress_every_64_files() {
        let t = TmpDir::new("progress");
        for i in 0..130 {
            t.write(&format!("f{i:03}.txt"), b"x\n");
        }
        let ev = tree(&t.0, &plain("needle"), &TreeOptions::default());
        assert_eq!(
            ev,
            [
                Event::Progress { files: 64 },
                Event::Progress { files: 128 },
                Event::Done {
                    files: 130,
                    skipped: 0,
                    truncated: false,
                    cancelled: false
                }
            ]
        );
    }

    #[test]
    #[ignore]
    fn timing_search_own_crates() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let query = plain("fn ");
        let start = std::time::Instant::now();
        let (mut files, mut hits, mut with_hits) = (0, 0, 0);
        in_tree(
            &root,
            &query,
            &LoadOptions::default(),
            &TreeOptions::default(),
            &no_cancel(),
            &mut |e| match e {
                Event::File { hits: h, .. } => {
                    with_hits += 1;
                    hits += h.len();
                }
                Event::Done { files: f, .. } => files = f,
                Event::Progress { .. } => {}
            },
        );
        println!(
            "searched {files} files, {with_hits} with hits, {hits} hits in {:?}",
            start.elapsed()
        );
        assert!(hits > 0);
    }
}
