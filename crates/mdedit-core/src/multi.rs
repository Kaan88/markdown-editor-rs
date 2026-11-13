//! Several selections at once ("multiple carets"): one edit applied at every selection as a single
//! undo step, motions applied to every selection, and the column (rectangular) selection.
//!
//! The selections are a plain list. [`normalize`] puts it in order and merges what overlaps, and
//! every function here returns a normalised list together with the index of the primary selection
//! (the one the shell scrolls to and shows in the status bar). Cost is proportional to the number
//! of selections (times a rope lookup each), never quadratic, so a column selection down a large
//! file is fine.

use crate::buffer::Buffer;
use crate::edit::{Cmd, Outcome, nothing};
use crate::history::{History, Kind};
use crate::selection::{self, Motion, Selection};
use std::ops::Range;

/// What every selection does in one multi-caret edit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MultiOp<'a> {
    /// Replace each selection with this text.
    Insert(&'a str),
    /// Replace the k-th selection IN DOCUMENT ORDER with the k-th text. Falls back to
    /// nothing-happens (no change) unless `texts.len()` equals the number of selections after
    /// normalising.
    InsertEach(&'a [&'a str]),
    /// Remove each selection, or the grapheme before each caret that has none.
    Backspace,
    /// Remove each selection, or the grapheme after each caret that has none.
    Delete,
}

/// Sorts selections by position and merges those that overlap or touch at a caret
/// (two empty selections at the same offset become one; an empty selection inside or at the
/// edge of a non-empty one is absorbed; two non-empty ones that overlap become their union,
/// keeping the direction (anchor before or after caret) of the one named by `primary` if it is
/// involved, else of the first). `primary` is an index into `sels`; the result is the
/// normalised list in document order and the index of the selection that holds the primary.
/// Offsets are clamped to the buffer and snapped to grapheme boundaries. `goal_col` of a
/// selection that was not merged is kept.
///
/// Details the contract leaves open: two non-empty selections that only touch (`a..b` and
/// `b..c`) stay separate; an empty primary does not decide a merged direction (the first
/// non-empty member does; "first" is by start, then end, in the normalised order); a merged selection has no `goal_col`; a `primary` past the end of
/// `sels` counts as 0; an empty list gives an empty list and index 0.
pub fn normalize(buf: &Buffer, sels: &[Selection], primary: usize) -> (Vec<Selection>, usize) {
    if sels.is_empty() {
        return (Vec::new(), 0);
    }
    let primary = if primary < sels.len() { primary } else { 0 };
    // (fixed selection, index in `sels`)
    let mut v: Vec<(Selection, usize)> = sels
        .iter()
        .enumerate()
        .map(|(i, s)| {
            (
                Selection {
                    anchor: buf.snap_to_grapheme(s.anchor),
                    caret: buf.snap_to_grapheme(s.caret),
                    goal_col: s.goal_col,
                },
                i,
            )
        })
        .collect();
    v.sort_unstable_by_key(|(s, i)| {
        let r = s.range();
        (r.start, r.end, *i)
    });
    let mut out: Vec<Selection> = Vec::with_capacity(v.len());
    let mut pidx = 0;
    let mut i = 0;
    while i < v.len() {
        let first = v[i].0;
        let r = first.range();
        let (lo, mut hi) = (r.start, r.end);
        let mut count = 1;
        let mut holds_primary = v[i].1 == primary;
        // The direction of the first non-empty member, and of the primary when it is non-empty.
        let mut first_dir = (!first.is_empty()).then_some(first.anchor > first.caret);
        let mut primary_dir =
            (holds_primary && !first.is_empty()).then_some(first.anchor > first.caret);
        let mut j = i + 1;
        while j < v.len() {
            let s = v[j].0;
            let r = s.range();
            let touches = r.start < hi || (r.start == hi && (r.is_empty() || lo == hi));
            if !touches {
                break;
            }
            count += 1;
            hi = hi.max(r.end);
            if !s.is_empty() {
                let back = s.anchor > s.caret;
                first_dir.get_or_insert(back);
                if v[j].1 == primary {
                    primary_dir = Some(back);
                }
            }
            holds_primary |= v[j].1 == primary;
            j += 1;
        }
        if holds_primary {
            pidx = out.len();
        }
        if count == 1 {
            out.push(first);
        } else {
            let back = primary_dir.or(first_dir).unwrap_or(false);
            let (anchor, caret) = if back { (hi, lo) } else { (lo, hi) };
            out.push(Selection {
                anchor,
                caret,
                goal_col: None,
            });
        }
        i = j;
    }
    (out, pidx)
}

/// The text `op` puts at the k-th selection.
fn text_for<'a>(op: MultiOp<'a>, k: usize) -> &'a str {
    match op {
        MultiOp::Insert(t) => t,
        MultiOp::InsertEach(texts) => texts[k],
        MultiOp::Backspace | MultiOp::Delete => "",
    }
}

/// The range the k-th selection replaces, in the text before the edit.
fn range_for(buf: &Buffer, op: MultiOp<'_>, sel: Selection) -> Range<usize> {
    if !sel.is_empty() {
        return sel.range();
    }
    let at = sel.caret;
    match op {
        MultiOp::Backspace => buf.prev_grapheme(at)..at,
        MultiOp::Delete => at..buf.next_grapheme(at),
        MultiOp::Insert(_) | MultiOp::InsertEach(_) => at..at,
    }
}

/// Applies `op` at every selection as one undo step (`Kind::Other`). `sels` need not be
/// normalised: this normalises first (with `primary`). Ranges to remove are worked out on the
/// text as it is before the edit; where two of them overlap (two carets backspacing over the
/// same grapheme) they are merged and the text is removed once. Returns the outcome (its `sel`
/// is the primary selection after the edit, its `change` covers every replacement, `None` when
/// nothing changed), every selection after the edit in document order (each an empty selection
/// at the end of its inserted text, or at the place of its removal; merged again if any now
/// coincide), and the index of the primary in that list. When nothing changes, the returned
/// list is the normalised input.
///
/// Undo puts back the primary selection only (the others are the shell's to keep or drop).
pub fn edit(
    buf: &mut Buffer,
    hist: &mut History,
    sels: &[Selection],
    primary: usize,
    op: MultiOp<'_>,
) -> (Outcome, Vec<Selection>, usize) {
    let (norm, p) = normalize(buf, sels, primary);
    if norm.is_empty() {
        return (nothing(Selection::at(0)), norm, 0);
    }
    let before = norm[p];
    if let MultiOp::InsertEach(texts) = op
        && texts.len() != norm.len()
    {
        return (nothing(before), norm, p);
    }
    // The replacements in document order, merged where removals overlap, and for every selection
    // where its caret ends up in the new text.
    let mut groups: Vec<(usize, usize, &str)> = Vec::with_capacity(norm.len());
    let mut after: Vec<usize> = Vec::with_capacity(norm.len());
    // The new text's offsets are the old ones plus this, for what lies after the groups before
    // the last one.
    let mut delta: isize = 0;
    for (k, sel) in norm.iter().enumerate() {
        let r = range_for(buf, op, *sel);
        let t = text_for(op, k);
        if let Some(g) = groups.last_mut()
            && t.is_empty()
            && g.2.is_empty()
            && r.start < g.1
        {
            g.1 = g.1.max(r.end);
            after.push((g.0 as isize + delta) as usize);
            continue;
        }
        if let Some(g) = groups.last() {
            delta += g.2.len() as isize - (g.1 - g.0) as isize;
        }
        groups.push((r.start, r.end, t));
        after.push((r.start as isize + delta) as usize + t.len());
    }
    let mut cmd = Cmd::new(buf);
    // Back to front, so the offsets of the text before the edit stay valid.
    for &(s, e, t) in groups.iter().rev() {
        cmd.replace(s..e, t);
    }
    let out = cmd.finish(hist, Kind::Other, before, Selection::at(after[p]));
    if out.change.is_none() {
        return (out, norm, p);
    }
    let carets: Vec<Selection> = after.iter().map(|&o| Selection::at(o)).collect();
    let (list, pi) = normalize(buf, &carets, p);
    (
        Outcome {
            sel: list[pi],
            change: out.change,
        },
        list,
        pi,
    )
}

/// Applies `motion` to every selection (`selection::apply`, with `extend` as given), then
/// normalises. Returns the list in document order and the primary's index.
pub fn move_all(
    buf: &Buffer,
    sels: &[Selection],
    primary: usize,
    motion: Motion,
    extend: bool,
    tab_width: usize,
) -> (Vec<Selection>, usize) {
    let moved: Vec<Selection> = sels
        .iter()
        .map(|&s| selection::apply(buf, s, motion, extend, tab_width))
        .collect();
    normalize(buf, &moved, primary)
}

/// A column selection: on every line from `anchor_line` to `caret_line` (either may be the
/// larger; both clamped to the buffer), a selection from display column `anchor_col` to display
/// column `caret_col` (anchor at `anchor_col`, caret at `caret_col`; a column past a line's end
/// is the line's end, so short lines get shorter or empty selections; a column inside a tab is
/// the tab's start, as `offset_at_display_col` gives). Returns them in document order and the
/// index of the one on `caret_line` (the primary). `goal_col` is `None`.
pub fn column(
    buf: &Buffer,
    anchor_line: usize,
    anchor_col: usize,
    caret_line: usize,
    caret_col: usize,
    tab_width: usize,
) -> (Vec<Selection>, usize) {
    let last = buf.len_lines().saturating_sub(1);
    let (a, c) = (anchor_line.min(last), caret_line.min(last));
    let (lo, hi) = (a.min(c), a.max(c));
    let list = (lo..=hi)
        .map(|line| Selection {
            anchor: selection::offset_at_display_col(buf, line, anchor_col, tab_width),
            caret: selection::offset_at_display_col(buf, line, caret_col, tab_width),
            goal_col: None,
        })
        .collect();
    (list, c - lo)
}

/// The display column of `offset` on its line (re-exported from `selection`, for the shell).
pub fn display_col(buf: &Buffer, offset: usize, tab_width: usize) -> usize {
    selection::display_col(buf, offset, tab_width)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::{redo, undo};
    use crate::testutil::{Rng, check_change, lines_of};
    use unicode_segmentation::UnicodeSegmentation;

    fn txt(buf: &Buffer) -> String {
        buf.rope().to_string()
    }

    fn s(anchor: usize, caret: usize) -> Selection {
        Selection {
            anchor,
            caret,
            goal_col: None,
        }
    }

    fn at(o: usize) -> Selection {
        Selection::at(o)
    }

    fn carets(v: &[Selection]) -> Vec<usize> {
        v.iter().map(|x| x.caret).collect()
    }

    fn norm(text: &str, sels: &[Selection], primary: usize) -> (Vec<Selection>, usize) {
        normalize(&Buffer::from_text(text), sels, primary)
    }

    /// Runs `edit` on a fresh buffer, checks the change, one undo step, undo and redo, and
    /// returns the new text, the list and the primary index.
    fn go(
        text: &str,
        sels: &[Selection],
        primary: usize,
        op: MultiOp<'_>,
    ) -> (String, Vec<Selection>, usize) {
        let mut buf = Buffer::from_text(text);
        let mut hist = History::new();
        let (want_before, wp) = normalize(&buf, sels, primary);
        let (out, list, pi) = edit(&mut buf, &mut hist, sels, primary, op);
        let after = txt(&buf);
        match &out.change {
            None => {
                assert_eq!(after, text);
                assert_eq!(list, want_before);
                assert_eq!(pi, wp);
                assert_eq!(out.sel, want_before.get(wp).copied().unwrap_or(at(0)));
                assert!(!hist.can_undo());
                assert_eq!(buf.revision(), 0);
            }
            Some(c) => {
                check_change(text, &after, c);
                assert_eq!(hist.steps(), (1, 0), "one undo step");
                assert_eq!(out.sel, list[pi]);
                let g = unicode_bounds(&after);
                for x in &list {
                    assert!(
                        x.is_empty() && g.contains(&x.caret),
                        "{list:?} in {after:?}"
                    );
                }
                assert!(list.windows(2).all(|w| w[0].caret < w[1].caret));
                let u = undo(&mut buf, &mut hist).unwrap();
                assert_eq!(txt(&buf), text);
                assert_eq!(u.sel, want_before[wp]);
                check_change(&after, text, u.change.as_ref().unwrap());
                assert!(!hist.can_undo() && !hist.is_modified());
                let r = redo(&mut buf, &mut hist).unwrap();
                assert_eq!(txt(&buf), after);
                assert_eq!(r.sel, out.sel);
            }
        }
        (after, list, pi)
    }

    fn unicode_bounds(t: &str) -> Vec<usize> {
        let mut v: Vec<usize> = t.grapheme_indices(true).map(|(i, _)| i).collect();
        v.push(t.len());
        v
    }

    // ---- normalize

    #[test]
    fn normalize_sorts_and_tracks_the_primary() {
        let (v, p) = norm("abcdefghij", &[at(5), at(1), at(3)], 0);
        assert_eq!((carets(&v), p), (vec![1, 3, 5], 2));
        let (v, p) = norm("abcdefghij", &[at(5), at(1), at(3)], 1);
        assert_eq!((carets(&v), p), (vec![1, 3, 5], 0));
        let (v, p) = norm("abcdefghij", &[s(8, 6), at(1)], 0);
        assert_eq!((v, p), (vec![at(1), s(8, 6)], 1));
        let (v, p) = norm("abc", &[], 0);
        assert!(v.is_empty() && p == 0);
        // A primary past the end counts as the first.
        let (v, p) = norm("abcdef", &[at(4), at(2)], 7);
        assert_eq!((carets(&v), p), (vec![2, 4], 1));
    }

    #[test]
    fn normalize_merges_duplicates_and_touching_carets() {
        let (v, p) = norm("abcdef", &[at(2), at(2), at(2)], 2);
        assert_eq!((v, p), (vec![at(2)], 0));
        // An empty selection at either edge of, or inside, a non-empty one is absorbed.
        for e in [1, 2, 3] {
            let (v, p) = norm("abcdef", &[s(1, 3), at(e)], 1);
            assert_eq!((v, p), (vec![s(1, 3)], 0), "caret at {e}");
            let (v, p) = norm("abcdef", &[at(e), s(1, 3)], 0);
            assert_eq!((v, p), (vec![s(1, 3)], 0), "caret at {e}");
        }
        // Two non-empty ones that only touch stay apart.
        let (v, p) = norm("abcdef", &[s(3, 5), s(1, 3)], 0);
        assert_eq!((v, p), (vec![s(1, 3), s(3, 5)], 1));
        // ... and a caret between them joins the first, not the second, and does not bridge.
        let (v, _) = norm("abcdef", &[s(1, 3), at(3), s(3, 5)], 0);
        assert_eq!(v, vec![s(1, 3), s(3, 5)]);
        // Carets one apart stay apart.
        let (v, _) = norm("abcdef", &[at(2), at(3)], 0);
        assert_eq!(carets(&v), vec![2, 3]);
    }

    #[test]
    fn normalize_merges_nested_and_overlapping_with_direction() {
        let t = "abcdefghij";
        assert_eq!(norm(t, &[s(1, 6), s(2, 4)], 0).0, vec![s(1, 6)]);
        assert_eq!(norm(t, &[s(1, 6), s(4, 2)], 1).0, vec![s(6, 1)]);
        assert_eq!(norm(t, &[s(1, 6), s(4, 2)], 0).0, vec![s(1, 6)]);
        // The primary is not involved: the first in document order (by start, then end)
        // decides, here 1..6 and not 2..4.
        let (v, p) = norm(t, &[s(4, 2), s(1, 6), at(9)], 2);
        assert_eq!((v, p), (vec![s(1, 6), at(9)], 1));
        let (v, _) = norm(t, &[s(2, 4), s(6, 1), at(9)], 2);
        assert_eq!(v, vec![s(6, 1), at(9)]);
        // Overlap, both directions.
        assert_eq!(norm(t, &[s(1, 4), s(3, 6)], 1).0, vec![s(1, 6)]);
        assert_eq!(norm(t, &[s(4, 1), s(3, 6)], 0).0, vec![s(6, 1)]);
        assert_eq!(norm(t, &[s(4, 1), s(6, 3)], 1).0, vec![s(6, 1)]);
        // A chain of three.
        assert_eq!(norm(t, &[s(5, 7), s(1, 3), s(2, 6)], 0).0, vec![s(1, 7)]);
        // The primary is a caret absorbed into a backward selection: the selection decides.
        assert_eq!(norm(t, &[at(3), s(5, 1)], 0), (vec![s(5, 1)], 0));
        // A merged primary index.
        let (v, p) = norm(t, &[at(9), s(1, 4), at(2)], 2);
        assert_eq!((v, p), (vec![s(1, 4), at(9)], 0));
    }

    #[test]
    fn normalize_clamps_snaps_and_keeps_goal_col() {
        assert_eq!(norm("abc", &[at(100)], 0).0, vec![at(3)]);
        assert_eq!(norm("abc", &[s(50, 1)], 0).0, vec![s(3, 1)]);
        // Inside a two-byte char and a three-byte one.
        assert_eq!(norm("aİb", &[at(2)], 0).0, vec![at(1)]);
        assert_eq!(norm("a漢b", &[s(2, 3)], 0).0, vec![at(1)]);
        // Inside a CRLF.
        assert_eq!(norm("a\r\nb", &[at(2), at(3)], 0).0, vec![at(1), at(3)]);
        // Inside a combining sequence, and a ZWJ one.
        assert_eq!(norm("e\u{301}x", &[at(1)], 0).0, vec![at(0)]);
        let fam = "\u{1F468}\u{200D}\u{1F469}";
        assert_eq!(norm(fam, &[at(4), at(7)], 0).0, vec![at(0)]);
        // goal_col survives when nothing merges, not when something does.
        let g = Selection {
            anchor: 1,
            caret: 1,
            goal_col: Some(7),
        };
        assert_eq!(norm("abcdef", &[g, at(4)], 0).0[0], g);
        assert_eq!(norm("abcdef", &[g, at(1)], 0).0[0].goal_col, None);
    }

    // ---- insert

    #[test]
    fn insert_at_several_carets_on_one_line_and_on_several_lines() {
        let (t, v, p) = go("abcd", &[at(1), at(3)], 0, MultiOp::Insert("X"));
        assert_eq!((t.as_str(), carets(&v), p), ("aXbcXd", vec![2, 5], 0));
        // Given out of order, primary the first given.
        let (t, v, p) = go("abcd", &[at(3), at(1)], 0, MultiOp::Insert("XY"));
        assert_eq!((t.as_str(), carets(&v), p), ("aXYbcXYd", vec![3, 7], 1));
        let (t, v, p) = go(
            "abc\ndef\nghi",
            &[at(0), at(4), at(8)],
            2,
            MultiOp::Insert("> "),
        );
        assert_eq!(t, "> abc\n> def\n> ghi");
        assert_eq!((carets(&v), p), (vec![2, 8, 14], 2));
        // Two carets at the same place insert once.
        let (t, v, _) = go("ab", &[at(1), at(1)], 0, MultiOp::Insert("-"));
        assert_eq!((t.as_str(), carets(&v)), ("a-b", vec![2]));
        // At both ends and in an empty buffer.
        let (t, v, _) = go("ab", &[at(0), at(2)], 0, MultiOp::Insert("|"));
        assert_eq!((t.as_str(), carets(&v)), ("|ab|", vec![1, 4]));
        let (t, v, _) = go("", &[at(0)], 0, MultiOp::Insert("é"));
        assert_eq!((t.as_str(), carets(&v)), ("é", vec![2]));
    }

    #[test]
    fn insert_replaces_non_empty_selections() {
        let (t, v, p) = go("abcdefgh", &[s(0, 2), s(7, 3)], 1, MultiOp::Insert("-"));
        assert_eq!((t.as_str(), carets(&v), p), ("-c-h", vec![1, 3], 1));
        // Mixed with carets; a longer text than the selection.
        let (t, v, _) = go(
            "abcdefgh",
            &[at(0), s(2, 3), at(8)],
            0,
            MultiOp::Insert("<<>>"),
        );
        assert_eq!(t, "<<>>ab<<>>defgh<<>>");
        assert_eq!(carets(&v), vec![4, 10, 19]);
        // Empty text with selections deletes them; with only carets it does nothing.
        let (t, v, _) = go("abcdef", &[s(1, 2), s(3, 5)], 0, MultiOp::Insert(""));
        assert_eq!((t.as_str(), carets(&v)), ("acf", vec![1, 2]));
        let (t, v, p) = go("abc", &[at(0), at(2)], 1, MultiOp::Insert(""));
        assert_eq!((t.as_str(), carets(&v), p), ("abc", vec![0, 2], 1));
    }

    #[test]
    fn insert_each_in_document_order() {
        let (t, v, _) = go(
            "a b c",
            &[at(0), at(2), at(4)],
            0,
            MultiOp::InsertEach(&["1", "2", "3"]),
        );
        assert_eq!((t.as_str(), carets(&v)), ("1a 2b 3c", vec![1, 4, 7]));
        // Given out of order: the texts still go in document order.
        let (t, _, p) = go(
            "a b c",
            &[at(4), at(0), at(2)],
            0,
            MultiOp::InsertEach(&["x", "yy", "z"]),
        );
        assert_eq!((t.as_str(), p), ("xa yyb zc", 2));
        // Different lengths, an empty one, a replaced selection.
        let (t, v, _) = go(
            "abcdef",
            &[at(1), s(2, 4), at(6)],
            0,
            MultiOp::InsertEach(&["", "漢字", "ı"]),
        );
        assert_eq!(t, "ab漢字ef\u{131}");
        assert_eq!(carets(&v), vec![1, 8, 12]);
        // The count must equal the number after normalising: duplicates count once.
        let (t, _, _) = go("abc", &[at(1), at(1)], 0, MultiOp::InsertEach(&["z"]));
        assert_eq!(t, "azbc");
        for texts in [&["a"][..], &["a", "b", "c"][..], &[][..]] {
            let (t, v, p) = go("abcd", &[at(1), at(3)], 1, MultiOp::InsertEach(texts));
            assert_eq!((t.as_str(), carets(&v), p), ("abcd", vec![1, 3], 1));
        }
        let (t, _, _) = go("abc", &[at(1), at(1)], 0, MultiOp::InsertEach(&["a", "b"]));
        assert_eq!(t, "abc");
    }

    // ---- backspace and delete

    #[test]
    fn backspace_and_delete_with_mixed_selections() {
        let (t, v, p) = go("abcdef", &[s(1, 3), at(5), at(0)], 1, MultiOp::Backspace);
        assert_eq!((t.as_str(), carets(&v), p), ("adf", vec![0, 1, 2], 2));
        let (t, v, p) = go("abcdef", &[s(3, 1), at(5), at(0)], 1, MultiOp::Delete);
        // The caret at 0 deletes "a", touching the selection: those two end up together.
        assert_eq!((t.as_str(), carets(&v), p), ("de", vec![0, 2], 1));
        // The removals touch, so the carets end up together.
        let (t, v, _) = go("abcdef", &[s(4, 1), at(0)], 0, MultiOp::Delete);
        assert_eq!((t.as_str(), carets(&v)), ("ef", vec![0]));
        // A caret right after a selection, and right before one.
        let (t, v, _) = go("abcdef", &[s(1, 3), at(4)], 0, MultiOp::Backspace);
        assert_eq!((t.as_str(), carets(&v)), ("aef", vec![1]));
        let (t, v, _) = go("abcdef", &[at(1), s(2, 4)], 0, MultiOp::Delete);
        assert_eq!((t.as_str(), carets(&v)), ("aef", vec![1]));
    }

    #[test]
    fn backspace_and_delete_at_the_ends() {
        // Offset 0 and the end do nothing for that caret; the others still act.
        let (t, v, _) = go("abc", &[at(0), at(2)], 0, MultiOp::Backspace);
        assert_eq!((t.as_str(), carets(&v)), ("ac", vec![0, 1]));
        let (t, v, _) = go("abc", &[at(1), at(3)], 0, MultiOp::Delete);
        assert_eq!((t.as_str(), carets(&v)), ("ac", vec![1, 2]));
        // Nothing to do at all: no change, no history.
        let (t, v, p) = go("abc", &[at(0)], 0, MultiOp::Backspace);
        assert_eq!((t.as_str(), carets(&v), p), ("abc", vec![0], 0));
        let (t, _, _) = go("abc", &[at(3)], 0, MultiOp::Delete);
        assert_eq!(t, "abc");
        let (t, v, _) = go("", &[at(0), at(5)], 0, MultiOp::Backspace);
        assert_eq!((t.as_str(), carets(&v)), ("", vec![0]));
        let (t, _, _) = go("", &[at(0)], 0, MultiOp::Delete);
        assert_eq!(t, "");
    }

    #[test]
    fn adjacent_and_coinciding_carets() {
        // One grapheme apart: the removals touch, and the carets end up together.
        let (t, v, p) = go("abcd", &[at(2), at(3)], 1, MultiOp::Backspace);
        assert_eq!((t.as_str(), carets(&v), p), ("ad", vec![1], 0));
        let (t, v, p) = go("abcd", &[at(1), at(2)], 0, MultiOp::Delete);
        assert_eq!((t.as_str(), carets(&v), p), ("ad", vec![1], 0));
        // Three in a row.
        let (t, v, _) = go("abcdef", &[at(2), at(3), at(4)], 0, MultiOp::Backspace);
        assert_eq!((t.as_str(), carets(&v)), ("aef", vec![1]));
        // The same place twice removes one grapheme.
        let (t, v, _) = go("abc", &[at(2), at(2)], 0, MultiOp::Backspace);
        assert_eq!((t.as_str(), carets(&v)), ("ac", vec![1]));
        // Two apart: their removals do not touch.
        let (t, v, _) = go("abcde", &[at(2), at(4)], 0, MultiOp::Backspace);
        assert_eq!((t.as_str(), carets(&v)), ("ace", vec![1, 2]));
    }

    #[test]
    fn crlf_is_one_grapheme() {
        let (t, v, _) = go("a\r\nb\r\nc", &[at(3), at(6)], 0, MultiOp::Backspace);
        assert_eq!((t.as_str(), carets(&v)), ("abc", vec![1, 2]));
        let (t, v, _) = go("a\r\nb\r\nc", &[at(1), at(4)], 0, MultiOp::Delete);
        assert_eq!((t.as_str(), carets(&v)), ("abc", vec![1, 2]));
        // A caret between CR and LF is at the CRLF's start.
        let (t, v, _) = go("a\r\nb", &[at(2)], 0, MultiOp::Insert("x"));
        assert_eq!((t.as_str(), carets(&v)), ("ax\r\nb", vec![2]));
        let (t, _, _) = go("a\r\nb", &[at(2)], 0, MultiOp::Delete);
        assert_eq!(t, "ab");
        // A lone CR and LF are one byte each.
        let (t, _, _) = go("a\rb\nc", &[at(2), at(4)], 0, MultiOp::Backspace);
        assert_eq!(t, "abc");
    }

    #[test]
    fn multi_byte_text_and_clusters() {
        let (t, v, _) = go("çşğ", &[at(2), at(6)], 0, MultiOp::Backspace);
        assert_eq!((t.as_str(), carets(&v)), ("ş", vec![0, 2]));
        let (t, v, _) = go("漢字かな", &[at(0), at(6)], 0, MultiOp::Delete);
        assert_eq!((t.as_str(), carets(&v)), ("字な", vec![0, 3]));
        // A combining mark goes with its letter.
        let (t, v, _) = go("e\u{301}x", &[at(3), at(0)], 0, MultiOp::Backspace);
        assert_eq!((t.as_str(), carets(&v)), ("x", vec![0]));
        let fam = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}";
        let text = format!("a{fam}b{fam}");
        let end = text.len();
        let (t, v, _) = go(&text, &[at(1 + fam.len()), at(end)], 0, MultiOp::Backspace);
        assert_eq!((t.as_str(), carets(&v)), ("ab", vec![1, 2]));
        let (t, v, _) = go(&text, &[at(1), at(2 + fam.len())], 0, MultiOp::Delete);
        assert_eq!((t.as_str(), carets(&v)), ("ab", vec![1, 2]));
        // Inserting a letter before a combining mark makes a cluster: the caret snaps back to
        // the cluster's start.
        let (t, v, _) = go("\u{301}", &[at(0)], 0, MultiOp::Insert("e"));
        assert_eq!((t.as_str(), carets(&v)), ("e\u{301}", vec![0]));
        // Inside a multi-byte char: floored.
        let (t, _, _) = go("çx", &[at(1)], 0, MultiOp::Insert("-"));
        assert_eq!(t, "-çx");
    }

    #[test]
    fn one_undo_step_restores_the_text_and_the_primary() {
        let mut buf = Buffer::from_text("one\ntwo\nthree");
        let mut hist = History::new();
        let sels = [at(0), at(4), at(8)];
        let (o, list, p) = edit(&mut buf, &mut hist, &sels, 1, MultiOp::Insert("# "));
        assert_eq!(txt(&buf), "# one\n# two\n# three");
        assert_eq!(o.sel, at(8));
        assert_eq!((list.len(), p), (3, 1));
        let ch = o.change.unwrap();
        assert_eq!(ch.at, 0);
        assert_eq!(hist.steps(), (1, 0));
        // A following edit is its own step; typing does not merge into a multi step.
        edit(&mut buf, &mut hist, &list, p, MultiOp::Insert("x"));
        assert_eq!(hist.steps(), (2, 0));
        undo(&mut buf, &mut hist).unwrap();
        assert_eq!(txt(&buf), "# one\n# two\n# three");
        let u = undo(&mut buf, &mut hist).unwrap();
        assert_eq!(txt(&buf), "one\ntwo\nthree");
        assert_eq!(u.sel, at(4));
        let r = redo(&mut buf, &mut hist).unwrap();
        assert_eq!(txt(&buf), "# one\n# two\n# three");
        assert_eq!(r.sel, at(8));
    }

    // ---- move_all

    fn mv(
        text: &str,
        sels: &[Selection],
        primary: usize,
        motion: Motion,
        extend: bool,
    ) -> (Vec<Selection>, usize) {
        move_all(&Buffer::from_text(text), sels, primary, motion, extend, 4)
    }

    #[test]
    fn move_left_and_right() {
        let t = "abc\ndef\nghi";
        let (v, p) = mv(t, &[at(1), at(5)], 1, Motion::Right, false);
        assert_eq!((carets(&v), p), (vec![2, 6], 1));
        let (v, _) = mv(t, &[at(4), at(8)], 0, Motion::Left, false);
        assert_eq!(carets(&v), vec![3, 7]);
        // Carets that collide merge, and the primary follows.
        let (v, p) = mv(t, &[at(0), at(1)], 1, Motion::Left, false);
        assert_eq!((carets(&v), p), (vec![0], 0));
        let (v, p) = mv(t, &[at(10), at(11)], 0, Motion::Right, false);
        assert_eq!((carets(&v), p), (vec![11], 0));
        // Without extend, Left and Right collapse a selection to its end.
        let (v, _) = mv(t, &[s(1, 3), s(6, 5)], 0, Motion::Right, false);
        assert_eq!(v, vec![at(3), at(6)]);
        let (v, _) = mv(t, &[s(1, 3), s(6, 5)], 0, Motion::Left, false);
        assert_eq!(v, vec![at(1), at(5)]);
        // With extend, the anchors stay and overlapping selections merge.
        let (v, _) = mv(t, &[s(0, 1), s(1, 2)], 0, Motion::Right, true);
        assert_eq!(v, vec![s(0, 3)]);
        let (v, _) = mv(t, &[at(2), at(6)], 1, Motion::Left, true);
        assert_eq!(v, vec![s(2, 1), s(6, 5)]);
    }

    #[test]
    fn move_home_end_and_documents() {
        let t = "  ab\ncd\n  ef";
        let (v, _) = mv(t, &[at(3), at(6), at(10)], 0, Motion::Home, false);
        assert_eq!(carets(&v), vec![2, 5, 8]);
        let (v, _) = mv(t, &[at(2), at(5)], 0, Motion::Home, false);
        assert_eq!(carets(&v), vec![0, 5]);
        let (v, _) = mv(t, &[at(0), at(5)], 0, Motion::End, false);
        assert_eq!(carets(&v), vec![4, 7]);
        // Everything to the start: one caret.
        let (v, p) = mv(t, &[at(3), at(9)], 1, Motion::DocStart, false);
        assert_eq!((v, p), (vec![at(0)], 0));
        let (v, _) = mv(t, &[at(3), at(9)], 0, Motion::End, true);
        assert_eq!(v, vec![s(3, 4), s(9, 12)]);
    }

    #[test]
    fn move_up_and_down_keep_the_goal_column() {
        let t = "abcdef\nab\nabcdef\nx";
        // From column 4 of line 0 and line 2, down one: the short line clamps, line 3 clamps.
        let (v, _) = mv(t, &[at(4), at(14)], 0, Motion::Down(1), false);
        assert_eq!(carets(&v), vec![9, 18]);
        assert_eq!(v[0].goal_col, Some(4));
        // And again: the goal brings the first one back to column 4.
        let (v, _) = mv(t, &v, 0, Motion::Down(1), false);
        assert_eq!(carets(&v), vec![14, 18]);
        // Collision at a line start: carets at the start of line 1 and line 2, Up merges the
        // second into the first's place only when they land together.
        let (v, p) = mv(t, &[at(0), at(7)], 1, Motion::Up(1), false);
        assert_eq!((carets(&v), p), (vec![0], 0));
        // The first line: Up goes to the line start, the last line: Down to its end.
        let (v, _) = mv(t, &[at(3), at(18)], 0, Motion::Up(1), false);
        assert_eq!(carets(&v), vec![0, 11]);
        let (v, _) = mv(t, &[at(2), at(17)], 0, Motion::Down(1), false);
        assert_eq!(carets(&v), vec![9, 18]);
        // Extending: anchors stay.
        let (v, _) = mv(t, &[at(1), at(8)], 0, Motion::Down(1), true);
        assert_eq!(v[0].anchor, 1);
        assert_eq!(v[0].caret, 8);
    }

    #[test]
    fn move_by_words() {
        let t = "foo bar\nbaz qux";
        let (v, _) = mv(t, &[at(0), at(8)], 0, Motion::WordRight, false);
        assert_eq!(carets(&v), vec![3, 11]);
        let (v, _) = mv(t, &[at(7), at(15)], 0, Motion::WordLeft, false);
        assert_eq!(carets(&v), vec![4, 12]);
        let (v, p) = mv(t, &[at(1), at(2)], 1, Motion::WordRight, false);
        assert_eq!((carets(&v), p), (vec![3], 0));
        let (v, _) = mv(t, &[at(0), at(4)], 0, Motion::WordRight, true);
        assert_eq!(v, vec![s(0, 3), s(4, 7)]);
    }

    // ---- column

    fn col(text: &str, al: usize, ac: usize, cl: usize, cc: usize) -> (Vec<Selection>, usize) {
        column(&Buffer::from_text(text), al, ac, cl, cc, 4)
    }

    #[test]
    fn column_down_and_up_with_short_lines() {
        let t = "abcdef\nab\nabcdef";
        let (v, p) = col(t, 0, 1, 2, 4);
        assert_eq!((v, p), (vec![s(1, 4), s(8, 9), s(11, 14)], 2));
        // Backwards: the anchor is on the lower line; the caret on the top one is the primary.
        let (v, p) = col(t, 2, 4, 0, 1);
        assert_eq!((v, p), (vec![s(4, 1), s(9, 8), s(14, 11)], 0));
        // Right to left on the same lines.
        let (v, p) = col(t, 0, 4, 2, 1);
        assert_eq!((v, p), (vec![s(4, 1), s(9, 8), s(14, 11)], 2));
        // A column past every line's end: empty selections at the ends.
        let (v, _) = col(t, 0, 9, 2, 9);
        assert_eq!(v, vec![at(6), at(9), at(16)]);
        // One line: an ordinary selection, and an empty column range is a caret.
        assert_eq!(col(t, 0, 1, 0, 3), (vec![s(1, 3)], 0));
        assert_eq!(col(t, 1, 1, 1, 1), (vec![at(8)], 0));
        // Lines past the end are clamped; an empty buffer works.
        let (v, p) = col(t, 1, 0, 99, 1);
        assert_eq!((v, p), (vec![s(7, 8), s(10, 11)], 1));
        assert_eq!(col("", 0, 0, 5, 3), (vec![at(0)], 0));
        // Empty lines give carets at their start.
        let (v, _) = col("ab\n\nab", 0, 1, 2, 2);
        assert_eq!(v, vec![s(1, 2), at(3), s(5, 6)]);
    }

    #[test]
    fn column_with_tabs_and_wide_characters() {
        // Tab width 4: the tab covers columns 0..4.
        let t = "\tabc\nxxxxabc";
        let (v, _) = col(t, 0, 4, 1, 6);
        assert_eq!(v, vec![s(1, 3), s(9, 11)]);
        // A column inside a tab is the tab's start.
        let (v, _) = col(t, 0, 2, 1, 2);
        assert_eq!(v, vec![at(0), at(7)]);
        let (v, _) = col(t, 0, 2, 0, 5);
        assert_eq!(v, vec![s(0, 2)]);
        // Wide and combining characters count one column per char, as the motions do.
        let t = "漢字ab\nabcde";
        let (v, _) = col(t, 0, 1, 1, 3);
        assert_eq!(v, vec![s(3, 7), s(10, 12)]);
    }

    // ---- display_col

    #[test]
    fn display_col_is_the_selection_one() {
        let buf = Buffer::from_text("a\tb\nçx");
        assert_eq!(display_col(&buf, 3, 4), 5);
        assert_eq!(display_col(&buf, 6, 4), 1);
    }

    // ---- the randomized run against a String model

    mod model {
        use super::*;

        pub fn prev_b(t: &str, p: usize) -> usize {
            unicode_bounds(t)
                .into_iter()
                .rev()
                .find(|&b| b < p)
                .unwrap_or(0)
        }

        pub fn next_b(t: &str, p: usize) -> usize {
            unicode_bounds(t)
                .into_iter()
                .find(|&b| b > p)
                .unwrap_or(t.len())
        }

        pub fn snap(t: &str, p: usize) -> usize {
            unicode_bounds(t)
                .into_iter()
                .rev()
                .find(|&b| b <= p)
                .unwrap()
        }

        /// The new text, where each selection's caret ends (snapped, not merged), and whether
        /// any replacement was not a no-op.
        pub fn edit(text: &str, sels: &[Selection], op: MultiOp<'_>) -> (String, Vec<usize>, bool) {
            let mut out = String::new();
            let mut pos = Vec::new();
            let mut last = 0;
            let mut changed = false;
            for (k, sel) in sels.iter().enumerate() {
                let r = sel.range();
                let (a, b, t) = match op {
                    MultiOp::Insert(t) => (r.start, r.end, t),
                    MultiOp::InsertEach(ts) => (r.start, r.end, ts[k]),
                    MultiOp::Backspace if r.is_empty() => (prev_b(text, r.start), r.start, ""),
                    MultiOp::Delete if r.is_empty() => (r.start, next_b(text, r.start), ""),
                    _ => (r.start, r.end, ""),
                };
                // Normalised selections never make overlapping removals.
                assert!(a >= last, "overlap");
                changed |= a != b || !t.is_empty();
                out.push_str(&text[last..a]);
                out.push_str(t);
                last = b;
                pos.push(out.len());
            }
            out.push_str(&text[last..]);
            // Positions were taken while building, so they are offsets in the final text.
            let pos = pos.into_iter().map(|p| snap(&out, p)).collect();
            (out, pos, changed)
        }
    }

    const CHARS: [&str; 14] = [
        "a",
        "b",
        " ",
        "é",
        "İ",
        "漢",
        "\t",
        "\u{301}",
        "👨‍👩‍👧",
        "\r",
        "\n",
        "e",
        "\r\n",
        "ş",
    ];
    const START: &str = "Merhaba dünya!\r\nİstanbul ışık\n\tindented\rçalışma\r\n  two  \n\
        漢字かな\n👨‍👩‍👧 family e\u{301}\n\n   \nlast line";

    #[test]
    fn randomized_edits_match_a_string_model() {
        for seed in [1u64, 2, 3, 4, 5] {
            let mut rng = Rng::new(seed);
            let mut text = START.to_string();
            for round in 0..400 {
                let b = unicode_bounds(&text);
                let n = 1 + rng.below(7);
                let sels: Vec<Selection> = (0..n)
                    .map(|_| {
                        let i = rng.below(b.len());
                        let j = if rng.chance(50) {
                            i
                        } else {
                            (i + rng.below(5)).saturating_sub(2).min(b.len() - 1)
                        };
                        s(b[j], b[i])
                    })
                    .collect();
                let primary = rng.below(n);
                let (normed, np) = normalize(&Buffer::from_text(&text), &sels, primary);
                // Invariants of the normalised list.
                for w in normed.windows(2) {
                    let (x, y) = (w[0].range(), w[1].range());
                    assert!(x.end <= y.start, "{normed:?}");
                    assert!(
                        !(x.end == y.start && (x.is_empty() || y.is_empty())),
                        "{normed:?}"
                    );
                }
                assert!(
                    normed
                        .iter()
                        .all(|x| b.contains(&x.anchor) && b.contains(&x.caret))
                );
                let strs: Vec<&str> = (0..normed.len() + usize::from(rng.chance(10)))
                    .map(|_| *rng.pick(&CHARS))
                    .collect();
                let word: String = (0..rng.below(4)).map(|_| *rng.pick(&CHARS)).collect();
                let op = match rng.below(5) {
                    0 => MultiOp::Insert(&word),
                    1 => MultiOp::InsertEach(&strs),
                    2 | 3 => MultiOp::Backspace,
                    _ => MultiOp::Delete,
                };
                let mut buf = Buffer::from_text(&text);
                let mut hist = History::new();
                let (out, list, pi) = edit(&mut buf, &mut hist, &sels, primary, op);
                let after = txt(&buf);
                let ctx = format!("seed {seed} round {round} {op:?} {sels:?} on {text:?}");
                if let MultiOp::InsertEach(ts) = op
                    && ts.len() != normed.len()
                {
                    assert_eq!(after, text, "{ctx}");
                    assert!(out.change.is_none(), "{ctx}");
                    assert_eq!((list, pi), (normed, np), "{ctx}");
                    continue;
                }
                let (want, pos, changed) = model::edit(&text, &normed, op);
                assert_eq!(after, want, "{ctx}");
                assert_eq!(out.change.is_some(), changed, "{ctx}");
                if !changed {
                    assert_eq!((list, pi), (normed, np), "{ctx}");
                    continue;
                }
                let mut want_list: Vec<usize> = Vec::new();
                let mut want_pi = 0;
                for (k, &p) in pos.iter().enumerate() {
                    if want_list.last() != Some(&p) {
                        want_list.push(p);
                    }
                    if k == np {
                        want_pi = want_list.len() - 1;
                    }
                }
                assert_eq!(carets(&list), want_list, "{ctx}");
                assert_eq!(pi, want_pi, "{ctx}");
                assert_eq!(out.sel, list[pi], "{ctx}");
                check_change(&text, &after, out.change.as_ref().unwrap());
                assert_eq!(hist.steps(), (1, 0), "{ctx}");
                let u = undo(&mut buf, &mut hist).unwrap();
                assert_eq!(txt(&buf), text, "{ctx}");
                assert_eq!(u.sel, normed[np], "{ctx}");
                let r = redo(&mut buf, &mut hist).unwrap();
                assert_eq!(txt(&buf), after, "{ctx}");
                assert_eq!(r.sel, out.sel, "{ctx}");
                assert_eq!(buf.len_lines(), lines_of(&after).len(), "{ctx}");
                // Carry on with the edited text, bounded in size.
                text = if after.len() > 3000 {
                    START.to_string()
                } else {
                    after
                };
            }
        }
    }

    // ---- speed

    /// Prints the time for 100,000 carets, one per line, inserting a character, and for undoing
    /// and redoing it.
    #[test]
    #[ignore]
    fn timing_100000_carets() {
        use std::time::Instant;
        let n = 100_000;
        let text: String = (0..n).map(|i| format!("line {i}\n")).collect();
        let mut buf = Buffer::from_text(&text);
        let mut hist = History::new();
        let t = Instant::now();
        let (sels, p) = column(&buf, 0, 0, n - 1, 0, 4);
        println!(
            "column selection of {} lines: {:?}",
            sels.len(),
            t.elapsed()
        );
        assert_eq!(sels.len(), n);
        let t = Instant::now();
        let (out, list, _) = edit(&mut buf, &mut hist, &sels, p, MultiOp::Insert("x"));
        println!("edit (insert) at {n} carets: {:?}", t.elapsed());
        assert!(out.change.is_some());
        assert_eq!(list.len(), n);
        assert!(txt(&buf).starts_with("xline 0\nxline 1\n"));
        let t = Instant::now();
        let (list, _) = move_all(&buf, &list, 0, Motion::Right, false, 4);
        println!("move_all Right: {:?}", t.elapsed());
        let t = Instant::now();
        let (_, list, _) = edit(&mut buf, &mut hist, &list, 0, MultiOp::Backspace);
        println!("edit (backspace) at {n} carets: {:?}", t.elapsed());
        assert_eq!(list.len(), n);
        let t = Instant::now();
        undo(&mut buf, &mut hist).unwrap();
        println!("undo: {:?}", t.elapsed());
        let t = Instant::now();
        undo(&mut buf, &mut hist).unwrap();
        println!("undo (insert): {:?}", t.elapsed());
        assert_eq!(txt(&buf), text);
        let t = Instant::now();
        redo(&mut buf, &mut hist).unwrap();
        println!("redo: {:?}", t.elapsed());
    }
}
