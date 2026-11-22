# Design: how each part is built

Reference. One section per part, written when the part was built and numbered in the order they
were written, so the numbers are a history and not an outline: use the table below to find a
subject. When a part changes, **correct its section in place**; do not add a dated note under it.
What is being worked on is in `docs/STATE.md`, and why things were decided is in
`docs/DECISIONS.md`.

## Before you trust a section

- **Sections 1 to 15 are the design of phase 1**, written on 2026-11-12 before anything was
  built, for a program that could only read. Most of it still describes the code. What does not:
  section 1 says the shared crates are path dependencies (they are pinned by commit, see
  `docs/CODE.md`); sections 13, 14 and 15 (what phase 2 was to add, the test plan, the work
  packages) are history.
- **Some subjects have two or three sections**, the core first and the interface later: typing
  help (30, 45), Markdown commands (31, 46), compare (35, 51, 58), replace in files (36, 52).
  Section 43 amends 37 and 41. Section 19 sits after 28. Read all of a subject's sections.
- **A section's date is when it was written**, and "not published", "branch `f/…`" or "core
  only" in a heading was true that day. Everything up to section 68 has been released.
- **Not written yet.** These are built and have no section here: the notes library and its pane
  (on `main` for 1.3.0), visual editing (1.2.0), the preview pane as a pane (1.0.0; sections 54,
  65 and 66 cover the hand-off and the live preview), pinned tabs, and the tabs left out at
  start. Until their sections exist, the code and its tests are the reference, and the old
  handoff in `docs/archive/` has the story. Whoever next changes one of them writes its section.

## Find a subject

| Subject | Sections |
|---|---|
| Crates, threads, how bytes become pixels, the first frame | 1, 3, 4, 5 |
| The decisions of the phase 1 design | 2 |
| Loading, encodings, line endings, saving, indentation | 6, 7, 16, 23, 34, 56 |
| The text view, selection, carets, column selection | 8, 10, 25, 57, 59 |
| Zoom, whitespace, matching bracket, word wrap, folding, the minimap | 20, 21, 22, 24, 27, 28, 37, 43 |
| Find, replace, find in files | 9, 18, 26, 32, 40, 41, 43 |
| Compare two files | 35, 51, 58 |
| Replace in files | 36, 52 |
| Line commands, typing help, Markdown commands, tables, completion | 29, 30, 31, 44, 45, 46, 60, 63 |
| The shell: title row, bar, panes, the command line, the palette, settings | 11, 12, 33, 42, 47, 50, 78 |
| Split view, two views of one document | 38, 53 |
| Macros | 39 |
| Unsaved text across a close, crash recovery | 19, 61 |
| mdview: the hand-off, the live preview, the Preview button's modes | 54, 65, 66, 78 |
| The window: shadow, corners, the frosted backdrop | 48, 55, 64, 67 |
| Paste an image, plain paste, rich text pasted as Markdown | 62, 78 |
| Updates | 17 |
| AI rewriting | 68 |
| History: what phase 2 was to add, the test plan, work packages, review fixes | 13, 14, 15, 49 |

## 0. How sections 1 to 15 began (2026-11-12)

The opening of this document as it was first written, under the title "Design: mdedit phase 1
(read-only)". Kept as it was.

> Written 2026-11-12. Phase 1 of `docs/PLAN.md`: "Read-only: open a file in the shared shell, tabs,
> scrolling, selection, copy, find, highlighting, encodings detected and shown. Done when
> first-frame, memory and scroll budgets are met on the small and the 100 MB file." Editing is phase
> 2; this design keeps the places editing will plug into (section 13) but builds none of it.
>
> The starting points are the four spike results in `spikes/*/RESULTS.md` (buffer, highlight,
> longlines, ime), the shared crates in `../markdown-shared-rs/crates`, and the viewer's shell in
> `../mv/bins/mdv-app/src`, which the editor's shell follows closely: the shared plan's step 4 (a
> window framework in `mdshared-ui`) was not done, so its fallback applies and mdedit writes its own
> window loop on the shared widgets, modelled on the viewer's.
>
> Nothing here is approved by the user; the user approves the interface by looking at the running
> application. Every number marked "spike" was measured on a loaded machine and is noisy.

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
(`release`, `debug`, `deps`). `--update-bar available|installing|installed|installed-plain` shows the bar with a
made-up version for a shot.

Once installed, the bar (and the About dialog's update button) offers Restart when the updater has a
file to start (`mdshared_app::relaunch::target`: the installed exe on Windows, `$APPIMAGE` on Linux).
Restart is `After::Restart`: it goes through `request` exactly as Exit does (unsaved text is kept
for the session, or the Save changes question is asked and a cancel aborts it), then sets
`App::restart`; `run()` spawns the file with `--restarted` after the window is dropped. The new
process waits in `relaunch::wait_for_predecessor` until the old instance's port and lock are gone,
before the single-instance hand-over, so it never hands its files to the dying window. Beside the
button the bar reads "mdedit X is installed. Restart to use it." (`Updates::bar_label_with`); without
a file to start it keeps "It starts the next time you open mdedit."

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

## 33. Fuy matching

The matcher lives in the shared crate (`mdshared_app::fuy`); the core's own copy was removed on
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

## 37. Fold chevrons (2026-11-14)

Foldable lines show a chevron at the right end of the gutter row: DOWN when open, RIGHT when
folded (`glyph::CHEVRON_DOWN`/`CHEVRON_RIGHT`, 12 px, in a 24 px cell, `view/metrics.rs`
`fold_cell`). Open chevrons are faint and show only while the pointer is in the gutter
(`Hit::Gutter`, so entering and leaving repaints the Text region); a folded line always shows
its chevron. A click on the cell (`Hit::Fold(line)`) or on the fold box (the three dots,
`Hit::FoldBox(line)`) calls `Ready::toggle_fold_at`. Clicking the line numbers still places the
caret. Whether a line heads a block is asked only for the visible lines, with
`fold::heads_block` (same answer as `block_end(..).is_some()`, tested against it). The cursor over
a cell is the hand and the cell is tinted. Both hits need the pane to have the keyboard
(`DrawState::band`): the second pane of a split has no chevron hits.

## 38. Split view, resizable and remembered (2026-11-14)

- A divider between the panes (`Hit::Divider`, grab zone `DIVIDER_GRAB` 3 px each side, found in
  `App::hit_at` from `Areas.divider` before any region's hits). Dragging sets the ratio, clamped
  so each pane keeps its minimum width; a double click resets it to half. Cursor: column resize.
- The session keeps `split=<L|R>|<permille>|<path>`: the second pane's file (a document without a
  file is not kept), the side the second pane is on and the ratio (clamped 100..900 permille).
  `Docs::restore_split` applies it after the first activate, and only when both files are open.
- A click in the unfocused pane focuses it, re-lays out with the roles swapped and then places
  the caret (`text_press` is the same code as for the focused pane). Its scrollbars are
  draggable (`Scroller::OtherText`, `OtherTextH`, Region::Other); the wheel scrolls it, Shift+wheel
  sideways, through `pane_doc`.
- The second pane's tab has a 2 px muted underline inset 12 px (`chrome.rs`).
- Still two files, not two views of one (see HANDOFF, "where it would have to move").

## 39. Macros kept in a file (2026-11-14)

`macros.txt` beside the settings, plain text: header `mdedit-macros 1`, then `last` (the macro
being played by the play keys) and `m <name>` blocks, each step on a line `k <mods|-> <key>
[t:<text>]`; names and typed text are percent-encoded. Bounds: 20 saved macros (`SAVED_MAX`), name
64 chars (`NAME_MAX`), file 4 MiB (`FILE_MAX`), typed text 4096 bytes per step (`TYPED_MAX`),
steps bounded as before. A file over the limit or with a bad header is ignored; a bad line drops
that macro whole and never panics. The file is read the first time a macro is needed
(`ensure_loaded`), so the launch does not read it, and written when a recording finishes or a
saved macro is used or deleted (atomic write). "Save the macro as..." asks for the name in the
Go to line box (`Ui.naming`, placeholder text); Enter saves, Escape cancels. The Macro submenu
lists the saved macros, most recently used first (`Act::PlaySaved(i)`); "Delete saved
macros..." opens a list dialog like Manage recent files. The play keys run the last macro.

## 40. Find memory (2026-11-14)

The three find options and the last 20 queries and 20 replacements (no duplicates, newest first,
an entry over 1000 bytes or with a control character is not kept) are in the session
(`find_case`, `find_word`, `find_regex`, `find_hist`, `replace_hist`) and restored at start
(`recall.rs`, `Recall`). A query is remembered when it is used: Enter in the find box, Next and
Previous, Replace and Replace all, find in files and in tabs; the replacement on Replace and
Replace all. Up and Down in the find or replace box step through the list (the draft typed before
is the last step down) **only when that list is not empty**; with no history they do what they did
(they fall through to the text, which moves its caret). The brief said "only if Up/Down do nothing
else there today": they do move the text caret from that box today, so this reading was chosen.

## 41. Replace in the selection (2026-11-14)

When the replace strip opens (the Replace command) over a selection that spans more than one
line, that range is kept (`Ready::replace_scope`) with `scope_on` true, and a fourth toggle
(`Hit::InSelection`, `glyph::SELECT_ALL`, tooltip "Replace only in the selection from when
this opened") shows left of the others. The lines it covers show a faint accent tint while
the strip is open. With the toggle on, Replace all calls `replace::replace_all_in` on that one
range (the new range it returns becomes the scope), one undo step; Replace changes the selected
match only if it lies inside the range, and Next after it goes to the next match inside the
range, round its ends (`Docs::find_step_scoped`). The range follows edits (`map_scope`) and goes
when the strip closes. The match counter and marks still cover the whole file.

## 42. Title row on Linux (2026-11-14)

A press on the title row used to call `drag_window()` at once. On Wayland that hands the pointer
to the compositor and the second click of a double click never arrives, so the window did not
maximise. Off Windows the press now only records the point (`Drag::Title`); `drag_window()` is
called when the pointer has gone more than 4 px on either axis with the button down
(`title_moved`), and a press and release that did not move counts toward the double click,
which toggles maximise (`title_press`). Windows keeps the immediate drag
(`DEFER_TITLE_DRAG = cfg!(not(windows))`). Unit tests cover the decision; the behaviour on Wayland
was not run (it needs the mouse).

## 43. Amendments to 37 and 41 (2026-11-14)

In-selection replace governs everything the strip shows. `Ready::match_window()` is the indices of
`find.matches` inside the scope; the counter ("k of N", or "No matches in selection"), the current
match (`fit_current_to_scope`, run when matches arrive and when the toggle changes), the match
marks in the text and the minimap marks all read it, so matches outside are neither counted nor
marked. Turning the toggle off restores whole-file behaviour on the same frame (the minimap
signature includes the scope).

The fold chevron has no tooltip (the fold box keeps its own). It is centred in the gap from the
number column's right edge to the text: centre 4 px inside the gutter edge, so at least 3 px clear
each side; hover tint width `FOLD_HOVER_W` = 12 px. The gutter did not change width.

## 44. Line commands (2026-11-14)

Sort lines, Sort descending, Sort ignoring case, Reverse, Remove duplicates, Join (Ctrl+J), all
`EditOp`s over `mdedit_core::lines`; one undo step each, every caret/selection block. A block over
the core's limit is refused with a toast. The text menu's top level is 12 rows: Undo, Redo, Cut,
Copy, Paste, Select all, Toggle comment (Markdown documents: `Markdown ›` in its place), `Lines ›`,
`Convert ›`, Find, Replace, Reload. No sort icon.

## 45. Typing help (2026-11-14)

Each `Ready` holds a `typing::Assist` built from prefs and language. Typed char, Enter and
Backspace (and Tab/Shift+Tab on a Markdown list line with one caret) go through the core first
(`typing_edit`), for one or many carets; when it answers `None` the plain edit runs. One keystroke
is one undo step; "(ab" then undo removes the run. Macros record the key and replay through the
same path (test). The editor has no IME composition or overwrite mode, so there was nothing to
route there; the find and replace boxes use their own `find_key` path and are untouched.

## 46. Markdown commands (2026-11-14)

Bold Ctrl+B, Italic Ctrl+I, Code, Strikethrough, Heading level Ctrl+Shift+H, Link (no key: Ctrl+K
is Toggle comment), Align table, in a `Markdown ›` submenu of the text menu for Markdown
documents; on any other document they toast "not a Markdown document". No existing key was taken.
The function list already outlines Markdown headings (`mdedit-outline`, `Flavor::Markdown`).

## 47. Settings (2026-11-14)

`Settings…` (Ctrl+,, main menu, SLIDERS glyph) opens the shared paged form
`settings::pages::Form` (D-086; the design is `docs/settings-redesign.md` on `mock/settings`)
over rows from `mdedit_core::prefs::Prefs`. Nine pages, nothing scrolls: Appearance (Theme,
Window backdrop on Windows, Window corners where offered; Font, Font size), Editor (Line numbers,
Highlight the current line, Word wrap, Minimap, Minimap width as its child; Live preview),
Indentation (Tab width, Detect indentation, its two children Indent with spaces and Indent
size), Typing (Close brackets, Automatic indent, Suggest words; Trim trailing spaces, End with a
line break), Notes library (the folder, stacked, with Choose… beside it), Tabs (Restore tabs,
Start on the last pinned tab, Offer to close + its days, Don't reopen tabs barely looked at + its
seconds and Leave out the last tab too), AI service, AI rewrite (section 63's rows) and Updates
(Check at launch, Look for a newer version, and on an AppImage Make mdedit the default).
`modal::pages` builds all but the AI pages, `App::ai_pages` those two (help always under its row,
since the engine writes it); `settings_pages` puts them before Updates. Ids, `Value` and
`Outcome` are the old form's, so `apply_setting` is unchanged. The four action rows of the old
form are buttons on their rows with the same ids: Choose… (Library folder), Save/Remove (API key,
live only when it would do something), the remove icon (Provider, a custom one chosen), the
fetch icon (Model, API connections). A row under a switch is disabled while the switch is off,
with "Needs the switch above." (seconds, last tab) or dimmed without a reason (the days row: a
second reason is the 11 px the smallest window lacks with both tab rules off); AI rewrite's rows
stay, disabled with "Choose a connection on AI service first.", while the connection is off. So
the pages are made again after every change (`ai_settings_refresh`, `set_pages`, which keeps the
page, the focus and a text box's caret). The dialog opens with the keyboard in the list, on the
page last shown this run (`Ui::settings_page`); AI: Settings and the AI bar open AI service
(`open_settings_at`). `--settings-section` names a page (`ai-service`) or an old group (`View`,
`Preview`, `Editing`, `Session`, `AI`, `System`) and is applied after `--ai-demo`. The update
button says "Check now" or "Update" (96 px), the About box's longer words stay there. A program's
path in a help line keeps its last 40 characters of folders (`short_path`). The test
`every_page_fits_at_the_default_and_the_minimum_at_every_scale` lays every page out with
`Form::fit` at 1280x800 and 520x360, scale 1, 1.5 and 2, in seven states (tab rules on and off,
an update offered, every connection, a failed model list with a typed model, a refused key) and
fails naming the page and the pixels over; the closest is Tabs with both rules off, 7 px at
520x360. A Choice with three options or fewer is a segmented group (the shared rule), but Model
and Effort are marked `Item::dropdown()`: their option count follows the fetched list and the
model, and a row that changed shape with it would read as another setting. A
Font family row is a single string; an unknown family falls back to the monospace stack. Changes
apply live: tab width, font size, line numbers and highlight-line through process-wide atomics in
`live.rs` (caches are cleared in `relayout_all`), the others through `Docs::set_prefs`. They are
saved under `[preferences]` in the session file; `--pref key=value` overrides for a run. Minimap and
word wrap are defaults for new windows/documents (OR'd with the remembered state at start). The
existing view toggles stay. `apply_on_save` (trim, final newline) runs in the save path as one undo
step, nothing when nothing differs, and not when the text is lossy or has an unencodable char
(checked in that order). Limit: Tab inserts spaces ("indent with spaces") only for a caret or a
selection within one line; a block still gets the core's tabs.

## 48. Window shadow (2026-11-14)

`mdshared_ui::shadow::Shadow::attach(&window)` after window creation, kept in `App.shadow`;
`update(scale, active, maximized)` on resize, scale change and focus change, as in the viewer.

## 49. Review fixes (2026-11-14)

Submenus: one rule, `overlays::sub_side(parent, window_w)`: right of the parent when `SUB_WIDTH`
(260 px) fits, otherwise left of it with the submenu's right edge on the parent's left edge. The
cause of the overlap was that the Lines, Convert and Markdown menus ignored the `align_right` flag
the cascade passed (the old rule chose "left" for any parent at x >= 200), so a context menu in the
middle of the window put the submenu on its parent. Settings: every help and label line is measured
against the room the form gives it in a test (default, 560x640, scale 1.5). Replace strip:
`chrome::find_frame` is the one rectangle the find row and the strip use (same x, width, height
and text inset; the strip's box has a REPLACE icon where the find box has SEARCH); options left of
it, Replace, Replace all (padding shrinks down to 12 px, then the widths scale) and Close right of
it, Close under the menu button. `--menu-at F` places the text-menu shots' right-click.

## 50. Command palette (2026-11-14)

`Ctrl+P` opens the shared `mdshared_ui::palette::Palette` over the window (`palette.rs` holds the
sources, `app/palette_events.rs` the hosting). It is modal like a dialog: drawn last, every key,
pointer and wheel event goes to it, Escape or `Ctrl+P` again closes it, and choosing closes it first
and then acts, so what the choice starts (a dialog, the find box) begins clean. The caret blink
timer runs only while it is open. Nothing exists before the first `Ctrl+P`: `App::palette` is an
empty host.

The first character picks the source. No prefix: open tabs ("Open tabs"), then recent files
("Recent"), then the folder's files ("Files", relative names, at most 5,000, listed by a worker with
`search::walk_files` the first time the box has text and kept until the folder changes). `>`:
commands, each with its icon and first shortcut as the hint, the ones that cannot run now greyed
(the same conditions as the menus, `palette::enabled`); the palette itself, Escape and the theme
cycle are left out. `:`: go to line, a message row ("Line 120" or why not), Enter jumps (it uses the
Go to line parser). `@`: the symbols of the active file from `mdedit-outline` (a worker, made when
first asked, per document revision). `?`: the prefixes; choosing one puts it in the box. Placeholder:
"Type a file name, > for commands, : for a line, @ for symbols". `--palette <query>` opens it for a
shot.

Decisions: a greyed command cannot be chosen (Enter does nothing); the file list is not shown for
the empty query (the tabs and recent files are), so opening the palette costs no walk; the macro
commands are enabled by whether a macro can play, as in the Macro menu.

## 51. Compare two files in the window (2026-11-14)

The comparison is `mdedit-core::diff` (section 35); this is what the window does with it
(`compare.rs` the data and the state, `app/compare_events.rs` the commands, the view's changes in
`view.rs` and `minimap.rs`). A comparison stands on the split view: the file in the left pane
against the one in the right. There are no merge arrows: both panes are ordinary editors and the
difference is only shown.

**Starting.** "Compare with the other pane" (enabled when the split shows two files), "Compare with
file…" (a file dialog, non-blocking off Windows and blocking on Windows exactly as Open files; the
file opens at the right, the active file stays at the left) and "Compare with saved" (enabled when
the active file has a path and unsaved changes: the file as it is on disk opens as a read-only tab
"name (saved)" at the left and the text as edited is at the right). They are in the View menu, the
tab's right-click menu (the last two) and the palette. `--compare A B` opens both files and
compares them; `--compare-saved` and `--next-difference N` are for shots.

**Rows.** Each compared document holds an `Align` (`Ready::align`): which line is on which row,
from `Diff::rows`, with filler rows opposite the other side's extra lines. In compare mode
`Ready::top` is a row, `shown_rows` is the row count, and the places that went from line to row
through the folds (`line_at_row`, `row_of`) go through `Ready::line_at_row` / `row_of_line` /
`line_on_row`, which use the align when there is one. A filler row is a flat tint; a click on it
puts the caret on the line that follows it. Word wrap is off and folds are ignored during a
comparison: `Folds::suspend` makes every question answer as if there were no folds while the folds
still follow edits, and `Docs::sync_wrap` gives a compared document no wrap; both come back when it
ends (the Word wrap item still toggles the setting, which shows when the comparison ends).

**Marks.** Changed rows are tinted (red on the left for a deleted block, green on the right for an
inserted one, amber for changed lines on both sides), the gutter too; inside a changed line the
part that differs (`intra_line`) is tinted more. The minimap tints the changed rows across its
column and skips filler rows. The status strip says "N differences" (", approximate" when a bound
was hit), "No differences", or "Comparing…" while the diff is out of date.

**Scrolling.** The panes scroll together by row: before each frame whichever pane's top moved since
the last frame leads (the active one if both did) and the other takes its top; the minimap, the
scroll bar, the wheel and keys all move the same field, so all of them are covered. After a new diff
each pane keeps the line that was at its top.

**Work and cost.** `Compare::step`, run by the timers and by a `Wake::Compare` from the worker,
watches the two revisions: when either changes it waits 150 ms (the debounce restarts on each
edit), then starts a worker on copies of the two buffers (a rope clone) that calls `diff::diff`
with a cancel flag; a result for revisions that are no longer current is dropped. Typing never
waits for it. While the diff is out of date the old one stays on screen (a line it has no row for
goes after its last row, the marks inside lines are left out and the strip says "Comparing…").
Nothing exists, runs or wakes the loop when no comparison is on; when it is on and settled there is
no timer.

**Keys and ending.** F7 and Shift+F7: the caret goes to the start of the next or previous
difference (a toast says when there is none). Stop comparing, Escape (after the find box, the
replace strip and the find marks have had theirs), closing either tab, ending the split, or an
other tab taking a pane ends it; the saved copy closes with it. Ending gives back the rows as lines
(each top is the line that was at the top), wrap and folds.

Decisions without the owner: F7 and Shift+F7 are the keys (nothing else used them); the saved copy
is at the left (what the file was against what it is); the colours are fixed per theme in
`compare.rs` (the shared palette has none for differences); the intra-line marks are not drawn
while the diff is stale; the options (`ignore_ws` and the others) are in the core but have no
control yet.

## 52. Replace in files in the window (2026-11-14)

The core is `mdedit-core::replace_files` (section 36); this is the panel and the dialog
(`replace_ui.rs` the state and the drawing, `app/replace_files.rs` the steps). After a folder
search the results panel has a box for the replacement and "Replace all…" at the right of its
title row.

**Steps.** "Replace all…" plans on a worker (`replace_files::plan`, with the files of tabs that
have unsaved changes given as `skip`; nothing is read from the tabs' text, nothing is written).
When the plan is made the confirmation dialog opens: "Replace N occurrences in M files?", the
`"find" → "replacement"` line, a scrollable list of the files with a tick box each (all ticked, a
click toggles) and their counts, the preview of the row under the pointer or the selected one
(three lines, before and after side by side, "and N more"), "Keep backup copies (.bak)" (off by
default), "K files skipped: show" (a collapsed line that lists them and why) and the buttons
"Replace" and "Cancel". Nothing is written before "Replace". Replace applies the ticked files on
a worker (`replace_files::apply`): the panel's title row shows "Replacing… 120 of 800" with a
Cancel button, which stops between files (the files written stay written and are in the report).
Then the panel shows the report: changed files with their counts, replaced, skipped and failed
with the reasons. "Undo replace" is enabled until the next search or replace, the folder closes,
or the program ends; its tip says how many files can still be undone. Tabs whose files changed
reload by the existing reload-on-change path (`Docs::check_changed`); a tab with unsaved changes
was left out of the plan and keeps its text.

Decisions without the owner: the dialog is one more `DialogKind` with the dialogs' own
conventions (Enter replaces, Escape cancels; Up, Down, Home, End and Space move and tick in the
list); the preview shows the first few lines of the file under the pointer (or the cursor row), not
a diff of the whole file, and a low window gives it up before it squeezes the list; the Replace
button is greyed and does nothing with no file ticked; replacing with an empty text is allowed (it
deletes the matches); the panel's find results are cleared when a replacement is made (they are
out of date) and a new search ends the report and the undo.

## 53. Two views of one document (2026-11-14)

Written before the code. The split view (section 38) shows two tabs; this lets both panes show the
same tab. Until now a document's caret, scroll, folds, find mark and cached colours were fields of
`docs.rs` `Ready`, so two panes on one document would have fought over them.

**Shape.** `Ready` keeps what is the text's: the buffer, history, encoding and line endings (`info`
with the stamps), language, bookmarks, the find query with its match list, the long-line chunk
indexes, the typing help, the word-wrap flag. `ViewState` holds what is one way of looking at it:

| `ViewState` (per view) | stays in `Ready` (per document) |
|---|---|
| `sel`, `extra` (all carets), `column`, the preferred column (`Selection::goal_col`) | `buffer`, `history`, `info`, `revision`, `language`, `colourable`, `assist` |
| `top`, `top_row`, `row_rest`, `left` | `bookmarks` (lines; shown in both views) |
| `folds` | `find`: query, options, matches, capped, stale, searching, generation |
| `rows`, `rows_stamp` (the wrap cache) | `chunks` (indexes of long lines: a function of the text) |
| `colors`, `map_colors` (and the service request key `slot`) | `wrap` (the setting, copied before each frame) |
| `find_current` (index of the current match) | `read_only` |
| `replace_scope`, `scope_on` | |
| the bracket-pair cache, `widest`, `view_lines`, `view_w`, `advance`, `wide_advance` | |
| `align` (compare mode's filler rows) | |

Hover and what is pressed live in `ui`, per window; the minimap's plan, its picture cache and its
window are `App::map_plan`, `map_cache`, `map_lines`, already per pane.

`Ready.view` is always the view of the pane that has the keyboard, so every existing caller
(`r.view.sel`, `r.view.top`, and the single-pane code paths generally) reads the same fields it
read as `r.sel`. A document shown in both panes has a second one, `Ready.twin`, the view of the
pane without the keyboard. Moving the keyboard to the other pane swaps the two (`mem::swap`, no
copying) and the panes swap sides in `Docs` as they already do for two tabs (`other_left` flips).
Code that works on the other pane (layout, drawing, its scroll bars, its minimap, its colour
requests) swaps the twin in for the length of the call and out again (`Docs::with_pane`); it never
sees a half-state because nothing else runs between. This costs the single-pane case nothing: the
fields are inline in `Ready` as before and there is no indirection.

`Docs.other` stays "the document in the second pane"; when it is the active document and
`Docs.same_file` is set, both panes show it. `Docs::other()` (the tab index that is not the active
one) is `None` then, so everything written for two tabs (compare, "End the split" on close) keeps
its meaning, and `Docs::pane1()` is the new "the document of the second pane, whichever".

**Folds are per view.** A fold is "these lines are not what I am looking at"; the reason to open a
second pane is to look at two places, and one of them folded away from the other is a normal use.
Sharing them would also make a fold in one pane hide the lines under the other's caret and scroll
position. Each view's folds follow edits by `Folds::shift`, as today; an edit that adds or removes
lines inside a fold opens it in that view, and `unfold_carets` keeps a caret off hidden lines. The
fold commands (F9, Shift+F9, Ctrl+F9) and the chevron clicks act on the view that has the keyboard.

**An edit in one view, the other view.** Every edit goes through `Docs::edit_in` and so does Undo
and Redo; the view that has the keyboard (the acting one) is handled as it is today, including the
selection a step restores (Undo puts back the step's `sel_before` in the acting view). The twin is
only mapped through the change: the core's `Change` now also lists its replacements in order
(`Change::pieces`, empty for the usual single replacement, filled by `Change::merge`), and
`Change::map_offset` carries an offset through them. That is what multi-caret editing and Replace
all already do to find where the carets end up, now available for any offset, so a caret between
two replaced matches stays between them instead of being thrown to the edge of the merged span.
Mapped: the twin's selections and extra carets (an offset before a replacement stays, inside one
goes to its start, after one moves by the difference; an insertion at the caret leaves it before
the inserted text), the scroll line (its start offset is taken before the edit and mapped, the
fraction and `top_row` kept, so text inserted above keeps the same line on top), the replace
range, the fold list, the current match, the colour windows (`LineColors::edited`) and nothing
else: `left` and the geometry stay. The column selection under way is dropped (it only goes on
while the selections are the ones it made). The twin never reveals its caret: the other view must
not jump. A reload keeps both views' caret line and column and scroll, as it keeps the one now.

**Highlighting.** The worker holds one parse per document and answered one visible range (`view`
or `peek`, the same slot) and one minimap range (`map`) for it. Two views of one document need two
of each, so the requests get a key, `slot` (0 or 1, carried by the `ViewState`, so it follows the
view when the panes swap), and `Colored` returns it. Per document the worker keeps a request
record per slot; it still parses the text once, the frontier is shared, and with one slot in use
it works exactly as before. `view` still names the document whose parse runs on in the
background; the twin's requests are `peek`s. An edit clears every slot's pending request, as it
cleared the one.

**Find and replace.** The query, its options and its match list are the document's, so the marks
show in both panes and the counter ("3 of 12") is the one of the pane that has the keyboard. Next
and Previous start from the acting view's current match or caret and move it; the other view's
current match stays until it is used or its match is edited away. A new search puts each view's
current match at the first match from its own caret. "In selection" is per view: the range is
taken when the strip opens, from the view that has the keyboard, follows edits in both views, and
goes when the strip closes. Replace one and Replace all act through the acting view's selection
and range and are one undo step; the twin is mapped through them.

**Other commands.** Go to line, bookmarks (set and next), fold commands, select all, the clipboard
commands and everything typed act on the pane that has the keyboard; bookmarks are the document's
and are drawn in both. A macro plays into the pane that has the keyboard (it records keys, and the
panes' focus keys are not recordable), so the other pane only follows by mapping. Compare is not
offered for one document against itself: "Compare with the other pane" is greyed (as it is with no
split), and "Compare with file…" or "with saved" end the same-file split first. A running
comparison ends before the same-file split starts. Each pane has its own minimap, with its own
band, find marks and caret mark.

**Panes and tabs.** "Show this file in the other pane" (View menu, tab menu, palette; no key)
opens the split with the active document in both panes, or, with a split open on another tab, puts
the active document in the second pane too (the other tab stays open, not shown). The new view
starts where the first is: the same caret, scroll and folds. The tab row shows the document once;
its second-pane underline (section 38) is drawn as two short lines with a gap, one under each half
of the tab. Closing the tab closes both views. Turning the split off keeps the view that has the
keyboard as the document's and drops the other. Choosing another tab while the panes show one
document puts that tab in the pane that has the keyboard, and the document stays in the other
pane with that pane's view (its caret and scroll are what it keeps), as the second pane's tab did
before. A click in the other pane, F8, or its minimap give it the keyboard and swap the views.

**Session.** `split=<L|R>|<permille>|<path>` keeps its form. A path that is the active tab's own
file is a same-file split; an old file never had that, so it loads as before, and a build that
does not know this reads a new file as "no split" (its `restore_split` refuses the active tab). A
document without a file is not kept in a split, as before. The second view's caret and scroll are
not saved, no more than the first's are: it starts as a copy of the first.

**Budgets.** A twin costs one more `ViewState` (the wrap cache and two colour windows, a few kB)
and the second view's layout and paint, which a second pane already cost. Typing in a document that
is in both panes does the acting view's work as before plus mapping the twin (a handful of
offsets) and two colour windows' `edited`; measured in the HANDOFF section for this change.

## 54. Hand-off with mdview and the live preview (2026-11-14)

The protocol (request, wire format, live text, finding the other application, placing windows) is
`markdown-shared-rs/docs/SISTER.md`; this section is what the editor does with it. The code is
`bins/mdedit/src/link.rs` (the links and the courier thread) and `bins/mdedit/src/app/handoff.rs`
(receiving, the two commands, placing).

**Receiving ("Edit in mdedit", from the viewer).** `--from`, `--follow`, `--place x,y,w,h` and
`--goto L[:C]` are launch arguments; a second launch hands them to the running window as a
`Request`. A request that is not `follow` opens the file (or its tab), puts the caret at the line
and column, moves the window if it carries a place, and raises it. A `follow` request only moves the
caret, and only in the file that is the active tab; it raises nothing and opens nothing. A place is
applied with `mdshared_ui::place::set`, which takes the window-geometry rectangle (on Windows and
X11 the outer one, on KDE Wayland `frameGeometry`), so the shadow's margin (section 55) does not
change what the viewer asked for; where a desktop cannot place a window the toast says so.

**Sending ("Preview in mdview", Ctrl+Shift+L, the Preview button's arrow, tab menu, palette;
section 78).** On a saved file:
`sister::open("mdview", ...)` at the caret's line, then the document is linked. The same command
again ends the link; while it lasts the menus name the entry "Stop the preview in mdview" and the
status strip says "Previewing". "Preview side by side" always opens, and first puts this window on the left half of
its screen (`place::halves` of `work_area`) and asks for the right half for the viewer; where
placing is impossible it says so, sends no place, and the viewer opens where it would. A document
without a file cannot be previewed (a toast says so): the viewer reads the disk.

**What is sent, and when.** Nothing runs while no link exists: no thread, no timer, `link_poll`
returns at once. While linked, each pass of the window loop (`Docs::link_poll`) compares the
caret's line and the document's version (load and edit counters) with what the viewer has. A
change is sent as `follow`: the line alone, or, when the live preview is on and the text changed,
the whole text with the line. At most ten sends a second (`INTERVAL` 100 ms): inside the interval
the pass asks the loop to come back when it is over, and of the changes meanwhile the newest goes.
Switching to a linked tab tells the viewer again where that document is. The text is a rope
snapshot (a clone that shares the nodes), so the UI thread's whole cost is the pass, a clone and a
hand-over to a slot (a mutex and a condition variable): measured at 0.3 microseconds for a 200 KB
document in the test in `app/handoff.rs` (HANDOFF, 2026-11-14 section on the shadow); the
copy to bytes and the loopback write happen on the courier thread.

**The courier.** One thread, one slot: a newer update replaces one that waits, so a slow or hung
viewer never blocks the window and never gets a backlog. A send nobody answers is retried every
250 ms for `GRACE` (5 s) after the viewer was started, since it is still starting; after that the
viewer is gone and the link ends. A send with text that was not answered leaves the text out for
`TEXT_RETRY` (5 s) and sends the line alone (an older viewer does not take text; section "Why
older receivers are safe" of SISTER.md). On exit the courier is given 300 ms to tell the viewer
the links are over.

**The cap.** A document over `MAX_TEXT` (2 MiB) sends no text: the viewer shows the file (the
save-driven preview), and one toast says so when the link starts. The line still follows.

**What ends a link.** The user (the command again); the tab closing; the file being renamed or
saved under another name (the link names a path, and the tab no longer has it); the viewer being
gone (above); the process. Ending one tells the viewer to go back to the file (`live_off`), if it
was sent text, except when the viewer is the one that is gone. Turning the setting off sends
`live_off` for every linked document and keeps the line following; turning it on sends the text
again at the next change. A save sends nothing: the viewer sees the file change and returns to the
file itself (its rule in SISTER.md). The viewer, for its part, asks `sister::alive("mdedit")` every
5 s while a tab is live, so an editor that crashed does not leave the preview frozen.

**Setting.** "Live preview while typing" (`live_preview`, default on) under "Preview" in Settings;
off: "mdview shows the file when you save". The settings form has a test that every help line fits
in the default window, a narrow one and at scale 1.5, and that this row is there.

**Not covered.** A viewer that is an older build takes the line and not the text; an editor that
is not running when the viewer asks to edit is started by the viewer with the request's arguments.

## 55. Window shadow routes (2026-11-14)

Replaces section 48's "KWin only". The window is created and kept as the viewer's is
(`markdown-shared-rs/docs/LINUX.md`, "Window shadow"): on KDE, KWin's own protocol with the window
as it was; on any other Wayland compositor a transparent window with a client-drawn margin of 36
logical px on every side (patched winit and softbuffer; `[patch.crates-io]` in the root
`Cargo.toml`, feature `client-margin`); on X11 none; on Windows the shadow `with_undecorated_shadow`
gives. `MD_SHADOW=client|kwin|none` forces a route for tests.

**The app's size is the content size.** `App.size`, the bitmap `screen`, layout and hit testing
are all in content pixels. Only `present` and three events know the surface: `Resized` and
`ScaleFactorChanged` turn the window's size into the content size (`content_size`, after
`Shadow::update` has re-read the margin: it is 0 when maximized, fullscreen or tiled), and
`CursorMoved` turns surface coordinates into content coordinates (`to_content`). The edge test
for resizing is `x < EDGE` with no lower bound, so the outer band (negative content coordinates)
starts a resize too. The saved window size is the content size (`logical_size`), so it does not
grow by the margin at each start (a test goes through five restarts at six scales). The minimum
size goes through `set_min_content_size`.

**Present.** The softbuffer surface has the surface size. `paint_frame` paints the margin when the
buffer's one is stale (a new size, scale, focus or margin; both buffers, one frame each) and then
the whole surface is damaged. Otherwise the damage rectangles are the content's, moved by the
margin, and the content is copied to `(margin, margin)` row by row; an old buffer (`age != 1`)
gets all of the content. A focus change that only changes the shadow's look still produces a
present (`expose`). With margin 0 every line reduces to the old code: the surface is the content,
the offsets are 0, `paint_frame` returns false (nothing painted), so `whole` is `age != 1` as
before and the damage is the same rectangles.

**Alpha.** The client route's surface honours alpha, so every content pixel must have alpha 255. The
shared `Bitmap` fills, blends and blits write or keep 255, but `Bitmap::new` is all zero: a pixel no
region paints stays transparent. One shot (`compare-saved`, minimaps on both panes) has such a
column, drawn black until now. `compose` therefore sets alpha to 255 when it allocates `screen`,
which changes no colour; a test composes frames and checks every pixel. The black column itself is
an existing drawing gap, left as it is.

## 56. Indentation follows the file (2026-11-14)

`mdedit-core::indent::detect(&Buffer) -> Option<Indent>`, pure, run when a document is read, when
unsaved text is put back, never while typing (a keystroke costs nothing more). It looks at the
first 2,000 lines and at most 64 KiB of their leading white space. A line with text counts when
its leading white space is only tabs or only spaces (a run of both, as in a comment's tab and
space, counts for neither). Fewer than three counted indented lines: `None`. More tab lines than
space lines: `Tab`; fewer: spaces; equal: `None`. For spaces the unit is the width that most often
is the step in from one counted line to the next (an unindented line is width 0; only steps in
count, so dedents do not vote) among 2, 3, 4 and 8; tie: 4 if among them, else the smallest; no
such step: `None`.

`Ready.detected` keeps the result; `Prefs::assist_for(language, detected)` builds the `Assist`
with the detected unit when the new setting "Detect indentation from the file" (`detect_indent`,
on by default, Editing, above "Indent with spaces") is on, else with the settings' unit. So
Tab, Shift+Tab, block indent, Enter after an opener, the Markdown list Tab and the several-caret
Tab all use the file's unit; with no clear style, or the setting off, the two settings below
apply as before. A document made empty or restored from a backup is detected from that text; Save
As refreshes the typing help too (the language changes with the name). New and compared tabs now
take the preferences too (they were made with the defaults until a setting changed).

**A line keeps its own kind.** When Enter or the Markdown list Tab adds one unit to a line that
already starts with the other kind (tabs on the line, spaces configured; or the reverse), the
added unit is of the line's kind (a tab; or the unit's width in spaces, the tab width when the
unit is a tab), so the editor never makes a mixed run of leading white space from a pure one. A
line that is mixed already, or has no indent, gets the configured unit. Block Tab (`indent_with`)
still inserts the unit in front of every line.

**Status strip.** A field between the language and the line endings shows the unit in force:
"Spaces: 4" or "Tabs". It has no action; hovering says "Indentation: 4 spaces, detected from the
file" (or "from the settings"). Where the fields would not fit the strip (a long selection count,
carets, Previewing, the narrowest window of 520 px) the field is left out and the same tooltip
sits on the language field instead.

## 57. Column selection (2026-11-15)

Built on the multi-selection model: one selection per line from the first to the last line of the
rectangle, the caret's line is the primary. Alt+drag and Alt+Shift+Arrows already existed; no new
shortcuts.

Columns are visual (`mdedit_core::vcol`): a tab counts to its next stop, a wide (East Asian wide
or fullwidth) character 2, a combining character 0, everything else 1. A column inside a tab or a
wide character is its start. Caret motions keep their own count; only this selection measures so.

A line shorter than the column gets a caret (or a shorter selection) at its end; there is no
virtual space drawn. Dragging past a line's end keeps the rectangle's width over short lines.
Typing replaces or inserts on every row; on a column selection that is still the one made, a row
that is only a caret at the end of a line narrower than the rectangle's left column is first
padded with spaces up to that column (`multi::column_pads`, one undo step; not for paste or
Enter). Backspace and Delete on a column with text remove the selected
text and leave rows that are only a caret alone; with only carets they act as usual.

Copy and cut join all rows (empty rows kept) with the document's line ending. Pasting text with as
many lines as carets puts one line on each; a copied column pastes back row for row, including a
last empty row. Otherwise the text goes in whole at each caret, as before. Undo is one step.
Escape leaves one caret. Dragging outside the text scrolls it.

Word wrap: rows are logical lines (not wrapped rows), columns are logical columns; dragging past
the end of a wrapped line does not extend the width. On Linux many window managers take Alt+drag to
move windows; Alt+Shift+Arrows is the keyboard way, kept the same on every platform.
`--select-column <line:col>-<line:col>` (1-based visual columns) is for screenshots.

## 58. Compare: copy a change across (2026-11-15)

The caret of the active pane is in a difference block (or on the line right after the filler rows
of a block this side has no lines of). "Copy this change to the other side" replaces the other
document's lines of that block with this side's; "Take this change from the other side" does the
reverse in this document.

Only an editable document changes. In "Compare with saved" the saved copy (read-only) is never the
target: Copy greys out with the working side active in the right pane, Take works. The commands
are greyed (View menu, palette) when not compared, when the diff is out of date, when the caret is
not in a difference, or when the target is read-only or lossy. Run by other paths (macro, test)
they say why in a toast.

One undo step in the changed document (`Kind::Other`). The target keeps its own line break; the
break after the block, or the missing one at end of file, stays as it was. A block the source
lacks removes the target's lines; a block the target lacks is inserted before the line named by
the range. The source is unchanged; the diff refreshes by the normal edit path (worker, off the
keystroke path). Only the block's lines are read, so large files cost nothing extra. Not
recordable in macros. There are no gutter arrow buttons (the compare view has no gutter between
panes and the panes are ordinary editors). Next and Previous difference (F7, Shift+F7) are also in
the View menu now. `--copy-change across|take` is for screenshots.

## 59. Multi-caret line commands (2026-11-15)

Rules per command (full text in the header of `crates/mdedit-core/src/multiline.rs`). The lines a
selection touches are as for one selection. Selections that share a line (for the commands marked
"adjacent", also lines that follow one another) make one block; a command acts once on each block.

- Duplicate: every block is copied below itself; selections move onto the copy.
- Move up/down (adjacent): every block moves one line; a block at the document's edge stays, the
  others still move.
- Delete line (adjacent): all touched lines go; one caret per removed block, in the same column.
- Toggle comment: the touched lines are one set: all commented means uncomment, otherwise comment
  every non-blank line at the smallest indentation. Only blank lines: each caret's line.
- Case: each non-empty selection; carets change nothing.
- Trim, tabs to spaces, line endings: the whole document as with one caret; carets follow text.
- Sort, reverse, remove duplicates (adjacent): each block alone (duplicates are looked for inside
  a block, lines never move between blocks); only carets: the whole document, as with one caret.
- Join: a block joins into its first line; a selection in one line joins it with the next.
- Heading level: every block steps by its own first non-blank line. Align table: each table once.
- The size limits (`BLOCK_TOO_LARGE`) apply to the blocks together.
- Indent and Outdent are unchanged (Outdent with several carets still leaves one caret).

The whole command is one undo step (`History::fuse_since`). With one caret nothing changes: one
`is_empty` check.

## 60. Markdown table editing (2026-11-15)

A table is what `markdown::format_table` already finds (header, delimiter row, pipe lines; limits
`TABLE_ROWS` 2000, `TABLE_COLS` 64). Extended: the prefix of a row is its spaces, tabs and `>`
markers, and the table ends where the quote depth changes; a pipe in an escaped `\|` or in an
inline code span does not separate cells (`markdown::bounds_of`).

Tab and Shift+Tab (`table::next_cell`): only in a Markdown document, one caret, selection (if any)
within one line, caret in a table; otherwise the ordinary indent runs. The table is lined up
(display width, alignment row respected, minimum 3) and the caret goes to the start of the text
of the next or previous cell. After a row's last cell: the first cell of the next row, the
delimiter row skipped. Tab in the last cell of the last row adds an empty row. A short row gets
the missing cell. Shift+Tab in the header's first cell stays (only lines up). Nothing is lined up
while typing.

Enter (`table::row_after`): no selection, caret after the last non-blank character of a table
row: a new row of empty cells under it (from the header or delimiter row: the first body row),
columns lined up, caret in its first cell. At the end of the last body row, when its cells are all
empty, the row is emptied (its `>` markers stay) so the table ends and the caret is on a line of
its own; in the middle of a table it adds a row. Enter elsewhere in a row is the normal Enter
(list and quote continuation unchanged).

Commands (`table::edit`, `TableOp`) act on the caret's row or column: Insert row below/above,
Delete row, Insert column right/left, Delete column, Move column left/right. The header and
delimiter rows are not deleted, there is no row above the header, the last column is not deleted,
column moves and inserts keep the alignment row. New rows and columns are empty cells; a new
column's delimiter is `---`. The table is lined up. They are refused with a toast ("This is for
Markdown files.", "The caret is not in a table.", or that the row or column command is refused
inside a table). Every operation is one undo step, one replacement of the part of the table that
changed; line endings of the rows that stay are kept, new rows use the document's.

The commands are enabled for any open document like the other Markdown commands. There is no
setting: Tab in a table is taken only in Markdown documents. A table whose header begins with a
list marker (`- | a |`) is not recognised.

## 61. Crash recovery (2026-11-15)

**Store.** The folder `recover` beside the session file (so `MDEDIT_SETTINGS` moves it; none with
`--no-session` or in a shot). One file per modified document, `<run tag>-<document id>.rec`
(`mdedit_core::recovery`): a header (magic line, `path`, `untitled`, `encoding`, `eol`, `disk` =
size and modification time of the file when it was read, `bytes`) then `---` and the text. A copy
is valid only when the text after the header has exactly `bytes` bytes. The header holds size and
time and the length, not a content hash (the length catches a cut-short copy; a hash would need
the text twice).

**When written.** `Recovery::poll` (`bins/mdedit/src/recovery.rs`) runs once per pass of the
event loop and compares each tab's buffer revision and encoding with what it last saw (integer
compares; no work with no modified tab). A changed document is written 3 s after its last change
(`RECOVERY_IDLE_MS`), and at the latest 30 s after the first change not yet written
(`RECOVERY_MAX_MS`), so continuous typing writes about twice a minute. The write is on a thread
that starts at the first write (none at launch): the UI thread takes an O(1) rope snapshot and a
header and sends them; the thread writes `<name>.tmp`, syncs, renames over the old copy. Jobs for
the same name that queue up are coalesced to the newest.

**Removed** (by a job to the thread) when the tab is saved, undone to the file's text, closed
(also "Don't save"), or discards its changes; all copies are removed on a normal exit
(`Recovery::finish`, joined first so no write comes after), together with the folder when empty.
Exception: a tab whose restored text was not yet put back keeps its copy (it is in no other
place); the next start offers it again.

**Limit.** A document over `RECOVERY_MAX_BYTES` (64 MiB) gets no copy (a copy it already had is
removed): rewriting it every half minute would cost more than it is worth. Above it, a crash loses
the unsaved text, as before; a normal close still keeps it through the session's backup.
Read-only tabs (a comparison's saved side) and documents read lossily get none.

**Normal exit and crash.** The session already keeps unsaved text on a normal close
(`write_backups`, section 19). That is unchanged and is separate from the recovery store except
that both are put back by the same path (`Doc::restore`, `put_back`). Because a normal exit
deletes the recovery files, what is found at the start is from a run that did not exit normally.
Recovery does not depend on "Restore tabs at start", and neither does the text kept on a normal
close: closing never asks, so a tab with unsaved text comes back at the next start whatever the
setting says (Audit 1, 2026-11-19); with the setting off only the tabs with nothing unsaved stay
closed. Each entry put back is rewritten to describe the tab it came back as (no file when the
file is gone, the untitled number it really got), so the session goes on naming its backup until
the text is saved or discarded. A recovery write in flight makes the next look come within a
second, so a failed write is retried even when nothing else wakes the window; a write that cannot
be handed to the writer counts as failed. There is no setting and no "autosave to the real
file".

**At start** (only when the folder has files; one directory listing otherwise): files that are
not whole and `.tmp` files are deleted. For a copy whose file still exists the tab is added (or
the session's tab for that path is used) with `restore` set to the copy, loaded at once like a
session backup; when the file is read, the text goes over it as one edit (Undo gives the file's
text), and the encoding the copy names is applied when the text can be written in it. A copy of a
document without a file, or whose file is gone, opens as an untitled document with its text. The
copy is renamed to this run's name for that tab at once, so a second crash before the next write
loses nothing. One toast "Recovered N unsaved documents" (singular for 1).

**A copy wins over a session backup.** A session backup is only replaced at the next normal exit,
so after a crash the session file can still list the backup of an earlier run beside a newer
recovery copy. `run()` scans the recovery folder first and skips a session `unsaved` entry that a
copy covers (`recovery::covers`: same path, or both without a path and the same untitled number);
`Recovery::restore` takes the scanned list and lets a copy replace a backup; a copy is never
deleted in favour of a backup. Stale entries (text saved, discarded or closed during the run) are
dropped from the session while it runs (`Unsaved::prune`, called from
`App::persist_if_changed`; not once the exit has set the list).

**File changed since.** If the file's size or time differs from the `disk` in the header, the text
is put back as modified, the tab's stamp is set to the copy's (so Save ends in the existing "The
file changed on disk" question) and the changed-on-disk banner shows; Reload drops the recovered
text. The file on disk is never written by recovery.

**Edge cases.** Two windows sharing a session folder are not possible (single instance). Header
values with line breaks are escaped; a header over 64 KiB is refused. `--demo-recovered N` shows
the toast for a screenshot.

## 62. Paste an image into a Markdown file (2026-11-15)

- Paste reads text first. Only when there is none (empty or not text) is the clipboard asked for an image.
- Only for a document with a file whose language is Markdown, or an unsaved document. Other files: paste does
  nothing, as before.
- Unsaved document (and an image on the clipboard): toast "Save the document as a Markdown file first, then
  paste the image"; nothing else happens.
- The file is `<document name without extension>-image-<n>.png` in the document's folder, the first n from 1
  that does not exist (created exclusively, so nothing is overwritten).
- The inserted text is `![](name)` with a forward slash path and these characters written %XX: space, `%`,
  `(`, `)`, `[`, `]`, `<`, `>`, `#`, `?`, `"`, control characters. Other characters (accented letters) stay.
- It goes in as an ordinary paste: one undo step (with several carets it is inserted at each). The file stays
  after undo; the toast says "Image saved as <name>".
- Read-only or lossy documents: the paste is refused as for text and the new file is removed again.
- Image data over 64 MB, or not a PNG on Linux, is ignored. Write errors: toast "Could not save the image: ...".
- Windows: the registered "PNG" clipboard format (keeps transparency) first, else CF_DIB (24 and 32 bit,
  alpha dropped) converted with the `png` crate. Linux: `wl-paste --type image/png` or
  `xclip -selection clipboard -t image/png -o` (Wayland session tries wl-paste first), run only on such a
  paste, stopped after 3 seconds.
- Drops are not touched: dropping a file opens it today.

**Image files copied in the file manager (Windows, 2026-11-15).** A copy in Explorer puts a file list on
the clipboard (`CF_HDROP`), not pixels.

- The file list is looked at before the pixels, under the same conditions (no text on the clipboard; a
  Markdown file or an unsaved document; the unsaved document gets the same "Save the document" toast).
- Every file in the list must be an image by its extension (png, jpg, jpeg, gif, webp, bmp, svg, avif, any
  case). If one is not, the paste does nothing: no partial paste, and the pixels are not looked at.
- A file is linked where it is, by its path from the document's folder with forward slashes
  (`img/a.png`, `../pics/a.png`); names are compared without case. Nothing is copied and there is no toast.
- A file with no such path (another drive, another network share) is copied beside the document under its
  own name, `name-<n>.ext` when that is taken (created exclusively), and linked there. The toast names the
  first copy. Inside one network share a file is linked by its path like on a drive.
- Several files go in one to a line, as one paste (one undo step). If a copy fails, or the text is
  refused (read-only), the copies already made are removed.
- `platform::clipboard_formats` is the one place that opens the Windows clipboard for these formats;
  `clipboard_png` and `clipboard_files` both go through it (64 MB limit for each format).
- Not on Linux: a copied file is not looked for there (`clipboard_files` is empty).

## 63. Word completion and snippets (2026-11-15)

No language server: the words of the open documents, and snippets.

- **Opening.** The command Complete word: Ctrl+Space, and Alt+/ (Ctrl+Space can belong to an input
  method on Linux; Alt+/ needs a layout with a bare `/` key, so not the Turkish one). It is in the
  text's right-click menu and the command palette. With the setting "Suggest words while typing"
  (Settings, Editing; off by default; `complete_auto`) the list also opens after the third letter
  of a word. Not while a macro is recorded; the command is not recordable.
- **When there is no list.** A selection, several carets (the command says "Completion works with
  one caret."), a read-only or lossily read document, another popup open, the word's start not on
  screen. The command with nothing to offer says "Nothing to complete here."; the automatic list
  says nothing, and does not read the text again for a longer form of the same word.
- **The words.** A word is letters, digits and `_`, 3 to 64 bytes. They are read when the list
  opens and not while typing on: 1 MiB of the active document around the caret (cut at line
  breaks), then the other open documents from their start, 1 MiB each and 4 MiB together, at most
  50,000 distinct words (`limits.rs`). Nothing is indexed, kept or watched between lists.
- **Matching.** A candidate begins with the same letter as what is typed (any case) and holds
  the typed letters in order; scored by the shared fuy matcher. Order: those that begin with
  the typed letters, snippets before words, score, how often the word occurs, shorter, then by
  name. The word equal to what is typed is left out; a snippet with that trigger stays.
- **The list.** The shared popup menu, up to 8 rows, its labels starting where the word starts;
  under the line, or above it when there is more room there. The first row is chosen. A snippet
  row says "snippet" at its right.
- **Keys while it is open.** Letters, digits and `_` are typed and narrow the list; Backspace
  widens it; Up and Down move; Enter and Tab take the chosen row; Escape closes. Any other key
  closes the list and does its own work. When nothing matches any more the list closes. Enter
  and Tab are never taken when no list is open. A click on a row takes it.
- **Taking a word** replaces the typed letters by the word: one undo step back to what was typed.
- **Snippets.** Built in, for Markdown only: `table`, `code`, `link`. The user's are in
  `snippets.toml` beside the settings file, read when a list opens and the file is new or changed
  (at most 1 MiB, 1000 snippets, 64 KiB each). The format is a small part of TOML: `[[snippet]]`
  tables with `trigger` (letters, digits, `_`), `text`, and optional `types` (language names;
  none means every file type). A line that cannot be read is skipped. A snippet of the file
  replaces a built-in one with the same trigger. "Open snippets file" (command palette only: the
  main menu has no room for a row in an 800 px window) makes the file with its instructions as
  comments when it is not there, and opens it.
- **Snippet text.** `$1`, `$2`, `${1:default}` are stops in number order, `$0` is where the caret
  ends (the end when there is none), `$$` is a dollar sign; a number used twice is a stop at its
  first place only (no mirrored typing). Line breaks become the document's; later lines get the
  indentation of the line the trigger was on, and a leading tab becomes one indent unit.
- **Filling in.** The first stop is selected. Tab goes to the next stop and Shift+Tab back, as
  long as the caret is inside the stop in use; the last stop ends it. A caret anywhere else ends
  it and Tab is a Tab again. This Tab comes before the table keys, so the `table` snippet's
  stops are walked first.
- **Cost.** Nothing at start, and with the setting off nothing on a keystroke (one test of the
  setting). Measured in a release build on the Windows PC: a list at every limit (5 MiB read,
  50,000 words) takes 34 ms to open, once; typing on only narrows what was found.

## 64. Rounded window corners (2026-11-14, branch `next`)

Setting "Window corners" (View group of Settings, under Theme): Square, Small (4 logical px),
Round (8, the default; the radius of the apps' dialogs). Kept in the session file as `corners=`
(the logical radius; written only when not 8), in `Session.corners`. The code is shared
(`mdshared_ui::corners`, `shadow::Frame`, docs/LINUX.md in the shared repository).

| Where | Result |
|---|---|
| Windows 11 | `corners::set_window_corners` sets `DWMWA_WINDOW_CORNER_PREFERENCE` (do not round, small, round) at window creation and on a change; the system draws it and squares a maximized window. Nothing is drawn by the app. |
| Windows 10 | The attribute is unknown, the call fails and is ignored: square, no message. |
| Wayland, client margin | Content corner squares (radius r device px, `radius * scale` rounded) are cut anti-aliased into the ARGB buffer; the shadow under them is the window's own. Square when the margin is 0 (maximized, tiled, fullscreen). |
| KDE Wayland | With a radius above 0 `Auto` uses the client margin instead of KWin's protocol (`shadow::decide(.., round)`), because KWin's route is opaque. Square keeps KWin's route. |
| X11 | Opaque window (no margin by default): square. `MD_SHADOW=client` rounds like Wayland and squares while maximized or fullscreen. |

**Drawing.** `present` copies the damaged rectangles into the surface buffer as before; when
`Shadow::rounds()` it then calls `Shadow::paint_corners(buffer, sw, sh, &screen.pixels, damage)`:
only the corner squares a copied rectangle touches (all four after a whole copy: buffer age not
1) are rebuilt, from the app's bitmap, as content at its coverage over the shadow beneath
(premultiplied). At most `4 * r^2` pixels, none when square or maximized; a corner not copied again
keeps its cut, so partial repaints stay correct. A change of the setting redraws the whole window
(`full`) and repaints the margin for two frames (the gate). The shadow's margin tiles are
unchanged (a rounded rectangle's blurred shadow differs from the plain one by about one alpha
step); the shadow beneath the uncovered pixels is a small table beside the tiles.

**Row.** Offered where it can work: Windows, or a window with a shadow route (Wayland); hidden on
X11 without the margin. Help line from `corners::setting_help()` (Linux: "Some desktops need a
restart to apply": on KDE, switching on from Square needs a restart because the window was
created opaque; switching to Square applies at once).

**Shots.** `--frame square|small|round` with `--shot` renders the window as the margin route shows
it (content, shadow margin, corners) over a flat backdrop; the existing shots are unchanged.
`tests/shots.sh` takes `frame-*` at scale 1 and 2 in both themes. The Windows runner draws the
same Wayland-style picture (it is the shared pixel code, not the DWM result).

## 65. Preview of a document that has no file (2026-11-15)

**A document that has no file.** "Preview in mdview" and "Preview side by side" work on an
untitled document. Nothing is written to disk for it: the text travels in the message.

- The document goes by a name, `mdedit-<process id>-<document number>` (`link::doc_name`), which
  is the same for as long as the tab lives and belongs to no other tab or editor. The message
  carries `?doc=<name>`, `?title=<the tab's name>` and, the first time, `?open` (see "A document
  that has no file" in the shared `docs/SISTER.md`). No path is sent.
- The viewer is started bare when none runs (it could not be given a path), and the first message
  asks it to show the document; with "Preview side by side" that message also carries the
  viewer's place. The first message is tried for the same 5 s as an opening of a file.
- The text is always sent, whatever "Live preview while typing" says: there is no file the viewer
  could read instead. Typing sends it again at the usual pace (at most every 100 ms, the newest
  wins); a caret move alone sends the line.
- A document over the protocol's cap (2 MiB) is not previewed: a toast says "This document is too
  large to preview before it is saved."
- A viewer that does not know such documents answers the plain `ok`. The editor then ends the
  link and says "This mdview cannot preview a document that has no file: save it, or update
  mdview." Nothing is opened in that viewer.
- Saving the document (Save or Save As) tells the viewer once that the document is that file now
  (`?doc=<name>` with the path). The link carries on as a link to the file, with the rules of a
  saved file: the text is sent only while it differs from the file and "Live preview while
  typing" is on.
- Closing the tab, "Preview in mdview" again, or leaving the editor ends the preview as for a
  file. The viewer keeps the last text it was given.
- The viewer's "Edit in mdedit" on such a tab asks the running editor for the document by its
  name; the editor switches to that tab and goes to the line. A name that is not a tab of this
  editor is ignored.
- Menus: the two commands are listed in the View menu and the tab menu for a document that has no
  file, whatever its language (it has no name to tell its kind by), and are no longer disabled
  there. The palette lists them whenever a tab is open.

## 66. The preview follows the caret inside long blocks (2026-11-15)

**The caret's place, not only its line.** While a document is previewed, each update to the
viewer carries the caret's line and now also its byte in that line, from 1 (`?col=`, the same
number `--goto line:column` takes). The viewer shows the row of the page that holds that place:
the line inside a long paragraph, list item or code block, the row of a table, and for a
paragraph written as one long line the row the caret is in (the viewer's notes on its branch `f/finer-sync`,
`docs/next/finer-sync.md`, have the mapping).

- An update is due when the caret is on another line, as before, or has moved to another
  stretch of 32 bytes of the same line (`link::COLUMN_STEP`). Moving inside a stretch sends
  nothing, so a short line never sends more than it did; a long line sends at most the usual
  ten updates a second while the caret travels along it.
- An update that goes anyway (a new line, typed text) carries the exact byte.
- The first hand-over (`Preview in mdview`) starts the viewer with the line alone, as before
  (`--line`); when the caret is further along a long line, the first update says so.
- The viewer's "Edit in mdedit" now names a column when the row at the top of its page is
  inside a line that wraps; the caret goes to that byte (`go_to`, which always took one).

**Older builds.** Nothing new on the wire: `?col=` was always part of a request and an mdview
that predates this skips it, showing the block's first line as before. An older mdedit sends no
column and this release's mdview shows the line's beginning.

## 67. Window backdrop: a frosted window on Windows 11 (2026-11-15)

**The setting.** `backdrop = off | mica | acrylic` in the settings file (`Prefs::backdrop`,
default `off`); the row "Window backdrop" sits under Theme in Settings > View and exists only in a
Windows build. `--pref backdrop=mica` works like every other preference.

**What is frosted.** The regions Title (title row and tab strip), Bar, Files (the side pane) and
Status. The text area, the minimap, the results panel, menus, the command palette, tooltips and
dialog cards stay opaque. A dialog's dimming and the soft shadows of menus fall on the frosted
chrome and darken it, as they do on the flat chrome.

**When it is frosted.** All of these must hold, otherwise the chrome is today's flat colours:

- the setting is Mica or Acrylic;
- Windows build 22621 or later (22H2: the first with `DWMWA_SYSTEMBACKDROP_TYPE`);
- "Transparency effects" is on (`HKCU\...\Themes\Personalize\EnableTransparency`, missing counts
  as on);
- high contrast is off (`SPI_GETHIGHCONTRAST`);
- battery saver is off (`GetSystemPowerStatus`);
- the system accepted the backdrop calls;
- the window has the keyboard. The system shows no material behind an inactive window, so an
  inactive window paints its flat chrome itself: pixel for pixel what it shows with the setting
  off.

These are read when the setting changes and each time the window gains the keyboard, so turning
transparency effects off takes hold the next time the window is activated. With the setting off
the cost is one comparison per frame: no system call, and the window is never made transparent.

**How.** Route 4 of the spike (`spikes/frosted/RESULTS.md` on `spike/frosted-window`):
`DwmEnableBlurBehindWindow` with an empty region makes the window's alpha channel count, and
`DwmSetWindowAttribute(DWMWA_SYSTEMBACKDROP_TYPE)` puts Mica (2) or Acrylic (3) behind it. Both are
called on the live window when the setting is first turned on; nothing changes at window creation.

The Painter is unchanged and the chrome is drawn opaque exactly as today. After a frosted region is
drawn, the alpha byte of its pixels is set to a mark (254). Everything drawn later with opaque
spans (menu, palette, dialog card, tooltip) writes alpha 255 and so clears the mark pixel by pixel;
blended spans (shadows, dimming) leave it. When a frame is copied to the window, marked pixels (and
only in the copy) are "keyed out": the key colour, the palette's plainest chrome colour (the
darkest of title/bar/pane in the dark theme, the lightest in the light one), becomes transparent,
and every other colour gets the smallest alpha that still reproduces it over the key, premultiplied.
Over a backdrop equal to the key the result is exactly today's pixels; over another backdrop each
pixel moves by `keep * (backdrop - key)`, so text keeps its subpixel rendering and its contrast
against its surroundings.

**Veil.** Mica is close to a flat colour, so the key is fully transparent (veil 0). Acrylic shows
what is behind the window, so the chrome keeps a veil of its own colour over it (150 of 255);
without it text over a busy window behind does not read well.

**Mica Alt is left out**: in the dark theme it is close to black and makes the chrome heavier than
the text area; it adds nothing the other two do not.

**Where the code is.** Shared: `mdshared_app::backdrop` (the type, keys, row texts),
`mdshared_ui::frost` (mark, key colour, veil, key-out, and the state `Frost`, tested against a
stand-in system: moved there from the editor when the branch was merged, so that mdview uses the
same one), `mdshared_ui::platform::backdrop_available` / `backdrop_allowed` / `set_backdrop`.
Editor: `bins/mdedit/src/frost.rs` (which regions), `App::sync_frost`, the mark in the region loop
of `compose`, the key-out in `present`.

**Shots.** A `--shot` has no window, so it never shows the material. `tests/shots.sh` gained
`settings-view` and `settings-view-dark` (the View section, which had no shot, with the new row
under Theme on Windows) and `settings-view-backdrop` (the row set to Mica; the chrome in it is flat,
which is the point: no window, no frost). On Linux these three show the section without the row.

## 68. AI rewriting (2026-11-19, branch `f/ai`)

Asked for by the owner on 2026-11-19, designed in mockups the owner approved the same day.

**Commands.** AI: Format as Markdown, AI: Fix Markdown, AI: Rewrite with instruction…, the saved
prompts (each "AI: <name>" in the palette), AI: Edit saved prompts, AI: Settings, AI: Stop
rewriting. In the palette always; in the text menu's AI submenu while a connection is chosen. A
command works on the selection, or on the whole document when nothing is selected, up to 200 KB.

**The engine.** `bins/mdedit-ai` is a separate program, started on the first AI command or when
Settings › AI needs it, kept for the session, and killed with the window. mdedit's release build
aborts on a panic, so the network, JSON and child-process code lives where a failure cannot take
unsaved text with it: if the engine stops, a bar says so and the text is untouched. It speaks JSON
lines on stdin and stdout (`bins/mdedit-ai/src/proto.rs`). Connections:

- Claude Code (local): `claude -p` with stream-json, no tools, no session kept, `--model`, `--effort`.
- Codex (local): `codex exec --json --ephemeral --sandbox read-only`, model and
  `model_reasoning_effort` with `-m` and `-c`.
- Claude API: Messages, streamed, `output_config.effort`; models from `/v1/models` with the effort
  levels each takes.
- OpenAI-compatible: `chat/completions`, streamed, `reasoning_effort` only when set; presets for
  OpenAI, OpenRouter, Groq, Together, DeepSeek, Mistral, Gemini, xAI, Ollama, LM Studio, or a
  base URL.

API keys are kept in the system's credential store (`keyring`), under the service "mdedit", one
slot per connection or provider, and for "Custom" one per base URL (`openai:custom:<url>`, the
scheme and host in lower case, no trailing `/`), so a key typed for one server is never sent to
another. In the credential store that slot is named by the hex SHA-256 of the URL
(`keys::target`): Windows compares credential names without case, and a URL can be long. A key
draft belongs to the slot it was typed for and is dropped when the endpoint changes; a new base
URL's key is looked up 500 ms after typing stops. A key is at least 4 characters. mdedit sends a
key to the engine once, when it is saved, and never writes it to settings.ini.

The engine takes keys out of every error message before it leaves the process (`proto::redact`):
the active key wherever it appears (from the raw error body, before the body is cut for the
message); a credential-looking word after `Bearer ` or `x-api-key:` (16+ characters with a digit,
or 32+; "Bearer authentication is required" stays); a word of 20+ characters starting like a known
provider's key (`sk-`, `ghp_`, `github_pat_`, `xai-`, `gsk_`, `AIza` and the other GitHub
prefixes).

**Streams.** The answer is read and parsed on its own thread (`sse::read`), so a cancel ends a run
within 0.2 s even when the server sends nothing; nothing for 3 minutes ends it as a network
failure; a line or an event over 1 MB, or an answer over 2 MB, ends it as too long; at most 8
events wait read ahead. The thread stops at the answer's last event (`message_stop`, `[DONE]`) and
lets go of the body. After a cancel, an idle end or an error it lives until its read returns, at
the latest at the agent's 30-minute body timeout; it holds no lock, job or slot of the engine. An
error status's body is read the same way (`sse::read_body`: 64 KB, 20 s idle, cancel). A CLI whose
`--version` exits with an error is not taken as found.

**In the window** (`src/ai.rs` the client and state, `src/app/ai_events.rs` the behaviour,
`src/ai_ui.rs` the drawing). While a rewrite runs the status strip says so and Esc stops it; the
editor stays usable. The answer is cleaned (one code fence around everything is taken off; the
line break at the end follows the original; the line breaks become the file's), then:

- with "Show changes before applying" (on by default), the review: Changes, the lines removed and
  added with unchanged runs folded, or Preview, the text before and after drawn side by side by
  mdview's renderer (two preview panes, opened the first time), scrolling together. The choice is
  remembered. Apply, Copy result, Discard.
- without it, the result goes in at once.

Applying is one undo step (`edit::replace_range`). The range is checked against the text it was
taken from when the answer comes and again on Apply (a reload, an edit in another view). When it
cannot go in — the text changed, the document is read-only or was closed — nothing is put in: the
review says why in a band over the result and offers Copy result and Run again. A read-only
document with "Show changes" off gets that review too, so the result can still be copied. Run
again goes back to the review's document and opens the instruction panel there, filled with the
instruction (and keeping its title); nothing is sent until Rewrite. The instruction panel keeps
the document, range and text it shows; if another tab, selection or text is current when Rewrite
is pressed, the panel shows the new scope ("Changed since: …") and sends only on the next Rewrite.
A document closed while its rewrite runs, or while its review is open, keeps the review with Copy
result and Discard (no Run again).
Failures show in a bar with the one thing to do (Open AI settings, Try again).

**Saved prompts** are `ai-prompts.md` beside the snippets file: `## Name` then the instruction.
Edit opens it in a tab; it is read again when it changes.

**Settings › AI**: Connection (with who receives the text under it); the program and a Check for
the CLIs; Provider and Base URL for OpenAI-compatible; the API key (drawn as dots, never copied)
with Save or Remove; Model (fetched, or Claude Code's aliases, or Other… with a name); Fetch
again; Effort (the model's levels, or the connection's); Show changes before applying; Saved
prompts › Edit…. The rows are made again as answers come in.

**Budgets.** Nothing is started at launch and nothing is read until an AI command or the AI
section is used; the engine is not in the process at all until then.

**Shots.** `--ai-demo instruction|review|preview|stale|closed|working|bar|settings` shows each state with
a made-up result and no engine (`--pref ai_connection=claude-api` to choose the connection).

**Not run** (2026-11-19): real calls with real keys, the Codex CLI, a signed-in Claude Code
rewrite, the credential store on Windows and macOS, anything on Windows.

## 69. The plan screen (2026-11-21)

The card of `docs/ai-library-plan.md` 5.3: the notes a split would make, read and changed, and
nothing written. Since phase S3 (D-081) the card is shared: `mdshared_ui::plan` (`plan.rs`:
state, events, the numbers of the layout, the strip's arithmetic, the files each note would be
written as; `plan/draw.rs`: drawing; `plan/demo.rs`: the documents of the shots and its tests)
in markdown-shared-rs. It takes the document as an `Arc<dyn mdshared_app::split::Lines>` and a
key as its name plus whether it was the context-menu key. What stays in mdedit: `plan_ui.rs`
(re-exports the card and finds the headings with the function list, `headings_of`),
`plan_demo.rs` (`--plan-demo`), `app/plan.rs` (the window's part: opening, closing, parking,
routing input). The notes, the rest, the warnings and the counts are all `mdshared_app::split`,
kept in mdedit-core as `mdedit_core::split`, which adds `Lines` for `Buffer` (the rope's lines
and chunks) and `split::document`; the caps (`SPLIT_MAX_NOTES` and the rest) are the shared
ones, re-exported by `limits`. The list's and the preview's row arithmetic is
`mdshared_ui::rows`.

- **The screen is over everything**, like the palette: while it is open it takes every key,
  pointer, wheel and typing event, and its tick, hover, press and caret are in the overlay
  signature in `app.rs`. A change of the look that does not raise `tick` is never repainted.
- **Rows** are the notes and then "The rest", one more row than there are notes. The selected
  row is the preview's; the strip's bands name the same row numbers.
- **The strip** is `strip_bands(plan, width)`: widths from `widths`, which keeps 2 px for every
  note and shares out the rest in proportion, falling back to a plain proportion when the
  minimums do not fit. Edges are whole pixels, so the bands tile. Separators only when every
  note band is at least 6 px.
- **Where the notes go** is `retarget`: each note's folder is the group's folder plus its
  sections, its name is `folder_name(note_slug(title))`, the title in lower case with hyphens
  (D-075, D-083); the disk is asked through a `Reader` (the shots
  and the tests give a table), once for each name, remembered until the group changes.
- **Escape** closes the card; a plan that was changed is parked in `App::plan_parked`, and
  `plan_show` brings the same screen back, scroll and selection included.
- **"Write the notes"** records `write_pressed` and returns `Act::Write`; the window does
  nothing with it yet.
- Shots: `--plan-demo split|split-gaps|split-rest|split-warn|split-again|split-rename|split-many|split-small`.

## 70. The write, the record and undo (2026-11-21)

`docs/ai-library-plan.md` 4.1, 4.3, 4.4, 4.5, 5.9, 6.2, 6.3. The model is `mdshared_app::runs`,
re-exported as `mdedit_core::runs` (shared since phase S3, D-081; no window, no AI: `Request`, `Report`, `Record`, `Run`, `UndoPlan`, `Gate`), the window's part is
`bins/mdedit/src/app/plan_write.rs` (the request a plan makes, the gate, the report, undo, Finish,
"AI runs…"), and the screen gains nothing but a `problem()` and `root()`.

- **The order is 4.3's, and every prefix is a state one can live with.** The record first; then
  the folders; then the pictures; then the notes; then the read-back; then the library file; then
  the record closed; step 8, the original in the Recycle Bin when the card's box is ticked, last
  (77). Nothing is linked before it exists, and the source is otherwise read and never written.
- **Creating is `mdshared_app::files::create_new`**, which checks the name, writes a temporary
  beside it and renames without replacing: a file is never overwritten, and a name taken between
  the check and the rename is refused, not replaced. Pictures are `name.ext`, or `name 2.ext`
  when that name holds other bytes. The group's library file is made from `# Name` and its
  `home:` line and refused at 512 library files.
- **The record** is `<library>/.runs/<UTC>-split.json`, with absolute paths made relative to the
  library folder first. `state` is `writing`, `links pending`, `done`, `stopped` or `undone`; its
  `notes[]` holds each note's path, SHA-256, line ranges, heading shift, sections and the exact
  line that was written into the library file. **It says what happened, not what was planned**
  (D-069): `created` is on each note and picture and on the group, set only when this run made
  the file, and `folders` lists only the folders that were not there, each parent the run made
  included; the record is written at step 1 with `created` false and no folder, again after the
  notes are written, and again when the group's file is made. A record with no `created` says
  false. `RECORD_VERSION` is 1: `prune` keeps the newest 20 or the last 90 days **of the records
  that load**, and a file that does not (a later version's, or not a record) is never counted and
  never deleted; `list_all` gives those as `Unread`, which "AI runs..." draws with nothing in the
  row to press.
- **The gate.** Every library write goes through `App::library_attempt` (refuses a file open with
  unsaved changes, re-reads it, checks the new text, writes it atomically), and the check is
  `runs::check_links_added` / `check_links_removed`: the new file must be the old notes plus (or
  minus) exactly the planned lines, under their sections — a count is not enough. One call per
  library file, one write.
- **Undo** asks first, with the numbers, and acts by these rules, all in `runs::undo` and
  `runs::prepare` and none in the window. (0) **What the record may reach** (`Reach`, D-072): the
  library folder, or the home the group's library file declares now, read from that file; no `..`
  in a path; the group's file a `.md` directly in the library; notes `.md`, pictures an image
  extension; an empty or relative library is none; a path that fails refuses the whole record,
  naming it; `inside` compares part by part in the platform's case after the disk has been asked
  where links lead, and an empty folder is nothing, not everything. (1) **The original** is
  compared by length and SHA-256 always (`source_state`): gone, changed, or too large to compare,
  undo is refused (D-071); a recycled one must be back. (2) The links, first: the exact recorded
  line under the recorded section; a line edited or moved is kept with its note. The links and the
  headings come out in one text, `runs::with_removals`, which the window's gate and the tests' gate
  both call; the gate says an error when nothing was written. (3) **The library is read again from
  its files** (`list_library`: every `.md` directly in the folder, past the reader's 512, up to
  4,096, a file over 1 MiB as the reader takes it) and **a note is removed only if no library file
  lists it** (D-070): the plan says so beforehand (`held`, `links_kept`), and `undo` checks again
  after the links step. Then only a note or picture with `created` true, a regular file
  (`symlink_metadata`, never a link), with the recorded SHA-256 read through a 64 MiB cap, and
  re-read as it goes. A file that was there before the run is `before`: its line comes out, it
  stays. (4) The group's file when `group.created` and only its heading and `home:` are left.
  (5) The folders the record lists, deepest first, while empty, never a link. (6) The record is
  marked `undone`, or `links pending` when a link could not be taken out. The panel's title reads
  "The run is not undone · {why} Try again." and the held notes are listed. `with_headings_removed`
  joins with the file's own line ending, as every writer in `mdshared_app::library` does.
  **Finish** writes the line the record holds (not the absolute path) through the same `Reach`.
  Undo's removals go to the Recycle Bin where the file's drive has one, and are a plain delete
  where it has none (77).
- **"AI runs…"** lists the records when it is opened (never at start), newest first, with "Show
  the notes", "Undo this run…" and, for a run that stopped, "Finish".
- **"Split into notes…"** is in the tab's menu, the palette, the AI menu and the Notes library
  submenu. It is disabled with "Save {name} first: the notes are made from the file on disk."
  until the document in front has a file and no unsaved changes.
- **The Notes pane's foot** draws "Plan ready: N notes · Show · Drop" while a plan is parked; the
  count and the parked flag are in the Files region's signature.
- Shots: `--dialog runs|run-undo|run-undo-recycled|run-undo-original|runs-newer`, and `--plan-demo split --plan-parked`.
- Tests: `mdshared_app::runs` (24 in the module, in the shared repo) and `crates/mdedit-core/tests/undo_rules.rs` (27: each
  of D-069 to D-072 forced on disk, and each one fails with its fix reverted), `app::plan_write_tests`
  (8) and `app::undo_rules_tests` (5: the window's gate, the flat undo, the refusal when the
  original is gone, the list of unreadable records).
- **Not built:** the progress row and Cancel (the write runs between frames), so a run is never
  left in `stopped` by this program; step 8. ("Ask the AI…" is section 76.)

## 71. The notes pane on a large library (2026-11-21)

The shared pin moved to `fb7777c`: `mdshared_app::library::Library` gained `omitted` and reads
512 files rather than 64, and `mdshared_app::paths::same_path` compares paths part by part. What
the window does with them is `bins/mdedit/src/app/library.rs`, `library_pane.rs` and `docs.rs`.
The rows the pane lists (`Row`, `Kind`, `rows()`, the fold keys `key_of`) and what it asks of a
path (`same_note`, `listing`, `lists`, `link_path`, `without_notes`, `folder_of`, `home_of`,
`empty_state`, `omitted_label`) are `mdshared_app::library::tree` since phase S (shared
`ecfcdf9`), so mdview's Notes face can list the same tree; `library_pane.rs` re-exports them and
keeps the drawing, the cursor, folding and the name box.

- **The re-read runs on a job, never in the event loop.** `library_tick` still does the cheap
  look (`Library::changed`: the folder's names and their stamps, 2.25 ms on the big library) and
  decides whether a read is worth starting. The first read after the pane is shown stays where it
  was; every later one is `mdshared_app::job::Job::start`, with `Wake::Library` as its wake. At
  most one is kept (`library_job`): a tick that finds one running waits for its result, and the
  next tick after it finishes reads again if the folder moved meanwhile.
- **A stale result is dropped.** `library_accept` calls `read.changed()` first: when a stamp moved
  while the thread read, the result mixes two states, is not shown, and `checked` is cleared so
  the next pass reads at once instead of waiting out the interval.
- **The rows do not flicker.** Nothing touches `ui.library.rows` until `library_accept` has a
  result worth showing; the rows, the scroll and the keyboard's cursor are the ones on screen
  while the read runs. `build` restores the cursor by `RowId` (the note's path under its heading,
  or the heading's key), so it follows its note when the rows arrive from the thread.
- **Nothing is left behind.** Closing the pane, switching its face away from Notes, and changing
  the library folder cancel the job; a result that arrives after one of those is dropped by
  `library_accept`'s pane and folder check. A cancelled job returns nothing to show (`None`), and
  a finished one is taken once.
- **The staleness check keeps no notes.** `Docs::library_stamp` is a slim `Library`: the same
  groups with their file and stamp and `omitted`, with no name, home, sections or entries.
  `Library::changed` is the same call over it, so a hidden flat note still does not make the
  pane re-read; the second whole library that used to be kept for this is gone (`stamp_of`).
- **A capped folder says what it left out.** When `omitted > 0` the pane reserves one row at its
  foot (above the "Plan ready" row when both show) and draws, in its dim style and with a
  separating line, "512 of 583 groups shown": `omitted_label(shown, read, omitted)`. The shown
  count is the groups drawn, the total is the groups the reader kept plus those it left out. The
  count and the kept groups are in the Files region's signature.
- Tests: `mdshared_app::library::tree::tests` (the omitted count is copied by `without_notes`,
  and the label's two numbers) and `app::library_pane_tests` (the text arrives from the thread with the old rows
  still on screen, a stale result is dropped and `checked` cleared, the cursor follows its note,
  closing the pane and switching the face drop a running read, the slim copy holds no notes, and
  the omitted count repaints the pane).
- Measured in `docs/measurements/2026-11-21-library-rework.md`: a library change's event-loop
  pass 40.3–48.9 ms before against 2.3–2.6 ms to start the read and 5.8–6.2 ms to take its
  result; private memory 31.50 MiB against 30.90 on one 60-file library.

## 72. Visual editing follows the tab (2026-11-21)

The owner, using it: in visual editing (`Ctrl+E`) switching to another tab dropped the window
into the text and the page side by side, and switching back did not bring the mode back. The
mode is the window's, not one file's: `bins/mdedit/src/app/visual.rs`.

- **Another tab the page can show is followed.** `visual_sync` compares the active document with
  `Visual::doc`; when they differ and the new document is `previewable`, `follow_tab` takes it:
  the old document's block closes (its room in the page and the question about it go with it),
  no block is open on the new one, and the page starts where that document's text was, at its
  caret, as `toggle_visual` starts a fresh pane.
- **The placement waits for the text.** `Visual::place` is set on every hand-over and cleared by
  `place_page` once the pane has that document's text (`Pane::shown`, and not `held`), so a tab
  whose file is still being read is placed when its text arrives rather than showing the top.
  The page is asked for the caret's line and then told to forget it, so the reader's own
  scrolling is left alone from there on, as in the mode.
- **A document the page cannot show ends the mode** (a `.rs` file; also no tab at all), and the
  pane goes back to what it was before visual editing: `Visual::pane_opened` says whether visual
  editing opened it. `end_visual` closes a pane it opened and leaves one the owner had open;
  `toggle_preview_pane` closes the pane in either case, because that is what it was asked to do
  (`stop_visual` is the shared half that does not touch the pane).
- **It does not come back by itself.** Nothing turns the mode on when a Markdown tab is shown;
  only `Ctrl+E` does.
- **Closing tabs.** Closing the tab the mode is on follows the tab that takes its place; closing
  the last one ends the mode, and the pane it opened goes with it.
- **An answer about the old text is dropped.** A block answer the page still owed when another
  document took the page is about the old text: `Pane::set_text` clears the answer it holds and
  sets `block_stale`, `poll` drops the one still to come, and an ask about the new text clears
  it. Without that, switching tabs right after a click on the page could open a block of the old
  document's lines in the new one.
- Tests: `app::preview::tests` (`visual_editing_follows_the_tab_to_another_markdown_document`,
  `a_file_the_page_cannot_show_ends_visual_editing_and_closes_the_pane_it_opened`,
  `a_file_the_page_cannot_show_leaves_a_pane_the_owner_had_open`,
  `visual_editing_follows_a_closed_tab`, `closing_the_last_tab_ends_visual_editing`) and
  `preview_pane::tests` (`a_block_answer_the_old_text_was_still_owed_is_dropped`).

## 73. Check first, then write: the plan's guards (2026-11-21)

Audit 3's D2 and A6, A7, A8, A11, C1, C2, C3, C5 and C7 (`docs/review-prompts/audit3-report.md`),
against `docs/ai-library-plan.md` 4.3 and 6.2. The writer is still `mdedit_core::runs`; the checks
are `bins/mdedit/src/app/plan_write.rs` (`plan_request`, `plan_preflight`, `plan_blocked`), and the
screen (`mdshared_ui::plan`, re-exported by `plan_ui.rs`) and the window (`app/plan.rs`) carry
what they need.

- **Everything that can be refused is checked before the first byte, and the reason is the card's.**
  `plan_preflight` refuses a library root that is empty or relative, a group's folder that is the
  library folder, a note that would be a `.md` file directly in the library folder, a new group's
  file name that is taken, the library's file cap and the group's note cap, a group's library file
  open with unsaved changes, a source whose length or SHA-256 is not the one the plan was made
  from, and a link insertion whose dry run (`with_links_added`, then `check_links_added`) would not
  read back. `plan_blocked` is the same check, worked out when the screen opens, when the
  destination changes and when a kept plan is shown; the footer draws it and disables the button,
  and `plan_write` runs it again so a press that gets past the screen is still refused before
  anything is written.
- **The group's file is made before the notes.** `plan_write` makes it with `create_new` once the
  checks pass, then runs `runs::write` with `group.created = false`, and closes the record again
  with `created = true` when the run started: an empty group is a state one can live with, and
  undo takes it away. What step 6 can still fail on is a race.
- **C1, fixed in the shared writer.** A note with no section is written in the group's own notes:
  after the file's `#` heading and its `home:` line, after the notes already there, and before the
  first section heading. It is `library::add_note_in_group` (and `move_note_in_group` for "Move
  to…" on a group's own row), a variant beside `add_note`, which is unchanged byte for byte
  because mdview pins the shared crates (rule 8). The run's links (`runs::with_links_added`), "Add
  this file" (`library_put`), "New note here…" (`library_new_note`) and "Move to…" (`library_move`)
  all use it; the refusal the dry run made for this case is gone. A file with no section heading
  still takes the note at the end, exactly as `add_note` does, and the `home:` line is never
  written above.
- **C3 is the dry run too.** The shared reader strips a heading's trailing `#`, so `## C#` reads
  back as `C`; the dry run catches it, and the refusal names it. A group called `C#` is caught by
  the read-back check the preflight now makes on the text it would write.
- **The destination carries its file.** `Dest` holds the group's library file, so two groups of one
  name send the notes and the links to the file that was chosen; nothing is looked up by name
  again. Two titles that differ only in case are compared with `mdshared_app::paths::same_path` and
  the second gets `name 2.md` before either is written.
- **The plan reads the library first.** `plan_open_active` loads the library, so every route into
  the plan (the tab's menu, the palette, the AI menu, the Notes library submenu) has the groups it
  already holds, and it refuses for want of a library folder. `open_runs` lists the records from
  `library_dir()` and the undo paths resolve against it, so one record has one folder.
- **A kept plan carries its source.** The length and SHA-256 of the file are taken when the plan is
  made; showing it again after the document changed refuses with "… has changed since this plan was
  made. Save it, then split it again." A document whose bytes are not text (`info.lossy`) is
  refused where the plan opens, as Save As refuses it.
- **A group's folder may not be the library folder.** Refused where the folder is picked, where a
  `home:` line is read ("New note here…"), and by the plan's check: its notes would be `.md` files
  the reader takes for library files.
- **No session, no library.** `snippets_path` (and so `library_dir`, the snippets and the saved
  prompts) is `None` unless there is a settings file: `--no-session` reads nothing of the owner's,
  and `MDEDIT_SETTINGS` is the one file such a build reads, with its library beside it. The empty
  path no longer passes the confinement check, because no plan is opened without a library folder.
- Tests: `app::plan_write_tests`, one per fault above (the taken name, the destination changed to
  an unsaved file, a sectionless note written into a group with sections, both caps, a `C#` section,
  two titles in one case, two groups of one name, a kept plan after a save, a lossy document, a plan
  opened before any read, "AI runs…" before any read, and the group's file made before its notes);
  `app::library_write_tests`, one for each pane writer on a group's own row ("Add this file", "New
  note here…", "Move to…", in one file and in two).
  The old test of step 6 failing was restaged on a `.tmp` file in the way, since a taken group name
  no longer reaches the write.


## 74. Move the notes: the core (2026-11-22)

`docs/ai-library-plan.md` 5.12. `mdshared_app::moves` (shared from the start of phase S3, D-081),
headless, beside the record it writes to (`runs`). The window side is not built.

- **Plans.** `plan_group(library, group)` has a row per listed note that is not where its group's
  folder and headings put it (`Start::Folder` or `Start::Headings`); `plan_library(old, new)` has
  the library files, the notes inside the old folder and up to `RECORDS_MAX` (400) records, the
  newest. A row is blocked, not dropped, for a taken name (case folded where the disk folds it),
  two notes going to one name, a note listed twice, a symlink, a missing file; a note from outside
  the library comes unticked and says where it lives.
- **Preflight** refuses before the first file: the folders and every file as planned (digest and
  length), every destination free, no library file or note open with unsaved changes (the gate's
  `free`), and every library file read as UTF-8 and dry-run with the lines it would get. A file
  the reader lists from but that is not UTF-8 is refused by name (D-084).
- **The write**: the record first, with every file marked done; the folders; each file by a rename
  that never replaces, or across a volume a copy made new, read back and compared; the library
  files through the gate, each edit checked against the line it replaces; the old copies after
  the lines name the new ones; the emptied folders, deepest first. A refused library file rolls
  everything back to the byte and the record says so, keeping any folder it could not remove.
- **Undo** reads the record's paths through `reach_of` (D-072): a note that came from outside the
  library stays where the move put it (D-084); every other file goes back only while its digest
  is the one moved, nothing is at its old place, and its line is the run's (D-070). `undo_preflight`
  makes a write's refusals before the first file goes back.
- **A note's pictures and links (D-091)**, `moves::carry(&mut plan)` after a plan is made, and
  `moves_links.rs`. A row it has not seen (`Row::carry` is `None`) moves as before D-091. It reads
  each movable note (UTF-8, up to 8 MiB; another note's links are left alone) for its relative
  inline links, pictures and reference definitions with `wikilink::scan` (so fenced code and code
  spans are skipped), and passes over absolute paths, schemes, `#anchors`, wiki-links, HTML,
  folders and targets that are not there. Who else links is read from every library file, every
  note of every group, and every `.md` in the folders the notes and their targets are in (up to
  10,000 files; past that, or one that cannot be read, nothing is carried); a wiki-link counts by
  name. A target is **carried** when this note is its only user (two links from the note are one
  user), it is a plain file inside the note's folder, not a `.md`, not in `.runs`: it goes to the
  same place below the note's new folder, a `Carried` row on the note with its digest. A taken
  name, two files going to one name, a path too long or a file in the way blocks the note's row
  (`Verdict::CarriedTaken`, "there is already a file called map.png where its picture would go").
  Every other target stays and the link is counted in `Carry::rewrites`.
- **Carrying in the write.** A note's carried files are rows of their own after it in the record
  (`FileKind::Carried`, `with` naming the note), moved and checked like the note (`preflight`
  makes the same refusals for them), never moved when their note did not move, and put back with
  it in a rollback. After the files move, each moved note with `links` is read again and every
  link whose target exists, before or after, is pointed at where its file is now
  (`links::rewrite`): forward slashes, the characters the source escaped with `%` escaped again
  (a space always outside `<…>`), a relative path, or the absolute one on another drive. The new
  text is written whole through `MoveGate::rewrite` (default `rewrite_note`: one atomic write,
  only while the file says what was read), after the library files and in the same unit: a
  refusal rolls everything back. The lines are recorded as `ChangeKind::Link` changes, and the
  note's digest is the rewritten one. A record with carried files or link lines is not read by
  builds before D-091, so they cannot half-undo it.
- **Undoing it.** A carried file goes back only while its note does, else it is kept ("the note it
  went with stays where it is"); the note's lines go back to the exact words the record holds when
  those still lead where the recomputed line does, so `<./pics/../pics/s x.png>` comes back as it
  was written.
- Tests: `mdshared_app::moves` (61, `moves_tests.rs`, and 2 in `moves_links.rs`), one per rule of
  5.12 and of D-091 and per failure point, each failing with its fix reverted. Not done: links
  from other notes *into* a moved note, `<img src>` and wiki-links are not rewritten.

## 75. Move the notes: the window (2026-11-22)

The card, the write and undo of section 74's core, in the window. `move_ui.rs` (state, events,
the layout's numbers, what a row says, the shots' plan), `move_draw.rs` (drawing) and
`app/moving.rs` (the window's part: the gate, the disk, opening, the write, undo, `--move-demo`).

- **Where it is offered.** "Move the notes…" on a group row's menu (`LibAct::MoveNotes`) runs
  `moves::plan_group` and then `moves::carry` (D-091). A plan with no row opens nothing: its `blocked` reason, or "Every note of
  Work is already where it belongs", is a toast. The library move is not offered: the Settings row
  for the library folder is a text field and a picker, with no place for a second step.
- **The card** is the plan card's family made small: 880 x 620 through `dialog_card`, a 92 px
  header (the kind of move, where the files go, and the notes the plan keeps: missing notes,
  files that stay behind), "FILES (n)" with Tick all and Untick all, 62 px three-line rows (the
  tick, the name and beside it, faint, what the note carries and rewrites, "and 2 pictures · 1
  link rewritten" (`move_ui::aside`); from → to, each shown below the group's folder or the library and cut from the
  front when the two do not fit; why it moves, or the core's verdict in the warning colour), and
  the footer ("3 of 7 files · 3 cannot move", Cancel, **Move**). A blocked row draws a disabled
  tick and `Plan::set_ticked` keeps it unticked; an outside row comes unticked and is ticked by
  hand. Move is disabled with its reason while nothing is ticked. Escape closes it. It is routed
  through the plan screen's entry points (`plan_is_open` is true for either) and has its own
  signature in the overlay's.
- **The write.** `preflight`, then `write`, through `MovesGate`: `free` is
  `library_file_free` (a tab with unsaved changes refuses), `edit` is `library_attempt_at` with
  `moves::apply_edits` as the change, `rewrite` (a moved note's links) is `library_file_free` then
  `moves::rewrite_note`. A refusal keeps the card, its ticks and the sentence in the
  footer and on the bar; nothing has moved. The disk is `Disk` with `docs::move_no_replace`
  (`MoveFileExW` without `MOVEFILE_REPLACE_EXISTING`; a hard link and an unlink elsewhere), whose
  `ERROR_NOT_SAME_DEVICE` is given to the core as `CrossesDevices` so it copies. After it: tabs
  open on a moved or carried file follow it (`Docs::follow_move`), the library is read again, and
  the results panel lists what moved, what went with the notes, the notes whose links were
  rewritten, what was kept and why, what could not move, and the folders removed, with "Undo this
  run". Undo's panel lists the carried files under "Put back with their notes".
- **"AI runs…"** lists a move by `moves::headline`; "Undo this run…" asks with `moves::prepare`'s
  sentence and answers with `moves::undo` through the same gate; "Show the notes" opens the first
  moved note where it is; Finish is not offered for a move.
- Shots: `--move-demo move` (`move-notes`, and `move-notes-small` at 800x600) and `--move-demo
  results` (`move-results`), both themes, from a plan made in the program: three notes carry
  pictures or a file or rewrite links, and "Trip photos" is blocked because `map.png` is taken.
- Tests: `move_ui::tests` (4) and `app::moving_tests` (9).

## 76. Split by AI (2026-11-22, branch `f/split-ai`)

`docs/ai-library-plan.md` 5.2, phase 4b: "Ask the AI…" on the plan card. The plan at headings
opens first, with no request; the AI is a second step on the same screen.

- **The contract is one shared module.** `mdshared_app::split::answer` holds the system text
  (`SYSTEM`), the outline (`outline`: each heading in full up to 200 bytes, the first 80 bytes of
  each paragraph, fence-aware, with line numbers; headings only when that passes 200 KB; refused
  as `TooLong` when even the headings do), the prompt (`<instruction>` and `<document name lines>`
  tags, the name's `"<>` and controls turned into `_`), the retry prompt, the reader (`read`: the
  first `{…}` in the text through the depth-bounded `runs::parse_json`, every field optional, at
  most 200 notes kept) and `plan`, which turns an answer into a `Plan` by checks 4 to 10 in order:
  whole numbers in the document, an end not before the start; a boundary inside a code block or a
  table moves past it, inside a paragraph to its nearer edge; in order, an overlap starts after
  the note before; blank ends trimmed; a heading's words are the title, else the model's, cleaned
  and cut to 120 bytes, else "Lines a-b"; three sections at most, dot-only names dropped, the
  group's existing spelling wins without regard to case. What was dropped is counted
  (`Dropped`). Nothing in it can panic; a fuzz test feeds it 400 random answers. The engine is told
  `system` and `prompt` and nothing about splits.
- **The card.** `PlanScreen.ai_offer` (off unless the window sets it: a connection is chosen)
  draws "Ask the AI…" beside the level switch. `set_ai_plan` puts a plan from the AI on the screen
  with an `AiCut`: the header shows the instruction, "N notes · by {connection · model · effort}"
  ("· sent the headings only" when so) and "At headings", which cuts at headings again. A row
  whose boundary the checks moved says why (`Fix`: a code block, a table, a paragraph's edge, an
  overlap). `ai_notice` is the window's line above the list (asking, unreadable, too long, a
  failure), with its links (`AiAct`: Stop, Run again, At headings); it comes before the dropped
  line, which comes before the warnings.
- **The window** (`app/split_ai.rs`). "Ask the AI…" opens the instruction panel in split mode
  (`Instruction.split`); the plan waits in `App::plan_behind` and comes back when the panel
  closes. The panel lists only prompts of `kind: split` (`ai-prompts.md` gains an optional
  `kind:` line; Save prompt writes it), and says what is sent and its size. Split sends one
  request (`Ask::Split`); the screen says it is asking, with Stop, and Esc stops it too. An answer
  that cannot be read is asked for once more with the fault named; a second says so, with "Run
  again" and "Split at headings instead". The record of the write keeps the instruction and
  `sent` (connection, model, bytes, requests).
- **One request, so no parallel setting.** A document is one request; D-080's per-connection
  number of requests in flight has nothing to run here and is left for the first phase that sends
  several.
- **Not built:** "Show what is sent" (3.10); merging or moving a note from the AI's plan beyond
  what the card already offers.
- Shots: `--plan-demo split-ai-ask|split-ai-asking|split-ai|split-ai-dropped|split-ai-unreadable|split-ai-long`,
  with a connection given by `--pref`, both themes; the answers are made in the program and read
  by the same reader, and no request is made.
- Tests: `split::answer` (shared: every check of 5.2 has an answer that trips it), the card's AI
  tests in `mdshared_ui::plan` (5), `app::split_ai_tests` (6, with the stand-in engine of
  `ai_tests`), and `mdedit_core::ai`'s `kind:` tests (2).

## 77. The original in the Recycle Bin (2026-11-22, branch `f/recycle`)

`docs/ai-library-plan.md` 5.4, phase T, D-046: a box on the plan card puts the document the notes
were cut from in the Recycle Bin, as step 8 of the write, last.

- **The dependency is `trash` 5.4.0, pinned exact, behind `mdshared_ui::platform::recycle`** under
  the cargo feature `recycle`, off by default: mdview never builds it, mdedit opts in. Its default
  features are off (no `chrono`, no `jiff`), `coinit_apartmentthreaded` and `freedesktop` on. It
  adds one package to `Cargo.lock` and 30 KiB to the exe, and nothing runs at start.
- **What the system is asked** (`platform/recycle.rs`). `recycle_refusal(path)` says why a file
  cannot go, in the card's words (`BinRefusal`): on Windows a UNC path, or a drive that is not a
  local fixed one (`GetVolumePathNameW`, `GetDriveTypeW`), has no bin; the `NoRecycleFiles` policy,
  the volume's "delete instead" setting (`BitBucket\Volume\{GUID}\NukeOnDelete`) and a failing
  `SHQueryRecycleBinW` make it unavailable, with the reason. Elsewhere the file must be on the same
  device as the home bin (`$XDG_DATA_HOME/Trash` or `~/.local/share/Trash`; `~/.Trash` on macOS).
  `recycle(path)` asks again, takes only a regular file, runs `trash::delete` on a thread of its
  own (the crate's COM start panics on failure, and a panic ends the release build: on a fresh
  thread there is nothing to fail against), and checks the file is gone. On Linux it first checks
  `/proc/mounts` is text, since the crate unwraps it. `open_recycle_bin()` opens it in Explorer;
  elsewhere it says it cannot.
- **The card** (`mdshared_ui::plan`). `PlanScreen.recycle: Option<RecycleBox>`, `None` unless the
  window sets it, so a caller that does not is drawn as before. The row reads "Put {name} in the
  Recycle Bin"; ticked, "Every line is in a note, so nothing is lost. It can be restored from the
  Recycle Bin.", and the footer's sentence ends "then puts {name} in the Recycle Bin.". Disabled,
  it gives the reason: the window's (`refused`: no bin on this drive, only for one document at a
  time, not available here) or "N lines are in no note." with the one-click answer "Keep them in a
  note called 'The rest'", which ticks the rest and nothing else. The owner's wish (`wanted`) is
  kept while the box is disabled; then the strip's gaps, the legend's swatch and the reason are in
  the warning colour.
- **Step 8** (`runs::recycle_source`, shared). Only when every planned note was written, read back
  and listed, the record is closed as done, and the source is still the length and SHA-256 that
  was read; then `Bin::recycle`, then `source.recycled` in the record and a `recycled` step line.
  The window asks the drive again first, since the card's answer may be old. A refusal leaves the
  source where it was and says so in the toast and in the report ("Not in the Recycle Bin");
  success adds "In the Recycle Bin" and, when a library file lists the source as a note
  (`runs::listed_in`), "is still listed under {group}; it will show as missing."
- **Undo** (`runs::undo_with`). A run whose source is in the bin is refused until it is back
  ("Put '{name}' back from the Recycle Bin first, then undo."), and the question then has "Open the
  Recycle Bin". Its removals go to the bin where the drive has one (`recycle::ForUndo`), with "The
  notes go to the Recycle Bin." in the question, and are a plain delete where it has none, as
  before. A file the bin refuses is kept and listed apart ("Not put in the Recycle Bin, kept").
  D-071's second question ("undo anyway") is not built: the bin emptied means undo stays refused.
- **The window holds a `Box<dyn RecycleBin>`** (`bins/mdedit/src/recycle.rs`): the system's in a
  build, `NoBin` in a test window, and a test's `FolderBin` (a folder files are moved into, with
  its calls counted) where a test wants one. No test reaches the system's bin, except the shared
  `#[ignore]` test that recycles a temporary file of its own and is run by hand.
- **Not built:** restoring from the bin (the owner does it in Explorer); a run of several documents
  (the reason exists, and nothing sets it yet).
- Shots: `--plan-demo split-bin-offered|split-bin|split-bin-gaps|split-bin-wanted|split-bin-drive|split-bin-several|split-bin-system`
  and the reports `split-bin-report|split-bin-kept`, both themes; `runs-undo` and
  `runs-undo-recycled` now show the sentence and the button (they ask this machine's bin).
- Tests: `platform::recycle` (shared, 4, and 1 ignored), the card's `recycle_*` (6),
  `runs`' step 8 and undo (9), and `app::recycle_tests` (11).

## 78. The Preview button's modes, New and Save on the bar, plain and rich paste (2026-11-22, branch `f/issues-7-8`)

GitHub issues #7 and #8. Code: `chrome.rs` (the bar), `overlays.rs` (`preview_menu`, the empty
screen), `app/commands.rs` (`paste`, `paste_clip`, `preview_command`), `app/modal.rs` (two rows),
`mdedit-core/src/prefs.rs` (`PreviewMode`, `paste_rich`); in `markdown-shared-rs`,
`mdshared-app/src/html_md.rs` and `mdshared-ui` `platform::clipboard_html`, `cf_html::fragment`.
Tests: `app/issues_tests.rs`, `keys::tests::no_two_commands_share_a_shortcut`, the handoff and
preview tests, `prefs` and `chrome` tests, and `html_md`'s 14 in the shared repo.

- **The Preview button and its arrow.** `prefs.preview_mode` (`preview_mode = window | mdview |
  side-by-side`, default `window`) is what a click on Preview does: the pane, Preview in mdview or
  Preview side by side (`preview_command`). `chrome::preview_mode` falls back to the pane while
  mdview is not found, and to mdview in a build that offers no pane of its own. The button is
  lit when a click on it would close or stop its own mode's preview, and its tooltip says what the
  click does, so the three agree: in the mdview mode it is lit while the document is previewed in
  mdview at all, side by side or not, since `App::preview` stops either; in the side-by-side mode
  while the windows are arranged so (a click gives the arrangement up), and unlit while the
  document is only previewed (a click arranges the windows). "Arranged" is what the click finds
  (`App::side_by_side`: this window still where the arrangement put it): `compose` makes
  `links.side_by_side` agree with it (`sync_side_by_side`) beside the bar's other state, before
  the frame is laid out, so a window the user moved shows the button unlit. The button and the arrow show for
  what `chrome::previewable` offers the preview for, as the tab menu does: a Markdown file, a
  document with no file, or one previewed in mdview now. A file of another language shows the
  button only while the pane is open, and then `chrome::button_mode` is the pane whatever the
  setting: the button closes the pane, and the arrow lists the pane and the radio, not the mdview
  commands. A 20 px arrow beside it
  (`BarItem::PreviewMenu`) opens `PopupKind::Preview`: the three modes as commands that run once
  and leave the setting alone, labelled for what a click does ("Close the preview", "Stop the
  preview in mdview"), with no tick; then a disabled header, "The Preview button opens", and the
  three as radio rows (`Act::PreviewDefault`), which save the setting and close the menu. Without
  mdview the menu has only the pane, and no radio. Settings > Editor has the same choice as
  "Preview button: Here | mdview | Beside" (short, to fit 520 px).
- **The View menu no longer lists the preview.** Its entries were ticks on commands that open a
  window, read as switches, and duplicated the button. The tab menu keeps them, worded like the
  arrow's. Visual editing keeps its tick: it is a mode of the window.
- **The bar: New, Open, Save, and AI edit.** New and Save are icon buttons either side of Open, at
  every width (as Reload is), so the labels of the other buttons still fit at 1280. Save is
  enabled while the document has unsaved changes (as the main menu has it) and runs the same
  `Save` command. AI edit (label "AI edit") sits after the preview group only while an AI
  connection is chosen, is enabled while a document is ready, and opens the AI menu under itself
  (the same `ai_menu`, with its Modify button). It is in the Files group of `bar_plan`, so it goes
  with that group at narrow widths. The bar's signature (`region_sig`) carries the unsaved state,
  the AI setting and the preview mode: without it Save stayed grey after an edit.
- **The empty window** has "New file" beside "Open file…" (`Hit::EmptyNew`, the `NewFile`
  command, caret in the new tab), and its hint says a new one can be started: "Open a file, drag
  one in, or start a new one.", short enough for one line in the narrowest text area (360 px, a
  520 px window with the Files pane), which a test measures on Windows.
- **Ctrl+Shift+V pastes plain text** (`PastePlain`): the clipboard's text as it is, never an
  image link, a file link or a conversion. Preview in mdview moved to Ctrl+Shift+L: free in the
  table and the palette, and not taken by the desktops (Ctrl+Shift+E is IBus's emoji key on
  Linux; Cmd+Shift+Q logs out on macOS). A test checks that no two commands share a shortcut.
  The text menu has "Paste as plain text" too. Ctrl+V and Ctrl+Shift+V read the clipboard through
  `app::commands::clipboard`, which a test replaces with its own on its thread, so the commands
  themselves are tested without a system clipboard.
- **Rich text pasted as Markdown** (`paste_rich`, Settings > Typing, off). Ctrl+V reads the
  clipboard's HTML only when the setting is on, the document is Markdown or has no file yet (and
  no language other than Markdown was chosen for it), and the clipboard has text; nothing is read
  at start. A table is not a Markdown table: its rows come out as lines with ` | ` between the
  cells. `html_md::to_markdown` turns it into Markdown
  (headings, bold, italic, strikethrough, code, fenced code with its language, links, images that
  are not `data:`, lists to 16 levels, quotes, rules, line breaks, tables as `a | b` rows) and
  answers `None` when there is nothing to mark or the HTML is `white-space: pre` (a code editor's
  copy), and the plain text is pasted. It is a linear tokenizer with no recursion, no indexing
  that can panic, input capped at 16 MiB, output at 8 times the input; tests feed it 25,000
  generated and random inputs and every cut of a real copy. Windows reads "HTML Format" and takes
  its fragment (`cf_html::fragment`); Linux and macOS ask arboard for `get().html()`, which reads
  `text/html` (X11, Wayland) and the pasteboard's HTML (macOS): not run there yet. Word's own
  list paragraphs (`mso-list`) come out as paragraphs.
- Shots: `preview-menu`, `preview-menu-mdview`, `bar-new-save-ai`, `bar-ai-menu`, `bar-900`,
  `bar-560`, `settings-editor-preview`, `settings-typing-rich`, `settings-min-typing`, each in
  both themes, and `settings-min-editor-dark`; `empty`, the `menu-view-*` and `menu-tab-*` shots
  changed. After the review: `preview-lit-mdview` and `preview-lit-side` (the button lit; the two
  pictures are the same, the tooltips differ), `preview-menu-open-mdview` and
  `preview-menu-open-side` (the arrow offering to stop), and `empty-min-files` (the hint at
  520x400 with the Files pane), each in both themes. `--demo-link` with an mdview mode chosen
  draws as with mdview found, and side by side in that mode. The shorter hint changes every shot
  of the empty window (`empty`, `move-results`, `plan-*`, `runs*`, `update-installed*`).
