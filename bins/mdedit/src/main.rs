//! mdedit: winit for the window and input, softbuffer for the pixels, every control drawn by this
//! crate or by mdshared-ui. Phase 1 is read-only.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// `eprintln!` for this crate, shadowing `std`'s in every module below: a stderr that is gone (a
/// closed pipe) is ignored instead of being a panic, which the release build (`panic = "abort"`)
/// would turn into the end of the program. `tests/closed_stderr.rs` holds it.
macro_rules! eprintln {
    ($($arg:tt)*) => {{
        use std::io::Write as _;
        let _ = writeln!(std::io::stderr(), $($arg)*);
    }};
}

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
mod hunks_ui;
mod image_paste;
mod import_draw;
mod import_ui;
mod keys;
mod languages;
mod library_pane;
mod link;
mod live;
mod macros;
mod marks;
mod minimap;
mod move_draw;
mod move_ui;
mod note_draw;
mod note_ui;
mod notes_plan_draw;
mod notes_plan_ui;
mod notes_task;
mod outline;
mod overlays;
mod palette;
mod panes;
mod places;
mod plan_demo;
mod plan_ui;
mod platform;
mod preview_pane;
mod recall;
mod recovery;
mod recycle;
mod replace_ui;
mod results;
mod session;
mod status;
mod task_draw;
mod task_ui;
mod trace;
mod ui;
mod updater;
mod view;

fn main() {
    // First: a crash from here on leaves a report for the next start.
    if let Some(dir) = app::crash_dir_of_this_launch() {
        mdshared_app::crash::install("mdedit", env!("CARGO_PKG_VERSION"), dir);
    }
    std::process::exit(app::run());
}
