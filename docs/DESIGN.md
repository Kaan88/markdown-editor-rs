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
| Motion: the animator, the menus' fade, the Animations setting | 96 |
| Paste an image, plain paste, rich text pasted as Markdown | 62, 78 |
| Updates | 17 |
| AI rewriting, and its review with ticks | 68, 79 |
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
| `bins/mdedit/src/updater.rs` | `APP` (name, version, manifest address, the engine and the AI engine, the installer's uninstall key), `fetch` with the user agent `mdedit`, `--check-update` |
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
  two ways the shell moves the view in either mode. Scrolling goes on until the last row is at
  the view's top. A place put back (`Ready::restore_place`, D-199) sets `ViewState::fit_top`,
  and the next `geom` lowers the top to `wrap::lowest_full_top` (the last row at the view's
  bottom) once, when the view's height and width are known; without wrap `geom` clamps every
  frame.
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

The folder is `backup` beside the session file (`%LOCALAPPDATA%\mdedit\backup`, or beside the
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
`settings::pages::Form` (D-086; the design is `docs/archive/settings-redesign.md` on `mock/settings`)
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

**A viewer closed while nothing is sent (2026-11-22).** A send nobody answers finds a viewer gone
only when there is something to send; a viewer the user closed, or that crashed, while the caret
stood still left the menu's "Stop the preview" and "Previewing" up for good. Two things end such links
now, and the menus follow them:

- `Links::check` asks the courier to try `sister::alive("mdview")` (a bare connection, up to
  300 ms, on the courier's thread, after whatever waits to be sent); when nobody answers, the
  links end exactly as for an unanswered send (`take_gone` at `Wake::Link`). When the viewer does
  answer, an event ask makes the courier ask again after `RECHECK` (1 s), 2 s and 4 s (D-102,
  D-105), and no more: closing mdview's window is what hands this window the focus, and a viewer
  whose window was just closed still answers until its process ends. The check is asked when the
  window gains focus (`focus_gained`), when a menu opens from the menu bar or a key (`open_popup`;
  a submenu opened from it with `open_sub` asks nothing of its own, the check made for the menu it
  hangs from covers it) and when a tab's menu opens. A key's press or a mouse press asks too
  (`check_on_input`), at most once in 2 s; a key's release and auto-repeat do not. An input ask is
  answered once and starts no rechecks, so steady typing does not probe again and again. The
  window schedules no check on a timer of its own, and nothing is asked while nothing is linked
  (no courier). While a link is still in its first `GRACE` (a viewer it just started may not
  answer yet) a check the focus or a menu asks for is kept, and the courier waits until that
  grace ends on its own thread and makes it once, so an ask inside the grace is not lost; an input
  ask is let go there, and the next press asks again. The rechecks a check sets going are timers
  on the courier's thread, not the window's.
- The viewer's tab closing while the viewer runs: the viewer sends `?closed` (`Request::closed`,
  SISTER.md "A view closed") with the file, or with `?doc=` for a document without a file, for
  each tab the editor asked it to show (its `editor_shown`), however the tab went; when its whole
  window closes it sends one for each such tab still open before its process ends. `handed_over`
  ends that link alone (`Links::closed`; the path is made canonical first). An older viewer never
  sends it, and an older editor does nothing with it (a live update with no line).
- However a link ends (stopped, its tab closed, `?closed`, the viewer found gone or too old), the
  links note it (`Links::take_ended`) and `follow_links` lists the open menu again in place
  (`relist_preview_menus`; it recurses into an open submenu, and a menu whose items changed in
  number is closed), so a "Stop the preview" does not outlive its link. The menus it lists
  again are the Preview button's menu and a tab's menu (the View menu no longer lists the
  preview, DESIGN 78; it is still relisted for anything it may show). It runs at `Wake::Link`, after a
  `?closed`, and after every pass of the loop. `Menu::chosen` and `Menu::pointer` also ignore a
  row that names no item, so a menu listed with fewer items cannot pick one that is gone.

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

- with "Show changes before applying" (on by default), the review: Changes, a list of the changes
  with a tick each (section 79), or Preview, the text before and after drawn side by side by
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
  library folder first. `state` is `writing`, `links pending`, `done`, `stopped`, `undo pending`
  (an undo began and could not take out every link and note, D-156) or `undone`; its
  `notes[]` holds each note's path, SHA-256, line ranges, heading shift, sections and the exact
  line that was written into the library file. **It says what happened, not what was planned**
  (D-069): `created` is on each note and picture and on the group, set only when this run made
  the file, and `folders` lists only the folders that were not there, each parent the run made
  included; the record is written at step 1 with `created` false and no folder, again after the
  notes are written, and again when the group's file is made. A record with no `created` says
  false. The record also keeps, for each library file the links went into, the text it held
  before the run's first add and the digest of what its last add left (`added`, D-193), kept
  only when the file holds exactly those links added to that text and the record still reads
  back with it. A rewrite's record is measured in its longest state word, so one that fits when
  written still reads back as `undo pending` (`rewrite_fits`, D-191). `RECORD_VERSION` is 1:
  `prune` keeps the newest 20 or the last 90 days **of the records
  that load**, and a file that does not (a later version's, or not a record) is never counted and
  never deleted; `list_all` gives those as `Unread`, which "AI runs..." draws with nothing in the
  row to press.
- **The gate.** Every library write goes through `App::library_attempt` (refuses a file open with
  unsaved changes, re-reads it, checks the new text, writes it), and the check is
  `runs::check_links_added` / `check_links_removed`: the new file must be the old notes plus (or
  minus) exactly the planned lines, under their sections — a count is not enough. One call per
  library file, one write. The file is read by `library_write::library_text`: a UTF-8 byte order
  mark is set aside and written back, a file that is not UTF-8 text (another code page, UTF-16)
  is refused with "is not UTF-8 text". It is written by `library_put_text` through
  `mdshared_app::files::replace`: a temporary name of its own beside the file
  (`name.part`, then `name.<pid>.<n>.part`), never the owner's `name.tmp`; the file's permissions
  kept; a read-only file refused; nothing left behind when it fails (D-161). The preflight refuses
  a read-only group file before a byte is written, so the plan's card says so and its button is
  disabled; the gate would otherwise refuse it after the notes are on disk.
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
  both call; the gate says an error when nothing was written. While the file still holds what
  the run's add left (`added`), the window's gate gives back the text from before the add, byte
  for byte, blank lines, line ends, a missing last newline and the byte order mark as they were
  (`with_removals_added` through `Gate::remove_links_added`, D-193); a file changed since is
  undone line by line. (3) **The library is read again from
  its files** (`list_library`: every `.md` directly in the folder, past the reader's 512, up to
  4,096, a file over 1 MiB as the reader takes it) and **a note is removed only if no library file
  lists it** (D-070): the plan says so beforehand (`held`, `links_kept`), and `undo` checks again
  after the links step. Then only a note or picture with `created` true, a regular file
  (`symlink_metadata`, never a link), with the recorded SHA-256 read through a 64 MiB cap, and
  re-read as it goes. A file that was there before the run is `before`: its line comes out, it
  stays. A note open in a tab with changes not saved is kept, with its line, and said
  (`recycle::SparingUnsaved`, D-158). A note or picture a run started later names, in a record
  not undone, is that run's (`later`): it stays with its line, the question says "2 files are a
  later run's and stay.", and the report lists them under "A later run's, kept" (D-192). (4) The group's file when `group.created` and only its
  heading and `home:` are left. (5) The folders the record lists, deepest first, while empty,
  never a link. (6) The record is marked `undone`, or `undo pending` when a link could not be
  taken out or the bin would not take a note (D-156): Undo can be pressed again, Finish is no
  longer offered, and `runs::finish` refuses it ("This run is being undone…"). The panel's title reads "The run is not undone · {why} Try again." and the held notes
  are listed; the run's row says "partly undone". `with_headings_removed`
  joins with the file's own line ending, as every writer in `mdshared_app::library` does.
  **Finish** writes the line the record holds (not the absolute path) through the same `Reach`.
  Undo's removals go to the Recycle Bin where the file's drive has one, and are a plain delete
  where it has none (77).
- **"AI runs…"** lists the records when it is opened (never at start), newest first, with "Show
  the notes", "Undo this run…" and "Finish" for a split or a Markdown run that stopped, is still
  writing, or has its links pending (`overlays::offers_finish`, D-157). When step 6 fails the
  toast says "The notes are written, but {file} could not be updated: {why}. AI runs can finish it
  or undo it." Finish under a section writes the heading the file lacks, and the record learns of
  it (`sections_added`), so undo takes the heading out with the line. The undo question of a
  rewrite names every file it puts back (D-162).
- **"Split into notes…"** is in the tab's menu, the palette, the AI menu and the Notes library
  submenu. It is disabled with "Save {name} first: the notes are made from the file on disk."
  until the document in front has a file and no unsaved changes. A tab that has not read its file
  again since another program saved it (still loading, changed on disk, another stamp, or, for a
  UTF-8 file, other bytes than the tab's text) is refused with "{name} changed on disk since this
  tab read it. Reload it first: the notes are made from the file on disk." (`app/plan.rs`, D-155).
- **The Notes pane's foot** draws "Plan ready: N notes · Show · Drop" while a plan is parked; the
  count and the parked flag are in the Files region's signature.
- Shots: `--dialog runs|run-undo|run-undo-recycled|run-undo-original|run-undo-later|run-undone-later|runs-newer`, and `--plan-demo split --plan-parked`.
- Tests: `mdshared_app::runs` (24 in the module, in the shared repo) and `crates/mdedit-core/tests/undo_rules.rs` (28: each
  of D-069 to D-072 forced on disk, and each one fails with its fix reverted; a finished run under
  a section undone with its heading), `app::plan_write_tests` and `app::undo_rules_tests` (5: the
  window's gate, the flat undo, the refusal when the original is gone, the list of unreadable
  records). The links-pending test stands the group's file there beforehand and blocks every
  temporary name `files::replace` would take, so the write of step 6 is the one thing that fails;
  it then checks Finish once the names are free. Links pending with a group file the run made
  is staged by a switch compiled into the tests only (`plan_write::REFUSE_LINKS`): the file
  stands with no link, and Undo takes it with the notes. `app::library_write_tests` has the owner's
  `.tmp` left alone, a BOM kept, a file not UTF-8 left as it is, and the template never written
  over a `notes.md`.
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
- **An add shows at once.** "Add this file to the notes library…" (`library_put`) writes the note
  and its line, opens what is folded above it down to the section (`unfold_to`), makes the rows
  again and redraws, so the note is in the tree without a fold and an unfold. A name being typed
  in the pane's box is discarded with the rows it was drawn over, and the keyboard goes back to
  the text, caret and all (`stop_edit`, `drop_library_field_focus`, `caret_reset`), as Escape
  does it.
- **The delete question fits its words.** "Delete this library file…" says, after the count, that
  the notes it lists are kept on disk, and says nothing when it lists none. Its card grows from
  the lines that sentence wraps to: `overlays::card` takes the body's height for
  `DialogKind::LibraryDelete`, measured by `overlays::body_height` with the same face, size and
  width it is drawn with (laid out at an empty clip, so nothing is painted), instead of a fixed
  196 that ran a long group or file name's fourth line through the divider. The floor stays 196,
  the two lines of the question that lists no notes; one that lists notes takes a third line and
  the card grows a few pixels. Shots `library-delete` and `library-delete-long`. "Take this
  section out of the library…" (`DialogKind::LibraryRemove`) names the section, whose name can be
  as long as a file's: its card grows the same way, from the same floor, and a card capped at the
  window's height cuts its body at the divider rather than draw it into the buttons (audit 4,
  F19; D-164). Shot `library-remove-long`.
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
  a row's menu left open closes when its row is another after the read and stays when it is not
  (`App::library_build`, through which every rebuild of the rows goes, the row of an open menu
  taken before a name box goes; audit 4 F16, D-181, D-197),
  closing the pane and switching the face drop a running read, the slim copy holds no notes, the
  omitted count repaints the pane, and an add shows the note at once under a folded group or a
  folded section, the keyboard back in the text when a name being typed is dropped).
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
  caret, as `toggle_visual` starts a fresh pane. A question the page asked while the pane held
  it back (`Visual::pending`) is taken with `Visual::take_pending`, which gives it only while the
  document it was asked of is the one in front: after a tab switch it is dropped, never asked of
  the new document (audit 4, F13; D-160). Tested through `preview_sync`
  (`a_question_held_for_the_page_is_not_asked_of_the_tab_switched_to`).
- **The placement waits for the text.** `Visual::place` is set on every hand-over and cleared by
  `place_page` once the pane has that document's text (`Pane::shown`, and not `held`), so a tab
  whose file is still being read is placed when its text arrives rather than showing the top.
  The page is asked for the caret's line and then told to forget it, so the reader's own
  scrolling is left alone from there on, as in the mode.
- **A tab still being read waits.** Its language is unknown until it is read: `visual_sync`
  neither follows nor ends while the active tab has no text and is loading, and a failed read
  ends the mode, as a document the page cannot show does. A block open on the document the page
  still shows closes when the wait starts, so the old tab's block is not left open over the new
  one's text when it arrives.
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
  `visual_editing_waits_for_a_note_still_loading`, `visual_editing_waits_for_a_file_it_will_not_show`,
  `visual_editing_ends_when_the_waiting_note_cannot_be_read`,
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
  open with unsaved changes, a group's library file that is read-only or not UTF-8 text (D-161),
  a source whose length or SHA-256 is not the one the plan was made
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
  written above. `library_put` also makes the pane's rows again with what is folded above the new
  note opened, and drops any name being typed there (§71).
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
  the reader takes for library files. The comparison is `library_write::same_folder`: both sides
  through `mdshared_app::paths::comparable`, which asks the disk about every part that exists and
  folds a `.` or `..` that follows a part that is not there, so `home: Trip/..` is the library
  folder whether or not `Trip` is there, on every system (on Linux and macOS `std::path::absolute`
  keeps such a `..`; audit 4 F14, D-165; the fold is shared's since `d1330b0`).
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
  The test of step 6 failing stands the group's file there and blocks every temporary name
  `files::replace` takes (section 70), since a taken group name no longer reaches the write and a
  `.tmp` file of the owner's is no longer in its way.


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
  open on a moved or carried file follow it (`Docs::follow_move`), and so do its place in the
  recent files, its caret place and its bookmarks, as a rename's do (audit 4, F11d; D-159); the
  library is read again, and
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
  connection is chosen; while the Preview button is hidden (a document with no preview) it takes
  the button's place right after the Notes button's separator, so it never stands apart across an
  empty slot (the owner's report, 2026-11-24; shots `bar-ai-no-preview`, `bar-new-save-ai`).
  `plan_width` still counts the slot, so what the bar shows at a width is the same for every tab.
  It is enabled while a document is ready, and opens the AI menu under itself
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

## 79. The AI review with a tick per change (2026-11-23, branch `f/review-hunks`)

Phase R2 of `docs/ai-library-plan.md` (5.5), asked for by the owner (D-077); the look is the
approved drawing on `mock/ai-review` (`rewrite-hunks`), not its code. Decisions: D-107, D-108.

**What it replaces.** `ai::review`, which flattened a diff into lines and forgot the hunks, and
`ReviewLine`, are gone. The review keeps the comparison and one tick per change.

**`mdedit_core::hunks::Hunks`** (headless, `crates/mdedit-core/src/hunks.rs`). The text before, the
text the model sent, `diff::diff` of the two (the comparison Compare uses) and a tick per
non-equal hunk, all ticked at first. `compose()` makes the text Apply puts in, in one pass: an
unchanged run gives the old lines, a change gives the new lines when ticked and the old ones when
not. All ticked is the model's result to the byte and none ticked the original to the byte (both
are returned as they are, because the comparison ignores line breaks, so a mixed-ending file's
unchanged lines would otherwise come out with the model's endings); a subset is composed from
whole lines, and a last line with no break never runs into the line after it (the break of the
text's kind is put between). Apply is `ai_replace` of that text: one `ReplaceRange`, one undo
step. `merge::Splice` is not used (N edits for one step). Copy result copies the same text.
Nothing in it can panic on a model's answer: every index comes from the comparison of the two
strings, and slices are taken through the buffer's line ranges.

**`hunks_ui.rs`** (the shell, no `App`, the shape of `plan_ui.rs`). `Layout::build(hunks, text
width, measure)` makes the list: per change a 30 px header (`HEAD_H`), its old lines then its new
ones (`LINE_H` 20 px per row, two 44 px number columns, text from x + 128), three lines of
context (`CONTEXT`), longer unchanged runs folded into "··· N unchanged lines". `wrap` cuts a
line into rows that fit the column (after a space when the row has one, inside a word when not,
between characters always), measuring windows of at most 512 characters, so a 200 KB line is never
shaped whole. A row's height is its rows times `LINE_H` and the layout keeps the running sums
(`tops`, `ends`, `total`), so the scroll arithmetic is exact. The layout is made again only when
the text width or the scale changes, not when a tick does. Tabs are drawn as four spaces. The
words that differ in a changed pair of lines come from `Diff::intra_line` (pairs of equal count
only, as in Compare) and are tinted. A change that is not ticked is drawn dimmed (its tints and its
text half way to the ground) and its header reads "not taken · the old text stays"; the current
change has a 3 px accent stripe and a tinted header, and "Space ticks or unticks". The scrollbar is
the shared overlay one: a press on its track brings the thumb there and takes it (`Review::grab`),
a drag on the thumb scrolls, both by `scroll::offset_for_thumb`.

**The window** (`ai_ui.rs`, `app/ai_events.rs`). Card 1040 x 736. Under the header (title, "file ·
scope · via", the Changes / Preview switch, the totals) a 46 px bar: previous and next change,
"Change 3 of 12 · lines 32–34 · +8 −3", "where" and the strip (a mark per change at its place in
the old text: accent when ticked, grey when not, the current one taller; each mark is a click),
Tick all, Untick all. Footer: "12 changes · 8 to apply · +212 −83 lines" and under it "Unticked
changes keep the old text. Apply is one step: Ctrl+Z brings the old text back."; Discard, Copy
result and "Apply 8 changes", which at zero ticked reads "Nothing is ticked", is drawn as a
disabled button and takes no click (Enter says so in a toast). Keys: F7 and Shift+F7 the next and
the previous change (scrolled to the top of the list), Space ticks the current one, Enter applies,
Esc closes and keeps the result (phase 0); Up, Down, PageUp, PageDown and the wheel scroll. In
Preview the arrows are drawn off and F7/Shift+F7 do nothing: the pages have no rows to scroll to
(D-108), so stepping would change the count and move nothing; the strip and Tick all still work. A
review that is blocked (text changed, read-only, closed) keeps its list and ticks, and its Copy
result copies what is ticked.

**Preview.** The BEFORE page is the old text, the AFTER page the composed text, headed "8 of 12
changes taken". A tick sets a due time 150 ms ahead (`compare::DEBOUNCE`, Compare's delay) and the
AFTER page is sent the new text then, once for any number of ticks in that time (`ai_review_due`,
called from `timers`); a text sent while the page is still drawing the last one is held and sent
when it answers. This is the cheap form of the plan. **The tinted, level preview of the mockup
(`rewrite-preview`) is not built: D-108.**

**Tests.** Core: every change ticked is the result to the byte, none the original, subsets, a
line with no break, CRLF, an insertion and a deletion, every subset of a spread of texts (no line
is made up). `hunks_ui`: wrapping (nothing lost, rows fit, between characters, windows, a 300
character line), context and folds and numbers (the first review's test, kept), a taller row moves
the sums. The app (`app/review_tests.rs`): Apply of a subset is one undo step and Ctrl+Z restores
the original; all ticked is the result to the byte; nothing ticked offers no Apply and Enter
changes nothing; F7, Shift+F7 and Space; the last change of a 400-line file by F7 and by dragging
the scrollbar, and a press on the track; a 300 character line is in rows that hold all of it; an
unticked change's tint is not the ticked one's, and comes back; Copy result copies the composed
text; the strip's marks and the arrows; the preview's page is made again once, 150 ms after the
last tick; a closed document's review keeps its list and ticks, offers no Apply and copies what is
ticked. In tests, Copy result and the closed review's copy are read from the engine's `COPIED`
slot, not the platform clipboard, so a contended or unavailable clipboard cannot fail them.

**Shots** (`tests/shots.sh`, both themes unless said): `review-hunks` (all ticked, the change with
the long line current, the line wrapped), `review-partial` (changes 2, 4, 5 and 7 unticked, dimmed),
`review-nothing` (none ticked: the disabled button), `review-long` (400 lines, twenty-eight
changes, scrolled to the eighteenth, the scrollbar showing; made under `target/data`),
`review-preview-partial` (the preview with some unticked), `review-closed` (the document closed:
the warning banner, Copy result only, and no Run again) and `review-min` (800x560, light: the
strip is left out where it does not fit).
`--ai-demo review-hunks|review-partial|review-nothing|review-long|review-preview-partial|review-closed`
rewrite the open document with the file `<name>-after.md` beside it
(`fixtures/review-demo-after.md`).

**Fixed with it.** "The AI review has no scrollbar and cuts long lines".

## 80. Dragging a note in the library (2026-11-23, phase DD)

**What it does, and what it does not.** A note row of the Notes pane can be dragged onto a group
row or a section row; its line leaves the library file it was in and is written under the heading
it was dropped on (D-130). That is the write the row menu's "Move to…" already makes
(`app/library_write.rs`, `library_move`): the source's line is looked at first, and a line that no
longer holds the note (the file changed and the pane has not read it again) writes neither file
(audit 4 C12a, D-182); then the destination file is written, the source line is taken out second,
both only when neither is open with unsaved changes, and each write reads the file again and
checks it reads back as the old notes plus the change. The note's file is never
copied, moved, renamed or deleted, and nothing outside the library files is written. No other row
starts a drag: a group is a library file whose order is its file name, and a section is a block of
lines that would be a cross-file rewrite; the design of record for drag and drop says "notes
only", and what those two should do is the owner's to say. Files dropped from File Explorer are
not taken by the pane either: winit's Windows drop handler is given the drop point and throws it
away, so the window knows a file arrived but not which row it fell on (D-130).

**Where the state is.** `Drag::LibraryNote { row, x, y, moving }` in `ui.rs` is the press: the
row, the pointer where it was pressed, and whether it has become a move (after 4 px in either
direction). The pane holds the note pressed by what it is, not by its index
(`LibraryPane::hold`, a `RowId`; `held_at` is its row as the rows are now): `build()` finds it and
the marked heading again whenever the rows are made again, so the release acts on the note that
was pressed, and on nothing when it is no longer in the rows (audit 4, C2). `LibraryPane::drop`
is the row the note would go into, kept so the drawing and the release agree. A press on a note
is no longer a button (`ui.pressed`); `left_release` takes it (`library_note_release`): without a
move it activates the held note's row exactly as a click did, with one it drops. While a note is
held and moving, `library_drag_key` takes every key before any other handling: `Esc` clears the
drag, the hold and the timer, so nothing is written, and every other key is kept from the pane
and the text, so nothing folds, removes or renames a row mid-drag.

**Where a drop goes.** `LibraryPane::drop_row(x, y, area)` turns the pointer into a row (the
list's own geometry: `list(r)`, `ROW`); a point beside the pane at a row's height is on no row, so
a note let go over the editor moves nothing (audit 4, C1). A note row stands for the heading it is
written under, so dropping on a note goes into its section. The row must be a group or a section, and the
place must differ from the note's own (same library file, same heading path, headings compared
without regard to ASCII case as the library compares them); otherwise there is no mark and a drop
there writes nothing. `row_place` and `note_place` answer with the library file and heading path.

**The move.** `app/library_drag.rs`: `library_drag_release` reads the row under the pointer again
at the button's release, takes the held note's file, line, title and path from its row as the
rows are now, and calls `library_move`. On success the target heading is unfolded, the rows are made again and the
cursor follows the note to its new row (the note may have left the screen). A refused write says
why in the toast it always did.

**Holding at the edge.** A pointer over the pane within 12 px of the list's top or bottom arms
`autoscroll_due` (the selection drag's timer, `AUTOSCROLL_TICK`); each tick `library_drag_scroll`
moves the list one row toward the pointer and looks up the target again, and re-arms while the
pointer stays there. A pointer over the editor at that height scrolls nothing. The release and
`Esc` clear the timer. While a note is held and moving, `library_tick` does not take a finished
read or start one; anything else that makes the rows again is followed by the note's identity.

**Drawing.** The target row is drawn as the pane's keyboard row is: a filled rounded box in the
accent colour with a 1 px inner box of the selection tint, before the row's icons and text. The
note held is ringed in the plain line colour and its text is drawn in the faint colour, so it reads
as set aside. The note's title follows the pointer as a chip (a `frame` in the popup colour with an
accent stroke, a document icon and the title), clamped inside the pane so it never leaves it. The
Files region's signature carries the drag (row, moving, target, the pointer and the press point):
the chip moves with the pointer while no row changes, so without them the pane would not repaint
during a drag (the reason `region_sig` exists).

**Tests** (`app/library_drag_tests.rs`): a note dropped on a section is written under it, once,
with the cursor on its new row; a note dropped on a note goes under that note's heading; a note
dragged across groups is listed in the other file only; a note dropped on its own heading writes
nothing and says nothing; a drag let go outside the pane moves nothing and opens nothing; `Esc`
abandons it; a press and release is still a click; an unsaved library file refuses the drop with
its word; a hold at the edge scrolls; and the drawing changes in the target row, the row held and
the chip and nowhere else. Audit 4 added: a note let go over the editor at a row's height is not
moved and the edge scroll is not armed there; a key while a note is held reaches nothing and the
note moved is the one held; rows made again under a held note still move that note; and a click
opens the note pressed after the rows were made again. Each was proved by reverting its fix and
watching it fail.

**Shots:** `library-drag` and `library-drag-dark` (`--library-demo drag`): a made-up library with
"Asakusa and the river" held over "Food", the chip at the pointer.

**Not built:** dragging a group or a section; a keyboard equivalent beyond "Move to…" (which is
the keyboard path); files from File Explorer (see above); a drop between two windows.

## 81. Add a folder to the library (2026-11-23, phase 4a)

`ai-library-plan.md` 5.7, taken without the AI: the folder the owner picks becomes a plan of
links, and no file of theirs is copied, moved or renamed. The command is "Add a &folder." in the
Library menu and "Add a f&older…" in the Notes pane's whole menu, whose "f" is Refresh's (`o` is
free there; the two menus word it differently because their letters differ, as everywhere in the
app). Three pieces, in the shape the plan and the move already have:

- **The walk** (`crates/mdedit-core/src/import.rs`): `walk` reads a folder with the Files pane's
  own rules -- `mdshared_app::notes::skips_entry` for a dot name, the hidden attribute, `target`
  and `node_modules`, a link to a folder not followed, `EXTENSIONS` (`.md`, `.markdown`, `.txt`)
  only -- and stops at `library::MAX_ENTRIES` with `more` set. The folder's own files come first,
  then each subfolder's, by name, case-insensitively. Each file keeps its subfolders (at most
  `MAX_SECTIONS` = 3; a fourth is folded into the third), its title (the first `#` heading outside
  a fence, else its name) and its first `PREVIEW_LINES` lines. Nothing there can panic on what a
  folder holds: names are read lossily and a heading is looked for at a character boundary.
  Audit 4 bounded and narrowed it. It stops after `MAX_READ` (50,000) entries read or at a folder
  `MAX_DEPTH` (64) deep, with `stopped` set, and the plan of such a walk is refused with that
  reason rather than written in part (C8, C12). A title is one line, a bare CR ending a line as LF
  does (an old Mac file), with control characters turned to spaces and at most `TITLE_MAX` (200)
  characters (C7). The window's walk is `walk_for(folder, library)`: the library folder, when the
  folder picked holds it, is left out (`library_left_out`, C5), and a file whose line a library
  file would not read back as that file -- `%20` in its name, which the shared reader takes for a
  space -- is left out and kept in `unlinkable` for the card to name, so one such name no longer
  refuses the whole folder at Add (C6).
- **The plan** (`import::plan`): files the group's library file already lists come unticked and
  marked, and `tick_all` leaves them so; a section row's box is ticked when everything under it
  is, unticked when nothing is, and drawn with a dash between the two (a section row's click
  ticks all of it when any is unticked, nothing otherwise). The plan refuses itself before a
  single tick is counted -- a folder with no name, more than 2,000 files, nothing to add -- and
  `problem()` adds the two it knows as they change: nothing ticked, and a group that would pass
  2,000 with both numbers. A group or subfolder whose name ends in `#` (or a heading that reads
  back as nothing) is refused: the library's reader strips a closing run of `#`, so `C#` would
  come back as `C`. A file the group lists by a path with `.` or `..` in it is the same file
  (`tree::same_note`), and sections that differ only in ASCII case are one row, as the section's
  box and the library's writer already took them (audit 4, C12).
- **The write** (`bins/mdedit/src/app/importing.rs`): the folder becomes the group -- its name,
  and `home:` naming the folder, relative to the library when it is inside it so the library can
  move as a whole -- and the links go into one library file, `tree::link_path` relative inside the
  library and whole outside it. A library that already has a group of that name is joined: its
  file, its `home:`, its notes -- when its `home:` is the folder picked or a folder that holds it,
  or it names none. A group of that name that keeps its notes in another folder refuses the
  plan, naming the group and that folder, and the card's group line names the group's own folder,
  not the one picked; the `home:` is read again at Add (audit 4, C4; the owner may choose to join
  such a group instead). The dry run comes first, on the text as it reads now. A new group's file
  is then made whole -- heading, `home:` and links -- by one exclusive `files::create_new`, so no
  moment holds a group with no notes and a file that took the name meanwhile is left alone (C9);
  links into a group the library has go in through `library_attempt_at`, which re-reads, checks
  and writes atomically. The card closes on success. Refused with the card left open and the
  reason in the footer, the button still on: a group file open with unsaved changes (it would be
  thrown away), a group whose folder is elsewhere, a line that would not read back. The notes
  pane turns to the Notes face on the group, and the toast counts the lines.
- **The read on its thread.** "Add a folder to the library…" while a folder is being read stops
  that read ("Stopped: … is not added to the library."), and the toast that starts a read says so.
  A read that ends while Settings, another card, the AI panel, the task window or a dialog is open
  waits for it to close; the event loop asks again after every event (`import_take` in
  `about_to_wait`), so it costs nothing while nothing is being read (audit 4, C8).

The card (`import_ui.rs`, `import_draw.rs`) is the plan's and the move's: a 1,040 x 700 card
shrunk to fit, the folder and its count in the header, the rows at 46 px with a tick box, a
heading one line and a file two (name, then its folder and file name), the selected row previewed
on the right in mono, its path and "the file is not moved or changed" at the foot. A reason the
button is disabled takes a whole line above the counts, so neither is elided. Under 900 px wide
the preview is dropped and the list keeps the card; the card scrolls its rows with the shared
`Rows`. Its own `region_sig` carries the ticks, the selection, the refusal and the scroll.

Shots: `import-folder`, `import-folder-sections` (three headings with a dash), `import-folder-taken`
(the group already in the library, its file marked), `import-folder-small` (800 x 600: no
preview), `import-folder-over` (a folder over 2,000: the button greyed and the reason on its own
line), `import-folder-skipped` (two `%20` files and the library's own folder left out, said on the
header's last line), `import-folder-elsewhere` (the group's folder is another: named in the header,
refused in the footer), `import-folder-stopped` (the walk's bound), `import-folder-refused` (an Add
refused at the write, its reason in the footer and the button on), each in both themes and drawn
by `--plan-demo <name>`.

Not done in 4a, deliberate: no AI in the card (5.7's second half), no drag and drop, no
conversion of a file to Markdown, and no removal of a link -- the pane's own remove is 4b's.

## 82. A Markdown note from a text file (2026-11-23, branch `f/p5`)

`docs/ai-library-plan.md` 5.8. "Markdown note from a file…" on a group's row menu: a text file
becomes a new note in that group's folder, made by the AI; the original is left where it is and is
never renamed or changed.

- **The row, then the picker.** `LibAct::MarkdownNote` beside "Move the notes…" (`overlays.rs`,
  drawn with `glyph::HEADING`). It opens the system file picker only: `Picked::Note { group }` in
  `app.rs` carries the group's library file, so `note_file_picked(group, Some(source))` is the one
  door into the card, and `Some(None)` (the picker cancelled) does nothing.
- **What is read** (`app/note.rs::read_source`). Through the editor's own loader, capped at
  `NOTE_MAX` 200 KB (`LoadOptions.max_bytes`), regular files only. Refused with a name and a reason:
  not a file, empty, over the cap ("200 KB of 200 KB"), lossy or NUL bytes ("not a text file: some
  of its bytes are not text this editor can read"), nothing but whitespace ("has no text"). A note
  is Markdown, so its text is written as UTF-8 whatever the source was (BOM, UTF-16 or a legacy
  code page) and `Source::encoding` says so under the title; a source in UTF-8 is written as it
  reads. It keeps the name, the line count, the byte length, the digest (SHA-256) and the source's
  own line split, which `ai::note_body` uses for the note. The file is read once, capped at
  `NOTE_MAX` + 1, and the text, the count, the length and the digest are all of those bytes, so a
  file changed between two reads cannot give a card whose check passes against other text; a bare
  CR line split (an old Mac file) gives a note with LF, and counts as a line break (audit 4, C12).
- **The card** (`note_ui.rs`, `note_draw.rs`, over `Dialog::draw` like the move card). `Phase`:
  Ready (what is sent, what it is asked to do, what will be written, in the footer's words), Asking
  ("Asking Claude Code · Sonnet 5.5: N characters so far", a pulse, a Bar and a footer link, Esc or
  Stop cancels through `Ai::cancel_id`), Changes (`mdedit_core::hunks`' own changes: the source's
  lines against the note's, unchanged runs folded to 3 lines, red and green, `- N lines + M lines`
  from `Hunks::totals`), and Result (the note as
  it will be written, numbered). A `Changes | Result` segmented switch in the header, 28 tall, on
  the title's line, painted only in Changes and Result. Two lines of body above the list ("What
  changes to make the note." / "The note as it will be written."). Enter sends in Ready and writes
  in Changes; the footer names the file and says the source is not changed; `blocked` puts the
  reason in the footer's left, and Make the note/the primary is drawn disabled then.
- **Writing** (`App::note_write`). Every check again, because the answer may be minutes old: the
  library folder, `note_blocked`, and the source re-read and compared by length and SHA-256 ("has
  changed since the note was made. Make the note again"). Then `runs::write_note` — the same write
  of 4.3 with `Kind::Markdown`, in `.runs`, undo and all — `library_rows()`, the note opened in a
  tab, and `open_notes_of` on the report: the Notes face, the group's notes open, the cursor on the
  new line.
- **The refusals before the request and in `note_blocked`**: AI off ("Choose a connection in
  Settings › AI first."); no library folder; the group's folder is the library folder itself
  ("so the note would be a library file"); the note would land beside the library's own files; the
  group's folder is a file; the group's library file open with unsaved changes (`library_file_free`
  in the app's words); the file cannot be read; the group would pass 2,000 notes; the line would
  not read back as the old notes plus this one (`runs::check_links_added`, a dry run). Only "the
  source has changed" disables the button for good (`note_write_refused`); every other refusal at
  the write, and a write that started and made no note (its folder could not be made, the disk is
  full), comes back as `note_write_failed`: the card stays with the AI's answer, the reason in its
  footer and the button on, so a second press can pass. A run that made no note leaves nothing:
  `discard_run` takes away its record, the record's `.done` log and the folders it made, deepest
  first and only while empty, since shared `runs::write_of` still marks such a run Done
  (audit 4, C3). A name taken by a file while the answer was out gives the note the next free
  name.
- **Naming.** `library_write::free_name(home, stem)`: the source's file name with its extension
  dropped, lower case, runs of anything but letters and digits turned into one hyphen, cut to 60
  characters; `stem.md`, else `stem 2.md`, `stem 3.md` …; `pasta notes.txt` → `pasta-notes.md`.
- **The request.** One, through `Ai::ask(Ask::Note)`: the whole file and `ai::FORMAT` as the
  instruction ("Format this text as Markdown: headings for its sections, lists, tables for tabular
  data, task lists for things to do, emphasis and code where the text calls for them. Do not change
  the wording."), `Sent` with the connection, model, bytes and requests so the record holds it. An
  answer that is whitespace, or the same text as the source, is "The AI changed nothing, so there
  is no note to write." A cancellation is not news. `Ai::running()` counts it, so a rewrite cannot
  start over it.
- **In "AI runs…"** (`overlays.rs`) a record of this kind reads "Markdown · source → group · N
  notes"; undo, Show the notes and the Recycle Bin question are split's own, unchanged. A record
  of a kind this build does not write — an unknown kind word, a later version's (D-145, after
  D-134) — is not listed with the runs it could undo: `list_all` puts it in the newer-version
  list (the same fallback an unreadable record has, "Written by a newer version of mdedit", with
  nothing enabled), and nothing in it is shown, finished or undone. `Kind::Rewrite` is no longer
  such a kind: phase W writes it and DESIGN 83 gives it its own row, Show and undo.
- **The demonstration** `--note-demo ready|asking|changes|result|blocked` (`note_ui::demo`): a file
  and an answer made in the program, `card.demo` so Enter says "A demonstration of the card: it
  writes nothing." and nothing is sent or written.
- Shots: `note-ready`, `note-asking`, `note-changes`, `note-result`, `note-blocked`,
  `note-failed` (a write that made no note: the answer kept, the reason in the footer, the
  button on), both themes, `note-changes-small` at 800x600.
- Budgets: `docs/measurements/2026-11-23-note-card.md` — 7 interleaved runs against `main` at
  `6e6579f`, nothing beyond the runs' ranges.
- Tests: `app::note_tests` (13): the menu item and the whole path to the note and its record, undo,
  undo refused when the source has changed, a taken name, AI off, a file too long, a file that is
  not text, the group file unsaved, the group folder the library's own, Esc in Asking, the switch
  repainting the body and Cancel writing nothing, the source changed while the answer was out, and
  every demonstration. Audit 4 added five: a name taken while the answer was out, a write that
  makes no note (the card and the answer kept, no record left, a second press writing it), the
  record and folders of such a run taken away, a bare-CR source, and the card's text and digest
  of one read; and undo is now checked byte for byte. `mdshared_app::runs` has the kind's own two (write, undo); and
  `plan_write_tests::a_record_of_a_kind_this_build_does_not_write_is_listed_as_a_newer_versions`
  proves a record of an unknown kind word is listed as a later version's and nothing in it is
  shown or undone (D-145).

## 83. The AI task window over files (2026-11-23, branch `f/pw`)

Phase W of `docs/ai-library-plan.md` (5.6), on top of R2: the owner asked for "AI task on files"
that can file, split or **rewrite files in place** (D-074, D-076). The drawing is the mockup
`ai-files`; the window takes files, not the open document, and opens none of them in a tab.

**The window** (`task_ui.rs` state and numbers, `task_draw.rs` drawing, `app/task.rs` the three
tasks). Card 880 x 704: the header and "Add…" (phase E), the WHAT TO DO switch (`File them |
Split them | Rewrite them`), the 36 px instruction field, the saved prompts of the chosen kind
(96 px, `kind: file` new in `mdedit_core::ai`), WHERE IT GOES, the list under its three group
headings, and a footer with "Show what is sent" and the primary button. A review of a file
(R2's list) is drawn over it as the front panel; keys, pointer, wheel and right click reach the
review first (`app/input.rs`). The window's own state raises one tick, in the overlay signature
and in the full-window bounds, so every change paints.

**The WHERE IT GOES row** (`task_ui::dest_row`, 2026-11-25, the owner's report of the folder
drawn over the section field). The label's 100 px, the group's button (240 px) and, for File
them, the section field (180 px), each narrowed in proportion when the card has less room; then
the folder the files go to (`→ …\library\Recipes\Pasta\`, the last three parts) right-aligned
in what the row leaves, cut from its left (`library_pane::elide_left`) so its end stays readable,
the arrow kept. Under 72 px of room it is left out; the button still names the group. A test
draws the window at every width from 520 to 1600 px with a long folder and with a one-letter one
and finds the button and the field the same, pixel for pixel, and inside the card. Shots
`task-dest-long` (1280), `task-dest-long-760`, `task-dest-long-520` (the folder left out),
`--task-demo files-long-dest`.

**Three kinds of file**, decided once when a file is added (`App::task_probe`), and what each
does for each task: **Text** (the editor's loader, no NUL, not lossy) is read and sent; **Picture**
(the image paste's extensions) is never read — its name only is sent, and it is linked; **Other**
(PDF, Word, archives, bytes that are not text) is never read. "File them" links an Other file by
name, as the note pane can link any file; "Split them" and "Rewrite them" leave it out with its
reason on the row. A file already listed by a library file reads "already in the library",
unticked, compared with `same_note`. A Rewrite run takes at most 20 files and 200 KB each; the
reason says which and how much. What is left out is never counted, never sent and never written.

**File them.** With a connection, the ticked files go in batches of 40, one request after the
other, each a digest: name, size, date, first heading, first 1,200 bytes, 2 KB at most. The
request has the instruction and the batch numbered 0..n; **the model never sees a path** — the
answer's `id`s are the window's places. The JSON contract lives in mdedit (`task_ui::FILING_SYSTEM`,
`filing_prompt`), never in the engine. The answer is read with the envelope rules of 5.2: an id
not sent is ignored, the first answer for an id wins, a `with` must name a text file of the same
batch, titles are one clean line of 120 characters, sections are at most three folder names. A
file the answer leaves out is under "NOT PLACED", unticked. With the AI off the plan is composed
here instead: every file under the section typed beside the destination. Written then, in order,
one read-back per link: a name derived by the library's own rule, under the sections, a relative
link from the group's file (`link_path`), the group's file made first with its `home:` when new.
An existing group's file is written as every library writer writes it (D-195): refused while a
tab holds it with changes not saved (`library_file_free`; the window stays, the toast says why),
read by `library_text` (a byte order mark kept, a file not UTF-8 refused and left as it is) and
written by `library_put_text`.
The plan face shows each file's title and `→` section, and "Add N to the library".

**Split them.** Each file's plan card in turn (5.1–5.3, unchanged), opened from the file rather
than a tab, its Recycle Bin box refused by `RECYCLE_ONE_DOCUMENT`, its AI offer the window's
instruction when one is typed; Writing that plan closes the file's place and opens the next, in
order. Escape closes the plan back to the window and leaves the file alone (its plan is not
parked, because it is a step of the run).

**Rewrite them.** One file at a time: the prompt is `core_ai::prompt` of the text and the
instruction; the answer is fitted (`fit_end`), written in the file's own line endings
(`with_eol`), and shown in R2's review over the whole file, titled "Rewrite <name>", through the
same panel and the same keys. Apply is `Hunks::compose` of the ticked changes; **the run's record
is started and each file's original is written into it before the file is** (`runs::start_rewrite`,
`runs::rewrite_into`, D-076), so a crash between leaves the original where undo finds it. The
file is written only while it still holds the text the answer was made from, to the byte and the
byte-order mark; a file changed since is refused with the review staying open and its list and
ticks kept. Undo is in "AI runs…" with the other runs: every original goes back while the file
still holds what the run wrote; a file changed since is kept and listed (`undo`, `UndoPlan`).
Discard leaves a file as it was and the run goes on with the next; Copy result copies and goes on;
Escape keeps the review (R2), and the run waits. The run's record says `done` when the last file
is applied or the window is closed; an empty record (nothing applied) is removed.

**The AI is off.** The window still opens. File them "Make the plan" and writes without a
request; Split them cuts at headings; Rewrite them is disabled, the footer says "The AI connection
is off", and "Open AI settings" is beside Cancel. Nothing starts.

**Show what is sent** (`task_request_text`) is the literal text of every request the window would
send: the system text and prompt per batch, per file, or per split outline; with the AI off it
says nothing is sent. It opens as a read-only copy tab and parks the window, which "AI task on
your files…" brings back. A request in flight stops when the window closes (Escape stops it
first, then a second Escape closes), and an engine error is the footer's sentence with Run
still usable.

**Tests** (`task_ui::tests`, `app/task_tests.rs`; the fake engine answers `TASKFILE` with a
filing JSON). A window opens empty; a picture is a picture, never read; a digest holds the
heading and no path; the model's answer places titles, sections and `with`, ignores an id not
sent and a `with` that names a picture; an unplaced file is unticked and not written; a 41st file
waits for a second request; unticked files are not sent; a new group is made in the library and
chosen; no group means nothing is written; with the AI off the plan is composed and links written,
a PDF among them linked by name; a rewrite is reviewed then written in place, the record holding
the original, and `runs::undo` puts it back; a file changed since the answer is not written, says
why, and leaves no record; the second file waits for the first review; Split opens a plan on the
file and no tab; Show what is sent is a read-only tab and parks the window; Rewrite with the AI
off opens Settings; Escape stops the request and a second closes. Each was reverted and watched
fail (the record test in `mdshared-app`, the rest here).

**Shots** (`tests/shots.sh`, both themes unless said): `task-files` (File them, a text, a picture
and a PDF with its reason), `task-files-plan` (the plan face, `PLACED` and `NOT PLACED`),
`task-files-off` (AI off: "no request", Make the plan), `task-other` (Rewrite them: the picture
and PDF left out, in place), `task-review` (R2's review over `pasta.md`, the note that Apply
writes in place), and `task-sent` (light: the read-only request tab). `--task-demo
files|files-plan|files-off|files-other|review|sent` builds them from `fixtures/task-demo/`, with
no engine and nothing sent.

**Budgets.** Nothing here is on the launch path: `task_ui`, `task_draw` and `app/task.rs` are
reached from the command and the drop alone, the window reads files only then, and it adds no
crate (`docs/measurements/2026-11-23-task-window.md`: 7 interleaved runs against R2, no figure
outside the runs' ranges; the executable +102,400 bytes).

**One command, and the Add menu (phase E, 2026-11-25, D-207).** There is one command, "AI
task…" (`C::AiTaskFiles`; "AI tas&k…" in the AI and Library menus, listed in the palette). Its
**Add…** button opens a menu under it (`draw_add_menu`, 320 px): each open tab that has a file, by
name, the one in front first and labelled "· in front", those already in the window left out;
then "Files…", the picker. Nothing is added by opening the window. The menu closes on a choice,
on Escape (which leaves the window open), on any other key and on a click elsewhere; while a
request is out it takes no click (`Hit::AddItem` joins audit 4's B5 list). "AI task on this
file…" (`C::AiTaskThisFile`) is now only in a **tab's** menu ("AI tas&k on this file…"), where
the pointer names the file: `tab_command` opens the window with that tab's file, which need not
be the one in front, or says to save it first. The palette no longer lists it
(`palette::listed`). Shot `task-add` (both themes) is the open menu over made-up tabs.

## 84. Sort a folder by AI (2026-11-23, phase 4)

`ai-library-plan.md` 5.7's second half, on 4a's card. The AI proposes the **headings** and the
**file under each**; the card stays the one place a plan is read, edited and written, and the
import's writer from 4a is the only thing that writes (D-150). It needs an AI connection: with the
connection off the card says "The AI is off: choose a connection in Settings › AI." where the
offer would be, and nothing is asked (D-079).

**The core** (`crates/mdedit-core/src/import_ai.rs`):

- `scheme_ask(&plan)` numbers the plan's notes (not the files the group already lists) and builds
  one request: per file its path inside the folder, its title and the first `START_SENT` (120)
  bytes; `SYSTEM` asks for at most `MAX_SECTIONS` (8) sections, each with at most `MAX_SUBS` (4)
  subsections. A request over `PROMPT_MAX` (200 KB) is refused with the size, not cut.
- `batches(&plan, &scheme)` makes `BATCH` (40) files per request, preceded by the scheme's
  numbered heading paths; `FILE_SYSTEM` asks for a section number, a confidence and a one-line
  `why` per id. At most `MAX_AI_FILES` (500) files are in one sort.
- `read_scheme` / `read(&str, ids)` read an answer with a depth-bounded JSON reader, every field
  optional and every number checked; no input can panic (rule 11). A section or subsection past
  the caps, empty or repeated is dropped and counted. A name is cut at `NAME_MAX` (120) bytes, a
  `why` at `WHY_MAX`; a title is taken only if it reads back as one line.
- `settle(&mut plan, &scheme, &batches, &answers, complete)` puts each note under the scheme's
  path: `same_path_names` compares without case, so `Pasta` and `pasta` are one heading. An id the
  batch did not send, a number the scheme does not have, a file the answer leaves out, and a
  batch that never came all set `unsorted`, `ticked = false` and the reason; confidence 0.6 and
  over is "sure" (`SURE`). `reorder` gives the rows the scheme's order, then already-listed files,
  then **Unsorted**, by title within a heading. An answer for a run that is no longer in flight is
  dropped.

**The card** (`import_ui.rs`, `import_draw.rs`, `app/import_ai.rs`, `app/import_ai_tests.rs`):

- `SortStage`: `Off`, `Asking`, `Scheme(Scheme)`, `Filing { scheme, done, total, in_flight }`,
  `Filed { placed, unsorted, requests, stopped, why }` and `Failed(String)`. The folder plan is
  `Off`; **Ask the AI** goes to `Asking`; the scheme comes to `Scheme`; **Sort N files** goes to
  `Filing`; the settled plan to `Filed`. The card keeps its ticks, selection and scroll across the
  stages: the scheme list is the folder list with the AI's headings, the same `Rows`, the same
  preview and "the file is not moved or changed" at its foot.
- `Scheme` is edited in place: **Add section** makes a row under the selection (or at the end)
  and opens its box, **Rename** (F2, or a second click) opens the box, Enter keeps it, Escape puts
  the old name back, **Delete** takes the row and a top section's subsections. The name goes
  through `TextField`, with `plan_preedit` for an input method; the caret and the box's text are
  in the region signature. Enter on the list is **Sort N files**; Escape is **Keep the folders**.
  A request cannot start with no sections or two sections named alike; the footer says which.
- While `Asking` or `Filing`, the list shows the plan (the folder's rows, then the scheme's), the
  head says "Asking the AI…" or "SORTING THE FILES…", the footer counts requests answered of
  total and in flight, and **Stop** cancels every request (Escape does the same) and gives back
  the scheme as it was edited. The second footer line states what is sent ("only each file's
  path, title, start and size") with the connection, model and effort, or, with no connection,
  the reason and the way to Settings. The preview's last line (`scheme_help`) says "Rename, add or
  delete before the files are sent" only in `Scheme`; while `Filing` it says the files are being
  sent and Stop brings the scheme back to change (audit 4).
- `Filed` shows each row's confidence and `why` dim under its name, a section row's "N files · M
  ticked", and the unsorted note with no tick. The head offers **Tick all**, **Untick all** and
  **Tick only the sure ones**; the footer says "The AI placed 9 files in 4 sections in 2 requests
  · 1 unsorted" and **Add N files**. A stopped run says how many it did not reach instead of
  "unsorted", and **Ask again**; a failed scheme or an unreadable first answer shows **Try again**
  and **Keep the folders**. Add is 4a's: one `library::rewrite`, one line per ticked file, the
  toast and the pane afterwards.
- **Requests at once** (D-080) is a number row on the settings' AI rewrite page, 1..8 with
  steppers, stored per connection. The run reads it when it starts, and the card's requests go out
  up to it at a time (three by default, one for a local model). `app/ai_events.rs` routes
  `Ask::SortScheme` and `Ask::SortFiles(batch)` to the card; `ai.rs`'s `SortRun` holds the scheme,
  batches, answers, in-flight ids and counters, and `Ai::running` includes it, so Escape and the
  engine ending reach it as they do a split.

**Shots**: `import-folder-scheme`, `import-folder-scheme-rename`, `import-folder-ai`,
`import-folder-ai-sorting`, `import-folder-ai-asking`, `import-folder-ai-off`,
`import-folder-ai-failed` and `import-folder-unsorted`, each in both themes, drawn by
`--plan-demo <name>`.

**Not done, deliberate**: the AI does not change a file, move one or choose its path; there is no
re-ask of one batch and no manual "Unsorted" heading in the scheme (Unsorted is added when it is
needed); and the request's answer is not kept in a run record -- a sort leaves no record, unlike a
split, because nothing is written until Add.

## 85. Pictures and links in the library (2026-11-25, phase F)

`docs/ai-pictures-plan.md` phase F, after the owner's report that a picture filed by "File them"
opened as garbage. **A library row opens by its kind** (`app/library.rs`, `note_opener`): a text
note opens in a tab; a picture, a PDF, an office document or a media file (the `OUTSIDE` list) is
handed to the system's own viewer (`system::open`); a program (`PROGRAMS`) is never run: a
script, which is text, opens in a tab like any text (Fable's review of the milestone: it is text,
and a tab runs nothing), and a program's bytes, or one that cannot be read, are only shown in
its folder (`system::reveal`); anything else is opened in a tab only when its first 8 KB hold no
NUL byte, and otherwise shown in its folder. "Show the notes" in AI runs opens a run's first
file the same way (`open_note`), so a filed PDF goes to its program, not into a tab. The palette's note targets and
"Open another…" go the same way (`open_note`). Tests record what was handed to the system
(`handed_to_system`) instead of starting it.

**File them never files a picture** (D-204): a picture in the task window is unticked with the
reason "not filed: a picture is not a note of its own", under the header "PICTURES · NOT FILED".
Pictures are the work of V1 to V6 (Write notes).

**A run of File them leaves a record** (`runs::Kind::Links`, shared `50e3401`): the group file's
text before the run, the lines added, and the group's own creation when the run made it
(`links_record`). Undo in "AI runs" takes the lines out to the byte and, when the run made the
group and it is unchanged since, the group file; it never touches the linked files, and a Links
record that says it created a note, a folder or a picture is refused as not its own. A 4a import
leaves the same record (`import_record`). The "AI runs" row reads "Linked · N files → Group".

## 86. The AI task from a row of the Notes pane (2026-11-25, phase L)

`docs/ai-pictures-plan.md` 4.2, after the owner's "clicking and using on the library should be
possible too". **Every row's menu heads with the AI task** (`overlays::library_row_menu_with_ai`,
its access key K): "AI task on this note…" on a note, "AI task on these N notes…" on a section or
a group, "…its one note…" for one, greyed with "No note under it" for none ("Its file is gone"
on a missing note). The number is counted when the menu opens. **A section or a group means
every note under it** (`library_pane::notes_under`): the rows of the library made again with
nothing folded, every note row deeper than it until the next row at its depth, each file once,
missing ones left out; so a folded section takes what it holds.

The line opens the window (`task_from_library`) on those notes, **on Rewrite them**, and the
header says where they came from instead of its usual first sentence: "The 4 notes of Recipes ›
Pasta, from the Notes pane." (`LibraryPane::came_from`; a note: "Taxes, a note of Personal, from
the Notes pane."). The sentence is dropped once another file is added. Rows can be unticked there;
there is no selecting in the pane (multi-select, MS, is later, D-208).

**"AI task…" takes the keyboard's row** (`library_keyboard_row`): while the pane has the
keyboard, the command works on the row the cursor is on, as its menu line would; otherwise it
opens the window empty on File them. Nothing in the shared tree changed.

Shots: `library-ai-menu` (a section's menu, the line at its head) and `task-from` (the window
opened from a section), both themes. Tests: `app/library_ai_tests.rs`.

**After Fable's review of the milestone.** The notes the line opens the window on take part in
Rewrite them and Split them: `already` (the library lists the file) keeps a file out of File
them only. A press on AI edit or the main menu (`Hit::Bar(AiEdit | More)`) no longer takes the
keyboard from the pane, so "AI task…" chosen there with the mouse takes the pane's row; with
the keyboard on a row with no note under it, "AI task…" opens the window empty. A window parked
by "Show what is sent" comes back as it was when the line is used, and says so; the notes are
not put into it. "AI task on this file…" on a note the library lists starts on Rewrite them.

## 87. After a run: what opens, and the task window's settings (2026-11-25, phase O)

`docs/ai-pictures-plan.md` 4.4, D-211, D-203, D-214, after the owner's "no preview shows up after
AI". **The cause**, found in the code: after three of the four tasks nothing opened a tab, so
there was nothing for the preview to follow, and nothing ever asked for one; and a note just
opened is still being read, so `is_markdown()` (which needs the text) said no. **What each run
does now:**

- **A Markdown note from a file** opens the note (`open_after_run(path, true)`): with "First note
  and its preview" (the default) the preview is shown the way the Preview button shows it
  (`show_preview`: the pane, mdview or side by side by Settings › Editor; never closes one; a tab
  still loading a `.md` or `.markdown` file counts as previewable by its name).
- **A split** opens its first note (`open_after_run(path, false)`) as well as putting the Notes
  pane on it; the preview follows the tab if it was showing and is not turned on.
- **Rewrite them**: after Apply, `check_changed` re-reads a tab that holds the file, so it and a
  preview following it show the new text at once; a tab with unsaved changes is told instead.
- **File them** opens nothing (the files were not changed): the Notes pane shows, unfolded to the
  group's section, its cursor on the first line the run added (`library_show_link`).
- "Nothing" opens no tab after a split or a note; the pane and the report still say where.

In tests `show_preview` records the command it would run (`preview::preview_asked`) instead of
starting the pane's renderer or mdview.

**Settings › AI tasks** (`AI_TASKS`, rows 106 to 109): "After a run, open" (`AfterRun`:
`preview`, `open`, `nothing` in `settings.ini` as `ai_after_run`), "Files per filing request"
(`ai_task_batch`), "Files in one rewrite run" (`ai_rewrite_files`), "Largest rewrite (KB)"
(`ai_rewrite_kb`), each clamped on read and in `Prefs::clamp`. The window takes them as
`task_ui::Limits` when it opens or comes back (`Limits::of`); a file's row keeps the limits it
was counted with, so its reason says the figure ("only 3 files a run", "12 KB of 10 KB"). Shots
`settings-ai-tasks` in both themes.

**After Fable's review of the milestone.** A note still loading takes the preview mode Settings ›
Editor chose (`chrome::preview_mode`), since the button's mode needs the text; the preview is
asked for only when the note came to the front. Split them over several files in the task
window opens the first run's note only (asked at the start of `plan_write`, before the window
marks the file done). A window brought back after "Largest rewrite" changed reads again the text
files it kept without their text (`task_read_again`). The Add menu lists only as many tabs as
the card has room for (`add_menu_lines`), so "Files…" is always on screen. The batch setting is
the task window's; the folder import keeps its own 40 (`import_ai::BATCH`), being a feature of
its own with its own budget of requests.

## 88. File them copies the files in (2026-11-25, phase C1)

`docs/ai-pictures-plan.md` 4.3, D-209, D-215, after the owner's "I want to copy or move them
along with images". **The switch.** File them has THE FILES, `Link them | Copy them in`, beside
the task on the task row (where the other tasks show "WHAT TO DO"), on "Copy them in" by default
and remembered once changed (`Prefs::ai_files`, `FilesGo`: `copy`, `link` in `settings.ini`).
Too narrow for the full words it reads `Link | Copy in`, and the caption THE FILES goes first.
On the plan face each placed row adds what happens to its file: "copied in", or "linked where it
is: already inside the library's folder" (`TaskScreen::copy_note`, from the paths alone: nothing
is read to draw it). The plan's "how many pictures go with it" is said after the run, in the
toast ("2 copied in with 3 pictures, 1 linked where it is"), not on the row: counting them means
reading every Markdown file on the draw.

**The copy** is shared, `mdshared_app::moves::copy_in` (a child of `moves`, so it uses the
mover's own link reader and rewriter and its path rules): `plan(wanted, inside)` gives each file
a `Copy` (`New` to a free name, `-2` to `-99`; `There` when a file with the same bytes is at
that name; `Stays` for a file inside the library's folder or the group's home, a link, one past
64 MiB, or one that cannot be read), and a Markdown file its `pictures` (those its text shows
from its own folder or below, to the same place beside the copy, so its words do not change;
one whose place holds other bytes is not carried) and its other relative links rewritten to lead
where they did, to the copy of a file of the same run. A file that is not UTF-8 is copied byte
for byte and its row's note says its links are as they were. `write(plan)` makes each copy with
`create_new` (never replacing a file), records the folders it made, and on a failure takes away
what it made while it is the bytes it wrote (`take_back`). The app copies first, then writes the
lines to the copies; when the lines are refused (the group's file has unsaved changes, a UTF-16
file) it calls `take_back` and nothing is left.

**The record** is `Kind::Copies` (`runs::copies_record`, `links_record`'s record with the
copies marked): a line that leads to a copy this run made is a note with `created: true` and
its digest; every other line is a file linked as it was. A picture carries `original` and
`original_sha256`. **Undo** (`plan_of`, `undo_with`): no source; a copy of any extension goes
while it is the bytes written and no library file lists it; a picture copy goes only while its
original is where it was with the same digest, so the copy is never the only one; folders it
made go when empty; the group's file comes back to the byte as for a run of links. `Reach::of`
refuses a record whose copy is outside the library and its home, or whose made picture names no
original. AI runs reads "Copied · N files → Group". Tests: `moves_copy_tests.rs` (16),
`runs_copies_tests.rs` (11) in shared; `task_tests` (6) here. Shots `task-files-copy`,
`task-files-copy-520`, `task-files-link`, both themes.

**After Fable's review of C1.** The pictures of each Markdown file are planned once, before its
links: planned again in the pass that renames a file, a picture the file had taken was dropped,
and one two files share was marked as there for both, so neither wrote it. A file whose links
change is named again by the text it will hold, so a file there that holds exactly that text is
linked, not copied under `-2`. `write(plan, library, home)` keeps the folders it made above the
group's home apart (`Written::made_outside`): only `take_back` removes them, since the record's
folders must be inside the library's reach. Each copy of a file is recorded with its original
and that file's digest (`Record::copied`, JSON `"copied"`), and undo keeps a copy, and its line,
while its original is gone or changed, as it keeps a picture's copy. The app refuses a copy into
the library folder itself (A7, `LIBRARY_HOME_IS_LIBRARY`), and does not list a second time a file
whose very bytes the group's folder holds and the library lists. The files are read and copied on
the UI thread, as a run of links reads them: a file is at most 64 MiB, and the copies are of the
files the owner chose one by one, not a folder's walk. A press on AI edit or the main menu takes
the keyboard from the Notes pane again (the review of the milestone had let the pane keep it,
so the menu's other commands, and the typing after them, went to the pane): the press remembers
the pane's row (`LibraryPane::menu_row`), "AI task…" chosen from that menu works on it, and
any other choice, a press outside the menu or Escape forgets it. The task window keeps the size
it is drawn at (`s.win`), so its menus are placed in the window as it is after a resize. Split
them opens only its first run's note by a mark of the window's own (`split_opened`), not by its
files done, which Rewrite them in the same window marks too.

## 89. File them moves the files in (2026-11-25, phase C2)

THE FILES has a third choice, **Move them in** (`FilesGo::Move`, key `move`). It is C1's copy
and then the originals go: nothing is moved by a rename, so a failure part way leaves a copy and
an original, never a file in neither place.

**What goes** is `copy_in::move_plan(plan, written, library, home)`, asked once the copies are
made and listed. An original goes only when it is on the copy's drive (else its row's reason is
"on another drive: copied"), still the bytes that were copied, listed by no group, and linked to
by no note of the library outside the library's reach (the library folder and the group's home),
since undo could not give that note back. A picture goes only when no other file shows it: the
files the library lists and the Markdown files beside the originals, read as the mover's carry
rule reads them (`Users::others_than`). The library's notes inside the reach that link to an
original that goes are rewritten to lead to its copy (`Relink`). Any other file on the disk is
not looked for; the footer says so on Move them in (`MOVE_WARNING`). The texts undo needs (a
rewritten note's text before, an original whose copy's links changed) are kept up to 3 MiB, past
which the file is copied only.

**The order.** The copies, the lines, then the record (`runs::move_in_record`, a run of copies
with `removed` and `originals`), then `copy_in::move_out`: each note rewritten while it still says
what was read, then each original to the Recycle Bin (`recycle::Through`) while it is the bytes
that were copied and its copy is there, and only when every note that links to it was rewritten.
A record that cannot be written moves nothing. The toast says what was moved, what was only
copied and why, and how many notes now lead to the copies (`task_ui::moved_text`).

**Undo** (D-210) puts back before it removes: each original at the path the record holds, only
while nothing is there, only in a folder that is still there, only with its own bytes (its kept
text, or the copy's while they are the original's), made with `create_new`; each rewritten note
while it holds what the run wrote. Then the copies go as C1's do, since their originals are there
again. What cannot be put back is kept and listed (`UndoPlan::put_back_kept`); the question says
"Puts back N files…" (`UndoPlan::put_back`), and a copy kept because its original is gone says
so (`copies_kept`), not "changed since". `Reach::of` takes a taken original only as an absolute
path with no `..` whose copy is inside the reach.

**Fable's second review of C1, the nits.** A file that stays after all hands the pictures it was
to write to another file of the run that shows them; the link passes run until a chain of any
length settles (at most one more than the files); the row an AI edit or main menu click
remembered lives in that menu (`Popup::library_row`) and dies with it; a new Split them run opens
its first note however many the window ran before.

In a narrow window the task switch drops "them" ("File · Split · Rewrite") and THE FILES reads "Link · Copy · Move" when its shorter words do not fit, so its three segments are never cut.

**Fable's review of C2.** The files settle before their pictures: a picture goes only when every
file of the run that shows it goes (`Remove::shown_by`), and `move_out` keeps one whose file the
bin would not take (`SHOWN_BY_STAYING`). A file or a library note a tab holds with changes not
saved keeps what it is and what it links to where it is (`move_plan`'s `unsaved`; `UNSAVED`,
`UNSAVED_NOTE`), and undo asks the bin before it writes a note (`Bin::may_write`, which
`recycle::SparingUnsaved` refuses for such a tab). An original goes only while its copy is the
bytes that were written and the two are not one file. A record that would not read back
(`RECORD_MAX`) is not written: `move_in_record` says `FileTooLarge`, the window writes the copies'
record and says each file was copied only (`RECORD_FULL`). The question names each file it puts
back; a record that would put back a file of another kind than its copy, or whose copy the record
did not make, is refused whole. A rewritten note goes back only once the original it leads to is
back, and a copy a kept note still leads to stays (`UndoReport::led_to`, its own row in the
report: "Copies kept for the notes that lead to them"). The reach is the one undo will use
(`runs::undo_home`). The toast gives each file copied and not moved its own reason; a tab on a
rewritten note or a moved original is read again at once. The report's rows read "Put back" and
"changed since, or its place is taken", which fit a rewrite's undo too. A file whose very bytes the
library already holds and lists gets no copy from the run, so its original stays where it is
(`ALREADY_LISTED`) with the picture it shows: its record could not put it back (Fable's
re-review).

Tests: `moves_copy_tests.rs` (+4), `runs_copies_tests.rs` (+6) in shared; `task_tests` (+4),
`task_ui` (+1), `replace_ui` (+1) here. Shots `runs-undo-moved`, `runs-undone-moved`, both themes.

Tests of the phase: `moves_copy_tests.rs` (+7), `runs_copies_tests.rs` (+4) in shared; `task_tests` (+3),
`library_ai_tests`, `replace_ui` here. Shots `task-files-move`, `task-files-move-520`, both
themes.

## 90. The engine sends pictures (2026-11-25, phase V1)

The engine half of `docs/ai-pictures-plan.md` sections 6 and 7, and the editor reading the
engine's greeting. Nothing of the window yet: that is V3.

**Protocol 2** (`proto.rs`). `run` takes `images` (`path`, `sha256`), `long_side` (0 is 1568,
at most 2576) and `strip` (default on); an older request without them reads as before. The new
`prepare` sends one `picture` event per picture (format, pixels as sent and of the original,
bytes, digest, what was done, a PNG thumbnail of 160 px in base64), or `error` with the reason
naming the file, then `prepared`. `hello` says `"version":2`.

**The pictures** (`pictures.rs`). A file is read up to 64 MiB and made ready by `resize.rs`, the
spike's resizer rewritten. It sniffs the format from the bytes, reads the size from the header
before anything is allocated, and refuses HEIC, AVIF, TIFF and SVG with their names. A picture
inside both limits goes byte for byte, without its details where `strip` is on, all without
decoding, by lists of what is kept (D-224):
- JPEG: APP0, APP2's colour profile, APP14 and the image's own segments, cut at its end;
- PNG: the image, colour, transparency, density and animation chunks, cut at IEND;
- GIF: without comments and application blocks but the loop count, cut at its end;
- WebP: without EXIF and XMP, VP8X's flags cleared.

An animation (GIF, WebP, PNG with `acTL`) goes whole or is refused. One whose blocks cannot be
walked while its details are to go is refused with the reason. Over a limit, or sideways by its orientation, it is
decoded, turned, scaled by area average and written again (JPEG at 85, PNG and WebP as PNG). A
BMP has its own small reader and goes as a PNG (D-222). A run makes every picture ready first.
One whose digest is not the one `prepare` gave fails the run with kind `image` ("IMG_4032.jpg
changed since it was measured"), and nothing is sent. Otherwise `sent` says the bytes and digest
of each, and the bytes of the system text and prompt. It is said through `Job::sending`, which
each sender calls as its request leaves: never for a run that failed before (D-225).

**The senders.** The Claude API gets the user content as a list: image blocks, then the text.
OpenAI-style servers get the text, then one `image_url` data URL per picture; a 400 or 422, for a request with
pictures, whose server says in its own words that the model takes no images (`Fail::said`, the
model's and host's names left out, a refusal of one picture excepted) is `no_vision`: "This model
does not read pictures: llama3 at localhost:11434. It said: …" (D-225). Claude Code gets `--input-format stream-json` and
one user message on stdin, pictures first (D-221). Every Claude Code run, text or not, adds
`--strict-mcp-config` (D-213). Codex gets the pictures written as `1.jpg`, `2.png`… in a new
folder `mdedit-ai-pictures-<pid>-<time>-<n>` under the temp folder (0700 on unix), one
`--image` each, then `--` and `-`. The folder goes when the run ends. Such folders older than
six hours are removed by each Codex picture run and, on a thread, when the engine starts (D-222,
D-225). The window offers Codex no pictures until a
try by hand (D-221).

**The editor.** `ai::hello_version` reads the greeting, and `Client::version` keeps it. A done
text that contains the greeting's words is no longer taken for it. `Client::pictures_refused`
gives `OLD_ENGINE` for an engine older than 2; the window will ask it (V3).

The engine's exe grows from 2.27 MB to 2.78 MB (Windows release, the decoders and the JPEG
encoder). It is not on the editor's launch path, which does not change.

Tests: `resize.rs` (33), `pictures.rs` (5), `proto.rs` (+1), `api/tests.rs` (+3),
`cli/tests.rs` (+4), `tests/engine.rs` (+4, and +2 with fake CLIs on unix only, not run on
this PC), `ai.rs` (+1), `ai_tests.rs` (+1). Each was checked to fail with its fix reverted. No
picture has been sent to a real provider by the engine. V0 sent one through Claude Code by hand.

## 91. Write notes: the answers and the composer (2026-11-25, phase V2)

`mdedit-core/src/notes_ai.rs`, the core of the pictures task (plan section 8). Nothing of it is
reached from the window yet (V3).

**The requests.** `look_prompt` names each picture of a batch of four by its number and its
file's name, never its folder, and asks for the text written out only when "Write out the text"
is on. `arrange_prompt` holds no picture. It gives the instruction, how many notes, the group's
sections, what the look said of each picture (of its text the first 300 characters, on one line), and each
text file whole between two marked lines. `LOOK_SYSTEM` and `ARRANGE_SYSTEM` say that words in a
picture are never instructions, and that a picture goes in a note only as
`![words](picture:N)`.

**The readers** keep `import_ai`'s depth-bounded JSON reader, made crate-visible. The eleven
checks are each a test (`notes_ai_tests.rs`, `check_1` to `check_11`):

- the envelope (2 MB, one object, version 1);
- numbers that were not sent, and the first answer wins;
- titles: one line of 120 characters, no brackets;
- sections: three levels of 64 characters, through `library::folder_name`;
- a picture's fields cut and marked " (cut)", and its kind one of five words or "other";
- a body: 200 KB cut at a line; LF; no controls but tab and line feed; no front matter; no first
  heading that repeats the title;
- pictures: a sent number becomes `Part::Picture`, placed once; any other picture loses its `!`;
- links to anything but `http:`, `https:` or `mailto:` are unlinked; `<` is escaped outside code;
- nothing else stays live (D-223): a fence counts only at the first column, a code span only
  closed on its line, and a link only as the app writes it again (one plain address, its words
  escaped); every other `[`, `]`, unclosed backtick, `!` before a link and `<` is escaped, so no
  definition is one. The brackets and backtick runs of a line are matched once, in linear time;
- the picture's text as a quote;
- 100 notes; an empty note or a second note of one path dropped and counted;
- a picture in no kept note listed as not used, whatever the model said;
- no model text in a path except through `note_slug` and `folder_name`.

**The composer.** `compose` writes `# Title` and the body. Each picture is the app's own line,
`![alt](pictures/<name>)`, on a paragraph of its own: the name is escaped for an address (space, parentheses, angle brackets, `#`, `?`, `%`, `\`); the
alt is one line of 200 characters, its brackets escaped; an empty alt takes what the look said.
`joined` makes "One note" of several, each after the first under its title as `##`.
`per_picture` makes "One per picture", and the notes with the AI off, with no answer: the look's
title or the file's stem; a second note of one path becomes "Title 2", the number on the first
40 characters, so the name's cap of 64 never cuts it (D-223). `picture_name` names a
copy after what it shows (the note rule) or after the original. `numbered` gives `-2` for a
taken name; whether a name is taken, and whether the same bytes are there already, is the
writer's (V5).

## 92. Write notes in the task window (2026-11-25, phase V3)

`notes_task.rs` (state, pure), `app/notes_run.rs` (the window's part), `app/notes_demo.rs`
(`--task-demo notes…`). Write notes is the first of the window's four tasks.

**Measuring.** A picture added to the window is measured at once by the engine's `prepare`
(nothing sent anywhere): its row says what goes ("sent smaller: 1568 × 1176 · 373 KB") and
shows the engine's thumbnail, made 80 px once when it arrives. Strip or Larger changed measures
again. With the AI off nothing is measured. Codex and an engine older than protocol 2 are offered
no pictures, the reason where the button is (D-221).

**The run.** "Send 5 pictures and 1 file": look requests of four pictures, as many out at once
as the connection takes (D-080), then one arrange request with no picture in it. One per picture
with pictures, or the AI off, makes the notes here with no second request. A text file no request
reads says so on its row and is not counted (D-226). The looks are kept for the next Send while
the instruction is the same and the text was written out if it is asked now; the button counts
only the pictures still to look at.

**Failures** (plan 15). A failed look keeps what was answered: "Try it again" sends the failed
batches then those never sent; "Go on with N" sends those never sent without the failed. A
picture the answer says nothing of is a failure of its own (a batch with none said fails whole).
A picture changed since it was measured is measured again first. A model that reads no pictures
stops the run with Open AI settings. A failed arrange offers "Try it again" and "One note per
picture instead" (for the window only, not the setting). A notice bar over the list's foot says
which.

**Pasting.** Ctrl+V on Write notes with no text on the clipboard adds its picture, saved as
`%TEMP%\mdedit-pasted\Screenshot <ms>.png`; the window's pasted files go when it closes, and any
older than a day when a picture is pasted (D-227).

## 93. "What is sent" as a card (2026-11-25, plan section 13)

`sent_card.rs`, `sent_card_draw.rs`, `app/sent_card.rs`. On Write notes, "Show what is sent"
opens a card over the window (the other tasks keep the text tab): a tab per request, "Look 1 of
2" … "Arrange", or arrows past four. A look tab lists its pictures as they are sent (thumbnail,
format, pixels, bytes, what was done, size on disk), read from the window's files at each draw,
then the request's text word for word, selectable (drag, Shift+click, Ctrl+A, Ctrl+C; nothing
selected copies all). The arrange tab says no picture is in it and gives its text with the
group's sections. "Open the text in a tab" does what the old button did. With the AI off the
card is a short one saying nothing is sent. The first window of Write notes with a picture and
the AI on says once, in a band above the button: "A picture is sent whole: everything visible in
it, a password or a face included." (pref `ai_pictures_told`, D-229).

## 94. The plan card of Write notes (2026-11-25, phase V4)

`notes_plan_ui.rs` (state, edits, the read view), `notes_plan_draw.rs`, `app/notes_plan.rs`.
When the plan is read, a card replaces the task window: on the left the group, its sections, the
notes and their pictures (thumbnail, copy name, what the model said), and NOT USED; on the right
the note selected "As it will read" or as Markdown (`compose`, word for word). Every edit is made
on `run.arranged` at once, which is what the write reads: tick a note or a picture in or out, F2
renames a note, a section (in every note under it) or a copy (`run.copy_names`), the row menu's
"Put in…" and "Join with…". A rename that would make two notes one file is refused.

Footer: Discard (the plan goes, the looks stay), Back and Esc (to the window, whose button then
reads "Review the plan"), "Arrange again…" (the arrange asked again with its own instruction, no
picture sent; the plan before it is kept until the answer is read, and given back if it fails or
is stopped), and "Write N notes". The plan lives in its window: Cancel on a window that keeps one
says once that closing throws it away (D-228).

## 95. Write notes writes the notes (2026-11-25, phase V5)

`app/notes_write.rs` (the window's part), `mdedit-core/src/notes_write.rs` (the names, the
copies' list, `compose_at`), and shared `runs.rs`'s `Kind::Notes` (`f/move-in` `ac9fde1`).

**Before anything is written** (`notes_job`), each refusal writing nothing and saying why: a plan
and a group made; every note and copy inside the library or the group's declared folder; no note
path taken; at most 100 notes of 200 KB, 100 copies, 200 MB of copies, 64 MiB a copy; an
extension undo knows; the group file saved, writable and not full; a dry run of the links that
reads back. The card's button follows a light check of its own, and the write asks these when
pressed.

**The order** (plan 4.3): the record (state "writing", every note and every planned copy in it),
each folder saved to the record before it is made, the copies into `<note folder>/pictures/` once
per folder (a name taken by other bytes numbered `-2` … `-99`; the same bytes reused), the notes
(`compose_at`: a copy by its name, with Copy off a path relative to the note, or `file:///` on
another drive), the read-back, the links (the group file's text before them kept for undo), and
the record closed Done or LinksPending. A copy's name is the owner's from the card, else the
look's title with Rename on, else the original's own name, always with the original's extension.
A pasted picture and one on a network share are always copied (D-227, D-230).

**A copy that fails** is said in the report and the toast, and its note is written with a line in
its place saying so, checked again against the 200 KB limit. When no note was written, or a pasted
picture's copy failed, the window stays open with its pictures and says how to write again;
otherwise it closes, the Notes pane shows the first note and it opens with its preview.

**Undo** (AI runs…) of a `Kind::Notes` run checks no source. It removes the notes that are
unchanged and keeps the changed ones; removes a copy only while it is the bytes written, its
original (if it names one) is there unchanged, and no kept note shows it ("Pictures kept for the
notes that show them"); never an original; restores the links and removes the empty folders it
made. A run that died at any step is undone or finished from its record. An older mdedit reads a
notes record as written by a newer version.

## 96. Motion: one animator, and the menus' fade (2026-11-25)

`bins/mdedit/src/motion.rs`, its wiring in `app.rs` (`compose`) and `app/commands.rs`
(`timers`, `woken`), tests in `app/motion_tests.rs`. Built on shared `anim` (`09b01c1`; shared's
`docs/ANIMATION.md` says how its pieces work). D-232, D-233, D-235, D-250, D-252.

**The animator.** `App::motion` holds the window's one `Animator`. `compose` calls
`begin_frame` first and `end_frame` last; the deadline is kept in `Motion::due`, which `timers`
turns into a wake-up and clears when it fires (it asks for a frame, `about_to_wait` requests the
redraw). While nothing moves the deadline is `None`, so there is no timer and no frame: an idle
window, menu open or not, costs nothing.

**The pin `09b01c1`** (through `8d35a07`, shared PR 24; D-256) changed one thing for mdedit: the
stack's wait for the lift is skipped when nothing animates. The rest of that pin is not mdedit's:
the one-frame hover flash (#31) was in `chrome::*_animated`, and mdedit draws its title row with
the static `chrome::draw_tab`, `draw_win_button`, `draw_new_tab_button` and `button_fill`;
colour rows and drag-number rows (#30) are opt-in rows of shared's Settings form that mdedit does
not build; the application's veil (#27, `Frost::set_veil`) is for a backdrop strength mdedit does
not offer; and the pool's cancel (#28) and the window-procedure hand-over (#29) are for apps using
`mdshared_app::pool` and `single_mutex`, which mdedit does not. `pages::CARD_RADIUS`,
`Form::menu_rect` and `Form::menu_bounds` are additions a host may use; the before-and-after
shots of the pin are identical but for the clock-stamped `runs*`.

**Tooltips.** `Ui::tip` is shared's `Tooltip<Hit>`: `update_tip` (`app/input.rs`) gives it the
control under the pointer, its text from `chrome::tooltip` (or the replace strip's own) and its
rectangle, and the pointer's place **on every move** — `Tooltip::hover_at`, so the
`document::TIP_DELAY` 500 ms counts from the pointer coming to rest, not from entering the
control (D-257). `timers` turns `Tooltip::deadline` into the wake-up and `Tooltip::tick` shows
it; a neighbour's tip shows at once while one shows, a press hides it and blocks that control
until the pointer leaves (`Tooltip::press`), and a key, a scroll, a popup or the pointer leaving
the window hides it (`Ui::hide_tip`, which also forgets the control and place it was pointed at,
so coming back arms its delay again). It is drawn as a bubble with the presence `Motion::tip_presence` asks for in
the group `TIP_GROUP`: it fades in by `document::TIP` and is gone at once, so no frame is asked
for after it goes. Under `Policy::Never` the presence is 1 on its first frame and the bubble is
drawn by `Bubble::draw`, the pixels of before. The fade's `t` is in the overlay signature with
the bubbles', so each frame of it redraws what the tip covers; a tip taken away mid-fade goes at
once and the pixels under it come back. A move that changes neither the control nor the pointer's
place is skipped before any text is asked for (`Ui::tip_point`): 215 ns a move within one
control and 25 ns when skipped, a batch of 200,000 divided
(`docs/measurements/2026-11-26-tooltip-moves.md`). `tests/shots.sh` shows it part way in with
`--tip-ms 90` (`tip-rising-light`, `tip-rising-dark`).

**Its tests**, four from M-C — `with_motion_off_the_tip_is_there_on_its_first_frame` (the whole
tip on its first frame, `Bubble::draw`, no timer), `a_tip_fades_in_and_settles_with_no_timer_left`,
`a_tip_waits_from_the_pointer_coming_to_rest` (through `pointer_moved`) and
`a_press_hides_the_tip_until_the_pointer_leaves` — and three from the review of them:
`a_tip_taken_away_mid_fade_goes_at_once_and_the_pixels_come_back`,
`a_neighbours_tip_shows_at_once_without_a_second_fade` and
`every_frame_of_the_fade_damages_what_the_tip_covers` — and two from the review of them, a
scroll hiding the tip and a pointer coming back to the same place waiting its whole delay again
(D-260).

**Whether anything moves** is `Motion::policy`, the one place: `Policy::Never` in a shot and in
every test (`args.shot` or `cfg(test)`), and with the Animations setting off (`prefs.animations`,
read every frame); otherwise `Policy::System`, which is instant while the system asks for less
motion. The system's wish is read, and watched with `watch_motion_reduced`, called on a short
helper thread because it waits for its hidden window (a hidden window on its
own thread on Windows, the portal's helper on Linux), once the window has settled after its
first frame (`App::watch_motion`, from `about_to_wait`'s settled block, so Linux's portal has
answered before the first menu), and failing that from the first menu that could fade (the
setting was off then); never on the first frame's path, never while nothing can move. Its reader
is a field (`Motion::reader`), which a test replaces. `Wake::Motion` and a window gaining the focus read it again
(`refresh`; macOS has nothing to watch). A test that wants motion takes `Motion::moving` with a
`ManualClock`. A shot of a menu part way in (`--menu-fade-ms MS`, with `--shot` and `--menu`
only, at most 1000 ms) animates whatever the system says, on a clock stepped 16 ms at a time
(the animator takes a longer step for a stall): `menu-main-fade` in both themes.

**The menus' fade.** `Motion::menus` asks each open menu's presence, in the group `MENUS`, by the
popup's `serial` (a new number for every `Popup` made, so a menu opened again, or Recent replacing
the main menu, enters again; a menu listed again in place keeps its serial). Each enters by
`MENU` (in `motion.rs`, the one place timings live: shared's `document::MENU` as it is, 117 ms
decelerating since shared `f45a71c`, D-250): it fades as one thing and drops
`MENU_DROP` (6 px) into place, rising for a menu above its anchor; a submenu enters the same way
beside its parent. The AI menu's Modify buttons are drawn inside the same `pop`. Closing is
instant: the popup goes, and the regions under its bounds are drawn the next frame. The word
completion list (`PopupKind::Complete`) never fades: `fades` leaves it out, and it draws as
before. The menu inside the notes plan card is drawn by the card and does not fade.

**What a fading frame draws** (`menu_frame`). The frame that opens a menu draws as before (the
regions under the overlays, then the overlays), and keeps the pixels under the menus' presence
bounds in `Motion::backing` just before drawing them. A later frame in which only the menus moved
(`MenuFrame::Alone`: no overlay changed, no region under them drawn, no other overlay shown, the
menus where the backing was taken) puts the backing back and draws the menus over it: only their
bounds are drawn and presented; while the backing is held, its rectangle counts as under the
overlays, so a region drawn there is drawn whole before the menus. Any other frame while they move is `Whole`: everything under the
overlays is drawn again and the backing taken again. The frame that sees them arrive draws them at
rest and lets the backing go. The menus' damage is their `presence_bounds()` (the shadow, the drop
and a device pixel), not `rect().inset(-20)`.

**Frost.** The overlay painter is given the frost key (`set_frost_key`), so a menu fading over
frosted chrome mixes with it as the compositor shows it (`Snapshot::crossfade_keyed`) instead of
turning opaque half way. Not seen on a frosted window.

## 97. Motion: toasts rise in, stack and leave (2026-11-25)

`bins/mdedit/src/motion.rs` (`toasts`, `Motion::sync_toasts`, `toast_y`), the status in
`docs.rs` (`set_toast`, `confirm_toast`, `clear_toast`, `toast_line`), the drawing in
`app.rs` (`compose`), tests in `app/motion_tests.rs`. D-251, D-252.

**The status and the toasts.** `Docs` keeps one status message: its text, until when (2.5 s from
when it is set, `strings::TOAST_MS`), whether it stacks, and a serial, new for every message. The
window's toasts follow it in `Motion::sync_toasts`, asked on every frame before the overlays are
laid out: a serial not seen yet is taken in, by `Stack::replace_newest` for `set_toast` (a status:
in place, no exit, no entrance) after `remove` of every other toast the last frame showed — the
ones still playing their exit included, which `current` does not name — by `Stack::show` for
`confirm_toast` (a confirmation: under the others, and it enters once they have lifted,
`Stack::with_wait_for_lift`), and `Stack::clear` when the status is cleared. A status that takes
over **a toast still waiting for its lift** — and only such a toast — is put at presence 1
through `Animator::set` for that frame: the stack gives the wait up, but the animator keeps the
delay it was first given for the name, since a new motion for an unchanged target is ignored, and
the status would wait in the dark. Which toast that is is `Motion::toast_waiting`, the key
`Stack::show` returned while the stack already held something (a toast at rest, or one leaving),
**and** that key must still be at nothing on the last frame: one past its wait is coming in, and
the status lets it go on from where it is. A toast that came onto an empty stack was pushed
without a wait, so a status taking it over lets it keep rising, first frame or part way. A repeat
of the waiting toast leaves `toast_waiting` as it was, since `revive` keeps the animator's delay.
A confirmation said again never stands twice: while it is the newest it keeps its own toast,
key and presence (its time reset, so Copy path pressed twice does not blink), and one standing
higher is taken out first and comes in again at the bottom. A message whose time is already past
when it is taken in shows nothing. What is left of its time is counted on from that frame on the
frame's clock, so a test's or a shot's clock moved by hand runs it too. `Stack::tick` sends away
what is due; its `deadline` is `toast_until`, the timer that wakes for it (`timers`), and a
message not taken in yet asks for a frame there (`toast_unseen`).

**Drawing.** `Stack::frame` gives each toast, bottom first, with its presence and its lift (the
heights and 8 px gaps of the ones under it, on its way by `fluent::MOVE`). `compose` walks them
the other way, newest last, so the bottom toast's body covers the shadow the one above it throws;
places each with `Bubble::toast` and raises it by its lift to a whole device pixel (`toast_y`,
which moves it by whole device pixels from its place at rest); one that would reach above the
text area is left out unless it is the bottom one, however short the window. A toast at rest is
drawn with `Bubble::draw`, one on its way with `draw_presence` (it fades and rises 8 px). The
overlays' bounds hold each toast's `presence_bounds()`, and the overlay signature holds each
presence, so a moving toast redraws what is under it each frame. Under `Policy::Never` every
presence is at once 1 or gone: one toast at rest is the pixels of before. The wait for the lift
is a delay, and `Stack::frame` asks for it only while the animator animates (shared PR 24, in
the pin `09b01c1`): with nothing moving there is no lift, so a stacked toast is there on the
frame it is pushed. `with_no_motion_two_confirmations_are_both_there_at_once` and
`the_one_toast_a_very_short_window_shows_is_there_at_once` hold that, in all three ways of
having no motion. The status's snap (`Animator::set`) stays: with motion on, the animator keeps
a name's delay even after `Stack::replace` gives the wait up.

**Who stacks** (`confirm_toast`): File path copied, Path copied, Copied the result, the crash
report copied, Image saved as … (both paste paths), Renamed to …, the macro saved, a note added
to the library, Saved as … (an AI prompt), The picture on the clipboard is added, a note moved,
a group deleted. Everything else replaces (`set_toast`).

**Shots.** `--toast TEXT` (repeatable: the earlier ones at rest and stacked, the last one coming
in) and `--motion-ms MS` (the last one that far in, on the shot's clock). A toast pushed onto
another waits for its lift, so a toast shot steps the animator's clock (`Motion::shot_fade`):
each earlier toast is stepped to rest, and the last one by `--motion-ms`, or to rest without it.
`toast-*`, `toast-rising-*` (50 ms), `toasts-stacking-*` (200 ms: the lift done, the newcomer
part way in), `toasts-stacked-*`, `toast-repeated-*` (the first said again, at the bottom),
`toasts-short-*` (520 × 220, the third left out).

## 98. Motion: the Settings form and the tree's chevrons (M-D, 2026-11-26)

`motion.rs` (`DISCLOSURE`, `SIDEBAR`, `SETTINGS`), `panes.rs` (`draw_files`), `app.rs` (`compose`,
the shot's `--settings-ms` and `--tree-ms`), shared's `settings::pages::Form::draw_animated` and
`glyph::draw_disclosure_animated_with`. D-259.

**The Settings form.** `Form::draw_animated` slides switches and the selected segment's fill.
It fades an unselected category's highlight and the rows of an open dropdown, not general
control hover fills. The paged form has no scrollbar. Changing pages or closing the dialog
is instant and forgets the old page's tracks. The `SETTINGS` group repaints the whole window
when due; an early frame that skips the form keeps the group and its pending paint deadline.
With Animations off, a real toggle in the app matches a reference drawn with `Form::draw` on
the first frame. The tree test compares the live Files pane's chevrons against plain glyphs.

**The tree's chevron.** Each folder row's disclosure is `glyph::draw_disclosure_animated_with`
under the name `("files", path)`, turned by `motion::DISCLOSURE` (shared's `DISCLOSURE_MOTION`,
167 ms, `fluent::MOVE`): at rest it is the plain right or down chevron, pixel for pixel. The pane
draws inside the group `SIDEBAR`; a frame whose deadlines have come damages the Files region
(`group_due(SIDEBAR, now)`), and a frame that draws something else keeps the group
(`keep_group(SIDEBAR)`) so a turn goes on from where it was instead of being taken for new.

**Shots.** `--settings-ms MS` draws the old knob position, changes the Typing switch at time
zero, then advances to the requested sample (`settings-switch-*`, 60 ms). `--tree-ms MS`
samples the first closed folder's turn (`tree-chevron-*`, 60 ms). Both use the shot's manual
clock and run in both themes. The Settings test uses 16 ms ticks: the first is early at a
60 Hz cadence and skips the form; the second reaches the original deadline.

**The title row (M-E, D4).** `chrome::draw_title` draws its tabs with `chrome::draw_tab_animated`
(named `("tab", document id)`, so a tab that moves keeps its fade), its caption buttons with
`draw_win_button_animated` (named `("win", hit)`; Close's fill turns red and its glyph white) and
the new-tab button with `draw_new_tab_button_animated` (`("new-tab",)`), all in the group
`TITLE`: a quiet tab's hover fill and its close button fade by `document::HOVER` (shared's
`FADE`, 83 ms linear). While the group is in flight the frame damages the title row, and a frame
that skips it keeps the group. The bar's buttons (`Ctx::fill_for`) are not animated: they are
not the title row (X3's `Menu::draw_in_animated` covers a menu's rows, not the bar's).
The shared fix of the `t == 0` hover flash (shared #31) is in the pin, so the fade starts from
the rest look rather than a frame of the full one. Shots: `--hover-ms MS` takes the tip away and
moves on a clock from the first frame that sets the hover — a still window would draw the
finished fade and the steps after it would prove nothing (`title-hover-*`, `tab-hover-*`, 40 ms
of 83, both themes; the second shot needs two files for a quiet tab).

**A menu's hovered row, the palette and the bars (M-E, X3).** `motion::draw_menu` draws each
menu with `Menu::draw_in_animated`, keyed by the popup serial, in the group `ROWS`. A frame
whose group is due damages the overlay; a frame that skips the popup keeps the group and its
earlier deadline. The completion popup uses plain drawing for both entrance and hover. The
command palette draws by `Palette::draw_presence` at `Motion::palette_t` (117 ms decelerating)
and is damaged by its
`presence_bounds()` — the same bounds a menu is damaged by, `rect().inset(-20)` having gone from
both. The anchored language list stays instant. Each update, crash, stale-tab and rename
notice uses `notice::draw_sliding`; `notice::bar_height` sets its visible height for
layout and clips its button hits. The editor moves down as the strip grows, with no empty space
reserved ahead of it.
Shots: `--motion-clock MS`, which makes the shot animate from its first frame and then steps the
clock by hand: `menu-row-hover-*`, `palette-in-*` and `{update,crash,stale,rename}-sliding-*`,
40 ms in both themes.
