//! Where the caret and the scroll were in files, kept after the file is closed and across runs:
//! for the most recently used files, the caret's line and column and the first line in view. The
//! session file holds them (`place=` lines); the open documents hold their own while they are
//! open, and this list is what a closed file left behind and what a file read again is given.
//! A file's language, when one was chosen for it (the status bar's language field), is kept
//! with its place. The same shape as [`crate::marks`].

use mdshared_app::paths::same_path;
use std::path::{Path, PathBuf};

/// Files kept at most.
pub const FILES_MAX: usize = 100;

/// The caret's line and column (characters, from 0) and the first line in view (from 0), and
/// the language chosen for the file over its extension's, by name ([`crate::languages`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Place {
    pub line: usize,
    pub col: usize,
    pub top: usize,
    pub lang: Option<&'static str>,
}

impl Place {
    /// A place worth keeping: anywhere but the very start with no language chosen, which is how
    /// a file opens anyway.
    pub fn is_start(&self) -> bool {
        *self == Place::default()
    }

    /// `line,col,top` or `line,col,top,Language`, the session line's value before the path.
    pub fn to_setting(self) -> String {
        match self.lang {
            Some(lang) => format!("{},{},{},{lang}", self.line, self.col, self.top),
            None => format!("{},{},{}", self.line, self.col, self.top),
        }
    }

    /// A language this build does not offer is left out; the place stays.
    pub fn parse(value: &str) -> Option<Place> {
        let mut parts = value.splitn(4, ',');
        let mut num = || parts.next()?.trim().parse::<usize>().ok();
        let (line, col, top) = (num()?, num()?, num()?);
        let lang = parts
            .next()
            .and_then(|name| crate::languages::static_name(name.trim()));
        Some(Place {
            line,
            col,
            top,
            lang,
        })
    }
}

/// One file and where it was left.
pub type Entry = (PathBuf, Place);

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Places {
    /// Most recent first.
    list: Vec<Entry>,
    /// Counts every change of what is kept, so the session's signature can tell without looking.
    pub version: u64,
}

impl Places {
    /// What a session file held (already in order, most recent first).
    pub fn from_session(list: &[Entry]) -> Places {
        let mut m = Places::default();
        for (path, place) in list {
            if place.is_start() || m.list.iter().any(|(p, _)| same_path(p, path)) {
                continue;
            }
            m.list.push((path.clone(), *place));
            if m.list.len() == FILES_MAX {
                break;
            }
        }
        m
    }

    /// Where a file was left, if it is kept.
    pub fn get(&self, path: &Path) -> Option<Place> {
        self.list
            .iter()
            .find(|(p, _)| same_path(p, path))
            .map(|(_, place)| *place)
    }

    /// A file's place as it leaves (it was closed): kept in front; the start removes the file.
    pub fn put(&mut self, path: &Path, place: Place) {
        let at = self.list.iter().position(|(p, _)| same_path(p, path));
        let before = self.list.clone();
        if let Some(i) = at {
            self.list.remove(i);
        }
        if !place.is_start() {
            self.list.insert(0, (path.to_path_buf(), place));
            self.list.truncate(FILES_MAX);
        }
        if self.list != before {
            self.version += 1;
        }
    }

    /// What the session file gets: the open files' places (in the order given) in front of the
    /// closed files', each file once, at most `FILES_MAX`.
    pub fn snapshot<'a>(&self, open: impl Iterator<Item = (&'a Path, Place)>) -> Vec<Entry> {
        let mut out: Vec<Entry> = Vec::new();
        // A file open at its start is at its start, whatever it left when it was closed.
        let mut open_paths: Vec<&Path> = Vec::new();
        for (path, place) in open {
            open_paths.push(path);
            if !place.is_start() && !out.iter().any(|(p, _)| same_path(p, path)) {
                out.push((path.to_path_buf(), place));
            }
        }
        for (path, place) in &self.list {
            if !open_paths.iter().any(|p| same_path(p, path)) {
                out.push((path.clone(), *place));
            }
        }
        out.truncate(FILES_MAX);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    fn at(line: usize, col: usize, top: usize) -> Place {
        Place {
            line,
            col,
            top,
            lang: None,
        }
    }

    #[test]
    fn a_closed_file_leaves_its_place_in_front_and_the_start_removes_it() {
        let mut m = Places::default();
        m.put(&p("C:/a.txt"), at(40, 3, 30));
        assert_eq!(m.get(&p("C:/a.txt")), Some(at(40, 3, 30)));
        assert_eq!(m.version, 1);
        // The same again changes nothing, and nothing of a file left at its start.
        m.put(&p("C:/a.txt"), at(40, 3, 30));
        m.put(&p("C:/never.txt"), Place::default());
        assert_eq!(m.version, 1);
        m.put(&p("C:/b.txt"), at(1, 0, 0));
        assert_eq!(m.list[0].0, p("C:/b.txt"));
        m.put(&p("C:/a.txt"), Place::default());
        assert!(m.get(&p("C:/a.txt")).is_none());
        // Only the most recent files are kept.
        for i in 0..150 {
            m.put(&p(&format!("C:/f{i}.txt")), at(i + 1, 0, 0));
        }
        assert_eq!(m.list.len(), FILES_MAX);
        assert!(m.get(&p("C:/f149.txt")).is_some() && m.get(&p("C:/f49.txt")).is_none());
    }

    #[test]
    fn the_session_gets_the_open_files_first_and_reads_back() {
        let mut m = Places::default();
        m.put(&p("C:/closed.txt"), at(9, 1, 2));
        m.put(&p("C:/open.txt"), at(5, 5, 5));
        let open = [
            (p("C:/open.txt"), at(7, 0, 3)),
            (p("C:/new.txt"), at(2, 2, 0)),
        ];
        let snap = m.snapshot(open.iter().map(|(path, place)| (path.as_path(), *place)));
        assert_eq!(
            snap,
            [
                (p("C:/open.txt"), at(7, 0, 3)),
                (p("C:/new.txt"), at(2, 2, 0)),
                (p("C:/closed.txt"), at(9, 1, 2)),
            ]
        );
        assert_eq!(
            Places::from_session(&snap).get(&p("C:/new.txt")),
            Some(at(2, 2, 0))
        );
        assert_eq!(Place::parse(&at(3, 4, 5).to_setting()), Some(at(3, 4, 5)));
        assert_eq!(Place::parse("1,2"), None);
        assert_eq!(Place::parse("a,2,3"), None);
        // A language chosen for the file rides along, and keeps a file at its start.
        let md = Place {
            lang: Some("Markdown"),
            ..Place::default()
        };
        assert!(!md.is_start());
        assert_eq!(md.to_setting(), "0,0,0,Markdown");
        assert_eq!(Place::parse("0,0,0,Markdown"), Some(md));
        assert_eq!(
            Place::parse("0,0,0,Plain text").unwrap().lang,
            Some("Plain text")
        );
        // One this build does not know is dropped.
        assert_eq!(Place::parse("1,2,3,Klingon"), Some(at(1, 2, 3)));
    }
}
