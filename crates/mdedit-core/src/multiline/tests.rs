//! Tests of the line commands with several selections. Texts are written with the selections in
//! them: `^` is a caret, `[a]` a selection with the anchor first, `{a}` one with the caret first.
//! Every run also checks the change against the texts, one undo step, undo and redo.

use super::*;
use crate::edit::{redo, undo};
use crate::testutil::check_change;

/// The text without the marks and the selections they stand for.
fn parse(marked: &str) -> (String, Vec<Selection>) {
    let mut text = String::new();
    let mut sels = Vec::new();
    let mut open: Option<(usize, bool)> = None;
    for c in marked.chars() {
        match c {
            '^' => sels.push(Selection::at(text.len())),
            '[' => open = Some((text.len(), true)),
            '{' => open = Some((text.len(), false)),
            ']' | '}' => {
                let (start, forward) = open.take().expect("closing mark without an opening one");
                let (anchor, caret) = if forward {
                    (start, text.len())
                } else {
                    (text.len(), start)
                };
                sels.push(Selection {
                    anchor,
                    caret,
                    goal_col: None,
                });
            }
            _ => text.push(c),
        }
    }
    (text, sels)
}

/// The text with the marks of `sels` put back (the reverse of [`parse`]).
fn render(text: &str, sels: &[Selection]) -> String {
    // (offset, order at one offset, mark): closers go before carets and openers.
    let mut marks: Vec<(usize, u8, char)> = Vec::new();
    for s in sels {
        if s.is_empty() {
            marks.push((s.caret, 1, '^'));
        } else if s.anchor < s.caret {
            marks.push((s.anchor, 2, '['));
            marks.push((s.caret, 0, ']'));
        } else {
            marks.push((s.caret, 2, '{'));
            marks.push((s.anchor, 0, '}'));
        }
    }
    marks.sort();
    let mut out = String::new();
    let mut at = 0;
    for (pos, _, mark) in marks {
        out.push_str(&text[at..pos]);
        out.push(mark);
        at = pos;
    }
    out.push_str(&text[at..]);
    out
}

/// Runs `op` on `marked` with `primary` as the primary selection; returns the text with the new
/// selections marked. Checks the change, that the command is one undo step and that undo and
/// redo restore the texts, and that nothing is recorded when nothing changed.
fn go_with(marked: &str, primary: usize, op: LineOp<'_>) -> String {
    let (text, sels) = parse(marked);
    let mut buf = Buffer::from_text(&text);
    let mut hist = History::new();
    let (out, list, pi) = run(&mut buf, &mut hist, &sels, primary, op);
    let after = buf.rope().to_string();
    assert_eq!(list[pi], out.sel, "the outcome is the primary");
    for s in &list {
        assert!(s.anchor <= after.len() && s.caret <= after.len());
        assert!(after.is_char_boundary(s.anchor) && after.is_char_boundary(s.caret));
    }
    match &out.change {
        None => {
            assert_eq!(after, text);
            assert!(!hist.can_undo());
            assert_eq!(buf.revision(), 0);
        }
        Some(c) => {
            check_change(&text, &after, c);
            assert_ne!(after, text);
            assert_eq!(hist.steps(), (1, 0), "one undo step");
            assert!(hist.is_modified());
            undo(&mut buf, &mut hist).unwrap();
            assert_eq!(buf.rope().to_string(), text, "undo restores the text");
            assert!(!hist.can_undo() && !hist.is_modified());
            redo(&mut buf, &mut hist).unwrap();
            assert_eq!(buf.rope().to_string(), after, "redo applies it again");
        }
    }
    render(&after, &list)
}

fn go(marked: &str, op: LineOp<'_>) -> String {
    go_with(marked, 0, op)
}

// ---- duplicate

#[test]
fn duplicate_two_carets_copy_their_lines_and_move_onto_the_copies() {
    assert_eq!(
        go("a^\nb\nc^\nd", LineOp::Duplicate("\n")),
        "a\na^\nb\nc\nc^\nd"
    );
}

#[test]
fn duplicate_carets_on_one_line_copy_it_once() {
    assert_eq!(go("a^b^c\nd", LineOp::Duplicate("\n")), "abc\na^b^c\nd");
}

#[test]
fn duplicate_three_carets_on_following_lines_copy_each_below_itself() {
    assert_eq!(
        go("^a\n^b\n^c", LineOp::Duplicate("\n")),
        "a\n^a\nb\n^b\nc\n^c"
    );
}

#[test]
fn duplicate_selections_sharing_a_line_are_one_block() {
    // [a\nb] and [c] meet on line 1: lines 0 and 1 are copied once.
    assert_eq!(
        go("[a\nb] [c]\nd", LineOp::Duplicate("\n")),
        "a\nb c\n[a\nb] [c]\nd"
    );
}

#[test]
fn duplicate_selections_over_separate_blocks() {
    assert_eq!(
        go("[a\nb]\nx\n{c\nd}", LineOp::Duplicate("\n")),
        "a\nb\n[a\nb]\nx\nc\nd\n{c\nd}"
    );
}

#[test]
fn duplicate_a_last_line_without_a_break() {
    assert_eq!(go("a^\nb^", LineOp::Duplicate("\r\n")), "a\na^\nb\r\nb^");
}

// ---- move

#[test]
fn move_down_two_carets_each_swap_with_the_next_line() {
    assert_eq!(go("a^\nb\nc^\nd", LineOp::Move(true)), "b\na^\nd\nc^");
}

#[test]
fn move_up_two_carets() {
    assert_eq!(go("a\nb^\nc\nd^", LineOp::Move(false)), "b^\na\nd^\nc");
}

#[test]
fn move_adjacent_lines_move_as_one_block() {
    assert_eq!(go("a\nb^\nc^\nd", LineOp::Move(true)), "a\nd\nb^\nc^");
    assert_eq!(
        go("a\nb^\nc^\nd\ne", LineOp::Move(false)),
        "b^\nc^\na\nd\ne"
    );
}

#[test]
fn move_three_carets_one_at_the_edge_stays() {
    // The first line cannot go up; the others still do.
    assert_eq!(
        go("a^\nb\nc^\nd\ne^", LineOp::Move(false)),
        "a^\nc^\nb\ne^\nd"
    );
    // The last line cannot go down.
    assert_eq!(go("a^\nb\nc\nd^", LineOp::Move(true)), "b\na^\nc\nd^");
}

#[test]
fn move_selections_keep_their_columns() {
    assert_eq!(
        go("a[bc]\nxy\nd{ef}\nzz", LineOp::Move(true)),
        "xy\na[bc]\nzz\nd{ef}"
    );
}

#[test]
fn move_nothing_when_every_block_is_at_the_edge() {
    assert_eq!(go("^a\n^b", LineOp::Move(false)), "^a\n^b");
    assert_eq!(go("a^\nb^", LineOp::Move(true)), "a^\nb^");
}

#[test]
fn move_last_line_without_a_break() {
    assert_eq!(go("a\nb\nc^", LineOp::Move(false)), "a\nc^\nb");
    assert_eq!(go("a^\nb\nc", LineOp::Move(true)), "b\na^\nc");
}

// ---- delete

#[test]
fn delete_two_carets_remove_their_lines() {
    assert_eq!(go("a^\nb\nc^\nd", LineOp::Delete), "b^\nd^");
}

#[test]
fn delete_keeps_the_column() {
    assert_eq!(go("ab^c\nxyz\nde^f\nuvw", LineOp::Delete), "xy^z\nuv^w");
}

#[test]
fn delete_adjacent_and_overlapping_selections_are_one_block() {
    assert_eq!(go("a^\nb^\nc\nd", LineOp::Delete), "c^\nd");
    assert_eq!(go("[a\nb]\n[c\nd]\ne", LineOp::Delete), "e^");
    assert_eq!(go("[a\nb] [c]\nd\ne", LineOp::Delete), "d^\ne");
}

#[test]
fn delete_the_last_lines_takes_the_break_before() {
    // Both carets end up in the same place, so they are one.
    assert_eq!(go("a\nb^\nc\nd^", LineOp::Delete), "a\nc^");
    assert_eq!(go("a^\nb\nc^", LineOp::Delete), "b^");
    assert_eq!(go("a\nb^\nc\nxyz\nd^", LineOp::Delete), "a\nc^\nx^yz");
}

#[test]
fn delete_every_line() {
    assert_eq!(go("a^\nb^", LineOp::Delete), "^");
}

// ---- comment

#[test]
fn comment_two_carets_comment_both_lines() {
    assert_eq!(go("a^\nb\nc^", LineOp::Comment("//")), "// a^\nb\n// c^");
}

#[test]
fn comment_all_commented_lines_are_uncommented_together() {
    assert_eq!(go("// a^\nb\n// c^", LineOp::Comment("//")), "a^\nb\nc^");
}

#[test]
fn comment_a_mix_comments_every_line() {
    // One commented, one not: all get the comment (the decision is over all lines).
    assert_eq!(
        go("// a^\nb\nc^", LineOp::Comment("//")),
        "// // a^\nb\n// c^"
    );
}

#[test]
fn comment_uses_the_smallest_indent_of_all_selections() {
    assert_eq!(
        go("    a^\nx\n  b^\n", LineOp::Comment("#")),
        "  #   a^\nx\n  # b^\n"
    );
}

#[test]
fn comment_overlapping_and_adjacent_selections() {
    assert_eq!(
        go("[a\nb]\n[c\nd]", LineOp::Comment("//")),
        "[// a\n// b]\n[// c\n// d]"
    );
    assert_eq!(
        go("[a\nb] [c]\nd", LineOp::Comment("//")),
        "[// a\n// b] [c]\nd"
    );
}

#[test]
fn comment_three_carets_on_blank_lines_comment_each() {
    assert_eq!(
        go("^\nx\n^\n\n^", LineOp::Comment("//")),
        "// ^\nx\n// ^\n\n// ^"
    );
}

#[test]
fn comment_blank_lines_inside_are_left_alone() {
    assert_eq!(
        go("[a\n\nb]\nc^", LineOp::Comment("#")),
        "[# a\n\n# b]\n# c^"
    );
}

// ---- case

#[test]
fn case_changes_each_selection() {
    assert_eq!(
        go("[ab] cd {ef} ^gh", LineOp::Case(true)),
        "[AB] cd {EF} ^gh"
    );
    assert_eq!(go("[AB] x [CD]", LineOp::Case(false)), "[ab] x [cd]");
}

#[test]
fn case_carets_alone_change_nothing() {
    assert_eq!(go("a^b c^d", LineOp::Case(true)), "a^b c^d");
}

#[test]
fn case_that_changes_the_length_keeps_later_selections_on_their_text() {
    assert_eq!(
        go("[\u{df}] x [ab] y [\u{df}]", LineOp::Case(true)),
        "[SS] x [AB] y [SS]"
    );
}

#[test]
fn case_adjacent_selections() {
    assert_eq!(go("[ab][cd]", LineOp::Case(true)), "[AB][CD]");
}

// ---- trim, tabs, line endings

#[test]
fn trim_moves_every_caret_with_its_text() {
    assert_eq!(
        go("a  ^\nb\t \nc ^d  \ne^  ", LineOp::Trim),
        "a^\nb\nc ^d\ne^"
    );
}

#[test]
fn trim_a_caret_after_the_run_moves_back() {
    assert_eq!(go("a   \nb^", LineOp::Trim), "a\nb^");
    assert_eq!(go("a  ^\n x  ^\n", LineOp::Trim), "a^\n x^\n");
}

#[test]
fn tabs_to_spaces_moves_every_caret() {
    assert_eq!(
        go("^\ta\t^b\n\t[c]", LineOp::TabsToSpaces(4)),
        "^    a   ^b\n    [c]"
    );
}

#[test]
fn line_endings_move_every_caret() {
    assert_eq!(
        go("a^\nb\nc[d]\n", LineOp::LineEndings("\r\n")),
        "a^\r\nb\r\nc[d]\r\n"
    );
    assert_eq!(go("a\r\nb^\r\nc^", LineOp::LineEndings("\n")), "a\nb^\nc^");
}

// ---- sort, reverse, unique

#[test]
fn sort_each_selection_alone() {
    assert_eq!(
        go(
            "[c\nb\na]\nz\n[f\ne\nd]",
            LineOp::Sort(SortOrder::Ascending)
        ),
        "[a\nb\nc]\nz\n[d\ne\nf]"
    );
}

#[test]
fn sort_never_moves_a_line_into_another_block() {
    assert_eq!(
        go("[b\na]\nA\n[d\nc]", LineOp::Sort(SortOrder::Ascending)),
        "[a\nb]\nA\n[c\nd]"
    );
}

#[test]
fn sort_adjacent_selections_are_one_block() {
    assert_eq!(
        go("[c\nb]\n[a\nd]", LineOp::Sort(SortOrder::Ascending)),
        "[a\nb\nc\nd]"
    );
}

#[test]
fn sort_carets_on_following_lines_sort_those_lines() {
    assert_eq!(
        go("z\n[c]\n[b]\n[a]\ny", LineOp::Sort(SortOrder::Ascending)),
        "z\n[a\nb\nc]\ny"
    );
}

#[test]
fn sort_one_line_selections_apart_do_nothing() {
    assert_eq!(
        go("b[x]\na\nc[y]", LineOp::Sort(SortOrder::Ascending)),
        "b[x]\na\nc[y]"
    );
}

#[test]
fn sort_carets_only_sort_the_whole_document() {
    assert_eq!(
        go("c^\nb\na^", LineOp::Sort(SortOrder::Ascending)),
        "[a\nb\nc]"
    );
}

#[test]
fn sort_descending_and_ignoring_case() {
    assert_eq!(
        go("[a\nb]\nx\n[B\na]", LineOp::Sort(SortOrder::Descending)),
        "[b\na]\nx\n[a\nB]"
    );
    assert_eq!(
        go("[b\nA]\nx\n[B\na]", LineOp::Sort(SortOrder::IgnoreCase)),
        "[A\nb]\nx\n[a\nB]"
    );
}

#[test]
fn reverse_each_block() {
    assert_eq!(
        go("[a\nb\nc]\nx\n[1\n2]", LineOp::Reverse),
        "[c\nb\na]\nx\n[2\n1]"
    );
}

#[test]
fn unique_looks_in_one_block_only() {
    assert_eq!(
        go("[a\na\nb]\na\n[b\nb\nc]", LineOp::Unique),
        "[a\nb]\na\n[b\nc]"
    );
}

#[test]
fn unique_three_blocks_the_last_without_a_break() {
    assert_eq!(
        go("[a\na]\nx\n[b\nb]\ny\n[c\nc]", LineOp::Unique),
        "[a]\nx\n[b]\ny\n[c]"
    );
}

// ---- join

#[test]
fn join_two_carets_join_their_lines_with_the_next() {
    assert_eq!(go("a^\nb\nc^\n  d\ne", LineOp::Join), "a^ b\nc^ d\ne");
}

#[test]
fn join_carets_on_following_lines_join_all_of_them() {
    assert_eq!(go("a^\nb^\nc^\nd", LineOp::Join), "[a b c d]");
}

#[test]
fn join_selections_over_blocks() {
    assert_eq!(
        go("[a\nb\nc]\nx\n[d\ne]", LineOp::Join),
        "[a b c]\nx\n[d e]"
    );
}

#[test]
fn join_selections_that_follow_one_another_are_joined_each() {
    assert_eq!(go("[a\nb]\n[c\nd]\ne", LineOp::Join), "[a b]\n[c d]\ne");
}

#[test]
fn join_the_last_line_has_no_next_line() {
    assert_eq!(go("a^\nb\nc^", LineOp::Join), "a^ b\nc^");
}

// ---- headings

#[test]
fn heading_each_block_steps_by_its_own_first_line() {
    assert_eq!(go("a^\nx\n# b^", LineOp::Heading), "# a^\nx\n## b^");
}

#[test]
fn heading_two_carets_on_one_line_step_once() {
    assert_eq!(go("a^b^c", LineOp::Heading), "# a^b^c");
}

#[test]
fn heading_carets_at_the_line_start_go_after_the_prefix() {
    assert_eq!(go("^a\n^b", LineOp::Heading), "# ^a\n# ^b");
}

#[test]
fn heading_selections_over_lines() {
    assert_eq!(
        go("[a\nb]\nx\n[## c]", LineOp::Heading),
        "[# a\n# b]\nx\n[### c]"
    );
}

#[test]
fn heading_cycles_back_to_none() {
    assert_eq!(go("###### a^\n###### b^", LineOp::Heading), "a^\nb^");
}

#[test]
fn heading_blank_lines_do_nothing() {
    assert_eq!(go("^\n\n^", LineOp::Heading), "^\n\n^");
}

// ---- tables

#[test]
fn table_two_carets_in_one_table_align_it_once() {
    assert_eq!(
        go("|a^|b|\n|-|-|\n|ccc|d^|\n", LineOp::Table),
        "| a^   | b   |\n| --- | --- |\n| ccc | d^   |\n"
    );
}

#[test]
fn table_two_tables_and_carets_outside() {
    assert_eq!(
        go(
            "^x\n\n|a^|b|\n|-|-|\n|ccc|d|\n\n|e|f^|\n|-|-|\n|g|h|\ny^",
            LineOp::Table
        ),
        "^x\n\n| a^   | b   |\n| --- | --- |\n| ccc | d   |\n\n| e   | f^   |\n| --- | --- |\n| g   | h   |\ny^"
    );
}

#[test]
fn table_outside_a_table_does_nothing() {
    assert_eq!(go("a^\nb^", LineOp::Table), "a^\nb^");
}

// ---- the whole

#[test]
fn one_selection_is_the_single_command() {
    use crate::lines as l;
    use crate::markdown as m;
    let texts = [
        "",
        "a",
        "b\na\n",
        "  x\n\ty \n\n z\nz\r\nx\n",
        "c\nb\nb\na",
        "# h\n| a | b |\n|-|-|\n| c | d |\n\nend  ",
    ];
    type Single = fn(&mut Buffer, &mut History, Selection) -> Outcome;
    let ops: Vec<(LineOp<'static>, Single)> = vec![
        (LineOp::Duplicate("\n"), |b, h, s| {
            l::duplicate_lines(b, h, s, "\n")
        }),
        (LineOp::Move(true), |b, h, s| {
            l::move_lines(b, h, s, true, "\n")
        }),
        (LineOp::Move(false), |b, h, s| {
            l::move_lines(b, h, s, false, "\n")
        }),
        (LineOp::Delete, l::delete_lines),
        (LineOp::Comment("//"), |b, h, s| {
            l::toggle_comment(b, h, s, "//")
        }),
        (LineOp::Case(true), |b, h, s| l::change_case(b, h, s, true)),
        (LineOp::Trim, l::trim_trailing_whitespace),
        (LineOp::TabsToSpaces(4), |b, h, s| {
            l::tabs_to_spaces(b, h, s, 4)
        }),
        (LineOp::LineEndings("\r\n"), |b, h, s| {
            l::convert_line_endings(b, h, s, "\r\n")
        }),
        (LineOp::Sort(SortOrder::Ascending), |b, h, s| {
            l::sort_lines(b, h, s, SortOrder::Ascending)
        }),
        (LineOp::Reverse, l::reverse_lines),
        (LineOp::Unique, l::unique_lines),
        (LineOp::Join, l::join_lines),
        (LineOp::Heading, m::cycle_heading),
        (LineOp::Table, m::format_table),
    ];
    for text in texts {
        let probe = Buffer::from_text(text);
        let cuts: Vec<usize> = (0..=text.len())
            .filter(|&i| text.is_char_boundary(i) && probe.snap_to_grapheme(i) == i)
            .collect();
        for (op, single) in &ops {
            for &a in &cuts {
                for &c in &cuts {
                    let s = Selection {
                        anchor: a,
                        caret: c,
                        goal_col: None,
                    };
                    let mut b1 = Buffer::from_text(text);
                    let mut h1 = History::new();
                    let one = single(&mut b1, &mut h1, s);
                    let mut b2 = Buffer::from_text(text);
                    let mut h2 = History::new();
                    let (many, list, _) = run(&mut b2, &mut h2, &[s], 0, *op);
                    let ctx = format!("{op:?} on {text:?} with {a}..{c}");
                    assert_eq!(b1.rope().to_string(), b2.rope().to_string(), "{ctx}");
                    assert_eq!(one.change.is_some(), many.change.is_some(), "{ctx}");
                    assert_eq!(list.len(), 1, "{ctx}");
                    assert_eq!(
                        (one.sel.anchor, one.sel.caret),
                        (many.sel.anchor, many.sel.caret),
                        "{ctx}"
                    );
                }
            }
        }
    }
}

#[test]
fn the_primary_stays_the_primary() {
    let (text, sels) = parse("a^\nb\nc^");
    for primary in 0..2 {
        let (out, list, pi) = run(
            &mut Buffer::from_text(&text),
            &mut History::new(),
            &sels,
            primary,
            LineOp::Duplicate("\n"),
        );
        assert_eq!(pi, primary);
        assert_eq!(list[pi], out.sel);
    }
    // Selections that merge keep a primary.
    let mut buf = Buffer::from_text(&text);
    let (out, list, pi) = run(
        &mut buf,
        &mut History::new(),
        &[Selection::at(1), Selection::at(5)],
        1,
        LineOp::Delete,
    );
    assert_eq!(list.len(), 1);
    assert_eq!((pi, list[0]), (0, out.sel));
}

#[test]
fn selections_are_taken_in_any_order() {
    let (text, mut sels) = parse("a^\nb\nc^");
    sels.reverse();
    let mut buf = Buffer::from_text(&text);
    let (_, list, _) = run(&mut buf, &mut History::new(), &sels, 0, LineOp::Delete);
    assert_eq!(buf.rope().to_string(), "b");
    assert_eq!(list.len(), 1);
}

#[test]
fn nothing_to_do_changes_no_selection_and_records_nothing() {
    assert_eq!(go("a^\nb^", LineOp::Trim), "a^\nb^");
    assert_eq!(go("x^ y^", LineOp::Join), "x^ y^");
}

#[test]
fn a_second_command_is_a_second_undo_step() {
    let (text, sels) = parse("c^\nb\na^");
    let mut buf = Buffer::from_text(&text);
    let mut hist = History::new();
    let (out, ..) = run(&mut buf, &mut hist, &sels, 0, LineOp::Duplicate("\n"));
    assert!(out.change.is_some());
    let (_, list, _) = run(&mut buf, &mut hist, &[Selection::at(0)], 0, LineOp::Delete);
    assert_eq!(list.len(), 1);
    assert_eq!(hist.steps(), (2, 0));
    undo(&mut buf, &mut hist).unwrap();
    undo(&mut buf, &mut hist).unwrap();
    assert_eq!(buf.rope().to_string(), text);
    assert!(!hist.can_undo());
}

#[test]
fn too_large_looks_at_each_block() {
    let (text, sels) = parse("[a\nb]\nx\n[c\nd]");
    let buf = Buffer::from_text(&text);
    assert!(!too_large(
        &buf,
        &sels,
        0,
        LineOp::Sort(SortOrder::Ascending)
    ));
    assert!(!too_large(&buf, &sels, 0, LineOp::Join));
    assert!(!too_large(&buf, &sels, 0, LineOp::Delete));
    let big = format!("{}\n{}\n", "x".repeat(crate::limits::LINE_BLOCK_BYTES), "y");
    let buf = Buffer::from_text(&big);
    let sels = [
        Selection {
            anchor: 0,
            caret: big.len() - 1,
            goal_col: None,
        },
        Selection::at(big.len()),
    ];
    assert!(too_large(
        &buf,
        &sels,
        0,
        LineOp::Sort(SortOrder::Ascending)
    ));
    assert!(too_large(&buf, &sels, 0, LineOp::Join));
}
