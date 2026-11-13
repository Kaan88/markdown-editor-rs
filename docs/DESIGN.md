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
`panes.rs`, `overlays.rs`, `status.rs`, `view.rs` (and `view/*.rs` if it is split), `minimap.rs`,
`session.rs`,
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

## 18. Find and replace (2026-11-13)

| Where | What |
|---|---|
| `mdedit-core` `find.rs` | `FindOptions { match_case, whole_word, regex }`. Whole word uses the word class of the caret motions. A regular expression (`regex-automata`'s meta engine) is applied per line, skips lines over `REGEX_LINE_MAX` (1 MiB) and empty matches, and is refused over `REGEX_SIZE_LIMIT`. `Query::check` gives the reason a pattern does not compile |
| `mdedit-core` `replace.rs` | `expand` (`$0`..`$99`, `${name}`, `$$`), `replace_one` (only when the range is a match now), `replace_all` (matches found first, replaced front to back in place, one undo step; `within` for a range) |
| `docs.rs` | `FindState::options` and `error`; `EditOp::ReplaceOne` and `ReplaceAll` go through `Docs::edit` like every edit, so marks, colours and line-ending counts follow. `REPLACE_ALL_MAX_BYTES` is 24 MiB |
| `ui.rs` | `Ui::replace` (a second text field), `replace_open`, `field` (which box has the keyboard while `find_focused`), the three options; `Hit::Replace`, `ReplaceOne`, `ReplaceAll`, `ReplaceClose`, `FindOpt(..)`; `REPLACE_H` |
| `chrome.rs` | `draw_replace_strip`, `replace_plan` (the box shrinks, then the toggles go), `draw_field` for both boxes |
| `app.rs` | The strip is part of the Bar region, above the update bar: `notice_h` adds both |

Keys: Ctrl+H opens the strip (the replacement box when there is a query, else the find box);
Tab moves between the boxes; Enter in the replacement box replaces the selected match and
selects the next, and only goes to the next when no match is selected; Ctrl+Shift+Enter
replaces all; Escape leaves the box, then closes the strip, then clears the query. The
replacement's line breaks become the document's. A pattern that does not compile shows "Not a
valid pattern" where the count is.

## 20. Zoom and go to line (2026-11-13)

`Docs::zoom` is the text size in percent (the status strip reads it); `App::set_zoom` sets it
and `View::zoom` together, and the view measures its metrics again. The scroll is in lines, so
the first visible line stays. `metrics::zoom_stepped` keeps steps on a grid of 10 between 50
and 400.

Go to line is a third box of the bar, `BarField::Goto` with `Ui::goto`, drawn in the find
box's frame while it has the keyboard (`Ui::going_to`). `strings::parse_line_col` reads it and
`Docs::go_to` clamps the line and column to the text.

## 21. Matching bracket and twin words (2026-11-13)

`mdedit_core::bracket::matching` finds the pair. `Ready::brackets` keeps the answer for the
revision and caret (a `Cell`, so the text region's signature can ask too: a change of pair
repaints the whole text, since the partner may be on any line). `Ready::twin_word` gives the
selected whole word; `View::draw` looks for it in each visible line's text and tints the
matches under the glyphs, after the find marks and before the selection.

## 22. Show whitespace and the View menu (2026-11-13)

`Docs::whitespace` reaches `View::draw` through `DrawState`. After a chunk's glyphs are
drawn, its source bytes are walked: a run of spaces is measured once and divided evenly, a tab
is measured by itself; marks are plain fills in device px. The line-ending mark is a one-glyph
layout kept in the line-number cache (`View::eol_mark`). `PopupKind::View` and `Act::ViewMenu`
work like the Recent menu opened from the main menu.

## 23. Encoding and line endings from the status strip (2026-11-13)

`status::draw` records `Hit::StatusEncoding` and `Hit::StatusEol` (region Status) over its last
two fields and fills them on hover. `PopupKind::Eol` and `PopupKind::Encoding` are menus built
in `status.rs` and made with the shared `Menu::above`, anchored at the field's top right.
`Act::Eol` goes through `Docs::edit` as `EditOp::LineEndings`, like the other whole-document
line commands. `Act::SaveIn` calls `Docs::save_in`, which checks `save::first_unencodable`
before changing `info.encoding`. `Act::ReadAs(Option<Encoding>)` goes through `App::request` as
`After::ReadAs(id, encoding)`, which sets `Doc::forced` and starts a load; `start_load` and
`load_now` copy `Doc::forced` into `LoadOptions::forced`. In the core a forced load is one
decoding pass (`Detect::Forced`): a byte order mark of the chosen kind is skipped, invalid
input becomes U+FFFD and the result is lossy.

## 24. Word wrap (2026-11-13)

`bins/mdedit/src/view/wrap.rs`. With `Docs::wrap` on (copied into each `Ready` by
`Docs::sync_wrap` before a frame) and no line over `WRAP_LINE_MAX`, `Ready::wrapped()` holds
and the view changes in four places, all behind that test, so nothing changes with wrap off.

- Rows: `wrap::rows(r, line)` asks the core's `display::wrap_rows` for the row starts of a
  line at the view's width as last drawn, each a `ChunkStart` (offset and column counts from
  the line's start). They are kept per line in `Ready::rows` until the revision, the width or
  the advances change. A row is then a `Chunk`: `View::shaped` shapes it and `View::draw`
  places it at x 0 and its own y. `draw` first builds a list of `ShownLine`s (line, y,
  height, parts) for both modes and draws from it.
- Position: `Ready::top` is a whole line number and `Ready::top_row` the row in it.
  `wrap::step` walks rows from a position, `wrap::rows_between` counts them up to a limit;
  nothing counts the rows of the document. `view::scroll_rows` and `view::scroll_to` are the
  two ways the shell moves the view in either mode.
- `view::reveal` hands over to `wrap::reveal`; `View::hit` finds the row under y by stepping
  from the top row; Up and Down call `wrap::vertical`, which keeps `Selection::goal_col` as
  the column in the row.
- `App::compose` does not use caret bands when the document is wrapped.

## 25. Multiple carets (2026-11-13)

`Ready::sel` stays the primary selection; `Ready::extra` holds the others in document order
(`all_sels` and `set_sels` go between the two forms). The core's `multi` module does the work
on a list: `normalize` (sort, merge), `edit` (one `Cmd`, one undo step, `MultiOp::Insert`,
`InsertEach`, `Backspace`, `Delete`), `move_all`, `column`. `Docs::edit` turns Insert, Newline,
Indent, Backspace, Delete and Paste into a `MultiOp` when `extra` is not empty and clears
`extra` for every other operation, so no other command has to know about several carets.
`Ready::column` remembers the corners of a column selection made by keys for as long as the
selections are still the ones it made. `View::draw` fills the other selections and draws
their carets inside the loop over a line's parts, finding those that reach a part by binary
search, so a column down a large file costs what is on screen.

## 26. Find in files (2026-11-13)

The search itself is in the core (`search::in_tree`): it walks the folder, reads each file with
`load_path` and reports through a callback, so it knows nothing of threads. `Results::start_tree`
runs it on a thread of its own, sends the events over a channel and wakes the loop with
`Wake::Results` at most every 40 ms and at the end; `Results::poll` takes them in. A new search
or closing the panel sets the old search's cancel flag and drops its channel, so a worker that
is still reading ends at the next file and its events go nowhere. The panel is a sixth region
(`Region::Results`), cut from the bottom of the text by `Areas::with_results`; its signature is
`Results::generation` plus the scroll, so rows coming in repaint the panel and nothing else.
A match is a byte range: opening it calls `Docs::select_in`, which selects it at once in a tab
that is read, or leaves it in `Doc::pending_select` for the load to apply.

## 27. Code folding (2026-11-13)

`mdedit_core::fold` finds a block from the indentation of the lines around one line, when
asked; nothing scans the document as it is typed in. `Folds` is the sorted set of folded
blocks of a document (`Ready::folds`), each a first line that shows and a last line; with it
goes a running count of hidden lines, so `row_of` (a line's row among the lines that show) and
`line_at_row` are binary searches.

`Ready::top` stays a line of the document, so it means the same when folds open and close.
Without word wrap the view turns it into a row and back where it does arithmetic on it
(`view.rs` `top_rows`, `set_top_rows`: scrolling, the clamp in `geom`, `reveal`, the
scrollbar, a click) and draws rows `visible(g.top, ..)` through `line_at_row`. With word wrap
the walk over rows (`view/wrap.rs` `step`, `rows_between`, `top`) goes from a line to
`Folds::next` or `prev` of it. Without folds every one of these is what it was.

A caret is never on a hidden line: `reveal` opens the fold over what it brings into view, and
`geom`, which runs before every frame and click, opens those over the carets
(`Ready::unfold_carets`). Up and Down without wrap are made as long as the lines between the
rows are many (`app/input.rs`). `Ready::changed` gives each edit to `Folds::shift`.

## 28. Minimap (2026-11-13)

A narrow column at the right edge of the text area that shows the document in miniature, with
the part the editor shows marked. Off by default; View > Minimap (no key), `--minimap` for shots,
kept in the session as `minimap=1`, and Ctrl+Shift+M (also a right click on the column: "Hide
minimap"). `bins/mdedit/src/minimap.rs` holds the pure parts and the
drawing; `view.rs` (`map_scene`, `map_scroll`) and `app.rs` / `app/input.rs` wire it in.

**Region.** `Region::Minimap` and `Region::MinimapOther` (the eighth and ninth), `Areas::minimap`
and `minimap_other`, `Hit::Minimap` and `Hit::MinimapOther`, `Drag::Minimap { grab, doc }`.
`Areas::with_minimap` runs after the results panel and the split: it takes 96 logical px off the
right of `text` and of `other` (each pane of a split view has its own column, at its own right
edge) and gives them to the new regions, full height, unless a pane would be left with under
320 px. The second pane's region is its text and no more, so its column is beside it; where the
panes part, the line is the second pane's own when it is on the right, and the left pane's
column draws it at its right edge when it is on the left. So the
text, its gutter and its vertical scrollbar simply get a narrower rectangle; `view.rs` does not
know the column is there. It repaints alone, and it has no signature of its own beyond the
document's revision, the view's place, the colour answers, the theme, the pointer and what the
column marks (below).

**Rows.** One row per line that shows (`Folds::line_at_row` maps a row to its line; folded lines
are skipped; word wrap is ignored). A row is 2 px of ink and 1 px of gap at scale 1.0, a character
column 1 px, all scaled and rounded with at least 1 (`Px`). `segments` turns a line's characters
into filled segments: a run of non-whitespace characters of one colour, tabs going to the next
`TAB_WIDTH` stop, nothing past the columns that fit. A run that crosses a colour change is split
there. Only the first 512 bytes of a line are read. No text is laid out; the marks are filled
rectangles in a cached `Bitmap` (the rows without the band), blitted with the band over it, so a
repaint caused only by the pointer entering the band does not draw the rows again. The cache is
keyed by document, revision, window start, both colour generations, folds, theme, scale and size.
Typing re-draws the window (about 250 rows) in the same frame as the text.

**Window.** `Plan::with_range(rows, band_first, band_rows, range, row_h, column_h)`. When all rows fit, the
document starts at the top. Otherwise `top_px = fraction * (rows_px - column_px)`, where the
fraction is the band's first row over its range, which for a view without word wrap is exactly the
editor's scroll position over its scroll range. The window is at most `column_h / row_h` rows
however large the file is; nothing walks the document (a row count is `len_lines` less the folds'
prefix sum). With word wrap the band is the lines from the view's top line to the one at its
bottom, to a wrapped row: a line's row is shared equally by its wrapped rows, so the band starts
and ends at the fraction of a line that the top and the bottom row of the view are. The bottom
row can be only partly in view and counts whole, so the band can be up to one wrapped row too
tall; its top is exact to a wrapped row. The scroll fraction is the editor's own with wrap, the
top line over the lines less one (`Plan::range`), and a press or drag sets the top line and the
wrapped row in it (`view::map_scroll`).

**Colours.** Each segment takes its colour in the editor at 78 % over the page (60 % was tried
first and compared with 78 % in a light and a dark shot: at 60 % the bars read mostly grey and
comments, strings and keywords were hard to tell apart; at 78 % they are told apart at a glance
and the column is still quieter than the text); a line without
colours uses `foreground(dark)` the same way. They come from the highlight worker with a new
request, `HighlightService::map`, answered in a `Colored` with `map` set and stored in
`Ready::map_colors`. `peek` could not be used: the worker keeps one request slot per document and
the minimap's window is another range of the document the editor is viewing, so the worker has a
second slot. Order of work in the worker: the view (or a peek) first, then the map, tried in 2 ms
slices (the view's are 8 ms) so a view request arriving meanwhile waits at most that long, then
the background parse. The shell asks when the window leaves the lines asked for last (the request
holds half a window more each side), the theme changes, or an edit makes the answer stale.
Until an answer arrives a line is drawn with the editor's own colours when it has them, else in
the text colour. The first frame does not wait for these answers (`--shot` does).

**Marks.** Drawn over the rows each time the column is drawn, from what the editor already
keeps, and only for the window's lines: the selections (primary and several carets) in the
editor's selection colour at the same column scale, the whole row width for a line selected
through its end (`sel_cols`); the caret's row, a full-width line in the text colour at low alpha
(only in the pane with the keyboard); the find matches, a 3 px mark at the row's right edge for
those of `Ready::find.matches` inside the window (the current one in its own colour; at most 4096
a drawing; no search of its own); the bookmarks, a mark at the row's left edge in the accent
colour. They are not in the cached bitmap, so a caret move redraws the column (blit and marks)
and nothing else.

**Band and pointer.** The rows in view get the text colour at 8 % (14 % while the pointer is over
the column or a drag runs). A press outside the band centres the view on the row pressed
(`first_to_centre_on`) and then holds the band where it ended up, so a drag moves it by as many
pixels as the pointer moves; a press inside it holds it where it was pressed. A drag turns the
band's wanted top into the view's first row with the inverse of the column's scrolling
(`first_for_band_top`), so the band follows the pointer exactly and nothing feeds back. In a file
too long for its rows to fit, the band after a press is generally not under the pointer: the
minimap scrolls with the editor, as Sublime's does. The wheel over the column scrolls the text as
it does over the text. A press on the second pane's column acts on that pane's document and
gives it the keyboard, as a click on its text does; the wheel over it scrolls that pane. No
tooltip. A right click opens a menu with one item, "Hide minimap" (the command).

**Macros.** The toggle is recordable in a macro, as Word wrap and Show whitespace are (it is not
in `macros::recordable`'s list of exclusions).

**Not built:** one row per wrapped row; a width setting; wide (CJK) characters count one column.

## 19. Unsaved text survives a close (2026-11-13)

Closing the window with a session does not ask about unsaved changes: `App::request` with
`After::Exit` calls `keep_unsaved`, and only asks if that fails or there is no session.

| Where | What |
|---|---|
| `docs.rs` `write_backups` | Writes the text of each modified tab to `<n>.txt` in the folder (to a `.tmp` name, synced, renamed), removes every other file there, returns the list. A tab still waiting for its text keeps the backup it has |
| `session.rs` `Unsaved` | `unsaved=<n>.txt\|<untitled number>\|<path or nothing>`, one line per tab. Only names made of digits and `.txt` are read |
| `app.rs` (start) | For each entry whose backup exists: a file's tab is added with `Doc::restore` set and read at once; a document without a file (or whose file is gone) is opened with the text |
| `docs.rs` `apply_load`, `Ready::put_back` | Once the file is read, the backup replaces the whole text as one edit, before the colours are asked for |

The folder is `backup` beside the session file (`%LOCALAPPDATA%\mdeditackup`, or beside the
file `MDEDIT_SETTINGS` names). `Doc::modified` is also true while `restore` is set, so such a
tab shows its mark and closing it asks.

## 29. Sort, reverse, remove duplicates, join (2026-11-14, core only)

In `mdedit-core::lines`, with the shape of the other line commands: `(buf, hist, sel, ..) ->
Outcome`, one undo step (`Kind::Other`), one merged `Change`, the selection returned unchanged with
no change and no history entry when no byte would change. They take a single selection; as for the
other line commands the shell drops the extra carets first. Move line up and down already exist
(`move_lines`) and are unchanged.

| Function | Block | What it does |
|---|---|---|
| `sort_lines(buf, hist, sel, SortOrder)` | selection's lines, **whole document when the selection is empty** | `Ascending` and `Descending` compare the line text (without its break) byte for byte; `IgnoreCase` compares each char lowercased by its simple mapping (`char::to_lowercase`, first char). Stable |
| `reverse_lines(buf, hist, sel)` | as sort | reverses the order of the lines |
| `unique_lines(buf, hist, sel)` | as sort | removes every line equal (bytes) to an earlier one of the block; keeps the first |
| `join_lines(buf, hist, sel)` | selection's lines; **caret's line with the next** when the selection is empty or in one line | each break plus the next line's leading white space becomes one space, none when that line is empty or blank. The first line's text is kept as it is |
| `block_too_large(buf, sel, whole_if_empty)` | | whether the block is over `LINE_BLOCK_BYTES` (64 MiB); the four commands do nothing then, so the shell can say why |

"The lines of the selection" are those of `touched_lines` (a non-empty selection ending at column 0
leaves that line out). A last line that is empty and has no break (the text after the document's
final break) is not part of a sort, reverse or dedupe block, so the final break stays last.

Line endings: sort and reverse move the line texts and leave every break where it was, so a mixed
document keeps its pattern and a missing final break stays missing. `unique_lines` removes a line
with its own break, except that the block's last line hands its break (or none) to the last line
that stays. `join_lines` ends the joined line with the break of the last line joined. A CRLF is
never split.

Selection afterwards: sort, reverse and dedupe select from the start of the first line of the
block to the end of its last line's text (so running another one of them acts on the same lines).
`join_lines` with an empty selection puts the caret at the first join point; with a selection it
selects the joined line's text. Unlike `move_lines`, the first three write only the bytes that
differ (the common start and end of the block are kept).

## 30. Typing help (2026-11-14, core only)

`mdedit-core::typing`. The shell calls `type_char`, `enter` or `backspace` first, with the list of
selections and the primary's index (as `multi::edit`); `None` means "do the plain edit", `Some` is
the finished edit: `(Outcome, Vec<Selection>, primary index)`. The settings are an `Assist`
(`Assist::for_language(name, Indent, tab_width)`, or `Prefs::assist(language)`): `auto_pair`,
`auto_indent`, the `Pairs`, the `Indent` unit (`Tab` or `Spaces(n)`), `colon_blocks`, `markdown`.
With a switch off the function returns `None` for what that switch covers (`auto_pair`: `type_char`
for pairs, step-over, wrapping, and `backspace`; `auto_indent`: `enter`, and the dedent of a typed
closer). A pure step over a closer returns `Some` with `change: None` and moved selections, and no
history entry.

**Pairing.** `Pairs::for_language(name)` (the highlighter's name or an extension, case-insensitive):
plain text, "tex": brackets only; Markdown: brackets, `"`, backtick; JSON, Rust, the Lisps,
Haskell, OCaml: brackets and `"`; JavaScript, TypeScript, Go, the shells, Ruby, Perl, SQL: all
four; anything else: brackets, `"`, `'`. An opener typed with no selection inserts its closer when
the next char is white space, a closer (`) ] }`) or the end; a quote also needs the previous char
to be no word char (letter, digit, `_`), not the same quote and not a backslash. A closer typed
before the same closer steps over it. With a selection an enabled opener wraps it (two inserts, no
copy of the text) and the selection stays on its text, in its direction. Backspace between an empty
pair (`()`, `""`, ...) removes both, as its own step.

**Indentation.** Enter inserts the break (`eol`), then the leading blanks of the line up to the
caret (at most 4096 bytes, `TYPING_SCAN`); one more unit when the last non-blank char before the
caret is `{`, `[`, `(` (or `:` with `colon_blocks`); with the caret directly between `{}`, `[]` or
`()` the closer goes to its own line at the outer indent and the caret to the middle line at the
inner one. A selection is replaced. A `}`, `]` or `)` typed as the first non-blank of its line
(blanks only before the caret) removes up to one unit of indent (a trailing tab, or spaces) when
the indent is deeper than that of the line holding its opener, found by `bracket::opener_before`
(nesting of the same kind, `BRACKET_SCAN` bytes, a shared budget of four times that per keystroke
over all carets); never below the opener's indent.

**Several carets, undo.** Every selection acts alone; carets with nothing special get the plain
edit inside the same step; if the edits would overlap, `None`. One undo step per keystroke. A single
caret typing a pair is `Kind::Typing`: `history` now keeps the caret, not the end of the inserted
text, as the place where the next typed char joins the run, so "(", "a", "b" is one step and the
step over the `)` ends it. Everything else made here is `Kind::Other`.

**Markdown (`markdown` set, with `auto_indent`).** Enter on a list item or quote line, caret at or
after where the item's text starts: continues it with the same indent and quote markers and the
same marker and spacing; `N.` and `N)` continue with N+1 (zero padding kept, later items are not
renumbered); a task item continues as `[ ] `; `>` and nested `> >` continue the quote. On an item
with no text, Enter replaces the line instead: a nested item moves out one unit (marker kept), a
top-level one loses its marker (a quote prefix stays), an empty quote loses its marker, a nested
empty quote one level. A caret inside the marker gets the plain Enter. `list_indent(buf, hist, sel,
outdent, &Assist) -> Option<Outcome>` is for the Tab key: with the caret in a list line up to where
its text starts (or a selection over lines, each list line moving), it adds or removes one unit of
the item's indent (after quote markers); `None` when nothing applies, so the shell does its normal
Tab. It takes one selection.

## 31. Markdown commands (2026-11-14, core only)

`mdedit-core::markdown`. `toggle_wrap` and `link_from_selection` take the list of selections and
return as the typing functions do (`Option<Typed>`); `toggle_heading`, `cycle_heading` and
`format_table` take one selection and return an `Outcome` (no change when nothing applies).

- `toggle_wrap(buf, hist, sels, primary, marker)`: a selection with the marker just inside both
  ends or just outside both is unwrapped; otherwise wrapped; it stays on the same text. A caret
  inside a span of the marker on its line (paired from the line's first marker, 4096 bytes each
  way) unwraps it; any other caret inserts the pair, the caret between. A one-char marker beside
  the same char is not a marker (`*` leaves `**a**` alone). Two carets in one span unwrap it once.
- `toggle_heading(buf, hist, sel, level)` (1 to 6): lines of the selection become `#` headings of
  that level after their indent (up to 3 spaces); the same level removes the prefix (all of them
  when all non-blank lines have it); blank lines are skipped. `cycle_heading` steps none, 1 ..
  6, none by the first non-blank line and sets all lines alike.
- `format_table(buf, hist, sel)`: when the caret's line is in a pipe table (header, delimiter row
  of `:?-+:?` cells with as many cells as the header, then lines containing `|`), rewrites it as
  `| cell | cell |` with every column padded to its widest cell by display width
  (`display::width_of`, wide characters 2), by the column's alignment, the delimiter row rebuilt
  with its colons (3 dashes at least). Cell text is trimmed only, `\|` is not a separator, the
  header's indent is used for all rows, line endings are untouched, a short row keeps its cells.
  A selection end keeps its cell and offset. Nothing outside a table, over 2000 rows, 64 columns,
  64 KiB in a line or 4 MiB in all, or when already lined up.
- `link_from_selection(buf, hist, sels, primary)`: `[text](|)`; a selection that is an
  `http(s)://` URL (no white space, up to 2048 bytes) becomes `[|](url)`; an empty one inserts
  `[|]()`. Inserts only; the text is not copied.

## 32. Replace in several ranges (2026-11-14, core only)

`replace::replace_all` already took one `within` range, and works as documented: one undo step,
literal, whole-word and regex, counts, selection mapped. New: `replace_all_in(buf, hist, sel, query,
template, eol, within: &[Range<usize>], cancel) -> (Outcome, usize, Vec<Range<usize>>)` for the
selections of several carets or a column. The ranges are clamped, floored to char boundaries,
sorted, empty ones dropped and overlapping ones merged (touching ones stay apart, so a match is
whole inside one range); each is searched as `within` is (a regex works on the lines it touches and
keeps the matches inside it, whole-word looks beyond the range's ends). The count is the sum; the
third result is the merged ranges in the new text (a start moves with the text, an end is the end
of its last replacement) for the shell to select again. `replace_all` is this with one range.

## 33. Fuzzy matching

The matcher lives in the shared crate (`mdshared_app::fuzzy`); the core's own copy was removed on
2026-11-14 before anything used it.

## 34. Preferences and the save options (2026-11-14, core only)

`mdedit-core::prefs::Prefs`: font family (empty: default), `font_pt` (7 to 32, default 14, the
shell's `FONT_PX`), `tab_width` (1 to 16, default 4), `indent_with_spaces` (off) and `indent_size`
(1 to 16, 4), `auto_pair`, `auto_indent`, `line_numbers`, `highlight_line`, `restore_session`
(on), `trim_on_save`, `final_newline_on_save`, `minimap`, `word_wrap` (off). `Prefs::parse(text)`
reads the `[preferences]` section (or text with no header) of the settings file as `key=value`
lines: unknown keys and sections ignored, bad values the default, numbers past a limit clamped;
`serialize()` writes every key (flags `1`/`0`, the family escaped) so read-write-read is stable.
Keys: `font_family`, `font_size`, `tab_width`, `indent_spaces`, `indent_size`, `auto_pair`,
`auto_indent`, `trim_on_save`, `final_newline_on_save`, `line_numbers`, `highlight_line`, `minimap`,
`word_wrap`, `restore_session`. `Prefs::indent()` and `Prefs::assist(language)` build the
typing settings.

`apply_on_save(buf, hist, sel, prefs, eol) -> Outcome` makes the two save options one undo step:
trailing blanks removed from every line (`lines::trim_trailing_whitespace`'s walk), then a break
`eol` added at the end of a non-empty document that does not end with one. No change, no history
entry when nothing differs. The shell calls it before encoding, with the document's most frequent
ending as `eol`.

## 35. Compare two files (2026-11-14)

Headless core only (`mdedit-core::diff`); no window part exists yet. Not run in a window.

**Input.** `diff(a: &Buffer, b: &Buffer, opts: DiffOptions, cancel: &AtomicBool) -> Diff`. Lines are
compared by content without their line break, so a CRLF file and an LF file with the same text are
equal. A text that ends with a line break has no empty last line (`diff::line_count`), so "a\n" and
"a" compare equal, and the empty text has no lines. `DiffOptions { ignore_trailing_ws,
ignore_ws, ignore_case }`: trailing white space is cut; all white space is dropped (a b = ab); letters
are compared by their lower-case forms. Each line is read once from the rope's chunks (in one piece
when it lies in one chunk, otherwise copied into a reused string), hashed to 128 bits
(`hash.rs`, two multiply-fold streams) and interned to a `u32`; from there only ids are compared. A
hash collision (negligible at 128 bits) would show two different lines as equal and nothing else.

**Algorithm.** Myers O(ND) in linear space (middle snake, recursion on the two halves), after the
common prefix and suffix are cut and the lines that occur on one side only are set aside (changed for
certain; this shrinks the problem much when the files differ much). The result is minimal (the
equal lines are a longest common subsequence; the tests check this against a dynamic-programming LCS
on 3000 random small cases) unless a bound is hit. A moved block is a deletion and an insertion.

**Bounds** (`limits.rs`). `DIFF_MAX_LINES` 2,000,000 lines per side: with more, nothing is hashed
and the result is one changed block. `DIFF_WORK` 80,000,000 steps (a step is a diagonal visited
or a line pair compared) for the whole comparison, and `DIFF_MAX_D` 50,000 edits for one split. A
stretch that is not finished stays one changed block, the rest is still compared exactly, and
`Diff::approximate` is set (everything reported `Equal` is equal; the hunks still tile both files).
`cancel` is looked at every 4096 lines while hashing and at every step of the diagonals loop; a
cancelled result is coarse and has `cancelled` set. Measured on this machine (release build, the
machine busy with other builds, load average about 16): 200,000 lines with 2,000 random block edits 37 ms
(hashing included); 2,000,000 lines with 10,000 edits 0.7 s; the worst case for the bound (2,000,000
lines against 2,000,000 lines drawn from 1,000 values, so no line is set aside) 0.57 s and 200,000
against 200,000 from 4 values 0.64 s, both approximate; 2,000,000 identical lines 0.13 s.

**Output.** `Diff::hunks()`: `Hunk { kind: Equal | Insert | Delete | Change, a: Range, b: Range }`
in order, tiling both files. Rows for two panes: `rows()`, `row(i)` give `Row { a: Option<line>,
b: Option<line>, kind }`; equal lines share a row; an inserted or deleted block has filler rows
opposite it; a changed block pairs its lines in order and the longer side's surplus has fillers.
`row_of_a(line)`, `row_of_b(line)` map a line to its row, `row(i)` gives each side's line back;
`next_hunk(row)` and `prev_hunk(row)` give the index of the next changed hunk starting after the row (before
the row), `hunk_rows(i)` its rows, `hunk_at(row)` the changed hunk a row is in. `stats()` gives
`DiffStats { added, removed, changed, hunks }`: inserted lines plus the surplus of changed blocks,
deleted lines plus their surplus, the lines a changed block pairs, and the number of changed hunks.
Nothing is stored per line: rows come from the hunks (a binary search over their first rows).

**Inside a line.** `intra_line(row, a, b, opts)` for a row of a changed hunk with as many lines on each
side (at most `INTRA_HUNK_LINES` 256, lines of at most `INTRA_LINE_BYTES` 64 KiB) gives the byte
range of each line left after the characters the two lines share at the start and at the end are cut
(case folded when `ignore_case`). Cuts are moved to grapheme boundaries of both lines, never inside a
UTF-8 sequence or a cluster; an empty range means pure insertion or deletion. None for other rows
and for a pair equal under the options.

## 36. Replace in files (2026-11-14)

Headless core only (`mdedit-core::replace_files`, plus `search::walk_files` and
`save::save_bytes`, both added without changing what was there; `save_path` now runs through a shared
`save_with`). No window part exists yet. Not run in a window.

**Plan** (`plan(Source, &Query, replacement, &PlanOptions, skip, cancel, progress) -> Plan`).
`Source::Tree(path)` (a folder, or one file) or `Source::Files(list)`. A tree is walked by
`search::walk_files`, which keeps find in files' order and choices: files then subfolders, by name ignoring
ASCII case; `.git`, `node_modules`, `target`, and names starting with a dot (`skip_hidden`) are skipped;
links to folders are not followed. Each file is read once (the bytes are kept only long enough to
hash them), decoded with the editor's loader (`load_reader`, the system code page from
`PlanOptions::load`) and searched with `find_all`, so literal, whole word, regex and case behave as in
the single-file replace; the replacement is the same template (`$1`..`$99`, `${name}`, `$$`).
A file is left out, with a reason in `Plan::skipped`, when it is over `REPLACE_FILE_BYTES` (16 MiB, the cap of
find in files), contains NUL, cannot be read, changed while it was read, is listed in `skip` (the
caller's documents open with unsaved changes), or has matches but some bytes could not be decoded (it
could not be written back unchanged). Files without a match are searched and not listed. Per file
in the plan: encoding, line-ending counts, number of matches (at most `REPLACE_MATCHES_PER_FILE`
100,000, with `capped`), a preview of the first `REPLACE_PREVIEW_LINES` 5 matches (line, the text around
the match before and after, each part at most 120 bytes), and the byte length and 128-bit hash of what
was read. Limits: `REPLACE_WALK_FILES` 500,000 files looked at, `REPLACE_PLAN_FILES` 10,000 files and
`REPLACE_PLAN_BYTES` 1 GiB in the plan (`Plan::truncated`), `REPLACE_SKIPPED_KEPT` 5,000 skipped entries
kept (`skipped_count` is the true number). `cancel` is looked at between files (and in the load and the
search); `progress(done, total)` after each file.

**Apply** (`apply(&Plan, chosen: &[usize], &ApplyOptions, cancel, progress) -> Report`). `chosen` are indexes
into `plan.files` (out of range ones ignored, duplicates once). For each: skipped when in
`ApplyOptions::skip`; the file is read again and skipped (reason "changed since the search") unless it
has the recorded length and hash; decoded forced to the recorded encoding (so the byte order mark state
is the same); `replace::replace_all` runs on the buffer with the file's most frequent line ending for
line breaks inside the replacement (the rope keeps every other break as it was, so the bytes outside the
replaced text are untouched, mixed endings included); the result must fit the encoding
(`save::first_unencodable`, else a failure, nothing written); a read-only file is a failure; with `backup` the
original bytes go to `<name>.bak`, else `.bak1`.. `.bak99` (created exclusively, an existing one is never
overwritten, all taken is a failure), and the backup of a file that then fails to save is removed; the
new text is written by `save::save_path` (temporary file beside the target, flush, sync, then
`ReplaceFileW` or rename), with the stamp seen before the read as the expected one. A failure in one file
does not stop the others. `cancel` is looked at before each file; a file being written is finished, so
a cancelled run leaves every file either whole-old or whole-new, and `Report::changed` lists the new
ones. A replacement equal to the text, or with no matches left, is a skip. `Report { changed,
replacements, skipped, failures, cancelled, reverts }`.

**Revert** (`revert(&Report, cancel, progress) -> RevertReport`). Each `Revert` holds the length and
hash of the original and of what apply wrote, and the original bytes in memory (all files together up to
`REPLACE_REVERT_BYTES` 64 MiB, `ApplyOptions::keep_bytes`) or the backup path. A file is restored (through
`save::save_bytes`: the same safe write) only if it still has exactly the bytes apply wrote; the backup is
checked against the original hash. Files edited since, or whose original was not kept, are skipped with
a reason. Backups are left in place. `Report::revertible()` counts the files that can be restored.

**Measured** (release, same busy machine): 5,000 files of 60 lines (12 MB, 300,000 matches) in 50 folders:
plan 54 to 72 ms; apply 3.6 to 21 s and revert 3.1 to 21 s over three runs, which is the one
`sync_all` per file of the safe write (the disk was shared with other builds); not measured on Windows.
