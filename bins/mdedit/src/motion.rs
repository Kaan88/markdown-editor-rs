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
//!
//! A toast rises and fades in, and fades and sinks out as it leaves of itself ([`TOAST`]),
//! through the shared `anim::Stack`: a status takes the place of the toasts showing, in place
//! (`Docs::set_toast`); a one-shot confirmation comes in under them, once they have lifted
//! (`Docs::confirm_toast`).

use crate::ui::{Popup, PopupKind};
use mdedit_highlight::DocId;
use mdshared_ui::anim::stack::Stack;
use mdshared_ui::anim::{Animator, Clock, Id, ManualClock, Policy, Snapshot, document, pop};
use mdshared_ui::gfx::{Painter, Rect};
use mdshared_ui::menu::MENU_DROP;
use mdshared_ui::overlay::TOAST_H;
use mdshared_ui::theme::Palette;
use std::time::Instant;

/// How a menu enters and leaves: the one place its timing is chosen. The shared preset as it
/// is: in over 117 ms, decelerating (`document::MENU_ENTER`, the owner's choice over 167 ms on
/// 2026-11-25, which shared took up at `f45a71c`); the exit is the preset's and unused (closing
/// is instant).
pub const MENU: document::Pair = document::MENU;

/// The damage group of the open menus: what is asked for while they are drawn.
pub const MENUS: &str = "menus";

/// The damage group of a menu's hover rows: a row's fill fades in and out with the pointer
/// (`Menu::draw_in_animated`, shared's `HOVER`), inside the popup's own bounds.
pub const ROWS: &str = "menu-rows";

/// The earliest outstanding paint for a skipped region or overlay. The animator advances tracks when
/// kept, but offers a fresh deadline relative to each frame: preserve the paint already
/// promised so successive early frames cannot keep postponing it.
#[derive(Default)]
pub struct PaintDeadline(Option<Instant>);

impl PaintDeadline {
    pub fn due(&self, now: Instant) -> bool {
        self.0.is_some_and(|at| now >= at)
    }

    pub fn update(&mut self, anim: &Animator, group: &str, kept: bool) -> Option<Instant> {
        let next = anim
            .group_deadlines()
            .iter()
            .find(|(id, _)| *id == Id::new(group))
            .map(|(_, when)| *when);
        self.0 = match (kept, self.0, next) {
            (true, Some(old), Some(new)) => Some(old.min(new)),
            (true, Some(old), None) => Some(old),
            _ => next,
        };
        self.0
    }
}

/// The height both the notice drawing and the editor layout give a sliding bar.
pub fn banner_height(t: f32) -> f32 {
    mdshared_ui::notice::bar_height(Rect::new(0.0, 0.0, 0.0, crate::view::BANNER_H), t)
}

/// How a tooltip enters: the shared preset, a plain short fade, where the eye already is. It
/// goes at once (there is nothing to watch leave), and the wait before it is
/// `document::TIP_DELAY` from the pointer coming to rest (`Tooltip::hover_at`).
pub const TIP: document::Pair = document::TIP;

/// The damage group of the tooltip.
pub const TIP_GROUP: &str = "tip";

/// The damage group of the left pane (the Files tree's chevrons).
pub const SIDEBAR: &str = "sidebar";

/// The damage group of the title row (a tab's, a caption button's or the new-tab button's
/// hover, and a drop-down it opens).
pub const TITLE: &str = "title";

/// How a tree row's disclosure turns: shared's own for it, 167 ms both ways, made of
/// [`fluent::MOVE`] and ended as the row's body opens.
pub const DISCLOSURE: mdshared_ui::anim::Motion = mdshared_ui::glyph::DISCLOSURE_MOTION;

/// The damage group of the Settings form.
pub const SETTINGS: &str = "settings";

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
    pub sidebar_due: PaintDeadline,
    pub title_due: PaintDeadline,
    pub rows_due: PaintDeadline,
    pub settings_due: PaintDeadline,
    /// The command palette's presence as the last frame drew it (1 when it is not open).
    pub palette_t: f32,
    /// Presences of the update, crash, stale-tab and rename bars, in layout order.
    pub banner_t: [f32; 4],
    /// A layout-only pass is not a paint. Notices already present on the first paint snap.
    pub banners_composed: bool,
    /// The rename offer we have already shown. Changing tabs does not create a new offer.
    rename_seen: Option<DocId>,
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
    /// The toasts on screen and leaving ([`toasts`]).
    pub toasts: Stack<String>,
    /// The serial of the status message the toasts last took in (`Docs::toast_line`).
    toast_seen: Option<u64>,
    /// The toast last pushed onto a stack that already held something: with the wait for the
    /// lift, it is the only one that can still be at nothing when a status takes its place.
    toast_waiting: Option<u64>,
    /// The tooltip's presence as the last frame drew it, 0 when none showed
    /// ([`Motion::tip_presence`]): the compositor's and a test's to read, never to set.
    pub tip_t: f32,
    /// This frame's toasts, the bottom one first ([`Motion::sync_toasts`]).
    pub toast_frame: Vec<ToastShown>,
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
            sidebar_due: PaintDeadline::default(),
            title_due: PaintDeadline::default(),
            rows_due: PaintDeadline::default(),
            settings_due: PaintDeadline::default(),
            palette_t: 1.0,
            banner_t: [0.0; 4],
            banners_composed: false,
            rename_seen: None,
            due: None,
            still,
            setting: true,
            watching: false,
            forced: None,
            reader: crate::platform::motion_reduced,
            backing: None,
            drawn: None,
            clock: None,
            toasts: toasts(),
            toast_seen: None,
            toast_waiting: None,
            tip_t: 0.0,
            toast_frame: Vec::new(),
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

    /// How present the tooltip is this frame, asked in the group [`TIP_GROUP`]: it fades in and
    /// is gone at once when it goes. `shown` is whether one is on screen. The answer is kept in
    /// `tip_t` for the rest of the frame (the region's signature, and tests).
    pub fn tip_presence(&mut self, shown: bool) -> f32 {
        self.anim.group(TIP_GROUP);
        let t = self.anim.presence(
            Id::new(TIP_GROUP).with("in"),
            shown,
            TIP.enter,
            mdshared_ui::anim::Motion::SNAP,
        );
        self.anim.ungroup();
        self.tip_t = t;
        t
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

    /// The command palette's presence: it pops in like a menu (117 ms, decelerating) and is
    /// drawn by `Palette::draw_presence`; its deadline comes through `end_frame`.
    pub fn palette(&mut self) -> f32 {
        self.anim
            .presence("palette", true, document::MENU_ENTER, document::MENU_ENTER)
    }

    /// The same offer is returning to view, not appearing for the first time.
    pub fn rename_returns(&self, offer: Option<DocId>) -> bool {
        offer.is_some()
            && offer == self.rename_seen
            && self.anim.get::<f32>(banner_id(3, offer)).is_none()
    }

    /// Ask before layout: a change in summed rounded heights damages the resized regions.
    pub fn sync_banners(&mut self, shown: [bool; 4], rename_offer: Option<DocId>) {
        let first = !std::mem::replace(&mut self.banners_composed, true);
        let returning = shown[3] && self.rename_returns(rename_offer);
        self.rename_seen = rename_offer.filter(|_| shown[3] || self.rename_seen == rename_offer);
        self.banner_t = std::array::from_fn(|i| {
            if !shown[i] {
                0.0
            } else if first || (i == 3 && returning) {
                // Establish the track at rest too, or the next frame would start an entrance.
                self.anim.set(banner_id(i, rename_offer), 1.0_f32);
                1.0
            } else {
                self.anim.presence(
                    banner_id(i, rename_offer),
                    true,
                    document::MENU_ENTER,
                    MENU.exit,
                )
            }
        });
    }
}

fn banner_id(i: usize, offer: Option<DocId>) -> Id {
    if i == 3 {
        Id::new(("bar", i, offer))
    } else {
        Id::new(("bar", i))
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
    anim: &mut Animator,
) {
    if !keep {
        *backing = menus.filter(Menus::moving).map(|_| {
            let r = popup_bounds(popup);
            (p.snapshot(r), r)
        });
    }
    let main = menus.map_or(1.0, |m| m.main.1);
    let sub = menus.and_then(|m| m.sub).map_or(1.0, |s| s.1);
    anim.group(ROWS);
    draw_menu(p, pal, popup, room, main, mouse, anim);
    if let Some(s) = popup.sub.as_deref_mut() {
        draw_menu(p, pal, s, room, sub, mouse, anim);
    }
    anim.ungroup();
}

/// One menu at presence `t`: faded as a whole and dropped [`MENU_DROP`] px into place (raised,
/// for a menu above its anchor), the AI menu's Modify buttons with it.
#[allow(clippy::too_many_arguments)]
fn draw_menu(
    p: &mut Painter,
    pal: &Palette,
    popup: &mut Popup,
    room: Rect,
    t: f32,
    mouse: (f32, f32),
    anim: &mut Animator,
) {
    let ai = popup.kind == PopupKind::Ai;
    let drop = if popup.menu.opened_above() {
        MENU_DROP
    } else {
        -MENU_DROP
    };
    let bounds = popup.menu.presence_bounds();
    let (animated, id) = (fades(popup.kind), ("row-hover", popup.serial));
    let menu = &mut popup.menu;
    pop(p, bounds, t, (0.0, drop), |p| {
        if animated && anim.animates() {
            menu.draw_in_animated(p, pal, room, anim, id);
        } else {
            menu.draw_in(p, pal, room);
        }
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

// ---- toasts --------------------------------------------------------------------------------

/// A toast's entrance and exit: the shared preset as it is (167 ms each, decelerating in and
/// accelerating out). A toast rises 8 px into place as it fades in, and fades and sinks as it
/// leaves of itself; a toast taken away (`clear`) or put in another's place goes at once.
pub const TOAST: document::Pair = document::TOAST;

/// The damage group of the toasts.
pub const TOASTS: &str = "toasts";

/// The gap between two stacked toasts, in logical px.
pub const TOAST_GAP: f32 = 8.0;

/// The most toasts kept at once, those leaving included: a fourth sends the oldest away.
pub const TOASTS_MAX: usize = 3;

/// The toasts' stack: entering and leaving by [`TOAST`], at most [`TOASTS_MAX`] shown and as
/// many kept, and a new one entering only once the toasts above it have lifted ("New waits for
/// the lift", D-252).
pub fn toasts() -> Stack<String> {
    Stack::new()
        .with_motion(TOAST.enter, TOAST.exit)
        .with_max(TOASTS_MAX)
        .with_max_drawn(TOASTS_MAX)
        .with_wait_for_lift()
}

/// A toast as a frame shows it: its key, its text, its presence and how far above the bottom
/// place it stands, in logical px.
#[derive(Clone, Debug, PartialEq)]
pub struct ToastShown {
    pub key: u64,
    pub text: String,
    pub t: f32,
    pub lift: f32,
}

/// What the window's status says, as the toasts follow it: `serial` names one message (a new
/// one for each `set_toast` and `confirm_toast`), `stack` says it comes in under the toasts
/// showing instead of in their place.
#[derive(Clone, Copy, Debug)]
pub struct ToastLine<'a> {
    pub serial: u64,
    pub text: &'a str,
    pub until: Instant,
    pub stack: bool,
}

impl Motion {
    /// Brings the toasts up to date with the status line at `now`, the frame's time, and asks
    /// for this frame of them, in the group [`TOASTS`]. Asked on every frame, whatever it draws,
    /// so a toast at rest is never forgotten and taken for new. Each leaves of itself when its
    /// time is up.
    ///
    /// A message set by `set_toast` takes the place of the toasts showing: the newest one shown
    /// takes its text at once and keeps its presence and its place (a readout under key repeat
    /// never flickers), and every other toast goes at once, the ones still leaving included;
    /// with none shown it comes in as a new toast. One it takes over that had not come in yet
    /// (it was waiting for the toasts above to lift) stands there at once, rather than waiting
    /// in the dark for a lift that no longer matters.
    /// One set by `confirm_toast`, a one-shot confirmation of its own, comes in under the ones
    /// showing and then enters once they have lifted. Said again, it never stands twice: the
    /// newest keeps its own toast where it is, at rest, while one standing higher is taken out
    /// and comes in at the bottom, so the one just triggered is the one in sight.
    /// A status cleared takes every toast away at once.
    pub fn sync_toasts(&mut self, line: Option<ToastLine<'_>>, now: Instant) {
        self.toasts.tick(now);
        let serial = line.map(|l| l.serial);
        let mut come_in: Option<u64> = None;
        if serial != self.toast_seen {
            self.toast_seen = serial;
            // What is left of its time, by the wall clock it was set by, counted on from this
            // frame on the frame's clock (the same clock in a window; a test's or a shot's runs
            // by hand).
            let left = line.and_then(|l| l.until.checked_duration_since(Instant::now()));
            match line {
                // A message already past its time (set long before this frame) is not shown.
                Some(l) if left.is_some_and(|d| !d.is_zero()) => {
                    let until = left.map(|d| now + d);
                    if l.stack {
                        // A confirmation said again that is not the newest goes to the bottom:
                        // what the user just triggered is the one in sight, even in a short
                        // window. The newest keeps its toast, its key and its presence, so a
                        // repeat never makes it blink; `show` only resets its time.
                        let newest = self.toasts.current().last().map(|(k, _)| k);
                        let equal = self.toasts.find(|t| t.as_str() == l.text);
                        let kept = equal.is_some() && equal.map(|(k, _)| k) == newest;
                        if let Some((old, _)) = equal
                            && !kept
                        {
                            self.toasts.remove(old);
                        }
                        // `show` pushes unless the same one is still the newest; a push onto a
                        // stack that holds anything, a toast at rest or one leaving, gives the
                        // newcomer the wait for the lift. Only such a toast can be at nothing
                        // when a status takes its place. A repeat revives the newest and does
                        // not touch the animator, so a toast that was waiting goes on waiting
                        // and stays the one to snap.
                        if !kept {
                            let held = !self.toasts.is_empty();
                            let key = self.toasts.show(l.text.to_string(), until);
                            self.toast_waiting = held.then_some(key);
                        } else {
                            self.toasts.show(l.text.to_string(), until);
                        }
                    } else {
                        // A status replaces in place, and every other toast goes at once — the
                        // ones still playing their exit too, named by the last frame's list,
                        // which holds them, not by `current`, which does not.
                        let newest = self.toasts.current().last().map(|(k, _)| k);
                        let others: Vec<u64> = (self.toast_frame.iter())
                            .map(|s| s.key)
                            .filter(|k| Some(*k) != newest)
                            .collect();
                        for k in others {
                            self.toasts.remove(k);
                        }
                        // Everything the last frame held is gone, so a push here is onto
                        // nothing and enters at once: it is not one that waits.
                        if newest.is_none() {
                            debug_assert!(self.toasts.is_empty());
                        }
                        let key = self.toasts.replace_newest(l.text.to_string(), until);
                        // The toast it takes over may still have been waiting for its lift, and
                        // the animator keeps that wait even after the stack gives it up (a new
                        // motion for an unchanged target is ignored): it stands there now. Both
                        // things must hold: it was pushed onto others, and it is still at
                        // nothing on the last frame. One past its wait is coming in, and a
                        // status lets it go on from where it is.
                        let waiting = newest.is_some_and(|k| {
                            self.toast_waiting == Some(k)
                                && !self.toast_frame.iter().any(|s| s.key == k && s.t > 0.0)
                        });
                        self.toast_waiting = None;
                        if waiting {
                            come_in = Some(key);
                        }
                    }
                }
                Some(_) => {}
                None => {
                    self.toasts.clear();
                    self.toast_waiting = None;
                }
            }
        }
        self.toast_frame.clear();
        if self.toasts.is_empty() {
            return;
        }
        self.anim.group(TOASTS);
        if let Some(key) = come_in {
            self.anim.set(Id::new(TOASTS).with(("in", key)), 1.0);
        }
        let shown = (self.toasts).frame(&mut self.anim, TOASTS, |_| TOAST_H, TOAST_GAP);
        self.toast_frame
            .extend(shown.into_iter().map(|s| ToastShown {
                key: s.key,
                text: s.item.clone(),
                t: s.t,
                lift: s.lift,
            }));
        self.anim.ungroup();
    }

    /// Whether `line` is a fresh message the toasts have not taken in yet: a frame is wanted.
    pub fn toast_unseen(&self, line: Option<ToastLine<'_>>, now: Instant) -> bool {
        line.is_some_and(|l| Some(l.serial) != self.toast_seen && l.until > now)
    }

    /// When the next toast leaves of itself.
    pub fn toasts_due(&self) -> Option<Instant> {
        self.toasts.deadline()
    }
}

/// Where a toast whose place at rest is `rest` stands `lift` logical px above it: moved by a
/// whole number of device pixels at `scale`, so its text stays on the grid it has at rest.
pub fn toast_y(rest: f32, lift: f32, scale: f32) -> f32 {
    rest - (lift * scale).round() / scale
}
