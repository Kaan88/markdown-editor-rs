//! The queries and replacements typed before, for Up and Down in the find and replace boxes: the
//! last `KEPT`, newest first, no duplicates, kept in the session.

/// Entries kept in each list.
pub const KEPT: usize = 20;
/// An entry longer than this many bytes is not kept.
pub const ENTRY_MAX: usize = 1000;

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Recall {
    /// Newest first.
    items: Vec<String>,
    /// While stepping: the entry shown, an index into `items`.
    at: Option<usize>,
    /// What was in the box before the first Up, for the Down that comes back to it.
    draft: String,
}

impl Recall {
    /// A list read from the session: the first of `items` is the newest. Bad entries (empty,
    /// too long, with line breaks, repeated) and entries beyond `KEPT` are dropped.
    pub fn from_items<I: IntoIterator<Item = String>>(items: I) -> Recall {
        let mut r = Recall::default();
        for s in items {
            if r.items.len() < KEPT && Recall::keepable(&s) && !r.items.contains(&s) {
                r.items.push(s);
            }
        }
        r
    }

    pub fn items(&self) -> &[String] {
        &self.items
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    fn keepable(s: &str) -> bool {
        !s.is_empty() && s.len() <= ENTRY_MAX && !s.chars().any(char::is_control)
    }

    /// Makes `s` the newest entry; one already there moves up instead of repeating. Stepping
    /// starts over.
    pub fn push(&mut self, s: &str) {
        self.at = None;
        if !Recall::keepable(s) {
            return;
        }
        self.items.retain(|i| i != s);
        self.items.insert(0, s.to_string());
        self.items.truncate(KEPT);
    }

    /// Up: the next older entry, `current` being what the box holds now (kept as the draft at
    /// the first step). `None` when there is no older one or the list is empty: the box stays.
    pub fn older(&mut self, current: &str) -> Option<&str> {
        let next = match self.at {
            None => {
                if self.items.is_empty() {
                    return None;
                }
                self.draft = current.to_string();
                // An entry equal to what the box holds is not a step.
                usize::from(self.items[0] == current)
            }
            Some(i) => i + 1,
        };
        let s = self.items.get(next)?;
        self.at = Some(next);
        Some(s)
    }

    /// Down: the next newer entry, then the draft the box had before stepping. `None` when not
    /// stepping.
    pub fn newer(&mut self) -> Option<&str> {
        let i = self.at?;
        if i == 0 {
            self.at = None;
            return Some(&self.draft);
        }
        self.at = Some(i - 1);
        self.items.get(i - 1).map(String::as_str)
    }

    /// The box was edited by hand: stepping starts over from the newest.
    pub fn typed(&mut self) {
        self.at = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_are_newest_first_without_duplicates_and_capped() {
        let mut r = Recall::default();
        for w in ["a", "b", "c", "a"] {
            r.push(w);
        }
        assert_eq!(r.items(), ["a", "c", "b"]);
        for i in 0..40 {
            r.push(&format!("q{i}"));
        }
        assert_eq!(r.items().len(), KEPT);
        assert_eq!(r.items()[0], "q39");
        assert_eq!(r.items()[KEPT - 1], "q20");
        // Not kept: empty, with a line break, too long.
        let before = r.items().to_vec();
        r.push("");
        r.push("a\nb");
        r.push(&"x".repeat(ENTRY_MAX + 1));
        assert_eq!(r.items(), before);
        r.push(&"x".repeat(ENTRY_MAX));
        assert_eq!(r.items()[0].len(), ENTRY_MAX);
    }

    #[test]
    fn up_and_down_step_and_come_back_to_the_draft() {
        let mut r = Recall::from_items(["new".to_string(), "mid".into(), "old".into()]);
        assert_eq!(r.older("draft"), Some("new"));
        assert_eq!(r.older("new"), Some("mid"));
        assert_eq!(r.older("mid"), Some("old"));
        // No older one: the box stays.
        assert_eq!(r.older("old"), None);
        assert_eq!(r.newer(), Some("mid"));
        assert_eq!(r.newer(), Some("new"));
        assert_eq!(r.newer(), Some("draft"));
        assert_eq!(r.newer(), None);
        // A box that already holds the newest entry goes to the one before it.
        assert_eq!(r.older("new"), Some("mid"));
        // Typing starts over; a push does too.
        r.typed();
        assert_eq!(r.older("x"), Some("new"));
        r.push("fresh");
        assert_eq!(r.older("y"), Some("fresh"));
        // An empty list does nothing.
        let mut e = Recall::default();
        assert_eq!(e.older("a"), None);
        assert_eq!(e.newer(), None);
    }

    #[test]
    fn a_list_from_the_session_drops_bad_entries() {
        let long = "z".repeat(ENTRY_MAX + 1);
        let r = Recall::from_items(
            ["a", "", "b\tc", "a", "d", long.as_str()]
                .into_iter()
                .map(String::from)
                .chain((0..50).map(|i| format!("n{i}"))),
        );
        assert_eq!(r.items().len(), KEPT);
        assert_eq!(&r.items()[..2], ["a", "d"]);
    }
}
