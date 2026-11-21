//! The preview pane: the active Markdown document drawn by mdview's renderer, beside the text.
//!
//! Two builds, one pane. By default (cargo feature `embedded-preview`) the drawing loop runs on a
//! thread of mdedit's own, with mdview's renderer built in, and formulas and diagrams go to the
//! `mdview-engine` process shipped beside mdedit (or, failing that, beside mdview): the pane is
//! always offered. Built with `--no-default-features`, the page is drawn by mdview itself:
//! `mdview --preview-server` runs without a window and answers over a pipe, and the pane is
//! offered only while mdview is installed. That build is kept to compare the two.
//!
//! The editor sends what changed (the text, the pane's size, the caret's line, a scroll) and
//! keeps a copy of the frame that the answers update. While a request is unanswered, a newer
//! text waits for the answer: the pane never queues more than one behind.

use crate::app::Wake;
use crate::docs::Waker;
use mdedit_highlight::DocId;
use mdshared_ui::gfx::Rect;
#[cfg(not(feature = "embedded-preview"))]
use mdview_preview::wire;
use mdview_preview::wire::{Request, Update};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

#[cfg(not(feature = "embedded-preview"))]
/// How long the server has to say who it is before an mdview too old for it is stopped.
const HELLO: Duration = Duration::from_secs(3);

/// What the threads behind the pane report.
enum Event {
    Update(Update),
    /// The server ended or could not start; the reason is shown in the pane.
    Ended(String),
}

/// What the pane showed last, to send only what changed.
#[derive(Default, PartialEq, Clone, Debug)]
pub(crate) struct Sent {
    doc: Option<DocId>,
    pub(crate) version: Option<(u64, u64)>,
    view: Option<(u32, u32, u32, bool)>,
    follow: Option<(u32, u32)>,
    /// The editor's top and its last screen ([`crate::view::source_ends`]) the page was last aligned
    /// to.
    align: Option<(f64, f64, f64)>,
    /// The gap last asked for: source lines (from 1) and its height in device pixels.
    gap: Option<(u32, u32, f32)>,
}

/// Which side synced scrolling follows: the one the reader moved last. The other side follows
/// and sends nothing back, so neither can set the other off again.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Lead {
    /// The editor scrolled (or was typed in, clicked, scrolled with its own wheel): the page
    /// puts the editor's top line at its top.
    #[default]
    Editor,
    /// The wheel turned over the page: the editor puts the page's top line at its top.
    Page,
}

/// One open preview pane.
pub struct Pane {
    requests: Sender<Request>,
    events: Receiver<Event>,
    /// The frame as the last update left it: device pixels, `Bitmap` layout.
    pub frame: mdshared_text::bitmap::Bitmap,
    /// Changes with every update applied, for the pane's repaint.
    pub generation: u64,
    /// The page's height and the row at the pane's top (device pixels).
    pub total: f32,
    pub top: f32,
    /// Why the pane shows no page, when it shows none.
    pub ended: Option<String>,
    pub(crate) sent: Sent,
    /// Requests sent and not answered yet.
    pub(crate) waiting: u32,
    /// A newer text is held back until the answer comes.
    pub(crate) held: bool,
    /// When the oldest unanswered request went, for the timings below.
    asked: Option<Instant>,
    /// Timings for `--trace` and the benchmarks: request to pixels, and the server's own work.
    pub last_round_trip: Option<Duration>,
    pub last_server: Option<Duration>,
    /// Every request-to-pixels time and the server's work for each frame, for the benchmarks.
    pub round_trips: Vec<Duration>,
    pub server_times: Vec<Duration>,
    /// When the pane was opened, and when its first page arrived.
    pub opened: Instant,
    pub first_page: Option<Duration>,
    pub kind: &'static str,
    /// Synced scrolling: who leads, and the source line at the page's top (from 1) as the last
    /// update gave it, until the editor has taken it.
    pub lead: Lead,
    /// The selection on the page, its rectangles `[x, y, w, h]` in page pixels, as the last
    /// update that changed it gave them; empty when nothing is selected.
    pub selection: Vec<[f32; 4]>,
    /// The page has the keyboard: a press on it gave it, a press in the text or any key the
    /// page does not use takes it back. Copy and Select all then act on the page.
    pub focused: bool,
    /// The answer to the last [`copy`](Self::copy), until the editor has taken it.
    copied: Option<mdview_preview::Copied>,
    /// A task box clicked on the page, until the editor has taken it: the document and version
    /// of the text it is about, the byte of its mark, and whether the click checks it.
    task: Option<(DocId, (u64, u64), usize, bool)>,
    /// The cursor for the place the pointer last rested on, as the page last answered.
    pub pointer: mdview_preview::Pointer,
    /// A place to ask the cursor for once nothing is awaited (only the latest is kept).
    hover_due: Option<(f32, f32)>,
    /// A row to scroll the page to (device pixels) once nothing is awaited, as a scrollbar's
    /// thumb moves faster than the page answers (only the latest is kept).
    scroll_due: Option<f32>,
    page_line: Option<f32>,
    /// Visual editing: the page row where the room for the open block starts, as the last
    /// update gave it, and the answer to the last [`block_at`](Self::block_at) until taken.
    pub gap_y: Option<f32>,
    /// Answers still due before the last gap request has been taken: a `gap_y` in an update
    /// before that is about the gap before it.
    gap_due: u32,
    block: Option<Option<mdview_preview::wire::BlockLines>>,
}

/// The pane needs no mdview: this build draws it itself.
pub const STANDALONE: bool = cfg!(feature = "embedded-preview");

/// The mdview installed, if any.
pub fn viewer() -> Option<PathBuf> {
    mdshared_app::sister::find(crate::link::VIEWER)
}

/// The engine for formulas and diagrams: the one shipped with mdedit, else the one beside the
/// installed mdview. None leaves the renderer to its own search (and the formulas as source).
#[cfg_attr(not(feature = "embedded-preview"), allow(dead_code))]
fn engine(viewer: Option<&Path>) -> Option<PathBuf> {
    let exe = std::env::current_exe().ok();
    own_engines(exe.as_deref())
        .into_iter()
        .chain(viewer.and_then(|v| Some(v.parent()?.join(engine_file()))))
        .find(|p| p.is_file())
}

fn engine_file() -> String {
    format!("mdview-engine{}", std::env::consts::EXE_SUFFIX)
}

/// Where mdedit's own engine is, in the order looked at: beside the executable (Windows, the
/// macOS bundle, the AppImage), then `../lib/mdedit/` (the Linux packages and the tarball, which
/// keep it off `bin/`, where mdview's packages put theirs).
fn own_engines(exe: Option<&Path>) -> Vec<PathBuf> {
    let Some(dir) = exe.and_then(Path::parent) else {
        return Vec::new();
    };
    let mut at = vec![dir.join(engine_file())];
    if let Some(prefix) = dir.parent().filter(|_| cfg!(target_os = "linux")) {
        at.push(prefix.join("lib/mdedit").join(engine_file()));
    }
    at
}

impl Pane {
    /// Opens a pane drawn by `viewer` (the renderer build) or on a thread of this process (the
    /// embedded build). `wake` is called whenever an answer is waiting.
    pub fn open(viewer: Option<&Path>, wake: Waker) -> Pane {
        let (requests, rx) = mpsc::channel::<Request>();
        let (tx, events) = mpsc::channel::<Event>();
        let opened = Instant::now();
        let kind = start(viewer, rx, tx, wake);
        Pane {
            requests,
            events,
            frame: mdshared_text::bitmap::Bitmap::new(1, 1),
            generation: 0,
            total: 0.0,
            top: 0.0,
            ended: None,
            sent: Sent::default(),
            waiting: 0,
            held: false,
            asked: None,
            last_round_trip: None,
            last_server: None,
            round_trips: Vec::new(),
            server_times: Vec::new(),
            opened,
            first_page: None,
            kind,
            lead: Lead::Editor,
            page_line: None,
            selection: Vec::new(),
            focused: false,
            copied: None,
            task: None,
            pointer: mdview_preview::Pointer::Arrow,
            hover_due: None,
            scroll_due: None,
            gap_y: None,
            gap_due: 0,
            block: None,
        }
    }

    fn send(&mut self, req: Request) {
        if self.requests.send(req).is_ok() {
            self.waiting += 1;
            self.asked.get_or_insert_with(Instant::now);
        }
    }

    /// Sends the pane's size and theme when they changed. `width`, `height` in device pixels.
    pub fn set_view(&mut self, width: u32, height: u32, scale: f32, dark: bool) {
        let v = (width.max(1), height.max(1), scale.to_bits(), dark);
        if self.sent.view != Some(v) {
            self.sent.view = Some(v);
            self.send(Request::View {
                width: v.0,
                height: v.1,
                scale,
                dark,
            });
        }
    }

    /// Sends the document's text when it is another document or another version of it, unless
    /// an answer is still awaited (then it goes with the answer). `text` is called only to send.
    pub fn set_text(
        &mut self,
        doc: DocId,
        version: (u64, u64),
        base_dir: Option<PathBuf>,
        text: impl FnOnce() -> String,
    ) {
        if self.sent.doc == Some(doc) && self.sent.version == Some(version) {
            self.held = false;
            return;
        }
        if self.waiting > 0 && self.sent.doc == Some(doc) {
            self.held = true;
            return;
        }
        self.held = false;
        if self.sent.doc != Some(doc) {
            // Another document starts at its caret and its top, not where the last one was.
            self.sent.follow = None;
            self.sent.align = None;
            self.lead = Lead::Editor;
        }
        self.sent.doc = Some(doc);
        self.sent.version = Some(version);
        self.send(Request::Text {
            text: text(),
            base_dir,
        });
    }

    /// Sends the caret's line (from 1) and byte column (from 0) when it moved.
    pub fn follow(&mut self, line: u32, column: u32) {
        if self.sent.follow != Some((line, column)) {
            self.sent.follow = Some((line, column));
            self.send(Request::Follow { line, column });
        }
    }

    /// Scrolls the page `by` device pixels; the page leads synced scrolling from here.
    pub fn scroll(&mut self, by: f32) {
        if by != 0.0 {
            self.lead = Lead::Page;
            self.send(Request::Scroll { by });
        }
    }

    /// Scrolls the page to the row `top` (device pixels), at once when nothing is awaited, else
    /// once the answers are in, so a thumb dragged fast does not queue requests that each
    /// measure from a row the page has not reached yet.
    pub fn scroll_to(&mut self, top: f32) {
        if self.waiting == 0 {
            self.scroll_due = None;
            self.scroll(top - self.top);
        } else {
            self.scroll_due = Some(top);
        }
    }

    /// What a scrollbar on the page measures, in logical px at `scale`: the page's height, the
    /// pane's, and the row at the pane's top.
    pub fn extent(&self, scale: f32) -> (f32, f32, f32) {
        (
            self.total / scale,
            self.frame.height as f32 / scale,
            self.top / scale,
        )
    }

    /// The reader turned to the editor (a key, a click, its own wheel): it leads again, and the
    /// page is aligned to it at the next sync.
    pub fn editor_leads(&mut self) {
        if self.lead != Lead::Editor {
            self.lead = Lead::Editor;
            self.sent.align = None;
        }
    }

    /// While the editor leads: sends its top and its last screen of scrolling (source lines
    /// from 0, fractional; [`crate::view::source_ends`]) when they moved.
    pub fn align(&mut self, top: f64, (tail, end): (f64, f64)) {
        let now = (top, tail, end);
        let moved = |a: (f64, f64, f64)| {
            (a.0 - now.0).abs() > 1e-3 || (a.1 - now.1).abs() > 1e-3 || (a.2 - now.2).abs() > 1e-3
        };
        if self.lead == Lead::Editor && self.sent.align.is_none_or(moved) {
            self.sent.align = Some(now);
            self.send(Request::Align {
                line: (top + 1.0) as f32,
                tail: (tail + 1.0) as f32,
                end: (end + 1.0) as f32,
            });
        }
    }

    /// The main button went down on the page at `x, y` (device pixels from the page's top
    /// left): the page takes the keyboard and a selection starts there.
    pub fn press(&mut self, x: f32, y: f32, clicks: u32) {
        self.focused = true;
        self.send(Request::Press { x, y, clicks });
    }

    /// The pointer moved with the button down.
    pub fn drag(&mut self, x: f32, y: f32) {
        self.send(Request::Drag { x, y });
    }

    /// The page takes the keyboard and keeps its selection (a right-click on it).
    pub fn focus(&mut self) {
        self.focused = true;
    }

    pub fn release(&mut self) {
        self.send(Request::Release);
    }

    pub fn select_all(&mut self) {
        self.send(Request::SelectAll);
    }

    /// The page gives the keyboard back, its selection cleared.
    pub fn blur(&mut self) {
        if self.focused {
            self.focused = false;
            self.send(Request::Unselect);
        }
    }

    /// Asks for the selection's text; [`take_copied`](Self::take_copied) has it once answered.
    pub fn copy(&mut self) {
        self.send(Request::Copy);
    }

    pub fn take_copied(&mut self) -> Option<mdview_preview::Copied> {
        self.copied.take()
    }

    /// The pointer rests at `x, y` (device pixels from the page's top left): asks the page for
    /// the cursor there, at once when nothing is awaited, else once the answers are in, so a
    /// moving pointer does not queue requests. [`pointer`](Self::pointer) has the answer.
    pub fn hover(&mut self, x: f32, y: f32) {
        if self.waiting == 0 {
            self.hover_due = None;
            self.send(Request::Hover { x, y });
        } else {
            self.hover_due = Some((x, y));
        }
    }

    /// Visual editing: asks which top-level block is at `x, y` (device pixels from the page's
    /// top left); [`take_block`](Self::take_block) has the answer.
    pub fn block_at(&mut self, x: f32, y: f32) {
        self.send(Request::BlockAt { x, y });
    }

    /// Visual editing: asks which top-level block holds source line `line` (from 1).
    pub fn block_of_line(&mut self, line: u32) {
        self.send(Request::BlockOfLine { line });
    }

    /// The answer to [`block_at`](Self::block_at) or [`block_of_line`](Self::block_of_line):
    /// `Some(None)` when no block is there.
    pub fn take_block(&mut self) -> Option<Option<mdview_preview::wire::BlockLines>> {
        self.block.take()
    }

    /// Visual editing: leaves source lines `first` to `last` (from 1) out of the page with
    /// room `height` device pixels tall, or puts them back with `None`. Sent when it changed.
    pub fn set_gap(&mut self, gap: Option<(u32, u32, f32)>) {
        if self.sent.gap != gap {
            self.sent.gap = gap;
            if gap.is_none() {
                self.gap_y = None;
            }
            self.send(Request::Gap { gap });
            self.gap_due = self.waiting;
        }
    }

    /// Visual editing: the page forgets the caret line it followed, so a new text or size
    /// leaves it where the reader scrolled it. Following starts again with the next line sent.
    pub fn unfollow(&mut self) {
        self.sent.follow = None;
        self.send(Request::Unfollow);
    }

    /// The gap last asked for: its source lines (from 1) and height (device pixels).
    pub fn sent_gap(&self) -> Option<((u32, u32), f32)> {
        self.sent.gap.map(|(a, b, h)| ((a, b), h))
    }

    /// Nothing is awaited from the page, and no cursor question is held back.
    #[cfg(test)]
    pub fn idle(&self) -> bool {
        self.waiting == 0 && self.hover_due.is_none() && self.scroll_due.is_none()
    }

    /// The task box clicked on the page: the document and version of the text it is about, the
    /// byte offset of its mark, and whether the click checks it.
    pub fn take_task(&mut self) -> Option<(DocId, (u64, u64), usize, bool)> {
        self.task.take()
    }

    /// While the page leads: the source line at its top (from 0, fractional) from an update the
    /// editor has not followed yet.
    pub fn take_page_top(&mut self) -> Option<f64> {
        let line = self.page_line.take()?;
        (self.lead == Lead::Page).then_some(line as f64 - 1.0)
    }

    /// Takes the answers in. True when the frame changed or the pane ended.
    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        while let Ok(event) = self.events.try_recv() {
            match event {
                Event::Update(mut u) => {
                    self.waiting = self.waiting.saturating_sub(u.answers);
                    self.gap_due = self.gap_due.saturating_sub(u.answers);
                    if let Some(rects) = u.selection.take() {
                        self.selection = rects;
                        // The pane is painted again by its generation.
                        self.generation += 1;
                        changed = true;
                    }
                    if let Some(c) = u.copied.take() {
                        self.copied = Some(c);
                        changed = true;
                    }
                    // About the last text sent: the server took every request in order.
                    if let Some(p) = u.pointer.take()
                        && p != self.pointer
                    {
                        self.pointer = p;
                        changed = true;
                    }
                    if let Some((at, checked)) = u.task.take()
                        && let (Some(doc), Some(version)) = (self.sent.doc, self.sent.version)
                    {
                        self.task = Some((doc, version, at as usize, checked));
                        changed = true;
                    }
                    if self.waiting == 0
                        && let Some(at) = self.asked.take()
                    {
                        self.last_round_trip = Some(at.elapsed());
                        self.round_trips.push(at.elapsed());
                    }
                    if let Some(b) = u.block.take() {
                        self.block = Some(b);
                        changed = true;
                    }
                    if self.sent.gap.is_some() && self.gap_due == 0 && u.gap_y != self.gap_y {
                        self.gap_y = u.gap_y;
                        changed = true;
                    }
                    self.last_server = Some(Duration::from_micros(u.micros as u64));
                    if u.is_ack() {
                        continue;
                    }
                    self.server_times
                        .push(Duration::from_micros(u.micros as u64));
                    self.first_page.get_or_insert_with(|| self.opened.elapsed());
                    let mut size = (self.frame.width, self.frame.height);
                    if u.apply(&mut self.frame.pixels, &mut size) {
                        (self.frame.width, self.frame.height) = size;
                        self.top = u.top;
                        self.total = u.total;
                        self.page_line = Some(u.line);
                        self.generation += 1;
                        changed = true;
                    } else {
                        // The copy was lost (a band for another size): ask for a whole frame.
                        self.sent.view = None;
                    }
                }
                Event::Ended(why) => {
                    self.ended = Some(why);
                    self.waiting = 0;
                    self.generation += 1;
                    changed = true;
                }
            }
        }
        if self.waiting == 0
            && self.ended.is_none()
            && let Some((x, y)) = self.hover_due.take()
        {
            self.send(Request::Hover { x, y });
        }
        if self.waiting == 0
            && self.ended.is_none()
            && let Some(top) = self.scroll_due.take()
        {
            self.scroll(top - self.top);
        }
        changed
    }
}

impl Drop for Pane {
    fn drop(&mut self) {
        let _ = self.requests.send(Request::Quit);
    }
}

/// The embedded build: the loop runs on a thread here; formulas and diagrams go to the engine
/// installed beside mdview.
#[cfg(feature = "embedded-preview")]
fn start(
    viewer: Option<&Path>,
    rx: Receiver<Request>,
    tx: Sender<Event>,
    wake: Waker,
) -> &'static str {
    if let Some(engine) = engine(viewer) {
        mdview_render::set_engine_path(engine);
    }
    let (tx2, wake2) = (tx.clone(), wake.clone());
    let spawned = std::thread::Builder::new()
        .name("preview".into())
        .spawn(move || {
            mdview_preview::serve(rx, |u| {
                let ok = tx.send(Event::Update(u)).is_ok();
                wake(Wake::Preview);
                ok
            });
        });
    if let Err(e) = spawned {
        let _ = tx2.send(Event::Ended(format!("The preview could not start: {e}")));
        wake2(Wake::Preview);
    }
    "embedded"
}

/// The renderer build: `mdview --preview-server`, a writer thread for its input and a reader
/// thread for its answers.
#[cfg(not(feature = "embedded-preview"))]
fn start(
    viewer: Option<&Path>,
    rx: Receiver<Request>,
    tx: Sender<Event>,
    wake: Waker,
) -> &'static str {
    use std::io::{BufReader, BufWriter};
    use std::process::{Command, Stdio};
    let ended = |tx: &Sender<Event>, wake: &Waker, why: String| {
        let _ = tx.send(Event::Ended(why));
        wake(Wake::Preview);
    };
    let Some(viewer) = viewer else {
        let _ = tx.send(Event::Ended("mdview is not installed.".into()));
        wake(Wake::Preview);
        return "mdview";
    };
    let child = Command::new(viewer)
        .arg("--preview-server")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();
    let mut child = match child {
        Ok(c) => c,
        Err(e) => {
            ended(&tx, &wake, format!("mdview could not be started: {e}"));
            return "mdview";
        }
    };
    let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
        ended(&tx, &wake, "mdview could not be started".into());
        return "mdview";
    };
    let child = std::sync::Arc::new(std::sync::Mutex::new(child));
    // The writer: requests in order, then Quit closes the input and the server ends.
    std::thread::Builder::new()
        .name("preview-in".into())
        .spawn(move || {
            let mut out = BufWriter::with_capacity(1 << 16, stdin);
            while let Ok(req) = rx.recv() {
                let quit = req == Request::Quit;
                if wire::write_request(&mut out, &req).is_err() || quit {
                    break;
                }
            }
        })
        .ok();
    // The reader: the greeting within HELLO, or the mdview is too old and is stopped.
    let (hello_tx, hello_rx) = mpsc::channel::<bool>();
    let watch = child.clone();
    let (tx2, wake2) = (tx.clone(), wake.clone());
    std::thread::Builder::new()
        .name("preview-wait".into())
        .spawn(move || {
            if hello_rx.recv_timeout(HELLO) != Ok(true) {
                if let Ok(mut c) = watch.lock() {
                    let _ = c.kill();
                }
                let _ = tx2.send(Event::Ended(
                    "This mdview cannot draw a preview here. Update mdview.".into(),
                ));
                wake2(Wake::Preview);
            }
        })
        .ok();
    std::thread::Builder::new()
        .name("preview-out".into())
        .spawn(move || {
            let mut input = BufReader::with_capacity(1 << 20, stdout);
            let ok = wire::read_magic(&mut input);
            let _ = hello_tx.send(ok);
            if !ok {
                return;
            }
            loop {
                match wire::read_update(&mut input) {
                    Ok(Some(u)) => {
                        if tx.send(Event::Update(u)).is_err() {
                            break;
                        }
                        wake(Wake::Preview);
                    }
                    Ok(None) | Err(_) => {
                        ended(&tx, &wake, "mdview's preview stopped.".into());
                        break;
                    }
                }
            }
            if let Ok(mut c) = child.lock() {
                let _ = c.wait();
            }
        })
        .ok();
    "mdview"
}

/// Draws the pane: the line that parts it from the text, then the page, or why there is none.
/// `bar` is `Some(active)` when the page has a scrollbar (visual editing), `active` while the
/// pointer is on it or its thumb is held.
pub fn draw(
    c: &mut crate::chrome::Ctx,
    pane: Option<&Pane>,
    docs: &crate::docs::Docs,
    rect: Rect,
    bar: Option<bool>,
) {
    use mdshared_ui::gfx::Face;
    let pal = c.pal;
    c.p.fill(rect, pal.page);
    let px = c.p.px();
    c.p.fill(Rect::new(rect.x, rect.y, px, rect.h), pal.line);
    let Some(pane) = pane else {
        return;
    };
    let shows = docs.active_doc().is_some_and(crate::app::previewable);
    let note = |c: &mut crate::chrome::Ctx, text: &str| {
        let r = Rect::new(rect.x + 16.0, rect.y + 16.0, rect.w - 32.0, 24.0);
        c.p.text(Face::Ui, 13.0, r.x, r.cy(), text, pal.muted, r.w);
    };
    if let Some(why) = &pane.ended {
        note(c, why);
        return;
    }
    if !shows {
        note(c, "No preview: this is not a Markdown document.");
        return;
    }
    if pane.frame.width <= 1 {
        return;
    }
    let (x0, y0, x1, y1) = c.p.dev_rect(rect);
    let h = pane.frame.height as i32;
    c.p.blit(&pane.frame, 0, h, x0 + 1, y0);
    // The selection over the page, as mdview blends it.
    let alpha = if pal.dark { 0.42 } else { 0.30 };
    let color = pal.selection;
    for &[x, y, w, h] in &pane.selection {
        let left = x0 + 1 + x.round() as i32;
        let top = y0 + (y - pane.top).round() as i32;
        let (a, b) = (left.max(x0 + 1), (left + w.round() as i32).min(x1));
        let (t, u) = (top.max(y0), (top + h.round() as i32).min(y1));
        if a < b && t < u {
            c.p.fill_dev(a, t, b, u, color, alpha);
        }
    }
    if let Some(active) = bar {
        let (content, view, top) = pane.extent(1.0 / px);
        if let Some((track, thumb)) = mdshared_ui::scroll::scrollbar(rect, content, view, top) {
            mdshared_ui::scroll::draw_scrollbar(c.p, pal, track, thumb, active);
            c.hit(crate::ui::Hit::ScrollBar(crate::ui::Scroller::Page), track);
        }
    }
}

#[cfg(test)]
mod engine_tests {
    use super::*;

    #[test]
    fn the_own_engine_is_looked_for_beside_the_exe_then_in_lib_mdedit() {
        let file = engine_file();
        let at = own_engines(Some(Path::new("/usr/bin/mdedit")));
        assert_eq!(at[0], Path::new("/usr/bin").join(&file));
        if cfg!(target_os = "linux") {
            assert_eq!(at[1], Path::new("/usr/lib/mdedit").join(&file));
            assert_eq!(at.len(), 2);
        } else {
            assert_eq!(at.len(), 1);
        }
        assert!(own_engines(None).is_empty());
    }
}
