//! Shortcut lookup over the command table, with `mdshared_ui::keys` for parsing. That parser has
//! no Insert key, which Ctrl+Insert (Copy) needs, so a key here is either one of its names or
//! Insert.

use crate::commands::{COMMANDS, CommandId};
pub use mdshared_ui::keys::{KeyName, Mods};
use winit::keyboard::{Key as WKey, NamedKey};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Key {
    Name(KeyName),
    Insert,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chord {
    pub mods: Mods,
    pub key: Key,
}

/// The key a winit key stands for.
pub fn key_of(key: &WKey) -> Option<Key> {
    if let WKey::Named(NamedKey::Insert) = key {
        return Some(Key::Insert);
    }
    mdshared_ui::keys::key_name(key).map(Key::Name)
}

/// Parses "Ctrl+Shift+F", "F5", "Ctrl+Insert".
pub fn parse(s: &str) -> Option<Chord> {
    if let Some(c) = mdshared_ui::keys::parse(s) {
        return Some(Chord {
            mods: c.mods,
            key: Key::Name(c.key),
        });
    }
    // Only Insert is left to read: strip the modifiers the same way.
    let mut mods = Mods::default();
    let mut rest = s.trim();
    loop {
        let lower = rest.to_ascii_lowercase();
        if let Some(r) = lower.strip_prefix("ctrl+") {
            mods.ctrl = true;
            rest = &rest[rest.len() - r.len()..];
        } else if let Some(r) = lower.strip_prefix("shift+") {
            mods.shift = true;
            rest = &rest[rest.len() - r.len()..];
        } else if let Some(r) = lower.strip_prefix("alt+") {
            mods.alt = true;
            rest = &rest[rest.len() - r.len()..];
        } else {
            break;
        }
    }
    matches!(rest.to_ascii_lowercase().as_str(), "insert" | "ins").then_some(Chord {
        mods,
        key: Key::Insert,
    })
}

impl Chord {
    pub fn matches(&self, key: &Key, mods: Mods) -> bool {
        match (&self.key, key) {
            (Key::Name(a), Key::Name(b)) => mdshared_ui::keys::Chord {
                mods: self.mods,
                key: a.clone(),
            }
            .matches(b, mods),
            (Key::Insert, Key::Insert) => self.mods == mods,
            _ => false,
        }
    }
}

/// The command bound to a key, from the table.
pub fn command_for(key: &Key, mods: Mods) -> Option<CommandId> {
    COMMANDS.iter().find_map(|c| {
        c.keys
            .iter()
            .filter_map(|s| parse(s))
            .any(|chord| chord.matches(key, mods))
            .then_some(c.id)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const NONE: Mods = Mods {
        ctrl: false,
        shift: false,
        alt: false,
    };
    const CTRL: Mods = Mods {
        ctrl: true,
        shift: false,
        alt: false,
    };
    const CTRL_SHIFT: Mods = Mods {
        ctrl: true,
        shift: true,
        alt: false,
    };
    const SHIFT: Mods = Mods {
        ctrl: false,
        shift: true,
        alt: false,
    };

    fn n(k: KeyName) -> Key {
        Key::Name(k)
    }

    #[test]
    fn every_table_shortcut_parses() {
        for c in COMMANDS {
            for s in c.keys {
                assert!(parse(s).is_some(), "{s} for {:?}", c.id);
                // The shared parser reads all of them but Insert, which it has no name for.
                if !s.ends_with("Insert") {
                    assert!(mdshared_ui::keys::parse(s).is_some(), "{s}");
                }
                assert!(
                    !(s.contains("Ctrl+") && s.contains("Alt+")),
                    "{s} would be AltGr"
                );
            }
        }
    }

    #[test]
    fn table_lookup() {
        let c = |k: Key, m: Mods| command_for(&k, m);
        assert_eq!(c(n(KeyName::Char('o')), CTRL), Some(CommandId::OpenFiles));
        assert_eq!(
            c(n(KeyName::Char('o')), CTRL_SHIFT),
            Some(CommandId::OpenFolder)
        );
        assert_eq!(c(n(KeyName::F(5)), NONE), Some(CommandId::Reload));
        assert_eq!(c(n(KeyName::F(3)), NONE), Some(CommandId::FindNext));
        assert_eq!(c(n(KeyName::F(3)), SHIFT), Some(CommandId::FindPrevious));
        assert_eq!(c(n(KeyName::Char('w')), CTRL), Some(CommandId::CloseTab));
        assert_eq!(c(n(KeyName::F(4)), CTRL), Some(CommandId::CloseTab));
        assert_eq!(c(n(KeyName::Tab), CTRL), Some(CommandId::NextTab));
        assert_eq!(c(n(KeyName::Tab), CTRL_SHIFT), Some(CommandId::PreviousTab));
        assert_eq!(
            c(n(KeyName::Char('1')), CTRL_SHIFT),
            Some(CommandId::ToggleFiles)
        );
        assert_eq!(c(n(KeyName::Char('c')), CTRL), Some(CommandId::Copy));
        assert_eq!(c(Key::Insert, CTRL), Some(CommandId::Copy));
        assert_eq!(c(Key::Insert, NONE), None);
        assert_eq!(c(n(KeyName::Char('a')), CTRL), Some(CommandId::SelectAll));
        assert_eq!(c(n(KeyName::Char('f')), CTRL), Some(CommandId::Find));
        assert_eq!(c(n(KeyName::Char('q')), CTRL), Some(CommandId::Exit));
        assert_eq!(c(n(KeyName::Escape), NONE), Some(CommandId::Escape));
        assert_eq!(c(n(KeyName::Char('o')), NONE), None);
        assert_eq!(c(n(KeyName::Char('v')), CTRL), Some(CommandId::Paste));
        assert_eq!(c(Key::Insert, SHIFT), Some(CommandId::Paste));
        assert_eq!(c(n(KeyName::Delete), SHIFT), Some(CommandId::Cut));
        assert_eq!(c(n(KeyName::Delete), NONE), None);
        assert_eq!(c(n(KeyName::Char('s')), CTRL), Some(CommandId::Save));
        assert_eq!(
            c(n(KeyName::Char('s')), CTRL_SHIFT),
            Some(CommandId::SaveAs)
        );
        assert_eq!(c(n(KeyName::Char('z')), CTRL), Some(CommandId::Undo));
        assert_eq!(c(n(KeyName::Char('z')), CTRL_SHIFT), Some(CommandId::Redo));
        assert_eq!(c(n(KeyName::Char('y')), CTRL), Some(CommandId::Redo));
    }

    #[test]
    fn parses_insert() {
        assert_eq!(
            parse("Ctrl+Insert"),
            Some(Chord {
                mods: CTRL,
                key: Key::Insert
            })
        );
        assert_eq!(parse("Insert").map(|c| c.mods), Some(NONE));
        assert_eq!(parse("Ctrl+Bogus"), None);
    }
}
