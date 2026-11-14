//! The preferences the drawing and the column arithmetic read on every call, kept as process-wide
//! values so that no signature along the way changes: the base font size, the width of a tab, and
//! whether line numbers and the current line's band show. The settings dialog sets them; the
//! defaults are the old constants, so nothing changes until a setting does.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};

static TAB: AtomicUsize = AtomicUsize::new(4);
static FONT_PT: AtomicU32 = AtomicU32::new(0x4160_0000); // 14.0
static NUMBERS: AtomicBool = AtomicBool::new(true);
static FAMILY: Mutex<String> = Mutex::new(String::new());
static LINE_BAND: AtomicBool = AtomicBool::new(true);

/// Columns of a tab.
pub fn tab_width() -> usize {
    TAB.load(Ordering::Relaxed)
}

/// The base text size at 100 % zoom, px (the preference calls it points).
pub fn font_pt() -> f32 {
    f32::from_bits(FONT_PT.load(Ordering::Relaxed))
}

pub fn line_numbers() -> bool {
    NUMBERS.load(Ordering::Relaxed)
}

pub fn highlight_line() -> bool {
    LINE_BAND.load(Ordering::Relaxed)
}

/// The family list the text is drawn with: the preference's family first, then the built-in list,
/// so a name that is not installed falls back to what was there before.
pub fn font_stack() -> String {
    let user = FAMILY.lock().map(|f| f.clone()).unwrap_or_default();
    if user.is_empty() {
        crate::view::MONO.to_string()
    } else {
        format!("{user}, {}", crate::view::MONO)
    }
}

/// Takes the values of `prefs`. True when one that changes what is drawn changed.
pub fn apply(prefs: &mdedit_core::prefs::Prefs) -> bool {
    let before = (
        tab_width(),
        font_pt().to_bits(),
        line_numbers(),
        highlight_line(),
        font_stack(),
    );
    if let Ok(mut f) = FAMILY.lock() {
        *f = prefs.font_family.trim().to_string();
    }
    TAB.store(prefs.tab_width.clamp(1, 16), Ordering::Relaxed);
    FONT_PT.store(prefs.font_pt.to_bits(), Ordering::Relaxed);
    NUMBERS.store(prefs.line_numbers, Ordering::Relaxed);
    LINE_BAND.store(prefs.highlight_line, Ordering::Relaxed);
    before
        != (
            tab_width(),
            font_pt().to_bits(),
            line_numbers(),
            highlight_line(),
            font_stack(),
        )
}
