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

use crate::app::Wake;
use crate::docs::{Docs, Waker};
use mdedit_highlight::DocId;
use mdshared_app::sister::{self, MAX_TEXT, Request};
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
/// How long the exit waits for the courier to tell the viewer the links are over.
const FINISH_WAIT: Duration = Duration::from_millis(300);

/// A document's text as it is: the load it came from and the edits since.
pub type Version = (u64, u64);

/// What to tell the viewer: this file is at this line, and (live) this is its text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Update {
    pub path: PathBuf,
    pub line: u32,
    /// Until when a send nobody answers is tried again.
    pub retry_until: Instant,
    /// The text, when the preview is live: its version and a snapshot. `None` when it is not.
    pub text: Option<(Version, Rope)>,
}

impl Update {
    /// The live update: no raising, no opening, no placing.
    fn request(&self) -> Request {
        Request {
            follow: true,
            line: Some(self.line),
            from: Some(FROM.into()),
            ..Request::new(&self.path)
        }
    }
}

/// The request that ends the viewer's live text for `path`.
fn off_request(path: &Path) -> Request {
    Request {
        live_off: true,
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
    pub version: Version,
    /// Its text is to be sent: the setting is on, it is within the cap, and the document differs
    /// from its saved state (otherwise the viewer shows the file, and is told to go back to it
    /// once the text is the file's again).
    pub live: bool,
}

struct Linked {
    id: DocId,
    /// The file the viewer was given; a document that is saved under another name is not it.
    path: PathBuf,
    /// The line the viewer was last told.
    sent: Option<u32>,
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
        path: PathBuf,
        line: Option<u32>,
        version: Option<Version>,
        now: Instant,
    ) {
        self.linked.retain(|l| l.id != id);
        self.linked.push(Linked {
            id,
            path,
            sent: line,
            sent_version: version,
            retry_until: now + GRACE,
        });
        self.active = Some(id);
        self.sent_at = Some(now);
    }

    /// Starts what sends the updates; `waker` is called when the viewer turns out to be gone.
    pub fn start(&mut self, waker: Waker) {
        self.courier.get_or_insert_with(|| {
            Courier::spawn(
                |request| sister::send(VIEWER, request),
                Timing {
                    retry: RETRY,
                    text_retry: TEXT_RETRY,
                },
                move || waker(Wake::Link),
            )
        });
    }

    pub fn unlink(&mut self, id: DocId) {
        self.keep(|l| l.id != id);
    }

    /// Ends the links `keep` says no to, telling the viewer to go back to their files.
    fn keep(&mut self, mut keep: impl FnMut(&Linked) -> bool) {
        let courier = &self.courier;
        self.linked.retain(|l| {
            let kept = keep(l);
            if let Some(c) = courier.as_ref().filter(|_| !kept) {
                c.off(&l.path);
            }
            kept
        });
        if self.linked.is_empty() {
            self.courier = None;
        }
    }

    /// The editor exits: ends every link, and gives the courier a moment to tell the viewer.
    pub fn finish(&mut self) {
        if let Some(courier) = self.courier.take() {
            for l in &self.linked {
                courier.off(&l.path);
            }
            courier.finish(FINISH_WAIT);
        }
        self.linked.clear();
    }

    /// Whether the viewer was found gone since the last call; the links are over then.
    pub fn take_gone(&mut self) -> bool {
        let gone = self.courier.as_ref().is_some_and(Courier::is_gone);
        if gone {
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
        if link.sent == Some(active.line) && !text_due {
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
        link.sent = Some(active.line);
        link.sent_version = active.live.then_some(active.version);
        Poll {
            send: Some(Update {
                path: link.path.clone(),
                line: active.line,
                retry_until: link.retry_until,
                text: None,
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
        self.links.keep(|l| {
            tabs.iter()
                .any(|d| d.id == l.id && d.path.as_deref() == Some(l.path.as_path()))
        });
        let live = self.prefs.live_preview;
        let doc = self.active_doc();
        let active = doc.and_then(|d| {
            let r = d.ready()?;
            Some(Active {
                id: d.id,
                line: r.buffer.line_of(r.view.sel.caret) as u32 + 1,
                version: (d.revision, r.buffer.revision()),
                live: live && r.buffer.len_bytes() <= MAX_TEXT && d.modified(),
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
}

/// The thread that sends to the viewer, and the one slot it takes from.
struct Courier {
    slot: Arc<Slot>,
    gone: Arc<AtomicBool>,
}

#[derive(Default)]
struct Slot {
    state: Mutex<State>,
    wake: Condvar,
}

#[derive(Default)]
struct State {
    next: Option<Update>,
    /// Files whose live text is to end; the courier skips those it sent none for.
    offs: Vec<PathBuf>,
    closed: bool,
    /// The thread has done all it was asked and returned.
    ended: bool,
}

impl Courier {
    /// `send` hands a request to the viewer's window and says whether one answered; `ended` is
    /// called once when the viewer is found gone.
    fn spawn(
        send: impl Fn(&Request) -> bool + Send + 'static,
        timing: Timing,
        ended: impl Fn() + Send + 'static,
    ) -> Courier {
        let slot = Arc::new(Slot::default());
        let gone = Arc::new(AtomicBool::new(false));
        let (s, g) = (slot.clone(), gone.clone());
        std::thread::spawn(move || {
            run(&s, &g, send, timing, ended);
            s.state.lock().unwrap().ended = true;
            s.wake.notify_all();
        });
        Courier { slot, gone }
    }

    /// Replaces what waits in the slot: of several updates the newest counts.
    fn put(&self, update: Update) {
        self.slot.state.lock().unwrap().next = Some(update);
        self.slot.wake.notify_all();
    }

    /// The viewer is to show `path` as the file again; an update that waits for it is dropped.
    fn off(&self, path: &Path) {
        let mut state = self.slot.state.lock().unwrap();
        if state.next.as_ref().is_some_and(|u| u.path == path) {
            state.next = None;
        }
        state.offs.push(path.to_path_buf());
        self.slot.wake.notify_all();
    }

    fn is_gone(&self) -> bool {
        self.gone.load(Ordering::Acquire)
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

fn run(
    slot: &Slot,
    gone: &AtomicBool,
    send: impl Fn(&Request) -> bool,
    timing: Timing,
    ended: impl Fn(),
) {
    // The files the viewer was sent text for, and the version of it.
    let mut live: Vec<(PathBuf, Version)> = Vec::new();
    // Until when text is left out: the viewer did not take the last.
    let mut text_after: Option<Instant> = None;
    loop {
        let (off, update) = {
            let mut state = slot.state.lock().unwrap();
            loop {
                if let Some(path) = state.offs.pop() {
                    break (Some(path), None);
                }
                if let Some(update) = state.next.take() {
                    break (None, Some(update));
                }
                if state.closed {
                    return;
                }
                state = slot.wake.wait(state).unwrap();
            }
        };
        if let Some(path) = off {
            if let Some(i) = live.iter().position(|(p, _)| *p == path) {
                live.swap_remove(i);
                send(&off_request(&path));
            }
            continue;
        }
        let Some(update) = update else { continue };
        if deliver(&update, &mut live, &mut text_after, &send, timing) {
            continue;
        }
        if Instant::now() < update.retry_until {
            // The viewer is still starting. A newer update takes this one's place.
            std::thread::sleep(timing.retry);
            let mut state = slot.state.lock().unwrap();
            state.next.get_or_insert(update);
            continue;
        }
        gone.store(true, Ordering::Release);
        ended();
        return;
    }
}

/// Sends `update`: with the text when it is live, differs from what the viewer has and the viewer
/// is not known to refuse text; the line alone otherwise. A document that is not live any more
/// goes back to its file first. False when not even the line was answered.
fn deliver(
    update: &Update,
    live: &mut Vec<(PathBuf, Version)>,
    text_after: &mut Option<Instant>,
    send: &impl Fn(&Request) -> bool,
    timing: Timing,
) -> bool {
    let mut request = update.request();
    let known = live.iter().position(|(p, _)| *p == update.path);
    match &update.text {
        None => {
            if let Some(i) = known {
                live.swap_remove(i);
                send(&off_request(&update.path));
            }
        }
        Some((version, rope)) => {
            let fresh = known.map(|i| live[i].1) != Some(*version);
            if fresh && text_after.is_none_or(|t| Instant::now() >= t) {
                request.text = Some(rope.to_string());
                if send(&request) {
                    match known {
                        Some(i) => live[i].1 = *version,
                        None => live.push((update.path.clone(), *version)),
                    }
                    return true;
                }
                // No live link for now (an older viewer, or none): the line goes alone.
                *text_after = Some(Instant::now() + timing.text_retry);
                request.text = None;
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
            sent.send.map(|u| (u.path, u.line)),
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
                ok
            },
            timing,
            move || _ = ended_tx.send(()),
        );
        (c, sent_rx, ended_rx)
    }

    fn update(line: u32, retry_until: Instant) -> Update {
        Update {
            path: "/a.md".into(),
            line,
            retry_until,
            text: None,
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
                true
            },
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
        c.off(Path::new("/b.md"));
        c.off(Path::new("/a.md"));
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
            path: "/b.md".into(),
            ..with_text(1, 1, "b")
        });
        assert_eq!(next(&sent).0.text.as_deref(), Some("b"));
        links.finish();
        let (r, _) = sent.try_recv().expect("sent before finish returned");
        assert!(r.live_off && r.path == Path::new("/b.md"));
        assert!(links.is_empty() && links.courier.is_none());
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
}
