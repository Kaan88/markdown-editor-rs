//! The function list: the headings, functions and types of the active document, shown in the left
//! pane in the Files tree's place. The list is made by `mdedit_outline::symbols` on a worker
//! thread that is given the text after every change while the list shows; a click on a row goes
//! to its line, and the row of the caret's place is marked.

use crate::app::Wake;
use crate::chrome::{Ctx, glyph};
use crate::docs::{Docs, Waker};
use crate::ui::{Drag, Hit, Scroller, Ui};
use mdedit_highlight::DocId;
use mdedit_outline::{Flavor, Kind, Symbol, symbols};
use mdshared_ui::gfx::{Face, Rect, mix};
use mdshared_ui::pane::{HEAD, draw_head, draw_row_fill};
use mdshared_ui::scroll::{draw_scrollbar, scrollbar};
use ropey::Rope;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};

/// A row of the list and the part of it that is filled.
const ROW: f32 = 25.0;
const ROW_H: f32 = 23.0;
/// Symbols kept of one document.
pub const SYMBOLS_MAX: usize = 20_000;

struct Job {
    key: (DocId, u64),
    text: Rope,
    flavor: Flavor,
}

struct Done {
    key: (DocId, u64),
    symbols: Vec<Symbol>,
    more: bool,
}

#[derive(Default)]
pub struct Outline {
    /// The left pane shows the function list, not the Files tree.
    pub on: bool,
    pub symbols: Vec<Symbol>,
    /// The document has more symbols than are kept.
    pub more: bool,
    /// The document's language has no rules for a list.
    pub no_rules: bool,
    /// The document and its revision last given to the worker, and whether its list has come.
    sent: Option<(DocId, u64)>,
    waiting: bool,
    pub scroll: f32,
    pub content: f32,
    pub view: f32,
    /// The row last marked as the caret's; when it changes the list scrolls to the new one.
    marked: Option<usize>,
    /// Counts every change of what the list shows.
    pub generation: u64,
    worker: Option<Arc<Link>>,
}

/// What the window and the worker share: the newest text to list, the newest list made, and
/// the flag that stops a list being made. A slot each, not a channel: only the newest of either
/// is wanted (and a channel of each type costs the exe some 15 KB).
#[derive(Default)]
struct Link {
    slots: Mutex<(Option<Job>, Option<Done>)>,
    ready: Condvar,
    cancel: AtomicBool,
}

impl Link {
    /// Gives the worker a text; the list it is making of an older one is given up.
    fn give(&self, job: Job) {
        let mut slots = self.slots.lock().unwrap();
        slots.0 = Some(job);
        self.cancel.store(true, Ordering::Relaxed);
        self.ready.notify_one();
    }

    fn take_done(&self) -> Option<Done> {
        self.slots.lock().unwrap().1.take()
    }
}

/// The worker: waits for a text, makes its list, and wakes the window.
fn work(link: Arc<Link>, waker: Waker) {
    loop {
        let job = {
            let mut slots = link.slots.lock().unwrap();
            loop {
                if let Some(job) = slots.0.take() {
                    // Under the lock, so a text given later still stops this one.
                    link.cancel.store(false, Ordering::Relaxed);
                    break job;
                }
                slots = link.ready.wait(slots).unwrap();
            }
        };
        let (symbols, more) = symbols(&job.text, job.flavor, SYMBOLS_MAX, &link.cancel);
        if link.cancel.load(Ordering::Relaxed) {
            continue;
        }
        link.slots.lock().unwrap().1 = Some(Done {
            key: job.key,
            symbols,
            more,
        });
        waker(Wake::Outline);
    }
}

impl Outline {
    /// The active document and what its list would be made with.
    fn wanted(docs: &Docs) -> Option<((DocId, u64), Option<Flavor>, Rope)> {
        let doc = docs.active_doc()?;
        let r = doc.ready()?;
        let flavor = r.language.and_then(|l| Flavor::for_language(l.name()));
        Some(((doc.id, r.revision), flavor, r.buffer.snapshot()))
    }

    fn clear(&mut self) {
        if !self.symbols.is_empty() || self.more {
            self.symbols.clear();
            self.more = false;
        }
        self.scroll = 0.0;
        self.marked = None;
        self.generation += 1;
    }

    /// Called every frame: gives the worker the active document's text when the list shows and
    /// the text is not the one it was last given.
    pub fn sync(&mut self, docs: &Docs, visible: bool, waker: &Waker) {
        if !self.on || !visible {
            return;
        }
        let Some((key, flavor, text)) = Self::wanted(docs) else {
            if self.sent.take().is_some() {
                self.no_rules = false;
                self.waiting = false;
                self.clear();
            }
            return;
        };
        if self.sent == Some(key) {
            return;
        }
        // Another document: its list does not start as the last one's.
        if self.sent.map(|s| s.0) != Some(key.0) {
            self.clear();
        }
        self.sent = Some(key);
        let Some(flavor) = flavor else {
            if !self.no_rules || !self.symbols.is_empty() {
                self.no_rules = true;
                self.waiting = false;
                self.clear();
            }
            return;
        };
        if self.no_rules {
            self.no_rules = false;
            self.generation += 1;
        }
        self.waiting = true;
        let link = self.worker.get_or_insert_with(|| {
            let link = Arc::new(Link::default());
            let (theirs, waker) = (link.clone(), waker.clone());
            std::thread::spawn(move || work(theirs, waker));
            link
        });
        link.give(Job { key, text, flavor });
    }

    /// Makes the list now, on this thread (`--shot` and tests).
    pub fn fill_now(&mut self, docs: &Docs) {
        self.clear();
        let Some((key, flavor, text)) = Self::wanted(docs) else {
            return;
        };
        self.sent = Some(key);
        self.no_rules = flavor.is_none();
        if let Some(flavor) = flavor {
            let cancel = AtomicBool::new(false);
            (self.symbols, self.more) = symbols(&text, flavor, SYMBOLS_MAX, &cancel);
        }
    }

    /// Takes in the lists the worker made. True when what shows changed.
    pub fn poll(&mut self) -> bool {
        let Some(done) = self.worker.as_ref().and_then(|w| w.take_done()) else {
            return false;
        };
        let mut changed = false;
        for d in [done] {
            if Some(d.key) != self.sent {
                continue;
            }
            changed |= self.waiting;
            self.waiting = false;
            if d.symbols != self.symbols || d.more != self.more {
                self.symbols = d.symbols;
                self.more = d.more;
                changed = true;
            }
        }
        if changed {
            self.generation += 1;
        }
        changed
    }

    /// The row of the symbol `line` is in: the last one that starts at or before it.
    pub fn row_of(&self, line: usize) -> Option<usize> {
        self.symbols
            .partition_point(|s| s.line <= line)
            .checked_sub(1)
    }

    pub fn scroll_by(&mut self, dy: f32) {
        self.scroll = (self.scroll + dy)
            .clamp(0.0, (self.content - self.view).max(0.0))
            .round();
    }
}

/// The list below the pane's title row.
pub fn list(r: Rect) -> Rect {
    Rect::new(r.x, r.y + HEAD, r.w, (r.h - HEAD).max(0.0))
}

/// The row of the active document's caret.
pub fn caret_row(docs: &Docs, outline: &Outline) -> Option<usize> {
    let r = docs.active_ready()?;
    outline.row_of(r.buffer.line_of(r.view.sel.caret))
}

pub fn draw(c: &mut Ctx, docs: &Docs, ui: &mut Ui, r: Rect) {
    let pal = *c.pal;
    c.p.fill(r, pal.pane);
    draw_head(c.p, &pal, r, "FUNCTIONS");
    // Back to the Files tree.
    let b = Rect::new(r.right() - 38.0, r.y + 6.0, 28.0, 28.0);
    if let Some(fill) = c.fill_for(Hit::OutlineFiles, false, false) {
        c.p.rbox(b, 5.0, fill);
    }
    c.p.icon(glyph::FOLDER, 14.0, b.cx(), b.cy(), pal.muted);
    c.hit(Hit::OutlineFiles, b);

    let dragging = matches!(ui.drag, Some(Drag::Splitter { .. }));
    let area = list(r);
    let current = caret_row(docs, &ui.outline);
    let out = &mut ui.outline;
    out.content = out.symbols.len() as f32 * ROW + 8.0;
    out.view = area.h;
    // The caret went into another symbol: its row comes into view.
    if current != out.marked {
        out.marked = current;
        if let Some(i) = current {
            let y = i as f32 * ROW;
            if y < out.scroll || y + ROW > out.scroll + area.h {
                out.scroll = (y - area.h / 3.0).round();
            }
        }
    }
    out.scroll = out.scroll.clamp(0.0, (out.content - area.h).max(0.0));
    let scroll = out.scroll;
    if out.symbols.is_empty() {
        let text = if docs.active_ready().is_none() {
            "No document"
        } else if out.no_rules {
            "No list for this kind of file"
        } else if out.waiting {
            "Reading\u{2026}"
        } else {
            "Nothing to list"
        };
        let at = Rect::new(r.x, area.y + 30.0, r.w, 20.0);
        c.p.text_center(Face::Ui, 13.0, at, text, pal.muted);
    }
    let quiet = mix(pal.text, pal.muted, 0.35);
    let old = c.p.clip(area);
    let first = (scroll / ROW).floor() as usize;
    for (i, s) in out.symbols.iter().enumerate().skip(first) {
        let y = area.y + i as f32 * ROW - scroll;
        if y > area.bottom() {
            break;
        }
        let rr = Rect::new(r.x + 6.0, y, r.w - 12.0, ROW_H);
        let hit = Hit::OutlineRow(i);
        let selected = current == Some(i);
        draw_row_fill(
            c.p,
            &pal,
            rr,
            c.hover == Some(hit),
            c.pressed == Some(hit),
            selected,
        );
        let x = r.x + 18.0 + f32::from(s.depth.min(5)) * 12.0;
        // What holds other things stands out from what is held.
        let (face, color) = match s.kind {
            Kind::Function => (Face::Ui, if selected { pal.text } else { quiet }),
            Kind::Heading | Kind::Type | Kind::Section => (Face::Semibold, pal.text),
        };
        let max = (rr.right() - x - 12.0).max(0.0);
        c.p.text(face, 12.5, x, rr.cy(), &s.name, color, max);
        c.hit(hit, rr);
    }
    c.p.restore(old);
    let thumb_hit = Hit::ScrollBar(Scroller::Files);
    if let Some((track, thumb)) = scrollbar(area, out.content, area.h, scroll) {
        let active = c.hover == Some(thumb_hit)
            || matches!(
                ui.drag,
                Some(Drag::Thumb {
                    target: Scroller::Files,
                    ..
                })
            );
        draw_scrollbar(c.p, &pal, track, thumb, active);
        c.hit(thumb_hit, track);
    }
    crate::panes::edge(c, r, dragging);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn symbol(line: usize, name: &str) -> Symbol {
        Symbol {
            line,
            depth: 0,
            kind: Kind::Function,
            name: name.into(),
        }
    }

    #[test]
    fn the_caret_is_in_the_last_symbol_at_or_before_it() {
        let mut o = Outline::default();
        assert_eq!(o.row_of(3), None);
        o.symbols = vec![symbol(2, "a"), symbol(10, "b"), symbol(11, "c")];
        assert_eq!(o.row_of(0), None);
        assert_eq!(o.row_of(2), Some(0));
        assert_eq!(o.row_of(9), Some(0));
        assert_eq!(o.row_of(10), Some(1));
        assert_eq!(o.row_of(500), Some(2));
    }

    #[test]
    fn the_worker_lists_the_newest_text() {
        let link = Arc::new(Link::default());
        let (woke_tx, woke) = std::sync::mpsc::channel();
        let waker: Waker = Arc::new(move |_| {
            let _ = woke_tx.send(());
        });
        let theirs = link.clone();
        std::thread::spawn(move || work(theirs, waker));
        for (n, text) in [
            "fn one() {}
",
            "fn one() {}
fn two() {}
",
        ]
        .into_iter()
        .enumerate()
        {
            link.give(Job {
                key: (7, n as u64),
                text: Rope::from_str(text),
                flavor: Flavor::Rust,
            });
        }
        // One list or two come, the last of the newest text.
        let mut last: Option<Done> = None;
        while last.as_ref().is_none_or(|d| d.key != (7, 1)) {
            woke.recv_timeout(std::time::Duration::from_secs(20))
                .unwrap();
            last = link.take_done().or(last);
        }
        let names: Vec<String> = last.unwrap().symbols.into_iter().map(|s| s.name).collect();
        assert_eq!(names, ["one", "two"]);
    }
}
