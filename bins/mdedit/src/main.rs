//! mdedit: winit for the window and input, softbuffer for the pixels, every control drawn by this
//! crate or by mdshared-ui. Phase 1 is read-only.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ai;
mod ai_ui;
mod app;
mod args;
mod chrome;
mod commands;
mod compare;
mod docs;
mod format_bar;
mod frost;
mod image_paste;
mod instance;
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
    // First: a crash from here on leaves a report for the next start.
    if let Some(dir) = app::crash_dir() {
        mdshared_app::crash::install("mdedit", env!("CARGO_PKG_VERSION"), dir);
    }
    std::process::exit(app::run());
}
