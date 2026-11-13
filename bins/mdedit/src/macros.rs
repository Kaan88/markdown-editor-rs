//! Macros: the keys pressed while one is recorded, played back through the code that handled
//! them the first time. A step is a key with its modifiers and the text it typed, so a macro
//! holds whatever the keyboard did to the text and the find box: typing, caret moves, the
//! editing commands, a search. What the mouse did and what was chosen from a menu is not in it.
//! The last macro, and up to `SAVED_MAX` named ones, are kept in `macros.txt` beside the settings
//! file: plain text, one step per line, read when first needed and written when one changes.

use crate::commands::CommandId as C;
use crate::keys::Key;
use mdshared_ui::keys::{KeyName, Mods};
use std::path::PathBuf;

/// Keys a macro holds; those pressed after that are left out.
pub const STEPS_MAX: usize = 10_000;
/// Runs of "play to the end of the file" at most.
pub const RUNS_MAX: usize = 100_000;

#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    pub key: Key,
    pub mods: Mods,
    pub typed: Option<String>,
}

/// Named macros kept; the least recently used goes when a new one comes.
pub const SAVED_MAX: usize = 20;
/// A name has at most this many characters.
pub const NAME_MAX: usize = 64;
/// The file is read up to this many bytes; a larger one is ignored.
pub const FILE_MAX: usize = 4 << 20;
/// The text one step typed, at most this many bytes.
pub const TYPED_MAX: usize = 4096;
const HEADER: &str = "mdedit-macros 1";

#[derive(Clone, Debug, PartialEq)]
pub struct Named {
    pub name: String,
    pub steps: Vec<Step>,
}

#[derive(Default)]
pub struct Macros {
    /// The macro that Play runs: the last one recorded or played. Read through `last`, which
    /// loads the file first.
    pub steps: Vec<Step>,
    /// The saved macros, the most recently used first. Read through `saved`.
    pub named: Vec<Named>,
    /// The file; none with `--no-session`.
    pub path: Option<PathBuf>,
    loaded: bool,
    /// The keys of the macro being recorded.
    pub recording: Option<Vec<Step>>,
    pub playing: bool,
}

impl Macros {
    /// Reads the file once, the first time a macro is wanted: starting the editor does not
    /// touch it. What was recorded before that (nothing can be) is kept.
    fn ensure_loaded(&mut self) {
        if self.loaded {
            return;
        }
        self.loaded = true;
        let Some(path) = self.path.as_deref() else {
            return;
        };
        let store = Store::load(path);
        if self.steps.is_empty() {
            self.steps = store.last;
        }
        self.named = store.saved;
    }

    /// Three saved macros in memory (`--demo-macros`, for shots); no file is read or written.
    pub fn demo(&mut self) {
        self.loaded = true;
        let typed = |t: &str| Step {
            key: Key::Name(KeyName::Char(t.chars().next().unwrap_or('a'))),
            mods: Mods::default(),
            typed: Some(t.to_string()),
        };
        let key = |k: KeyName| Step {
            key: Key::Name(k),
            mods: Mods::default(),
            typed: None,
        };
        self.steps = vec![key(KeyName::Home), typed("- "), key(KeyName::Down)];
        self.named = vec![
            Named {
                name: "Bullet the line".into(),
                steps: self.steps.clone(),
            },
            Named {
                name: "Add a semicolon".into(),
                steps: vec![key(KeyName::End), typed(";")],
            },
            Named {
                name: "Wrap in quotes".into(),
                steps: vec![
                    key(KeyName::Home),
                    typed("\""),
                    key(KeyName::End),
                    typed("\""),
                    key(KeyName::Down),
                ],
            },
        ];
    }

    /// The macro Play runs.
    pub fn last(&mut self) -> &[Step] {
        self.ensure_loaded();
        &self.steps
    }

    /// The saved macros, most recently used first.
    pub fn saved(&mut self) -> &[Named] {
        self.ensure_loaded();
        &self.named
    }

    fn persist(&mut self) {
        let Some(path) = self.path.clone() else {
            return;
        };
        let store = Store {
            last: self.steps.clone(),
            saved: self.named.clone(),
        };
        if let Err(e) = store.save(&path) {
            eprintln!("mdedit: could not save the macros: {e}");
        }
    }

    /// Keeps the macro Play runs under `name` (after trimming it), as the most recently used. A
    /// name already taken is replaced. False for a name that is empty or has control characters,
    /// or without a macro.
    pub fn save_as(&mut self, name: &str) -> bool {
        self.ensure_loaded();
        let Some(name) = clean_name(name) else {
            return false;
        };
        if self.steps.is_empty() {
            return false;
        }
        self.named.retain(|n| n.name != name);
        self.named.insert(
            0,
            Named {
                name,
                steps: self.steps.clone(),
            },
        );
        self.named.truncate(SAVED_MAX);
        self.persist();
        true
    }

    /// Makes saved macro `i` the one Play runs, and the most recently used.
    pub fn use_saved(&mut self, i: usize) -> bool {
        self.ensure_loaded();
        if i >= self.named.len() {
            return false;
        }
        let n = self.named.remove(i);
        self.steps = n.steps.clone();
        self.named.insert(0, n);
        self.persist();
        true
    }

    /// Deletes saved macro `i` (the last macro stays).
    pub fn delete_saved(&mut self, i: usize) -> bool {
        self.ensure_loaded();
        if i >= self.named.len() {
            return false;
        }
        self.named.remove(i);
        self.persist();
        true
    }

    /// Deletes every saved macro.
    pub fn delete_all_saved(&mut self) {
        self.ensure_loaded();
        self.named.clear();
        self.persist();
    }

    /// Takes a key in while recording, unless it is bound to a command a macro leaves out.
    pub fn note(&mut self, key: &Key, mods: Mods, typed: Option<&str>, command: Option<C>) {
        if self.playing || !command.is_none_or(recordable) {
            return;
        }
        if let Some(steps) = self.recording.as_mut().filter(|s| s.len() < STEPS_MAX) {
            steps.push(Step {
                key: key.clone(),
                mods,
                typed: typed.map(str::to_string),
            });
        }
    }

    /// Ends the recording: the keys taken become the macro, unless there are none, in which
    /// case the macro there was stays. The number of keys taken.
    pub fn finish(&mut self) -> usize {
        let steps = self.recording.take().unwrap_or_default();
        let n = steps.len();
        if n > 0 {
            self.ensure_loaded();
            self.steps = steps;
            self.persist();
        }
        n
    }
}

// ---- the file --------------------------------------------------------------------------------

/// A name as it is kept: trimmed, at most `NAME_MAX` characters, no control characters.
pub fn clean_name(name: &str) -> Option<String> {
    let name = name.trim();
    (!name.is_empty() && name.chars().count() <= NAME_MAX && !name.chars().any(char::is_control))
        .then(|| name.to_string())
}

/// What `macros.txt` holds.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Store {
    pub last: Vec<Step>,
    pub saved: Vec<Named>,
}

/// Bytes outside `[A-Za-z0-9_.-]` as `%XX`, so a name or typed text is one word on one line.
fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// The text `encode` made; `None` for a bad escape or bytes that are not UTF-8.
fn decode(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let hex = s.get(i + 1..i + 3)?;
            out.push(
                u8::from_str_radix(hex, 16)
                    .ok()
                    .filter(|_| hex.bytes().all(|h| h.is_ascii_hexdigit()))?,
            );
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

fn key_token(key: &Key) -> String {
    match key {
        Key::Insert => "insert".into(),
        Key::Name(n) => match n {
            KeyName::Char(c) => format!("c{:x}", *c as u32),
            KeyName::F(n) => format!("f{n}"),
            KeyName::Tab => "tab".into(),
            KeyName::Escape => "escape".into(),
            KeyName::Enter => "enter".into(),
            KeyName::Space => "space".into(),
            KeyName::Backspace => "backspace".into(),
            KeyName::Delete => "delete".into(),
            KeyName::Home => "home".into(),
            KeyName::End => "end".into(),
            KeyName::PageUp => "pageup".into(),
            KeyName::PageDown => "pagedown".into(),
            KeyName::Up => "up".into(),
            KeyName::Down => "down".into(),
            KeyName::Left => "left".into(),
            KeyName::Right => "right".into(),
        },
    }
}

fn key_from(token: &str) -> Option<Key> {
    let name = match token {
        "insert" => return Some(Key::Insert),
        "tab" => KeyName::Tab,
        "escape" => KeyName::Escape,
        "enter" => KeyName::Enter,
        "space" => KeyName::Space,
        "backspace" => KeyName::Backspace,
        "delete" => KeyName::Delete,
        "home" => KeyName::Home,
        "end" => KeyName::End,
        "pageup" => KeyName::PageUp,
        "pagedown" => KeyName::PageDown,
        "up" => KeyName::Up,
        "down" => KeyName::Down,
        "left" => KeyName::Left,
        "right" => KeyName::Right,
        t => {
            if let Some(n) = t.strip_prefix('f') {
                let n: u8 = n.parse().ok().filter(|n| (1..=24).contains(n))?;
                KeyName::F(n)
            } else {
                let hex = t.strip_prefix('c')?;
                if hex.is_empty() || hex.len() > 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return None;
                }
                let c = char::from_u32(u32::from_str_radix(hex, 16).ok()?)?;
                if c.is_control() {
                    return None;
                }
                KeyName::Char(c)
            }
        }
    };
    Some(Key::Name(name))
}

fn step_line(step: &Step) -> String {
    let mut mods = String::new();
    for (on, c) in [
        (step.mods.ctrl, 'c'),
        (step.mods.shift, 's'),
        (step.mods.alt, 'a'),
    ] {
        if on {
            mods.push(c);
        }
    }
    if mods.is_empty() {
        mods.push('-');
    }
    let mut line = format!("k {mods} {}", key_token(&step.key));
    if let Some(t) = &step.typed {
        line.push_str(" t:");
        line.push_str(&encode(t));
    }
    line
}

fn parse_step(line: &str) -> Option<Step> {
    let mut words = line.strip_prefix("k ")?.split(' ');
    let mods = match words.next()? {
        "-" => Mods::default(),
        m => {
            let mut mods = Mods::default();
            let mut last = 0;
            for c in m.chars() {
                let (rank, flag) = match c {
                    'c' => (1, &mut mods.ctrl),
                    's' => (2, &mut mods.shift),
                    'a' => (3, &mut mods.alt),
                    _ => return None,
                };
                if rank <= last {
                    return None;
                }
                last = rank;
                *flag = true;
            }
            mods
        }
    };
    let key = key_from(words.next()?)?;
    let typed = match words.next() {
        None => None,
        Some(t) => {
            let text = decode(t.strip_prefix("t:")?)?;
            (text.len() <= TYPED_MAX).then_some(text).map(Some)?
        }
    };
    words.next().is_none().then_some(Step { key, mods, typed })
}

impl Store {
    /// Reads the file; a missing, oversized or damaged one gives nothing. A macro with a bad
    /// line is dropped whole (replaying half of one would do something nobody recorded).
    pub fn load(path: &std::path::Path) -> Store {
        use std::io::Read;
        let Ok(file) = std::fs::File::open(path) else {
            return Store::default();
        };
        let mut bytes = Vec::new();
        if file
            .take(FILE_MAX as u64 + 1)
            .read_to_end(&mut bytes)
            .is_err()
        {
            return Store::default();
        }
        Store::parse(&bytes)
    }

    pub fn parse(bytes: &[u8]) -> Store {
        let Some(text) = (bytes.len() <= FILE_MAX)
            .then(|| std::str::from_utf8(bytes).ok())
            .flatten()
        else {
            return Store::default();
        };
        let mut lines = text.lines();
        if lines.next().map(str::trim_end) != Some(HEADER) {
            return Store::default();
        }
        // The macro being read: None before the first section.
        enum At {
            Nowhere,
            Last,
            Named(String),
        }
        let mut store = Store::default();
        let mut at = At::Nowhere;
        let mut steps: Vec<Step> = Vec::new();
        let mut bad = false;
        let close = |at: &mut At, steps: &mut Vec<Step>, bad: &mut bool, store: &mut Store| {
            let done = std::mem::replace(at, At::Nowhere);
            let steps = std::mem::take(steps);
            if !std::mem::take(bad) && !steps.is_empty() {
                match done {
                    At::Nowhere => {}
                    At::Last => store.last = steps,
                    At::Named(name) => {
                        if store.saved.len() < SAVED_MAX
                            && !store.saved.iter().any(|n| n.name == name)
                        {
                            store.saved.push(Named { name, steps });
                        }
                    }
                }
            }
        };
        for line in lines {
            let line = line.trim_end();
            if line == "last" || line.starts_with("m ") {
                close(&mut at, &mut steps, &mut bad, &mut store);
                at = if line == "last" {
                    At::Last
                } else {
                    match decode(&line[2..]).and_then(|n| clean_name(&n)) {
                        Some(n) => At::Named(n),
                        None => {
                            bad = true;
                            At::Nowhere
                        }
                    }
                };
            } else if line.starts_with("k ") {
                if matches!(at, At::Nowhere) {
                    continue;
                }
                match parse_step(line) {
                    Some(step) if steps.len() < STEPS_MAX => steps.push(step),
                    _ => bad = true,
                }
            }
            // Anything else (blank lines, notes, lines of a later version) is skipped.
        }
        close(&mut at, &mut steps, &mut bad, &mut store);
        store
    }

    /// The file's text. The oldest saved macros are left out if the whole would pass `FILE_MAX`.
    pub fn serialize(&self) -> String {
        let mut out = String::from(HEADER);
        out.push('\n');
        let section = |head: String, steps: &[Step]| {
            let mut s = head;
            s.push('\n');
            for st in steps {
                s.push_str(&step_line(st));
                s.push('\n');
            }
            s
        };
        if !self.last.is_empty() {
            let part = section("last".into(), &self.last);
            if part.len() <= FILE_MAX / 2 {
                out.push_str(&part);
            }
        }
        for n in self.saved.iter().take(SAVED_MAX) {
            let part = section(format!("m {}", encode(&n.name)), &n.steps);
            if out.len() + part.len() > FILE_MAX {
                break;
            }
            out.push_str(&part);
        }
        out
    }

    pub fn save(&self, path: &std::path::Path) -> std::io::Result<()> {
        mdshared_app::settings::write_atomic(path, &self.serialize())
    }
}

/// Commands a macro can hold: those that work on the text, the caret and the search. Left out
/// are the macro commands themselves and those that open a dialog, change the tabs or the
/// files, or only change how things look.
pub fn recordable(id: C) -> bool {
    !matches!(
        id,
        C::RecordMacro
            | C::PlayMacro
            | C::PlayMacroToEnd
            | C::SaveMacroAs
            | C::DeleteMacros
            | C::NewFile
            | C::OpenFiles
            | C::OpenFolder
            | C::SaveAs
            | C::Reload
            | C::OpenRecent
            | C::ManageRecent
            | C::ClearRecent
            | C::CloseTab
            | C::CloseAllTabs
            | C::CloseOtherTabs
            | C::NextTab
            | C::PreviousTab
            | C::Exit
            | C::Theme
            | C::ThemeSystem
            | C::ThemeLight
            | C::ThemeDark
            | C::ShowShortcuts
            | C::About
            | C::CopyFilePath
            | C::ShowInFileManager
            | C::MakeDefault
            | C::ToggleSplit
            | C::FocusOther
            | C::ToggleFiles
            | C::ToggleOutline
            | C::FindInFiles
            | C::FindInTabs
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use mdshared_ui::keys::KeyName;

    #[test]
    fn the_minimap_toggle_is_recorded_as_word_wrap_is() {
        assert_eq!(recordable(C::ToggleMinimap), recordable(C::ToggleWrap));
        assert!(recordable(C::ToggleMinimap));
    }

    #[test]
    fn a_recording_takes_keys_and_becomes_the_macro() {
        let mut m = Macros::default();
        let key = Key::Name(KeyName::Char('a'));
        let none = Mods::default();
        // Nothing is taken while not recording.
        m.note(&key, none, Some("a"), None);
        assert_eq!(m.finish(), 0);
        m.recording = Some(Vec::new());
        m.note(&key, none, Some("a"), None);
        m.note(&key, none, None, Some(C::DuplicateLine));
        // Commands that are not for a macro, and keys played back, are left out.
        m.note(&key, none, None, Some(C::PlayMacro));
        m.note(&key, none, None, Some(C::CloseTab));
        m.playing = true;
        m.note(&key, none, Some("a"), None);
        m.playing = false;
        assert_eq!(m.finish(), 2);
        assert_eq!(m.steps.len(), 2);
        assert_eq!(m.steps[0].typed.as_deref(), Some("a"));
        // A recording with no keys leaves the macro as it was.
        m.recording = Some(Vec::new());
        assert_eq!(m.finish(), 0);
        assert_eq!(m.steps.len(), 2);
        // Past the cap nothing more is taken.
        m.recording = Some(vec![m.steps[0].clone(); STEPS_MAX]);
        m.note(&key, none, Some("a"), None);
        assert_eq!(m.finish(), STEPS_MAX);
    }

    fn typed(c: char) -> Step {
        Step {
            key: Key::Name(KeyName::Char(c)),
            mods: Mods::default(),
            typed: Some(c.to_string()),
        }
    }

    fn sample() -> Store {
        let all_keys = [
            KeyName::Char('a'),
            KeyName::Char('é'),
            KeyName::Char('\u{1F600}'),
            KeyName::F(5),
            KeyName::Tab,
            KeyName::Escape,
            KeyName::Enter,
            KeyName::Space,
            KeyName::Backspace,
            KeyName::Delete,
            KeyName::Home,
            KeyName::End,
            KeyName::PageUp,
            KeyName::PageDown,
            KeyName::Up,
            KeyName::Down,
            KeyName::Left,
            KeyName::Right,
        ];
        let mut steps: Vec<Step> = all_keys
            .into_iter()
            .map(|k| Step {
                key: Key::Name(k),
                mods: Mods {
                    ctrl: true,
                    shift: false,
                    alt: true,
                },
                typed: None,
            })
            .collect();
        steps.push(Step {
            key: Key::Insert,
            mods: Mods {
                ctrl: true,
                shift: true,
                alt: true,
            },
            typed: None,
        });
        steps.push(Step {
            key: Key::Name(KeyName::Enter),
            mods: Mods::default(),
            typed: Some("line one\n\tçay 100%\r ".into()),
        });
        steps.push(Step {
            key: Key::Name(KeyName::Char('x')),
            mods: Mods::default(),
            typed: Some(String::new()),
        });
        Store {
            last: steps.clone(),
            saved: vec![
                Named {
                    name: "Bullet the line".into(),
                    steps: vec![typed('-'), typed(' ')],
                },
                Named {
                    name: "çay % 100 \u{1F600}".into(),
                    steps,
                },
            ],
        }
    }

    #[test]
    fn the_file_round_trips_every_kind_of_step() {
        let store = sample();
        let text = store.serialize();
        assert!(text.starts_with("mdedit-macros 1\n"));
        // One step a line, plain text, no raw control characters inside a step.
        assert!(text.lines().all(|l| !l.contains('\t') && !l.contains('\r')));
        assert_eq!(Store::parse(text.as_bytes()), store);
        // Nothing saved is an empty file with a header, which reads back as nothing.
        assert_eq!(
            Store::parse(Store::default().serialize().as_bytes()),
            Store::default()
        );
    }

    #[test]
    fn a_bad_file_is_ignored_never_a_panic() {
        let good = sample().serialize();
        // Wrong or missing header, other versions, binary, not UTF-8, empty.
        for bad in [
            "",
            "mdedit-macros 2\nlast\nk - a\n",
            "last\nk - c61\n",
            "\u{feff}mdedit-macros 1\n",
        ] {
            assert_eq!(Store::parse(bad.as_bytes()), Store::default(), "{bad:?}");
        }
        assert_eq!(Store::parse(&[0xFF, 0xFE, 0, 1, 2]), Store::default());
        assert_eq!(
            Store::parse(b"mdedit-macros 1\nm \xFF\xFE\nk - up\n"),
            Store::default()
        );
        // A macro with one bad line is dropped whole; the ones around it are kept.
        let mixed = "mdedit-macros 1\nm one\nk - up\nm two\nk - up\nk zz c61\nk - down\nm three\nk - down\n";
        let got = Store::parse(mixed.as_bytes());
        let names: Vec<_> = got.saved.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(names, ["one", "three"]);
        // Each way a step can be wrong.
        for line in [
            "k - ",
            "k - nokey",
            "k - c",
            "k - cZZ",
            "k - c110000",
            "k - c1234567",
            "k - c0",
            "k - c1f",
            "k sc up",
            "k cc up",
            "k x up",
            "k - f0",
            "k - f99",
            "k - up t:%zz",
            "k - up t:%4",
            "k - up t:%FF",
            "k - up u:abc",
            "k - up t:a extra",
            "k",
        ] {
            let text = format!("mdedit-macros 1\nlast\n{line}\n");
            assert_eq!(Store::parse(text.as_bytes()).last, Vec::new(), "{line:?}");
        }
        // Steps before any section, unknown lines and blank lines are skipped.
        let odd = "mdedit-macros 1\nk - up\n\nnote: hi\nlast\nk - up\nfuture thing\n";
        assert_eq!(Store::parse(odd.as_bytes()).last.len(), 1);
        // A damaged copy of a good file never panics, whatever byte is cut or changed.
        for cut in 0..good.len().min(400) {
            let _ = Store::parse(&good.as_bytes()[..cut]);
            let mut b = good.clone().into_bytes();
            b[cut] = b'%';
            let _ = Store::parse(&b);
        }
    }

    #[test]
    fn limits_hold_on_read_and_write() {
        // Names: 1 to 64 characters, no control characters, trimmed.
        assert_eq!(clean_name("  a b  ").as_deref(), Some("a b"));
        assert_eq!(clean_name(""), None);
        assert_eq!(clean_name("   "), None);
        assert_eq!(clean_name("a\nb"), None);
        assert!(clean_name(&"x".repeat(NAME_MAX)).is_some());
        assert_eq!(clean_name(&"x".repeat(NAME_MAX + 1)), None);
        assert!(clean_name(&"é".repeat(NAME_MAX)).is_some());
        // At most SAVED_MAX macros are read, and a name is kept once (the first).
        let mut text = String::from("mdedit-macros 1\n");
        for i in 0..SAVED_MAX + 10 {
            text.push_str(&format!("m n{i}\nk - up\n"));
        }
        text.push_str("m n0\nk - down\n");
        let got = Store::parse(text.as_bytes());
        assert_eq!(got.saved.len(), SAVED_MAX);
        assert_eq!(got.saved[0].steps[0].key, Key::Name(KeyName::Up));
        // At most STEPS_MAX steps in a macro: one more drops the macro.
        let mut long = String::from("mdedit-macros 1\nlast\n");
        long.push_str(&"k - up\n".repeat(STEPS_MAX));
        assert_eq!(Store::parse(long.as_bytes()).last.len(), STEPS_MAX);
        long.push_str("k - up\n");
        assert!(Store::parse(long.as_bytes()).last.is_empty());
        // Typed text over TYPED_MAX bytes is refused.
        let big = format!(
            "mdedit-macros 1\nlast\nk - up t:{}\n",
            "a".repeat(TYPED_MAX + 1)
        );
        assert!(Store::parse(big.as_bytes()).last.is_empty());
        let fits = format!(
            "mdedit-macros 1\nlast\nk - up t:{}\n",
            "a".repeat(TYPED_MAX)
        );
        assert_eq!(Store::parse(fits.as_bytes()).last.len(), 1);
        // A file over FILE_MAX is ignored whole.
        let mut huge = String::from("mdedit-macros 1\nlast\nk - up\n");
        while huge.len() <= FILE_MAX {
            huge.push_str("note\n");
        }
        assert_eq!(Store::parse(huge.as_bytes()), Store::default());
        // Writing never goes over it either: the oldest saved macros are left out.
        let step = Step {
            key: Key::Name(KeyName::Enter),
            mods: Mods::default(),
            typed: Some("é".repeat(TYPED_MAX / 2)),
        };
        let store = Store {
            last: vec![],
            saved: (0..SAVED_MAX)
                .map(|i| Named {
                    name: format!("m{i}"),
                    steps: vec![step.clone(); 250],
                })
                .collect(),
        };
        let out = store.serialize();
        assert!(out.len() <= FILE_MAX);
        let back = Store::parse(out.as_bytes());
        assert!(!back.saved.is_empty() && back.saved.len() < SAVED_MAX);
        assert_eq!(back.saved[0].name, "m0");
    }

    #[test]
    fn macros_are_saved_named_reordered_deleted_and_survive_a_restart() {
        let dir = std::env::temp_dir().join(format!("mdedit-macros-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("macros.txt");
        let a = Key::Name(KeyName::Char('a'));
        let record = |m: &mut Macros, n: usize| {
            m.recording = Some(Vec::new());
            for _ in 0..n {
                m.note(&a, Mods::default(), Some("a"), None);
            }
            m.finish()
        };
        let mut m = Macros {
            path: Some(path.clone()),
            ..Macros::default()
        };
        // Nothing is read until a macro is wanted, and no file is made by asking.
        assert!(!m.loaded && !path.exists());
        assert!(m.last().is_empty() && m.saved().is_empty());
        assert!(!path.exists());
        // After recording the macro is the last one, and is kept at once.
        assert_eq!(record(&mut m, 2), 2);
        assert!(path.exists());
        assert!(m.save_as(" two keys "));
        assert!(!m.save_as("   ") && !m.save_as("a\tb"));
        assert_eq!(record(&mut m, 3), 3);
        assert!(m.save_as("three"));
        assert_eq!(
            m.saved()
                .iter()
                .map(|n| n.name.as_str())
                .collect::<Vec<_>>(),
            ["three", "two keys"]
        );
        // A name used again replaces that macro and moves it first.
        assert!(m.save_as("two keys"));
        assert_eq!(m.saved().len(), 2);
        assert_eq!(m.saved()[0].name, "two keys");
        assert_eq!(m.saved()[0].steps.len(), 3);
        // Playing a saved one makes it the last one and the most recently used.
        assert!(m.use_saved(1));
        assert_eq!(m.saved()[0].name, "three");
        assert_eq!(m.last().len(), 3);
        // A new process reads it all back.
        let mut again = Macros {
            path: Some(path.clone()),
            ..Macros::default()
        };
        assert_eq!(again.last().len(), 3);
        assert_eq!(again.saved(), m.saved());
        // More than SAVED_MAX: the least recently used go.
        for i in 0..SAVED_MAX + 5 {
            assert!(again.save_as(&format!("m{i}")));
        }
        assert_eq!(again.saved().len(), SAVED_MAX);
        assert_eq!(again.saved()[0].name, format!("m{}", SAVED_MAX + 4));
        // Deleting one, then all; the last macro stays.
        assert!(again.delete_saved(0) && !again.delete_saved(99));
        assert_eq!(again.saved().len(), SAVED_MAX - 1);
        again.delete_all_saved();
        let mut third = Macros {
            path: Some(path.clone()),
            ..Macros::default()
        };
        assert!(third.saved().is_empty() && third.last().len() == 3);
        // A damaged file is ignored, and a recording then writes a good one over it.
        std::fs::write(&path, b"garbage\xFF").unwrap();
        let mut fourth = Macros {
            path: Some(path.clone()),
            ..Macros::default()
        };
        assert!(fourth.last().is_empty() && fourth.saved().is_empty());
        assert_eq!(record(&mut fourth, 1), 1);
        assert_eq!(Store::load(&path).last.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
