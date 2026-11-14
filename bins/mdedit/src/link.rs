//! Previewing in mdview: the documents whose file the viewer shows, and what keeps the viewer
//! scrolled to their carets. docs/DESIGN.md, "The hand-off with mdview", has the whole design.
//!
//! A document is linked when "Preview in mdview" opened its file in the viewer. While any is,
//! each pass of the event loop looks at the caret's line of the active document ([`Docs::link_poll`]);
//! when that line is not the one the viewer was last told, at most ten times a second, the line is
//! put in the [`Courier`]'s one slot, newest wins. The courier is a thread that sends the slot's
//! request to the viewer's window, so a viewer that is slow or stuck cannot hold the window up (a
//! send waits up to 1.5 s for the answer). Nothing runs while nothing is linked: no timer, no
//! thread.
//!
//! A link ends when its tab closes or its file is renamed, when the user stops it, when the viewer
//! is gone (a send that nobody answers more than [`GRACE`] after the viewer was started), and with
//! the process. The unsaved text is not sent: the viewer shows the file, and reloads it on save.

use crate::app::Wake;
use crate::docs::{Docs, Waker};
use mdedit_highlight::DocId;
use mdshared_app::sister::{self, Request};
use std::path::PathBuf;
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
/// Between two tries during the grace time.
const RETRY: Duration = Duration::from_millis(250);

/// What to tell the viewer: this file is at this line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Update {
    pub path: PathBuf,
    pub line: u32,
    /// Until when a send nobody answers is tried again.
    pub retry_until: Instant,
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

struct Linked {
    id: DocId,
    /// The file the viewer was given; a document that is saved under another name is not it.
    path: PathBuf,
    /// The line the viewer was last told.
    sent: Option<u32>,
    retry_until: Instant,
}

/// What a pass of the loop has to do.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Poll {
    /// An update to send now.
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

    /// Links document `id`, whose file `path` the viewer was just asked to show at `line`.
    pub fn link(&mut self, id: DocId, path: PathBuf, line: Option<u32>, now: Instant) {
        self.linked.retain(|l| l.id != id);
        self.linked.push(Linked {
            id,
            path,
            sent: line,
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
                RETRY,
                move || waker(Wake::Link),
            )
        });
    }

    pub fn unlink(&mut self, id: DocId) {
        self.keep(|l| l.id != id);
    }

    /// Ends the links `keep` says no to.
    fn keep(&mut self, mut keep: impl FnMut(&Linked) -> bool) {
        self.linked.retain(|l| keep(l));
        if self.linked.is_empty() {
            self.courier = None;
        }
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

    /// What to do now that document `active` (with its caret on `line`, from 1) has the keyboard,
    /// if it has a caret. Switching to a linked document tells the viewer where it is again.
    pub fn poll(&mut self, now: Instant, active: Option<(DocId, u32)>) -> Poll {
        let Some((id, line)) = active else {
            self.active = None;
            return Poll::default();
        };
        let switched = self.active.replace(id) != Some(id);
        let Some(link) = self.linked.iter_mut().find(|l| l.id == id) else {
            return Poll::default();
        };
        if switched {
            link.sent = None;
        }
        if link.sent == Some(line) {
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
        link.sent = Some(line);
        Poll {
            send: Some(Update {
                path: link.path.clone(),
                line,
                retry_until: link.retry_until,
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
    /// or whose file is not the one shown, and hands the viewer the caret's line when it is due.
    /// The time to come back at, if an update is held back. Nothing happens while nothing is linked.
    ///
    /// A live preview of unsaved text would go here too: the same pass sees the active document,
    /// and an [`Update`] is what the courier sends.
    pub fn link_poll(&mut self, now: Instant) -> Option<Instant> {
        if self.links.is_empty() {
            return None;
        }
        let tabs = &self.tabs;
        self.links.keep(|l| {
            tabs.iter()
                .any(|d| d.id == l.id && d.path.as_deref() == Some(l.path.as_path()))
        });
        let caret = self
            .active_ready()
            .map(|r| r.buffer.line_of(r.view.sel.caret) as u32 + 1);
        let id = self.active_doc().map(|d| d.id);
        let poll = self.links.poll(now, id.zip(caret));
        if let Some(update) = poll.send {
            self.links.hand(update);
        }
        poll.due
    }
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
    closed: bool,
}

impl Courier {
    /// `send` hands a request to the viewer's window and says whether one answered; `retry` is the
    /// wait between tries while the viewer may be starting; `ended` is called once when the
    /// viewer is found gone.
    fn spawn(
        send: impl Fn(&Request) -> bool + Send + 'static,
        retry: Duration,
        ended: impl Fn() + Send + 'static,
    ) -> Courier {
        let slot = Arc::new(Slot::default());
        let gone = Arc::new(AtomicBool::new(false));
        let (s, g) = (slot.clone(), gone.clone());
        std::thread::spawn(move || run(&s, &g, send, retry, ended));
        Courier { slot, gone }
    }

    /// Replaces what waits in the slot: of several updates the newest counts.
    fn put(&self, update: Update) {
        self.slot.state.lock().unwrap().next = Some(update);
        self.slot.wake.notify_one();
    }

    fn is_gone(&self) -> bool {
        self.gone.load(Ordering::Acquire)
    }
}

impl Drop for Courier {
    /// Ends the thread once it has finished the send it is in.
    fn drop(&mut self) {
        self.slot.state.lock().unwrap().closed = true;
        self.slot.wake.notify_one();
    }
}

fn run(
    slot: &Slot,
    gone: &AtomicBool,
    send: impl Fn(&Request) -> bool,
    retry: Duration,
    ended: impl Fn(),
) {
    loop {
        let update = {
            let mut state = slot.state.lock().unwrap();
            loop {
                if state.closed {
                    return;
                }
                if let Some(update) = state.next.take() {
                    break update;
                }
                state = slot.wake.wait(state).unwrap();
            }
        };
        if send(&update.request()) {
            continue;
        }
        if Instant::now() < update.retry_until {
            // The viewer is still starting. A newer update takes this one's place.
            std::thread::sleep(retry);
            let mut state = slot.state.lock().unwrap();
            state.next.get_or_insert(update);
            continue;
        }
        gone.store(true, Ordering::Release);
        ended();
        return;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;

    fn at(base: Instant, ms: u64) -> Instant {
        base + Duration::from_millis(ms)
    }

    fn linked(base: Instant) -> Links {
        let mut links = Links::default();
        links.link(1, "/a.md".into(), Some(5), base);
        links
    }

    #[test]
    fn nothing_is_sent_for_the_line_the_viewer_has() {
        let base = Instant::now();
        let mut links = linked(base);
        assert_eq!(links.poll(at(base, 500), Some((1, 5))), Poll::default());
        // Another document, linked or not, is nobody's business.
        assert_eq!(links.poll(at(base, 500), Some((2, 9))), Poll::default());
        assert_eq!(links.poll(at(base, 500), None), Poll::default());
    }

    #[test]
    fn at_most_ten_a_second_and_the_newest_line_goes() {
        let base = Instant::now();
        let mut links = linked(base);
        // The link itself counted as an update: the next one waits out the interval.
        let held = links.poll(at(base, 30), Some((1, 6)));
        assert_eq!((held.send, held.due), (None, Some(at(base, 100))));
        // The caret moved on meanwhile; when the interval has passed, that line is the one sent.
        let held = links.poll(at(base, 60), Some((1, 8)));
        assert_eq!((held.send, held.due), (None, Some(at(base, 100))));
        let sent = links.poll(at(base, 100), Some((1, 9)));
        assert_eq!(
            sent.send.map(|u| (u.path, u.line)),
            Some(("/a.md".into(), 9))
        );
        assert_eq!(sent.due, None);
        // And the same line again is not sent twice, nor is the next one before 100 ms.
        assert_eq!(links.poll(at(base, 150), Some((1, 9))), Poll::default());
        let held = links.poll(at(base, 150), Some((1, 10)));
        assert_eq!((held.send, held.due), (None, Some(at(base, 200))));
        // Ten in a second, no more, for a caret that moves at every pass.
        let mut count = 0;
        for ms in 200..1200 {
            let line = 100 + ms as u32;
            count += links.poll(at(base, ms), Some((1, line))).send.is_some() as u32;
        }
        assert_eq!(count, 10);
    }

    #[test]
    fn a_tab_switch_to_a_linked_document_tells_the_viewer_again() {
        let base = Instant::now();
        let mut links = linked(base);
        links.link(2, "/b.md".into(), Some(1), base);
        // The caret of document 1 has not moved, but the user was in document 2 meanwhile.
        assert_eq!(links.poll(at(base, 200), Some((2, 1))), Poll::default());
        assert_eq!(links.poll(at(base, 300), Some((3, 1))), Poll::default());
        let back = links.poll(at(base, 400), Some((1, 5)));
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

    /// What a test courier reports: the line of each send and whether it was answered.
    type Sent = std::sync::mpsc::Receiver<(Option<u32>, bool)>;

    /// A courier whose sends answer from `answers` (then false), and report to a channel.
    fn courier(
        answers: Vec<bool>,
        retry: Duration,
    ) -> (Courier, Sent, std::sync::mpsc::Receiver<()>) {
        let (sent_tx, sent_rx) = channel();
        let (ended_tx, ended_rx) = channel();
        let answers = Mutex::new(answers.into_iter());
        let c = Courier::spawn(
            move |r| {
                let ok = answers.lock().unwrap().next().unwrap_or(false);
                assert!(r.follow && r.place.is_none() && r.from.as_deref() == Some(FROM));
                _ = sent_tx.send((r.line, ok));
                ok
            },
            retry,
            move || _ = ended_tx.send(()),
        );
        (c, sent_rx, ended_rx)
    }

    fn update(line: u32, retry_until: Instant) -> Update {
        Update {
            path: "/a.md".into(),
            line,
            retry_until,
        }
    }

    const WAIT: Duration = Duration::from_secs(5);

    #[test]
    fn a_send_nobody_answers_ends_the_link_after_the_grace_time() {
        let (c, sent, ended) = courier(vec![true], Duration::from_millis(1));
        let past = Instant::now();
        c.put(update(3, past));
        assert_eq!(sent.recv_timeout(WAIT).unwrap(), (Some(3), true));
        assert!(!c.is_gone());
        c.put(update(4, past));
        assert_eq!(sent.recv_timeout(WAIT).unwrap(), (Some(4), false));
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
        let (c, sent, _ended) = courier(vec![false, false, true], Duration::from_millis(1));
        c.put(update(3, Instant::now() + Duration::from_secs(60)));
        let tries: Vec<_> = (0..3).map(|_| sent.recv_timeout(WAIT).unwrap()).collect();
        assert_eq!(tries, [(Some(3), false), (Some(3), false), (Some(3), true)]);
        assert!(!c.is_gone());
    }

    #[test]
    fn a_newer_line_replaces_the_one_that_is_being_retried() {
        let (c, sent, _ended) = courier(vec![false, true], Duration::from_millis(200));
        c.put(update(3, Instant::now() + Duration::from_secs(60)));
        assert_eq!(sent.recv_timeout(WAIT).unwrap(), (Some(3), false));
        c.put(update(7, Instant::now() + Duration::from_secs(60)));
        assert_eq!(sent.recv_timeout(WAIT).unwrap(), (Some(7), true));
        assert!(sent.recv_timeout(Duration::from_millis(400)).is_err());
    }

    #[test]
    fn dropping_the_courier_ends_its_thread() {
        let (sent_tx, sent_rx) = channel::<()>();
        let c = Courier::spawn(
            |_| true,
            Duration::from_millis(1),
            move || _ = sent_tx.send(()),
        );
        let slot = c.slot.clone();
        drop(c);
        // The thread holds the other reference until it returns.
        let end = Instant::now() + WAIT;
        while Arc::strong_count(&slot) > 1 && Instant::now() < end {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(Arc::strong_count(&slot), 1);
        assert!(sent_rx.try_recv().is_err());
    }
}
