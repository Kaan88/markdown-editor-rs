//! A stand-in for Write notes' write (phase V5), so the plan card (phase V4) can be built and
//! tried before it. **Replaced by the write phase:** its `app/notes_write.rs` defines
//! `App::notes_write`, and this file and its `mod` line in `app.rs` are dropped at that merge.

use super::*;

impl App {
    /// "Write N notes" on the plan card: the write phase writes `run.arranged` here.
    pub(super) fn notes_write(&mut self) {}
}
