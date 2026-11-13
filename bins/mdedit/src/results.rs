//! The results panel under the text: what "Find in files" and "Find in open tabs" found, a row
//! for each file and under it a row for each match. A folder is searched on a worker thread
//! (`mdedit_core::search::in_tree`), whose events are taken in as they come; a click on a match
//! opens the file there.

use crate::app::Wake;
use crate::chrome::{Ctx, glyph};
use crate::docs::Waker;
use crate::ui::{Drag, Hit, Scroller, Ui};
use mdedit_core::find::Query;
use mdedit_core::load::LoadOptions;
use mdedit_core::search::{self, Event, Hit as Found, TreeOptions};
use mdedit_highlight::DocId;
use mdshared_ui::gfx::{Face, Rect, mix};
use mdshared_ui::pane::draw_row_fill;
use mdshared_ui::scroll::{draw_scrollbar, scrollbar};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

/// The panel's height, logical px, and what it leaves the text at least.
pub const RESULTS_H: f32 = 230.0;
pub const RESULTS_TEXT_MIN: f32 = 140.0;
/// The panel's title row.
const HEAD: f32 = 34.0;
/// A row of the list and the part of it that is filled.
const ROW: f32 = 25.0;
const ROW_H: f32 = 23.0;
/// Matches kept of one open tab.
pub const TAB_HITS_MAX: usize = 2000;
/// An open tab with more text than this is not searched by "Find in open tabs", which runs
/// while the window waits.
pub const TAB_BYTES_MAX: usize = 64 << 20;
/// The worker wakes the window at most this often while files with matches come in.
const WAKE_EVERY: Duration = Duration::from_millis(40);

/// What was found in one open tab: its id, name and file, the matches, and whether there were
/// more.
pub type TabHits = (DocId, String, Option<PathBuf>, Vec<Found>, bool);

pub enum Row {
    /// A file with matches: where it is, or the open tab it is (a new document has no file).
    File {
        path: Option<PathBuf>,
        doc: Option<DocId>,
        name: String,
        count: usize,
        more: bool,
    },
    Hit(Found),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum State {
    Idle,
    Running {
        files: usize,
    },
    Done {
        skipped: usize,
        truncated: bool,
        cancelled: bool,
    },
}

pub struct Results {
    pub open: bool,
    pub rows: Vec<Row>,
    pub hits: usize,
    pub files: usize,
    pub state: State,
    /// What was looked for and where, for the title row.
    pub what: String,
    pub place: String,
    pub scroll: f32,
    pub content: f32,
    pub view: f32,
    /// The row last gone to.
    pub selected: Option<usize>,
    /// Counts every change of what the panel shows.
    pub generation: u64,
    root: PathBuf,
    rx: Option<Receiver<Event>>,
    cancel: Arc<AtomicBool>,
}

impl Default for Results {
    fn default() -> Results {
        Results {
            open: false,
            rows: Vec::new(),
            hits: 0,
            files: 0,
            state: State::Idle,
            what: String::new(),
            place: String::new(),
            scroll: 0.0,
            content: 0.0,
            view: 0.0,
            selected: None,
            generation: 0,
            root: PathBuf::new(),
            rx: None,
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }
}

/// The name a folder or file is shown by.
fn short(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

impl Results {
    fn reset(&mut self, what: &str, place: String) {
        // A search still running stops; its events go nowhere.
        self.cancel.store(true, Ordering::Relaxed);
        self.cancel = Arc::new(AtomicBool::new(false));
        self.rx = None;
        self.rows.clear();
        (self.hits, self.files) = (0, 0);
        self.what = what.to_string();
        self.place = place;
        self.scroll = 0.0;
        self.selected = None;
        self.open = true;
        self.generation += 1;
    }

    /// Starts a search of `root` on a worker thread.
    pub fn start_tree(&mut self, root: &Path, query: Query, load: LoadOptions, waker: Waker) {
        self.reset(query.text(), format!("in {}", short(root)));
        self.root = root.to_path_buf();
        self.state = State::Running { files: 0 };
        let (tx, rx) = channel();
        self.rx = Some(rx);
        let cancel = self.cancel.clone();
        let root = root.to_path_buf();
        std::thread::spawn(move || {
            let mut woke = Instant::now();
            let opts = TreeOptions::default();
            search::in_tree(&root, &query, &load, &opts, &cancel, &mut |event| {
                let done = matches!(event, Event::Done { .. });
                if tx.send(event).is_err() {
                    cancel.store(true, Ordering::Relaxed);
                    return;
                }
                if done || woke.elapsed() >= WAKE_EVERY {
                    woke = Instant::now();
                    waker(Wake::Results);
                }
            });
        });
    }

    /// Shows what was found in the open tabs: for each, its id, name, file and matches.
    pub fn set_tabs(&mut self, what: &str, tabs: Vec<TabHits>, left_out: bool) {
        self.reset(what, "in the open tabs".into());
        let mut truncated = left_out;
        for (id, name, path, hits, more) in tabs {
            if hits.is_empty() {
                continue;
            }
            truncated |= more;
            self.push_file(path, Some(id), name, hits, more);
        }
        self.state = State::Done {
            skipped: 0,
            truncated,
            cancelled: false,
        };
    }

    fn push_file(
        &mut self,
        path: Option<PathBuf>,
        doc: Option<DocId>,
        name: String,
        hits: Vec<Found>,
        more: bool,
    ) {
        self.files += 1;
        self.hits += hits.len();
        self.rows.push(Row::File {
            path,
            doc,
            name,
            count: hits.len(),
            more,
        });
        self.rows.extend(hits.into_iter().map(Row::Hit));
    }

    /// Takes in what the worker has sent. True when the panel changed.
    pub fn poll(&mut self) -> bool {
        let Some(rx) = self.rx.take() else {
            return false;
        };
        let mut changed = false;
        let mut finished = false;
        for event in rx.try_iter() {
            changed = true;
            match event {
                Event::File { path, hits, more } => {
                    let name = path
                        .strip_prefix(&self.root)
                        .ok()
                        .filter(|p| !p.as_os_str().is_empty())
                        .map_or_else(|| short(&path), |p| p.display().to_string());
                    self.push_file(Some(path), None, name, hits, more);
                }
                Event::Progress { files } => self.state = State::Running { files },
                Event::Done {
                    skipped,
                    truncated,
                    cancelled,
                    ..
                } => {
                    self.state = State::Done {
                        skipped,
                        truncated,
                        cancelled,
                    };
                    finished = true;
                }
            }
        }
        if !finished {
            self.rx = Some(rx);
        }
        if changed {
            self.generation += 1;
        }
        changed
    }

    /// Closes the panel and stops a search still running.
    pub fn close(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        self.rx = None;
        if matches!(self.state, State::Running { .. }) {
            self.state = State::Done {
                skipped: 0,
                truncated: false,
                cancelled: true,
            };
        }
        self.open = false;
        self.generation += 1;
    }

    /// The match row after the selected one (before it when not `forward`), going round.
    pub fn step(&self, forward: bool) -> Option<usize> {
        let n = self.rows.len();
        let is_hit = |i: &usize| matches!(self.rows[*i], Row::Hit(_));
        match (self.selected, forward) {
            (Some(s), true) => (s + 1..n)
                .chain(0..=s.min(n.saturating_sub(1)))
                .find(is_hit),
            (Some(s), false) => (0..s.min(n)).rev().chain((s.min(n)..n).rev()).find(is_hit),
            (None, true) => (0..n).find(is_hit),
            (None, false) => (0..n).rev().find(is_hit),
        }
    }

    /// Where a match row leads: its file or tab, and the match's range in the text.
    pub fn target(&self, row: usize) -> Option<(Option<PathBuf>, Option<DocId>, Range<usize>)> {
        let Row::Hit(hit) = self.rows.get(row)? else {
            return None;
        };
        let file = self.rows[..row].iter().rev().find_map(|r| match r {
            Row::File { path, doc, .. } => Some((path.clone(), *doc)),
            Row::Hit(_) => None,
        })?;
        Some((file.0, file.1, hit.range.clone()))
    }

    /// Marks a row as the one gone to and scrolls it into the list.
    pub fn select(&mut self, row: usize) {
        self.selected = Some(row);
        let y = row as f32 * ROW;
        if y < self.scroll {
            self.scroll = y;
        } else if y + ROW > self.scroll + self.view {
            self.scroll = y + ROW - self.view;
        }
        self.generation += 1;
    }

    pub fn scroll_by(&mut self, dy: f32) {
        self.scroll = (self.scroll + dy)
            .clamp(0.0, (self.content - self.view).max(0.0))
            .round();
    }

    /// The title row's text.
    fn title(&self) -> String {
        let count = |n: usize, one: &str, many: &str| {
            if n == 1 {
                format!("1 {one}")
            } else {
                format!("{n} {many}")
            }
        };
        let mut s = format!(
            "\u{201C}{}\u{201D} {}: {} in {}",
            self.what,
            self.place,
            count(self.hits, "match", "matches"),
            count(self.files, "file", "files"),
        );
        match self.state {
            State::Idle => {}
            State::Running { files } => {
                s.push_str(&format!(" \u{00B7} searching, {files} files read\u{2026}"))
            }
            State::Done {
                skipped,
                truncated,
                cancelled,
            } => {
                if truncated {
                    s.push_str(" \u{00B7} more were left out");
                }
                if cancelled {
                    s.push_str(" \u{00B7} stopped");
                }
                if skipped > 0 {
                    s.push_str(&format!(
                        " \u{00B7} {} skipped",
                        count(skipped, "file", "files")
                    ));
                }
            }
        }
        s
    }
}

/// The list below the panel's title row.
pub fn list(r: Rect) -> Rect {
    Rect::new(r.x, r.y + HEAD, r.w, (r.h - HEAD).max(0.0))
}

pub fn draw(c: &mut Ctx, ui: &mut Ui, r: Rect) {
    let pal = *c.pal;
    c.p.fill(r, pal.pane);
    let px = c.p.px();
    c.p.fill(Rect::new(r.x, r.y, r.w, px), pal.line);
    let res = &mut ui.results;
    let title = res.title();
    c.p.text(
        Face::Semibold,
        12.5,
        r.x + 16.0,
        r.y + HEAD / 2.0 + 1.0,
        &title,
        pal.text,
        r.w - 64.0,
    );
    let b = Rect::new(r.right() - 36.0, r.y + 4.0, 26.0, 26.0);
    if let Some(fill) = c.fill_for(Hit::ResultsClose, false, false) {
        c.p.rbox(b, 5.0, fill);
    }
    c.p.icon(glyph::CLOSE, 10.0, b.cx(), b.cy(), pal.muted);
    c.hit(Hit::ResultsClose, b);

    let area = list(r);
    res.content = res.rows.len() as f32 * ROW + 6.0;
    res.view = area.h;
    res.scroll = res.scroll.clamp(0.0, (res.content - area.h).max(0.0));
    let scroll = res.scroll;
    if res.rows.is_empty() {
        let text = match res.state {
            State::Running { .. } => "Searching\u{2026}",
            _ => "Nothing found.",
        };
        let at = Rect::new(area.x, area.y + 14.0, area.w, 20.0);
        c.p.text_center(Face::Ui, 13.0, at, text, pal.muted);
    }
    let quiet = mix(pal.text, pal.muted, 0.35);
    let old = c.p.clip(area);
    let first = (scroll / ROW).floor() as usize;
    for (i, row) in res.rows.iter().enumerate().skip(first) {
        let y = area.y + i as f32 * ROW - scroll;
        if y > area.bottom() {
            break;
        }
        let rr = Rect::new(r.x + 6.0, y, r.w - 12.0, ROW_H);
        let cy = rr.cy();
        match row {
            Row::File {
                name, count, more, ..
            } => {
                c.p.icon(glyph::DOC, 13.0, r.x + 24.0, cy, pal.muted);
                let w = c.p.text(
                    Face::Semibold,
                    12.5,
                    r.x + 38.0,
                    cy,
                    name,
                    pal.text,
                    rr.w - 120.0,
                );
                let n = if *more {
                    format!("{count}+")
                } else {
                    count.to_string()
                };
                c.p.text(Face::Ui, 12.0, r.x + 48.0 + w, cy, &n, pal.faint, 80.0);
            }
            Row::Hit(hit) => {
                let h = Hit::ResultsRow(i);
                draw_row_fill(
                    c.p,
                    &pal,
                    rr,
                    c.hover == Some(h),
                    c.pressed == Some(h),
                    res.selected == Some(i),
                );
                let num = (hit.line + 1).to_string();
                c.p.text_right(Face::Ui, 12.0, r.x + 76.0, cy, &num, pal.faint);
                // The match gets the find mark's colour behind it.
                let x = r.x + 88.0;
                let max = (rr.right() - x - 14.0).max(0.0);
                let (a, b) = (
                    hit.mark.start.min(hit.snippet.len()),
                    hit.mark.end.min(hit.snippet.len()),
                );
                if hit.snippet.is_char_boundary(a) && hit.snippet.is_char_boundary(b) && a < b {
                    let xa = c.p.measure(Face::Ui, 13.0, &hit.snippet[..a]);
                    let xb = c.p.measure(Face::Ui, 13.0, &hit.snippet[..b]);
                    if xa < max {
                        let m =
                            Rect::new(x + xa - 1.0, y + 3.0, xb.min(max) - xa + 2.0, ROW_H - 6.0);
                        c.p.rbox_a(m, 3.0, pal.find_mark, if pal.dark { 0.38 } else { 0.45 });
                    }
                }
                c.p.text(Face::Ui, 13.0, x, cy, &hit.snippet, quiet, max);
                c.hit(h, rr);
            }
        }
    }
    c.p.restore(old);
    let thumb_hit = Hit::ScrollBar(Scroller::Results);
    if let Some((track, thumb)) = scrollbar(area, res.content, area.h, scroll) {
        let active = c.hover == Some(thumb_hit)
            || matches!(
                ui.drag,
                Some(Drag::Thumb {
                    target: Scroller::Results,
                    ..
                })
            );
        draw_scrollbar(c.p, &pal, track, thumb, active);
        c.hit(thumb_hit, track);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mdedit_core::Buffer;
    use mdedit_core::find::FindOptions;

    fn query(text: &str) -> Query {
        Query::new(text, FindOptions::default()).unwrap()
    }

    fn found(text: &str, what: &str) -> Vec<Found> {
        let cancel = AtomicBool::new(false);
        search::in_buffer(&Buffer::from_text(text), &query(what), 100, &cancel).0
    }

    #[test]
    fn rows_step_round_and_lead_to_their_file() {
        let mut res = Results::default();
        assert_eq!(res.step(true), None);
        res.set_tabs(
            "x",
            vec![
                (
                    1,
                    "a.txt".into(),
                    Some("a.txt".into()),
                    found("x\n.x\n", "x"),
                    false,
                ),
                (2, "Untitled 1".into(), None, Vec::new(), false),
                (3, "b.txt".into(), None, found("..x", "x"), true),
            ],
            false,
        );
        // File a, two matches, file b, one match: a tab with none has no row.
        assert_eq!((res.rows.len(), res.hits, res.files), (5, 3, 2));
        assert!(res.open);
        assert_eq!(
            res.state,
            State::Done {
                skipped: 0,
                truncated: true,
                cancelled: false
            }
        );
        assert_eq!(res.step(true), Some(1));
        assert_eq!(res.step(false), Some(4));
        let mut seen = Vec::new();
        for _ in 0..4 {
            let row = res.step(true).unwrap();
            res.select(row);
            seen.push(row);
        }
        assert_eq!(seen, [1, 2, 4, 1]);
        assert_eq!(res.step(false), Some(4));
        assert_eq!(res.target(0), None);
        assert_eq!(res.target(2), Some((Some("a.txt".into()), Some(1), 3..4)));
        assert_eq!(res.target(4), Some((None, Some(3), 2..3)));
        assert!(res.title().contains("3 matches in 2 files"));
        res.close();
        assert!(!res.open);
    }

    #[test]
    fn a_folder_is_searched_on_a_worker() {
        let dir = std::env::temp_dir().join(format!("mdedit-results-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("one.txt"), "needle\nhay\n").unwrap();
        std::fs::write(dir.join("sub").join("two.txt"), "hay\n  a needle\n").unwrap();
        std::fs::write(dir.join("three.txt"), "hay\n").unwrap();
        let load = LoadOptions {
            legacy: mdedit_core::encoding::legacy_for_code_page(1252),
            max_bytes: 1 << 20,
            forced: None,
        };
        let (tx, woken) = channel();
        let waker: Waker = Arc::new(move |_| {
            let _ = tx.send(());
        });
        let mut res = Results::default();
        res.start_tree(&dir, query("needle"), load, waker);
        assert_eq!(res.state, State::Running { files: 0 });
        let until = Instant::now() + Duration::from_secs(20);
        while matches!(res.state, State::Running { .. }) && Instant::now() < until {
            let _ = woken.recv_timeout(Duration::from_millis(50));
            res.poll();
        }
        assert_eq!(
            res.state,
            State::Done {
                skipped: 0,
                truncated: false,
                cancelled: false
            }
        );
        assert_eq!((res.hits, res.files, res.rows.len()), (2, 2, 4));
        let names: Vec<&str> = res
            .rows
            .iter()
            .filter_map(|r| match r {
                Row::File { name, .. } => Some(name.as_str()),
                Row::Hit(_) => None,
            })
            .collect();
        assert_eq!(names[0], "one.txt");
        assert_eq!(names[1].replace('\\', "/"), "sub/two.txt");
        let (path, doc, range) = res.target(3).unwrap();
        assert_eq!(path, Some(dir.join("sub").join("two.txt")));
        assert_eq!((doc, range), (None, 8..14));
        // A new search drops the old rows; closing stops it.
        res.start_tree(&dir, query("hay"), load, Arc::new(|_| {}));
        assert!(res.rows.is_empty() && res.selected.is_none());
        res.close();
        assert!(!res.poll());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
