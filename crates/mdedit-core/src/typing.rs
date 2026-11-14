//! Typing help: what a typed character, Enter and Backspace do beyond the plain edit. The shell
//! calls [`type_char`], [`enter`] or [`backspace`] first; `None` means "do the plain edit"
//! ([`crate::edit::insert`], `newline`, `backspace`, or their [`crate::multi`] forms), and
//! `Some` is the edit that was made instead (history and buffer already updated).
//!
//! What it does, each part with its own switch in [`Assist`]:
//!
//! * Pairing (`auto_pair`, the kinds in [`Pairs`]): an opener typed where the next character is
//!   white space, a closer (`)`, `]`, `}`) or the end of the line inserts its closer too, caret
//!   between; a quote only when the character before is not a word character, not the same
//!   quote and not a backslash (so an apostrophe after a letter types normally). A closer typed
//!   where the same closer is next steps over it. An opener typed over a selection wraps the
//!   selection and keeps it selected. Backspace between an empty pair removes both.
//! * Indentation (`auto_indent`): Enter keeps the line's leading white space before the caret;
//!   after an opener (`{`, `[`, `(`, ignoring blanks before the caret; also `:` when
//!   `colon_blocks`) it adds one [`Indent`] unit; between a directly adjacent `{}`, `[]` or `()`
//!   the closer goes to its own line at the outer indent and the caret to the line between at
//!   the inner indent. A `}`, `]` or `)` typed as the first non-blank character of a line whose
//!   indent is deeper than that of the line holding its opener removes one unit of indent.
//! * Markdown (`markdown`, with `auto_indent`): Enter continues a list item or a block quote,
//!   and empty items end it; [`list_indent`] moves items in and out.
//!
//! Every function works on a list of selections (several carets) as [`crate::multi::edit`]
//! does: each acts on its own, the edit is one undo step, and the result is the outcome of the
//! primary, every selection afterwards (in document order, merged where they coincide) and the
//! primary's index. When only some of the carets have something special to do, the others get
//! the plain edit within the same step. A single caret typing a pair is `Kind::Typing`, so it
//! joins the typing run before it and the characters typed after it, as plain typing does; every
//! other edit made here is its own step. Where the carets' edits would overlap (backspace over
//! a pair next to another caret) nothing special is done and `None` is returned. A pure step over
//! a closer has `change: None`, no history entry, and moved selections.
//!
//! Looking at text is bounded: indentation and blanks are read at most
//! [`TYPING_SCAN`](crate::limits::TYPING_SCAN) bytes from the caret, and the search for an
//! opener at most [`BRACKET_SCAN`](crate::limits::BRACKET_SCAN) bytes, four times that over all
//! the carets of one keystroke.

use crate::bracket;
use crate::buffer::Buffer;
use crate::edit::{Cmd, Outcome, nothing};
use crate::history::{History, Kind};
use crate::limits::{BRACKET_SCAN, TYPING_SCAN};
use crate::multi::normalize;
use crate::selection::Selection;
use std::ops::Range;

/// The result of a typing function: the primary's outcome, all selections, the primary's index.
pub type Typed = (Outcome, Vec<Selection>, usize);

/// One level of indentation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Indent {
    Tab,
    /// This many spaces (1 to 16; other values are clamped).
    Spaces(u8),
}

impl Indent {
    /// The text of one unit.
    pub fn text(self) -> String {
        match self {
            Indent::Tab => "\t".to_string(),
            Indent::Spaces(n) => " ".repeat(n.clamp(1, 16) as usize),
        }
    }

    /// The columns of one unit.
    fn cols(self, tab_width: usize) -> usize {
        match self {
            Indent::Tab => tab_width.max(1),
            Indent::Spaces(n) => n.clamp(1, 16) as usize,
        }
    }
}

/// Which pairs are made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pairs {
    /// `()`, `[]`, `{}`.
    pub brackets: bool,
    pub double_quote: bool,
    pub single_quote: bool,
    pub backtick: bool,
}

impl Pairs {
    pub const NONE: Pairs = Pairs {
        brackets: false,
        double_quote: false,
        single_quote: false,
        backtick: false,
    };

    /// The pairs for a language, by its name (the highlighter's name or a file extension,
    /// case-insensitive, white space around ignored):
    ///
    /// * no name, "text", "plain", "plaintext", "txt", "tex", "latex": brackets only;
    /// * "markdown", "md": brackets, `"` and `` ` `` (no `'`: apostrophes);
    /// * "json", "jsonc": brackets and `"`;
    /// * "rust", "rs", the Lisps ("lisp", "scheme", "clojure", "elisp", "racket"), "haskell",
    ///   "ocaml", "fsharp" and friends: brackets and `"` (a `'` there is a lifetime, a quote or a
    ///   prime);
    /// * "javascript", "js", "jsx", "typescript", "ts", "tsx", "go", the shells ("sh", "bash",
    ///   "zsh", "fish", "shell"), "ruby", "rb", "perl", "sql", "mysql": all four;
    /// * any other name (code in general): brackets, `"` and `'`.
    pub fn for_language(name: &str) -> Pairs {
        let n = name.trim().to_ascii_lowercase();
        let all = Pairs {
            brackets: true,
            double_quote: true,
            single_quote: true,
            backtick: true,
        };
        match n.as_str() {
            "" | "text" | "plain" | "plaintext" | "plain text" | "txt" | "tex" | "latex" => Pairs {
                brackets: true,
                ..Pairs::NONE
            },
            "markdown" | "md" => Pairs {
                brackets: true,
                double_quote: true,
                backtick: true,
                single_quote: false,
            },
            "json" | "jsonc" | "rust" | "rs" | "lisp" | "scheme" | "clojure" | "elisp"
            | "racket" | "haskell" | "hs" | "ocaml" | "ml" | "fsharp" => Pairs {
                brackets: true,
                double_quote: true,
                ..Pairs::NONE
            },
            "javascript" | "js" | "jsx" | "typescript" | "ts" | "tsx" | "go" | "golang" | "sh"
            | "bash" | "zsh" | "fish" | "shell" | "ruby" | "rb" | "perl" | "sql" | "mysql" => all,
            _ => Pairs {
                backtick: false,
                ..all
            },
        }
    }

    /// The closer made for `open` typed, if it is a pair under this set.
    fn closer(&self, open: char) -> Option<char> {
        match open {
            '(' if self.brackets => Some(')'),
            '[' if self.brackets => Some(']'),
            '{' if self.brackets => Some('}'),
            '"' if self.double_quote => Some('"'),
            '\'' if self.single_quote => Some('\''),
            '`' if self.backtick => Some('`'),
            _ => None,
        }
    }

    /// `c` is a closer of this set (a quote is its own closer).
    fn is_closer(&self, c: char) -> bool {
        match c {
            ')' | ']' | '}' => self.brackets,
            '"' => self.double_quote,
            '\'' => self.single_quote,
            '`' => self.backtick,
            _ => false,
        }
    }
}

/// The settings of the typing help.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Assist {
    pub pairs: Pairs,
    /// Pairing, stepping over, wrapping and pair-deleting at all.
    pub auto_pair: bool,
    /// Indentation on Enter, dedenting closers and (with `markdown`) list continuation.
    pub auto_indent: bool,
    pub indent: Indent,
    /// Columns of a tab, for comparing indents.
    pub tab_width: usize,
    /// A `:` at the end of a line opens a block (Python, YAML).
    pub colon_blocks: bool,
    /// Markdown lists and quotes on Enter, and [`list_indent`].
    pub markdown: bool,
}

impl Assist {
    /// Both helps on, the pairs of [`Pairs::for_language`], `colon_blocks` for "python", "py",
    /// "yaml", "yml", "nim", "coffeescript", "gdscript"; `markdown` for "markdown" and "md".
    pub fn for_language(name: &str, indent: Indent, tab_width: usize) -> Assist {
        let n = name.trim().to_ascii_lowercase();
        Assist {
            pairs: Pairs::for_language(&n),
            auto_pair: true,
            auto_indent: true,
            indent,
            tab_width,
            colon_blocks: matches!(
                n.as_str(),
                "python" | "py" | "yaml" | "yml" | "nim" | "coffeescript" | "gdscript"
            ),
            markdown: matches!(n.as_str(), "markdown" | "md"),
        }
    }
}

// ---- the machinery: acts, and applying them as one step

/// One replacement, in the offsets of the text before the edit.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Edit {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) text: String,
}

/// What one selection does. A position of the selection afterwards is `(offset, extra)`: the
/// offset in the old text mapped through the edits (an insertion at the offset stays after it),
/// plus `extra` bytes.
pub(crate) struct Act {
    pub(crate) edits: Vec<Edit>,
    pub(crate) anchor: (usize, usize),
    pub(crate) caret: (usize, usize),
    /// Not the plain edit.
    pub(crate) special: bool,
    /// A single insertion of typed text that may join a typing run.
    pub(crate) typing: bool,
}

impl Act {
    pub(crate) fn replace(r: Range<usize>, text: &str, caret_extra: usize, special: bool) -> Act {
        let at = (r.start, caret_extra);
        Act {
            edits: vec![Edit {
                start: r.start,
                end: r.end,
                text: text.to_string(),
            }],
            anchor: at,
            caret: at,
            special,
            typing: false,
        }
    }

    /// No edit; the caret goes to `to`.
    fn go(to: usize) -> Act {
        Act {
            edits: Vec::new(),
            anchor: (to, 0),
            caret: (to, 0),
            special: true,
            typing: false,
        }
    }
}

/// Where the old offset `p` is after `edits` (ascending, not overlapping) with `prefix[k]` the
/// total growth of the first `k` of them.
fn map_pos(edits: &[Edit], prefix: &[isize], p: usize) -> usize {
    let k = edits.partition_point(|e| e.end < p || (e.end == p && e.start < e.end));
    match edits.get(k) {
        Some(e) if e.start < p && p < e.end => {
            (e.start as isize + prefix[k]) as usize + e.text.len()
        }
        _ => (p as isize + prefix[k]) as usize,
    }
}

/// Makes the act of every selection as one step. `None` when no act is special, or the edits
/// overlap.
pub(crate) fn run(
    buf: &mut Buffer,
    hist: &mut History,
    sels: &[Selection],
    primary: usize,
    mut plan: impl FnMut(&Buffer, Selection) -> Act,
) -> Option<Typed> {
    let (norm, p) = normalize(buf, sels, primary);
    if norm.is_empty() {
        return None;
    }
    let acts: Vec<Act> = norm.iter().map(|&s| plan(buf, s)).collect();
    if !acts.iter().any(|a| a.special) {
        return None;
    }
    let typing = norm.len() == 1 && acts[0].typing;
    let mut edits: Vec<Edit> = Vec::new();
    let mut points: Vec<((usize, usize), (usize, usize))> = Vec::new();
    let mut last_edits: Vec<Edit> = Vec::new();
    for a in acts {
        points.push((a.anchor, a.caret));
        // Two carets removing the same text (both inside one wrapped span) remove it once.
        let twin = !a.edits.is_empty()
            && a.edits == last_edits
            && a.edits.iter().all(|e| e.end > e.start && e.text.is_empty());
        if !twin {
            last_edits.clone_from(&a.edits);
            edits.extend(a.edits);
        }
    }
    let mut prefix = Vec::with_capacity(edits.len() + 1);
    let mut grown = 0isize;
    let mut end = 0usize;
    for e in &edits {
        if e.start < end || e.end < e.start {
            return None;
        }
        prefix.push(grown);
        grown += e.text.len() as isize - (e.end - e.start) as isize;
        end = e.end;
    }
    prefix.push(grown);
    let at = |(o, extra): (usize, usize)| map_pos(&edits, &prefix, o) + extra;
    let new_sels: Vec<Selection> = points
        .iter()
        .map(|&(a, c)| Selection {
            anchor: at(a),
            caret: at(c),
            goal_col: None,
        })
        .collect();
    let before = norm[p];
    let mut cmd = Cmd::new(buf);
    for e in edits.iter().rev() {
        cmd.replace(e.start..e.end, &e.text);
    }
    let kind = if typing { Kind::Typing } else { Kind::Other };
    let out = cmd.finish(hist, kind, before, new_sels[p]);
    let (list, pi) = normalize(buf, &new_sels, p);
    Some((
        Outcome {
            sel: list[pi],
            change: out.change,
        },
        list,
        pi,
    ))
}

// ---- reading the text around a caret

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn char_before(buf: &Buffer, at: usize) -> Option<char> {
    let at = buf.floor_char(at);
    if at == 0 {
        None
    } else {
        buf.char_at(buf.floor_char(at - 1))
    }
}

/// The chars before `at`, nearest first.
fn back_chars(buf: &Buffer, at: usize) -> impl Iterator<Item = char> + '_ {
    let rope = buf.rope();
    let mut it = rope.chars_at(rope.byte_to_char(buf.floor_char(at)));
    std::iter::from_fn(move || it.prev())
}

/// The last character before `at` on its line that is not a space or tab, looking at most
/// [`TYPING_SCAN`] bytes back; `None` at the line's start or when blanks go on that far.
fn last_non_blank(buf: &Buffer, at: usize) -> Option<char> {
    let mut seen = 0;
    for c in back_chars(buf, at) {
        match c {
            ' ' | '\t' => {
                seen += 1;
                if seen > TYPING_SCAN {
                    return None;
                }
            }
            '\n' | '\r' => return None,
            c => return Some(c),
        }
    }
    None
}

/// The leading spaces and tabs of the line starting at `line_start`, up to `upto`, at most
/// [`TYPING_SCAN`] bytes.
fn indent_between(buf: &Buffer, line_start: usize, upto: usize) -> String {
    let end = upto.min(line_start + TYPING_SCAN);
    let mut s = String::new();
    'scan: for piece in buf.chunks(line_start..end) {
        for c in piece.chars() {
            if c != ' ' && c != '\t' {
                break 'scan;
            }
            s.push(c);
        }
    }
    s
}

/// Columns of a run of blanks.
fn blank_cols(s: &str, tab_width: usize) -> usize {
    let tab = tab_width.max(1);
    s.chars().fold(0, |col, c| {
        if c == '\t' {
            (col / tab + 1) * tab
        } else {
            col + 1
        }
    })
}

/// The line holding `at`, as (start, text of at most [`TYPING_SCAN`] bytes from its start, the
/// whole line fits).
fn line_head(buf: &Buffer, at: usize) -> (usize, String, bool) {
    let line = buf.line_of(at);
    let r = buf.line_range(line);
    let end = buf.floor_char((r.start + TYPING_SCAN).min(r.end));
    let mut s = String::new();
    buf.text(r.start..end, &mut s);
    (r.start, s, end == r.end)
}

// ---- type_char

/// What typing `ch` does, or `None` for the plain edit.
pub fn type_char(
    buf: &mut Buffer,
    hist: &mut History,
    sels: &[Selection],
    primary: usize,
    ch: char,
    cfg: &Assist,
) -> Option<Typed> {
    let is_close_bracket = matches!(ch, ')' | ']' | '}');
    let pairing = cfg.auto_pair;
    let dedent = cfg.auto_indent && is_close_bracket;
    if !(pairing && (cfg.pairs.closer(ch).is_some() || cfg.pairs.is_closer(ch)) || dedent) {
        return None;
    }
    let mut budget = BRACKET_SCAN * 4;
    let text = ch.to_string();
    run(buf, hist, sels, primary, |buf, s| {
        let r = s.range();
        let len = buf.len_bytes();
        let r = r.start.min(len)..r.end.min(len);
        let plain = || Act::replace(r.clone(), &text, ch.len_utf8(), false);
        if pairing {
            let next = buf.char_at(r.end);
            if s.is_empty() && cfg.pairs.is_closer(ch) && next == Some(ch) {
                return Act::go(r.start + ch.len_utf8());
            }
            if let Some(close) = cfg.pairs.closer(ch) {
                if !s.is_empty() {
                    let open = ch.to_string();
                    let (a, c) = if s.anchor <= s.caret {
                        ((r.start, open.len()), (r.end, 0))
                    } else {
                        ((r.end, 0), (r.start, open.len()))
                    };
                    return Act {
                        edits: vec![
                            Edit {
                                start: r.start,
                                end: r.start,
                                text: open,
                            },
                            Edit {
                                start: r.end,
                                end: r.end,
                                text: close.to_string(),
                            },
                        ],
                        anchor: a,
                        caret: c,
                        special: true,
                        typing: false,
                    };
                }
                let space_next =
                    next.is_none_or(|n| n.is_whitespace() || matches!(n, ')' | ']' | '}'));
                let quote = close == ch;
                let prev = char_before(buf, r.start);
                let prev_ok = !quote || prev.is_none_or(|p| !is_word(p) && p != ch && p != '\\');
                if space_next && prev_ok {
                    let mut a = Act::replace(
                        r.start..r.start,
                        &format!("{ch}{close}"),
                        ch.len_utf8(),
                        true,
                    );
                    a.typing = true;
                    return a;
                }
            }
        }
        if dedent
            && s.is_empty()
            && let Some(k) = dedent_len(buf, r.start, ch, cfg, &mut budget)
        {
            return Act::replace(r.start - k..r.start, &text, 1, true);
        }
        plain()
    })
}

/// How many bytes of the blanks before `caret` go when `closer` is typed there, or `None`: the
/// line must be blank before the caret, and deeper than the line of the closer's opener.
fn dedent_len(
    buf: &Buffer,
    caret: usize,
    closer: char,
    cfg: &Assist,
    budget: &mut usize,
) -> Option<usize> {
    let line_start = buf.line_start(buf.line_of(caret));
    if caret == line_start || caret - line_start > TYPING_SCAN {
        return None;
    }
    let cur = indent_between(buf, line_start, caret);
    if cur.len() != caret - line_start || *budget == 0 {
        return None;
    }
    let limit = BRACKET_SCAN.min(*budget);
    let opener = bracket::opener_before(buf.rope(), caret, closer, limit);
    *budget -= opener.map_or(limit, |o| caret - o).min(*budget);
    let opener = opener?;
    let o_start = buf.line_start(buf.line_of(opener));
    let o_indent = indent_between(buf, o_start, opener.min(o_start + TYPING_SCAN));
    let (cur_w, open_w) = (
        blank_cols(&cur, cfg.tab_width),
        blank_cols(&o_indent, cfg.tab_width),
    );
    if cur_w <= open_w {
        return None;
    }
    if cur.ends_with('\t') {
        return Some(1);
    }
    let spaces = cur.len() - cur.trim_end_matches(' ').len();
    let k = cfg
        .indent
        .cols(cfg.tab_width)
        .min(cur_w - open_w)
        .min(spaces);
    (k > 0).then_some(k)
}

// ---- backspace

/// Backspace between an empty pair removes both characters (one step of its own). `None` for
/// everything else: plain Backspace.
pub fn backspace(
    buf: &mut Buffer,
    hist: &mut History,
    sels: &[Selection],
    primary: usize,
    cfg: &Assist,
) -> Option<Typed> {
    if !cfg.auto_pair {
        return None;
    }
    run(buf, hist, sels, primary, |buf, s| {
        let r = s.range();
        if s.is_empty() {
            let c = buf.floor_char(r.start);
            if let (Some(p), Some(n)) = (char_before(buf, c), buf.char_at(c))
                && cfg.pairs.closer(p) == Some(n)
            {
                let from = c - p.len_utf8();
                return Act::replace(from..c + n.len_utf8(), "", 0, true);
            }
            let prev = buf.prev_grapheme(c);
            return Act::replace(prev..c, "", 0, false);
        }
        Act::replace(r, "", 0, false)
    })
}

// ---- Markdown lines

/// A Markdown line's prefix: block quote markers, then a list marker.
struct MdLine {
    /// Where each `>` is.
    quotes: Vec<usize>,
    /// After the quote markers and the one space after each.
    quote_end: usize,
    list: Option<MdList>,
    /// Where the item's text (or the quote's) starts.
    content: usize,
}

struct MdList {
    /// The marker starts here; between `quote_end` and this is the item's indent.
    start: usize,
    kind: MdKind,
    /// The marker's text and the blanks after it, `[ ] ` excluded.
    marker_end: usize,
    task: bool,
}

enum MdKind {
    Bullet,
    /// The number, the digits' width and the delimiter.
    Ordered(u64, usize, char),
}

fn blank(b: u8) -> bool {
    b == b' ' || b == b'\t'
}

/// Parses the start of a line (ASCII structure only; `text` may be cut anywhere).
fn parse_md(text: &str) -> MdLine {
    let b = text.as_bytes();
    let mut i = 0;
    let mut quotes = Vec::new();
    let mut quote_end = 0;
    loop {
        let mut j = i;
        while j < b.len() && blank(b[j]) {
            j += 1;
        }
        if b.get(j) == Some(&b'>') {
            quotes.push(j);
            i = j + 1;
            if b.get(i) == Some(&b' ') {
                i += 1;
            }
            quote_end = i;
        } else {
            i = j;
            break;
        }
    }
    let mut line = MdLine {
        quotes,
        quote_end,
        list: None,
        content: quote_end,
    };
    let start = i;
    let kind_end = match b.get(start) {
        Some(b'-' | b'*' | b'+') => Some((MdKind::Bullet, start + 1)),
        Some(d) if d.is_ascii_digit() => {
            let mut k = start;
            while k < b.len() && b[k].is_ascii_digit() {
                k += 1;
            }
            match b.get(k) {
                Some(&dl @ (b'.' | b')')) if k - start <= 9 => text[start..k]
                    .parse::<u64>()
                    .ok()
                    .map(|n| (MdKind::Ordered(n, k - start, dl as char), k + 1)),
                _ => None,
            }
        }
        _ => None,
    };
    if let Some((kind, mut k)) = kind_end
        && b.get(k).is_some_and(|&c| blank(c))
    {
        while k < b.len() && blank(b[k]) {
            k += 1;
        }
        let marker_end = k;
        let mut content = k;
        let mut task = false;
        if b.get(k) == Some(&b'[')
            && matches!(b.get(k + 1), Some(b' ' | b'x' | b'X'))
            && b.get(k + 2) == Some(&b']')
            && b.get(k + 3).is_none_or(|&c| blank(c))
        {
            task = true;
            content = k + 3;
            while content < b.len() && blank(b[content]) {
                content += 1;
            }
        }
        line.list = Some(MdList {
            start,
            kind,
            marker_end,
            task,
        });
        line.content = content;
    }
    line
}

/// The text one unit shallower than `blanks`: a trailing tab goes, else up to one unit of spaces.
fn outdented(blanks: &str, cfg: &Assist) -> String {
    if blanks.is_empty() {
        return String::new();
    }
    if let Some(s) = blanks.strip_suffix('\t') {
        return s.to_string();
    }
    let spaces = blanks.len() - blanks.trim_end_matches(' ').len();
    let k = cfg.indent.cols(cfg.tab_width).min(spaces);
    blanks[..blanks.len() - k].to_string()
}

/// The text of one unit to add to a line whose leading blanks are `base`: the configured unit,
/// except that a line indented only with the other kind goes on in its own kind (a tab for tabs;
/// for spaces the unit's width, or the tab width when the unit is a tab), so a line never gets
/// mixed leading white space from here.
fn unit_after(base: &str, cfg: &Assist) -> String {
    let (tabs, spaces) = (base.contains('\t'), base.contains(' '));
    match (cfg.indent, tabs, spaces) {
        (Indent::Spaces(_), true, false) => "\t".to_string(),
        (Indent::Tab, false, true) => " ".repeat(cfg.indent.cols(cfg.tab_width)),
        (unit, ..) => unit.text(),
    }
}

/// What Enter does on a Markdown list or quote line: the replacement of `range` and where the
/// caret goes in it. `None` when the line is neither, or the caret is before its text.
fn md_enter(
    buf: &Buffer,
    r: &Range<usize>,
    eol: &str,
    cfg: &Assist,
) -> Option<(Range<usize>, String, usize)> {
    let (ls, head, whole) = line_head(buf, r.start);
    let line_end = buf.line_range(buf.line_of(r.start)).end;
    let md = parse_md(&head);
    if md.quotes.is_empty() && md.list.is_none() {
        return None;
    }
    let col = r.start - ls;
    if col < md.content || (md.list.is_none() && col < md.quote_end) {
        return None;
    }
    // The line's text after the prefix is empty: only blanks to its end.
    let empty = r.is_empty()
        && whole
        && head.as_bytes()[md.content..].iter().all(|&c| blank(c))
        && head.len() == line_end - ls;
    if empty {
        let new = match &md.list {
            Some(l) => {
                let indent = &head[md.quote_end..l.start];
                if indent.is_empty() {
                    head[..md.quote_end].to_string()
                } else {
                    format!(
                        "{}{}{}",
                        &head[..md.quote_end],
                        outdented(indent, cfg),
                        &head[l.start..md.content]
                    )
                }
            }
            None => {
                let cut = md.quotes[md.quotes.len() - 1];
                if md.quotes.len() == 1 {
                    String::new()
                } else {
                    head[..cut].to_string()
                }
            }
        };
        let n = new.len();
        return Some((ls..line_end, new, n));
    }
    let mut prefix = String::from(eol);
    match &md.list {
        Some(l) => {
            prefix.push_str(&head[..l.start]);
            match l.kind {
                MdKind::Bullet => prefix.push_str(&head[l.start..l.marker_end]),
                MdKind::Ordered(n, w, d) => {
                    prefix.push_str(&format!("{:0w$}{d}", n.saturating_add(1)));
                    let blanks_from = head[l.start..].find(d).map_or(l.start, |k| l.start + k + 1);
                    prefix.push_str(&head[blanks_from..l.marker_end]);
                }
            }
            if l.task {
                prefix.push_str("[ ] ");
            }
        }
        None => prefix.push_str(&head[..md.quote_end]),
    }
    let n = prefix.len();
    Some((r.clone(), prefix, n))
}

// ---- Enter

/// Enter: the line break `eol` and then, by the settings, the indentation (see the module
/// documentation). With `auto_indent` off: `None`.
pub fn enter(
    buf: &mut Buffer,
    hist: &mut History,
    sels: &[Selection],
    primary: usize,
    eol: &str,
    cfg: &Assist,
) -> Option<Typed> {
    if !cfg.auto_indent {
        return None;
    }
    run(buf, hist, sels, primary, |buf, s| {
        let len = buf.len_bytes();
        let r = buf.floor_char(s.range().start.min(len))..buf.floor_char(s.range().end.min(len));
        let r = r.start..r.end.max(r.start);
        if cfg.markdown
            && let Some((range, text, caret)) = md_enter(buf, &r, eol, cfg)
        {
            return Act::replace(range, &text, caret, true);
        }
        let ls = buf.line_start(buf.line_of(r.start));
        let base = indent_between(buf, ls, r.start);
        let last = last_non_blank(buf, r.start);
        let opens =
            matches!(last, Some('{' | '[' | '(')) || (cfg.colon_blocks && last == Some(':'));
        let between = matches!(
            (char_before(buf, r.start), buf.char_at(r.end)),
            (Some('{'), Some('}')) | (Some('['), Some(']')) | (Some('('), Some(')'))
        );
        let mut text = String::from(eol);
        text.push_str(&base);
        if opens {
            text.push_str(&unit_after(&base, cfg));
        }
        let caret = text.len();
        if between {
            text.push_str(eol);
            text.push_str(&base);
        }
        Act::replace(r, &text, caret, true)
    })
}

// ---- list_indent

/// Tab or Shift+Tab (`outdent`) in a Markdown list: moves the list items one [`Indent`] unit in
/// or out. With no selection, the caret must be in the item's leading part (up to where its text starts) of
/// a list line; with a selection, every line it touches (the rule of [`crate::lines`]) that is a
/// list item moves, the others stay. The unit goes in after the line's quote markers; going out
/// removes a trailing tab, or up to one unit of spaces, of the item's indent. Numbers are not
/// renumbered. `None` when `markdown` is off, nothing is a list item there, or nothing would
/// move: the shell then does its normal Tab. A single selection only (other carets are not
/// moved). One undo step; the selection keeps covering its text.
pub fn list_indent(
    buf: &mut Buffer,
    hist: &mut History,
    sel: Selection,
    outdent: bool,
    cfg: &Assist,
) -> Option<Outcome> {
    if !cfg.markdown {
        return None;
    }
    let len = buf.len_bytes();
    let (a, c) = (
        buf.floor_char(sel.anchor.min(len)),
        buf.floor_char(sel.caret.min(len)),
    );
    let r = a.min(c)..a.max(c);
    let (first, last) = crate::edit::touched_lines(buf, &r);
    // (position, bytes removed, text inserted), ascending.
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    for line in first..=last {
        let (ls, head, _) = line_head(buf, buf.line_start(line));
        let md = parse_md(&head);
        let Some(l) = &md.list else { continue };
        if r.is_empty() && c - ls > md.content.max(l.start) {
            continue;
        }
        if !outdent {
            let unit = unit_after(&head[md.quote_end..l.start], cfg);
            edits.push((ls + md.quote_end, 0, unit));
        } else {
            let indent = &head[md.quote_end..l.start];
            let keep = outdented(indent, cfg).len();
            if keep < indent.len() {
                edits.push((ls + md.quote_end + keep, indent.len() - keep, String::new()));
            }
        }
    }
    if edits.is_empty() {
        return None;
    }
    let map = |p: usize, grow_at: bool| {
        let mut q = p as isize;
        for (pos, rem, ins) in edits.iter().map(|(a, b, c)| (*a, *b, c)) {
            if rem == 0 {
                if p > pos || (p == pos && grow_at) {
                    q += ins.len() as isize;
                }
            } else if p >= pos + rem {
                q -= rem as isize;
            } else if p > pos {
                q -= (p - pos) as isize;
            }
        }
        q as usize
    };
    let high = a.max(c);
    let after = Selection {
        anchor: map(a, r.is_empty() || a == high),
        caret: map(c, r.is_empty() || c == high),
        goal_col: None,
    };
    let mut cmd = Cmd::new(buf);
    for (pos, rem, ins) in edits.iter().rev() {
        cmd.replace(*pos..*pos + *rem, ins);
    }
    let out = cmd.finish(hist, Kind::Other, sel, after);
    if out.change.is_none() {
        Some(nothing(sel))
    } else {
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::{insert, redo, undo};
    use crate::testutil::{Rng, check_change, marked_parse, marked_show};

    const SP4: Indent = Indent::Spaces(4);

    fn cfg(lang: &str) -> Assist {
        Assist::for_language(lang, SP4, 4)
    }

    fn parse(s: &str) -> (String, Vec<Selection>) {
        marked_parse(s, '|')
    }

    fn show(text: &str, sels: &[Selection]) -> String {
        marked_show(text, sels, '|')
    }

    /// Runs `f` on the document `s`; checks the change, one step, undo and redo. `None` when `f`
    /// returned `None` (and the buffer is untouched).
    fn go(
        s: &str,
        f: impl FnOnce(&mut Buffer, &mut History, &[Selection], usize) -> Option<Typed>,
    ) -> Option<String> {
        let (text, sels) = parse(s);
        let mut buf = Buffer::from_text(&text);
        let mut hist = History::new();
        let res = f(&mut buf, &mut hist, &sels, 0);
        let Some((out, list, pi)) = res else {
            assert_eq!(buf.rope().to_string(), text);
            assert!(!hist.can_undo());
            return None;
        };
        let after = buf.rope().to_string();
        assert_eq!(out.sel, list[pi]);
        match &out.change {
            None => {
                assert_eq!(after, text);
                assert!(!hist.can_undo());
            }
            Some(c) => {
                check_change(&text, &after, c);
                assert_eq!(hist.steps(), (1, 0), "one undo step");
                undo(&mut buf, &mut hist).unwrap();
                assert_eq!(buf.rope().to_string(), text);
                redo(&mut buf, &mut hist).unwrap();
                assert_eq!(buf.rope().to_string(), after);
            }
        }
        assert!(
            list.windows(2)
                .all(|w| w[0].range().end <= w[1].range().start)
        );
        for x in &list {
            assert!(buf.is_char_boundary(x.anchor) && buf.is_char_boundary(x.caret));
        }
        Some(show(&after, &list))
    }

    fn ty(s: &str, ch: char, c: &Assist) -> Option<String> {
        go(s, |b, h, sels, p| type_char(b, h, sels, p, ch, c))
    }

    fn nl(s: &str, c: &Assist) -> Option<String> {
        go(s, |b, h, sels, p| enter(b, h, sels, p, "\n", c))
    }

    fn bs(s: &str, c: &Assist) -> Option<String> {
        go(s, |b, h, sels, p| backspace(b, h, sels, p, c))
    }

    #[test]
    fn brackets_pair_before_blanks_closers_and_the_end() {
        let c = cfg("rust");
        assert_eq!(ty("|", '(', &c).unwrap(), "(|)");
        assert_eq!(ty("a |", '[', &c).unwrap(), "a [|]");
        assert_eq!(ty("|\n", '{', &c).unwrap(), "{|}\n");
        assert_eq!(ty("|\r\n", '{', &c).unwrap(), "{|}\r\n");
        assert_eq!(ty("f(|)", '(', &c).unwrap(), "f((|))");
        assert_eq!(ty("a| b", '(', &c).unwrap(), "a(|) b");
        // Before a word: the plain edit.
        assert_eq!(ty("|a", '(', &c), None);
        assert_eq!(ty("|é", '{', &c), None);
        // Not an opener.
        assert_eq!(ty("|", 'a', &c), None);
        assert_eq!(ty("|", '<', &c), None);
    }

    #[test]
    fn quotes_follow_the_word_rules() {
        let c = cfg("python");
        assert_eq!(ty("x = |", '"', &c).unwrap(), "x = \"|\"");
        assert_eq!(ty("(|)", '\'', &c).unwrap(), "(''|)".replace("''|", "'|'"));
        // After a letter or digit or `_`: an apostrophe types normally.
        assert_eq!(ty("don|", '\'', &c), None);
        assert_eq!(ty("x1|", '"', &c), None);
        assert_eq!(ty("_|", '"', &c), None);
        // After the same quote, or a backslash.
        assert_eq!(ty("\"|", '"', &c), None);
        assert_eq!(ty("\\|", '"', &c), None);
        // Step over the closing quote.
        assert_eq!(ty("\"a|\"", '"', &c).unwrap(), "\"a\"|");
        // Backtick only where the language uses it.
        assert_eq!(ty("|", '`', &c), None);
        assert_eq!(ty("|", '`', &cfg("go")).unwrap(), "`|`");
        // Plain text: brackets only. Markdown: no single quote.
        assert_eq!(ty("|", '"', &cfg("")), None);
        assert_eq!(ty("|", '(', &cfg("text")).unwrap(), "(|)");
        assert_eq!(ty("|", '\'', &cfg("markdown")), None);
        assert_eq!(ty("|", '`', &cfg("md")).unwrap(), "`|`");
        // Rust: no single quote pairs (lifetimes).
        assert_eq!(ty("&|", '\'', &cfg("rust")), None);
        assert_eq!(ty("|", '"', &cfg("json")).unwrap(), "\"|\"");
        assert_eq!(ty("|", '\'', &cfg("json")), None);
    }

    #[test]
    fn closers_step_over() {
        let c = cfg("rust");
        assert_eq!(ty("(a|)", ')', &c).unwrap(), "(a)|");
        assert_eq!(ty("{|}", '}', &c).unwrap(), "{}|");
        assert_eq!(ty("[a|]]", ']', &c).unwrap(), "[a]|]");
        // No closer after: the plain edit (a dedent does not apply either).
        assert_eq!(ty("(a|", ')', &c), None);
        assert_eq!(ty("(a|b)", ')', &c), None);
        // Stepping over makes no history entry.
        let (text, sels) = parse("(a|)");
        let mut buf = Buffer::from_text(&text);
        let mut hist = History::new();
        let (o, list, _) = type_char(&mut buf, &mut hist, &sels, 0, ')', &c).unwrap();
        assert!(o.change.is_none() && !hist.can_undo());
        assert_eq!(list, vec![Selection::at(3)]);
    }

    #[test]
    fn an_opener_wraps_the_selection() {
        let c = cfg("rust");
        assert_eq!(ty("a ‹bc› d", '(', &c).unwrap(), "a (‹bc›) d");
        assert_eq!(ty("a ›bc‹ d", '"', &c).unwrap(), "a \"›bc‹\" d");
        assert_eq!(ty("‹é\nz›", '{', &c).unwrap(), "{‹é\nz›}");
        // A quote wraps after a letter too; a pair not enabled replaces.
        assert_eq!(ty("x‹a›", '"', &c).unwrap(), "x\"‹a›\"");
        assert_eq!(ty("x‹a›", '\'', &c), None);
        // A closer typed over a selection replaces it.
        assert_eq!(ty("x‹a›", ')', &c), None);
    }

    #[test]
    fn backspace_removes_an_empty_pair() {
        let c = cfg("javascript");
        assert_eq!(bs("(|)", &c).unwrap(), "|");
        assert_eq!(bs("a[|]b", &c).unwrap(), "a|b");
        assert_eq!(bs("x = `|`", &c).unwrap(), "x = |");
        assert_eq!(bs("é{|}é", &c).unwrap(), "é|é");
        assert_eq!(bs("(a|)", &c), None);
        assert_eq!(bs("(|a)", &c), None);
        assert_eq!(bs("(|", &c), None);
        assert_eq!(bs("‹(›)", &c), None);
        assert_eq!(bs("(|)", &cfg("")).unwrap(), "|");
        assert_eq!(bs("\"|\"", &cfg("")), None);
        let mut off = cfg("rust");
        off.auto_pair = false;
        assert_eq!(bs("(|)", &off), None);
    }

    #[test]
    fn switches_turn_each_help_off() {
        let mut c = cfg("rust");
        c.auto_pair = false;
        assert_eq!(ty("|", '(', &c), None);
        assert_eq!(ty("(|)", ')', &c), None);
        assert!(nl("{|}", &c).is_some());
        let mut c = cfg("rust");
        c.auto_indent = false;
        assert_eq!(nl("  a|", &c), None);
        assert_eq!(ty("{\n    |", '}', &c), None);
        assert!(ty("|", '(', &c).is_some());
        c.markdown = true;
        assert_eq!(nl("- a|", &c), None);
    }

    #[test]
    fn enter_keeps_the_indent() {
        let c = cfg("text");
        assert_eq!(nl("  \ta|b", &c).unwrap(), "  \ta\n  \t|b");
        assert_eq!(nl("a|", &c).unwrap(), "a\n|");
        // Only the blanks before the caret count.
        assert_eq!(nl(" | a", &c).unwrap(), " \n | a");
        assert_eq!(nl("|", &c).unwrap(), "\n|");
        // The selection is replaced; the indent is that of its start's line.
        assert_eq!(nl("  a‹bc›d", &c).unwrap(), "  a\n  |d");
        // CRLF documents: the break given.
        let (text, sels) = parse("  a|");
        let mut buf = Buffer::from_text(&text);
        let mut hist = History::new();
        enter(&mut buf, &mut hist, &sels, 0, "\r\n", &c).unwrap();
        assert_eq!(buf.rope().to_string(), "  a\r\n  ");
    }

    #[test]
    fn enter_adds_a_unit_after_an_opener() {
        let c = cfg("rust");
        assert_eq!(nl("fn f() {|", &c).unwrap(), "fn f() {\n    |");
        assert_eq!(nl("  if (a &&|", &c).unwrap(), "  if (a &&\n  |");
        assert_eq!(nl("  x = [  |", &c).unwrap(), "  x = [  \n      |");
        let t = Assist::for_language("rust", Indent::Tab, 4);
        assert_eq!(nl("\t{|", &t).unwrap(), "\t{\n\t\t|");
        // Between a pair: closer on its own line at the outer indent.
        assert_eq!(nl("  {|}", &c).unwrap(), "  {\n      |\n  }");
        assert_eq!(nl("[|]", &c).unwrap(), "[\n    |\n]");
        assert_eq!(nl("f(|)", &c).unwrap(), "f(\n    |\n)");
        // Not directly between: the closer stays where it is.
        assert_eq!(nl("{ |}", &c).unwrap(), "{ \n    |}");
        assert_eq!(nl("{|a}", &c).unwrap(), "{\n    |a}");
        // Mismatched pair.
        assert_eq!(nl("{|)", &c).unwrap(), "{\n    |)");
    }

    #[test]
    fn a_unit_added_to_a_line_continues_that_lines_own_kind() {
        let spaces = cfg("rust");
        let tabs = Assist::for_language("rust", Indent::Tab, 4);
        // Tabs on the line, spaces configured: a tab. Spaces on the line, tabs configured: the
        // tab width in spaces. No leading white space or a mixed run: the configured unit.
        assert_eq!(nl("\tf(|", &spaces).unwrap(), "\tf(\n\t\t|");
        assert_eq!(nl("\t{|}", &spaces).unwrap(), "\t{\n\t\t|\n\t}");
        assert_eq!(nl("  f(|", &tabs).unwrap(), "  f(\n      |");
        let t8 = Assist::for_language("rust", Indent::Tab, 8);
        assert_eq!(
            nl("  f(|", &t8).unwrap(),
            "  f(\n  ".to_string() + &" ".repeat(8) + "|"
        );
        assert_eq!(nl("f(|", &tabs).unwrap(), "f(\n\t|");
        assert_eq!(nl("f(|", &spaces).unwrap(), "f(\n    |");
        assert_eq!(nl(" \tf(|", &spaces).unwrap(), " \tf(\n \t    |");
        // No result mixes the two where the line was pure.
        for line in ["\t", "\t\t", "  ", "    "] {
            for c in [&spaces, &tabs] {
                let out = nl(&format!("{line}f(|"), c).unwrap();
                let ind = out.split('\n').nth(1).unwrap().trim_end_matches('|');
                assert!(
                    !(ind.contains('\t') && ind.contains(' ')),
                    "{line:?}: {ind:?}"
                );
            }
        }
    }

    #[test]
    fn colon_blocks_only_where_asked() {
        assert_eq!(nl("if x:|", &cfg("python")).unwrap(), "if x:\n    |");
        assert_eq!(nl("  else:  |", &cfg("py")).unwrap(), "  else:  \n      |");
        assert_eq!(nl("key:|", &cfg("yaml")).unwrap(), "key:\n    |");
        assert_eq!(nl("a:|", &cfg("rust")).unwrap(), "a:\n|");
        assert_eq!(
            nl(
                "x {|",
                &Assist::for_language("python", Indent::Spaces(2), 4)
            )
            .unwrap(),
            "x {\n  |"
        );
    }

    #[test]
    fn a_typed_closer_at_the_line_start_dedents() {
        let c = cfg("rust");
        assert_eq!(ty("fn f() {\n    |", '}', &c).unwrap(), "fn f() {\n}|");
        assert_eq!(ty("  if x {\n      |", '}', &c).unwrap(), "  if x {\n  }|");
        assert_eq!(ty("f(\n    a,\n    |", ')', &c).unwrap(), "f(\n    a,\n)|");
        assert_eq!(ty("[\n  1,\n      |", ']', &c).unwrap(), "[\n  1,\n  ]|");
        // Tab indents lose the tab.
        assert_eq!(ty("{\n\t|", '}', &c).unwrap(), "{\n}|");
        // Not deeper than the opener's line: nothing is removed.
        assert_eq!(ty("{\n|", '}', &c), None);
        assert_eq!(ty("  {\n  |", '}', &c), None);
        assert_eq!(ty("{ a\n|b", '}', &c), None);
        // Text before the caret on the line, or no opener at all: plain.
        assert_eq!(ty("{\n    a |", '}', &c), None);
        assert_eq!(ty("    |", '}', &c), None);
        // The other kind of bracket does not match.
        assert_eq!(ty("(\n    |", '}', &c), None);
        // Nested of the same kind.
        assert_eq!(
            ty("{\n  {\n  }\n    |", '}', &c).unwrap(),
            "{\n  {\n  }\n}|"
        );
        // An opener beyond the limit: nothing.
        let far = format!("{{\n{}\n    |", "x\n".repeat(BRACKET_SCAN));
        assert_eq!(ty(&far, '}', &c), None);
    }

    #[test]
    fn pairs_join_the_typing_run() {
        let c = cfg("rust");
        let mut buf = Buffer::from_text("");
        let mut hist = History::new();
        let mut sel = Selection::at(0);
        // x ( a b ) c  -> "x(ab)c"
        for ch in ['x', '(', 'a', 'b', ')', 'c'] {
            let r = type_char(&mut buf, &mut hist, &[sel], 0, ch, &c);
            sel = match r {
                Some((o, _, _)) => o.sel,
                None => insert(&mut buf, &mut hist, sel, &ch.to_string()).sel,
            };
        }
        assert_eq!(buf.rope().to_string(), "x(ab)c");
        // "x(ab" is one run, the step over broke it, "c" is another.
        assert_eq!(hist.steps(), (2, 0));
        undo(&mut buf, &mut hist).unwrap();
        assert_eq!(buf.rope().to_string(), "x(ab)");
        undo(&mut buf, &mut hist).unwrap();
        assert_eq!(buf.rope().to_string(), "");
        // Two pairs in a row.
        let mut buf = Buffer::from_text("");
        let mut hist = History::new();
        let (o, _, _) = type_char(&mut buf, &mut hist, &[Selection::at(0)], 0, '(', &c).unwrap();
        let (o, _, _) = type_char(&mut buf, &mut hist, &[o.sel], 0, '[', &c).unwrap();
        assert_eq!((buf.rope().to_string().as_str(), o.sel.caret), ("([])", 2));
        assert_eq!(hist.steps(), (1, 0));
        let o = insert(&mut buf, &mut hist, o.sel, "z");
        assert_eq!((buf.rope().to_string().as_str(), o.sel.caret), ("([z])", 3));
        assert_eq!(hist.steps(), (1, 0));
        let u = undo(&mut buf, &mut hist).unwrap();
        assert_eq!(buf.rope().to_string(), "");
        assert_eq!(u.sel, Selection::at(0));
    }

    #[test]
    fn several_carets_each_act_alone() {
        let c = cfg("rust");
        assert_eq!(ty("a |\nb |\n", '(', &c).unwrap(), "a (|)\nb (|)\n");
        // One caret before a word gets the plain edit within the same step.
        assert_eq!(ty("|\n|x", '(', &c).unwrap(), "(|)\n(|x");
        // Step over at one, pair at another.
        assert_eq!(ty("(a|)\n|", ')', &c).unwrap(), "(a)|\n)|");
        // Wrap at several selections.
        assert_eq!(ty("‹a› ‹b›", '[', &c).unwrap(), "[‹a›] [‹b›]");
        // Enter at each.
        assert_eq!(nl("{|}\n  a|", &c).unwrap(), "{\n    |\n}\n  a\n  |");
        // Backspace at each pair.
        assert_eq!(bs("(|) [|]", &c).unwrap(), "| |");
        // A pair delete next to another caret's backspace would overlap: plain.
        assert_eq!(bs("(|)|", &c), None);
        // Carets that coincide after the edit merge.
        let c2 = cfg("text");
        assert_eq!(
            go("|a|", |b, h, s, p| type_char(b, h, s, p, '(', &c2)).unwrap(),
            "(|a(|)"
        );
    }

    #[test]
    fn dedent_with_several_carets() {
        let c = cfg("rust");
        assert_eq!(ty("{\n    |\n{\n    |", '}', &c).unwrap(), "{\n}|\n{\n}|");
        // Past the shared budget the rest are plain: many carets, each far from its opener.
        let unit = format!("{{\n{}\n    |\n", "x\n".repeat(30_000));
        let mut text = String::new();
        let mut caret_text = String::new();
        for _ in 0..8 {
            caret_text.push_str(&unit);
        }
        text.push_str(&caret_text);
        let r = ty(&text, '}', &c);
        assert!(r.is_some());
    }

    /// Anything goes: random documents and selections, bad offsets included.
    #[test]
    fn hostile_input_never_panics_and_undoes() {
        let mut rng = Rng::new(11);
        let pieces = [
            "(", ")", "{", "}", "[", "]", "\"", "'", "`", " ", "\t", "\n", "\r\n", "\r", "é", "a",
            ":", "-", "> ", "1. ", "- [ ] ", "日",
        ];
        let langs = ["", "rust", "python", "markdown", "go", "zzz"];
        for _ in 0..1500 {
            let mut text = String::new();
            for _ in 0..rng.below(14) {
                text.push_str(rng.pick(&pieces));
            }
            let mut c = Assist::for_language(
                rng.pick(&langs),
                if rng.chance(50) { SP4 } else { Indent::Tab },
                1 + rng.below(8),
            );
            c.auto_pair = rng.chance(85);
            c.auto_indent = rng.chance(85);
            let mut buf = Buffer::from_text(&text);
            let mut hist = History::new();
            let sels: Vec<Selection> = (0..1 + rng.below(4))
                .map(|_| Selection {
                    anchor: rng.below(text.len() + 4),
                    caret: rng.below(text.len() + 4),
                    goal_col: None,
                })
                .collect();
            let p = rng.below(sels.len() + 1);
            let res = match rng.below(4) {
                0 => type_char(
                    &mut buf,
                    &mut hist,
                    &sels,
                    p,
                    *rng.pick(&['(', ')', '}', '"', '\'', '`', 'a', ']']),
                    &c,
                ),
                1 => enter(&mut buf, &mut hist, &sels, p, rng.pick(&["\n", "\r\n"]), &c),
                2 => backspace(&mut buf, &mut hist, &sels, p, &c),
                _ => list_indent(&mut buf, &mut hist, sels[0], rng.chance(50), &c)
                    .map(|o| (o.clone(), vec![o.sel], 0)),
            };
            if let Some((o, list, pi)) = res {
                let after = buf.rope().to_string();
                if let Some(ch) = &o.change {
                    check_change(&text, &after, ch);
                    undo(&mut buf, &mut hist).unwrap();
                    assert_eq!(buf.rope().to_string(), text, "{text:?}");
                }
                assert!(pi < list.len());
            } else {
                assert_eq!(buf.rope().to_string(), text);
            }
        }
    }

    // ---- Markdown

    fn md() -> Assist {
        cfg("markdown")
    }

    #[test]
    fn enter_continues_lists() {
        let c = md();
        assert_eq!(nl("- a|", &c).unwrap(), "- a\n- |");
        assert_eq!(nl("  * a|", &c).unwrap(), "  * a\n  * |");
        assert_eq!(nl("+   a|", &c).unwrap(), "+   a\n+   |");
        assert_eq!(nl("\t- a|", &c).unwrap(), "\t- a\n\t- |");
        // Inside the item: the rest moves to the new item.
        assert_eq!(nl("- ab|cd", &c).unwrap(), "- ab\n- |cd");
        // Numbers count up, either delimiter; later items are not renumbered.
        assert_eq!(nl("1. a|", &c).unwrap(), "1. a\n2. |");
        assert_eq!(nl("9) a|", &c).unwrap(), "9) a\n10) |");
        assert_eq!(nl("1. a|\n2. b", &c).unwrap(), "1. a\n2. |\n2. b");
        assert_eq!(nl("007. a|", &c).unwrap(), "007. a\n008. |");
        // Tasks continue unchecked.
        assert_eq!(nl("- [x] a|", &c).unwrap(), "- [x] a\n- [ ] |");
        assert_eq!(nl("- [ ] a|", &c).unwrap(), "- [ ] a\n- [ ] |");
        assert_eq!(nl("3. [X] a|", &c).unwrap(), "3. [X] a\n4. [ ] |");
        // Quotes, nested too, and a list in a quote.
        assert_eq!(nl("> a|", &c).unwrap(), "> a\n> |");
        assert_eq!(nl("> > a|", &c).unwrap(), "> > a\n> > |");
        assert_eq!(nl("  >a|", &c).unwrap(), "  >a\n  >|");
        assert_eq!(nl("> - a|", &c).unwrap(), "> - a\n> - |");
        // Not lists: `-` without a space, a number without a delimiter, `**bold**`, a rule.
        assert_eq!(nl("-a|", &c).unwrap(), "-a\n|");
        assert_eq!(nl("12 a|", &c).unwrap(), "12 a\n|");
        assert_eq!(nl("**a**|", &c).unwrap(), "**a**\n|");
        assert_eq!(nl("---|", &c).unwrap(), "---\n|");
        // Caret in the marker or the indent: the plain Enter.
        assert_eq!(nl("-| a", &c).unwrap(), "-\n| a");
        assert_eq!(nl("|- a", &c).unwrap(), "\n|- a");
        // Other languages do not.
        assert_eq!(nl("- a|", &cfg("rust")).unwrap(), "- a\n|");
        // A number too long to be one.
        assert_eq!(nl("1234567890. a|", &c).unwrap(), "1234567890. a\n|");
    }

    #[test]
    fn enter_on_an_empty_item_ends_it() {
        let c = md();
        assert_eq!(nl("- |", &c).unwrap(), "|");
        assert_eq!(nl("-   |", &c).unwrap(), "|");
        assert_eq!(nl("- [ ] |", &c).unwrap(), "|");
        assert_eq!(nl("1. |", &c).unwrap(), "|");
        assert_eq!(nl("a\n- |\nb", &c).unwrap(), "a\n|\nb");
        // Nested: one level out, the marker kept.
        assert_eq!(nl("- a\n    - |", &c).unwrap(), "- a\n- |");
        assert_eq!(nl("      - |", &c).unwrap(), "  - |");
        assert_eq!(nl("\t- |", &c).unwrap(), "- |");
        assert_eq!(nl("    1. |", &c).unwrap(), "1. |");
        // In a quote: the quote stays; an empty quote ends, a nested one goes one level.
        assert_eq!(nl("> - |", &c).unwrap(), "> |");
        assert_eq!(nl("> |", &c).unwrap(), "|");
        assert_eq!(nl("> > |", &c).unwrap(), "> |");
        // Trailing blanks after the marker, the caret before them.
        assert_eq!(nl("- |  ", &c).unwrap(), "- \n|  ");
        // A selection is replaced, never ends the item.
        assert_eq!(nl("- ‹a›", &c).unwrap(), "- \n- |");
    }

    #[test]
    fn markdown_enter_with_crlf_and_hostile_lines() {
        let c = md();
        let (text, sels) = parse("- a|\r\n- b");
        let mut buf = Buffer::from_text(&text);
        let mut hist = History::new();
        enter(&mut buf, &mut hist, &sels, 0, "\r\n", &c).unwrap();
        assert_eq!(buf.rope().to_string(), "- a\r\n- \r\n- b");
        // Several carets in several items.
        assert_eq!(nl("- a|\n1. b|", &c).unwrap(), "- a\n- |\n1. b\n2. |");
        // A very long line is not read whole and never panics.
        let long = format!("- {}|", "a".repeat(10_000));
        assert!(nl(&long, &c).unwrap().ends_with("\n- |"));
        let blanks = format!("- {}|", " ".repeat(10_000));
        assert!(nl(&blanks, &c).is_some());
        let deep = format!("{}a|", "> ".repeat(3000));
        assert!(nl(&deep, &c).is_some());
    }

    fn tab(s: &str, out: bool) -> Option<String> {
        let (text, sels) = parse(s);
        let mut buf = Buffer::from_text(&text);
        let mut hist = History::new();
        let o = list_indent(&mut buf, &mut hist, sels[0], out, &md())?;
        let after = buf.rope().to_string();
        check_change(&text, &after, o.change.as_ref().unwrap());
        assert_eq!(hist.steps(), (1, 0));
        undo(&mut buf, &mut hist).unwrap();
        assert_eq!(buf.rope().to_string(), text);
        Some(show(&after, &[o.sel]))
    }

    #[test]
    fn tab_moves_list_items() {
        assert_eq!(tab("- |a", false).unwrap(), "    - |a");
        assert_eq!(tab("- |", false).unwrap(), "    - |");
        assert_eq!(tab("- a|", false), None);
        assert_eq!(tab("-| a", false).unwrap(), "    -| a");
        assert_eq!(tab("|- a", false).unwrap(), "    |- a");
        assert_eq!(tab("    - |a", true).unwrap(), "- |a");
        assert_eq!(tab("  - |a", true).unwrap(), "- |a");
        assert_eq!(tab("\t- |a", true).unwrap(), "- |a");
        assert_eq!(tab("- |a", true), None);
        // The caret in the item's text: the normal Tab.
        assert_eq!(tab("- a|b", false), None);
        // Not a list line.
        assert_eq!(tab("a|", false), None);
        // Quotes: the unit goes after the markers.
        assert_eq!(tab("> - |a", false).unwrap(), ">     - |a");
        assert_eq!(tab(">     - |a", true).unwrap(), "> - |a");
        // Several lines: the list ones move, the others stay; numbers are not changed.
        assert_eq!(
            tab("‹- a\nb\n2. c›", false).unwrap(),
            "‹    - a\nb\n    2. c›"
        );
        assert_eq!(tab("›    - a\n    - b‹", true).unwrap(), "›- a\n- b‹");
        // Ending at column 0 leaves the next line.
        assert_eq!(tab("‹- a\n›- b", false).unwrap(), "‹    - a\n›- b");
        // CRLF.
        assert_eq!(tab("‹- a\r\n- b›", false).unwrap(), "‹    - a\r\n    - b›");
        // Nothing to move.
        assert_eq!(tab("‹a\nb›", false), None);
    }
}
