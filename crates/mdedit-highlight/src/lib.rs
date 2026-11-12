//! The highlighting worker. One thread per window holds a
//! [`LineHighlighter`](mdshared_highlight::incremental::LineHighlighter) per open document and a
//! rope snapshot of its text. The UI thread sends it commands (open, view, theme, close) and
//! takes coloured lines back (an edit sends the new text and the first line it changed); the worker calls the `wake` closure after it sends, which the shell
//! turns into a winit user event. When it has nothing to do it blocks on its command channel, so
//! an idle window costs no CPU here.
//!
//! Order of work: the latest `view` request of the most recently viewed document first; then that
//! document's parse forward to the end of the text, in slices of about 2 ms with the command
//! channel checked between slices. Other documents wait until they are viewed.

pub use mdshared_highlight::incremental::{Language, Limits, Span, foreground};

use mdshared_highlight::incremental::{LineHighlighter, Lines};
use ropey::Rope;
use std::collections::HashMap;
use std::ops::Range;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::time::{Duration, Instant};

/// The shell's identifier for an open document (one per tab, kept across reloads).
pub type DocId = u64;

/// How long the worker serves a view request before it looks at its commands again.
const VIEW_SLICE: Duration = Duration::from_millis(8);
/// How long one step of the background parse runs before the commands are looked at again.
const ADVANCE_SLICE: Duration = Duration::from_millis(2);

/// [`Lines`] over a rope: line text without its LF, CRLF or CR.
pub struct RopeLines<'a>(pub &'a Rope);

impl Lines for RopeLines<'_> {
    fn len_lines(&self) -> usize {
        self.0.len_lines()
    }

    fn line(&self, index: usize, out: &mut String) {
        out.clear();
        if index >= self.0.len_lines() {
            return;
        }
        for chunk in self.0.line(index).chunks() {
            out.push_str(chunk);
        }
        // The rope breaks lines at LF, CRLF and CR (ropey's `cr_lines`); a CRLF is one break.
        if out.ends_with('\n') {
            out.pop();
        }
        if out.ends_with('\r') {
            out.pop();
        }
    }
}

/// Colours for a run of lines of one document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Colored {
    pub doc: DocId,
    /// The revision passed to `open`; the shell drops results for an older one.
    pub revision: u64,
    pub dark: bool,
    pub first_line: usize,
    /// One entry per line from `first_line`, as `LineHighlighter::spans` makes them.
    pub lines: Vec<Vec<Span>>,
}

enum Command {
    Open {
        doc: DocId,
        revision: u64,
        rope: Rope,
        language: Language,
        dark: bool,
    },
    View {
        doc: DocId,
        revision: u64,
        range: Range<usize>,
    },
    Edit {
        doc: DocId,
        revision: u64,
        rope: Rope,
        first_line: usize,
        line_delta: isize,
    },
    SetDark(bool),
    Close(DocId),
    Stop,
}

pub struct HighlightService {
    commands: Sender<Command>,
    results: Receiver<Colored>,
}

impl HighlightService {
    /// Starts the worker thread. `wake` is called from it after results are queued (at most once
    /// per batch); the shell sends itself a winit user event from it.
    pub fn spawn(wake: impl Fn() + Send + 'static) -> HighlightService {
        let (commands, command_rx) = mpsc::channel();
        let (result_tx, results) = mpsc::channel();
        let worker = Worker {
            commands: command_rx,
            results: result_tx,
            wake: Box::new(wake),
            docs: HashMap::new(),
            active: None,
            stopped: false,
        };
        std::thread::Builder::new()
            .name("mdedit-highlight".into())
            .spawn(move || worker.run())
            .expect("the highlight thread starts");
        HighlightService { commands, results }
    }

    /// Starts (or restarts, for a reload) highlighting `text` as `language`. A document whose
    /// text is over the language's file limit is accepted and never coloured. Replaces any state
    /// the worker held for `doc`.
    pub fn open(&self, doc: DocId, revision: u64, text: Rope, language: Language, dark: bool) {
        let _ = self.commands.send(Command::Open {
            doc,
            revision,
            rope: text,
            language,
            dark,
        });
    }

    /// The lines the shell wants colours for (it passes the visible lines plus a screen above and
    /// below). Replaces the previous request; the worker answers with one [`Colored`] covering
    /// them as soon as its parse has reached them. A request whose `revision` is not the one the
    /// document was opened with gets no answer.
    pub fn view(&self, doc: DocId, revision: u64, lines: Range<usize>) {
        let _ = self.commands.send(Command::View {
            doc,
            revision,
            range: lines,
        });
    }

    /// The document's text changed: `text` is the new text at `revision`, `first_line` the first
    /// line that differs from the old text and `line_delta` how many lines were added (negative:
    /// removed). The parse goes back to its last stored state at or before `first_line`; the view
    /// request stays and must be made again with the new revision to be answered.
    pub fn edit(
        &self,
        doc: DocId,
        revision: u64,
        text: Rope,
        first_line: usize,
        line_delta: isize,
    ) {
        let _ = self.commands.send(Command::Edit {
            doc,
            revision,
            rope: text,
            first_line,
            line_delta,
        });
    }

    /// Switches every document's theme; the current view request is answered again.
    pub fn set_dark(&self, dark: bool) {
        let _ = self.commands.send(Command::SetDark(dark));
    }

    /// Drops the worker's state for `doc`.
    pub fn close(&self, doc: DocId) {
        let _ = self.commands.send(Command::Close(doc));
    }

    /// Results queued since the last call, oldest first. Never blocks.
    pub fn take(&self) -> Vec<Colored> {
        self.results.try_iter().collect()
    }

    /// Like [`take`](Self::take), but waits until at least one result is queued or `deadline`
    /// passes. For the first frame and for `--shot`.
    pub fn wait(&self, deadline: Instant) -> Vec<Colored> {
        let mut out = Vec::new();
        let timeout = deadline.saturating_duration_since(Instant::now());
        match self.results.recv_timeout(timeout) {
            Ok(first) => out.push(first),
            Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => {}
        }
        out.extend(self.results.try_iter());
        out
    }
}

impl Drop for HighlightService {
    /// Tells the worker to stop; does not wait for it.
    fn drop(&mut self) {
        let _ = self.commands.send(Command::Stop);
    }
}

struct Doc {
    revision: u64,
    rope: Rope,
    highlighter: LineHighlighter,
    /// The lines last asked for.
    view: Option<Range<usize>>,
    /// The request in `view` has not been answered.
    pending: bool,
}

/// What one round of work did.
enum Work {
    /// A result was sent.
    Sent,
    /// Some parsing was done; there may be more.
    Ran,
    /// Nothing to do until a command arrives.
    Idle,
}

struct Worker {
    commands: Receiver<Command>,
    results: Sender<Colored>,
    wake: Box<dyn Fn() + Send>,
    docs: HashMap<DocId, Doc>,
    /// The most recently viewed document, the one whose parse runs on in the background.
    active: Option<DocId>,
    stopped: bool,
}

impl Worker {
    fn run(mut self) {
        while !self.stopped {
            self.drain();
            if self.stopped {
                break;
            }
            match self.work() {
                Work::Sent => (self.wake)(),
                Work::Ran => {}
                // The only place the thread waits: it sleeps in `recv` until the UI thread sends
                // something, so an idle worker uses no CPU and never polls.
                Work::Idle => match self.commands.recv() {
                    Ok(command) => self.handle(command),
                    Err(_) => break,
                },
            }
        }
    }

    /// Takes every command that is queued, without blocking.
    fn drain(&mut self) {
        loop {
            match self.commands.try_recv() {
                Ok(command) => self.handle(command),
                Err(TryRecvError::Empty) => return,
                Err(TryRecvError::Disconnected) => {
                    self.stopped = true;
                    return;
                }
            }
        }
    }

    fn handle(&mut self, command: Command) {
        match command {
            Command::Open {
                doc,
                revision,
                rope,
                language,
                dark,
            } => {
                // Made here, so syntect's first-use work (its lazily compiled regexes) happens on
                // this thread.
                let highlighter =
                    LineHighlighter::new(language, dark, rope.len_lines(), rope.len_bytes());
                self.docs.insert(
                    doc,
                    Doc {
                        revision,
                        rope,
                        highlighter,
                        view: None,
                        pending: false,
                    },
                );
                if self.active.is_none() {
                    self.active = Some(doc);
                }
            }
            Command::View {
                doc,
                revision,
                range,
            } => {
                if let Some(entry) = self.docs.get_mut(&doc).filter(|d| d.revision == revision) {
                    entry.view = Some(range);
                    entry.pending = true;
                    self.active = Some(doc);
                }
            }
            Command::Edit {
                doc,
                revision,
                rope,
                first_line,
                line_delta,
            } => {
                if let Some(entry) = self.docs.get_mut(&doc) {
                    entry
                        .highlighter
                        .invalidate_from(first_line, rope.len_lines(), line_delta);
                    entry.rope = rope;
                    entry.revision = revision;
                    // The lines asked for belong to the old text.
                    entry.pending = false;
                }
            }
            Command::SetDark(dark) => {
                for entry in self.docs.values_mut() {
                    entry.highlighter.set_dark(dark);
                }
                if let Some(entry) = self.active.and_then(|id| self.docs.get_mut(&id)) {
                    entry.pending = entry.view.is_some();
                }
            }
            Command::Close(doc) => {
                self.docs.remove(&doc);
                if self.active == Some(doc) {
                    self.active = None;
                }
            }
            Command::Stop => self.stopped = true,
        }
    }

    /// One time-boxed piece of work for the active document.
    fn work(&mut self) -> Work {
        let Some(id) = self.active else {
            return Work::Idle;
        };
        let Some(doc) = self.docs.get_mut(&id) else {
            return Work::Idle;
        };
        let src = RopeLines(&doc.rope);
        if doc.pending {
            let range = doc.view.clone().unwrap_or(0..0);
            let total = doc.rope.len_lines();
            let first_line = range.start.min(total);
            let deadline = Instant::now() + VIEW_SLICE;
            if doc.highlighter.frontier() < first_line {
                // The parse has not reached the first requested line: run it on.
                doc.highlighter.advance(&src, deadline);
                return Work::Ran;
            }
            return match doc.highlighter.spans(&src, range, deadline) {
                Some(lines) => {
                    doc.pending = false;
                    let _ = self.results.send(Colored {
                        doc: id,
                        revision: doc.revision,
                        dark: doc.highlighter.dark(),
                        first_line,
                        lines,
                    });
                    Work::Sent
                }
                None => Work::Ran,
            };
        }
        if doc.highlighter.done() {
            return Work::Idle;
        }
        doc.highlighter
            .advance(&src, Instant::now() + ADVANCE_SLICE);
        Work::Ran
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn rust() -> Language {
        Language::by_token("rust").unwrap()
    }

    fn generated_rust(lines: usize) -> String {
        let mut s = String::new();
        let mut count = 0;
        let mut n = 0;
        while count < lines {
            let piece = match n % 5 {
                0 => format!("/// Doc for item {n}\n"),
                1 => format!("pub fn item_{n}(a: u32, b: &str) -> u32 {{\n"),
                2 => format!("    let text = \"value {n}\"; // note\n"),
                3 => format!("    if a > {n} {{ return a * 2 + b.len() as u32; }}\n"),
                _ => "    a\n}\n".to_string(),
            };
            count += piece.matches('\n').count();
            s.push_str(&piece);
            n += 1;
        }
        s
    }

    struct Fixture {
        service: HighlightService,
        wakes: Arc<AtomicUsize>,
    }

    fn fixture() -> Fixture {
        let wakes = Arc::new(AtomicUsize::new(0));
        let counter = wakes.clone();
        let service = HighlightService::spawn(move || {
            counter.fetch_add(1, Ordering::SeqCst);
        });
        Fixture { service, wakes }
    }

    fn within(secs: u64) -> Instant {
        Instant::now() + Duration::from_secs(secs)
    }

    fn lines_of(rope: &Rope) -> Vec<String> {
        let src = RopeLines(rope);
        (0..src.len_lines())
            .map(|i| {
                let mut s = String::new();
                src.line(i, &mut s);
                s
            })
            .collect()
    }

    #[test]
    fn rope_lines_strip_lf_crlf_and_cr() {
        assert_eq!(lines_of(&Rope::from_str("a\nb\n")), ["a", "b", ""]);
        assert_eq!(lines_of(&Rope::from_str("a\r\nb\r\n")), ["a", "b", ""]);
        assert_eq!(lines_of(&Rope::from_str("a\rb\rc")), ["a", "b", "c"]);
        assert_eq!(
            lines_of(&Rope::from_str("a\r\nb\nc\rd\r\r\ne")),
            ["a", "b", "c", "d", "", "e"]
        );
        assert_eq!(lines_of(&Rope::from_str("")), [""]);
        assert_eq!(
            lines_of(&Rope::from_str("\u{e7}\u{15f}\r\n\u{540d}")),
            ["\u{e7}\u{15f}", "\u{540d}"]
        );
        // A line spread over several chunks.
        let long = format!("{}\r\n{}", "x".repeat(10_000), "y".repeat(10_000));
        let lines = lines_of(&Rope::from_str(&long));
        assert_eq!(lines[0].len(), 10_000);
        assert_eq!(lines[1].len(), 10_000);
        let mut out = String::from("stale");
        RopeLines(&Rope::from_str("a")).line(5, &mut out);
        assert_eq!(out, "");
    }

    #[test]
    fn the_service_colors_a_requested_range() {
        let f = fixture();
        let text = generated_rust(2000);
        let rope = Rope::from_str(&text);
        let total = rope.len_lines();
        f.service.open(1, 7, rope.clone(), rust(), false);
        f.service.view(1, 7, 1500..1560);
        let got = f.service.wait(within(5));
        assert_eq!(got.len(), 1);
        let colored = &got[0];
        assert_eq!((colored.doc, colored.revision, colored.dark), (1, 7, false));
        assert_eq!(colored.first_line, 1500);
        assert_eq!(colored.lines.len(), 60);
        // The same spans as one pass from the top.
        let mut direct = LineHighlighter::new(rust(), false, total, rope.len_bytes());
        let all = direct
            .spans(
                &RopeLines(&rope),
                0..1560,
                Instant::now() + Duration::from_secs(60),
            )
            .unwrap();
        assert_eq!(colored.lines[..], all[1500..]);
        assert!(f.wakes.load(Ordering::SeqCst) >= 1);
        // A range past the end is clamped.
        f.service.view(1, 7, total - 3..total + 50);
        let got = f.service.wait(within(5));
        assert_eq!(got[0].first_line, total - 3);
        assert_eq!(got[0].lines.len(), 3);
    }

    #[test]
    fn an_edit_recolours_from_the_changed_line() {
        let f = fixture();
        let text = generated_rust(600);
        let rope = Rope::from_str(&text);
        f.service.open(1, 1, rope.clone(), rust(), false);
        f.service.view(1, 1, 280..330);
        assert_eq!(f.service.wait(within(10)).len(), 1);
        // Open a block comment at line 300: every line after it changes colour, and one line
        // is added.
        let mut edited = rope.clone();
        let at = edited.line_to_char(300);
        edited.insert(
            at,
            "/* opened
",
        );
        f.service.edit(1, 2, edited.clone(), 300, 1);
        // The old revision is no longer answered.
        f.service.view(1, 1, 290..320);
        f.service.view(1, 2, 290..320);
        let got = f.service.wait(within(10));
        assert_eq!(got.len(), 1);
        assert_eq!((got[0].revision, got[0].first_line), (2, 290));
        let mut direct =
            LineHighlighter::new(rust(), false, edited.len_lines(), edited.len_bytes());
        let all = direct
            .spans(
                &RopeLines(&edited),
                0..320,
                Instant::now() + Duration::from_secs(60),
            )
            .unwrap();
        assert_eq!(got[0].lines[..], all[290..]);
        // An edit for a document the worker does not hold is ignored.
        f.service.edit(9, 1, Rope::from_str("x"), 0, 0);
        f.service.view(9, 1, 0..1);
        assert!(
            f.service
                .wait(Instant::now() + Duration::from_millis(100))
                .is_empty()
        );
    }

    #[test]
    fn a_stale_revision_gets_no_answer_and_the_current_one_does() {
        let f = fixture();
        let rope = Rope::from_str(&generated_rust(100));
        f.service.open(1, 2, rope, rust(), false);
        f.service.view(1, 1, 0..10);
        f.service.view(1, 3, 0..10);
        assert!(
            f.service
                .wait(Instant::now() + Duration::from_millis(300))
                .is_empty()
        );
        f.service.view(1, 2, 0..10);
        let got = f.service.wait(within(5));
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].revision, 2);
        // Reopening with a new revision makes the old one stale.
        f.service
            .open(1, 3, Rope::from_str("fn a() {}\n"), rust(), false);
        f.service.view(1, 2, 0..1);
        assert!(
            f.service
                .wait(Instant::now() + Duration::from_millis(300))
                .is_empty()
        );
        f.service.view(1, 3, 0..1);
        assert_eq!(f.service.wait(within(5))[0].revision, 3);
    }

    #[test]
    fn a_closed_document_gets_no_answer() {
        let f = fixture();
        f.service
            .open(1, 0, Rope::from_str(&generated_rust(50)), rust(), false);
        f.service.close(1);
        f.service.view(1, 0, 0..10);
        assert!(
            f.service
                .wait(Instant::now() + Duration::from_millis(300))
                .is_empty()
        );
        assert_eq!(f.wakes.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn set_dark_answers_the_view_again_with_the_flag_flipped() {
        let f = fixture();
        f.service
            .open(1, 0, Rope::from_str(&generated_rust(200)), rust(), false);
        f.service.view(1, 0, 0..40);
        let light = f.service.wait(within(5));
        assert_eq!(light.len(), 1);
        assert!(!light[0].dark);
        f.service.set_dark(true);
        let dark = f.service.wait(within(5));
        assert_eq!(dark.len(), 1);
        assert!(dark[0].dark);
        assert_eq!((dark[0].first_line, dark[0].lines.len()), (0, 40));
        assert_ne!(dark[0].lines, light[0].lines);
        f.service.set_dark(false);
        let back = f.service.wait(within(5));
        assert_eq!(back[0], light[0]);
    }

    #[test]
    fn a_document_over_the_limit_is_answered_plain() {
        let f = fixture();
        let big = "let x = 1;\n".repeat(200_000);
        assert!(!Limits::CODE.allows_file(big.len()));
        f.service.open(1, 0, Rope::from_str(&big), rust(), false);
        f.service.view(1, 0, 10..14);
        let got = f.service.wait(within(5));
        assert_eq!(got[0].lines, vec![Vec::<Span>::new(); 4]);
    }

    #[test]
    fn a_later_view_replaces_an_earlier_one_and_other_documents_work() {
        let f = fixture();
        let rope = Rope::from_str(&generated_rust(3000));
        f.service.open(1, 0, rope.clone(), rust(), false);
        f.service
            .open(2, 5, Rope::from_str(&generated_rust(100)), rust(), true);
        f.service.view(1, 0, 2900..2960);
        f.service.view(2, 5, 0..20);
        let mut got = Vec::new();
        let deadline = within(10);
        while got.iter().all(|c: &Colored| c.doc != 2) && Instant::now() < deadline {
            got.extend(f.service.wait(deadline));
        }
        let second = got.iter().find(|c| c.doc == 2).expect("doc 2 answered");
        assert!(second.dark);
        assert_eq!(second.lines.len(), 20);
        // The replaced request for doc 1 is not answered.
        assert!(got.iter().all(|c| c.doc != 1));
        f.service.view(1, 0, 2900..2960);
        let got = f.service.wait(within(10));
        assert_eq!(got[0].doc, 1);
    }

    #[test]
    fn an_idle_worker_sends_nothing_and_does_not_wake() {
        let f = fixture();
        let rope = Rope::from_str(&generated_rust(300));
        let total = rope.len_lines();
        f.service.open(1, 0, rope, rust(), false);
        // A view of the last lines makes the worker parse the whole text first.
        f.service.view(1, 0, total - 10..total);
        assert_eq!(f.service.wait(within(10)).len(), 1);
        // Let the background parse finish, then nothing may be sent or woken.
        std::thread::sleep(Duration::from_millis(100));
        let before = f.wakes.load(Ordering::SeqCst);
        assert_eq!(before, 1);
        assert!(
            f.service
                .wait(Instant::now() + Duration::from_millis(300))
                .is_empty()
        );
        assert_eq!(f.wakes.load(Ordering::SeqCst), before);
        assert!(f.service.take().is_empty());
    }

    #[test]
    fn dropping_the_service_stops_the_worker() {
        let wakes = Arc::new(AtomicUsize::new(0));
        let counter = wakes.clone();
        let service = HighlightService::spawn(move || {
            counter.fetch_add(1, Ordering::SeqCst);
        });
        service.open(1, 0, Rope::from_str("fn a() {}\n"), rust(), false);
        drop(service);
        // The closure was the worker's only other holder of the counter; once the thread ends it
        // is dropped.
        let deadline = within(5);
        while Arc::strong_count(&wakes) > 1 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(Arc::strong_count(&wakes), 1);
    }
}
