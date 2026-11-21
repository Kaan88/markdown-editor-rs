//! The session file: tabs, recent files, theme, window geometry, the Files pane. Section `[mdedit]`
//! of `%LOCALAPPDATA%\mdedit\settings.ini` (or the file `MDEDIT_SETTINGS` names), written with the
//! shared settings layer. A missing or damaged file gives defaults; unknown keys are skipped.

use mdshared_app::recents::Recents;
use mdshared_app::settings::{self, Writer, decode_path, encode_path, escape, unescape};
use mdshared_ui::corners::Corners;
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
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Unsaved {
    /// A name this application made: digits and `.txt`.
    pub file: String,
    /// The number of a document that has no file ("Untitled 2"), else 0.
    pub untitled: u32,
    pub path: Option<PathBuf>,
    /// The draft's id, the same in its recovery copy's header (kept as a `draft=<file>|<hex>`
    /// line of its own, which builds before 1.1 skip). `None` in a session from before 1.1.
    pub draft: Option<u64>,
}

/// What [`Unsaved::prune`] needs to know of an open tab (borrowed: it is asked on every pass of
/// the event loop while a backup is listed).
pub struct TabState<'a> {
    pub path: Option<&'a Path>,
    pub untitled: u32,
    pub modified: bool,
    /// The backup file the tab still waits to put back, by name.
    pub waiting: Option<String>,
    /// The tab's draft id ([`Unsaved::draft`]).
    pub draft: u64,
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

    /// The entries of `list` that are still needed while `tabs` are open. An entry goes once no
    /// tab waits for its backup file and no modified tab stands for its document (the same
    /// path, or for an entry without a path an untitled tab of that number): the text was saved,
    /// discarded or closed since. While the text is still unsaved in a tab the entry stays, as a
    /// fallback for a recovery copy that is missing.
    pub fn prune(list: &[Unsaved], tabs: &[TabState<'_>]) -> Vec<Unsaved> {
        list.iter()
            .filter(|u| {
                tabs.iter().any(|t| {
                    t.waiting.as_deref() == Some(u.file.as_str())
                        || t.modified
                            && match (u.draft, &u.path) {
                                // The draft's id says which tab it is, whatever its name now.
                                (Some(d), _) => t.draft == d,
                                (None, Some(p)) => t.path == Some(p.as_path()),
                                (None, None) => t.path.is_none() && t.untitled == u.untitled,
                            }
                })
            })
            .cloned()
            .collect()
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
            draft: None,
        })
    }

    fn serialize(&self) -> String {
        let path = self.path.as_deref().map(encode_path).unwrap_or_default();
        format!("{}|{}|{path}", self.file, self.untitled)
    }

    /// `draft=<file>|<hex>`: the id of the entry whose backup is `file`.
    fn parse_draft(value: &str) -> Option<(&str, u64)> {
        let (file, id) = value.split_once('|')?;
        Some((file.trim(), u64::from_str_radix(id.trim(), 16).ok()?))
    }
}

/// A pinned tab: its file, or the number of a document that has none ("Untitled 2"), whose
/// text comes back from the backup kept beside the session (`pin=#2`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Pin {
    File(PathBuf),
    Untitled(u32),
}

impl Pin {
    fn parse(value: &str) -> Option<Pin> {
        match value.trim().strip_prefix('#') {
            Some(n) => n.parse().ok().filter(|&n| n > 0).map(Pin::Untitled),
            None => (!value.is_empty()).then(|| Pin::File(decode_path(value))),
        }
    }

    fn serialize(&self) -> String {
        match self {
            Pin::File(path) => encode_path(path),
            Pin::Untitled(n) => format!("#{n}"),
        }
    }
}

/// A split view: the file in the second pane, the side it is on, and the left pane's share of
/// the width in thousandths.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Split {
    pub path: PathBuf,
    pub left: bool,
    pub permille: u16,
}

impl Split {
    /// The share kept in the file: the layout clamps it again to the pane's minimum width.
    pub const MIN: u16 = 100;
    pub const MAX: u16 = 900;

    pub fn new(path: PathBuf, left: bool, ratio: f32) -> Split {
        let permille = if ratio.is_finite() {
            (ratio * 1000.0)
                .round()
                .clamp(Split::MIN as f32, Split::MAX as f32) as u16
        } else {
            500
        };
        Split {
            path,
            left,
            permille,
        }
    }

    pub fn ratio(&self) -> f32 {
        self.permille as f32 / 1000.0
    }

    /// `<L|R>|<permille>|<path>`; anything else is no split.
    fn parse(value: &str) -> Option<Split> {
        let mut parts = value.splitn(3, '|');
        let left = match parts.next()?.trim() {
            "L" => true,
            "R" => false,
            _ => return None,
        };
        let permille: u16 = parts.next()?.trim().parse().ok()?;
        let path = parts.next().filter(|p| !p.is_empty())?;
        Some(Split {
            path: decode_path(path),
            left,
            permille: permille.clamp(Split::MIN, Split::MAX),
        })
    }

    fn serialize(&self) -> String {
        format!(
            "{}|{}|{}",
            if self.left { "L" } else { "R" },
            self.permille,
            encode_path(&self.path)
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Session {
    pub tabs: Vec<PathBuf>,
    /// The pinned tabs.
    pub pinned: Vec<Pin>,
    /// The tabs whose unsaved text was kept when the window last closed.
    pub unsaved: Vec<Unsaved>,
    pub active: usize,
    /// The tab that was showing, by what it is: `active` counts only tabs with a file, so a new
    /// document cannot be found by it. Read first; `active` is for sessions written before.
    pub active_tab: Option<Pin>,
    /// The pinned tab shown last (for the setting to start on it).
    pub last_pinned: Option<Pin>,
    /// The days mdedit was used: how many, and the date (days since 1970) of the last. See
    /// `mdshared_ui::stale`.
    pub use_days: (u32, u64),
    /// The day of use each tab was last shown on: one `seen` line each.
    pub seen: Vec<(Pin, u32)>,
    /// Each file tab's focused time in seconds (`focus=<seconds> <path>`), for the setting that
    /// leaves out tabs barely looked at.
    pub focus: Vec<(PathBuf, u64)>,
    /// The split view, when two tabs were side by side.
    pub split: Option<Split>,
    /// The find options (match case, whole word, regular expression).
    pub find_options: (bool, bool, bool),
    /// The last queries and replacements typed, newest first.
    pub find_history: Vec<String>,
    pub replace_history: Vec<String>,
    pub recent: Recents,
    pub theme: ThemeChoice,
    /// The rounding of the window's corners (Settings, "Window corners").
    pub corners: Corners,
    /// The text size, percent.
    pub zoom: u32,
    /// Show whitespace and line endings.
    pub whitespace: bool,
    /// Word wrap.
    pub wrap: bool,
    /// The minimap beside the text.
    pub minimap: bool,
    /// The format bar under the command bar, for Markdown documents; on until turned off.
    pub format_bar: bool,
    /// `WxH[ max][ @x,y]`, the viewer's format.
    pub geometry: String,
    /// Whether the Files pane shows; `None` when never set.
    pub files: Option<bool>,
    /// The Files pane's width, logical px, once the user dragged it.
    pub files_w: Option<u32>,
    /// The results panel's height, logical px, once the user dragged it.
    pub results_h: Option<u32>,
    /// What the results panel's boxes for the folder and the kinds of file held; empty for
    /// nothing typed.
    pub find_folder: String,
    pub find_kinds: String,
    /// Bookmarked lines of the files used most recently (`mark=1,5;path`), most recent first.
    pub marks: Vec<(PathBuf, Vec<usize>)>,
    /// Where the caret and the scroll were in the files used most recently
    /// (`place=line,col,top;path`), most recent first.
    pub places: Vec<crate::places::Entry>,
    pub folder: Option<PathBuf>,
    /// Look for a newer version at launch, at every launch.
    pub check_updates: bool,
    /// When the last look finished, seconds since the Unix epoch; 0 for never.
    pub update_checked: u64,
    /// The preferences (the `[preferences]` section, owned by the core).
    pub prefs: mdedit_core::prefs::Prefs,
}

impl Default for Session {
    fn default() -> Session {
        Session {
            tabs: Vec::new(),
            pinned: Vec::new(),
            active_tab: None,
            last_pinned: None,
            use_days: (0, 0),
            seen: Vec::new(),
            focus: Vec::new(),
            unsaved: Vec::new(),
            active: 0,
            split: None,
            find_options: (false, false, false),
            find_history: Vec::new(),
            replace_history: Vec::new(),
            recent: Recents::default(),
            theme: ThemeChoice::default(),
            corners: Corners::default(),
            zoom: 100,
            whitespace: false,
            wrap: false,
            minimap: false,
            format_bar: true,
            geometry: String::new(),
            files: None,
            files_w: None,
            results_h: None,
            find_folder: String::new(),
            find_kinds: String::new(),
            marks: Vec::new(),
            places: Vec::new(),
            folder: None,
            check_updates: true,
            update_checked: 0,
            prefs: mdedit_core::prefs::Prefs::default(),
        }
    }
}

impl Session {
    /// The session in `path`: none yet (no file) is an empty one; a file that cannot be read is
    /// an error, never taken as empty (the next exit would write over it and drop its drafts).
    #[cfg(test)]
    pub fn read(path: &Path) -> std::io::Result<Session> {
        Ok(settings::read(path)?.map_or_else(Session::default, |t| Session::parse(&t)))
    }

    /// [`read`](Self::read), tried again for up to two seconds while it fails: another program
    /// (a scanner, a sync client) may have the file open a moment.
    pub fn read_patiently(path: &Path) -> std::io::Result<Session> {
        Ok(settings::read_patiently(path)?.map_or_else(Session::default, |t| Session::parse(&t)))
    }

    pub fn parse(text: &str) -> Session {
        let mut s = Session::default();
        let mut recent = Vec::new();
        let mut drafts: Vec<(String, u64)> = Vec::new();
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
                    "draft" => drafts
                        .extend(Unsaved::parse_draft(value).map(|(f, id)| (f.to_string(), id))),
                    "pin" => s.pinned.extend(Pin::parse(value)),
                    "recent" if !value.is_empty() => recent.push(decode_path(value)),
                    "split" => s.split = Split::parse(value),
                    "find_case" => s.find_options.0 = value.trim() == "1",
                    "find_word" => s.find_options.1 = value.trim() == "1",
                    "find_regex" => s.find_options.2 = value.trim() == "1",
                    "find_hist" if s.find_history.len() < crate::recall::KEPT => {
                        s.find_history.push(unescape(value))
                    }
                    "replace_hist" if s.replace_history.len() < crate::recall::KEPT => {
                        s.replace_history.push(unescape(value))
                    }
                    "active" => s.active = value.trim().parse().unwrap_or(0),
                    "active_tab" => s.active_tab = Pin::parse(value),
                    "last_pinned" => s.last_pinned = Pin::parse(value),
                    "use_days" => {
                        let mut parts = value.split_whitespace();
                        let n = parts.next().and_then(|n| n.parse().ok());
                        let d = parts.next().and_then(|d| d.parse().ok());
                        if let (Some(n), Some(d)) = (n, d) {
                            s.use_days = (n, d);
                        }
                    }
                    "focus" => s.focus.extend(mdshared_app::brief::parse(value)),
                    "seen" => {
                        if let Some((day, pin)) = value.split_once(' ')
                            && let (Ok(day), Some(pin)) = (day.parse(), Pin::parse(pin))
                        {
                            s.seen.push((pin, day));
                        }
                    }
                    "theme" => {
                        s.theme = ThemeChoice::from_setting(value.trim().parse().unwrap_or(0))
                    }
                    "corners" => {
                        s.corners = Corners::from_setting(value.trim().parse().unwrap_or(8))
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
                    "minimap" => s.minimap = value.trim() == "1",
                    "format_bar" => s.format_bar = value.trim() != "0",
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
                    "results_h" => {
                        s.results_h = value
                            .trim()
                            .parse()
                            .ok()
                            .filter(|h| (40..=4000).contains(h))
                    }
                    "mark" => {
                        // Lines, then the path after the first `;`.
                        if let Some((lines, path)) = value.split_once(';') {
                            let lines: Vec<usize> = lines
                                .split(',')
                                .filter_map(|l| l.trim().parse().ok())
                                .collect();
                            if !lines.is_empty()
                                && !path.is_empty()
                                && s.marks.len() < crate::marks::FILES_MAX
                            {
                                s.marks.push((decode_path(path), lines));
                            }
                        }
                    }
                    "place" => {
                        // The place, then the path after the first `;`.
                        if let Some((place, path)) = value.split_once(';')
                            && let Some(place) = crate::places::Place::parse(place)
                            && !path.is_empty()
                            && s.places.len() < crate::places::FILES_MAX
                        {
                            s.places.push((decode_path(path), place));
                        }
                    }
                    "find_folder" => s.find_folder = unescape(value),
                    "find_kinds" => s.find_kinds = unescape(value),
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
        for u in &mut s.unsaved {
            u.draft = drafts.iter().find(|(f, _)| *f == u.file).map(|(_, id)| *id);
        }
        s.active = s.active.min(s.tabs.len().saturating_sub(1));
        s.prefs = mdedit_core::prefs::Prefs::parse(text);
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
        for u in &self.unsaved {
            if let Some(d) = u.draft {
                out.line("draft", &format!("{}|{d:016x}", u.file));
            }
        }
        for p in &self.pinned {
            out.line("pin", &p.serialize());
        }
        out.line("active", &self.active.to_string());
        if let Some(p) = &self.active_tab {
            out.line("active_tab", &p.serialize());
        }
        if let Some(p) = &self.last_pinned {
            out.line("last_pinned", &p.serialize());
        }
        if self.use_days != (0, 0) {
            out.line(
                "use_days",
                &format!("{} {}", self.use_days.0, self.use_days.1),
            );
        }
        for (p, day) in &self.seen {
            out.line("seen", &format!("{day} {}", p.serialize()));
        }
        for (path, secs) in &self.focus {
            out.line("focus", &mdshared_app::brief::to_setting(*secs, path));
        }
        if let Some(split) = &self.split {
            out.line("split", &split.serialize());
        }
        let (case, word, regex) = self.find_options;
        for (key, on) in [
            ("find_case", case),
            ("find_word", word),
            ("find_regex", regex),
        ] {
            if on {
                out.line(key, "1");
            }
        }
        for h in self.find_history.iter().take(crate::recall::KEPT) {
            out.line("find_hist", &escape(h));
        }
        for h in self.replace_history.iter().take(crate::recall::KEPT) {
            out.line("replace_hist", &escape(h));
        }
        for r in self.recent.paths() {
            out.line("recent", &encode_path(r));
        }
        out.line("theme", &self.theme.to_setting().to_string());
        if self.corners != Corners::default() {
            out.line("corners", &self.corners.to_setting().to_string());
        }
        if self.zoom != 100 {
            out.line("zoom", &self.zoom.to_string());
        }
        if self.whitespace {
            out.line("whitespace", "1");
        }
        if self.wrap {
            out.line("wrap", "1");
        }
        if self.minimap {
            out.line("minimap", "1");
        }
        if !self.format_bar {
            out.line("format_bar", "0");
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
        if let Some(h) = self.results_h {
            out.line("results_h", &h.to_string());
        }
        for (path, lines) in &self.marks {
            let lines: Vec<String> = lines.iter().map(usize::to_string).collect();
            out.line(
                "mark",
                &format!("{};{}", lines.join(","), encode_path(path)),
            );
        }
        for (path, place) in &self.places {
            out.line(
                "place",
                &format!("{};{}", place.to_setting(), encode_path(path)),
            );
        }
        if !self.find_folder.is_empty() {
            out.line("find_folder", &escape(&self.find_folder));
        }
        if !self.find_kinds.is_empty() {
            out.line("find_kinds", &escape(&self.find_kinds));
        }
        if let Some(f) = &self.folder {
            out.line("folder", &encode_path(f));
        }
        out.line("check_updates", if self.check_updates { "1" } else { "0" });
        out.line("update_checked", &self.update_checked.to_string());
        // The core writes its own section after this one.
        format!("{}\n{}", out.finish(), self.prefs.serialize())
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

    fn entry(file: &str, untitled: u32, path: Option<&str>) -> Unsaved {
        Unsaved {
            file: file.into(),
            untitled,
            path: path.map(PathBuf::from),
            draft: None,
        }
    }

    fn tab<'a>(
        path: Option<&'a str>,
        untitled: u32,
        modified: bool,
        waiting: Option<&str>,
    ) -> TabState<'a> {
        TabState {
            path: path.map(Path::new),
            untitled,
            modified,
            waiting: waiting.map(String::from),
            draft: 0,
        }
    }

    #[test]
    fn a_backup_stays_while_its_text_is_unsaved_and_goes_after() {
        let list = [entry("1.txt", 0, Some("/a")), entry("2.txt", 3, None)];
        // Nothing open: both are stale.
        assert!(Unsaved::prune(&list, &[]).is_empty());
        // A tab waits for its file, whether or not it is modified.
        let waiting = [tab(Some("/a"), 0, false, Some("1.txt"))];
        assert_eq!(Unsaved::prune(&list, &waiting), [list[0].clone()]);
        // A modified tab stands for the document.
        let open = [tab(Some("/a"), 0, true, None), tab(None, 3, true, None)];
        assert_eq!(Unsaved::prune(&list, &open), list);
        // Saved or closed: the tab is not modified, or is gone.
        let saved = [tab(Some("/a"), 0, false, None), tab(None, 3, false, None)];
        assert!(Unsaved::prune(&list, &saved).is_empty());
        // Another path, another untitled number, or a titled tab does not stand for it.
        let other = [
            tab(Some("/b"), 0, true, None),
            tab(None, 4, true, None),
            tab(Some("/c"), 3, true, None),
            tab(None, 0, true, Some("9.txt")),
        ];
        assert!(Unsaved::prune(&list, &other).is_empty());
        // Only the discarded one goes.
        let one = [tab(None, 3, true, None), tab(Some("/a"), 0, false, None)];
        assert_eq!(Unsaved::prune(&list, &one), [list[1].clone()]);
    }

    #[test]
    fn find_options_and_history_round_trip_and_are_bounded() {
        let s = Session {
            find_options: (true, false, true),
            find_history: vec![
                "new one".into(),
                "50%".into(),
                "a=b\\c".into(),
                "ünï".into(),
            ],
            replace_history: vec!["x y".into()],
            ..Session::default()
        };
        let back = Session::parse(&s.serialize());
        assert_eq!(back.find_options, (true, false, true));
        assert_eq!(back.find_history, s.find_history);
        assert_eq!(back.replace_history, s.replace_history);
        // A file with far more lines than that keeps the first KEPT of each.
        let many: String = (0..100)
            .map(|i| format!("find_hist={i}\nreplace_hist={i}\n"))
            .collect();
        let back = Session::parse(&format!("[mdedit]\n{many}"));
        assert_eq!(back.find_history.len(), crate::recall::KEPT);
        assert_eq!(back.replace_history[0], "0");
        // Defaults when absent.
        assert_eq!(
            Session::parse("[mdedit]\n").find_options,
            (false, false, false)
        );
    }

    #[test]
    fn a_split_round_trips_and_a_damaged_one_is_none() {
        let split = Split::new(PathBuf::from("C:/a b/y|z.md"), true, 0.37);
        assert_eq!(split.permille, 370);
        let s = Session {
            tabs: vec![PathBuf::from("C:/x.rs"), split.path.clone()],
            split: Some(split.clone()),
            ..Session::default()
        };
        assert_eq!(Session::parse(&s.serialize()).split, Some(split));
        // Out-of-range shares are clamped; nothing else gets through.
        assert_eq!(Split::new("a".into(), false, 7.0).permille, Split::MAX);
        assert_eq!(Split::new("a".into(), false, f32::NAN).permille, 500);
        assert_eq!(Split::parse("R|950|a").unwrap().permille, Split::MAX);
        assert_eq!(Split::parse("R|99999|a"), None);
        for bad in [
            "", "X|500|a", "L|abc|a", "L|500|", "L|500", "L", "|||", "L|-5|a",
        ] {
            assert_eq!(Split::parse(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn round_trip() {
        let mut s = Session {
            tabs: vec![PathBuf::from("C:/a b/x.rs"), PathBuf::from("C:/c/y=z%.md")],
            pinned: vec![Pin::File(PathBuf::from("C:/a b/x.rs")), Pin::Untitled(3)],
            active: 1,
            active_tab: Some(Pin::Untitled(3)),
            last_pinned: Some(Pin::File(PathBuf::from("C:/a b/x.rs"))),
            use_days: (12, 20405),
            seen: vec![
                (Pin::File(PathBuf::from("C:/a b/x.rs")), 9),
                (Pin::Untitled(2), 11),
            ],
            theme: ThemeChoice::Dark,
            files: Some(true),
            files_w: Some(300),
            results_h: Some(180),
            find_folder: "C:/some dir/x=y".into(),
            find_kinds: "*.md;*.txt;!a%b".into(),
            marks: vec![
                (PathBuf::from("C:/a b/x;y.rs"), vec![0, 7, 100]),
                (PathBuf::from("C:/z.md"), vec![3]),
            ],
            places: vec![
                (
                    PathBuf::from("C:/a b/x;y.rs"),
                    crate::places::Place {
                        line: 40,
                        col: 3,
                        top: 21,
                        lang: Some("Rust"),
                    },
                ),
                (
                    PathBuf::from("C:/z.md"),
                    crate::places::Place {
                        line: 0,
                        col: 9,
                        top: 0,
                        lang: None,
                    },
                ),
            ],
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
    fn a_pin_that_cannot_be_read_is_dropped() {
        let s = Session::parse("[mdedit]\npin=#0\npin=#x\npin=\npin=#7\n");
        assert_eq!(s.pinned, vec![Pin::Untitled(7)]);
    }

    #[test]
    fn the_window_corners_are_remembered() {
        // Round (8) is the default and is not written; the other choices are.
        assert!(!Session::default().serialize().contains("corners"));
        for c in [Corners::Square, Corners::Small] {
            let s = Session {
                corners: c,
                ..Session::default()
            };
            let text = s.serialize();
            assert!(
                text.contains(&format!("corners={}", c.to_setting())),
                "{text}"
            );
            assert_eq!(Session::parse(&text).corners, c);
        }
        // A missing, unreadable or unknown value is the default.
        assert_eq!(
            Session::parse("[mdedit]\ncorners=x\n").corners,
            Corners::Round
        );
        assert_eq!(
            Session::parse("[mdedit]\ncorners=99\n").corners,
            Corners::Round
        );
        assert_eq!(Session::parse("[mdedit]\n").corners, Corners::Round);
    }

    #[test]
    fn the_format_bar_is_on_until_turned_off() {
        assert!(Session::parse("").format_bar);
        let s = Session {
            format_bar: false,
            ..Session::default()
        };
        assert!(s.serialize().contains("format_bar=0"));
        assert!(!Session::parse(&s.serialize()).format_bar);
        assert!(!Session::default().serialize().contains("format_bar"));
    }

    #[test]
    fn the_minimap_is_remembered() {
        let s = Session {
            minimap: true,
            ..Session::default()
        };
        assert!(s.serialize().contains("minimap=1"));
        assert!(Session::parse(&s.serialize()).minimap);
        // Off is the default and is not written.
        // (The preferences section has a key of that name: only the session's own part counts.)
        let own = Session::default().serialize();
        assert!(
            !own.split("[preferences]")
                .next()
                .unwrap()
                .contains("minimap")
        );
        assert!(
            !Session::parse(
                "[mdedit]
minimap=0
"
            )
            .minimap
        );
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
                draft: Some(0xfedc_ba98_7654_3210),
            },
            Unsaved {
                file: Unsaved::file_name(12),
                untitled: 3,
                path: None,
                draft: None,
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
        assert_eq!(Session::parse("[mdedit]\nresults_h=5\n").results_h, None);
        assert_eq!(
            Session::parse("[mdedit]\nresults_h=200\n").results_h,
            Some(200)
        );
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

    #[test]
    fn a_session_that_cannot_be_read_is_an_error_not_an_empty_session() {
        let dir = std::env::temp_dir().join(format!("mdedit-session-read-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("session.ini");
        // None yet: an empty session, no error.
        assert!(Session::read(&path).unwrap().tabs.is_empty());
        let s = Session {
            tabs: vec![dir.join("a.txt")],
            ..Default::default()
        };
        std::fs::write(&path, s.serialize()).unwrap();
        assert_eq!(Session::read(&path).unwrap().tabs.len(), 1);
        // Held by another program: an error, after trying for a while.
        let Some(lock) = crate::recovery::Locked::new(&path) else {
            return;
        };
        assert!(Session::read(&path).is_err());
        let t0 = std::time::Instant::now();
        assert!(Session::read_patiently(&path).is_err());
        assert!(t0.elapsed() >= std::time::Duration::from_millis(1500));
        // Let go while it is being tried: it is read.
        let path2 = path.clone();
        let reader = std::thread::spawn(move || Session::read_patiently(&path2));
        std::thread::sleep(std::time::Duration::from_millis(300));
        drop(lock);
        assert_eq!(reader.join().unwrap().unwrap().tabs.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
