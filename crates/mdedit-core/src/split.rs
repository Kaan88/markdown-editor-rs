//! Splitting a document into notes at its headings. The checker lives in the shared crates
//! (`mdshared_app::split`, D-081) and is kept here by its old name, so `mdedit_core::split` reads
//! as before. What is mdedit's is how a [`Buffer`] reads as the plan's document: its
//! [`Lines`] are the rope's (lines end at LF, CRLF or CR, as ropey is built here), and a plan
//! made from a buffer shares the rope's chunks instead of copying the text.

use crate::buffer::Buffer;
use std::ops::Range;
use std::sync::Arc;

pub use mdshared_app::split::*;

impl Lines for Buffer {
    fn len_bytes(&self) -> usize {
        Buffer::len_bytes(self)
    }

    fn len_lines(&self) -> usize {
        Buffer::len_lines(self)
    }

    fn line_start(&self, line: usize) -> usize {
        Buffer::line_start(self, line)
    }

    fn line_range(&self, line: usize) -> Range<usize> {
        Buffer::line_range(self, line)
    }

    fn line_range_with_break(&self, line: usize) -> Range<usize> {
        Buffer::line_range_with_break(self, line)
    }

    fn text(&self, range: Range<usize>, out: &mut String) {
        Buffer::text(self, range, out);
    }

    fn char_at(&self, offset: usize) -> Option<char> {
        Buffer::char_at(self, offset)
    }

    fn chunks(&self) -> Box<dyn Iterator<Item = &str> + '_> {
        Box::new(self.rope().chunks())
    }

    fn line_text(&self, line: usize, out: &mut String) {
        Buffer::line_text(self, line, out);
    }
}

/// The document a plan is cut from, as the shared card takes it: a snapshot of the buffer, O(1)
/// because the rope's chunks are shared.
pub fn document(doc: &Buffer) -> Arc<dyn Lines> {
    Arc::new(doc.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff;
    use crate::testutil::Rng;

    /// Every method of the buffer's [`Lines`] answers as the shared plain-string one does, on
    /// text with every kind of line break, blank lines, a missing final break and wide `char`s.
    #[test]
    fn a_buffer_reads_as_lines_the_way_plain_text_does() {
        let pieces = [
            "a", "\n", "\r\n", "\r", " ", "\t", "# h", "ü", "日本", "\u{feff}", "```", "",
        ];
        let mut rng = Rng::new(5);
        for round in 0..400 {
            let mut s = String::new();
            for _ in 0..rng.below(24) {
                s.push_str(rng.pick(&pieces));
            }
            let buf = Buffer::from_text(&s);
            let text = Text::new(s.clone());
            let ctx = format!("round {round}: {s:?}");
            assert_eq!(Lines::len_bytes(&buf), text.len_bytes(), "{ctx}");
            assert_eq!(Lines::len_lines(&buf), text.len_lines(), "{ctx}");
            assert_eq!(line_count(&buf), line_count(&text), "{ctx}");
            assert_eq!(line_count(&buf), diff::line_count(&buf), "{ctx}");
            for line in 0..=Lines::len_lines(&buf) + 1 {
                assert_eq!(
                    Lines::line_start(&buf, line),
                    text.line_start(line),
                    "{ctx} line {line}"
                );
                assert_eq!(
                    Lines::line_range(&buf, line),
                    text.line_range(line),
                    "{ctx} line {line}"
                );
                assert_eq!(
                    Lines::line_range_with_break(&buf, line),
                    text.line_range_with_break(line),
                    "{ctx} line {line}"
                );
                let (mut a, mut b) = (String::new(), String::new());
                Lines::line_text(&buf, line, &mut a);
                text.line_text(line, &mut b);
                assert_eq!(a, b, "{ctx} line {line}");
            }
            for at in 0..=s.len() + 1 {
                assert_eq!(Lines::char_at(&buf, at), text.char_at(at), "{ctx} at {at}");
                let (mut a, mut b) = (String::new(), String::new());
                Lines::text(&buf, at..s.len() + 1, &mut a);
                text.text(at..s.len() + 1, &mut b);
                assert_eq!(a, b, "{ctx} from {at}");
            }
            let joined: String = Lines::chunks(&buf).collect();
            assert_eq!(joined, s, "{ctx}");
        }
    }

    #[test]
    fn a_plan_outlives_the_buffer_it_was_made_from() {
        // A screen holds the plan for as long as it is open, long after the tab's text may have
        // changed: the plan keeps its own snapshot, so it neither borrows nor follows the buffer.
        let fixture = "# Journal\n\nIntro.\n\n## One\n\nFirst.\n\n## Two\n\nSecond.\n";
        let headings = [
            Heading { line: 0, level: 1 },
            Heading { line: 4, level: 2 },
            Heading { line: 8, level: 2 },
        ];
        let p = {
            let mut doc = Buffer::from_text(fixture);
            let p = plan(document(&doc), "Journal.md", &headings, 2);
            doc.replace(0..0, "# edited afterwards\n\n");
            p
        };
        assert_eq!(p.check(), Ok(()));
        assert_eq!(p.lines(), 11);
        assert_eq!(p.document().len_bytes(), fixture.len());
        assert_eq!(p.note_text(0), "# One\n\nFirst.\n");
        assert_eq!(p.note_text(1), "# Two\n\nSecond.\n");
    }
}
