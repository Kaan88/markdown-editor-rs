//! Sizes and caps. Each one is a decision recorded in `docs/DESIGN.md`.

/// Larger files are refused with [`crate::LoadError::TooLarge`]. The budget is set on a 100 MB
/// file; a rope of a 1 GiB file costs about 1.2 GiB of memory.
pub const MAX_FILE_BYTES: u64 = 1 << 30;

/// How much the loader reads at a time. The decoded text goes into the rope chunk by chunk, so
/// the file's bytes are never held whole next to the rope.
pub const READ_CHUNK: usize = 64 * 1024;

/// The loader reports progress after at least this many bytes.
pub const PROGRESS_STEP: u64 = 4 << 20;

/// Lines whose display text is at most this many bytes are shaped whole; longer lines are shaped
/// in chunks of about [`CHUNK_BYTES`], only where they are on screen (spike "longlines").
pub const WHOLE_LINE_BYTES: usize = 4096;
pub const CHUNK_BYTES: usize = 1024;
/// A chunk boundary is looked for this far past [`CHUNK_BYTES`]: an ASCII space or tab first,
/// else the next grapheme boundary.
pub const CHUNK_LOOKAHEAD: usize = 512;

/// Columns per tab stop.
pub const TAB_WIDTH: usize = 4;

/// Find stops counting after this many matches, as the viewer does, and reports the count as
/// capped ("10000+").
pub const FIND_CAP: usize = 10_000;
/// A search gives up (capped) after comparing this many characters per byte of text, so a long
/// query against text that nearly matches everywhere cannot run away.
pub const FIND_WORK_PER_BYTE: usize = 64;
/// Buffers up to this size are searched on the UI thread; larger ones on a worker.
pub const SYNC_FIND_BYTES: usize = 4 << 20;

/// Copying a larger selection is refused with a message rather than building a string of it.
pub const COPY_LIMIT: usize = 256 << 20;
