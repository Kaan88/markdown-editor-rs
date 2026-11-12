//! The highlighting worker. One thread per window holds a
//! [`LineHighlighter`](mdshared_highlight::incremental::LineHighlighter) per open document and a
//! rope snapshot of its text. The UI thread sends it commands (open, view, theme, close) and
//! takes coloured lines back; the worker calls the `wake` closure after it sends, which the shell
//! turns into a winit user event. When it has nothing to do it blocks on its command channel, so
//! an idle window costs no CPU here.
//!
//! Order of work: the latest `view` request of the most recently viewed document first; then that
//! document's parse forward to the end of the text, in slices of about 2 ms with the command
//! channel checked between slices. Other documents wait until they are viewed.

pub use mdshared_highlight::incremental::{Language, Limits, Span, foreground};

use mdshared_highlight::incremental::Lines;
use ropey::Rope;
use std::ops::Range;
use std::time::Instant;

/// The shell's identifier for an open document (one per tab, kept across reloads).
pub type DocId = u64;

/// [`Lines`] over a rope: line text without its LF, CRLF or CR.
pub struct RopeLines<'a>(pub &'a Rope);

impl Lines for RopeLines<'_> {
    fn len_lines(&self) -> usize {
        todo!()
    }

    fn line(&self, index: usize, out: &mut String) {
        let _ = (index, out);
        todo!()
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

pub struct HighlightService {
    _private: (),
}

impl HighlightService {
    /// Starts the worker thread. `wake` is called from it after results are queued (at most once
    /// per batch); the shell sends itself a winit user event from it.
    pub fn spawn(wake: impl Fn() + Send + 'static) -> HighlightService {
        let _ = wake;
        todo!()
    }

    /// Starts (or restarts, for a reload) highlighting `text` as `language`. A document whose
    /// text is over the language's file limit is accepted and never coloured. Replaces any state
    /// the worker held for `doc`.
    pub fn open(&self, doc: DocId, revision: u64, text: Rope, language: Language, dark: bool) {
        let _ = (doc, revision, text, language, dark);
        todo!()
    }

    /// The lines the shell wants colours for (it passes the visible lines plus a screen above and
    /// below). Replaces the previous request; the worker answers with one [`Colored`] covering
    /// them as soon as its parse has reached them.
    pub fn view(&self, doc: DocId, revision: u64, lines: Range<usize>) {
        let _ = (doc, revision, lines);
        todo!()
    }

    /// Switches every document's theme; the current view request is answered again.
    pub fn set_dark(&self, dark: bool) {
        let _ = dark;
        todo!()
    }

    /// Drops the worker's state for `doc`.
    pub fn close(&self, doc: DocId) {
        let _ = doc;
        todo!()
    }

    /// Results queued since the last call, oldest first. Never blocks.
    pub fn take(&self) -> Vec<Colored> {
        todo!()
    }

    /// Like [`take`](Self::take), but waits until at least one result is queued or `deadline`
    /// passes. For the first frame and for `--shot`.
    pub fn wait(&self, deadline: Instant) -> Vec<Colored> {
        let _ = deadline;
        todo!()
    }
}

impl Drop for HighlightService {
    /// Tells the worker to stop; does not wait for it.
    fn drop(&mut self) {}
}
