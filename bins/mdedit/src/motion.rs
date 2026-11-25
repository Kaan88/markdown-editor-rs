//! Motion: the window's one animator, what decides whether anything moves, and the menus' fade
//! as they open.
//!
//! The animator is asked while a frame is composed (`App::compose` begins and ends its frame);
//! the deadline its `end_frame` gives is kept in [`Motion::due`], which `App::timers` turns into
//! a wake-up and clears when it fires. Nothing moving means no deadline: an idle window sleeps.
//!
//! Whether anything moves is decided in [`Motion::policy`] alone: never in a shot or a test
//! (time must not change a picture), never with the "Animations" setting off, and otherwise as
//! the system asks ([`mdshared_ui::platform::motion_reduced`]).
//!
//! A menu, context menu or submenu fades in and drops into place as it opens ([`MENU`]); closing
//! is instant, and the word completion list is never animated (it follows the typing). While a
//! menu fades, the frames that only move it put back the pixels that were under the menus
//! ([`Motion::backing`]) and draw the menus again over them: only the menus' bounds are drawn
//! and presented.

use crate::ui::{Popup, PopupKind};
use mdshared_ui::anim::{Animator, Clock, ManualClock, Policy, Snapshot, document, fluent, pop};
use mdshared_ui::gfx::{Painter, Rect};
use mdshared_ui::menu::MENU_DROP;
use mdshared_ui::theme::Palette;
use std::time::{Duration, Instant};

/// How long a menu takes to enter. The shared preset's 167 ms made quicker at the owner's word,
/// on mdview's fade (2026-11-25): "slightly faster", and at 117 ms "this is better".
pub const MENU_IN: Duration = Duration::from_millis(117);

/// How a menu enters and leaves: the one place its timing is chosen. The entrance is the shared
/// preset's decelerating curve over [`MENU_IN`]; the exit is the preset's and unused (closing is
/// instant).
pub const MENU: document::Pair = document::Pair {
    enter: mdshared_ui::anim::Motion::tween(MENU_IN, fluent::DECELERATE),
    exit: document::MENU.exit,
};

/// The damage group of the open menus: what is asked for while they are drawn.
pub const MENUS: &str = "menus";

/// The presence of an open menu: the popup's serial and how far it has entered (1 at rest).
pub type Shown = (u64, f32);

/// What a frame shows of the open menus: the menu and its submenu, each with its presence.
/// `None` for no menu, and for the completion list, which never fades.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Menus {
    pub main: Shown,
    pub sub: Option<Shown>,
}

impl Menus {
    /// Whether either menu is still entering.
    pub fn moving(&self) -> bool {
        self.main.1 < 1.0 || self.sub.is_some_and(|s| s.1 < 1.0)
    }
}

/// How a frame draws the open menus.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MenuFrame {
    /// As every overlay is drawn: with the regions under them, when they changed.
    Usual,
    /// Only the menus moved: their backing is put back and they are drawn over it.
    Alone,
    /// The menus moved and something else changed too: everything under the overlays is drawn
    /// again, the menus with it.
    Whole,
}

pub struct Motion {
    pub anim: Animator,
    /// When the animator next needs a frame; cleared by the timer that wakes for it.
    pub due: Option<Instant>,
    /// A shot or a test: nothing moves.
    still: bool,
    /// The "Animations" setting.
    setting: bool,
    /// Whether the system's wish is read and watched (from the first menu that could fade, so
    /// nothing of it is on the launch path).
    watching: bool,
    /// A policy that overrides the shot, the setting and the system: a shot of a menu half way
    /// in (`--menu-fade-ms`) animates whatever the system says.
    forced: Option<Policy>,
    /// What reads the system's wish for less motion: the platform's, or a test's.
    reader: fn() -> bool,
    /// The pixels under the open menus as the frame drew them before the menus, while they
    /// move, and the logical rectangle they cover.
    pub backing: Option<(Snapshot, Rect)>,
    /// What the screen shows of the menus.
    pub drawn: Option<Menus>,
    /// A clock moved by hand: a test's, or a shot's half way fade. Real time without one.
    pub clock: Option<ManualClock>,
}

/// Whether a window made from `args` holds still: a shot is drawn once and must not depend on
/// time. (Every test holds still too, `cfg(test)` at the call.)
pub fn still_for(args: &crate::args::Args) -> bool {
    args.shot.is_some()
}

/// The longest half way fade a shot draws, in ms: past the entrance everything is at rest.
pub const SHOT_FADE_MAX_MS: u32 = 1000;
/// The step of a shot's clock: a frame, never more than the animator's longest step (50 ms).
const SHOT_STEP_MS: u64 = 16;

/// The steps a shot's clock takes to reach `ms` (capped at [`SHOT_FADE_MAX_MS`]), each of at
/// most a frame: the animator takes a longer step for a stall and moves only 50 ms for it.
pub fn shot_steps(ms: u32) -> impl Iterator<Item = u64> {
    let total = u64::from(ms.min(SHOT_FADE_MAX_MS));
    (0..total.div_ceil(SHOT_STEP_MS)).map(move |k| (total - k * SHOT_STEP_MS).min(SHOT_STEP_MS))
}

impl Motion {
    /// `still` for a shot and a test: nothing moves, whatever the setting and the system say.
    pub fn new(still: bool) -> Motion {
        let mut m = Motion {
            anim: Animator::new(),
            due: None,
            still,
            setting: true,
            watching: false,
            forced: None,
            reader: crate::platform::motion_reduced,
            backing: None,
            drawn: None,
            clock: None,
        };
        m.anim.set_policy(m.policy());
        m
    }

    /// Whether anything moves, by the shot, the setting and the system: the one place that
    /// decides. `Never` makes every value its target at once and asks for no frame.
    pub fn policy(&self) -> Policy {
        if let Some(p) = self.forced {
            return p;
        }
        if self.still || !self.setting {
            Policy::Never
        } else {
            Policy::System
        }
    }

    /// Follows the "Animations" setting.
    pub fn set_setting(&mut self, on: bool) {
        if self.setting != on {
            self.setting = on;
            self.anim.set_policy(self.policy());
        }
    }

    /// The system asks for less motion (true) or not.
    pub fn set_reduced(&mut self, reduced: bool) {
        self.anim.set_system_reduced(reduced);
    }

    /// Reads the system's wish, and starts watching it the first time: called when a menu that
    /// could fade opens, never at launch. `wake` is called when the wish changes.
    pub fn watch(&mut self, wake: impl Fn() + Send + Sync + 'static) {
        if self.watching || self.policy() == Policy::Never {
            return;
        }
        self.watching = true;
        self.set_reduced((self.reader)());
        // The watch makes its hidden window and waits for it (up to 5 s on Windows): on a short
        // helper thread, never on this one. A thread that cannot start leaves the wish as read.
        let _ = std::thread::Builder::new()
            .name("mdedit-motion-watch".into())
            .spawn(move || crate::platform::watch_motion_reduced(wake));
    }

    /// Reads the system's wish again, if it is watched (on a wake, or when the window gains the
    /// focus: macOS has nothing to watch).
    pub fn refresh(&mut self) {
        if self.watching {
            self.set_reduced((self.reader)());
        }
    }

    /// Whether the system's wish is read and watched yet.
    pub fn watching(&self) -> bool {
        self.watching
    }

    /// For a shot of a menu half way in: motion on whatever the system says, on a clock moved
    /// by hand from now on, and the system never asked.
    pub fn shot_fade(&mut self) {
        self.forced = Some(Policy::Always);
        self.anim.set_policy(Policy::Always);
        self.watching = true;
        self.clock = Some(ManualClock::new());
    }

    /// Moves the hand-moved clock by `ms`; nothing without one.
    pub fn advance_ms(&self, ms: u64) {
        if let Some(c) = &self.clock {
            c.advance_ms(ms);
        }
    }

    /// The time of a frame.
    pub fn now(&self) -> Instant {
        match &self.clock {
            Some(c) => c.now(),
            None => Instant::now(),
        }
    }

    /// A motion for a test that moves, on a clock moved by hand, with the system asking for
    /// nothing.
    #[cfg(test)]
    pub fn moving(clock: ManualClock) -> Motion {
        let mut m = Motion::new(false);
        m.watching = true;
        m.reader = || false;
        m.clock = Some(clock);
        m
    }

    /// A test's reader of the system's wish.
    #[cfg(test)]
    pub fn set_reader(&mut self, reader: fn() -> bool) {
        self.reader = reader;
    }

    /// The presences of the open menus this frame, asked in the group [`MENUS`]. A popup's
    /// serial names it, so a menu opened again enters again. `None` for no popup and for the
    /// completion list.
    pub fn menus(&mut self, popup: Option<&Popup>) -> Option<Menus> {
        let popup = popup.filter(|p| fades(p.kind))?;
        self.anim.group(MENUS);
        let mut shown = |p: &Popup| {
            let t = self
                .anim
                .presence(("menu", p.serial), true, MENU.enter, MENU.exit);
            (p.serial, t)
        };
        let main = shown(popup);
        let sub = popup.sub.as_deref().map(&mut shown);
        self.anim.ungroup();
        Some(Menus { main, sub })
    }
}

/// Whether a popup of this kind fades as it opens: every menu but the completion list.
pub fn fades(kind: PopupKind) -> bool {
    kind != PopupKind::Complete
}

/// Everything a popup's fade touches: its menu's presence bounds and its submenu's, in one.
pub fn popup_bounds(popup: &Popup) -> Rect {
    let r = popup.menu.presence_bounds();
    match &popup.sub {
        Some(s) => union(r, s.menu.presence_bounds()),
        None => r,
    }
}

fn union(a: Rect, b: Rect) -> Rect {
    let (x, y) = (a.x.min(b.x), a.y.min(b.y));
    Rect::new(
        x,
        y,
        a.right().max(b.right()) - x,
        a.bottom().max(b.bottom()) - y,
    )
}

/// Draws the open popup and its submenu in `room`, each at its presence in `menus` (at rest
/// with none). While either moves, the pixels under them are kept in `backing` first, unless
/// `keep` says the backing in hand is what is under them (a frame that only moves them).
#[allow(clippy::too_many_arguments)]
pub fn draw_popup(
    p: &mut Painter,
    pal: &Palette,
    popup: &mut Popup,
    room: Rect,
    menus: Option<Menus>,
    mouse: (f32, f32),
    backing: &mut Option<(Snapshot, Rect)>,
    keep: bool,
) {
    if !keep {
        *backing = menus.filter(Menus::moving).map(|_| {
            let r = popup_bounds(popup);
            (p.snapshot(r), r)
        });
    }
    let main = menus.map_or(1.0, |m| m.main.1);
    let sub = menus.and_then(|m| m.sub).map_or(1.0, |s| s.1);
    draw_menu(p, pal, popup, room, main, mouse);
    if let Some(s) = popup.sub.as_deref_mut() {
        draw_menu(p, pal, s, room, sub, mouse);
    }
}

/// One menu at presence `t`: faded as a whole and dropped [`MENU_DROP`] px into place (raised,
/// for a menu above its anchor), the AI menu's Modify buttons with it.
fn draw_menu(
    p: &mut Painter,
    pal: &Palette,
    popup: &mut Popup,
    room: Rect,
    t: f32,
    mouse: (f32, f32),
) {
    let ai = popup.kind == PopupKind::Ai;
    let drop = if popup.menu.opened_above() {
        MENU_DROP
    } else {
        -MENU_DROP
    };
    let bounds = popup.menu.presence_bounds();
    let menu = &mut popup.menu;
    pop(p, bounds, t, (0.0, drop), |p| {
        menu.draw_in(p, pal, room);
        // The AI menu stands on its own under the bar's AI edit button.
        if ai {
            crate::ai_ui::draw_menu_modify(p, pal, menu, mouse);
        }
    });
}

/// How the frame draws the menus: `menus` is what this frame shows, `drawn` what the screen
/// shows; `busy` says something else under or over the menus is drawn this frame (they changed,
/// a region under them is drawn, the whole window is, or another overlay shows).
pub fn menu_frame(
    menus: Option<Menus>,
    drawn: Option<Menus>,
    backing: Option<&(Snapshot, Rect)>,
    bounds: Option<Rect>,
    busy: bool,
) -> MenuFrame {
    let Some(m) = menus else {
        return MenuFrame::Usual;
    };
    // Still entering, or arrived since the screen showed it on the way.
    let due = drawn != Some(m) && (m.moving() || drawn.is_some_and(|d| d.moving()));
    if !due {
        return MenuFrame::Usual;
    }
    let same_place = backing.is_some_and(|(_, r)| Some(*r) == bounds);
    if !busy && same_place && drawn.is_some_and(|d| d.main.0 == m.main.0) {
        MenuFrame::Alone
    } else {
        MenuFrame::Whole
    }
}
