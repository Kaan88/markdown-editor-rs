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
    /// The editor's top (a source line from 0, fractional) the page was last aligned to.
    align: Option<f64>,
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
    page_line: Option<f32>,
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

    /// The reader turned to the editor (a key, a click, its own wheel): it leads again, and the
    /// page is aligned to it at the next sync.
    pub fn editor_leads(&mut self) {
        if self.lead != Lead::Editor {
            self.lead = Lead::Editor;
            self.sent.align = None;
        }
    }

    /// While the editor leads: sends its top (a source line from 0, fractional) when it moved.
    pub fn align(&mut self, top: f64) {
        if self.lead == Lead::Editor && self.sent.align.is_none_or(|a| (a - top).abs() > 1e-3) {
            self.sent.align = Some(top);
            self.send(Request::Align {
                line: (top + 1.0) as f32,
            });
        }
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
                Event::Update(u) => {
                    self.waiting = self.waiting.saturating_sub(u.answers);
                    if self.waiting == 0
                        && let Some(at) = self.asked.take()
                    {
                        self.last_round_trip = Some(at.elapsed());
                        self.round_trips.push(at.elapsed());
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
pub fn draw(c: &mut crate::chrome::Ctx, pane: Option<&Pane>, docs: &crate::docs::Docs, rect: Rect) {
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
    let (x0, y0, _, _) = c.p.dev_rect(rect);
    let h = pane.frame.height as i32;
    c.p.blit(&pane.frame, 0, h, x0 + 1, y0);
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
