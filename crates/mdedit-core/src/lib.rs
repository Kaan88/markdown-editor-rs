//! mdedit's headless core: no window system, no threads of its own. The shell runs the slow
//! functions here (loading, finding in a large buffer) on its worker threads; everything else is
//! called on the UI thread.
//!
//! Positions are byte offsets into the buffer's UTF-8 text, always on a `char` boundary. Lines
//! are 0-based here; the status area shows them 1-based. `docs/DESIGN.md` has the full contract.

pub mod bracket;
pub mod buffer;
pub mod diff;
pub mod display;
pub mod edit;
pub mod encoding;
pub mod eol;
pub mod find;
pub mod fold;
mod hash;
pub mod history;
pub mod limits;
pub mod lines;
pub mod load;
pub mod multi;
pub mod replace;
pub mod replace_files;
pub mod save;
pub mod search;
pub mod selection;
#[cfg(test)]
mod testutil;

pub use buffer::{Buffer, Change, Position};
pub use encoding::Encoding;
pub use eol::{EolCounts, LineEnding};
pub use find::{FindOptions, Matches, Query};
pub use history::History;
pub use load::{FileInfo, LoadError, LoadOptions, Loaded};
pub use save::SaveError;
pub use selection::{Motion, Selection};
