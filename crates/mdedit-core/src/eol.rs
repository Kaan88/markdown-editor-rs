//! Line ending detection. The buffer keeps every line break exactly as the file had it; this only
//! counts them so the status area can name the style (and, from phase 3, so saving knows what to
//! write for new lines).

/// How many line breaks of each kind the text holds. A CR directly followed by LF is one CRLF,
/// also when the two arrive in different chunks of a streamed scan.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EolCounts {
    pub crlf: u64,
    pub lf: u64,
    pub cr: u64,
    /// The last byte scanned was a CR whose pair is not known yet. [`EolCounts::finish`] counts
    /// it as a lone CR.
    pending_cr: bool,
}

impl EolCounts {
    /// Counts the breaks in the next piece of UTF-8 text. Pieces may split a CRLF pair.
    pub fn scan(&mut self, text: &[u8]) {
        let mut i = 0;
        if self.pending_cr && !text.is_empty() {
            self.pending_cr = false;
            if text[0] == b'\n' {
                self.crlf += 1;
                i = 1;
            } else {
                self.cr += 1;
            }
        }
        while i < text.len() {
            let Some(j) = memchr::memchr2(b'\n', b'\r', &text[i..]) else {
                break;
            };
            i += j;
            if text[i] == b'\n' {
                self.lf += 1;
                i += 1;
            } else if i + 1 < text.len() {
                if text[i + 1] == b'\n' {
                    self.crlf += 1;
                    i += 2;
                } else {
                    self.cr += 1;
                    i += 1;
                }
            } else {
                self.pending_cr = true;
                i += 1;
            }
        }
    }

    /// Ends a streamed scan: a CR left pending at the very end is a lone CR.
    pub fn finish(&mut self) {
        if self.pending_cr {
            self.pending_cr = false;
            self.cr += 1;
        }
    }

    /// Counts over a whole text in one call (`scan` then `finish`).
    pub fn of(text: &str) -> EolCounts {
        let mut counts = EolCounts::default();
        counts.scan(text.as_bytes());
        counts.finish();
        counts
    }

    /// Counts over text given in pieces (a rope's chunks); a CRLF split between two pieces is
    /// one CRLF.
    pub fn of_chunks<'a>(chunks: impl IntoIterator<Item = &'a str>) -> EolCounts {
        let mut counts = EolCounts::default();
        for piece in chunks {
            counts.scan(piece.as_bytes());
        }
        counts.finish();
        counts
    }

    /// Updates counts of a text after an edit, given the counts of a window of the old text and
    /// of the same window of the new text (`Change::eol_removed` and `eol_added`). The result is
    /// exactly the counts of the new text when `self` was those of the old. Applying the sum of
    /// several edits' windows in one call is the same as applying them one by one.
    pub fn apply(&mut self, removed: &EolCounts, added: &EolCounts) {
        self.crlf = (self.crlf + added.crlf) - removed.crlf;
        self.lf = (self.lf + added.lf) - removed.lf;
        self.cr = (self.cr + added.cr) - removed.cr;
    }

    /// The window counts of two edits applied one after the other, as one pair for
    /// [`EolCounts::apply`]: the sums of their removed and of their added counts.
    pub(crate) fn add(&mut self, other: &EolCounts) {
        self.crlf += other.crlf;
        self.lf += other.lf;
        self.cr += other.cr;
    }

    /// The line break a new line should get in a text with these counts: the most frequent of
    /// CRLF, LF and CR (ties in that order), and the platform's (CRLF on Windows, else LF) when
    /// the text has none.
    pub fn dominant(&self) -> &'static str {
        if self.crlf == 0 && self.lf == 0 && self.cr == 0 {
            return if cfg!(windows) { "\r\n" } else { "\n" };
        }
        if self.crlf >= self.lf && self.crlf >= self.cr {
            "\r\n"
        } else if self.lf >= self.cr {
            "\n"
        } else {
            "\r"
        }
    }

    pub fn style(&self) -> LineEnding {
        match (self.crlf > 0, self.lf > 0, self.cr > 0) {
            (false, false, false) => LineEnding::None,
            (true, false, false) => LineEnding::Crlf,
            (false, true, false) => LineEnding::Lf,
            (false, false, true) => LineEnding::Cr,
            _ => LineEnding::Mixed,
        }
    }
}

/// `text` with every line break (CRLF, lone CR, lone LF) replaced by `eol`; borrowed when every
/// break already is `eol`. For pasted text, so a buffer of one style does not gain another.
pub fn normalize_eols<'a>(text: &'a str, eol: &str) -> std::borrow::Cow<'a, str> {
    let bytes = text.as_bytes();
    let mut out: Option<String> = None;
    let (mut i, mut copied) = (0, 0);
    while let Some(j) = memchr::memchr2(b'\n', b'\r', &bytes[i..]) {
        let at = i + j;
        let n = if bytes[at] == b'\r' && bytes.get(at + 1) == Some(&b'\n') {
            2
        } else {
            1
        };
        if &text[at..at + n] != eol {
            let o = out.get_or_insert_with(|| String::with_capacity(text.len()));
            o.push_str(&text[copied..at]);
            o.push_str(eol);
            copied = at + n;
        }
        i = at + n;
    }
    match out {
        Some(mut o) => {
            o.push_str(&text[copied..]);
            std::borrow::Cow::Owned(o)
        }
        None => std::borrow::Cow::Borrowed(text),
    }
}

/// The style the status area names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LineEnding {
    Crlf,
    Lf,
    Cr,
    /// More than one kind occurs.
    Mixed,
    /// The text has no line break at all (empty or one line).
    None,
}

impl LineEnding {
    /// "Windows (CRLF)", "Unix (LF)", "Macintosh (CR)", "Mixed". `None` shows the platform's
    /// default, "Windows (CRLF)" on Windows and "Unix (LF)" elsewhere, which is what a new line
    /// will get once editing exists.
    pub fn label(self) -> &'static str {
        match self {
            LineEnding::Crlf => "Windows (CRLF)",
            LineEnding::Lf => "Unix (LF)",
            LineEnding::Cr => "Macintosh (CR)",
            LineEnding::Mixed => "Mixed",
            LineEnding::None => {
                if cfg!(windows) {
                    "Windows (CRLF)"
                } else {
                    "Unix (LF)"
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counts(t: &str) -> (u64, u64, u64) {
        let c = EolCounts::of(t);
        (c.crlf, c.lf, c.cr)
    }

    #[test]
    fn kinds() {
        assert_eq!(counts("a\r\nb\r\nc"), (2, 0, 0));
        assert_eq!(counts("a\nb\nc\n"), (0, 3, 0));
        assert_eq!(counts("a\rb\rc"), (0, 0, 2));
        assert_eq!(counts("a\r\nb\nc\rd"), (1, 1, 1));
        assert_eq!(counts("abc"), (0, 0, 0));
        assert_eq!(counts(""), (0, 0, 0));
        assert_eq!(counts("\n\r"), (0, 1, 1));
        assert_eq!(counts("\r\r\n\n\r"), (1, 1, 2));
    }

    #[test]
    fn styles_and_labels() {
        assert_eq!(EolCounts::of("a\r\nb").style(), LineEnding::Crlf);
        assert_eq!(EolCounts::of("a\nb").style(), LineEnding::Lf);
        assert_eq!(EolCounts::of("a\rb").style(), LineEnding::Cr);
        assert_eq!(EolCounts::of("a\r\nb\nc").style(), LineEnding::Mixed);
        assert_eq!(EolCounts::of("abc").style(), LineEnding::None);
        assert_eq!(LineEnding::Crlf.label(), "Windows (CRLF)");
        assert_eq!(LineEnding::Lf.label(), "Unix (LF)");
        assert_eq!(LineEnding::Cr.label(), "Macintosh (CR)");
        assert_eq!(LineEnding::Mixed.label(), "Mixed");
        let none = LineEnding::None.label();
        assert_eq!(
            none,
            if cfg!(windows) {
                "Windows (CRLF)"
            } else {
                "Unix (LF)"
            }
        );
    }

    #[test]
    fn cr_at_end_of_one_piece_and_lf_at_start_of_the_next() {
        let mut c = EolCounts::default();
        c.scan(b"a\r");
        assert_eq!((c.crlf, c.cr), (0, 0));
        c.scan(b"\nb");
        c.finish();
        assert_eq!((c.crlf, c.lf, c.cr), (1, 0, 0));

        // A pending CR followed by something else is a lone CR.
        let mut c = EolCounts::default();
        c.scan(b"a\r");
        c.scan(b"b\n");
        c.finish();
        assert_eq!((c.crlf, c.lf, c.cr), (0, 1, 1));

        // An empty piece keeps the CR pending.
        let mut c = EolCounts::default();
        c.scan(b"\r");
        c.scan(b"");
        c.scan(b"\n");
        c.finish();
        assert_eq!((c.crlf, c.lf, c.cr), (1, 0, 0));

        // A CR at the very end is a lone CR once finished.
        let mut c = EolCounts::default();
        c.scan(b"a\r");
        c.finish();
        assert_eq!((c.crlf, c.lf, c.cr), (0, 0, 1));
        c.finish();
        assert_eq!(c.cr, 1);
    }

    #[test]
    fn dominant_break() {
        let platform = if cfg!(windows) { "\r\n" } else { "\n" };
        assert_eq!(EolCounts::of("abc").dominant(), platform);
        assert_eq!(EolCounts::of("a\r\nb\nc\nd").dominant(), "\n");
        assert_eq!(EolCounts::of("a\r\nb\nc\r\nd").dominant(), "\r\n");
        assert_eq!(EolCounts::of("a\rb\rc\nd").dominant(), "\r");
        // Ties go CRLF, LF, CR.
        assert_eq!(EolCounts::of("a\r\nb\nc\rd").dominant(), "\r\n");
        assert_eq!(EolCounts::of("a\nb\rc").dominant(), "\n");
    }

    #[test]
    fn normalizing() {
        use std::borrow::Cow;
        for eol in ["\n", "\r\n", "\r"] {
            let all = [
                "a\r\nb\nc\rd\r\n\r\ne",
                "",
                "x",
                "\n",
                "\r",
                "\r\n",
                "é\r\r\n\n",
            ];
            for t in all {
                let n = normalize_eols(t, eol);
                let want = t
                    .replace("\r\n", "\n")
                    .replace('\r', "\n")
                    .replace('\n', eol);
                assert_eq!(n, want, "{t:?} {eol:?}");
                let c = EolCounts::of(&n);
                assert_eq!(
                    c.crlf + c.lf + c.cr,
                    EolCounts::of(t).crlf + EolCounts::of(t).lf + EolCounts::of(t).cr
                );
            }
        }
        assert!(matches!(normalize_eols("a\nb\n", "\n"), Cow::Borrowed(_)));
        assert!(matches!(normalize_eols("a\r\nb", "\r\n"), Cow::Borrowed(_)));
        assert!(matches!(normalize_eols("abc", "\r\n"), Cow::Borrowed(_)));
        assert!(matches!(normalize_eols("a\nb", "\r\n"), Cow::Owned(_)));
    }

    #[test]
    fn of_chunks_pairs_across_pieces() {
        let t = "a\r\nb\r\r\n\n\r";
        for cut in 0..=t.len() {
            assert_eq!(
                EolCounts::of_chunks([&t[..cut], &t[cut..]]),
                EolCounts::of(t),
                "cut {cut}"
            );
        }
    }

    #[test]
    fn byte_by_byte_equals_whole() {
        let text = "a\r\nb\nc\r\rd\r\n\r\n\n\re\r";
        let whole = EolCounts::of(text);
        let mut c = EolCounts::default();
        for b in text.as_bytes() {
            c.scan(std::slice::from_ref(b));
        }
        c.finish();
        assert_eq!(c, whole);
    }
}
