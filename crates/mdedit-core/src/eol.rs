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
        let _ = text;
        todo!()
    }

    /// Ends a streamed scan: a CR left pending at the very end is a lone CR.
    pub fn finish(&mut self) {
        todo!()
    }

    /// Counts over a whole text in one call (`scan` then `finish`).
    pub fn of(text: &str) -> EolCounts {
        let _ = text;
        todo!()
    }

    pub fn style(&self) -> LineEnding {
        todo!()
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
        todo!()
    }
}
