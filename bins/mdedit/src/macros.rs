//! Macros: the keys pressed while one is recorded, played back through the code that handled
//! them the first time. A step is a key with its modifiers and the text it typed, so a macro
//! holds whatever the keyboard did to the text and the find box: typing, caret moves, the
//! editing commands, a search. What the mouse did and what was chosen from a menu is not in it.
//! There is one macro, kept until another is recorded or the editor closes.

use crate::commands::CommandId as C;
use crate::keys::Key;
use mdshared_ui::keys::Mods;

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

#[derive(Default)]
pub struct Macros {
    /// The macro that Play runs.
    pub steps: Vec<Step>,
    /// The keys of the macro being recorded.
    pub recording: Option<Vec<Step>>,
    pub playing: bool,
}

impl Macros {
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
            self.steps = steps;
        }
        n
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
}
