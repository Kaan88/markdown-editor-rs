//! mdedit's headless core: no window system, no threads of its own. The shell runs the slow
//! functions here (loading, finding in a large buffer) on its worker threads; everything else is
//! called on the UI thread.
//!
//! Positions are byte offsets into the buffer's UTF-8 text, always on a `char` boundary. Lines
//! are 0-based here; the status area shows them 1-based. `docs/DESIGN.md` has the full contract.

pub mod buffer;
pub mod display;
pub mod encoding;
pub mod eol;
pub mod find;
pub mod limits;
pub mod load;
pub mod selection;

pub use buffer::{Buffer, Position};
pub use encoding::Encoding;
pub use eol::{EolCounts, LineEnding};
pub use find::{FindOptions, Matches, Query};
pub use load::{FileInfo, LoadError, LoadOptions, Loaded};
pub use selection::{Motion, Selection};
