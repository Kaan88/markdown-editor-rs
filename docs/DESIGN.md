# Design: mdedit phase 1 (read-only)

Written 2026-11-12. Phase 1 of `docs/PLAN.md`: "Read-only: open a file in the shared shell, tabs,
scrolling, selection, copy, find, highlighting, encodings detected and shown. Done when
first-frame, memory and scroll budgets are met on the small and the 100 MB file." Editing is phase
2; this design keeps the places editing will plug into (section 13) but builds none of it.

The starting points are the four spike results in `spikes/*/RESULTS.md` (buffer, highlight,
longlines, ime), the shared crates in `../markdown-shared-rs/crates`, and the viewer's shell in
`../mv/bins/mdv-app/src`, which the editor's shell follows closely: the shared plan's step 4 (a
window framework in `mdshared-ui`) was not done, so its fallback applies and mdedit writes its own
window loop on the shared widgets, modelled on the viewer's.

Nothing here is approved by the user; the user approves the interface by looking at the running
application. Every number marked "spike" was measured on a loaded machine and is noisy.

## 1. Crates and ownership

| Path | Crate | What it holds | Work package |
|---|---|---|---|
| `crates/mdedit-core` | `mdedit-core` (lib) | Buffer, loading, encodings, line endings, display text, selection, find. Headless, no threads, no window system, no `unsafe` except the file replace in `save.rs` (section 16) | A |
| `../markdown-shared-rs/crates/mdshared-highlight/src/incremental.rs` | module of `mdshared-highlight` | The line highlighter: checkpoints, time-boxed runs, limits, language lookup by path | B |
| `crates/mdedit-highlight` | `mdedit-highlight` (lib) | The highlighting worker thread and the rope adapter | B |
| `bins/mdedit` | `mdedit` (bin `mdedit`) | The shell: window loop, text view, tabs, title row, bar, Files pane, menus, dialogs, status strip, session, `--shot`, `--trace` | C |
| `fixtures/` | | Small test files with exact bytes (`.gitattributes` keeps git from converting them) | C |

Dependencies (all versions in the root `Cargo.toml`'s `[workspace.dependencies]`, or in the
binary's own file for the platform crates; parley, winit, softbuffer and, through the shared
crates, swash and syntect are pinned to the viewer's exact versions):

```
mdedit (bin) ──> mdedit-core ──> ropey =1.6.1 (simd, cr_lines), encoding_rs 0.8, unicode-segmentation 1, memchr 2, mdshared-app
             ──> mdedit-highlight ──> mdshared-highlight (syntect =5.3.0), ropey
             ──> mdshared-ui (winit =0.30.13, rfd), mdshared-text (parley =0.12.0, swash =0.2.10), mdshared-app
             ──> parley =0.12.0, winit =0.30.13, softbuffer =0.4.8, bytemuck 1.25, windows-sys 0.59
```

The shared crates are path dependencies (`../markdown-shared-rs/crates/...`) until the shared
repository tags a version (its plan, step 5).

**Why the line highlighter goes into `mdshared-highlight`.** It needs the same syntax set, the same
two themes and the same language aliases as the viewer's code blocks, which are private functions
of that crate (`syntaxes`, `theme`, `syntax`); a child module can use them without making them
public. The module is additive: it adds no dependency (it reads text through a trait, not a rope)
and changes no function the viewer calls, so the viewer's output cannot change. The worker thread,
which is the editor's own policy, stays in the editor (`mdedit-highlight`).

### Public API of `mdedit-core`

The committed stubs are the reference; doc comments there give the details. Summary:

```rust
// limits
pub const MAX_FILE_BYTES: u64 = 1 << 30;      pub const READ_CHUNK: usize = 64 * 1024;
pub const PROGRESS_STEP: u64 = 4 << 20;       pub const WHOLE_LINE_BYTES: usize = 4096;
pub const CHUNK_BYTES: usize = 1024;          pub const CHUNK_LOOKAHEAD: usize = 512;
pub const DETECT_BYTES: usize = 1 << 20;      pub const WORD_LIMIT: usize = 64 * 1024;
pub const TAB_WIDTH: usize = 4;               pub const FIND_CAP: usize = 10_000;
pub const FIND_WORK_PER_BYTE: usize = 64;     pub const SYNC_FIND_BYTES: usize = 4 << 20;
pub const COPY_LIMIT: usize = 256 << 20;

// encoding
pub enum Encoding { Utf8, Utf8Bom, Utf16Le, Utf16Be, Legacy(&'static encoding_rs::Encoding) }
impl Encoding { pub fn label(self) -> String; pub fn bom_len(self) -> usize }
pub fn sniff_bom(head: &[u8]) -> Option<Encoding>;
pub fn legacy_for_code_page(code_page: u32) -> &'static encoding_rs::Encoding;

// eol
pub struct EolCounts { pub crlf: u64, pub lf: u64, pub cr: u64, /* private: pending_cr */ }
impl EolCounts { pub fn scan(&mut self, text: &[u8]); pub fn finish(&mut self);
                 pub fn of(text: &str) -> EolCounts; pub fn style(&self) -> LineEnding }
pub enum LineEnding { Crlf, Lf, Cr, Mixed, None }
impl LineEnding { pub fn label(self) -> &'static str }

// load
pub struct LoadOptions { pub legacy: &'static encoding_rs::Encoding, pub max_bytes: u64 } // Default
pub struct FileInfo { pub encoding: Encoding, pub eol: EolCounts, pub lossy: bool, pub has_nul: bool,
                      pub file_bytes: u64, pub stamp: mdshared_app::files::FileStamp,
                      pub longest_line_chars: usize, pub elapsed: Duration }
pub struct Loaded { pub buffer: Buffer, pub info: FileInfo }
pub enum LoadError { NotFound, IsDirectory, TooLarge { bytes: u64, max: u64 }, StillChanging,
                     Cancelled, Io(String) }                     // Display, Error
pub fn load_path(path: &Path, opts: &LoadOptions, progress: &dyn Fn(u64, u64),
                 cancel: &AtomicBool) -> Result<Loaded, LoadError>;
pub fn load_reader(reader: &mut dyn Read, total: Option<u64>, opts: &LoadOptions,
                   progress: &dyn Fn(u64, u64), cancel: &AtomicBool) -> Result<Loaded, LoadError>;
pub fn load_bytes(bytes: &[u8], opts: &LoadOptions) -> Loaded;

// buffer
pub struct Position { pub line: usize, pub col: usize }         // col in chars, tab = 1
pub struct Buffer { /* rope, revision */ }
impl Buffer {
    pub fn from_rope(rope: Rope) -> Buffer;  pub fn from_text(text: &str) -> Buffer;
    pub fn rope(&self) -> &Rope;             pub fn snapshot(&self) -> Rope;
    pub fn revision(&self) -> u64;           pub fn len_bytes(&self) -> usize;
    pub fn len_lines(&self) -> usize;        pub fn line_start(&self, line: usize) -> usize;
    pub fn line_range(&self, line: usize) -> Range<usize>;            // break excluded
    pub fn line_range_with_break(&self, line: usize) -> Range<usize>;
    pub fn line_of(&self, offset: usize) -> usize;
    pub fn line_text(&self, line: usize, out: &mut String);
    pub fn text(&self, range: Range<usize>, out: &mut String);
    pub fn chunks(&self, range: Range<usize>) -> impl Iterator<Item = &str> + '_;
    pub fn char_at(&self, offset: usize) -> Option<char>;
    pub fn is_char_boundary(&self, offset: usize) -> bool;
    pub fn prev_grapheme(&self, offset: usize) -> usize;
    pub fn next_grapheme(&self, offset: usize) -> usize;
    pub fn snap_to_grapheme(&self, offset: usize) -> usize;
    pub fn position(&self, offset: usize) -> Position;
    pub fn offset(&self, pos: Position) -> usize;                       // clamped
}

// display
pub struct DisplayLine { pub text: String, pub start_col: usize, pub columns: usize,
                         pub source_start: usize, /* private: map */ }
impl DisplayLine {
    pub fn new() -> DisplayLine;
    pub fn build(&mut self, source: &str, source_start: usize, start_col: usize, tab_width: usize);
    pub fn to_display(&self, source: usize) -> usize;
    pub fn to_source(&self, display: usize, round_up: bool) -> usize;
    pub fn column_of(&self, source: usize) -> usize;
    pub fn offset_at_column(&self, col: usize) -> usize;
}
pub enum Width { Narrow, Wide, Zero }
pub fn width_of(c: char) -> Width;                                 // unicode-width classes
pub struct Cols { pub col: usize, pub wide: usize, pub zero: usize }
impl Cols { pub fn after(self, text: &str, tab_width: usize) -> Cols }
pub struct ChunkStart { pub offset: usize, pub col: usize, pub wide: usize, pub zero: usize }
impl ChunkStart { pub fn cols(&self) -> Cols }
pub struct ChunkIndex { /* starts found so far, line end, complete */ }
impl ChunkIndex {
    pub fn new(buf: &Buffer, line: usize) -> ChunkIndex;
    pub fn starts(&self) -> &[ChunkStart];  pub fn is_complete(&self) -> bool;
    pub fn extend_while(&mut self, buf: &Buffer, tab_width: usize, more: impl FnMut(&ChunkStart) -> bool);
    pub fn extend_to(&mut self, buf: &Buffer, tab_width: usize, offset: usize);
}
pub fn chunk_starts(buf: &Buffer, line: usize, tab_width: usize) -> Vec<ChunkStart>; // whole line
pub fn columns(text: &str, start_col: usize, tab_width: usize) -> usize;

// selection
pub struct Selection { pub anchor: usize, pub caret: usize, pub goal_col: Option<usize> }
impl Selection { pub fn at(offset: usize) -> Selection; pub fn range(&self) -> Range<usize>;
                 pub fn is_empty(&self) -> bool }
pub enum Motion { Left, Right, WordLeft, WordRight, Home, End, Up(usize), Down(usize),
                  DocStart, DocEnd }
pub enum Unit { Grapheme, Word, Line }
pub fn apply(buf: &Buffer, sel: Selection, motion: Motion, extend: bool, tab_width: usize) -> Selection;
pub fn word_at(buf: &Buffer, offset: usize) -> Range<usize>;
pub fn line_at(buf: &Buffer, offset: usize) -> Range<usize>;
pub fn select_all(buf: &Buffer) -> Selection;
pub fn drag_to(buf: &Buffer, origin: Range<usize>, unit: Unit, offset: usize) -> Selection;

// find
pub struct FindOptions { pub match_case: bool }
pub struct Query { /* text, options */ }
impl Query { pub fn new(text: &str, options: FindOptions) -> Option<Query>;
             pub fn text(&self) -> &str; pub fn options(&self) -> FindOptions }
pub struct Matches { pub ranges: Vec<Range<usize>>, pub capped: bool, pub cancelled: bool }
pub fn find_all(rope: &Rope, query: &Query, cap: usize, cancel: &AtomicBool) -> Matches;
pub fn find_next(rope: &Rope, query: &Query, from: usize, forward: bool, wrap: bool,
                 cancel: &AtomicBool) -> Option<Range<usize>>;
pub fn first_at_or_after(ranges: &[Range<usize>], offset: usize) -> usize;
```

Invariants: every offset a function returns is a `char` boundary, and every caret or selection end
a grapheme boundary; functions taking an offset clamp it to `len_bytes()` and treat an offset
inside a `char` as that `char`'s start; `Buffer` never changes in phase 1 (`revision` stays 0; a
reload makes a new `Buffer`). The revision the shell tags jobs with is its own per-document counter,
bumped on every reload (and, from phase 2, on every edit).

### Public API of `mdshared_highlight::incremental` and `mdedit-highlight`

```rust
// mdshared_highlight::incremental
pub enum Language { Syntax(&'static SyntaxReference), Toml }      // Copy, PartialEq, Debug
impl Language { pub fn for_path(path: &Path, first_line: &str) -> Option<Language>;
                pub fn by_token(token: &str) -> Option<Language>;
                pub fn name(self) -> &'static str; pub fn limits(self) -> Limits }
pub struct Limits { pub max_line_chars: usize, pub max_file_bytes: usize }
impl Limits { pub const CODE: Limits;      // 1,000 chars, 2 MiB
              pub const MARKDOWN: Limits;  // 200 chars, 300 KiB
              pub fn allows_file(self, bytes: usize) -> bool }
pub struct Span { pub start: u32, pub end: u32, pub color: [u8; 3] }  // bytes within the line
pub trait Lines { fn len_lines(&self) -> usize; fn line(&self, index: usize, out: &mut String); }
pub fn foreground(dark: bool) -> [u8; 3];
pub fn checkpoint_spacing(lines: usize) -> usize;                  // max(16, lines / 3000)
pub struct LineHighlighter { /* private */ }
impl LineHighlighter {
    pub fn new(language: Language, dark: bool, total_lines: usize, total_bytes: usize) -> Self;
    pub fn language(&self) -> Language;  pub fn dark(&self) -> bool;  pub fn is_plain(&self) -> bool;
    pub fn set_dark(&mut self, dark: bool);
    pub fn frontier(&self) -> usize;     pub fn done(&self) -> bool;
    pub fn advance<L: Lines + ?Sized>(&mut self, src: &L, deadline: Instant) -> bool;
    pub fn spans<L: Lines + ?Sized>(&mut self, src: &L, lines: Range<usize>, deadline: Instant)
        -> Option<Vec<Vec<Span>>>;
    pub fn invalidate_from(&mut self, line: usize, total_lines: usize, line_delta: isize);
}

// mdedit_highlight
pub use mdshared_highlight::incremental::{Language, Limits, Span, foreground};
pub type DocId = u64;
pub struct RopeLines<'a>(pub &'a Rope);                             // impl Lines
pub struct Colored { pub doc: DocId, pub revision: u64, pub dark: bool, pub first_line: usize,
                     pub lines: Vec<Vec<Span>> }
pub struct HighlightService { /* private */ }
impl HighlightService {
    pub fn spawn(wake: impl Fn() + Send + 'static) -> HighlightService;
    pub fn open(&self, doc: DocId, revision: u64, text: Rope, language: Language, dark: bool);
    pub fn view(&self, doc: DocId, revision: u64, lines: Range<usize>);
    pub fn set_dark(&self, dark: bool);
    pub fn close(&self, doc: DocId);
    pub fn take(&self) -> Vec<Colored>;
    pub fn wait(&self, deadline: Instant) -> Vec<Colored>;
}                                                                   // Drop stops the worker
```

Invariants: a line's spans are sorted, contiguous from 0 to the line's byte length and never split
a `char`; an empty `Vec` means "draw in `foreground`"; the colours of a line computed from a
checkpoint equal those of a full pass from the top; the theme is syntect's `InspiredGitHub` (light)
and `base16-ocean.dark` (dark), as for the viewer's code blocks. Since a `DocId` is reused across a
reload, the shell passes a new revision to `open` on reload (a counter per document) and drops
`Colored` results whose revision differs.

### Modules of the `mdedit` binary

Internal to package C, listed so the layout is known: `main.rs`, `app.rs` (loop, compose, present,
`--shot`, `run`), `app/input.rs`, `app/commands.rs`, `args.rs` (the `Args` struct is fixed in the
stub), `commands.rs` (command table and strings), `keys.rs`, `docs.rs` (tabs and their documents,
load and find jobs), `ui.rs` (regions, hits, drags, popups, dialogs, panes), `chrome.rs`,
`panes.rs`, `overlays.rs`, `status.rs`, `view.rs` (and `view/*.rs` if it is split), `session.rs`,
`platform.rs` (`GetACP`, memory and CPU figures; the only `unsafe` in the crate), `trace.rs`.

## 2. Decisions

| Question | Decision | Why |
|---|---|---|
| Buffer | `ropey` 1.6.1, default features off, `simd` + `cr_lines` | Spike "buffer": O(log n) whatever the edit history, cheap snapshots, 74 ms load and +19.6% memory at 100 MB. `cr_lines` breaks lines at LF, CRLF and CR, as Notepad does; `unicode_lines` (VT, FF, NEL, LS, PS) stays off |
| Positions | Byte offsets into the UTF-8 buffer, always on a `char` boundary; caret and selection ends on grapheme boundaries | parley, syntect and find all speak bytes. ropey's char indices appear only inside `mdedit-core` |
| Lines | 0-based in code, 1-based on screen; the last line after a final line break exists and is empty | What ropey and every editor do |
| Columns (status "Col") | `char`s from the line start, a tab counts 1, plus 1 | Notepad's "Col" |
| Display columns (Up/Down goal) | Tabs to stops of 4, every other `char` 1 | Monospace; on CJK lines Up and Down keep the `char` column, not the drawn x |
| Horizontal extent and chunk x | Columns plus a count of wide (East Asian wide and fullwidth, 2 columns) and zero-width (combining marks, joiners, variation selectors, 0) characters, from `unicode-width`; x = narrow columns times the advance plus wide characters times the measured width of a wide glyph | A CJK line's extent and its chunks' x match the glyphs; other fallback glyphs and emoji sequences stay an estimate |
| Loading | On a worker thread, streamed into a `RopeBuilder`, never the whole file in memory beside the rope | Section 5 |
| Wrap | None in phase 1; horizontal scrolling | Plan: word wrap is phase 4 |
| Long lines | A line of at most 4 KB of display text is shaped whole; a longer one in chunks of about 1 KB, only the chunks on screen plus one each side; the chunk starts are found from the line start only as far as the view, caret or click needs, and kept for the 256 lines used last | Spike "longlines": cost and memory then do not depend on line length |
| Highlighting | syntect through `mdshared-highlight::incremental`, checkpoints every K = max(16, lines / 3000) lines, on a worker thread in time-boxed slices; lines over 1,000 chars (200 for Markdown) and files over 2 MiB (300 KiB for Markdown) stay plain | Spike "highlight" |
| Font | "Consolas, Cascadia Mono, Noto Sans Mono, DejaVu Sans Mono, monospace" (the viewer's `mono`), from the system, 14 logical px at zoom 100; no bundled fonts | The viewer's rule since 2026-11-08 |
| Find | Literal; case-insensitive by default with the viewer's folding; Match case as an option; matches capped at 10,000; on the UI thread up to 4 MiB of text, else on a worker | Section 9 |
| Invalid bytes | Never repaired silently: replaced by U+FFFD for display, the file marked `lossy` (read-only in every phase) and named in the status area and a toast | Plan, "Encodings and line endings" |
| Panics | `panic = "abort"` in the release profile | Nothing catches a panic, so unwinding buys only a dead worker thread instead of an ended process; the unwind tables cost about 0.9 MB. The release exe is 5,699,584 bytes with `chardetng` and `unicode-width` added, against the 6 MB budget |
| Fonts the system fallback lacks | The view's own font context puts Microsoft YaHei, Microsoft JhengHei and SimSun-ExtB behind the system's Han fallback, and Segoe UI Symbol and Segoe UI Emoji behind the common-script and Latin ones; the system family stays first | The system's Han fallback has no glyph for some simplified characters (U+4F24), and there is none for musical symbols (U+1D11E); both drew as boxes |
| IME | Not enabled in phase 1 (`set_ime_allowed` stays false, as in the viewer) | Read-only; the spike "ime" notes are for phase 2 |
| Zoom | Fixed at 100 in phase 1; metrics are computed from a zoom value so phase 4 only adds commands | Plan: zoom is phase 4 |

## 3. Threading model

| Thread | Runs | Talks back by |
|---|---|---|
| UI (winit event loop) | Input, commands, layout of visible lines (parley), painting, present, selection, find on small buffers, choosing the language | (owns all state) |
| Loader, one per load job (`std::thread::spawn`) | `mdedit_core::load::load_path`, then `Language::for_path` on the loaded first line | `mpsc::Sender<LoadDone>` + `proxy.send_event(Wake::Loaded)` |
| Find, one per search job on buffers over `SYNC_FIND_BYTES` | `mdedit_core::find::find_all` / `find_next` on a rope snapshot | `mpsc::Sender<FindDone>` + `Wake::Found` |
| Highlight worker, one per window, long-lived (`mdedit_highlight::HighlightService`) | `LineHighlighter::spans` and `advance` over rope snapshots | its own channel + the `wake` closure, which sends `Wake::Highlight` |
| Single-instance listener (`mdshared_app::single::listen_waking`) | Paths handed over by a second launch | `Wake::Instance` |
| File watcher (`mdshared_app::watch::FileWatcher`, notify's thread) | Change notices for the open files' folders | `Wake::Files` |

Rules:

- `EventLoopProxy<Wake>` is cloned into each closure; `Wake` carries no data, the data travels on
  `mpsc` channels the UI thread drains in `user_event`. `Wake` is
  `enum Wake { Instance, Files, Loaded, Found, Highlight }`.
- Every job result carries the document id and the buffer revision (and, for find, the query and
  options) it was computed for; the UI drops results that no longer match.
- A job is cancelled through an `Arc<AtomicBool>` that the core checks at least every 64 KiB.
  Starting a new load or search for the same document cancels the old one.
- Worker threads get rope snapshots (`Buffer::snapshot`, an O(1) clone that shares chunks), never
  references to UI state.
- Zero idle CPU: the loop waits (`ControlFlow::Wait`) unless a timer is due. Timers exist only for
  the caret blink (while the window and the text or find box have focus), the tooltip delay, the
  toast's expiry, drag auto-scroll, and saving the window size after a resize. Worker threads block
  on their channels when they have nothing to do.
- `--shot` has no event loop: it calls the same functions and blocks on the channels directly.

## 4. From file bytes to pixels

1. **Open.** The shell assigns a `DocId` (a counter), adds a tab in the `Loading` state and starts a
   loader thread with `LoadOptions { legacy: encoding::legacy_for_code_page(GetACP()), max_bytes:
   MAX_FILE_BYTES }`.
2. **Load** (`load_path`, worker). Stamp the file; read 64 KiB at a time; sniff the BOM from the
   first bytes; decode each chunk to UTF-8 (UTF-8: validate with a carry of an incomplete trailing
   sequence; UTF-16: `encoding_rs` decoder; legacy: `encoding_rs` decoder); count line endings and
   NULs and track the longest line on the decoded text; append to a `ropey::RopeBuilder`. If UTF-8
   without a BOM turns out invalid, drop the builder, choose an encoding from the bytes around the
   first invalid one (section 6) and decode the whole file again with it. Stamp again; if the stamp
   changed, retry (three attempts). Result:
   `Loaded { buffer, info }`.
3. **Language** (same worker): `Language::for_path(path, first line)`.
4. **Ready** (UI). The tab gets the buffer, info and language; the view scrolls to the top (or to
   `--goto`); if the language exists and `language.limits().allows_file(len_bytes)`, the shell
   calls `service.open(doc, revision, buffer.snapshot(), language, dark)` and
   `service.view(doc, revision, visible lines ± one screen)`.
5. **Colour** (highlight worker). From the checkpoint at or before the first requested line, run the
   lines, return `Colored { doc, revision, dark, first_line, lines }`. The UI keeps one
   `LineColors` window per document (the last `Colored` received) and requests a new range when the
   visible lines leave the middle of the covered range.
6. **Layout** (UI, per visible line). Read the line's display text (`DisplayLine::build` from the
   buffer text of the line, or of one chunk), shape it with parley in the monospace family with a
   brush per colour span (span byte offsets mapped through `DisplayLine::to_display`), cache the
   `parley::Layout` keyed by (doc, line, chunk start, revision, colours generation). The cache holds
   about four screens of lines and is dropped on theme or scale change.
7. **Paint** (UI, into the composed screen bitmap). In order: background, current-line band,
   find marks, selection, glyphs (`mdshared_text::TextEngine::draw_layout_clipped`, the view's own
   engine with the system subpixel order), caret, gutter numbers, scrollbars.
8. **Present.** The regions that changed are copied to softbuffer's buffer and presented with
   damage rectangles, as in the viewer's `App::present`.

## 5. First frame, and the 100 MB file

**Decision:** the active document is loaded on a worker thread that starts in `run()` before the
event loop exists, in parallel with creating the window, the surface and the font context. The
hidden first frame waits for that load until `LOAD_WAIT` = 400 ms after launch, and for the colours
of the first screen until `COLOUR_WAIT` = 80 ms after launch, then is drawn and the window shown
(the viewer's hidden-first-frame sequence: present hidden, show, present again).

- If the load is done in time (expected for the 100 MB file on a warm cache), the first frame shows
  the text. If not, it shows the tab and a "Loading name" line with a progress figure in the text
  area and the status strip; the text replaces it when `Wake::Loaded` arrives.
- If colours are not ready by `COLOUR_WAIT`, the text is drawn in the theme's foreground
  (`incremental::foreground`) and the colours replace it when they arrive. Files over the limits
  (the 100 MB file) are never coloured, so nothing waits.

**Why, from the spike numbers.** Spike "buffer": ropey loaded the 100 MB file in 74 ms (71.5-89.7)
warm with `from_reader`, and the first 60-line window cost 9.9 us. The streamed decode adds a UTF-8
validation and a line-ending and longest-line pass over the decoded bytes, both linear scans that
should cost tens of ms at 100 MB (not measured yet; package A measures them). Spike "ime": window
creation about 37 ms; `shared_fonts`: about 35 ms of font enumeration. Run in parallel, the
expected first frame for the 100 MB file is roughly 120-200 ms warm, inside the 500 ms budget with
room for a cold read from an SSD (cold reads were not measured anywhere). Loading on a worker
instead of before the window costs nothing on that path and keeps the window responsive when a
disk is slow or a file is large: the user gets a window with progress by 400 ms instead of a
process that shows nothing. The same path serves files opened later, where blocking the event loop
is not acceptable. The colour wait is short because syntect's first lines of a syntax cost 16-48 ms
of lazy regex compilation (spike "highlight", section 1) and the small-file budget is 100 ms; 80 ms
lets a typical start show colours at once without spending the budget on them.

Only the active tab loads at start; other restored tabs load when first activated and then stay
loaded.

**Memory for the 100 MB file.** The budget "about file size plus 25%" is read as: private bytes with
the 100 MB file minus private bytes with the small file, at most 125 MiB. What counts toward it:
the rope (spike: +19.6% over the file, about 120 MiB), nothing proportional to the line count in
the shell (all lines are one height, so no per-line arrays), at most `FIND_CAP` match ranges, no
highlighting (over the limit), layouts for visible chunks only. Peak during the load is the rope
plus 64 KiB plus the decoder's buffers, because the file's bytes are never read whole.

**Scroll positions** are `f64`: the vertical one in lines (fractional, for touchpad deltas), the
horizontal one in logical px. An `f32` in px loses whole pixels past about 16.7 million, which the
100 MB file (1.85 million lines at 19 px) passes.

## 6. Encodings

Detection, in order:

1. A byte order mark: `EF BB BF` is UTF-8 with BOM, `FF FE` UTF-16 LE, `FE FF` UTF-16 BE. The mark
   is not put into the buffer. (A file starting `FF FE 00 00` is read as UTF-16 LE; UTF-32 is not
   detected.)
2. No mark: UTF-8 if the bytes are valid UTF-8 from start to end (an empty file is UTF-8).
3. Otherwise `DETECT_BYTES` (1 MiB) around the first invalid byte are read again and counted. If
   they hold valid multi-byte UTF-8 sequences and at most one invalid byte for every four of
   those, the file is UTF-8 with invalid bytes (read as UTF-8, invalid sequences become U+FFFD,
   `lossy`).
4. Otherwise `chardetng` guesses a legacy encoding from the same bytes, with the system's code page
   (`GetACP()` mapped by `encoding::legacy_for_code_page`) as the hint: 1254 hints Turkish, 1251
   Cyrillic, and so on. When the guess is not sure, the system's code page is used
   (windows-1252 when the code page is not one of 874, 932, 936, 949, 950, 1250-1258, and on other
   systems). There is no command to choose an encoding by hand in phase 1.

Invalid input: after a UTF-8 BOM, or in a file chosen as UTF-8 in step 3, invalid sequences become
U+FFFD; in UTF-16, unpaired surrogates and an odd final byte become U+FFFD. Either sets
`FileInfo::lossy`. A legacy decode is encoded back as it goes and compared with the file's bytes;
bytes that do not come back the same (a multi-byte page's unmapped pairs, which `encoding_rs`
decodes to U+FFFD or to a character that encodes differently) set `lossy` too. Single-byte code
pages map every byte, so they never set it. NUL characters are kept (shown as control pictures)
and set `has_nul`; the status area's encoding field then adds ", contains NUL" and the first
showing of the tab gives a toast: "name contains NUL characters and may not be a text file."

Status area, the encoding field: `Encoding::label()`, that is "UTF-8", "UTF-8 with BOM",
"UTF-16 LE", "UTF-16 BE", or the code page's name ("Windows-1254"). A lossy file shows
"UTF-16 LE, invalid bytes replaced" and, when its tab is first shown, a toast:
"Some bytes in name could not be read and are shown as \u{FFFD}." (with the replacement character
itself in the text).

## 7. Line endings

The buffer keeps every break as the file had it (ropey with `cr_lines` treats LF, CRLF and CR each
as one break; a CRLF is never split by a line boundary). `EolCounts` counts CRLF, LF and lone CR
during the load, on the decoded text, with a CR at the end of one chunk paired with an LF at the
start of the next. `LineEnding` is `Crlf`, `Lf` or `Cr` when only one kind occurs, `Mixed` when
more than one does, `None` when there is no break. Status labels: "Windows (CRLF)", "Unix (LF)",
"Macintosh (CR)", "Mixed", and for `None` the platform default ("Windows (CRLF)" on Windows).

## 8. Selection model

`Selection { anchor, caret, goal_col }` in byte offsets, both ends on grapheme boundaries
(unicode-segmentation's `GraphemeCursor` over rope chunks; a CRLF is one grapheme). The caret is
drawn at `caret`; `range()` is the ordered pair. Phase 1 has a caret even though nothing is
editable: it shows where keyboard selection starts and what the status position means.

- Keys (text focused): Left, Right, Ctrl+Left, Ctrl+Right (words: letters, digits and `_`;
  whitespace; other; a word longer than `WORD_LIMIT`, 64 KiB, ends there, so a click or a key on a
  long line without spaces costs no more than that), Home (smart: first non-blank, then column 0),
  End, Up, Down (goal display column kept), Page Up and Page Down (by the visible line count less one), Ctrl+Home, Ctrl+End;
  each with Shift to extend. Without Shift, Left or Right on a selection collapses it to its start
  or end. Ctrl+A selects all.
- Mouse: press places the caret; Shift+press extends; drag selects, with auto-scroll when the
  pointer is above, below, left or right of the text area (only on the axes it is outside on);
  inside the text area a drag keeps the caret in view without the keyboard's margin, so the text
  does not move under the pointer; double click selects a word and the drag then grows by words;
  triple click selects the line with its break and the drag grows by lines.
- Copy: Ctrl+C or Ctrl+Insert copies the selection as it is in the buffer (its line breaks
  unchanged) with `mdshared_ui::platform::set_clipboard_text`. An empty selection copies nothing.
  Over `COPY_LIMIT` (256 MiB) a toast says the selection is too large to copy.
- Typed characters, Backspace, Delete, Enter and Ctrl+V do nothing in phase 1.

## 9. Find

The viewer's find box in the command bar (`mdshared_ui::textfield::TextField`, `FIND_W` 260 px),
with the counter, Previous and Next buttons and the Clear button. Right-click on the box opens its
editing menu with a checkable "Match case" item.

- Paste (Ctrl+V or the menu) puts the first line of the clipboard in the box, at most 1,000
  characters; only that much of the clipboard is read.
- Query: literal; case-insensitive by default with the viewer's folding
  (`mv/crates/core/src/find.rs`, `fold` and `chars_match`, copied into `mdedit-core`); a line break
  in the query matches any one line break in the text. Empty query: no matches, no counter.
- Matching walks the rope's chunks (`Buffer::chunks`), never copies the buffer, and handles
  matches that cross a chunk boundary. Matches do not overlap.
- Up to `SYNC_FIND_BYTES` (4 MiB) of text the search runs on the UI thread at every change of the
  query. Larger buffers search on a find thread; the counter shows "Searching" until it answers.
  Results carry (doc, revision, query, options).
- At most `FIND_CAP` (10,000) matches are kept; the counter then reads "10000+ matches". F3 past the
  last kept match runs `find_next` from the caret (on the find thread for large buffers).
- The current match is the first at or after the caret when the query changes. Enter or F3 goes to
  the next, Shift+Enter or Shift+F3 to the previous, wrapping. Going to a match selects it and
  scrolls it into view (vertically centred if it was off screen). Escape returns focus to the text
  and keeps the query and its marks; the Clear button clears both.
- Counter texts follow the viewer's `strings::find_label`: "No matches", "N matches", "I of N",
  with "+" after N when capped.
- Marks: every match on a visible line in `pal.find_mark` at about 0.45 alpha, the current one in
  `pal.find_current` at about 0.6, under the glyphs.

## 10. The text view

**Metrics.** Font size `14 * zoom / 100` logical px (`zoom` fixed at 100 in phase 1), times the
scale for device px. The advance is measured once per size by shaping "0000000000" and dividing by
10 (spike: 7.697 px at 14 px). The line height is `round(1.35 * size)` device px (19 at 100%), an
integer so line tops never drift; line `i` sits at `(i - top) * line_h` from the text area's top.
The baseline is centred in the line by the font's ascent and descent.

**Gutter.** Line numbers, 1-based, right-aligned, in the monospace face at the text size;
`digits = max(3, digits of len_lines)`; width `digits * advance + 16 + 14` logical px (left and
right padding). Numbers in `pal.faint`, the caret line's in `pal.text`. Same background as the
text. The gutter does not scroll horizontally.

**Text area.** Left padding 6 logical px after the gutter. Background `pal.page`. The caret line
gets a band of `mix(pal.page, pal.text, 0.035)` when the selection is empty. Uncoloured text uses
`incremental::foreground(dark)`. Selection fill `pal.selection` at about 0.35 alpha (the text
field's look); a selected line break shows as half an advance of selection after the line's end.
The caret is `max(1, round(1.5 * scale))` device px wide, line height tall, in `pal.text`, blinking
at `platform::caret_blink_time()` while the window and the text area have focus.

**Lines.** For each visible line: if its display text is at most `WHOLE_LINE_BYTES`, one layout of
the whole line. Otherwise a `display::ChunkIndex` gives chunks; the shell keeps one per line for the
256 lines used last, per document, dropped on reload, and extends it only as far as needed. A
chunk's x is its narrow columns times the advance plus its wide characters times the width of a
wide glyph (measured once per size by shaping ten Han characters); zero-width characters add
nothing. That matches the glyphs for ASCII and CJK text in the fonts used; glyphs from other
fallback fonts and emoji sequences keep it an estimate, so seams may gap or overlap by a few px
there. Only chunks whose estimated x range overlaps the horizontal view, plus one each side, are
built and shaped. The widest line is raised from each drawn chunk's x plus its shaped width.
Control characters and tabs are drawn through `DisplayLine`. Layouts are cached under the revision
the text was loaded at, so after a reload none of the old ones is drawn.

**Hit testing.** y gives the line (`top + y / line_h`, clamped); x gives the chunk (the last that
starts at or before x) and then the byte through parley's cursor hit test in that layout, mapped
back with `DisplayLine::to_source`; positions snap to grapheme boundaries.

**Scrolling.** Vertical scroll is `top: f64` in lines, clamped so the last line can reach the bottom
edge and no further. Wheel: 3 lines per notch; pixel deltas (touchpads) are converted by the line
height. Shift+wheel and horizontal wheel scroll horizontally by 3 advances per notch. Horizontal
scroll `left: f64` in logical px, clamped to `extent - view width`, where `extent` is
`(longest_line_chars + 2) * advance` from the load, raised to the widest line actually shaped.
Keyboard moves keep the caret in view with a margin of 2 lines and 4 advances (a mouse drag keeps
it in view with none), using the caret's estimated x, so a caret after wide characters is revealed
where it is drawn.

**Scrollbars.** The vertical one is `mdshared_ui::scroll::scrollbar` / `draw_scrollbar` with content
and view in lines. The horizontal one is the same geometry turned on its side, written in the view
module (12 px band at the bottom of the text area, shown only when `extent > view width`). Both are
draggable, like the viewer's.

**Repaint.** The text region repaints whole on scroll, resize, theme, selection, colours or find
changes. A caret blink, or a caret move by keyboard with no selection change and no scroll,
repaints only the line bands involved (the gutter number included), which is also what phase 2's
keystroke budget needs.

## 11. The shell around it

Regions (each with a signature, redrawn only when it changes, as in the viewer): `Title` (logo,
tabs, caption buttons), `Bar` (command bar), `Files` (optional pane on the left), `Text` (gutter,
text, scrollbars, or the empty state), `Status` (26 logical px strip across the window's bottom).
Overlays: menus, dialogs, toast, tooltip.

- **Title row**: as the viewer's (`mdshared_ui::chrome`): the mdedit mark (the green tile
  `#1d7d6b` with the M and pencil from `assets/mdedit.svg`, drawn with `Painter::polygon`), tabs
  labelled by `mdshared_app::paths::tab_labels`, the new-tab button (opens files), caption buttons,
  drag area, resize bands (`mdshared_ui::window::edge_at`). Window title: the file name and
  "mdedit", joined as the viewer's `strings::window_title` joins them.
- **Command bar**: Open, Reload, Recent, Files (toggle), the find box with counter and Previous and
  Next, Theme, More (main menu). It degrades with width as the viewer's `bar_plan` does.
- **Files pane**: `mdshared_ui::tree::FileTree` with a filter that shows every file, the viewer's
  pane head, rows, splitter and scrollbar.
- **Status strip**: right-aligned fields with 24 px gaps: "Ln L, Col C" (plus " (N selected)",
  N in characters up to 4 MiB of selection, else in bytes, "12.4 MB"), the language name or
  "Plain text" (plus ", not coloured" when over the size limit), the line-ending label, the
  encoding label. Left side: "Loading N%" while a load runs. Background `pal.bar`, hairline on top.
- **Menus**: main (Open files, Open folder, Recent, Reload, Close tab, Close all tabs, Theme, Files
  pane, Keyboard shortcuts, About, Exit), Recent, tab (Close tab, Close other tabs, Copy file path,
  Show in File Explorer), text (Copy, Select all), find box (Cut, Copy, Paste, Select all, Match
  case).
- **Dialogs**: About, Keyboard shortcuts (from the command table), Manage recent files, with the
  viewer's card, footer, buttons and keycaps (`mdshared_ui::overlay`).
- **Empty state** (no tab): the viewer's: an Open button and the recent files.
- **Command table** (`bins/mdedit/src/commands.rs`): Open files Ctrl+O, Open folder Ctrl+Shift+O,
  Reload F5, Close tab Ctrl+W and Ctrl+F4, Close all tabs Ctrl+Shift+W, Exit Ctrl+Q, Copy Ctrl+C and
  Ctrl+Insert, Select all Ctrl+A, Find Ctrl+F, Find next F3, Find previous Shift+F3, Files pane
  Ctrl+Shift+1, Next tab Ctrl+Tab, Previous tab Ctrl+Shift+Tab, Escape; Theme (System, Light,
  Dark), Keyboard shortcuts, About, Close other tabs, Copy file path, Show in File Explorer without
  keys. No Ctrl+Alt or AltGr shortcuts (spike "ime", pitfall 1).
- **Session**: `%LOCALAPPDATA%\mdedit\settings.ini` (`mdshared_app::settings::default_settings_path
  ("mdedit")`), or the file named by `MDEDIT_SETTINGS`. Section `[mdedit]`, keys `tab` (repeated,
  `encode_path`), `active`, `recent` (repeated), `theme` (0 system, 1 light, 2 dark), `geometry`
  (`WxH[ max][ @x,y]`, the viewer's format and limits), `files` (0 or 1), `files_w`, `folder`.
  Written with `settings::Writer` and `write_atomic` whenever what it holds changes (checked at
  the end of each pass of the event loop, not during a drag), so a shutdown or an ended process
  loses nothing, and on exit. While the window is maximized the saved size stays the restored
  one, with ` max` added.
- **Single instance**: `mdshared_app::single` with the app name "mdedit": a second launch hands its
  paths to the running window and exits.
- **Changes on disk**: `FileWatcher` over the open files. A file whose stamp changed is reloaded in
  place (phase 1 has nothing unsaved), keeping the caret's line and column and the scroll, clamped.
  A file that is gone keeps its text and gets the missing mark on its tab.
- **Theme**: System, Light, Dark; `mdshared_ui::theme::{LIGHT, DARK}` for the chrome; the syntect
  theme follows (`service.set_dark`).

## 12. Command line

`mdedit [flags] [files]`. Unknown flags and bad values are reported on stderr and ignored.

| Flag | Meaning |
|---|---|
| `--light`, `--dark` | Theme for this run |
| `--no-session` | Do not read or write the session; no single-instance hand-over |
| `--files`, `--no-files` | Show or hide the Files pane |
| `--folder <dir>` | The Files pane's folder |
| `--maximize` | Start maximized |
| `--goto <line[:col]>` | Caret at that 1-based position of the first file, scrolled into view (centred) |
| `--select <l:c>-<l:c>` | Select from the first position to the second |
| `--find <text>` | Put the text in the find box and go to the first match |
| `--match-case` | Find with Match case on |
| `--scroll <px>` | Scroll the text down this many logical px (after `--goto`) |
| `--hscroll <px>` | Scroll the text right this many logical px |
| `--shot <out.png>` | Render one state to a PNG without a window and exit |
| `--size WxH` | Logical size for `--shot` (default 1280x800) |
| `--scale <f>` | Scale for `--shot`, 0.5 to 4 (default the system's) |
| `--menu main\|recent\|tab\|text\|find` | Open that menu in the shot |
| `--dialog about\|shortcuts\|recent` | Open that dialog in the shot |
| `--hover x,y` | Pointer position in the shot (tooltips show at once) |
| `--trace` | Print timings and memory on stderr (below) |
| `--scroll-steps N` | After the first frame, the load and the first screen's colours, scroll N wheel steps (down, reversing at the ends), print frame-time statistics, exit. `0` exits at that point |

In a windowed run, `--goto`, `--select`, `--find`, `--scroll` and `--hscroll` apply when the first
file has loaded and the window has a size, in the order below.

A shot never touches the user's session (it may use `MDEDIT_SETTINGS`), loads every file
synchronously, waits up to 10 s for the colours of the visible lines, draws the caret in its
visible phase, and is deterministic: two runs give identical PNGs. Order of application: load,
`--goto`, `--select`, `--find`, `--scroll`, `--hscroll`, `--menu`, `--dialog`, `--hover`.

`--trace` lines start `mdedit:` and give milliseconds since launch for: arguments parsed, session
read, load started, event loop, window created, surface, load done (size, ms, encoding, line
endings), first colours, first frame presented hidden, window shown, "first frame after X ms";
private bytes and peak working set at the first frame and again when loading and highlighting have
gone idle; process CPU time at that idle point and at exit (for the idle-CPU check); then one line
per presented frame (layout, paint, compose, copy, present ms, rows). With `--scroll-steps`, a
summary: steps, p50, p95 and max of the frame times.

## 13. What phase 2 adds, and where

- `Buffer::replace(range, &str) -> Edit` (`Edit { at, removed: String, inserted: String,
  lines_removed, lines_inserted }`), which bumps `revision`; undo and redo store `Edit`s.
- `LineHighlighter::invalidate_from` exists and is tested in phase 1; phase 2 adds
  `HighlightService::edit(doc, revision, text, first_line, line_delta)` and the synchronous re-run
  of the edited line from a small cache of exact states (spike "highlight", recommendation 2).
- The layout cache is keyed by revision, and the view already repaints single line bands.
- `FileInfo` keeps the encoding, BOM, line-ending counts and `lossy` that phase 3's save needs.
- The IME notes of spike "ime" apply when the text area gets `set_ime_allowed(true)`.

Not in phase 1: editing, saving, undo, IME, word wrap, zoom commands, a go-to-line dialog (only
`--goto`), replace, regular expressions, an outline pane, focus mode, the viewer hand-off, Linux.

## 14. Test plan

Each package's own tests are listed with the package (section 15). Across the phase:

1. `cargo test --workspace` in this repository, and `cargo test --workspace` in
   `markdown-shared-rs`, both pass; `cargo fmt --all --check` is clean in both.
2. The viewer's no-pixel-change check passes after the change to `mdshared-highlight` (the viewer
   is built against the shared crates by path).
3. Budgets, measured on the Windows PC with `--trace` (release build, launched from Git Bash, five
   runs each, medians reported with ranges, the machine's load noted):
   - first frame: small file (`fixtures/sample.rs`) under 100 ms; 100 MB generated file under
     500 ms (`--scroll-steps 0`);
   - private memory: small file under 30 MiB; 100 MB file at most 125 MiB above the small file;
   - scroll: `--scroll-steps 300` at 1280x800, p95 frame under 4 ms, on the small file, a
     50,000-line Rust file and the 100 MB file;
   - idle CPU: no CPU time between the idle point and exit after 10 s idle with the window
     unfocused;
   - exe size, for the plan's 6 MB line.
4. The user runs the build and approves the interface by looking at it. Shots and tests do not
   stand in for that; reports say which checks ran and which did not.

## 15. Work packages

Three packages with no file in common. Each implementer gets their package text and this file.
Interfaces between packages are the stubs committed with this design; changing a signature means
telling the other packages (and this file).

| Package | Owns |
|---|---|
| A, headless core | `crates/mdedit-core/**` |
| B, highlighting | `markdown-shared-rs/crates/mdshared-highlight/src/incremental.rs`, `crates/mdedit-highlight/**` |
| C, shell | `bins/mdedit/**`, `fixtures/**` |

Files nobody edits in phase 1 without agreement: the root `Cargo.toml`, `Cargo.lock` (cargo
updates it; commit what it writes), `.gitignore`, `.gitattributes`, `docs/**`,
`markdown-shared-rs/crates/mdshared-highlight/src/lib.rs` (holds only the `pub mod incremental;`
line added for this design) and every other shared file.

## 16. Phases 2 and 3: editing and saving (2026-11-13)

Built as section 13 said, with these decisions. Nothing here is approved by the user yet.

**Core (`mdedit-core`).**

- `Buffer::replace(range, text) -> Change` is the only way the text changes. `Change` says what
  the shell's caches need: the byte range, the first line touched, how many lines it was and
  became, and the line-ending counts removed and added (so `FileInfo::eol` stays exact without a
  rescan).
- `edit.rs` holds the commands (insert, newline with auto-indent, backspace, delete, delete word
  left and right, indent, outdent, undo, redo). Each takes the buffer, the history and the
  selection and returns the new selection and one merged `Change`.
- `history.rs`: undo steps are lists of (position, removed, inserted). Typing merges into one step
  until the caret moves, a command runs, the focus leaves or a word ends; Backspace and Delete
  runs merge likewise. Removed text over 4 KiB is kept as a rope slice that shares the buffer's
  chunks. Held text is capped at `UNDO_BYTES` (64 MiB): the oldest steps go first, the newest
  never. `is_modified` is by history position against the save point, not by comparing text.
- `save.rs`: `save_path` encodes the rope in the file's encoding (byte order mark included, no
  line ending touched) into a temporary file beside the target, flushes and syncs it, then puts
  it in place with `ReplaceFileW`, which keeps the target's attributes, ACL, creation time and
  alternate streams (`rename` where that is not supported, and on other systems). It refuses when
  the file's stamp is not the one last seen (`ChangedOnDisk`), when the target is read-only, and
  when the encoding cannot hold a character (`Unencodable`, with line and column); in each case
  the target is untouched and no temporary file is left. That call is the crate's one `unsafe`
  block.

**Shell.**

- Every edit goes through `Docs::edit`, which bumps the document's revision and keeps in step:
  the line-ending counts, the colours (lines below move with the change; the edited line keeps
  its spans, shifted, so typed text takes the colour it lands in until the worker answers; other
  changed lines go plain), the find marks (moved with the text, then searched again 300 ms after
  the last edit), the long-line chunk indexes (dropped), and the highlight worker
  (`HighlightService::edit`: new rope, first changed line, line delta).
- The layout cache is keyed by what a layout shows (hash of the text, start column, colours,
  theme), not by revision and line number. An edit therefore re-shapes only lines whose text
  changed, and equal lines share one layout. Offsets inside a `Shaped` are relative to its text.
- Keys: typed text from `KeyEvent.text` (when Ctrl and Alt are both up or both down), Enter,
  Backspace, Delete, Ctrl+Backspace, Ctrl+Delete, Tab, Shift+Tab; commands Undo Ctrl+Z, Redo
  Ctrl+Y and Ctrl+Shift+Z, Cut Ctrl+X and Shift+Delete, Paste Ctrl+V and Shift+Insert, Save
  Ctrl+S, Save as Ctrl+Shift+S. Tab inserts a tab character. Enter and pasted line breaks use the
  document's most frequent line ending. Dead keys and AltGr arrive as `KeyEvent.text`; the IME
  stays off until phase 5.
- A file with replaced bytes (`lossy`) is read-only: typing shows a toast.
- Unsaved changes: a bullet before the name on the tab and in the window title. Closing a tab,
  other tabs, all tabs or the window asks for each modified file in turn (Save, Don't save,
  Cancel; Enter, N, Escape); Reload asks before dropping changes. A file that changes on disk
  while it has unsaved changes is not reloaded: a banner says so, and Save then asks before it
  overwrites.
- Saving runs on the UI thread (measured: 88 ms for 100 MB of UTF-8, 261 ms as UTF-16).
- `--type-steps N` types N keystrokes, one per frame, and prints the time from keystroke to
  pixels; `--type TEXT` types text for a shot; `--dialog unsaved` shows the question.

## 17. Updates

mdedit updates itself with the code mdview uses, all of it in `markdown-shared-rs`; the viewer's
`docs/UPDATE.md` describes the flow, the manifest and the swap. What is the editor's own:

| Where | What |
|---|---|
| `bins/mdedit/src/updater.rs` | `APP` (name, version, manifest address, no engine, no installer entry), `fetch` with the user agent `mdedit`, `--check-update` |
| `docs.rs` | `Docs::updates` (the shared `Updates`) and `Docs::check_updates` (the setting), so drawing code sees them |
| `session.rs` | `check_updates=0\|1` (default 1) and `update_checked=<seconds since 1970>` |
| `ui.rs` | `Areas::compute` takes the height of a bar under the command bar; it is part of the Bar region. `Hit::UpdateGo`, `Hit::UpdateClose` |
| `chrome.rs` `draw_update_bar` | Places the shared bar and records its hits |
| `overlays.rs` (About) | "Check for updates" (or "Update to x") and "Check at launch: on/off" |
| `app/commands.rs` ("updates") | `updates_at_launch`, `check_for_updates`, `install_update`: each starts a shared thread that posts `Wake::Update` |

The look at launch runs only with a session file and for a copy outside a cargo output folder
(`release`, `debug`, `deps`). `--update-bar available|installing|installed` shows the bar with a
made-up version for a shot.

New file: a document's path is optional. `Docs::new_untitled` makes a ready, empty "Untitled N"
document (UTF-8, the platform's line ending, no language until it has a name). It is left out of
the session, the watcher and reloads; `Docs::save` answers `SaveFailed::Untitled` and the shell
goes to Save as, where cancelling the file dialog also cancels whatever waited on the save.

Not built: choosing or converting the encoding, recovery of unsaved text after a crash, IME.
