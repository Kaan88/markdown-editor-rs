//! The "Window backdrop" setting at work: when the chrome is frosted and what the compositor was
//! asked for. The drawing itself is `mdshared_ui::frost` (the chrome is drawn as always, marked,
//! and its flat colour taken out again when the frame goes to the window).
//!
//! The chrome is frosted only while all of this holds: the setting names a backdrop, the system
//! shows one (`mdshared_ui::platform::backdrop_available`), the compositor took the request, and
//! the window has the keyboard. In every other case the chrome is its flat colours, opaque, as
//! without the feature. With the setting off nothing here runs beyond one comparison a frame.

use mdshared_app::backdrop::Backdrop;
use mdshared_ui::frost::{Key, key_colour, veil};
use mdshared_ui::theme::Palette;

use crate::ui::Region;

/// The regions that let the backdrop through: the title row with its tabs, the bar (with the
/// replace strip and the notices under it), the side pane and the status strip. The text, the
/// minimap and the results panel stay opaque; menus and dialogs are drawn over the chrome with
/// opaque fills and stay opaque by that (see `mdshared_ui::frost`).
pub fn frosted(r: Region) -> bool {
    matches!(
        r,
        Region::Title | Region::Bar | Region::Files | Region::Status
    )
}

/// What the window system is asked. The app's own is the real system; tests have a recording one.
pub trait System {
    /// Whether a backdrop shows on this system as it is set up now.
    fn available(&mut self) -> bool;
    /// Asks for `backdrop` behind the window (`Off` takes it away). Whether it was taken.
    fn set(&mut self, backdrop: Backdrop) -> bool;
}

#[derive(Debug, Default)]
pub struct Frost {
    /// The setting as of the last frame.
    wanted: Backdrop,
    /// What the compositor was last asked for and took.
    asked: Backdrop,
    /// The system's word on whether a backdrop shows, read when the setting changes and each
    /// time the window gets the keyboard back (the user may have been in the system's settings).
    available: bool,
    /// Whether the window had the keyboard at the last frame.
    focused: bool,
    /// The colour to take out of the chrome, while it is frosted.
    key: Option<Key>,
    /// The frosted regions of the last composed frame, in device px: x0, y0, x1, y1.
    pub areas: Vec<(usize, usize, usize, usize)>,
}

impl Frost {
    /// The key to take out of the marked pixels: `Some` while the chrome is frosted.
    pub fn key(&self) -> Option<&Key> {
        self.key.as_ref()
    }

    /// Whether [`Frost::sync`] has anything to do: the setting is on, or was when last looked at.
    pub fn concerns(&self, want: Backdrop) -> bool {
        want != Backdrop::Off || self.wanted != Backdrop::Off || self.asked != Backdrop::Off
    }

    /// Brings the state in line with the setting, the window's focus and the palette. Returns
    /// true when the chrome's look changed (frosted or not, or another key): the caller repaints
    /// everything and sends the whole frame.
    pub fn sync(
        &mut self,
        want: Backdrop,
        focused: bool,
        pal: &Palette,
        sys: &mut dyn System,
    ) -> bool {
        let gained = focused && !self.focused;
        let chosen = want != self.wanted;
        self.focused = focused;
        self.wanted = want;
        if want != Backdrop::Off && (chosen || gained) {
            self.available = sys.available();
        }
        let target = if self.available { want } else { Backdrop::Off };
        if target != self.asked {
            if sys.set(target) {
                self.asked = target;
            } else {
                // Refused: make sure nothing of a half-made request is left.
                if target != Backdrop::Off {
                    sys.set(Backdrop::Off);
                }
                self.asked = Backdrop::Off;
                self.available = false;
            }
        }
        let key = (self.asked != Backdrop::Off && focused)
            .then(|| Key::new(key_colour(pal), veil(self.asked)));
        let changed = key != self.key;
        self.key = key;
        if self.key.is_none() {
            self.areas.clear();
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mdshared_ui::theme::{DARK, LIGHT};

    /// Records what was asked; answers as told.
    struct Fake {
        available: bool,
        takes: bool,
        asked_available: usize,
        sets: Vec<Backdrop>,
    }

    impl Fake {
        fn new(available: bool) -> Fake {
            Fake {
                available,
                takes: true,
                asked_available: 0,
                sets: Vec::new(),
            }
        }
    }

    impl System for Fake {
        fn available(&mut self) -> bool {
            self.asked_available += 1;
            self.available
        }
        fn set(&mut self, backdrop: Backdrop) -> bool {
            self.sets.push(backdrop);
            self.takes || backdrop == Backdrop::Off
        }
    }

    #[test]
    fn off_asks_the_system_nothing() {
        let mut f = Frost::default();
        let mut sys = Fake::new(true);
        assert!(!f.concerns(Backdrop::Off));
        for focused in [false, true, false, true] {
            assert!(!f.sync(Backdrop::Off, focused, &DARK, &mut sys));
        }
        assert_eq!((sys.asked_available, sys.sets.len()), (0, 0));
        assert!(f.key().is_none());
    }

    #[test]
    fn frosted_only_with_the_keyboard() {
        let mut f = Frost::default();
        let mut sys = Fake::new(true);
        // Chosen while the window is not focused: asked for, not shown yet.
        assert!(!f.sync(Backdrop::Mica, false, &DARK, &mut sys));
        assert_eq!(sys.sets, [Backdrop::Mica]);
        assert!(f.key().is_none());
        // Focus: frosted. The same frame again: nothing changes, nothing is asked.
        assert!(f.sync(Backdrop::Mica, true, &DARK, &mut sys));
        assert!(f.key().is_some());
        let asked = sys.asked_available;
        assert!(!f.sync(Backdrop::Mica, true, &DARK, &mut sys));
        assert_eq!(sys.asked_available, asked);
        // Focus lost: flat again, and the compositor is not asked to change anything.
        f.areas.push((0, 0, 1, 1));
        assert!(f.sync(Backdrop::Mica, false, &DARK, &mut sys));
        assert!(f.key().is_none() && f.areas.is_empty());
        assert_eq!(sys.sets, [Backdrop::Mica]);
    }

    #[test]
    fn no_backdrop_on_the_system_means_flat_and_no_request() {
        let mut f = Frost::default();
        let mut sys = Fake::new(false);
        assert!(!f.sync(Backdrop::Mica, true, &DARK, &mut sys));
        assert!(f.key().is_none());
        assert!(sys.sets.is_empty(), "the window is never made transparent");
        // Still asked about again when the window gets the keyboard back, and taken up then.
        f.sync(Backdrop::Mica, false, &DARK, &mut sys);
        sys.available = true;
        assert!(f.sync(Backdrop::Mica, true, &DARK, &mut sys));
        assert_eq!(sys.sets, [Backdrop::Mica]);
        // Transparency switched off in the system's settings meanwhile: taken away again.
        f.sync(Backdrop::Mica, false, &DARK, &mut sys);
        sys.available = false;
        assert!(!f.sync(Backdrop::Mica, true, &DARK, &mut sys));
        assert!(f.key().is_none());
        assert_eq!(sys.sets, [Backdrop::Mica, Backdrop::Off]);
    }

    #[test]
    fn a_refused_request_is_undone_and_stays_flat() {
        let mut f = Frost::default();
        let mut sys = Fake::new(true);
        sys.takes = false;
        assert!(!f.sync(Backdrop::Acrylic, true, &DARK, &mut sys));
        assert!(f.key().is_none());
        assert_eq!(sys.sets, [Backdrop::Acrylic, Backdrop::Off]);
        // Not asked again every frame.
        assert!(!f.sync(Backdrop::Acrylic, true, &DARK, &mut sys));
        assert_eq!(sys.sets.len(), 2);
    }

    #[test]
    fn the_setting_and_the_palette_change_the_look() {
        let mut f = Frost::default();
        let mut sys = Fake::new(true);
        assert!(f.sync(Backdrop::Mica, true, &DARK, &mut sys));
        assert!(
            f.sync(Backdrop::Mica, true, &LIGHT, &mut sys),
            "another key"
        );
        assert!(f.sync(Backdrop::Acrylic, true, &LIGHT, &mut sys), "a veil");
        assert!(f.sync(Backdrop::Off, true, &LIGHT, &mut sys));
        assert_eq!(sys.sets, [Backdrop::Mica, Backdrop::Acrylic, Backdrop::Off]);
        assert!(f.key().is_none());
        // Off and settled: of no concern any more.
        assert!(!f.concerns(Backdrop::Off));
    }

    #[test]
    fn the_text_and_what_belongs_to_it_stay_opaque() {
        use crate::ui::REGIONS;
        let frosted: Vec<Region> = REGIONS.iter().copied().filter(|r| frosted(*r)).collect();
        assert_eq!(
            frosted,
            [Region::Title, Region::Bar, Region::Files, Region::Status]
        );
    }
}
