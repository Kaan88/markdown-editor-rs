//! A small least-recently-used map for laid-out lines. It holds at most `cap` entries; when an
//! insert goes over, the least recently used are dropped until three quarters of `cap` remain,
//! so a cache at its cap does not sort on every miss.

use std::collections::HashMap;
use std::hash::Hash;

pub struct Lru<K, V> {
    map: HashMap<K, (V, u64)>,
    tick: u64,
    cap: usize,
}

impl<K: Hash + Eq + Clone, V> Lru<K, V> {
    pub fn new(cap: usize) -> Lru<K, V> {
        Lru {
            map: HashMap::new(),
            tick: 0,
            cap: cap.max(1),
        }
    }

    pub fn get(&mut self, k: &K) -> Option<&V> {
        self.tick += 1;
        let tick = self.tick;
        self.map.get_mut(k).map(|(v, t)| {
            *t = tick;
            &*v
        })
    }

    pub fn insert(&mut self, k: K, v: V) {
        self.tick += 1;
        self.map.insert(k, (v, self.tick));
        if self.map.len() > self.cap {
            let keep = self.cap * 3 / 4;
            let mut ticks: Vec<(u64, K)> =
                self.map.iter().map(|(k, (_, t))| (*t, k.clone())).collect();
            ticks.sort_unstable_by_key(|(t, _)| *t);
            let drop = self.map.len() - keep;
            for (_, k) in ticks.into_iter().take(drop) {
                self.map.remove(&k);
            }
        }
    }

    pub fn clear(&mut self) {
        self.map.clear();
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn cap(&self) -> usize {
        self.cap
    }

    /// Changes the bound; a smaller one takes effect at the next insert.
    pub fn set_cap(&mut self, cap: usize) {
        self.cap = cap.max(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eviction_bound() {
        let mut c = Lru::new(100);
        for i in 0..1000 {
            c.insert(i, i * 2);
            assert!(c.len() <= c.cap(), "{} entries", c.len());
        }
        // The most recent survive.
        assert_eq!(c.get(&999), Some(&1998));
        assert_eq!(c.get(&0), None);
    }

    #[test]
    fn recently_used_survive() {
        let mut c = Lru::new(8);
        for i in 0..8 {
            c.insert(i, ());
        }
        // Touch 0 and 1, then overflow: they are kept, the oldest untouched go.
        assert!(c.get(&0).is_some() && c.get(&1).is_some());
        c.insert(100, ());
        assert_eq!(c.len(), 6);
        assert!(c.get(&0).is_some() && c.get(&1).is_some() && c.get(&100).is_some());
        assert!(c.get(&2).is_none());
        c.set_cap(2);
        c.insert(200, ());
        assert!(c.len() <= 2);
        c.clear();
        assert_eq!(c.len(), 0);
    }
}
