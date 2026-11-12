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
