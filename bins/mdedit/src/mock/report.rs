//! `hub-report`: the layout numbers of the window, read from the same constants the drawing uses.

use super::data::{self, APPS};
use super::win::{self, Scene};
use mdshared_ui::gfx::{Face, Painter};
use mdshared_ui::theme::Palette;

pub fn print(p: &mut Painter, _pal: &Palette, w: f32, h: f32) {
    use std::sync::atomic::{AtomicBool, Ordering};
    static DONE: AtomicBool = AtomicBool::new(false);
    if DONE.swap(true, Ordering::Relaxed) {
        return;
    }
    eprintln!("window {w} x {h}");
    for ww in [1280.0f32, 860.0, 760.0] {
        let c = win::cols(ww);
        let fixed = c.installed + c.latest + c.released + c.size + c.status + c.actions;
        eprintln!(
            "at {ww}: installed {} latest {} released {} download {} status {} actions {}, app column {:.0} px",
            c.installed,
            c.latest,
            c.released,
            c.size,
            c.status,
            c.actions,
            ww - fixed
        );
    }
    let mut worst = ("", 0.0f32);
    for a in &APPS {
        let wd = p.measure(Face::Semibold, 13.5, a.name) + 10.0 + 80.0;
        let blurb = p.measure(Face::Ui, 11.5, a.blurb);
        let wd = wd.max(blurb);
        if wd > worst.1 {
            worst = (a.name, wd);
        }
    }
    eprintln!("widest name or blurb: {} at {:.0} px", worst.0, worst.1);
    let sc = Scene::default();
    let n = win::rows(&sc).len();
    let list = win::HEAD_H + n as f32 * win::ROW_H;
    eprintln!("rows: {n} of {} px, list {list} px", win::ROW_H);
    for (hh, banner) in [(800.0f32, 0.0f32), (800.0, win::BANNER_H), (600.0, 0.0)] {
        let room = hh - 40.0 - 44.0 - banner - win::STATUS_H;
        let pane = room - list;
        eprintln!(
            "window height {hh}{}: room {room:.0}, pane {}",
            if banner > 0.0 { " with a banner" } else { "" },
            if pane >= win::PANE_MIN {
                format!("{pane:.0} px")
            } else {
                "hidden (under 150 px)".to_string()
            }
        );
    }
    let n8 = n + 1;
    let list8 = win::HEAD_H + n8 as f32 * win::ROW_H;
    let room = 800.0 - 40.0 - 44.0 - win::BANNER_H - win::STATUS_H;
    eprintln!(
        "with the Hub row ({n8} rows) and a banner at 800: pane {:.0} px",
        room - list8
    );
    eprintln!("hub row index {}", data::HUB);
}
