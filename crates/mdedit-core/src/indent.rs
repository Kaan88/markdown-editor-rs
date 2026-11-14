//! Which indentation a file uses, found once when it is loaded (and not while typing).
//!
//! [`detect`] looks at the first [`DETECT_LINES`] lines (and at most [`DETECT_BYTES`] bytes of
//! their leading white space) and answers [`Indent::Tab`], [`Indent::Spaces`] or `None` when the
//! file shows no clear style. The shell then lets the typing help and the Tab key use that unit
//! instead of the preference, so the editor never mixes tabs and spaces in a file.

use crate::buffer::Buffer;
use crate::typing::Indent;

/// Lines looked at, from the top of the file.
pub const DETECT_LINES: usize = 2000;
/// Bytes of leading white space looked at over all lines (a line with a megabyte of blanks costs
/// no more than this).
pub const DETECT_BYTES: usize = 64 * 1024;
/// Fewer indented lines than this: no detection.
pub const MIN_INDENTED: usize = 3;
/// The widths of a space unit that are looked for.
const UNITS: [usize; 4] = [2, 3, 4, 8];

/// The indentation of the first lines of `buf`.
///
/// A line counts when it has text and its leading white space is only tabs or only spaces (a
/// mixed run, as in a comment's tab and space, says nothing). Tabs against spaces: the larger
/// number of lines wins, a tie or fewer than [`MIN_INDENTED`] indented lines gives `None`. For
/// spaces the unit is the width that most often is the step *in* from one counted line to the
/// next one (an unindented line counts as width 0) among 2, 3, 4 and 8; on a tie 4 if it is one
/// of them, else the smallest. No step of those widths: `None`.
pub fn detect(buf: &Buffer) -> Option<Indent> {
    let mut tabs = 0usize;
    let mut spaces = 0usize;
    // Counted steps in, per entry of UNITS.
    let mut steps = [0usize; UNITS.len()];
    // The width of the previous counted line; a line that is not counted forgets it.
    let mut prev: Option<usize> = None;
    let mut budget = DETECT_BYTES;
    for line in buf.rope().lines().take(DETECT_LINES) {
        let (mut n_tab, mut n_space, mut text) = (0usize, 0usize, false);
        for c in line.chars() {
            match c {
                '\t' => n_tab += 1,
                ' ' => n_space += 1,
                '\r' | '\n' => break,
                _ => {
                    text = true;
                    break;
                }
            }
            if n_tab + n_space >= budget {
                break;
            }
        }
        budget = budget.saturating_sub(n_tab + n_space);
        // A blank line, or the end of the budget, counts for nothing and the sequence goes on.
        if text {
            match (n_tab, n_space) {
                (0, 0) => prev = Some(0),
                (_, 0) => {
                    tabs += 1;
                    prev = None;
                }
                (0, w) => {
                    spaces += 1;
                    if let Some(p) = prev
                        && w > p
                        && let Some(k) = UNITS.iter().position(|&u| u == w - p)
                    {
                        steps[k] += 1;
                    }
                    prev = Some(w);
                }
                _ => prev = None,
            }
        }
        if budget == 0 {
            break;
        }
    }
    if tabs + spaces < MIN_INDENTED || tabs == spaces {
        return None;
    }
    if tabs > spaces {
        return Some(Indent::Tab);
    }
    let best = *steps.iter().max()?;
    if best == 0 {
        return None;
    }
    let tied = |k: usize| steps[k] == best;
    let k = if tied(2) {
        2
    } else {
        (0..UNITS.len()).find(|&k| tied(k))?
    };
    Some(Indent::Spaces(UNITS[k] as u8))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(text: &str) -> Option<Indent> {
        detect(&Buffer::from_text(text))
    }

    #[test]
    fn two_spaces() {
        let t = "a:\n  b:\n    c: 1\n    d: 2\n  e: 3\nf:\n  g: 4\n";
        assert_eq!(d(t), Some(Indent::Spaces(2)));
    }

    #[test]
    fn four_spaces() {
        let t = "fn a() {\n    if x {\n        y();\n    }\n}\nfn b() {\n    z();\n}\n";
        assert_eq!(d(t), Some(Indent::Spaces(4)));
    }

    #[test]
    fn eight_spaces() {
        let t = "a {\n        b;\n        c {\n                d;\n        }\n}\n";
        assert_eq!(d(t), Some(Indent::Spaces(8)));
    }

    #[test]
    fn three_spaces() {
        let t = "a {\n   b;\n   c {\n      d;\n   }\n}\n";
        assert_eq!(d(t), Some(Indent::Spaces(3)));
    }

    #[test]
    fn tabs() {
        let t = "int main() {\n\tif (x) {\n\t\ty();\n\t}\n}\n";
        assert_eq!(d(t), Some(Indent::Tab));
    }

    #[test]
    fn a_tie_between_units_is_four() {
        // One step of 2 and one of 4.
        let t = "a\n  b\n  c\nd\n    e\n";
        assert_eq!(d(t), Some(Indent::Spaces(4)));
        // Tied 2 against 3, no 4: the smallest.
        let t = "a\n  b\n  c\nd\n   e\n";
        assert_eq!(d(t), Some(Indent::Spaces(2)));
    }

    #[test]
    fn mixed_goes_to_the_majority_and_a_tie_to_none() {
        let t = "a\n\tb\n\tc\n\td\n    e\n    f\n";
        assert_eq!(d(t), Some(Indent::Tab));
        let t = "a\n\tb\n\tc\n    d\n    e\n";
        assert_eq!(d(t), None);
    }

    #[test]
    fn a_tab_and_space_run_counts_for_neither() {
        // The comment lines are "tab, space": not counted.
        let t = "/*\n\t * a\n\t * b\n\t * c\n\t */\n";
        assert_eq!(d(t), None);
    }

    #[test]
    fn too_few_indented_lines() {
        assert_eq!(d(""), None);
        assert_eq!(d("a\nb\nc\n"), None);
        assert_eq!(d("a\n    b\n    c\n"), None);
        assert_eq!(d("a\n\tb\n\tc\n"), None);
    }

    #[test]
    fn blank_lines_and_odd_widths_do_not_count() {
        // Blank lines with spaces are not indented lines; widths of 5 are not a unit.
        let t = "a\n     \n\n     b\n     c\n     d\n";
        assert_eq!(d(t), None);
    }

    #[test]
    fn crlf_and_cr() {
        let t = "a {\r\n    b;\r\n    c {\r\n        d;\r\n    }\r\n}\r\n";
        assert_eq!(d(t), Some(Indent::Spaces(4)));
        let t = "a {\r    b;\r    c {\r        d;\r    }\r}\r";
        assert_eq!(d(t), Some(Indent::Spaces(4)));
    }

    #[test]
    fn only_the_top_of_the_file_is_read() {
        // 2000 unindented lines, then a tab-indented block: nothing seen.
        let mut t = "x\n".repeat(DETECT_LINES);
        t.push_str("a\n\tb\n\tc\n\td\n");
        assert_eq!(d(&t), None);
    }

    #[test]
    fn a_huge_run_of_blanks_is_bounded() {
        let mut t = String::from("a\n");
        t.push_str(&" ".repeat(10 << 20));
        t.push_str("b\n    c\n    d\n    e\n");
        // Only reads DETECT_BYTES of the run; the lines after it are not reached.
        assert_eq!(d(&t), None);
    }
}
