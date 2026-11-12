//! The window and its event loop, modelled on the viewer's `bins/mdv-app/src/app.rs`: a frameless
//! window, regions with signatures, damage-only present, a hidden first frame, timers only for
//! pending work. Also `--shot`.

mod commands;
mod input;

/// Starts the editor; the process exit code.
pub fn run() -> i32 {
    let _args = crate::args::Args::parse(std::env::args().skip(1));
    todo!()
}
