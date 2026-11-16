//! mdedit: winit for the window and input, softbuffer for the pixels, every control drawn by this
//! crate or by mdshared-ui. Phase 1 is read-only.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod args;
mod chrome;
mod commands;
mod compare;
mod docs;
mod frost;
mod image_paste;
mod keys;
mod link;
mod live;
mod macros;
mod marks;
mod minimap;
mod outline;
mod overlays;
mod palette;
mod panes;
mod platform;
mod preview_pane;
mod recall;
mod recovery;
mod replace_ui;
mod results;
mod session;
mod status;
mod trace;
mod ui;
mod updater;
mod view;

fn main() {
    end_on_panic();
    std::process::exit(app::run());
}

/// The release build unwinds so that the preview pane's thread can catch a panic in mdview's
/// renderer. Any other panic ends the process at once, as an aborting build would: a worker that
/// died quietly would leave the editor half working.
fn end_on_panic() {
    let report = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        report(info);
        if std::thread::current().name() != Some(preview_pane::THREAD) {
            std::process::abort();
        }
    }));
}
