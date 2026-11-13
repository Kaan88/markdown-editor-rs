//! mdedit: winit for the window and input, softbuffer for the pixels, every control drawn by this
//! crate or by mdshared-ui. Phase 1 is read-only.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod args;
mod chrome;
mod commands;
mod docs;
mod keys;
mod macros;
mod outline;
mod overlays;
mod panes;
mod platform;
mod results;
mod session;
mod status;
mod trace;
mod ui;
mod updater;
mod view;

fn main() {
    std::process::exit(app::run());
}
