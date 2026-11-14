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

    pub fn clear(&mut self) {
        self.items.iter_mut().for_each(|slot| *slot = None);
        self.head = 0;
        self.len = 0;
    }
}
