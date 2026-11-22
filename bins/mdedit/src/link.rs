//! Previewing in mdview: the documents whose file the viewer shows, and what keeps the viewer
//! scrolled to their carets and, while "Live preview while typing" is on, showing their text as it
//! is typed. docs/DESIGN.md, "The hand-off with mdview", has the whole design.
//!
//! A document is linked when "Preview in mdview" opened its file in the viewer. While any is,
//! each pass of the event loop looks at the active document ([`Docs::link_poll`]); when its caret
//! is on another line than the viewer was last told, or (live) its text is not the text the viewer
//! was last handed, at most ten times a second, an [`Update`] is put in the [`Courier`]'s one slot,
//! newest wins. The update carries a snapshot of the text (a rope clone, O(1)); the courier is a
//! thread that builds the string and sends it to the viewer's window, so neither that nor a viewer
//! that is slow or stuck (a send waits up to 1.5 s for the answer) holds the window up, and a slow
//! viewer only ever delays: what waits in the slot is replaced, never queued. Nothing runs while
//! nothing is linked: no timer, no thread.
//!
//! The text goes only when it differs from what the courier last sent for that file, and never for
//! a document over [`MAX_TEXT`], where the viewer keeps showing the file (the save-driven preview).
//! A send with text that nobody answers does not end the link: the viewer may be an older one that
//! takes no text, so the courier sends the line alone and tries text again after [`TEXT_RETRY`].
//!
//! A link ends when its tab closes or its file is renamed, when the user stops it, when the viewer
//! is gone (a send that nobody answers more than [`GRACE`] after the viewer was started), and with
//! the process. Ending one, but the viewer being gone, tells the viewer to go back to the file
//! (`live_off`) if it was sent text.
//!
//! A document that has no file yet is linked too ([`Target::Untitled`]): the viewer has nothing
//! to read, so it is shown by the first update, which carries the text and asks for a tab
//! (`open`), and its text goes at every change whatever the setting says. When it is saved the
//! link becomes the file's and the viewer is told which file its tab now is. A viewer too old to
//! know such documents answers so ([`Delivery::Older`]); the link ends and a toast says why.

use crate::app::Wake;
use crate::docs::{Docs, Waker};
use mdedit_highlight::DocId;
use mdshared_app::sister::{self, Delivery, MAX_TEXT, Rect, Request};
use ropey::Rope;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// The other application, and this one as it names itself in a request.
pub const VIEWER: &str = "mdview";
pub const FROM: &str = "mdedit";
/// Between two updates: at most ten a second.
pub const INTERVAL: Duration = Duration::from_millis(100);
/// For this long after the viewer was started, an update nobody answers is tried again: the
/// viewer is still starting. After it, the viewer is gone and the link ends.
pub const GRACE: Duration = Duration::from_secs(5);
/// How long text is left out after a send with text got no answer.
pub const TEXT_RETRY: Duration = Duration::from_secs(5);
/// Between two tries during the grace time.
const RETRY: Duration = Duration::from_millis(250);
/// How far along its line the caret moves before the viewer is told again, in bytes: a line
/// that is longer than this wraps to several rows in the viewer, and the viewer shows the row
/// the caret is in. Moving inside a shorter stretch sends nothing.
pub const COLUMN_STEP: u32 = 32;
/// How long the exit waits for the courier to tell the viewer the links are over.
const FINISH_WAIT: Duration = Duration::from_millis(300);
/// After a check finds the viewer running, how long until it is asked once more: a viewer whose
/// window was just closed still answers while its process ends, and closing it is what handed
/// this window the focus that asked.
const RECHECK: Duration = Duration::from_secs(1);

/// A document's text as it is: the load it came from and the edits since.
pub type Version = (u64, u64);

/// What the viewer shows for a linked document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    /// A file: the viewer reads it, and is sent the text only while that differs from it.
    File(PathBuf),
    /// A document that has no file yet: the viewer has only what it is sent, under `title`.
    /// `doc` is the name the document goes by between the two applications ([`doc_name`]).
    Untitled { doc: String, title: String },
}

impl Target {
    pub fn is_untitled(&self) -> bool {
        matches!(self, Target::Untitled { .. })
    }

    /// A request about this document, with nothing asked yet.
    fn request(&self) -> Request {
        match self {
            Target::File(path) => Request::new(path),
            Target::Untitled { doc, title } => Request::document(doc, title),
        }
    }
}

impl From<PathBuf> for Target {
    fn from(path: PathBuf) -> Target {
        Target::File(path)
    }
}

/// The name a document without a file goes by in requests: this process and the document's
/// number in it, so it is the same for as long as the tab lives and no other tab's.
pub fn doc_name(id: DocId) -> String {
    format!("{FROM}-{}-{id}", std::process::id())
}

/// The first update of a document without a file asks the viewer to show it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Opening {
    /// Where the viewer's window is to go ("Preview side by side").
    pub place: Option<Rect>,
}

/// What to tell the viewer: this document is at this line, and (live) this is its text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Update {
    pub target: Target,
    pub line: u32,
    /// The caret's byte in that line, from 1: where along a line that wraps the viewer is to
    /// be. A viewer that predates it shows the line's beginning.
    pub column: u32,
    /// Until when a send nobody answers is tried again.
    pub retry_until: Instant,
    /// The text, when the preview is live: its version and a snapshot. `None` when it is not.
    pub text: Option<(Version, Rope)>,
    /// The viewer is to show the document: only a document without a file is opened this way.
    pub open: Option<Opening>,
}

impl Update {
    /// This update taking the place of `older`, which was not sent: a request of the older one
    /// to show the document (and where) is kept, or a document without a file would never be
    /// opened when it changed while the viewer was starting.
    fn over(mut self, older: Option<Update>) -> Update {
        if self.open.is_none()
            && let Some(o) = older.filter(|o| o.target == self.target)
        {
            self.open = o.open;
        }
        self
    }

    /// The live update: no raising, no opening, no placing. The one that opens a document
    /// without a file does all three.
    fn request(&self) -> Request {
        Request {
            follow: self.open.is_none(),
            open: self.open.is_some(),
            place: self.open.and_then(|o| o.place),
            line: Some(self.line),
            column: Some(self.column),
            from: Some(FROM.into()),
            ..self.target.request()
        }
    }
}

/// The request that ends the viewer's live text for `target`.
fn off_request(target: &Target) -> Request {
    Request {
        live_off: true,
        from: Some(FROM.into()),
        ..target.request()
    }
}

/// The request that tells the viewer the document it knew as `doc` is the file `path` now.
fn saved_request(doc: &str, path: &Path) -> Request {
    Request {
        follow: true,
        doc: Some(doc.to_string()),
        from: Some(FROM.into()),
        ..Request::new(path)
    }
}

/// What a pass of the loop knows of the document that has the keyboard.
#[derive(Clone, Copy, Debug)]
pub struct Active {
    pub id: DocId,
    /// The caret's line, from 1.
    pub line: u32,
    /// The caret's byte in that line, from 1.
    pub column: u32,
    pub version: Version,
    /// Its text is to be sent: the setting is on, it is within the cap, and the document differs
    /// from its saved state (otherwise the viewer shows the file, and is told to go back to it
    /// once the text is the file's again). A document without a file is always sent while it is
    /// within the cap: the viewer has nothing else to show.
    pub live: bool,
}

struct Linked {
    id: DocId,
    /// The file the viewer was given; a document that is saved under another name is not it. Or
    /// the document without a file, until it is saved.
    target: Target,
    /// The viewer has not been asked to show it yet (a document without a file).
    open: Option<Opening>,
    /// The line the viewer was last told, and how many [`COLUMN_STEP`]s along it.
    sent: Option<(u32, u32)>,
    /// The version of the text last handed to the courier; `None` when none was, or not live.
    sent_version: Option<Version>,
    retry_until: Instant,
}

/// What a pass of the loop has to do.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Poll {
    /// An update to send now. Its text, if live, is for the caller to attach.
    pub send: Option<Update>,
    /// When to look again: an update is held back until then.
    pub due: Option<Instant>,
}

#[derive(Default)]
pub struct Links {
    linked: Vec<Linked>,
    /// The tab that had the keyboard at the last pass.
    active: Option<DocId>,
    /// When the viewer was last told something.
    sent_at: Option<Instant>,
    /// Exists while something is linked.
    courier: Option<Courier>,
    /// The windows were put side by side and have not been sent back: the menu's check mark.
    pub side_by_side: bool,
    /// A link ended since the last [`take_ended`](Self::take_ended), however it ended.
    ended: bool,
}

impl Links {
    pub fn contains(&self, id: DocId) -> bool {
        self.linked.iter().any(|l| l.id == id)
    }

    pub fn is_empty(&self) -> bool {
        self.linked.is_empty()
    }

    /// Links document `id`, whose file `path` the viewer was just asked to show at `line`. The
    /// viewer reads the file, so `version` is the text's when the viewer has it (the shell passes
    /// `None`: a document that is not modified is never sent, and a modified one goes at the next
    /// pass).
    pub fn link(
        &mut self,
        id: DocId,
        target: Target,
        line: Option<u32>,
        version: Option<Version>,
        now: Instant,
    ) {
        self.linked.retain(|l| l.id != id);
        self.linked.push(Linked {
            id,
            target,
            open: None,
            // The opening names the line alone: a caret further along it is told next.
            sent: line.map(|line| (line, 0)),
            sent_version: version,
            retry_until: now + GRACE,
        });
        self.active = Some(id);
        self.sent_at = Some(now);
    }

    /// Links document `id`, which has no file: the next pass sends the viewer its text and asks
    /// it to show a tab named `title`, at `opening.place` if that is given. The viewer was
    /// started just now if none ran, so the update is tried again for [`GRACE`].
    pub fn link_untitled(&mut self, id: DocId, title: String, opening: Opening, now: Instant) {
        self.linked.retain(|l| l.id != id);
        self.linked.push(Linked {
            id,
            target: Target::Untitled {
                doc: doc_name(id),
                title,
            },
            open: Some(opening),
            sent: None,
            sent_version: None,
            retry_until: now + GRACE,
        });
        self.active = Some(id);
    }

    /// Document `id`, linked while it had no file, was saved as `path`: the link is the file's
    /// from now on, and the viewer is told that its tab is that file's.
    fn saved(&mut self, id: DocId, path: PathBuf) {
        let Some(link) = self.linked.iter_mut().find(|l| l.id == id) else {
            return;
        };
        let was = std::mem::replace(&mut link.target, Target::File(path.clone()));
        (link.open, link.sent, link.sent_version) = (None, None, None);
        if let Some(courier) = &self.courier {
            courier.saved(was, path);
        }
    }

    /// Whether the viewer said, since the last call, that it does not know documents without a
    /// file (an older build); the links of such documents are over then.
    pub fn take_older(&mut self) -> bool {
        let older = self.courier.as_ref().is_some_and(Courier::take_older);
        if older {
            let before = self.linked.len();
            self.linked.retain(|l| !l.target.is_untitled());
            self.ended |= self.linked.len() != before;
            if self.linked.is_empty() {
                self.courier = None;
            }
        }
        older
    }

    /// Starts what sends the updates; `waker` is called when the viewer turns out to be gone.
    pub fn start(&mut self, waker: Waker) {
        self.courier.get_or_insert_with(|| {
            Courier::spawn(
                |request| sister::deliver(VIEWER, request),
                || sister::alive(VIEWER),
                Timing {
                    retry: RETRY,
                    text_retry: TEXT_RETRY,
                    recheck: RECHECK,
                },
                move || waker(Wake::Link),
            )
        });
    }

    pub fn unlink(&mut self, id: DocId) {
        self.keep(|l| l.id != id);
    }

    /// The viewer closed its tab of the file `path`, or of the document without a file it knows
    /// as `doc`: that link is over. (It is told to go back to the file if it was sent text, which
    /// for a tab it no longer has does nothing.)
    pub fn closed(&mut self, path: &Path, doc: Option<&str>) {
        self.keep(|l| match &l.target {
            Target::File(file) => path.as_os_str().is_empty() || file != path,
            Target::Untitled { doc: name, .. } => doc != Some(name.as_str()),
        });
    }

    /// Asks the courier whether the viewer still runs, for a viewer closed while nothing was
    /// sent to it: when it does not, the links end as for a send nobody answers. Called at moments
    /// the user acts (the window gains focus, a menu opens), never on a timer; nothing is asked
    /// while nothing is linked, or while a viewer started for a link may still be starting.
    /// Whether it was asked.
    pub fn check(&self, now: Instant) -> bool {
        let Some(courier) = &self.courier else {
            return false;
        };
        if self.linked.iter().any(|l| now < l.retry_until) {
            return false;
        }
        courier.check();
        true
    }

    /// Ends the links `keep` says no to, telling the viewer to go back to their files.
    fn keep(&mut self, mut keep: impl FnMut(&Linked) -> bool) {
        let courier = &self.courier;
        self.linked.retain(|l| {
            let kept = keep(l);
            if let Some(c) = courier.as_ref().filter(|_| !kept) {
                c.off(&l.target);
            }
            if !kept {
                self.ended = true;
            }
            kept
        });
        if self.linked.is_empty() {
            self.courier = None;
        }
    }

    /// Whether a link ended since the last call, in any way (stopped, its tab closed, the
    /// viewer's tab of it closed, the viewer gone): what shows the links, an open menu's check
    /// mark, is to be listed again.
    pub fn take_ended(&mut self) -> bool {
        std::mem::take(&mut self.ended)
    }

    /// The editor exits: ends every link, and gives the courier a moment to tell the viewer.
    pub fn finish(&mut self) {
        if let Some(courier) = self.courier.take() {
            for l in &self.linked {
                courier.off(&l.target);
            }
            courier.finish(FINISH_WAIT);
        }
        self.linked.clear();
    }

    /// Whether the viewer was found gone since the last call; the links are over then.
    pub fn take_gone(&mut self) -> bool {
        let gone = self.courier.as_ref().is_some_and(Courier::is_gone);
        if gone {
            self.ended |= !self.linked.is_empty();
            self.linked.clear();
            self.courier = None;
        }
        gone
    }

    /// What to do now that document `active` has the keyboard, if it has a caret. Switching to a
    /// linked document tells the viewer where it is again.
    pub fn poll(&mut self, now: Instant, active: Option<Active>) -> Poll {
        let Some(active) = active else {
            self.active = None;
            return Poll::default();
        };
        let switched = self.active.replace(active.id) != Some(active.id);
        let Some(link) = self.linked.iter_mut().find(|l| l.id == active.id) else {
            return Poll::default();
        };
        if switched {
            link.sent = None;
            link.sent_version = None;
        }
        let text_due = if active.live {
            link.sent_version != Some(active.version)
        } else {
            link.sent_version.is_some()
        };
        let at = (active.line, active.column.saturating_sub(1) / COLUMN_STEP);
        if link.sent == Some(at) && !text_due && link.open.is_none() {
            return Poll::default();
        }
        let ready = self.sent_at.map(|t| t + INTERVAL);
        if let Some(at) = ready.filter(|&at| now < at) {
            return Poll {
                send: None,
                due: Some(at),
            };
        }
        self.sent_at = Some(now);
        link.sent = Some(at);
        link.sent_version = active.live.then_some(active.version);
        Poll {
            send: Some(Update {
                target: link.target.clone(),
                line: active.line,
                column: active.column,
                retry_until: link.retry_until,
                text: None,
                open: link.open.take(),
            }),
            due: None,
        }
    }

    fn hand(&self, update: Update) {
        if let Some(courier) = &self.courier {
            courier.put(update);
        }
    }
}

impl Docs {
    /// One pass for the links, after whatever the loop has taken in: ends those whose tab is gone
    /// or whose file is not the one shown, and hands the viewer the caret's line and the text when
    /// they are due. The time to come back at, if an update is held back. Nothing happens while
    /// nothing is linked.
    pub fn link_poll(&mut self, now: Instant) -> Option<Instant> {
        if self.links.is_empty() {
            return None;
        }
        let tabs = &self.tabs;
        // A document that had no file and has one now was saved.
        let saved: Vec<(DocId, PathBuf)> = (self.links.linked.iter())
            .filter(|l| l.target.is_untitled())
            .filter_map(|l| {
                let path = tabs.iter().find(|d| d.id == l.id)?.path.clone()?;
                Some((l.id, path))
            })
            .collect();
        for (id, path) in saved {
            self.links.saved(id, path);
        }
        self.links.keep(|l| {
            tabs.iter().any(|d| {
                d.id == l.id
                    && match &l.target {
                        Target::File(path) => d.path.as_deref() == Some(path.as_path()),
                        Target::Untitled { .. } => d.path.is_none(),
                    }
            })
        });
        let live = self.prefs.live_preview;
        let doc = self.active_doc();
        let active = doc.and_then(|d| {
            let r = d.ready()?;
            let no_file = d.path.is_none();
            let caret = r.view.sel.caret;
            let line = r.buffer.line_of(caret);
            Some(Active {
                id: d.id,
                line: line as u32 + 1,
                column: (caret - r.buffer.line_range(line).start) as u32 + 1,
                version: (d.revision, r.buffer.revision()),
                live: (live || no_file)
                    && r.buffer.len_bytes() <= MAX_TEXT
                    && (no_file || d.modified()),
            })
        });
        let poll = self.links.poll(now, active);
        if let Some(mut update) = poll.send {
            if let Some(a) = active.filter(|a| a.live)
                && let Some(r) = self.active_ready()
            {
                update.text = Some((a.version, r.buffer.snapshot()));
            }
            self.links.hand(update);
        }
        poll.due
    }
}

/// The waits of a courier.
#[derive(Clone, Copy)]
struct Timing {
    /// Between two tries while the viewer may be starting.
    retry: Duration,
    /// How long text is left out after a send with text was not answered.
    text_retry: Duration,
    /// After a check found the viewer running, until it is asked once more.
    recheck: Duration,
}

/// The thread that sends to the viewer, and the one slot it takes from.
struct Courier {
    slot: Arc<Slot>,
    gone: Arc<AtomicBool>,
    /// The viewer answered that it does not know documents without a file.
    older: Arc<AtomicBool>,
}

#[derive(Default)]
struct Slot {
    state: Mutex<State>,
    wake: Condvar,
}

#[derive(Default)]
struct State {
    next: Option<Update>,
    /// Documents whose live text is to end; the courier skips those it sent none for.
    offs: Vec<Target>,
    /// Documents without a file that were saved, and the file each is now.
    saved: Vec<(Target, PathBuf)>,
    /// Whether the viewer still runs is to be asked ([`Courier::check`]).
    check: bool,
    closed: bool,
    /// The thread has done all it was asked and returned.
    ended: bool,
}

impl Courier {
    /// `send` hands a request to the viewer's window and says how it was received; `alive` says
    /// whether a viewer window runs, sending it nothing; `ended` is called once when the viewer
    /// is found gone, and each time it turns out to be too old for a document without a file.
    fn spawn(
        send: impl Fn(&Request) -> Delivery + Send + 'static,
        alive: impl Fn() -> bool + Send + 'static,
        timing: Timing,
        ended: impl Fn() + Send + 'static,
    ) -> Courier {
        let slot = Arc::new(Slot::default());
        let gone = Arc::new(AtomicBool::new(false));
        let older = Arc::new(AtomicBool::new(false));
        let (s, g, o) = (slot.clone(), gone.clone(), older.clone());
        std::thread::spawn(move || {
            run(&s, (&g, &o), (send, alive), timing, ended);
            s.state.lock().unwrap().ended = true;
            s.wake.notify_all();
        });
        Courier { slot, gone, older }
    }

    /// Replaces what waits in the slot: of several updates the newest counts.
    fn put(&self, update: Update) {
        let mut state = self.slot.state.lock().unwrap();
        state.next = Some(update.over(state.next.take()));
        drop(state);
        self.slot.wake.notify_all();
    }

    /// The viewer is to show `target` as the file again (a document without a file: as it was
    /// last sent); an update that waits for it is dropped.
    fn off(&self, target: &Target) {
        let mut state = self.slot.state.lock().unwrap();
        if state.next.as_ref().is_some_and(|u| u.target == *target) {
            state.next = None;
        }
        state.offs.push(target.clone());
        self.slot.wake.notify_all();
    }

    /// The document without a file `was` is the file `path` now; an update that waits for it
    /// as it was is dropped.
    fn saved(&self, was: Target, path: PathBuf) {
        let mut state = self.slot.state.lock().unwrap();
        if state.next.as_ref().is_some_and(|u| u.target == was) {
            state.next = None;
        }
        state.saved.push((was, path));
        self.slot.wake.notify_all();
    }

    /// Asks the thread to find out whether the viewer still runs, once it has sent what waits.
    /// Of several asks that wait, one is made.
    fn check(&self) {
        self.slot.state.lock().unwrap().check = true;
        self.slot.wake.notify_all();
    }

    fn is_gone(&self) -> bool {
        self.gone.load(Ordering::Acquire)
    }

    fn take_older(&self) -> bool {
        self.older.swap(false, Ordering::AcqRel)
    }
}

impl Courier {
    /// The editor exits: gives the thread a moment to send what waits for it.
    fn finish(self, wait: Duration) {
        self.slot.state.lock().unwrap().closed = true;
        self.slot.wake.notify_all();
        let state = self.slot.state.lock().unwrap();
        drop(
            self.slot
                .wake
                .wait_timeout_while(state, wait, |s| !s.ended)
                .unwrap(),
        );
    }
}

impl Drop for Courier {
    /// Ends the thread once it has sent what waits for it and finished the send it is in.
    fn drop(&mut self) {
        self.slot.state.lock().unwrap().closed = true;
        self.slot.wake.notify_all();
    }
}

/// What the courier takes from its slot.
enum Job {
    Off(Target),
    Saved(Target, PathBuf),
    Update(Update),
    /// Whether the viewer still runs; when it does and this is true, it is asked once more
    /// after [`Timing::recheck`].
    Check(bool),
}

fn run(
    slot: &Slot,
    (gone, older): (&AtomicBool, &AtomicBool),
    (send, alive): (impl Fn(&Request) -> Delivery, impl Fn() -> bool),
    timing: Timing,
    ended: impl Fn(),
) {
    // The documents the viewer was sent text for, and the version of it.
    let mut live: Vec<(Target, Version)> = Vec::new();
    // Until when text is left out: the viewer did not take the last.
    let mut text_after: Option<Instant> = None;
    // When a check that found the viewer running is made once more: one only, then nothing.
    let mut recheck: Option<Instant> = None;
    loop {
        let job = {
            let mut state = slot.state.lock().unwrap();
            loop {
                if let Some(target) = state.offs.pop() {
                    break Job::Off(target);
                }
                if let Some((was, path)) = state.saved.pop() {
                    break Job::Saved(was, path);
                }
                if let Some(update) = state.next.take() {
                    break Job::Update(update);
                }
                if std::mem::take(&mut state.check) && !state.closed {
                    break Job::Check(true);
                }
                if state.closed {
                    return;
                }
                state = match recheck {
                    Some(at) => match at.checked_duration_since(Instant::now()) {
                        Some(left) if !left.is_zero() => {
                            slot.wake.wait_timeout(state, left).unwrap().0
                        }
                        _ => {
                            recheck = None;
                            break Job::Check(false);
                        }
                    },
                    None => slot.wake.wait(state).unwrap(),
                };
            }
        };
        let update = match job {
            Job::Off(target) => {
                if let Some(i) = live.iter().position(|(t, _)| *t == target) {
                    live.swap_remove(i);
                    send(&off_request(&target));
                }
                continue;
            }
            Job::Saved(was, path) => {
                // The viewer reads the file from here on: nothing of it is live any more.
                live.retain(|(t, _)| *t != was);
                if let Target::Untitled { doc, .. } = &was {
                    send(&saved_request(doc, &path));
                }
                continue;
            }
            Job::Update(update) => update,
            Job::Check(again) => {
                // A viewer that was closed while nothing was sent to it: nobody answers the
                // bare connection, and the links are over as for an update nobody answered.
                // One that answers may be a window closed a moment ago whose process has not
                // ended yet: it is asked once more, a little later.
                if alive() {
                    if again {
                        recheck = Some(Instant::now() + timing.recheck);
                    }
                    continue;
                }
                gone.store(true, Ordering::Release);
                ended();
                return;
            }
        };
        match deliver(&update, &mut live, &mut text_after, &send, timing) {
            Delivery::Taken => continue,
            Delivery::Older => {
                // The viewer runs and will never show this document: say so, and go on for the
                // files that are linked.
                older.store(true, Ordering::Release);
                ended();
                continue;
            }
            Delivery::Nobody => {}
        }
        if Instant::now() < update.retry_until {
            // The viewer is still starting. A newer update takes this one's place.
            std::thread::sleep(timing.retry);
            let mut state = slot.state.lock().unwrap();
            state.next = Some(match state.next.take() {
                Some(newer) => newer.over(Some(update)),
                None => update,
            });
            continue;
        }
        gone.store(true, Ordering::Release);
        ended();
        return;
    }
}

/// Sends `update`: with the text when it is live, differs from what the viewer has and the viewer
/// is not known to refuse text; the line alone otherwise. A document that is not live any more
/// goes back to its file first. `Nobody` when not even the line was answered.
///
/// A document without a file has no line to fall back on: the viewer shows its text or nothing.
/// So its text goes whenever it is new or the viewer is asked to show the document, and when
/// nobody takes it, the same request without the text is asked once, only to learn whether a
/// viewer is there that is too old for it (an older one drops a large text unanswered).
fn deliver(
    update: &Update,
    live: &mut Vec<(Target, Version)>,
    text_after: &mut Option<Instant>,
    send: &impl Fn(&Request) -> Delivery,
    timing: Timing,
) -> Delivery {
    let mut request = update.request();
    let known = live.iter().position(|(t, _)| *t == update.target);
    let untitled = update.target.is_untitled();
    match &update.text {
        None => {
            if let Some(i) = known {
                live.swap_remove(i);
                send(&off_request(&update.target));
            }
        }
        Some((version, rope)) => {
            let fresh = known.map(|i| live[i].1) != Some(*version) || update.open.is_some();
            if fresh && (untitled || text_after.is_none_or(|t| Instant::now() >= t)) {
                request.text = Some(rope.to_string());
                match send(&request) {
                    Delivery::Taken => {
                        match known {
                            Some(i) => live[i].1 = *version,
                            None => live.push((update.target.clone(), *version)),
                        }
                        return Delivery::Taken;
                    }
                    Delivery::Older if untitled => return Delivery::Older,
                    _ => {}
                }
                request.text = None;
                if untitled {
                    // Asked once more without the text, to tell a viewer too old for such a
                    // document from none at all. It shows, raises and moves nothing.
                    request.open = false;
                    request.follow = true;
                    request.place = None;
                    return match send(&request) {
                        Delivery::Older => Delivery::Older,
                        _ => Delivery::Nobody,
                    };
                }
                // No live link for now (an older viewer, or none): the line goes alone.
                *text_after = Some(Instant::now() + timing.text_retry);
            }
        }
    }
    send(&request)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::{Receiver, channel};

    fn at(base: Instant, ms: u64) -> Instant {
        base + Duration::from_millis(ms)
    }

    impl From<&str> for Target {
        fn from(path: &str) -> Target {
            Target::File(path.into())
        }
    }

    fn linked(base: Instant) -> Links {
        let mut links = Links::default();
        links.link(1, "/a.md".into(), Some(5), Some((1, 0)), base);
        links
    }

    /// The document 1 with its caret on `line`, its text at edit `edit`.
    fn doc(line: u32, edit: u64, live: bool) -> Option<Active> {
        Some(Active {
            id: 1,
            line,
            column: 1,
            version: (1, edit),
            live,
        })
    }

    #[test]
    fn nothing_is_sent_for_what_the_viewer_has() {
        let base = Instant::now();
        let mut links = linked(base);
        assert_eq!(links.poll(at(base, 500), doc(5, 0, true)), Poll::default());
        // Another document, linked or not, is nobody's business.
        let other = Active {
            id: 2,
            ..doc(9, 3, true).unwrap()
        };
        assert_eq!(links.poll(at(base, 500), Some(other)), Poll::default());
        assert_eq!(links.poll(at(base, 500), None), Poll::default());
    }

    #[test]
    fn at_most_ten_a_second_and_the_newest_goes() {
        let base = Instant::now();
        let mut links = linked(base);
        // The link itself counted as an update: the next one waits out the interval.
        let held = links.poll(at(base, 30), doc(6, 0, false));
        assert_eq!((held.send, held.due), (None, Some(at(base, 100))));
        let held = links.poll(at(base, 60), doc(8, 0, false));
        assert_eq!((held.send, held.due), (None, Some(at(base, 100))));
        let sent = links.poll(at(base, 100), doc(9, 0, false));
        assert_eq!(
            sent.send.map(|u| (u.target, u.line)),
            Some(("/a.md".into(), 9))
        );
        assert_eq!(sent.due, None);
        // The same line again is not sent twice, nor is the next one before 100 ms.
        assert_eq!(links.poll(at(base, 150), doc(9, 0, false)), Poll::default());
        let held = links.poll(at(base, 150), doc(10, 0, false));
        assert_eq!((held.send, held.due), (None, Some(at(base, 200))));
        // Ten in a second, no more, for a caret that moves at every pass.
        let mut count = 0;
        for ms in 200..1200 {
            let line = 100 + ms as u32;
            count += links.poll(at(base, ms), doc(line, 0, false)).send.is_some() as u32;
        }
        assert_eq!(count, 10);
    }

    #[test]
    fn a_caret_moving_along_a_long_line_tells_the_viewer_every_stretch() {
        let base = Instant::now();
        let mut links = linked(base);
        let along = |column| {
            Some(Active {
                column,
                ..doc(5, 0, true).unwrap()
            })
        };
        // Inside the first stretch of the line the viewer was opened at: nothing.
        assert_eq!(
            links.poll(at(base, 200), along(COLUMN_STEP)),
            Poll::default()
        );
        // Past it: the line again, with the caret's byte in it.
        let sent = links
            .poll(at(base, 200), along(COLUMN_STEP + 1))
            .send
            .unwrap();
        assert_eq!((sent.line, sent.column), (5, COLUMN_STEP + 1));
        let r = sent.request();
        assert_eq!((r.line, r.column), (Some(5), Some(COLUMN_STEP + 1)));
        assert!(r.follow && r.text.is_none());
        // Within that stretch nothing more, and the pace is the usual one.
        assert_eq!(
            links.poll(at(base, 400), along(2 * COLUMN_STEP)),
            Poll::default()
        );
        let held = links.poll(at(base, 250), along(900));
        assert_eq!((held.send, held.due), (None, Some(at(base, 300))));
        let sent = links.poll(at(base, 300), along(900)).send.unwrap();
        assert_eq!(sent.column, 900);
        // Back to the start of the line is a move too; another line counts from its start.
        assert!(links.poll(at(base, 500), along(1)).send.is_some());
        let other = Some(Active {
            column: 3,
            ..doc(6, 0, true).unwrap()
        });
        assert_eq!(
            links
                .poll(at(base, 700), other)
                .send
                .map(|u| (u.line, u.column)),
            Some((6, 3))
        );
        // The wire carries it as `?col=`, which a viewer that predates it skips.
        let wire = String::from_utf8(r.to_wire(VIEWER)).unwrap();
        assert!(
            wire.contains(&format!(
                "?line=5
?col={}
",
                COLUMN_STEP + 1
            )),
            "{wire}"
        );
    }

    #[test]
    fn an_edit_without_a_new_line_is_an_update_when_live_and_not_otherwise() {
        let base = Instant::now();
        let mut links = linked(base);
        let sent = links.poll(at(base, 200), doc(5, 1, true));
        assert_eq!(sent.send.map(|u| u.line), Some(5));
        // The text the viewer was handed is not handed again, however often it is looked at.
        assert_eq!(links.poll(at(base, 400), doc(5, 1, true)), Poll::default());
        // Another edit within the interval waits for it, and the same ten a second hold.
        let held = links.poll(at(base, 250), doc(5, 2, true));
        assert_eq!(held.due, Some(at(base, 300)));
        assert!(links.poll(at(base, 300), doc(5, 2, true)).send.is_some());
        let mut count = 0;
        for ms in 400..1400 {
            count += links.poll(at(base, ms), doc(5, ms, true)).send.is_some() as u32;
        }
        assert_eq!(count, 10);
        // Not live (the setting is off, or the text is over the cap): edits are nobody's business.
        let mut off = Links::default();
        off.link(1, "/a.md".into(), Some(5), None, base);
        assert_eq!(off.poll(at(base, 500), doc(5, 7, false)), Poll::default());
        // And turning it off after text was sent tells the courier, once.
        let mut links = linked(base);
        assert!(links.poll(at(base, 200), doc(5, 1, true)).send.is_some());
        let ended = links.poll(at(base, 400), doc(5, 1, false));
        assert!(ended.send.is_some());
        assert_eq!(links.poll(at(base, 600), doc(5, 1, false)), Poll::default());
    }

    #[test]
    fn a_tab_switch_to_a_linked_document_tells_the_viewer_again() {
        let base = Instant::now();
        let mut links = linked(base);
        links.link(2, "/b.md".into(), Some(1), Some((1, 0)), base);
        let two = |line| {
            Some(Active {
                id: 2,
                line,
                ..doc(1, 0, true).unwrap()
            })
        };
        // The caret of document 1 has not moved, but the user was in document 2 meanwhile.
        assert_eq!(links.poll(at(base, 200), two(1)), Poll::default());
        let third = Active {
            id: 3,
            ..doc(1, 0, true).unwrap()
        };
        assert_eq!(links.poll(at(base, 300), Some(third)), Poll::default());
        let back = links.poll(at(base, 400), doc(5, 0, true));
        assert_eq!(back.send.map(|u| u.line), Some(5));
    }

    #[test]
    fn unlinking_the_last_document_stops_the_courier() {
        let mut links = linked(Instant::now());
        links.start(Arc::new(|_| {}));
        assert!(links.courier.is_some());
        links.unlink(7);
        assert!(links.courier.is_some() && links.contains(1));
        links.unlink(1);
        assert!(links.courier.is_none() && links.is_empty());
    }

    const FAST: Timing = Timing {
        retry: Duration::from_millis(1),
        text_retry: Duration::from_secs(60),
        recheck: Duration::from_millis(50),
    };

    /// What a test courier reports: each request that was sent and whether it was answered.
    type Sent = Receiver<(Request, bool)>;

    /// A courier whose sends answer from `answers` (then `rest`), and report to a channel.
    fn courier(answers: Vec<bool>, rest: bool, timing: Timing) -> (Courier, Sent, Receiver<()>) {
        let (sent_tx, sent_rx) = channel();
        let (ended_tx, ended_rx) = channel();
        let answers = Mutex::new(answers.into_iter());
        let c = Courier::spawn(
            move |r| {
                let ok = answers.lock().unwrap().next().unwrap_or(rest);
                assert!(
                    r.place.is_none()
                        && r.from.as_deref() == Some(FROM)
                        && (r.follow || r.live_off)
                );
                _ = sent_tx.send((r.clone(), ok));
                match ok {
                    true => Delivery::Taken,
                    false => Delivery::Nobody,
                }
            },
            || true,
            timing,
            move || _ = ended_tx.send(()),
        );
        (c, sent_rx, ended_rx)
    }

    fn update(line: u32, retry_until: Instant) -> Update {
        Update {
            target: "/a.md".into(),
            line,
            column: 1,
            retry_until,
            text: None,
            open: None,
        }
    }

    fn with_text(line: u32, edit: u64, text: &str) -> Update {
        Update {
            text: Some(((1, edit), Rope::from_str(text))),
            ..update(line, Instant::now() + Duration::from_secs(60))
        }
    }

    const WAIT: Duration = Duration::from_secs(5);

    fn next(sent: &Sent) -> (Request, bool) {
        sent.recv_timeout(WAIT).unwrap()
    }

    #[test]
    fn a_send_nobody_answers_ends_the_link_after_the_grace_time() {
        let (c, sent, ended) = courier(vec![true], false, FAST);
        let past = Instant::now();
        c.put(update(3, past));
        let (r, ok) = next(&sent);
        assert_eq!((r.line, ok), (Some(3), true));
        assert!(!c.is_gone());
        c.put(update(4, past));
        let (r, ok) = next(&sent);
        assert_eq!((r.line, ok), (Some(4), false));
        ended.recv_timeout(WAIT).unwrap();
        assert!(c.is_gone());
        // The links learn of it, and are over.
        let mut links = linked(Instant::now());
        links.courier = Some(c);
        assert!(links.take_gone() && links.is_empty() && links.courier.is_none());
        assert!(!links.take_gone());
    }

    #[test]
    fn a_viewer_that_is_still_starting_is_tried_again_and_the_link_stays() {
        let (c, sent, _ended) = courier(vec![false, false, true], true, FAST);
        c.put(update(3, Instant::now() + Duration::from_secs(60)));
        let tries: Vec<_> = (0..3)
            .map(|_| next(&sent))
            .map(|(r, ok)| (r.line, ok))
            .collect();
        assert_eq!(tries, [(Some(3), false), (Some(3), false), (Some(3), true)]);
        assert!(!c.is_gone());
    }

    #[test]
    fn a_newer_line_replaces_the_one_that_is_being_retried() {
        let timing = Timing {
            retry: Duration::from_millis(200),
            ..FAST
        };
        let (c, sent, _ended) = courier(vec![false], true, timing);
        c.put(update(3, Instant::now() + Duration::from_secs(60)));
        assert_eq!(next(&sent).0.line, Some(3));
        c.put(update(7, Instant::now() + Duration::from_secs(60)));
        assert_eq!(next(&sent).0.line, Some(7));
        assert!(sent.recv_timeout(Duration::from_millis(400)).is_err());
    }

    #[test]
    fn text_goes_with_the_line_once_and_only_when_it_changed() {
        let (c, sent, _ended) = courier(vec![], true, FAST);
        c.put(with_text(2, 1, "# one\n"));
        let (r, _) = next(&sent);
        assert_eq!(
            (r.text.as_deref(), r.line, r.follow),
            (Some("# one\n"), Some(2), true)
        );
        // The same text at another line: the line alone.
        c.put(with_text(3, 1, "# one\n"));
        let (r, _) = next(&sent);
        assert_eq!((r.text, r.line), (None, Some(3)));
        // Changed: the whole of it again.
        c.put(with_text(3, 2, "# one\nmore\n"));
        let (r, _) = next(&sent);
        assert_eq!(r.text.as_deref(), Some("# one\nmore\n"));
    }

    #[test]
    fn of_several_texts_that_wait_the_newest_is_the_one_sent() {
        // The first send is held until the others are in the slot.
        let (hold_tx, hold_rx) = channel::<()>();
        let (sent_tx, sent_rx) = channel();
        let hold = Mutex::new(Some(hold_rx));
        let c = Courier::spawn(
            move |r| {
                _ = sent_tx.send(r.clone());
                if let Some(rx) = hold.lock().unwrap().take() {
                    rx.recv_timeout(WAIT).unwrap();
                }
                Delivery::Taken
            },
            || true,
            FAST,
            || {},
        );
        c.put(with_text(1, 1, "one"));
        sent_rx.recv_timeout(WAIT).unwrap();
        for (edit, text) in [(2, "two"), (3, "three"), (4, "four")] {
            c.put(with_text(1, edit, text));
        }
        hold_tx.send(()).unwrap();
        let r = sent_rx.recv_timeout(WAIT).unwrap();
        assert_eq!(r.text.as_deref(), Some("four"));
        assert!(sent_rx.recv_timeout(Duration::from_millis(200)).is_err());
    }

    #[test]
    fn a_viewer_that_takes_no_text_gets_the_line_and_is_asked_again_later() {
        let timing = Timing {
            text_retry: Duration::from_millis(300),
            ..FAST
        };
        // The text is not answered, the line is.
        let (c, sent, _ended) = courier(vec![false, true], true, timing);
        c.put(with_text(2, 1, "one"));
        let (r, ok) = next(&sent);
        assert_eq!((r.text.is_some(), ok), (true, false));
        let (r, ok) = next(&sent);
        assert_eq!((r.text.is_some(), r.line, ok), (false, Some(2), true));
        assert!(!c.is_gone());
        // While it is held off, an edit sends the line alone; after it, the text again.
        c.put(with_text(2, 2, "two"));
        assert!(next(&sent).0.text.is_none());
        std::thread::sleep(Duration::from_millis(350));
        c.put(with_text(2, 3, "three"));
        assert_eq!(next(&sent).0.text.as_deref(), Some("three"));
    }

    #[test]
    fn live_text_ends_with_the_link_and_only_for_files_that_were_sent_text() {
        let (c, sent, _ended) = courier(vec![], true, FAST);
        c.put(with_text(2, 1, "one"));
        next(&sent);
        c.off(&"/b.md".into());
        c.off(&"/a.md".into());
        let (r, _) = next(&sent);
        assert!(r.live_off && r.path == Path::new("/a.md") && r.line.is_none());
        // The other file never had text: nothing was sent for it.
        assert!(sent.recv_timeout(Duration::from_millis(200)).is_err());
        // An update that is not live takes a file back to its file too.
        c.put(with_text(2, 2, "two"));
        next(&sent);
        c.put(update(2, Instant::now() + Duration::from_secs(60)));
        let (r, _) = next(&sent);
        assert!(r.live_off);
        assert_eq!(next(&sent).0.line, Some(2));
    }

    #[test]
    fn unlinking_sends_live_off_and_the_editor_exit_waits_for_it() {
        let (c, sent, _ended) = courier(vec![], true, FAST);
        let mut links = linked(Instant::now());
        links.courier = Some(c);
        links.hand(with_text(5, 1, "one"));
        assert_eq!(next(&sent).0.text.as_deref(), Some("one"));
        links.link(2, "/b.md".into(), Some(1), None, Instant::now());
        links.unlink(1);
        assert!(next(&sent).0.live_off);
        // The last one, at exit: the courier has sent it by the time `finish` returns.
        links.hand(Update {
            target: "/b.md".into(),
            ..with_text(1, 1, "b")
        });
        assert_eq!(next(&sent).0.text.as_deref(), Some("b"));
        links.finish();
        let (r, _) = sent.try_recv().expect("sent before finish returned");
        assert!(r.live_off && r.path == Path::new("/b.md"));
        assert!(links.is_empty() && links.courier.is_none());
    }

    #[test]
    fn a_viewer_closed_while_nothing_was_sent_ends_the_links_when_checked() {
        let alive = Arc::new(AtomicBool::new(true));
        let asked = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let (ended_tx, ended) = channel();
        let (a, n) = (alive.clone(), asked.clone());
        let c = Courier::spawn(
            |_| Delivery::Taken,
            move || {
                n.fetch_add(1, Ordering::AcqRel);
                a.load(Ordering::Acquire)
            },
            FAST,
            move || _ = ended_tx.send(()),
        );
        let base = Instant::now();
        let mut links = Links::default();
        links.link(1, "/a.md".into(), Some(5), None, base);
        links.courier = Some(c);
        // A viewer just started may not answer yet: nothing is asked during the grace time.
        assert!(!links.check(base));
        // Running: asked, once more a little later (`recheck`), then no more, and nothing ends.
        assert!(links.check(base + GRACE));
        let end = Instant::now() + WAIT;
        while asked.load(Ordering::Acquire) < 2 && Instant::now() < end {
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(ended.recv_timeout(FAST.recheck * 4).is_err());
        assert_eq!(asked.load(Ordering::Acquire), 2);
        assert!(!links.take_gone() && links.contains(1));
        // Closed: the next check finds nobody, and the links are over.
        alive.store(false, Ordering::Release);
        assert!(links.check(base + GRACE));
        ended.recv_timeout(WAIT).unwrap();
        assert!(links.take_gone() && links.is_empty() && links.courier.is_none());
        // Nothing linked: nothing is asked.
        assert!(!links.check(base + GRACE));
    }

    #[test]
    fn a_viewer_still_ending_when_checked_is_found_gone_when_asked_once_more() {
        // The window gained the focus because the viewer's window was just closed: its process
        // still answers at the first ask, and is gone a moment later. No second check is made.
        let asked = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let (ended_tx, ended) = channel();
        let n = asked.clone();
        let c = Courier::spawn(
            |_| Delivery::Taken,
            move || n.fetch_add(1, Ordering::AcqRel) == 0,
            FAST,
            move || _ = ended_tx.send(()),
        );
        let base = Instant::now();
        let mut links = Links::default();
        links.link(1, "/a.md".into(), Some(5), None, base);
        links.courier = Some(c);
        assert!(links.check(base + GRACE));
        ended.recv_timeout(WAIT).unwrap();
        assert_eq!(asked.load(Ordering::Acquire), 2);
        assert!(links.take_gone() && links.is_empty());
    }

    #[test]
    fn the_viewer_closing_its_tab_ends_that_link_alone() {
        let base = Instant::now();
        let mut links = linked(base);
        links.link(2, "/b.md".into(), Some(1), None, base);
        links.link_untitled(3, "Untitled 1".into(), Opening::default(), base);
        links.closed(Path::new("/b.md"), None);
        assert!(links.contains(1) && !links.contains(2) && links.contains(3));
        links.closed(Path::new("/c.md"), None);
        links.closed(Path::new(""), Some("mdedit-0-3"));
        assert!(links.contains(1) && links.contains(3));
        links.closed(Path::new(""), Some(&doc_name(3)));
        assert!(links.contains(1) && !links.contains(3));
        links.closed(Path::new("/a.md"), None);
        assert!(links.is_empty());
    }

    #[test]
    fn dropping_the_courier_ends_its_thread() {
        let (c, sent, _ended) = courier(vec![], true, FAST);
        let slot = c.slot.clone();
        drop(c);
        // The thread holds the other reference until it returns.
        let end = Instant::now() + WAIT;
        while Arc::strong_count(&slot) > 1 && Instant::now() < end {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(Arc::strong_count(&slot), 1);
        assert!(sent.try_recv().is_err());
    }

    // ---- a document that has no file ----------------------------------------------------------

    fn untitled() -> Target {
        Target::Untitled {
            doc: doc_name(1),
            title: "Untitled 1".into(),
        }
    }

    /// A courier whose sends answer from `answers` (then `Taken`), and report to a channel.
    fn courier_answering(answers: Vec<Delivery>) -> (Courier, Receiver<Request>, Receiver<()>) {
        let (sent_tx, sent_rx) = channel();
        let (ended_tx, ended_rx) = channel();
        let answers = Mutex::new(answers.into_iter());
        let c = Courier::spawn(
            move |r| {
                _ = sent_tx.send(r.clone());
                answers.lock().unwrap().next().unwrap_or(Delivery::Taken)
            },
            || true,
            FAST,
            move || _ = ended_tx.send(()),
        );
        (c, sent_rx, ended_rx)
    }

    fn opening(line: u32, edit: u64, text: &str) -> Update {
        Update {
            target: untitled(),
            open: Some(Opening::default()),
            ..with_text(line, edit, text)
        }
    }

    #[test]
    fn a_document_name_is_one_the_protocol_takes() {
        assert_eq!(sister::doc_id(&doc_name(7)), Some(doc_name(7).as_str()));
        assert_ne!(doc_name(7), doc_name(8));
    }

    #[test]
    fn a_document_without_a_file_is_opened_by_its_first_update_and_then_followed() {
        let base = Instant::now();
        let mut links = Links::default();
        let place = Rect {
            x: 960,
            y: 0,
            w: 960,
            h: 1000,
        };
        let asked = Opening { place: Some(place) };
        links.link_untitled(1, "Untitled 1".into(), asked, base);
        assert!(links.contains(1));
        // The first pass sends at once, whatever the caret and the text are: the viewer has
        // nothing yet. It asks for the tab and carries the place.
        let first = links.poll(base, doc(1, 0, true)).send.unwrap();
        assert_eq!(
            (first.target.clone(), first.open),
            (untitled(), Some(asked))
        );
        let r = first.request();
        assert!(r.open && !r.follow && r.place == Some(place) && r.path.as_os_str().is_empty());
        assert_eq!(r.doc, Some(doc_name(1)));
        assert_eq!(r.title.as_deref(), Some("Untitled 1"));
        // Nothing more until something changes; then ordinary updates, ten a second.
        assert_eq!(links.poll(at(base, 500), doc(1, 0, true)), Poll::default());
        let next = links.poll(at(base, 600), doc(1, 1, true)).send.unwrap();
        assert_eq!(next.open, None);
        let r = next.request();
        assert!(r.follow && !r.open && r.place.is_none() && r.doc.is_some());
        let held = links.poll(at(base, 650), doc(2, 2, true));
        assert_eq!((held.send, held.due), (None, Some(at(base, 700))));
        // Asking for it again (side by side) opens it again.
        links.link_untitled(1, "Untitled 1".into(), Opening::default(), at(base, 800));
        let again = links.poll(at(base, 800), doc(2, 2, true)).send.unwrap();
        assert_eq!(again.open, Some(Opening::default()));
    }

    #[test]
    fn the_opening_update_carries_the_text_even_when_the_viewer_has_it() {
        let (c, sent, _ended) = courier_answering(vec![]);
        c.put(opening(1, 1, "# one\n"));
        let r = sent.recv_timeout(WAIT).unwrap();
        assert!(r.open && r.text.as_deref() == Some("# one\n"));
        // The same text at another line: the line alone, as for a file.
        c.put(Update {
            target: untitled(),
            ..with_text(2, 1, "# one\n")
        });
        let r = sent.recv_timeout(WAIT).unwrap();
        assert!(r.follow && r.text.is_none() && r.line == Some(2) && r.doc.is_some());
        // Opened again with nothing changed (the reader closed the viewer's tab): the text goes.
        c.put(opening(2, 1, "# one\n"));
        let r = sent.recv_timeout(WAIT).unwrap();
        assert!(r.open && r.text.as_deref() == Some("# one\n"));
        // The end of the link tells the viewer, by the document's name.
        c.off(&untitled());
        let r = sent.recv_timeout(WAIT).unwrap();
        assert!(r.live_off && r.doc == Some(doc_name(1)) && r.path.as_os_str().is_empty());
    }

    /// Audit 1, finding 7: an edit while the viewer starts does not lose the request to open a
    /// document without a file, whether the newer update replaces the waiting one or the one
    /// being tried again.
    #[test]
    fn a_newer_update_keeps_the_request_to_open() {
        let (sent_tx, sent) = channel();
        let (gate, answers) = channel::<Delivery>();
        let answers = Mutex::new(answers);
        let c = Courier::spawn(
            move |r| {
                _ = sent_tx.send(r.clone());
                answers.lock().unwrap().recv().unwrap_or(Delivery::Taken)
            },
            || true,
            FAST,
            || {},
        );
        let newer = |edit, text: &str| Update {
            target: untitled(),
            ..with_text(1, edit, text)
        };
        // Replaced while it waits: the courier is busy with a file's update.
        c.put(update(3, Instant::now() + WAIT));
        assert_eq!(sent.recv_timeout(WAIT).unwrap().line, Some(3));
        c.put(opening(1, 1, "# one\n"));
        c.put(newer(2, "# two\n"));
        gate.send(Delivery::Taken).unwrap();
        let r = sent.recv_timeout(WAIT).unwrap();
        assert!(r.open && r.text.as_deref() == Some("# two\n"), "{r:?}");
        gate.send(Delivery::Taken).unwrap();
        // Replacing the one tried again: nobody answers the opening yet.
        c.put(opening(1, 3, "# three\n"));
        let r = sent.recv_timeout(WAIT).unwrap();
        assert!(r.open && r.text.as_deref() == Some("# three\n"));
        c.put(newer(4, "# four\n"));
        gate.send(Delivery::Nobody).unwrap();
        // The question without the text, then the newer text: still an opening.
        let r = loop {
            let r = sent.recv_timeout(WAIT).unwrap();
            if r.text.as_deref() == Some("# four\n") {
                break r;
            }
            gate.send(Delivery::Nobody).unwrap();
        };
        assert!(r.open, "{r:?}");
        gate.send(Delivery::Taken).unwrap();
    }

    #[test]
    fn a_viewer_that_is_starting_gets_the_whole_text_once_it_answers() {
        // Nobody answers the text, nor the question without it; then the viewer is up.
        let nobody = Delivery::Nobody;
        let (c, sent, _ended) = courier_answering(vec![nobody, nobody, nobody, nobody]);
        c.put(opening(1, 1, "# one\n"));
        let tries: Vec<Request> = (0..5).map(|_| sent.recv_timeout(WAIT).unwrap()).collect();
        let texts: Vec<bool> = tries.iter().map(|r| r.text.is_some()).collect();
        assert_eq!(texts, [true, false, true, false, true]);
        // Only the text opens: the question without it shows, raises and moves nothing.
        assert!(tries.iter().all(|r| r.open == r.text.is_some()));
        assert!(tries.iter().all(|r| r.follow != r.open));
        assert!(!c.is_gone() && !c.take_older());
        assert!(sent.recv_timeout(Duration::from_millis(200)).is_err());
    }

    #[test]
    fn a_viewer_too_old_for_such_a_document_ends_its_link_and_keeps_the_files() {
        // The old viewer answers `ok` to what it did not understand. A large text it drops
        // unanswered, so the question without the text is what tells.
        for answers in [
            vec![Delivery::Older],
            vec![Delivery::Nobody, Delivery::Older],
        ] {
            let (c, sent, ended) = courier_answering(answers.clone());
            let mut links = linked(Instant::now());
            links.link_untitled(2, "Untitled 1".into(), Opening::default(), Instant::now());
            links.courier = Some(c);
            links.hand(opening(1, 1, "# one\n"));
            for _ in &answers {
                sent.recv_timeout(WAIT).unwrap();
            }
            ended.recv_timeout(WAIT).unwrap();
            assert!(!links.take_gone());
            assert!(links.take_older() && !links.take_older());
            assert!(links.contains(1) && !links.contains(2));
            // The courier lives on for the file.
            links.hand(update(4, Instant::now()));
            assert_eq!(sent.recv_timeout(WAIT).unwrap().line, Some(4));
        }
        // With nothing else linked the courier ends too.
        let (c, _sent, ended) = courier_answering(vec![Delivery::Older]);
        let mut links = Links::default();
        links.link_untitled(2, "Untitled 1".into(), Opening::default(), Instant::now());
        links.courier = Some(c);
        links.hand(opening(1, 1, "x"));
        ended.recv_timeout(WAIT).unwrap();
        assert!(links.take_older() && links.is_empty() && links.courier.is_none());
    }

    #[test]
    fn a_saved_document_hands_its_tab_to_the_file() {
        let (c, sent, _ended) = courier_answering(vec![]);
        let base = Instant::now();
        let mut links = Links::default();
        links.link_untitled(1, "Untitled 1".into(), Opening::default(), base);
        links.courier = Some(c);
        links.hand(opening(3, 1, "# one\n"));
        assert!(sent.recv_timeout(WAIT).unwrap().open);
        links.saved(1, "/notes/one.md".into());
        let r = sent.recv_timeout(WAIT).unwrap();
        assert_eq!(r.path, Path::new("/notes/one.md"));
        assert!(r.follow && r.doc == Some(doc_name(1)) && r.text.is_none() && !r.open);
        // The link is the file's now: the line goes again, with no text for a saved file, and
        // no word about the document it was (the viewer was never sent the file's text).
        let next = links.poll(at(base, 500), doc(3, 1, false)).send.unwrap();
        assert_eq!(
            (next.target.clone(), next.open),
            ("/notes/one.md".into(), None)
        );
        links.hand(next);
        let r = sent.recv_timeout(WAIT).unwrap();
        assert!(r.follow && r.doc.is_none() && r.line == Some(3) && !r.live_off);
        assert!(sent.recv_timeout(Duration::from_millis(200)).is_err());
        // An update that waited for the document as it was is dropped by the save.
        let (hold_tx, hold_rx) = channel::<()>();
        let (sent_tx, sent_rx) = channel();
        let hold = Mutex::new(Some(hold_rx));
        let c = Courier::spawn(
            move |r| {
                _ = sent_tx.send(r.clone());
                if let Some(rx) = hold.lock().unwrap().take() {
                    rx.recv_timeout(WAIT).unwrap();
                }
                Delivery::Taken
            },
            || true,
            FAST,
            || {},
        );
        c.put(opening(1, 1, "a"));
        sent_rx.recv_timeout(WAIT).unwrap();
        c.put(Update {
            target: untitled(),
            ..with_text(1, 2, "ab")
        });
        c.saved(untitled(), "/notes/one.md".into());
        hold_tx.send(()).unwrap();
        let r = sent_rx.recv_timeout(WAIT).unwrap();
        assert_eq!(r.path, Path::new("/notes/one.md"));
        assert!(sent_rx.recv_timeout(Duration::from_millis(200)).is_err());
    }
}
