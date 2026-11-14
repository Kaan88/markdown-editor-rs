//! Word completion without a language server: the words of the text, ranked against what has
//! been typed of one. Headless; the shell decides when to ask and draws the list.
//!
//! The list is made once, when it opens ([`Words::scan`] over the active document around the
//! caret and over the other documents, within the limits of [`crate::limits`]); typing on only
//! narrows it ([`narrow`]), so the keystroke path never reads the text again.
//!
//! A word is a run of letters, digits and `_` of at least [`WORD_MIN`] bytes. A candidate matches
//! what was typed when it begins with the same letter (any case) and holds the typed letters in
//! order, scored by `mdshared_app::fuzzy` (a candidate that begins with them wins). Snippet
//! triggers are candidates too and come first among equals.

use crate::buffer::Buffer;
use crate::limits::{COMPLETE_PREFIX_MAX, COMPLETE_WORD_MAX, COMPLETE_WORDS};
use mdshared_app::fuzzy;
use std::collections::HashMap;
use std::ops::Range;

/// The shortest word that is offered.
pub const WORD_MIN: usize = 3;

pub fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// The word letters just before `caret`, at most [`COMPLETE_PREFIX_MAX`] bytes. Empty when the
/// caret is not at the end of a word (or in the middle of a character).
pub fn prefix_range(buf: &Buffer, caret: usize) -> Range<usize> {
    let caret = caret.min(buf.len_bytes());
    let line_start = buf.line_range(buf.line_of(caret)).start;
    let mut start = caret;
    while start > line_start && caret - start < COMPLETE_PREFIX_MAX {
        let prev = buf.prev_grapheme(start);
        // A grapheme of several characters (a letter and its accent) is not walked over
        // half-way; only a plain word character moves the start.
        let ok = prev < start
            && buf.char_at(prev).is_some_and(is_word_char)
            && buf.next_grapheme(prev) == start;
        if !ok {
            break;
        }
        start = prev;
    }
    start..caret
}

/// What a candidate is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Word,
    /// The snippet at this index of the list the shell holds.
    Snippet(usize),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cand {
    /// The word, or the snippet's trigger.
    pub text: String,
    pub kind: Kind,
    /// How often a word occurs; breaks ties.
    pub weight: u32,
}

/// The words of some text with their counts.
#[derive(Default)]
pub struct Words {
    map: HashMap<String, u32>,
    /// The cap of [`COMPLETE_WORDS`] was reached: later new words were left out.
    pub full: bool,
}

impl Words {
    pub fn new() -> Words {
        Words::default()
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    pub fn add(&mut self, word: &str) {
        if word.len() < WORD_MIN || word.len() > COMPLETE_WORD_MAX {
            return;
        }
        if let Some(n) = self.map.get_mut(word) {
            *n = n.saturating_add(1);
        } else if self.map.len() < COMPLETE_WORDS {
            self.map.insert(word.to_owned(), 1);
        } else {
            self.full = true;
        }
    }

    /// Adds the words of `buf` within `budget` bytes around `around` (the window is cut at line
    /// breaks, and a word cut by its ends is left out). Returns the bytes read.
    pub fn scan(&mut self, buf: &Buffer, around: usize, budget: usize) -> usize {
        let len = buf.len_bytes();
        let (mut a, mut b) = (0, len);
        if len > budget {
            let want_a = around.saturating_sub(budget / 2).min(len - budget);
            let want_b = want_a + budget;
            a = if want_a == 0 {
                0
            } else {
                let l = buf.line_of(want_a);
                if buf.line_range(l).start == want_a {
                    want_a
                } else {
                    buf.line_range_with_break(l).end
                }
            };
            b = if want_b >= len {
                len
            } else {
                buf.line_range(buf.line_of(want_b)).start
            };
            b = b.max(a);
        }
        let mut text = String::new();
        buf.text(a..b, &mut text);
        self.add_text(&text, false, false);
        b - a
    }

    /// Adds the words of `text`; `cut_start` and `cut_end` say that the text starts or ends in
    /// the middle of a word, which is then left out.
    pub fn add_text(&mut self, text: &str, cut_start: bool, cut_end: bool) {
        let mut start: Option<usize> = None;
        let mut first = true;
        for (i, c) in text.char_indices() {
            if is_word_char(c) {
                if start.is_none() {
                    start = Some(i);
                }
            } else if let Some(s) = start.take() {
                if !(first && cut_start && s == 0) {
                    self.add(&text[s..i]);
                }
                first = false;
            } else {
                first = false;
            }
        }
        if let Some(s) = start
            && !cut_end
            && !(first && cut_start && s == 0)
        {
            self.add(&text[s..]);
        }
    }

    /// The words as candidates, unordered.
    pub fn into_cands(self) -> Vec<Cand> {
        self.map
            .into_iter()
            .map(|(text, weight)| Cand {
                text,
                kind: Kind::Word,
                weight,
            })
            .collect()
    }
}

fn first_fold(s: &str) -> Option<char> {
    s.chars()
        .next()
        .map(|c| c.to_lowercase().next().unwrap_or(c))
}

/// The letters of `q` in order in `c`, ignoring case: the cheap test before scoring.
fn subsequence(q: &str, c: &str) -> bool {
    let mut cs = c.chars().flat_map(char::to_lowercase);
    q.chars()
        .flat_map(char::to_lowercase)
        .all(|qc| cs.any(|x| x == qc))
}

fn starts_with_fold(c: &str, q: &str) -> bool {
    let mut cs = c.chars().flat_map(char::to_lowercase);
    q.chars()
        .flat_map(char::to_lowercase)
        .all(|qc| cs.next() == Some(qc))
}

/// The candidates of `pool` that match `prefix`, best first. A word equal to the prefix is
/// left out (it is what is typed); a snippet whose trigger equals it is kept. With an empty
/// prefix only snippets match. Narrowing a list that was made for a shorter prefix of the same
/// word gives the same result as starting again, so the shell passes the survivors on.
pub fn narrow(prefix: &str, pool: Vec<Cand>) -> Vec<Cand> {
    let Some(first) = first_fold(prefix) else {
        let mut snippets: Vec<Cand> = pool
            .into_iter()
            .filter(|c| matches!(c.kind, Kind::Snippet(_)))
            .collect();
        snippets.sort_by(|a, b| a.text.cmp(&b.text));
        return snippets;
    };
    let mut scored: Vec<(bool, i32, Cand)> = pool
        .into_iter()
        .filter(|c| c.kind != Kind::Word || c.text != prefix)
        .filter(|c| first_fold(&c.text) == Some(first) && subsequence(prefix, &c.text))
        .filter_map(|c| {
            let m = fuzzy::score(prefix, &c.text)?;
            Some((starts_with_fold(&c.text, prefix), m.score, c))
        })
        .collect();
    scored.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| {
                let snip = |c: &Cand| matches!(c.kind, Kind::Snippet(_));
                snip(&b.2).cmp(&snip(&a.2))
            })
            .then_with(|| b.1.cmp(&a.1))
            .then_with(|| b.2.weight.cmp(&a.2.weight))
            .then_with(|| a.2.text.len().cmp(&b.2.text.len()))
            .then_with(|| a.2.text.cmp(&b.2.text))
    });
    scored.into_iter().map(|(_, _, c)| c).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(text: &str) -> Words {
        let mut w = Words::new();
        w.add_text(text, false, false);
        w
    }

    fn texts(v: &[Cand]) -> Vec<&str> {
        v.iter().map(|c| c.text.as_str()).collect()
    }

    #[test]
    fn words_are_runs_of_letters_digits_and_underscores() {
        let w = words("let foo_bar = baz42(ab, héllo);\nfoo_bar x");
        let mut c = w.into_cands();
        c.sort_by(|a, b| a.text.cmp(&b.text));
        assert_eq!(texts(&c), ["baz42", "foo_bar", "héllo", "let"]);
        assert_eq!(c[1].weight, 2);
    }

    #[test]
    fn short_and_long_words_are_left_out() {
        let long = "x".repeat(COMPLETE_WORD_MAX + 1);
        let w = words(&format!("ab abc {long}"));
        assert_eq!(texts(&w.into_cands()), ["abc"]);
    }

    #[test]
    fn a_cut_word_at_the_ends_is_left_out() {
        let mut w = Words::new();
        w.add_text("tial whole par", true, true);
        let mut c = w.into_cands();
        c.sort_by(|a, b| a.text.cmp(&b.text));
        assert_eq!(texts(&c), ["whole"]);
        let mut w = Words::new();
        w.add_text("   ok end ", true, true);
        assert_eq!(texts(&w.into_cands()), ["end"]);
    }

    #[test]
    fn the_list_is_capped() {
        let mut w = Words::new();
        for i in 0..COMPLETE_WORDS + 10 {
            w.add(&format!("word{i}"));
        }
        assert_eq!(w.len(), COMPLETE_WORDS);
        assert!(w.full);
        // A word already in still counts.
        w.add("word0");
    }

    #[test]
    fn scan_reads_the_whole_of_a_small_text() {
        let b = Buffer::from_text("alpha beta\ngamma");
        let mut w = Words::new();
        assert_eq!(w.scan(&b, 3, 1 << 20), b.len_bytes());
        assert_eq!(w.len(), 3);
    }

    #[test]
    fn scan_of_a_large_text_stays_near_the_caret_on_whole_lines() {
        let text: String = (0..1000).map(|i| format!("line{i:04} word\n")).collect();
        let b = Buffer::from_text(&text);
        let mut w = Words::new();
        let around = text.find("line0500").unwrap();
        let read = w.scan(&b, around, 2000);
        assert!(read <= 2000 + 16, "{read}");
        let c = w.into_cands();
        let t = texts(&c);
        assert!(t.contains(&"line0500"));
        assert!(!t.contains(&"line0001"));
        assert!(!t.contains(&"line0999"));
        // No word is cut: every line word has all its digits.
        assert!(t.iter().all(|w| *w == "word" || w.len() == 8), "{t:?}");
    }

    #[test]
    fn prefix_is_the_word_before_the_caret() {
        let b = Buffer::from_text("foo bar_ba");
        assert_eq!(prefix_range(&b, 10), 4..10);
        assert_eq!(prefix_range(&b, 3), 0..3);
        assert_eq!(prefix_range(&b, 4), 4..4);
        assert_eq!(prefix_range(&b, 0), 0..0);
        let b = Buffer::from_text("a\nbc");
        assert_eq!(prefix_range(&b, 4), 2..4);
        let b = Buffer::from_text("été");
        assert_eq!(prefix_range(&b, b.len_bytes()), 0..b.len_bytes());
    }

    #[test]
    fn a_long_run_of_letters_gives_a_bounded_prefix() {
        let b = Buffer::from_text(&"a".repeat(1000));
        assert_eq!(prefix_range(&b, 1000).len(), COMPLETE_PREFIX_MAX);
    }

    fn cands(list: &[&str]) -> Vec<Cand> {
        list.iter()
            .map(|t| Cand {
                text: (*t).into(),
                kind: Kind::Word,
                weight: 1,
            })
            .collect()
    }

    #[test]
    fn narrowing_ranks_prefixes_first_and_drops_the_typed_word() {
        let pool = cands(&["parser", "pars", "spare", "pa_rse_r", "parse", "other"]);
        let r = narrow("pars", pool);
        assert_eq!(texts(&r), ["parse", "parser", "pa_rse_r"]);
        // Case does not matter for matching; the candidate is inserted as it is.
        let r = narrow("PAR", cands(&["parser", "Parent"]));
        assert_eq!(r.len(), 2);
    }

    #[test]
    fn narrowing_the_survivors_equals_starting_again() {
        let pool = cands(&["configure", "config_file", "console", "contour", "cfg_x"]);
        let a = narrow("co", pool.clone());
        let b = narrow("conf", a.clone());
        assert_eq!(b, narrow("conf", pool));
        assert_eq!(texts(&b), ["configure", "config_file"]);
    }

    #[test]
    fn snippets_come_first_and_stay_when_equal_to_the_prefix() {
        let mut pool = cands(&["tablet", "table"]);
        pool.push(Cand {
            text: "table".into(),
            kind: Kind::Snippet(0),
            weight: 0,
        });
        let r = narrow("table", pool);
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].kind, Kind::Snippet(0));
        assert_eq!(r[1].text, "tablet");
        let mut pool = cands(&["tablet"]);
        pool.push(Cand {
            text: "tab".into(),
            kind: Kind::Snippet(1),
            weight: 0,
        });
        let r = narrow("tab", pool);
        assert_eq!(r[0].kind, Kind::Snippet(1));
        assert_eq!(r[1].text, "tablet");
    }

    #[test]
    fn an_empty_prefix_offers_only_snippets() {
        let mut pool = cands(&["word"]);
        pool.push(Cand {
            text: "link".into(),
            kind: Kind::Snippet(2),
            weight: 0,
        });
        let r = narrow("", pool);
        assert_eq!(texts(&r), ["link"]);
    }

    #[test]
    fn heavier_words_win_ties() {
        let mut pool = cands(&["abcd", "abce"]);
        pool[1].weight = 5;
        assert_eq!(texts(&narrow("abc", pool)), ["abce", "abcd"]);
    }
}
