//! mdedit's headless core: no window system, no threads of its own. The shell runs the slow
//! functions here (loading, finding in a large buffer) on its worker threads; everything else is
//! called on the UI thread.
//!
//! Positions are byte offsets into the buffer's UTF-8 text, always on a `char` boundary. Lines
//! are 0-based here; the status area shows them 1-based. `docs/DESIGN.md` has the full contract.

pub mod ai;
pub mod blocks;
pub mod bracket;
pub mod buffer;
pub mod complete;
pub mod diff;
pub mod display;
pub mod edit;
pub mod encoding;
pub mod eol;
pub mod find;
pub mod fold;
mod hash;
pub mod history;
pub mod hunks;
pub mod import;
pub mod import_ai;
pub mod indent;
pub mod limits;
pub mod lines;
pub mod load;
pub mod markdown;
pub mod merge;
pub mod multi;
pub mod multiline;
pub mod notes_ai;
pub mod prefs;
pub mod recovery;
pub mod replace;
pub mod replace_files;
pub mod save;
pub mod search;
pub mod selection;
pub mod snippets;
pub mod split;
pub mod table;
#[cfg(test)]
mod testutil;
pub mod typing;
pub mod vcol;
/// The run record and Move the notes live in the shared crates (D-081); kept here by their old
/// names so `mdedit_core::runs` and `mdedit_core::moves` read as before.
pub use mdshared_app::{moves, runs};

pub use buffer::{Buffer, Change, Position};
pub use encoding::Encoding;
pub use eol::{EolCounts, LineEnding};
pub use find::{FindOptions, Matches, Query};
pub use history::History;
pub use load::{FileInfo, LoadError, LoadOptions, Loaded};
pub use save::SaveError;
pub use selection::{Motion, Selection};
