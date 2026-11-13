//! Sizes and caps. Each one is a decision recorded in `docs/DESIGN.md`.

/// Larger files are refused with [`crate::LoadError::TooLarge`]. The budget is set on a 100 MB
/// file; a rope of a 1 GiB file costs about 1.2 GiB of memory.
pub const MAX_FILE_BYTES: u64 = 1 << 30;

/// How much the loader reads at a time. The decoded text goes into the rope chunk by chunk, so
/// the file's bytes are never held whole next to the rope.
pub const READ_CHUNK: usize = 64 * 1024;

/// A file that is not valid UTF-8 and has no byte order mark is judged on this many bytes from
/// around its first invalid byte: mostly valid UTF-8 there means UTF-8 with the invalid bytes
/// replaced, otherwise `chardetng` guesses the legacy code page.
pub const DETECT_BYTES: usize = 1 << 20;

/// The loader reports progress after at least this many bytes.
pub const PROGRESS_STEP: u64 = 4 << 20;

/// Lines whose display text is at most this many bytes are shaped whole; longer lines are shaped
/// in chunks of about [`CHUNK_BYTES`], only where they are on screen (spike "longlines").
pub const WHOLE_LINE_BYTES: usize = 4096;
pub const CHUNK_BYTES: usize = 1024;
/// A chunk boundary is looked for this far past [`CHUNK_BYTES`]: an ASCII space or tab first,
/// else the next grapheme boundary.
pub const CHUNK_LOOKAHEAD: usize = 512;

/// Word motions and double click look at most this many bytes each way from where they start, so
/// a word of millions of letters (a hex blob) cannot hold the UI thread: the word ends there.
pub const WORD_LIMIT: usize = 64 * 1024;

/// Columns per tab stop.
pub const TAB_WIDTH: usize = 4;

/// Find stops counting after this many matches, as the viewer does, and reports the count as
/// capped ("10000+").
pub const FIND_CAP: usize = 10_000;
/// A search gives up (capped) after comparing this many characters per byte of text, so a long
/// query against text that nearly matches everywhere cannot run away.
pub const FIND_WORK_PER_BYTE: usize = 64;
/// How far from the caret the partner of a bracket is looked for, in bytes each way.
pub const BRACKET_SCAN: usize = 1 << 16;
/// The longest selection, in bytes, whose other occurrences are marked, and the longest line
/// they are looked for in.
pub const TWIN_WORD_MAX: usize = 128;
pub const TWIN_LINE_MAX: usize = 8192;
/// A line longer than this many bytes (without its break) is skipped by regex find, so one huge
/// line cannot hold a search; plain and whole-word find are not limited by it.
pub const REGEX_LINE_MAX: usize = 1 << 20;
/// The compiled size a regex may reach (its program, in bytes): a pattern that needs more, such
/// as `(a{1000}){1000}`, is refused as invalid.
pub const REGEX_SIZE_LIMIT: usize = 1 << 20;
/// Buffers up to this size are searched on the UI thread; larger ones on a worker.
pub const SYNC_FIND_BYTES: usize = 4 << 20;

/// Copying a larger selection is refused with a message rather than building a string of it.
pub const COPY_LIMIT: usize = 256 << 20;

/// Text held by undo and redo steps (the removed text of every edit) is capped at this many
/// bytes: past it the oldest undo steps are dropped, never the newest one.
pub const UNDO_BYTES: usize = 64 << 20;
/// Removed text up to this many bytes is kept as a `String`; larger text is kept as a `Rope`
/// slice that shares the buffer's chunks instead of copying them.
pub const UNDO_SHARE_BYTES: usize = 4096;

/// The line commands that rebuild a block of lines in memory (sort, remove duplicates, reverse,
/// join) refuse a block of more bytes than this: they do nothing, and `lines::block_too_large`
/// says so, so the shell can tell the user.
pub const LINE_BLOCK_BYTES: usize = 64 << 20;
/// How far from the caret the typing helpers (`typing`) look for the text that decides what a
/// keystroke does: the line's indentation, the blanks before the caret. A longer run counts as
/// not found.
pub const TYPING_SCAN: usize = 4096;
