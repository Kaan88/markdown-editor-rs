//! The bookmarks of files, kept after the file is closed and across runs: for the most recently
//! used files, the bookmarked line numbers. The session file holds them; the open documents hold
//! their own while they are open (`Ready::bookmarks`), and this list is what a closed file left
//! behind and what a file opened again is given.

use mdshared_app::paths::same_path;
use std::path::{Path, PathBuf};

/// Files kept at most.
pub const FILES_MAX: usize = 50;
/// Bookmarked lines kept of one file at most.
pub const LINES_MAX: usize = 100;

/// One file and its bookmarked lines, 0-based, sorted.
pub type Entry = (PathBuf, Vec<usize>);

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Marks {
    /// Most recent first.
    list: Vec<Entry>,
    /// Counts every change of what is kept, so the session's signature can tell without looking.
    pub version: u64,
}

/// Sorted, without repeats, at most `LINES_MAX`.
fn tidy(lines: &[usize]) -> Vec<usize> {
    let mut v = lines.to_vec();
    v.sort_unstable();
    v.dedup();
    v.truncate(LINES_MAX);
    v
}

impl Marks {
    /// What a session file held (already in order, most recent first).
    pub fn from_session(list: &[Entry]) -> Marks {
        let mut m = Marks::default();
        for (path, lines) in list {
            let lines = tidy(lines);
            if lines.is_empty() || m.list.iter().any(|(p, _)| same_path(p, path)) {
                continue;
            }
            m.list.push((path.clone(), lines));
            if m.list.len() == FILES_MAX {
                break;
            }
        }
        m
    }

    /// The bookmarks a closed file left, if any.
    pub fn lines(&self, path: &Path) -> Option<&[usize]> {
        self.list
            .iter()
            .find(|(p, _)| same_path(p, path))
            .map(|(_, l)| l.as_slice())
    }

    /// A file's bookmarks as it leaves (it was closed): kept in front; none removes the file.
    pub fn put(&mut self, path: &Path, lines: &[usize]) {
        let lines = tidy(lines);
        let at = self.list.iter().position(|(p, _)| same_path(p, path));
        let before = self.list.clone();
        if let Some(i) = at {
            self.list.remove(i);
        }
        if !lines.is_empty() {
            self.list.insert(0, (path.to_path_buf(), lines));
            self.list.truncate(FILES_MAX);
        }
        if self.list != before {
            self.version += 1;
        }
    }

    /// What the session file gets: the open files' bookmarks (in the order given) in front of
    /// the closed files', each file once, at most `FILES_MAX`.
    pub fn snapshot<'a>(&self, open: impl Iterator<Item = (&'a Path, &'a [usize])>) -> Vec<Entry> {
        let mut out: Vec<Entry> = Vec::new();
        // A file open without bookmarks has none, whatever it left when it was closed.
        let mut open_paths: Vec<&Path> = Vec::new();
        for (path, lines) in open {
            open_paths.push(path);
            let lines = tidy(lines);
            if !lines.is_empty() && !out.iter().any(|(p, _)| same_path(p, path)) {
                out.push((path.to_path_buf(), lines));
            }
        }
        for (path, lines) in &self.list {
            if !open_paths.iter().any(|p| same_path(p, path)) {
                out.push((path.clone(), lines.clone()));
            }
        }
        out.truncate(FILES_MAX);
        out
    }
}

/// The bookmarks to give a file just read, `len_lines` lines long: those that are still lines
/// of it.
pub fn applied(lines: &[usize], len_lines: usize) -> Vec<usize> {
    lines.iter().copied().filter(|&l| l < len_lines).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    #[test]
    fn a_closed_file_leaves_its_lines_sorted_capped_and_in_front() {
        let mut m = Marks::default();
        assert_eq!(m.version, 0);
        m.put(&p("C:/a.txt"), &[5, 2, 5, 9]);
        assert_eq!(m.lines(&p("C:/a.txt")), Some(&[2usize, 5, 9][..]));
        assert_eq!(m.version, 1);
        // The same again changes nothing, and nothing of a file with none.
        m.put(&p("C:/a.txt"), &[9, 2, 5]);
        m.put(&p("C:/never.txt"), &[]);
        assert_eq!(m.version, 1);
        m.put(&p("C:/b.txt"), &[1]);
        assert_eq!(m.list[0].0, p("C:/b.txt"));
        // Too many lines: the first LINES_MAX.
        let many: Vec<usize> = (0..300).rev().collect();
        m.put(&p("C:/c.txt"), &many);
        let kept = m.lines(&p("C:/c.txt")).unwrap();
        assert_eq!((kept.len(), kept[0], kept[LINES_MAX - 1]), (100, 0, 99));
        // None removes the file's entry.
        m.put(&p("C:/a.txt"), &[]);
        assert!(m.lines(&p("C:/a.txt")).is_none());
    }

    #[test]
    fn only_the_most_recent_files_are_kept() {
        let mut m = Marks::default();
        for i in 0..60 {
            m.put(&p(&format!("C:/f{i}.txt")), &[i]);
        }
        assert_eq!(m.list.len(), FILES_MAX);
        assert!(m.lines(&p("C:/f59.txt")).is_some() && m.lines(&p("C:/f10.txt")).is_some());
        assert!(m.lines(&p("C:/f9.txt")).is_none());
        // A session with more is cut the same way, dropping empty and repeated entries.
        let list: Vec<Entry> = (0..80)
            .map(|i| (p(&format!("C:/g{i}.txt")), vec![i]))
            .chain([(p("C:/g0.txt"), vec![7]), (p("C:/empty.txt"), vec![])])
            .collect();
        let m = Marks::from_session(&list);
        assert_eq!(m.list.len(), FILES_MAX);
        assert_eq!(m.list[0].0, p("C:/g0.txt"));
        assert_eq!(m.lines(&p("C:/g0.txt")), Some(&[0usize][..]));
    }

    #[test]
    fn the_snapshot_puts_open_files_first_and_lets_an_open_file_without_marks_forget() {
        let mut m = Marks::default();
        m.put(&p("C:/closed.txt"), &[4]);
        m.put(&p("C:/open.txt"), &[1]);
        m.put(&p("C:/cleared.txt"), &[8]);
        let open: Vec<(PathBuf, Vec<usize>)> = vec![
            (p("C:/new.txt"), vec![3, 1]),
            (p("C:/cleared.txt"), vec![]),
            (p("C:/open.txt"), vec![2]),
        ];
        let snap = m.snapshot(open.iter().map(|(p, l)| (p.as_path(), l.as_slice())));
        let names: Vec<&str> = snap.iter().map(|(p, _)| p.to_str().unwrap()).collect();
        // The open ones with marks, then what is only in the closed list; the cleared file is
        // open and has none now, so it is gone.
        assert_eq!(names, ["C:/new.txt", "C:/open.txt", "C:/closed.txt"]);
        assert_eq!(snap[0].1, [1, 3]);
        assert_eq!(snap[1].1, [2]);
    }

    #[test]
    fn lines_past_the_end_of_a_changed_file_are_dropped() {
        assert_eq!(applied(&[0, 4, 9], 10), [0, 4, 9]);
        assert_eq!(applied(&[0, 4, 9], 5), [0, 4]);
        assert!(applied(&[3], 0).is_empty());
    }
}
