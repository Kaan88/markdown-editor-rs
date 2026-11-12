//! Literal find over the rope, walking its chunks without copying the buffer. Case-insensitive by
//! default with the viewer's folding (`mdview-core/src/find.rs`, `fold` and `chars_match`: simple
//! one-to-one lower-casing, U+00A0 read as a space, and the Turkish i forms kept apart so that `i`
//! matches `I` and `İ`, and `I` matches `i` and `ı`), or exact with `match_case`. A line break in the query (CRLF, LF or CR) matches any one line
//! break in the text. Matches never overlap and are reported in buffer order.

use ropey::Rope;
use std::ops::Range;
use std::sync::atomic::AtomicBool;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FindOptions {
    pub match_case: bool,
}

/// A prepared query.
#[derive(Clone, Debug)]
pub struct Query {
    text: String,
    options: FindOptions,
}

impl Query {
    /// `None` for an empty query.
    pub fn new(text: &str, options: FindOptions) -> Option<Query> {
        let _ = (text, options);
        todo!()
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn options(&self) -> FindOptions {
        self.options
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Matches {
    /// Byte ranges in buffer order, at most `cap` of them.
    pub ranges: Vec<Range<usize>>,
    /// A limit stopped the search: `cap` matches, or `FIND_WORK_PER_BYTE` spent. The real count
    /// may be higher.
    pub capped: bool,
    /// The `cancel` flag stopped the search; `ranges` is what was found before.
    pub cancelled: bool,
}

/// Every match from the start, up to `cap`. Checks `cancel` at least every 64 KiB.
pub fn find_all(rope: &Rope, query: &Query, cap: usize, cancel: &AtomicBool) -> Matches {
    let _ = (rope, query, cap, cancel);
    todo!()
}

/// The first match starting at or after `from` (forward), or the last one ending at or before
/// `from` (backward), wrapping around the buffer's end when `wrap` is set. For F3 past the last
/// counted match of a capped search.
pub fn find_next(
    rope: &Rope,
    query: &Query,
    from: usize,
    forward: bool,
    wrap: bool,
    cancel: &AtomicBool,
) -> Option<Range<usize>> {
    let _ = (rope, query, from, forward, wrap, cancel);
    todo!()
}

/// Index of the first match in `ranges` that starts at or after `offset` (the one F3 goes to
/// from the caret), or `ranges.len()`.
pub fn first_at_or_after(ranges: &[Range<usize>], offset: usize) -> usize {
    ranges.partition_point(|r| r.start < offset)
}
