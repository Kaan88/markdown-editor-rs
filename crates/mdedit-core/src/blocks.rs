//! Markdown blocks for the format bar: lists, block quotes, alerts, fenced blocks (code, math,
//! diagrams, details), thematic breaks and footnotes, plus [`line_state`], what the bar shows as
//! pressed for the caret's line.
//!
//! Each command takes one selection and is one undo step, like the line commands of
//! [`crate::lines`]: the selection comes back unchanged, with no change and no history entry,
//! when nothing changes. They read the touched lines only (each line up to [`LINE_SCAN`] bytes),
//! except [`cycle_alert`], which looks [`QUOTE_SCAN`] lines up and down for the edges of the
//! quote the caret is in, and [`footnote`], which reads a document of up to
//! [`FOOTNOTE_MAX_BYTES`] for the numbers already used.

use crate::buffer::Buffer;
use crate::edit::{Outcome, clamped, nothing, touched_lines};
use crate::history::History;
use crate::markdown::prefix_len;
use crate::selection::Selection;
use crate::typing::{Act, Edit, run};

/// The most bytes of a line the commands read; a longer line is read as its first bytes.
pub const LINE_SCAN: usize = 4096;
/// How many lines [`cycle_alert`] looks up and down for the edges of the quote around the caret.
pub const QUOTE_SCAN: usize = 2000;
/// The largest document [`footnote`] numbers; it does nothing in a larger one.
pub const FOOTNOTE_MAX_BYTES: usize = 4 << 20;

/// The kinds of list a line can be made.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ListKind {
    /// `- `
    Bullet,
    /// `1. `, `2. `, ...
    Numbered,
    /// `- [ ] `
    Task,
}

/// GitHub's alert kinds, in the order [`cycle_alert`] steps through them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlertKind {
    Note,
    Tip,
    Important,
    Warning,
    Caution,
}

impl AlertKind {
    pub const ALL: [AlertKind; 5] = [
        AlertKind::Note,
        AlertKind::Tip,
        AlertKind::Important,
        AlertKind::Warning,
        AlertKind::Caution,
    ];

    pub fn word(self) -> &'static str {
        match self {
            AlertKind::Note => "NOTE",
            AlertKind::Tip => "TIP",
            AlertKind::Important => "IMPORTANT",
            AlertKind::Warning => "WARNING",
            AlertKind::Caution => "CAUTION",
        }
    }

    fn next(self) -> Option<AlertKind> {
        let i = AlertKind::ALL.iter().position(|&k| k == self)?;
        AlertKind::ALL.get(i + 1).copied()
    }
}

/// The fenced blocks the bar makes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fence {
    /// ```` ``` ````
    Code,
    /// `$$`
    Math,
    /// ```` ```mermaid ````
    Diagram,
    /// `<details>` with a `<summary>`.
    Details,
}

impl Fence {
    /// The lines before and after the wrapped lines, and what goes between them when nothing is
    /// wrapped.
    fn parts(
        self,
    ) -> (
        &'static [&'static str],
        &'static [&'static str],
        &'static [&'static str],
    ) {
        match self {
            Fence::Code => (&["```"], &["```"], &[""]),
            Fence::Math => (&["$$"], &["$$"], &[""]),
            Fence::Diagram => (&["```mermaid"], &["```"], &["flowchart LR", "  A --> B"]),
            Fence::Details => (
                &["<details>", "<summary>Details</summary>", ""],
                &["", "</details>"],
                &[""],
            ),
        }
    }
}

/// What the format bar shows as pressed for a line.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct LineState {
    /// 0: not a heading.
    pub heading: u8,
    pub list: Option<ListKind>,
    pub quote: bool,
}

fn line_text(buf: &Buffer, line: usize) -> (usize, String, bool) {
    let r = buf.line_range(line);
    let end = buf.floor_char(r.end.min(r.start + LINE_SCAN));
    let mut s = String::new();
    buf.text(r.start..end, &mut s);
    (r.start, s, end == r.end)
}

fn is_blank(t: &str) -> bool {
    t.bytes().all(|c| c == b' ' || c == b'\t')
}

/// A list marker after `at` in `t`: (marker length with the blanks after it, kind). A task's
/// length takes in its box and the blank after it.
fn list_at(t: &str, at: usize) -> Option<(usize, ListKind)> {
    let b = t.as_bytes();
    let mut i = at;
    let numbered = if matches!(b.get(i), Some(b'-' | b'+' | b'*')) {
        i += 1;
        false
    } else {
        let digits = b[i.min(b.len())..]
            .iter()
            .take_while(|c| c.is_ascii_digit())
            .count();
        if digits == 0 || digits > 9 || !matches!(b.get(i + digits), Some(b'.' | b')')) {
            return None;
        }
        i += digits + 1;
        true
    };
    match b.get(i) {
        None => return Some((i - at, kind_of(numbered, false))),
        Some(b' ' | b'\t') => {}
        _ => return None,
    }
    let blanks = b[i..]
        .iter()
        .take_while(|&&c| c == b' ' || c == b'\t')
        .count();
    let after = i + blanks;
    let task = b.len() >= after + 3
        && b[after] == b'['
        && matches!(b[after + 1], b' ' | b'x' | b'X')
        && b[after + 2] == b']'
        && matches!(b.get(after + 3), None | Some(b' ' | b'\t'));
    if task {
        let end = (after + 4).min(b.len());
        return Some((end - at, ListKind::Task));
    }
    Some((after - at, kind_of(numbered, false)))
}

fn kind_of(numbered: bool, task: bool) -> ListKind {
    match (numbered, task) {
        (_, true) => ListKind::Task,
        (true, _) => ListKind::Numbered,
        _ => ListKind::Bullet,
    }
}

/// Up to 3 spaces of indent and then `>`: the byte offset of the `>` and the length of the
/// marker with one blank after it.
fn quote_at(t: &str) -> Option<(usize, usize)> {
    let b = t.as_bytes();
    let indent = b.iter().take_while(|&&c| c == b' ').count();
    if indent > 3 || b.get(indent) != Some(&b'>') {
        return None;
    }
    let len = if matches!(b.get(indent + 1), Some(b' ' | b'\t')) {
        2
    } else {
        1
    };
    Some((indent, len))
}

/// The kind of a `> [!KIND]` line.
fn alert_of(t: &str) -> Option<AlertKind> {
    let (at, len) = quote_at(t)?;
    let rest = t[at + len..].trim();
    let word = rest.strip_prefix("[!")?.strip_suffix(']')?;
    AlertKind::ALL
        .into_iter()
        .find(|k| k.word().eq_ignore_ascii_case(word))
}

/// What the bar shows for the line the caret is on.
pub fn line_state(buf: &Buffer, caret: usize) -> LineState {
    let line = buf.line_of(caret.min(buf.len_bytes()));
    let (_, t, _) = line_text(buf, line);
    let p = prefix_len(&t);
    let heading = {
        let rest = &t[p..];
        let h = rest.bytes().take_while(|&c| c == b'#').count();
        if (1..=6).contains(&h) && matches!(rest.as_bytes().get(h), None | Some(b' ' | b'\t')) {
            h as u8
        } else {
            0
        }
    };
    LineState {
        heading,
        list: list_at(&t, p).map(|(_, k)| k),
        quote: quote_at(&t).is_some(),
    }
}

fn one(buf: &mut Buffer, hist: &mut History, sel: Selection, act: Act) -> Outcome {
    if act.edits.is_empty() {
        return nothing(sel);
    }
    let mut act = Some(act);
    let plan = |_: &Buffer, _: Selection| act.take().expect("one selection");
    match run(buf, hist, &[sel], 0, plan) {
        Some((out, _, _)) => out,
        None => nothing(sel),
    }
}

/// An act that keeps the selection on its text: its ends map through the edits (an insertion at
/// an end goes after it when `grow` says so for that end).
fn keep(sel: Selection, edits: Vec<Edit>) -> Act {
    let at = |p: usize| {
        let grown: usize = edits
            .iter()
            .filter(|e| e.start == e.end && e.start == p)
            .map(|e| e.text.len())
            .sum();
        (p, if sel.is_empty() { grown } else { 0 })
    };
    Act {
        anchor: at(sel.anchor),
        caret: at(sel.caret),
        edits,
        special: true,
        typing: false,
    }
}

fn edit(start: usize, end: usize, text: impl Into<String>) -> Edit {
    Edit {
        start,
        end,
        text: text.into(),
    }
}

/// Makes the lines of the selection items of a `kind` list: a line with another list marker
/// gets this one in its place (a task keeps its box when made a task again), any other non-blank
/// line gets one after its indent and quote markers. When every non-blank line already is such
/// an item, the markers come off. Numbered items are numbered from 1 down the selection.
pub fn toggle_list(
    buf: &mut Buffer,
    hist: &mut History,
    sel: Selection,
    kind: ListKind,
) -> Outcome {
    let r = clamped(buf, sel);
    let (first, last) = touched_lines(buf, &r);
    let lines: Vec<(usize, String)> = (first..=last)
        .map(|l| {
            let (s, t, _) = line_text(buf, l);
            (s, t)
        })
        .filter(|(_, t)| !is_blank(t))
        .collect();
    if lines.is_empty() {
        return nothing(sel);
    }
    let marks: Vec<Option<(usize, ListKind)>> = lines
        .iter()
        .map(|(_, t)| list_at(t, prefix_len(t)))
        .collect();
    let all = marks.iter().all(|m| m.is_some_and(|(_, k)| k == kind));
    let mut edits = Vec::new();
    for (n, ((start, t), m)) in lines.iter().zip(&marks).enumerate() {
        let at = start + prefix_len(t);
        let len = m.map_or(0, |(l, _)| l);
        let new = if all {
            String::new()
        } else {
            match kind {
                ListKind::Bullet => "- ".to_string(),
                ListKind::Numbered => format!("{}. ", n + 1),
                ListKind::Task if m.is_some_and(|(_, k)| k == ListKind::Task) => continue,
                ListKind::Task => "- [ ] ".to_string(),
            }
        };
        if m.is_some_and(|(_, k)| k == ListKind::Bullet) && kind == ListKind::Bullet && !all {
            continue;
        }
        if t[at - start..at - start + len] == new {
            continue;
        }
        edits.push(edit(at, at + len, new));
    }
    let act = keep(sel, edits);
    one(buf, hist, sel, act)
}

/// Puts the lines of the selection in a block quote (`> ` in front of each, `>` on a blank line
/// so the quote stays one block), or, when every touched line already is quoted, takes one level
/// of quote markers off.
pub fn toggle_quote(buf: &mut Buffer, hist: &mut History, sel: Selection) -> Outcome {
    let r = clamped(buf, sel);
    let (first, last) = touched_lines(buf, &r);
    let lines: Vec<(usize, String)> = (first..=last)
        .map(|l| {
            let (s, t, _) = line_text(buf, l);
            (s, t)
        })
        .collect();
    let all = lines.iter().all(|(_, t)| quote_at(t).is_some());
    let mut edits = Vec::new();
    for (start, t) in &lines {
        if all {
            let (at, len) = quote_at(t).unwrap_or((0, 0));
            edits.push(edit(start + at, start + at + len, ""));
        } else if is_blank(t) {
            if lines.len() > 1 {
                edits.push(edit(*start, start + t.len(), ">"));
            }
        } else {
            edits.push(edit(*start, *start, "> "));
        }
    }
    let act = keep(sel, edits);
    one(buf, hist, sel, act)
}

/// Steps the alert the caret is in through GitHub's kinds: lines not in a quote become a
/// `> [!NOTE]` alert (quoted as [`toggle_quote`] does, with the `[!NOTE]` line above); a quote
/// that is not an alert gets the `> [!NOTE]` line; an alert becomes the next kind, and a
/// `CAUTION` alert goes back to plain lines (its `[!CAUTION]` line and its quote markers come
/// off).
pub fn cycle_alert(buf: &mut Buffer, hist: &mut History, sel: Selection, eol: &str) -> Outcome {
    let r = clamped(buf, sel);
    let (first, last) = touched_lines(buf, &r);
    let quoted = |l: usize| quote_at(&line_text(buf, l).1).is_some();
    if !quoted(first) {
        let lines: Vec<(usize, String)> = (first..=last)
            .map(|l| {
                let (s, t, _) = line_text(buf, l);
                (s, t)
            })
            .collect();
        let mut edits = Vec::new();
        for (i, (start, t)) in lines.iter().enumerate() {
            let head = if i == 0 {
                format!("> [!NOTE]{eol}")
            } else {
                String::new()
            };
            let mark = match (quote_at(t), is_blank(t)) {
                (Some(_), _) => "",
                (None, true) => ">",
                (None, false) => "> ",
            };
            if is_blank(t) && quote_at(t).is_none() {
                edits.push(edit(*start, start + t.len(), format!("{head}{mark}")));
            } else {
                edits.push(edit(*start, *start, format!("{head}{mark}")));
            }
        }
        let mut act = keep(sel, edits);
        // A caret on an empty line goes after the new `> `.
        if sel.is_empty() {
            let grown = act.edits[0].text.len();
            act.anchor.1 = grown;
            act.caret.1 = grown;
            act.anchor.0 = lines[0].0;
            act.caret.0 = lines[0].0;
            if !is_blank(&lines[0].1) {
                act.anchor = (sel.anchor, 0);
                act.caret = (sel.caret, 0);
            }
        }
        return one(buf, hist, sel, act);
    }
    // The quote's first and last lines.
    let mut top = first;
    while top > 0 && first - top < QUOTE_SCAN && quoted(top - 1) {
        top -= 1;
    }
    let lines = buf.len_lines();
    let mut bottom = first;
    while bottom + 1 < lines && bottom - first < QUOTE_SCAN && quoted(bottom + 1) {
        bottom += 1;
    }
    let (top_start, top_text, _) = line_text(buf, top);
    let edits = match alert_of(&top_text) {
        None => vec![edit(top_start, top_start, format!("> [!NOTE]{eol}"))],
        Some(k) => match k.next() {
            Some(n) => {
                let at = top_start + top_text.find("[!").unwrap_or(0) + 2;
                vec![edit(at, at + k.word().len(), n.word())]
            }
            None => {
                let mut v = vec![edit(top_start, buf.line_start(top + 1), "")];
                for l in top + 1..=bottom {
                    let (s, t, _) = line_text(buf, l);
                    if let Some((at, len)) = quote_at(&t) {
                        v.push(edit(s + at, s + at + len, ""));
                    }
                }
                v
            }
        },
    };
    let act = Act {
        anchor: (sel.anchor, 0),
        caret: (sel.caret, 0),
        edits,
        special: true,
        typing: false,
    };
    one(buf, hist, sel, act)
}

/// Puts the lines of the selection between `fence`'s opening and closing lines, keeping the
/// selection on them. On a blank line (nothing selected), the block is put in its place with its
/// starting text (an empty line, or a small diagram), the caret in it.
pub fn insert_fence(
    buf: &mut Buffer,
    hist: &mut History,
    sel: Selection,
    fence: Fence,
    eol: &str,
) -> Outcome {
    let r = clamped(buf, sel);
    let (first, last) = touched_lines(buf, &r);
    let (open, close, body) = fence.parts();
    let (start, text, _) = line_text(buf, first);
    let join = |v: &[&str]| v.join(eol);
    if sel.is_empty() && is_blank(&text) {
        let before = format!("{}{eol}", join(open));
        let all = format!("{before}{}{eol}{}", join(body), join(close));
        let caret = before.len()
            + body.last().map_or(0, |l| l.len())
            + body[..body.len() - 1]
                .iter()
                .map(|l| l.len() + eol.len())
                .sum::<usize>();
        let act = Act::replace(start..start + text.len(), &all, caret, true);
        return one(buf, hist, sel, act);
    }
    let end = buf.line_range(last).end;
    let edits = vec![
        edit(start, start, format!("{}{eol}", join(open))),
        edit(end, end, format!("{eol}{}", join(close))),
    ];
    let mut act = keep(sel, edits);
    // The selection stays on the wrapped text, not on the new lines around it.
    let open_len = act.edits[0].text.len();
    for p in [&mut act.anchor, &mut act.caret] {
        if p.0 == start {
            p.1 = open_len;
        } else if p.0 == end {
            p.1 = 0;
        }
    }
    one(buf, hist, sel, act)
}

/// A thematic break (`---`) after the caret's line, with a blank line before it so it is not
/// read as a heading underline, and the caret on a new line after it. On a blank line the break
/// goes in that line.
pub fn insert_rule(buf: &mut Buffer, hist: &mut History, sel: Selection, eol: &str) -> Outcome {
    let r = clamped(buf, sel);
    let (_, last) = touched_lines(buf, &r);
    let (start, text, _) = line_text(buf, last);
    let act = if is_blank(&text) {
        let above = last > 0 && !is_blank(&line_text(buf, last - 1).1);
        let rule = format!("{}---{eol}", if above { eol } else { "" });
        let n = rule.len();
        Act::replace(start..start + text.len(), &rule, n, true)
    } else {
        let end = buf.line_range(last).end;
        let rule = format!("{eol}{eol}---{eol}");
        let n = rule.len();
        Act::replace(end..end, &rule, n, true)
    };
    one(buf, hist, sel, act)
}

/// A footnote's text: `[^label]: ...`.
fn is_note(line: &str) -> bool {
    line.strip_prefix("[^")
        .and_then(|r| r.split_once("]:"))
        .is_some_and(|(label, _)| !label.is_empty() && !label.contains(']'))
}

/// A footnote: `[^n]` at the end of the selection, with `n` one more than the largest number a
/// `[^n]` in the document has, and `[^n]: ` on a new line at the end of the document, the caret
/// after it to write the note. Nothing in a document over [`FOOTNOTE_MAX_BYTES`].
pub fn footnote(buf: &mut Buffer, hist: &mut History, sel: Selection, eol: &str) -> Outcome {
    let len = buf.len_bytes();
    if len > FOOTNOTE_MAX_BYTES {
        return nothing(sel);
    }
    let mut text = String::new();
    buf.text(0..len, &mut text);
    let mut n = 0u32;
    let b = text.as_bytes();
    let mut i = 0;
    while let Some(k) = text[i..].find("[^") {
        let at = i + k + 2;
        let digits = b[at..].iter().take_while(|c| c.is_ascii_digit()).count();
        if digits > 0 && digits < 9 && b.get(at + digits) == Some(&b']') {
            n = n.max(text[at..at + digits].parse().unwrap_or(0));
        }
        i = at;
    }
    let n = n + 1;
    let at = clamped(buf, sel).end;
    let mark = format!("[^{n}]");
    // The text the note goes after: the mark is in it when it goes at the very end.
    let tail = if at == len {
        format!("{text}{mark}")
    } else {
        text
    };
    let body = tail.trim_end_matches(['\n', '\r']);
    let ends = &tail[body.len()..];
    let breaks = ends.matches('\n').count().max(ends.matches('\r').count());
    let ends_line = breaks > 0;
    let last = body.rsplit(['\n', '\r']).next().unwrap_or("");
    // Notes sit together; anything else gets a blank line before the first note.
    let gap = match (body.is_empty() || is_blank(last), is_note(last)) {
        (true, _) => if ends_line { "" } else { eol }.to_string(),
        (false, true) => if ends_line { "" } else { eol }.to_string(),
        (false, false) => match breaks {
            0 => format!("{eol}{eol}"),
            1 => eol.to_string(),
            _ => String::new(),
        },
    };
    let note = format!("{gap}[^{n}]: ");
    let edits = if at == len {
        vec![edit(at, at, format!("{mark}{note}"))]
    } else {
        vec![edit(at, at, mark), edit(len, len, note.clone())]
    };
    let end_extra = if at == len {
        edits[0].text.len()
    } else {
        note.len()
    };
    let act = Act {
        anchor: (len, end_extra),
        caret: (len, end_extra),
        edits,
        special: true,
        typing: false,
    };
    one(buf, hist, sel, act)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::{redo, undo};
    use crate::testutil::{check_change, marked_parse, marked_show};

    const C: char = '¦';

    /// Runs `f` on the marked text, checks the change and its one undo step, and returns the
    /// marked text after.
    fn go(s: &str, f: impl FnOnce(&mut Buffer, &mut History, Selection) -> Outcome) -> String {
        let (text, sels) = marked_parse(s, C);
        let mut buf = Buffer::from_text(&text);
        let mut hist = History::new();
        let out = f(&mut buf, &mut hist, sels[0]);
        let after = buf.rope().to_string();
        match &out.change {
            None => {
                assert_eq!(after, text);
                assert!(!hist.can_undo());
            }
            Some(c) => {
                check_change(&text, &after, c);
                assert_eq!(hist.steps(), (1, 0));
                undo(&mut buf, &mut hist).unwrap();
                assert_eq!(buf.rope().to_string(), text);
                redo(&mut buf, &mut hist).unwrap();
                assert_eq!(buf.rope().to_string(), after);
            }
        }
        marked_show(&after, &[out.sel], C)
    }

    fn list(s: &str, k: ListKind) -> String {
        go(s, |b, h, sel| toggle_list(b, h, sel, k))
    }

    #[test]
    fn lines_become_list_items_and_back() {
        use ListKind::*;
        assert_eq!(list("ab¦c", Bullet), "- ab¦c");
        assert_eq!(list("- ab¦c", Bullet), "ab¦c");
        assert_eq!(list("* ab¦c", Bullet), "ab¦c");
        assert_eq!(list("‹a\n\nb\nc›", Numbered), "‹1. a\n\n2. b\n3. c›");
        assert_eq!(list("‹1. a\n2. b›", Numbered), "‹a\nb›");
        // Another kind is replaced; a task made a task again keeps its box.
        assert_eq!(list("‹- a\n1. b›", Task), "‹- [ ] a\n- [ ] b›");
        assert_eq!(list("‹- [x] a\nb›", Task), "‹- [x] a\n- [ ] b›");
        assert_eq!(list("‹- [x] a\n- [ ] b›", Task), "‹a\nb›");
        assert_eq!(list("‹- [x] a\n- b›", Bullet), "‹- a\n- b›");
        // Indent and quote markers stay in front.
        assert_eq!(list("  > a¦", Numbered), "  > 1. a¦");
        // Blank lines are left alone; nothing for an empty line.
        assert_eq!(list("¦", Bullet), "¦");
    }

    #[test]
    fn quotes_go_on_and_come_off() {
        let q = |s| go(s, toggle_quote);
        assert_eq!(q("a¦b"), "> a¦b");
        assert_eq!(q("‹a\n\nb›"), "‹> a\n>\n> b›");
        assert_eq!(q("‹> a\n>\n> b›"), "‹a\n\nb›");
        assert_eq!(q("> > a¦"), "> a¦");
    }

    #[test]
    fn alerts_step_through_the_kinds_and_back_to_plain_lines() {
        let a = |s| go(s, |b, h, sel| cycle_alert(b, h, sel, "\n"));
        assert_eq!(a("a¦b\nc"), "> [!NOTE]\n> a¦b\nc");
        assert_eq!(a("¦"), "> [!NOTE]\n>¦");
        assert_eq!(a("x\n> a¦"), "x\n> [!NOTE]\n> a¦");
        assert_eq!(a("> [!NOTE]\n> a¦"), "> [!TIP]\n> a¦");
        assert_eq!(a("> [!warning]\n> a¦"), "> [!CAUTION]\n> a¦");
        assert_eq!(a("> [!CAUTION]\n> a¦\n>\n> b\nc"), "a¦\n\nb\nc");
    }

    #[test]
    fn fences_wrap_lines_or_start_a_block() {
        let f = |s, k| go(s, |b, h, sel| insert_fence(b, h, sel, k, "\n"));
        assert_eq!(f("‹a\nb›\nc", Fence::Code), "```\n‹a\nb›\n```\nc");
        assert_eq!(f("x¦", Fence::Math), "$$\nx¦\n$$");
        assert_eq!(f("a\n¦\nb", Fence::Code), "a\n```\n¦\n```\nb");
        assert_eq!(
            f("¦", Fence::Diagram),
            "```mermaid\nflowchart LR\n  A --> B¦\n```"
        );
        assert_eq!(
            f("¦", Fence::Details),
            "<details>\n<summary>Details</summary>\n\n¦\n\n</details>"
        );
    }

    #[test]
    fn a_rule_has_a_blank_line_above_it() {
        let r = |s| go(s, |b, h, sel| insert_rule(b, h, sel, "\n"));
        assert_eq!(r("a¦b\nc"), "ab\n\n---\n¦\nc");
        assert_eq!(r("a\n¦"), "a\n\n---\n¦");
        assert_eq!(r("a\n\n¦"), "a\n\n---\n¦");
    }

    #[test]
    fn a_footnote_takes_the_next_number_and_its_note_goes_at_the_end() {
        let f = |s| go(s, |b, h, sel| footnote(b, h, sel, "\n"));
        assert_eq!(f("a¦ b"), "a[^1] b\n\n[^1]: ¦");
        assert_eq!(
            f("a[^2] b¦\n\n[^2]: x\n"),
            "a[^2] b[^3]\n\n[^2]: x\n[^3]: ¦"
        );
        assert_eq!(f("a¦"), "a[^1]\n\n[^1]: ¦");
        assert_eq!(f("¦"), "[^1]\n\n[^1]: ¦");
        assert_eq!(f("a¦\n\n"), "a[^1]\n\n[^1]: ¦");
    }

    #[test]
    fn the_line_state_reads_heading_list_and_quote() {
        let s = |t: &str| line_state(&Buffer::from_text(t), 0);
        assert_eq!(s("## a").heading, 2);
        assert_eq!(s("#a").heading, 0);
        assert_eq!(s("- [ ] a").list, Some(ListKind::Task));
        assert_eq!(s("3) a").list, Some(ListKind::Numbered));
        assert_eq!(s("> - a").list, Some(ListKind::Bullet));
        assert!(s("> a").quote && !s("a").quote);
    }
}
