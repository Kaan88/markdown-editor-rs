//! The list of a review's changes (`docs/ai-library-plan.md` 5.5, D-077): a header per change with
//! its tick, the old lines and the new ones under it with their numbers, three lines of context,
//! unchanged runs folded, a scrollbar, and long lines wrapped inside the text column instead of
//! cut. The words that changed inside a changed pair of lines are tinted. A change that is not
//! ticked is drawn dimmed.
//!
//! Like `plan_ui.rs` it knows nothing of `App`: [`draw`] draws into any rectangle and says what
//! can be clicked there, [`Layout`] is made from the changes and a way to measure text, and
//! wrapping ([`wrap`]) needs no window, so it is tested headless.

use mdedit_core::hunks::{Hunks, Seg};
use mdshared_ui::check::{TICK, Tick, draw_tick};
use mdshared_ui::gfx::{Face, Painter, Rect, mix};
use mdshared_ui::scroll::{draw_scrollbar, scrollbar};
use mdshared_ui::theme::Palette;
use std::ops::Range;

/// A line of the list.
pub const LINE_H: f32 = 20.0;
/// A change's header.
pub const HEAD_H: f32 = 30.0;
/// Unchanged lines kept on each side of a change; longer runs fold.
pub const CONTEXT: usize = 3;
const NUM_W: f32 = 44.0;
/// Where the text starts, from the list's left edge.
const TEXT_DX: f32 = 128.0;
const TEXT_SIZE: f32 = 13.0;
/// Room kept at the right for the scrollbar.
const RIGHT_PAD: f32 = 22.0;
const TOP_PAD: f32 = 4.0;
const BOTTOM_PAD: f32 = 12.0;
/// The most characters measured at once when a line is wrapped, so a line of 200 KB is not
/// shaped whole for every row.
const WINDOW: usize = 512;

/// What can be clicked in the list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Hit {
    /// The tick box of a change.
    Tick(usize),
    /// Anywhere else on a change: it becomes the current one.
    Change(usize),
    /// The scrollbar's track.
    Track,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Same,
    Removed,
    Added,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Item {
    Fold {
        lines: usize,
    },
    Head {
        change: usize,
    },
    Line {
        change: Option<usize>,
        kind: Kind,
        old: Option<usize>,
        new: Option<usize>,
        /// The text as drawn (tabs as spaces).
        text: String,
        /// The rows the text wraps into: byte ranges of `text`.
        rows: Vec<Range<usize>>,
        /// The words that changed, a byte range of `text`.
        intra: Option<Range<usize>>,
    },
}

/// An item and where it is, from the list's top, in logical px.
#[derive(Clone, Debug)]
pub struct Placed {
    pub y: f32,
    pub h: f32,
    pub item: Item,
}

/// Every item of the list with its place, for one text width.
#[derive(Clone, Debug, Default)]
pub struct Layout {
    pub items: Vec<Placed>,
    pub total: f32,
    /// The top of each change's header, and the bottom of its last line.
    pub tops: Vec<f32>,
    pub ends: Vec<f32>,
}

/// The rows `text` takes in a column `width` wide: byte ranges, each fitting when it can. A row
/// breaks after a space when there is one in it, in the middle of a word when there is not. Never
/// empty: an empty text is one empty row.
pub fn wrap(text: &str, width: f32, measure: &mut dyn FnMut(&str) -> f32) -> Vec<Range<usize>> {
    let mut rows = Vec::new();
    let mut start = 0;
    while start < text.len() {
        let rest = &text[start..];
        // The window: at most WINDOW characters from here.
        let bounds: Vec<usize> = rest
            .char_indices()
            .map(|(i, _)| i)
            .skip(1)
            .take(WINDOW)
            .chain(std::iter::once(rest.len()))
            .collect();
        let whole = bounds.last().copied() == Some(rest.len()) && bounds.len() <= WINDOW;
        if whole && measure(rest) <= width {
            rows.push(start..text.len());
            return rows;
        }
        // The longest prefix of the window that fits, one character at least.
        let (mut lo, mut hi) = (0, bounds.len() - 1);
        while lo < hi {
            let mid = (lo + hi).div_ceil(2);
            if measure(&rest[..bounds[mid]]) <= width {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }
        let mut end = bounds[lo];
        if end < rest.len() {
            if let Some(c) = rest[end..].chars().next().filter(|c| c.is_whitespace()) {
                // The row ends where a word does: the space stays with it, out of sight.
                end += c.len_utf8();
            } else if let Some(sp) = rest[..end]
                .char_indices()
                .rev()
                .find(|(_, c)| c.is_whitespace())
                .map(|(i, c)| i + c.len_utf8())
                .filter(|&sp| sp > 0)
            {
                // Back to just after the last space in the row.
                end = sp;
            }
        }
        rows.push(start..start + end);
        start += end;
    }
    if rows.is_empty() {
        rows.push(0..0);
    }
    rows
}

/// `text` with its tabs as four spaces, and where `range` of it (if any) is in the result.
fn expand(text: &str, range: Option<Range<usize>>) -> (String, Option<Range<usize>>) {
    if !text.contains('\t') {
        return (text.to_string(), range);
    }
    let mut out = String::with_capacity(text.len() + 8);
    let (mut a, mut b) = (None, None);
    for (i, c) in text.char_indices() {
        if let Some(r) = &range {
            if i == r.start {
                a = Some(out.len());
            }
            if i == r.end {
                b = Some(out.len());
            }
        }
        if c == '\t' {
            out.push_str("    ");
        } else {
            out.push(c);
        }
    }
    if let Some(r) = &range {
        if r.end >= text.len() {
            b = Some(out.len());
        }
        if r.start >= text.len() {
            a = Some(out.len());
        }
    }
    (out, a.zip(b).map(|(a, b)| a..b.max(a)))
}

impl Layout {
    /// The list of `h` for a text column `text_w` wide.
    pub fn build(h: &Hunks, text_w: f32, measure: &mut dyn FnMut(&str) -> f32) -> Layout {
        let mut lay = Layout {
            tops: vec![0.0; h.len()],
            ends: vec![0.0; h.len()],
            ..Layout::default()
        };
        let mut y = TOP_PAD;
        let segs = h.segments();
        let n = segs.len();
        let put = |lay: &mut Layout, y: &mut f32, item: Item, rows: usize, row_h: f32| {
            let hh = rows.max(1) as f32 * row_h;
            lay.items.push(Placed { y: *y, h: hh, item });
            *y += hh;
        };
        for (k, seg) in segs.iter().enumerate() {
            match seg {
                Seg::Same { a, b } => {
                    let len = a.len();
                    let lead = if k == 0 { 0 } else { CONTEXT };
                    let tail = if k + 1 == n { 0 } else { CONTEXT };
                    let line =
                        |i: usize| (h.old_line(a.start + i), a.start + i + 1, b.start + i + 1);
                    let mut push = |lay: &mut Layout, y: &mut f32, i: usize| {
                        let (t, o, nw) = line(i);
                        let (text, _) = expand(&t, None);
                        let rows = wrap(&text, text_w, measure);
                        let r = rows.len();
                        put(
                            lay,
                            y,
                            Item::Line {
                                change: None,
                                kind: Kind::Same,
                                old: Some(o),
                                new: Some(nw),
                                text,
                                rows,
                                intra: None,
                            },
                            r,
                            LINE_H,
                        );
                    };
                    if len > lead + tail + 1 {
                        for i in 0..lead {
                            push(&mut lay, &mut y, i);
                        }
                        put(
                            &mut lay,
                            &mut y,
                            Item::Fold {
                                lines: len - lead - tail,
                            },
                            1,
                            LINE_H,
                        );
                        for i in len - tail..len {
                            push(&mut lay, &mut y, i);
                        }
                    } else {
                        for i in 0..len {
                            push(&mut lay, &mut y, i);
                        }
                    }
                }
                Seg::Change(i) => {
                    let Some(c) = h.change(*i) else { continue };
                    lay.tops[*i] = y;
                    put(&mut lay, &mut y, Item::Head { change: *i }, 1, HEAD_H);
                    for (j, l) in c.a.clone().enumerate() {
                        let intra = h.intra(*i, j).map(|(a, _)| a);
                        let (text, intra) = expand(&h.old_line(l), intra);
                        let rows = wrap(&text, text_w, measure);
                        let r = rows.len();
                        let item = Item::Line {
                            change: Some(*i),
                            kind: Kind::Removed,
                            old: Some(l + 1),
                            new: None,
                            text,
                            rows,
                            intra,
                        };
                        put(&mut lay, &mut y, item, r, LINE_H);
                    }
                    for (j, l) in c.b.clone().enumerate() {
                        let intra = h.intra(*i, j).map(|(_, b)| b);
                        let (text, intra) = expand(&h.new_line(l), intra);
                        let rows = wrap(&text, text_w, measure);
                        let r = rows.len();
                        let item = Item::Line {
                            change: Some(*i),
                            kind: Kind::Added,
                            old: None,
                            new: Some(l + 1),
                            text,
                            rows,
                            intra,
                        };
                        put(&mut lay, &mut y, item, r, LINE_H);
                    }
                    lay.ends[*i] = y;
                }
            }
        }
        lay.total = y + BOTTOM_PAD;
        lay
    }

    /// The index of the first item that ends below `y`.
    fn first_at(&self, y: f32) -> usize {
        self.items.partition_point(|p| p.y + p.h <= y)
    }
}

/// The word at the right of a change's header: what an untaken change leaves, and how the current
/// one is ticked. Nothing for the others.
pub fn header_note(taken: bool, current: bool) -> Option<&'static str> {
    if !taken {
        Some("not taken · the old text stays")
    } else if current {
        Some("Space ticks or unticks")
    } else {
        None
    }
}

#[cfg(test)]
thread_local! {
    /// What the last `draw` wrote at each change's header's right, for the tests: the change and
    /// its note. Cleared with every draw.
    static NOTES: std::cell::RefCell<Vec<(usize, Option<&'static str>)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// The note the last draw wrote at change `i`'s header: `None` when the header was not drawn.
#[cfg(test)]
pub fn header_note_drawn(i: usize) -> Option<Option<&'static str>> {
    NOTES.with(|n| {
        n.borrow()
            .iter()
            .find(|(c, _)| *c == i)
            .map(|(_, note)| *note)
    })
}

/// What the list keeps between frames.
#[derive(Default)]
pub struct ListState {
    /// How far it is scrolled, logical px.
    pub scroll: f32,
    /// The list's height and its content's, as last drawn.
    pub view: f32,
    pub content: f32,
    /// The change the keys act on.
    pub current: usize,
    /// Where the list was drawn, for the scrollbar's drag.
    pub area: Rect,
    /// A change to scroll to when the list is next drawn (its layout may not be made yet).
    go_to: Option<usize>,
    layout: Option<Layout>,
    key: (u32, u32, u64),
}

impl ListState {
    /// Makes change `i` the current one and scrolls it to the top at the next frame.
    pub fn go_to(&mut self, i: usize) {
        self.current = i;
        self.go_to = Some(i);
    }

    /// Whether the header of change `i` is in view, as last drawn.
    #[cfg(test)]
    pub fn visible(&self, i: usize) -> bool {
        let Some(top) = self.layout.as_ref().and_then(|l| l.tops.get(i)) else {
            return false;
        };
        *top >= self.scroll && top + HEAD_H <= self.scroll + self.view
    }

    /// The layout as last drawn.
    #[cfg(test)]
    pub fn layout(&self) -> Option<&Layout> {
        self.layout.as_ref()
    }

    pub fn max_scroll(&self) -> f32 {
        (self.content - self.view).max(0.0)
    }

    pub fn scroll_by(&mut self, dy: f32) {
        self.scroll = (self.scroll + dy).clamp(0.0, self.max_scroll());
    }
}

/// `r` cut to `area`.
fn clip_to(r: Rect, area: Rect) -> Rect {
    let (x, y) = (r.x.max(area.x), r.y.max(area.y));
    let (right, bottom) = (r.right().min(area.right()), r.bottom().min(area.bottom()));
    Rect::new(x, y, (right - x).max(0.0), (bottom - y).max(0.0))
}

/// The colours of removed and added lines, and their marks: (removed fill, added fill, removed
/// mark, added mark).
pub fn tints(pal: &Palette) -> ([u8; 3], [u8; 3], [u8; 3], [u8; 3]) {
    if pal.dark {
        (
            [0x4a, 0x23, 0x23],
            [0x1f, 0x3d, 0x29],
            [0xf0, 0x8a, 0x80],
            [0x7f, 0xd4, 0x97],
        )
    } else {
        (
            [0xfd, 0xec, 0xea],
            [0xe6, 0xf4, 0xea],
            [0xb4, 0x23, 0x18],
            [0x1a, 0x7f, 0x37],
        )
    }
}

/// The scrollbar's track and thumb for `area`, when the list is taller than it.
pub fn bar(area: Rect, st: &ListState) -> Option<(Rect, Rect)> {
    scrollbar(area, st.content, area.h, st.scroll)
}

/// Draws the list into `area` and pushes the places that can be clicked, the later ones over the
/// earlier. `ticked` says whether change `i` is ticked.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    p: &mut Painter,
    pal: &Palette,
    h: &Hunks,
    st: &mut ListState,
    area: Rect,
    hover: Option<Hit>,
    hits: &mut Vec<(Hit, Rect)>,
) {
    #[cfg(test)]
    NOTES.with(|n| n.borrow_mut().clear());
    let text_w = (area.w - TEXT_DX - RIGHT_PAD).max(40.0);
    // The width, the scale, and which comparison: the face and size measured are fixed, but a
    // list made from another `Hunks` (even one of the same size) must never be indexed by it.
    let key = ((text_w * 10.0) as u32, (p.px() * 1000.0) as u32, h.stamp());
    if st.layout.is_none() || st.key != key {
        let lay = Layout::build(h, text_w, &mut |s| p.measure(Face::Ui, TEXT_SIZE, s));
        st.layout = Some(lay);
        st.key = key;
    }
    let Some(lay) = st.layout.as_ref() else {
        return;
    };
    st.view = area.h;
    st.area = area;
    st.content = lay.total;
    if let Some(i) = st.go_to.take() {
        let top = lay.tops.get(i).copied().unwrap_or(0.0);
        st.scroll = (top - 8.0).max(0.0);
    }
    st.scroll = st.scroll.clamp(0.0, (st.content - st.view).max(0.0));
    let scroll = st.scroll;
    let current = st.current;
    let (minus_bg, plus_bg, minus, plus) = tints(pal);
    let old = p.clip(area);
    // The current change's stripe, under everything.
    let mut block_hits = Vec::new();
    for i in 0..h.len() {
        let (top, end) = (lay.tops[i] - scroll, lay.ends[i] - scroll);
        if end < 0.0 || top > area.h {
            continue;
        }
        let r = Rect::new(area.x, area.y + top, area.w - 12.0, end - top);
        block_hits.push((Hit::Change(i), clip_to(r, area)));
    }
    let first = lay.first_at(scroll);
    let num_right = |col: usize| area.x + (col + 1) as f32 * NUM_W - 4.0;
    let text_x = area.x + TEXT_DX;
    let mut tick_hits = Vec::new();
    for placed in lay.items.iter().skip(first) {
        let y = area.y + placed.y - scroll;
        if y > area.bottom() {
            break;
        }
        let row = Rect::new(area.x, y, area.w, placed.h);
        match &placed.item {
            Item::Fold { lines } => {
                p.fill(
                    Rect::new(row.x, row.y + 2.0, row.w, row.h - 4.0),
                    mix(pal.popup, pal.text, 0.05),
                );
                let s = if *lines == 1 { "" } else { "s" };
                p.text(
                    Face::Ui,
                    12.0,
                    text_x,
                    row.cy(),
                    &format!("··· {lines} unchanged line{s}"),
                    pal.muted,
                    area.right() - text_x - 12.0,
                );
            }
            Item::Head { change } => {
                let i = *change;
                let on = h.is_ticked(i);
                let cur = i == current;
                let bg = if cur {
                    mix(pal.popup, pal.accent, 0.13)
                } else {
                    mix(pal.popup, pal.text, 0.05)
                };
                p.fill(row, bg);
                let bx = Rect::new(area.x + 18.0, row.cy() - TICK / 2.0, TICK, TICK);
                draw_tick(p, pal, bx, Tick::new(on));
                let hb = Rect::new(bx.x - 6.0, row.y, TICK + 12.0, row.h);
                tick_hits.push((Hit::Tick(i), clip_to(hb, area)));
                let Some(c) = h.change(i) else { continue };
                let mut x = bx.right() + 12.0;
                let label = format!("Change {}", i + 1);
                x += p.text(Face::Semibold, 12.5, x, row.cy(), &label, pal.text, 200.0) + 12.0;
                let lines = if c.a.is_empty() {
                    format!("after line {}", c.a.start)
                } else if c.a.len() == 1 {
                    format!("line {}", c.a.start + 1)
                } else {
                    format!("lines {}–{}", c.a.start + 1, c.a.end)
                };
                x += p.text(Face::Ui, 12.0, x, row.cy(), &lines, pal.muted, 200.0) + 12.0;
                if c.added() > 0 {
                    x += p.text(
                        Face::Semibold,
                        12.0,
                        x,
                        row.cy(),
                        &format!("+{}", c.added()),
                        plus,
                        80.0,
                    ) + 8.0;
                }
                if c.removed() > 0 {
                    p.text(
                        Face::Semibold,
                        12.0,
                        x,
                        row.cy(),
                        &format!("−{}", c.removed()),
                        minus,
                        80.0,
                    );
                }
                let note = header_note(on, cur);
                #[cfg(test)]
                NOTES.with(|n| n.borrow_mut().push((i, note)));
                if let Some(note) = note {
                    p.text_right(
                        Face::Ui,
                        12.0,
                        area.right() - RIGHT_PAD + 6.0,
                        row.cy(),
                        note,
                        pal.faint,
                    );
                }
            }
            Item::Line {
                change,
                kind,
                old: o,
                new: n,
                text,
                rows,
                intra,
            } => {
                let taken = change.is_none_or(|c| h.is_ticked(c));
                let (bg, mark_c, mark) = match kind {
                    Kind::Same => (None, pal.text, ""),
                    Kind::Removed => (Some(minus_bg), minus, "−"),
                    Kind::Added => (Some(plus_bg), plus, "+"),
                };
                // A change not taken is dimmed: its colours half way to the plain ground.
                let fade = |c: [u8; 3]| if taken { c } else { mix(pal.popup, c, 0.45) };
                if let Some(bg) = bg {
                    p.fill(row, fade(bg));
                }
                let cy = y + LINE_H / 2.0;
                let num_c = if taken {
                    pal.faint
                } else {
                    mix(pal.popup, pal.faint, 0.7)
                };
                if let Some(o) = o {
                    p.text_right(Face::Ui, 11.5, num_right(0), cy, &o.to_string(), num_c);
                }
                if let Some(n) = n {
                    p.text_right(Face::Ui, 11.5, num_right(1), cy, &n.to_string(), num_c);
                }
                if !mark.is_empty() {
                    p.text_center(
                        Face::Semibold,
                        13.0,
                        Rect::new(area.x + 2.0 * NUM_W + 2.0, y, 24.0, LINE_H),
                        mark,
                        fade(mark_c),
                    );
                }
                let fg = if taken || bg.is_none() {
                    pal.text
                } else {
                    pal.muted
                };
                let width = area.right() - text_x - RIGHT_PAD + 10.0;
                for (k, r) in rows.iter().enumerate() {
                    let ry = y + k as f32 * LINE_H;
                    if ry + LINE_H < area.y || ry > area.bottom() {
                        continue;
                    }
                    let part = &text[r.clone()];
                    if let (Some(span), Some(bg)) = (intra.as_ref(), bg)
                        && taken
                    {
                        let (s, e) = (span.start.max(r.start), span.end.min(r.end));
                        if s < e
                            || (span.is_empty() && span.start >= r.start && span.start <= r.end)
                        {
                            let x0 = text_x
                                + p.measure(Face::Ui, TEXT_SIZE, &text[r.start..s.max(r.start)]);
                            let x1 =
                                text_x + p.measure(Face::Ui, TEXT_SIZE, &text[r.start..e.max(s)]);
                            p.rbox(
                                Rect::new(
                                    x0 - 1.0,
                                    ry + 2.0,
                                    (x1 - x0 + 2.0).max(3.0),
                                    LINE_H - 4.0,
                                ),
                                3.0,
                                mix(bg, mark_c, 0.3),
                            );
                        }
                    }
                    p.text(
                        Face::Ui,
                        TEXT_SIZE,
                        text_x,
                        ry + LINE_H / 2.0,
                        part,
                        fg,
                        width + 200.0,
                    );
                }
            }
        }
    }
    // The current change's left stripe, over the rows.
    if current < h.len() {
        let (top, end) = (lay.tops[current] - scroll, lay.ends[current] - scroll);
        if end > 0.0 && top < area.h {
            p.fill(Rect::new(area.x, area.y + top, 3.0, end - top), pal.accent);
        }
    }
    p.restore(old);
    hits.extend(block_hits);
    hits.extend(tick_hits);
    if let Some((track, thumb)) = bar(area, st) {
        let active = hover == Some(Hit::Track);
        draw_scrollbar(p, pal, track, thumb, active);
        hits.push((Hit::Track, track));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One unit per character: a column `w` wide holds `w` characters.
    fn m(s: &str) -> f32 {
        s.chars().count() as f32
    }

    #[test]
    fn a_long_line_wraps_into_rows_that_hold_all_of_it() {
        let text = "the quick brown fox jumps over the lazy dog ".repeat(7);
        let rows = wrap(&text, 30.0, &mut |s| m(s));
        assert!(rows.len() > 8);
        let joined: String = rows.iter().map(|r| &text[r.clone()]).collect();
        assert_eq!(joined, text, "nothing is lost");
        assert!(rows.iter().all(|r| m(text[r.clone()].trim_end()) <= 30.0));
        // Rows break after a space.
        assert!(text[rows[0].clone()].ends_with(' '));
    }

    #[test]
    fn a_word_longer_than_the_column_breaks_inside_it() {
        let text = "x".repeat(100);
        let rows = wrap(&text, 30.0, &mut |s| m(s));
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0], 0..30);
    }

    #[test]
    fn a_300_character_line_is_drawn_whole() {
        let text: String = (0..300)
            .map(|i| char::from(b'a' + (i % 26) as u8))
            .collect();
        let rows = wrap(&text, 100.0, &mut |s| m(s));
        let joined: String = rows.iter().map(|r| &text[r.clone()]).collect();
        assert_eq!(joined, text);
        assert_eq!(rows.len(), 3);
    }

    #[test]
    fn wrapping_cuts_between_characters_and_an_empty_line_is_one_row() {
        let text = "çğüşöı".repeat(20);
        let rows = wrap(&text, 7.0, &mut |s| m(s));
        for r in &rows {
            assert!(text.is_char_boundary(r.start) && text.is_char_boundary(r.end));
        }
        assert_eq!(wrap("", 10.0, &mut |s| m(s)), vec![0..0]);
        assert_eq!(wrap("ab", 0.5, &mut |s| m(s)).len(), 2);
    }

    #[test]
    fn a_huge_line_is_wrapped_in_windows() {
        let text = "word ".repeat(20_000);
        let mut calls = 0;
        let rows = wrap(&text, 80.0, &mut |s| {
            calls += 1;
            assert!(s.chars().count() <= WINDOW + 1, "never the whole line");
            m(s)
        });
        let joined: String = rows.iter().map(|r| &text[r.clone()]).collect();
        assert_eq!(joined, text);
        assert!(calls > 0);
    }

    #[test]
    fn tabs_are_drawn_as_spaces_and_the_tinted_words_follow() {
        let (t, r) = expand("a\tb\tc", Some(2..3));
        assert_eq!(t, "a    b    c");
        assert_eq!(r, Some(5..6));
        let (t, r) = expand("a\tb", Some(2..3));
        assert_eq!((t.as_str(), r), ("a    b", Some(5..6)));
    }

    fn lay(old: &str, new: &str, w: f32) -> (Hunks, Layout) {
        let h = Hunks::new(old, new);
        let l = Layout::build(&h, w, &mut |s| m(s));
        (h, l)
    }

    /// What the review of the first release drew: three lines of context around a change, a fold
    /// for the rest, the old lines and then the new ones with their own numbers.
    #[test]
    fn context_folds_and_numbers_as_the_review_always_had() {
        let old: String = (1..=12).map(|i| format!("l{i}\n")).collect();
        let new = old.replace("l6\n", "six\nsix and a half\n");
        let (_, l) = lay(&old, &new, 80.0);
        let kinds: Vec<String> = l
            .items
            .iter()
            .map(|p| match &p.item {
                Item::Fold { lines } => format!("fold {lines}"),
                Item::Head { change } => format!("head {change}"),
                Item::Line { kind, old, new, .. } => format!("{kind:?} {old:?} {new:?}"),
            })
            .collect();
        assert_eq!(kinds[0], "fold 2", "{kinds:?}");
        assert_eq!(kinds[1], "Same Some(3) Some(3)");
        assert_eq!(kinds[4], "head 0");
        assert_eq!(kinds[5], "Removed Some(6) None");
        assert_eq!(kinds[6], "Added None Some(6)");
        assert_eq!(kinds[7], "Added None Some(7)");
        assert_eq!(kinds[8], "Same Some(7) Some(8)");
        assert!(kinds.contains(&"fold 3".to_string()));
    }

    #[test]
    fn a_wrapped_line_makes_its_row_taller_and_the_sums_follow() {
        let long = "word ".repeat(40);
        let (_, l) = lay("a\nb\n", &format!("a\n{long}\n"), 40.0);
        let line = l
            .items
            .iter()
            .find(|p| {
                matches!(
                    &p.item,
                    Item::Line {
                        kind: Kind::Added,
                        ..
                    }
                )
            })
            .expect("the added line");
        assert!(line.h > LINE_H * 4.0, "{}", line.h);
        let last = l.items.last().unwrap();
        assert_eq!(l.total, last.y + last.h + BOTTOM_PAD);
        assert_eq!(l.ends[0], last.y + last.h);
    }

    #[test]
    fn the_changed_words_of_a_pair_are_marked() {
        let (_, l) = lay("one two three\n", "one 2 three\n", 80.0);
        let spans: Vec<_> = l
            .items
            .iter()
            .filter_map(|p| match &p.item {
                Item::Line { text, intra, .. } => Some((text.clone(), intra.clone())),
                _ => None,
            })
            .collect();
        assert_eq!(&spans[0].0[spans[0].1.clone().unwrap()], "two");
        assert_eq!(&spans[1].0[spans[1].1.clone().unwrap()], "2");
    }

    /// The first review's test, kept: a long unchanged run folds, the old lines come before the
    /// new ones, and the last line is the new "Z" numbered 10.
    #[test]
    fn the_review_folds_long_unchanged_runs() {
        let before = "a
1
2
3
4
5
6
7
8
z
";
        let after = "A
1
2
3
4
5
6
7
8
Z
";
        let (h, l) = lay(before, after, 80.0);
        assert_eq!(h.totals(), (2, 2));
        let items: Vec<&Item> = l.items.iter().map(|p| &p.item).collect();
        assert!(matches!(items[0], Item::Head { change: 0 }));
        let line = |k: usize| match items[k] {
            Item::Line {
                kind,
                old,
                new,
                text,
                ..
            } => (*kind, *old, *new, text.clone()),
            other => panic!("{other:?}"),
        };
        assert_eq!(line(1), (Kind::Removed, Some(1), None, "a".into()));
        assert_eq!(line(2), (Kind::Added, None, Some(1), "A".into()));
        assert_eq!(line(3), (Kind::Same, Some(2), Some(2), "1".into()));
        assert!(items.contains(&&Item::Fold { lines: 2 }));
        assert_eq!(
            line(items.len() - 1),
            (Kind::Added, None, Some(10), "Z".into())
        );
    }

    #[test]
    fn the_scroll_follows_the_current_change_and_stays_in_range() {
        let mut st = ListState {
            view: 100.0,
            content: 400.0,
            ..ListState::default()
        };
        st.scroll_by(1000.0);
        assert_eq!(st.scroll, 300.0);
        st.scroll_by(-1000.0);
        assert_eq!(st.scroll, 0.0);
    }

    #[test]
    fn a_change_not_taken_says_the_old_text_stays() {
        assert_eq!(
            header_note(false, false),
            Some("not taken · the old text stays")
        );
        assert_eq!(
            header_note(false, true),
            Some("not taken · the old text stays")
        );
        assert_eq!(header_note(true, true), Some("Space ticks or unticks"));
        assert_eq!(header_note(true, false), None);
    }
}
