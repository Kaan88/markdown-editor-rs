//! The session file: tabs, recent files, theme, window geometry, the Files pane. Section `[mdedit]`
//! of `%LOCALAPPDATA%\mdedit\settings.ini` (or the file `MDEDIT_SETTINGS` names), written with the
//! shared settings layer. A missing or damaged file gives defaults; unknown keys are skipped.

use mdshared_app::recents::Recents;
use mdshared_app::settings::{self, Writer, decode_path, encode_path, escape, unescape};
use std::io;
use std::path::{Path, PathBuf};

pub const SECTION: &str = "mdedit";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ThemeChoice {
    #[default]
    System,
    Light,
    Dark,
}

impl ThemeChoice {
    pub fn from_setting(v: u8) -> ThemeChoice {
        match v {
            1 => ThemeChoice::Light,
            2 => ThemeChoice::Dark,
            _ => ThemeChoice::System,
        }
    }

    pub fn to_setting(self) -> u8 {
        match self {
            ThemeChoice::System => 0,
            ThemeChoice::Light => 1,
            ThemeChoice::Dark => 2,
        }
    }

    pub fn is_dark(self, system_dark: bool) -> bool {
        match self {
            ThemeChoice::System => system_dark,
            ThemeChoice::Light => false,
            ThemeChoice::Dark => true,
        }
    }
}

/// A tab that had unsaved text when the window closed: its text is in `file` of the backup
/// folder beside the settings file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unsaved {
    /// A name this application made: digits and `.txt`.
    pub file: String,
    /// The number of a document that has no file ("Untitled 2"), else 0.
    pub untitled: u32,
    pub path: Option<PathBuf>,
}

impl Unsaved {
    /// The backup's name for a number.
    pub fn file_name(n: u32) -> String {
        format!("{n}.txt")
    }

    /// `file` is a name `file_name` gives, so joining it to the folder stays inside it.
    pub fn is_file_name(file: &str) -> bool {
        file.strip_suffix(".txt")
            .is_some_and(|n| !n.is_empty() && n.len() <= 9 && n.bytes().all(|b| b.is_ascii_digit()))
    }

    fn parse(value: &str) -> Option<Unsaved> {
        let mut parts = value.splitn(3, '|');
        let file = parts.next()?.trim();
        let untitled = parts.next()?.trim().parse().ok()?;
        let path = parts.next()?;
        Unsaved::is_file_name(file).then(|| Unsaved {
            file: file.to_string(),
            untitled,
            path: (!path.is_empty()).then(|| decode_path(path)),
        })
    }

    fn serialize(&self) -> String {
        let path = self.path.as_deref().map(encode_path).unwrap_or_default();
        format!("{}|{}|{path}", self.file, self.untitled)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Session {
    pub tabs: Vec<PathBuf>,
    /// The tabs whose unsaved text was kept when the window last closed.
    pub unsaved: Vec<Unsaved>,
    pub active: usize,
    pub recent: Recents,
    pub theme: ThemeChoice,
    /// The text size, percent.
    pub zoom: u32,
    /// Show whitespace and line endings.
    pub whitespace: bool,
    /// Word wrap.
    pub wrap: bool,
    /// `WxH[ max][ @x,y]`, the viewer's format.
    pub geometry: String,
    /// Whether the Files pane shows; `None` when never set.
    pub files: Option<bool>,
    /// The Files pane's width, logical px, once the user dragged it.
    pub files_w: Option<u32>,
    pub folder: Option<PathBuf>,
    /// Look for a newer version at launch, at most once a day.
    pub check_updates: bool,
    /// When the last look finished, seconds since the Unix epoch; 0 for never.
    pub update_checked: u64,
}

impl Default for Session {
    fn default() -> Session {
        Session {
            tabs: Vec::new(),
            unsaved: Vec::new(),
            active: 0,
            recent: Recents::default(),
            theme: ThemeChoice::default(),
            zoom: 100,
            whitespace: false,
            wrap: false,
            geometry: String::new(),
            files: None,
            files_w: None,
            folder: None,
            check_updates: true,
            update_checked: 0,
        }
    }
}

impl Session {
    pub fn load(path: &Path) -> Session {
        std::fs::read(path)
            .map(|b| Session::parse(&String::from_utf8_lossy(&b)))
            .unwrap_or_default()
    }

    pub fn parse(text: &str) -> Session {
        let mut s = Session::default();
        let mut recent = Vec::new();
        let mut in_section = true;
        for line in text.lines() {
            let t = line.trim();
            if t.starts_with('[') && t.ends_with(']') {
                in_section = &t[1..t.len() - 1] == SECTION;
                continue;
            }
            if !in_section {
                continue;
            }
            for (key, value) in settings::pairs(line) {
                match key {
                    "tab" if !value.is_empty() => s.tabs.push(decode_path(value)),
                    "unsaved" => s.unsaved.extend(Unsaved::parse(value)),
                    "recent" if !value.is_empty() => recent.push(decode_path(value)),
                    "active" => s.active = value.trim().parse().unwrap_or(0),
                    "theme" => {
                        s.theme = ThemeChoice::from_setting(value.trim().parse().unwrap_or(0))
                    }
                    "zoom" => {
                        s.zoom = value
                            .trim()
                            .parse()
                            .ok()
                            .filter(|z| (50..=400).contains(z))
                            .unwrap_or(100)
                    }
                    "whitespace" => s.whitespace = value.trim() == "1",
                    "wrap" => s.wrap = value.trim() == "1",
                    "geometry" => s.geometry = unescape(value),
                    "files" => {
                        s.files = match value.trim() {
                            "1" => Some(true),
                            "0" => Some(false),
                            _ => None,
                        }
                    }
                    "files_w" => {
                        s.files_w = value
                            .trim()
                            .parse()
                            .ok()
                            .filter(|w| (40..=4000).contains(w))
                    }
                    "folder" if !value.is_empty() => s.folder = Some(decode_path(value)),
                    "check_updates" => s.check_updates = value.trim() != "0",
                    "update_checked" => s.update_checked = value.trim().parse().unwrap_or(0),
                    _ => {}
                }
            }
        }
        // Added in reverse so the file's order, most recent first, is kept through `add`.
        for p in recent.into_iter().rev() {
            s.recent.add(p);
        }
        s.active = s.active.min(s.tabs.len().saturating_sub(1));
        s
    }

    pub fn serialize(&self) -> String {
        let mut out = Writer::new(SECTION);
        for t in &self.tabs {
            out.line("tab", &encode_path(t));
        }
        for u in &self.unsaved {
            out.line("unsaved", &u.serialize());
        }
        out.line("active", &self.active.to_string());
        for r in self.recent.paths() {
            out.line("recent", &encode_path(r));
        }
        out.line("theme", &self.theme.to_setting().to_string());
        if self.zoom != 100 {
            out.line("zoom", &self.zoom.to_string());
        }
        if self.whitespace {
            out.line("whitespace", "1");
        }
        if self.wrap {
            out.line("wrap", "1");
        }
        if !self.geometry.is_empty() {
            out.line("geometry", &escape(&self.geometry));
        }
        if let Some(f) = self.files {
            out.line("files", if f { "1" } else { "0" });
        }
        if let Some(w) = self.files_w {
            out.line("files_w", &w.to_string());
        }
        if let Some(f) = &self.folder {
            out.line("folder", &encode_path(f));
        }
        out.line("check_updates", if self.check_updates { "1" } else { "0" });
        out.line("update_checked", &self.update_checked.to_string());
        out.finish()
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        settings::write_atomic(path, &self.serialize())
    }

    /// The saved window size in logical px and whether it was maximized. Sizes outside
    /// 320..=16384 are ignored, as in the viewer.
    pub fn window_size(&self) -> Option<(u32, u32, bool)> {
        let mut words = self.geometry.split_whitespace();
        let (w, h) = words.next()?.split_once('x')?;
        let (w, h): (u32, u32) = (w.parse().ok()?, h.parse().ok()?);
        let ok = |v: u32| (320..=16384).contains(&v);
        if !ok(w) || !ok(h) {
            return None;
        }
        Some((w, h, words.any(|word| word == "max")))
    }

    /// Records the window's size. A maximized window keeps the restored size saved before (its
    /// own size is the screen's), so Restore down after the next start gives that size back.
    pub fn set_window_size(&mut self, width: u32, height: u32, maximized: bool) {
        let (width, height) = match (maximized, self.window_size()) {
            (true, Some((w, h, _))) => (w, h),
            _ => (width, height),
        };
        let position = self.window_position();
        self.geometry = format!("{width}x{height}{}", if maximized { " max" } else { "" });
        if let Some((x, y)) = position {
            self.geometry.push_str(&format!(" @{x},{y}"));
        }
    }

    /// The restored window's top-left corner in desktop device px, `@x,y` after the size.
    pub fn window_position(&self) -> Option<(i32, i32)> {
        let word = self
            .geometry
            .split_whitespace()
            .find_map(|w| w.strip_prefix('@'))?;
        let (x, y) = word.split_once(',')?;
        let (x, y): (i32, i32) = (x.parse().ok()?, y.parse().ok()?);
        let ok = |v: i32| (-100_000..=100_000).contains(&v);
        (ok(x) && ok(y)).then_some((x, y))
    }

    pub fn set_window_position(&mut self, x: i32, y: i32) {
        let size: String = self
            .geometry
            .split_whitespace()
            .filter(|word| !word.starts_with('@'))
            .collect::<Vec<_>>()
            .join(" ");
        self.geometry = format!("{size} @{x},{y}").trim_start().to_string();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let mut s = Session {
            tabs: vec![PathBuf::from("C:/a b/x.rs"), PathBuf::from("C:/c/y=z%.md")],
            active: 1,
            theme: ThemeChoice::Dark,
            files: Some(true),
            files_w: Some(300),
            folder: Some(PathBuf::from("C:/proj")),
            ..Session::default()
        };
        s.recent.add("C:/old.txt");
        s.recent.add("C:/new.txt");
        s.set_window_size(1100, 700, false);
        s.set_window_position(-8, 40);
        let text = s.serialize();
        assert!(text.starts_with("[mdedit]"));
        let back = Session::parse(&text);
        assert_eq!(back, s);
        assert_eq!(back.recent.paths()[0], PathBuf::from("C:/new.txt"));
        assert_eq!(back.window_size(), Some((1100, 700, false)));
        assert_eq!(back.window_position(), Some((-8, 40)));
    }

    #[test]
    fn unsaved_tabs_round_trip() {
        let mut s = Session {
            unsaved: Vec::new(),
            ..Session::default()
        };
        s.unsaved.extend([
            Unsaved {
                file: Unsaved::file_name(1),
                untitled: 0,
                path: Some(PathBuf::from("C:/a b/c|d.txt")),
            },
            Unsaved {
                file: Unsaved::file_name(12),
                untitled: 3,
                path: None,
            },
        ]);
        assert_eq!(Session::parse(&s.serialize()), s);
        // Only names this application makes are taken: nothing that could leave the folder.
        for bad in [
            "..\\x.txt|0|",
            "a.txt|0|",
            ".txt|0|",
            "1.txt|x|",
            "1.txt",
            "1.txt|0",
        ] {
            let text = format!("[mdedit]\nunsaved={bad}\n");
            assert!(Session::parse(&text).unsaved.is_empty(), "{bad}");
        }
        assert!(Unsaved::is_file_name("7.txt") && !Unsaved::is_file_name("7.txt.txt"));
    }

    #[test]
    fn unknown_keys_and_other_sections_are_ignored() {
        let text = "[mdedit]\nbogus=1\ntheme=1\nzoom=150\n\n[other]\ntheme=2\ntab=C:/x\n";
        let s = Session::parse(text);
        assert_eq!(s.theme, ThemeChoice::Light);
        assert!(s.tabs.is_empty());
        assert_eq!(Session::parse("garbage\n=\n\u{0}"), Session::default());
        // Active is clamped to the tabs there are.
        let s = Session::parse("[mdedit]\ntab=a\nactive=7\n");
        assert_eq!(s.active, 0);
        assert_eq!(Session::parse("[mdedit]\nfiles_w=5\n").files_w, None);
        assert_eq!(Session::parse("[mdedit]\nfiles=2\n").files, None);
    }

    #[test]
    fn geometry_limits() {
        let mut s = Session::default();
        assert_eq!(s.window_size(), None);
        s.geometry = "1280x800 max".into();
        assert_eq!(s.window_size(), Some((1280, 800, true)));
        s.geometry = "319x800".into();
        assert_eq!(s.window_size(), None);
        s.geometry = "1000x16385".into();
        assert_eq!(s.window_size(), None);
        s.geometry = "16384x320".into();
        assert_eq!(s.window_size(), Some((16384, 320, false)));
        s.geometry = "junk".into();
        assert_eq!(s.window_size(), None);
        s.geometry = "800x600 @100001,0".into();
        assert_eq!(s.window_position(), None);
        s.set_window_position(10, 20);
        assert_eq!(s.geometry, "800x600 @10,20");
        // Maximized: the restored size stays.
        s.set_window_size(900, 700, true);
        assert_eq!(s.geometry, "800x600 max @10,20");
        let mut e = Session::default();
        e.set_window_position(3, 4);
        assert_eq!(e.geometry, "@3,4");
        assert_eq!(e.window_size(), None);
    }

    #[test]
    fn theme_choice() {
        for c in [ThemeChoice::System, ThemeChoice::Light, ThemeChoice::Dark] {
            assert_eq!(ThemeChoice::from_setting(c.to_setting()), c);
        }
        assert!(ThemeChoice::System.is_dark(true));
        assert!(!ThemeChoice::Light.is_dark(true));
        assert!(ThemeChoice::Dark.is_dark(false));
    }

    #[test]
    fn maximizing_keeps_the_restored_size() {
        let mut s = Session::default();
        s.set_window_size(1100, 700, false);
        s.set_window_position(40, 30);
        s.set_window_size(2560, 1380, true);
        assert_eq!(s.window_size(), Some((1100, 700, true)));
        assert_eq!(s.window_position(), Some((40, 30)));
        // Restored again at a new size.
        s.set_window_size(900, 600, false);
        assert_eq!(s.window_size(), Some((900, 600, false)));
        // Maximized with no size saved before: its own size is all there is.
        let mut fresh = Session::default();
        fresh.set_window_size(2560, 1380, true);
        assert_eq!(fresh.window_size(), Some((2560, 1380, true)));
    }
}
