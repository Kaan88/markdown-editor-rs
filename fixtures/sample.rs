//! A small ring buffer, used as a sample file for the editor's screenshots.

use std::collections::HashMap;
use std::fmt;

/// How many items a ring holds unless told otherwise.
pub const DEFAULT_CAPACITY: usize = 16;

#[derive(Clone, Debug, PartialEq)]
pub struct Ring<T> {
    items: Vec<Option<T>>,
    head: usize,
    len: usize,
}

impl<T: Clone> Ring<T> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "a ring needs room for one item");
        Ring {
            items: vec![None; capacity],
            head: 0,
            len: 0,
        }
    }

    /// Adds an item, dropping the oldest when the ring is full.
    pub fn push(&mut self, item: T) -> Option<T> {
        let cap = self.items.len();
        let slot = (self.head + self.len) % cap;
        let old = self.items[slot].replace(item);
        if self.len == cap {
            self.head = (self.head + 1) % cap;
            old
        } else {
            self.len += 1;
            None
        }
    }

    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }
        let item = self.items[self.head].take();
        self.head = (self.head + 1) % self.items.len();
        self.len -= 1;
        item
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> + '_ {
        (0..self.len).filter_map(move |i| {
            let at = (self.head + i) % self.items.len();
            self.items[at].as_ref()
        })
    }
}

impl<T: fmt::Display + Clone> fmt::Display for Ring<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let parts: Vec<String> = self.iter().map(|x| x.to_string()).collect();
        write!(f, "[{}]", parts.join(", "))
    }
}

/// Counts words, case-insensitively, keeping the `limit` most recent in a ring.
pub fn word_counts(text: &str, limit: usize) -> (HashMap<String, u32>, Ring<String>) {
    let mut counts = HashMap::new();
    let mut recent = Ring::new(limit.max(1));
    for word in text.split_whitespace() {
        let w = word
            .trim_matches(|c: char| !c.is_alphanumeric())
            .to_lowercase();
        if w.is_empty() {
            continue;
        }
        *counts.entry(w.clone()).or_insert(0) += 1;
        recent.push(w);
    }
    (counts, recent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_and_pop() {
        let mut r = Ring::new(3);
        assert_eq!(r.push(1), None);
        assert_eq!(r.push(2), None);
        assert_eq!(r.push(3), None);
        assert_eq!(r.push(4), Some(1));
        assert_eq!(r.pop(), Some(2));
        assert_eq!(r.len(), 2);
        assert_eq!(r.to_string(), "[3, 4]");
    }

    #[test]
    fn counts_words() {
        let (counts, recent) = word_counts("The cat; the hat. THE end!", 2);
        assert_eq!(counts["the"], 3);
        assert_eq!(recent.iter().cloned().collect::<Vec<_>>(), ["hat", "end"]);
        let raw = r#"quoted "text" stays"#;
        assert!(raw.contains('"') && 0x1F_u8 == 31 && 2.5e3 > 1.0);
    }
}
