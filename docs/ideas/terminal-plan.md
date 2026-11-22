# A terminal: the plan

Written 2026-11-22 for the owner to approve. "You" in this document is the owner. Nothing here is
built. Section 12 has your answers to its questions, and section 13 lists the decisions they
settle, ready for a decisions log.

**What you asked for.** A serious plan for a terminal emulator, Windows first. Then, while this
was being written: "serious, but power use oriented. a standard emulator is all I need without
endless options. main window is all that matters for first release and basic options, along with
relevant ones existing from the shared crate."

**What that changes.** `docs/ideas/terminal.md` said a plain terminal is a **Don't** and only the block
version (commands folded, saved as notes, explained by the AI) is worth building. Your message
asks for the plain terminal first. So this plan builds a good standard terminal, and keeps the
blocks as a later phase that needs nothing torn up. The screen model records the shell's
command marks from the first day, so the blocks can come later without a rewrite.

**Your answers** (2026-11-22, to section 12): its own app, `mdterm`, in its own repository;
**no tabs in the first release**, splits maybe later; PowerShell 7 by default; both themes, as
the rest of the family; `vte` approved, so no `alacritty_terminal`, not even as a test oracle;
the bundled ConPTY only if phase 0 shows the system's is not good enough; mail was never asked
for, so nothing comes before the terminal. The sections below are rewritten to match.

**What I read.** `CLAUDE.md`; `docs/PLAN.md`; `docs/CODE.md`; `docs/ideas/terminal.md`; the end of
`docs/DECISIONS.md`; `docs/ai-library-plan.md` by heading and its first pages, as the model of a
plan here; `docs/ecosystem-ideas.md` as committed on `main` (§12 and "Overall"); the mock's
`README.md` and `numbers.txt` in `..\mdedit-term\shots\mock-term\`. In `..\markdown-shared-rs` at
`b4b2182`: `mdshared-text/src/text.rs` (the head), `mdshared-ui/src/settings.rs` (the API),
`mdshared-ui/src/theme.rs` (the palette), `mdshared-app/src/job.rs` (the head).

**What I ran.** In a scratch folder outside the repository: `cargo generate-lockfile` for one
throwaway package per candidate crate, compared with this workspace's `Cargo.lock`; and three
release builds with this repository's release profile (thin LTO, one codegen unit, abort on
panic, stripped) to price the two parser crates in the exe (section 4.4). Nothing was added to
any repository.

**What I did not do.** No ConPTY code was written or run. Nothing in section 3 about ConPTY's
behaviour was observed on this PC: it is from what I know of the API and of how Windows
Terminal, WezTerm and Alacritty use it, and each point that matters is a check in phase 0. No
throughput or latency figure here was measured; they are targets. I did not look at the 20 mock
shots themselves, only their list and the layout numbers.

---

## 1. What it is, and what it refuses to be

**It is** a standard terminal emulator for one person who lives in it: PowerShell, git, cargo,
ssh, WSL, vim, htop. It is fast to open, it scrolls and prints fast, it draws text as well as
mdedit does, and it looks like a sister of mdview and mdedit: the same title row, menus,
settings form, themes and shortcuts.

**It refuses to be:**

- **A multiplexer.** No splits in the first release, no sessions that outlive the window, no
  detach. tmux exists, and runs inside it.
- **Configurable without end.** About a dozen settings (section 8.3). No per-profile anything, no
  key-binding editor, no scripting, no plugins.
- **A GPU terminal.** It draws on the CPU into softbuffer, like the other two. Section 6 argues
  that this is enough.
- **A graphics terminal.** No sixel, no kitty images, no iTerm images in the first release.
- **The Windows default terminal.** Windows 11 can hand every new console window to a chosen
  terminal. That needs a COM hand-off server and its own registration. Not planned.
- **On the network.** It makes no request, except its own updates. ssh is a program you run in
  it, not a feature of it.

**Who uses it.** You, on this PC (Windows 11, build 26300), most of the day. Then on Linux and the
Mac, where the release pipeline already builds everything.

**Where it lives: its own app.** mdedit's charter says "Not an IDE. No language servers, no build
or debug, no plugins, **no terminal**". The reasons are its budgets: a terminal pane in mdedit
puts a PTY, a parser, a second kind of text drawing and a reader thread inside the editor, and
every one of them is on someone's path. A separate exe keeps every rule as it is:

| | Its own app (recommended) | A pane in mdedit |
|---|---|---|
| mdedit's charter | Untouched | Needs a written exception to "no terminal" |
| mdedit's launch path, memory, keystroke | Untouched | A new lazy path; the grid's memory counts against 30 MiB |
| Its own budgets | Its own, measured on their own | Shared with the editor's |
| A panic in the parser | Ends a terminal window | Ends the editor, with unsaved text (rule 11) |
| Code shared | The shared crates, like mdview | All of mdedit |
| Cost | The window loop copied once (about 3 days) | Fitting a grid region into mdedit's regions (about the same) |

The last row is why: a pane would not save time, and it would put the editor at risk. If you ever
want "open a terminal here" from mdedit, it is one command that starts the terminal exe in the
document's folder, as mdedit already starts mdview.

Its name is **`mdterm`** (your answer to question 1).

**Windows first.** Every phase is done and used on Windows before Linux and the Mac are started.
Linux and the Mac change only the PTY layer and the shell list (section 10, phase 8).

---

## 2. The budgets

The house budgets, read for a terminal. Each is measured before the release, the way
`docs/CODE.md` measures mdedit: release builds, before against after, on the same day.

| | Budget | How it is measured |
|---|---|---|
| First frame | under 100 ms | Window with an empty grid and the cursor, before the shell is started. The shell's own start (pwsh: 300 to 800 ms) is not ours and is reported apart as "prompt visible" |
| Private memory, one window, empty | under 30 MiB | As mdedit |
| Private memory, 10,000 lines of scrollback | under 40 MiB | `type` a 10,000-line log, then read the figure |
| Keystroke to pixels | under 8 ms of our own time | Key event to present, without the shell's echo. The echo through ConPTY is measured and reported apart |
| Frame while output streams | under 8 ms at 1280 x 800 | `type` a 100 MB file; the worst frame |
| Throughput, parser and screen alone | at least 200 MB/s | A bench over recorded output, no window |
| Throughput, end to end | no slower than Windows Terminal on the same PC | Time `type big.txt` in both |
| Idle CPU | zero | No timer but the cursor's blink, and the blink stops after a while (section 6.5) |

**Nothing on the launch path but the window.** The pseudoconsole and the shell are started after
the first frame is presented, on their own thread. The shell list (which of pwsh, WSL, Git Bash is
installed) is read only when the "New window with" menu or the settings open, never at launch.

---

## 3. The Windows plumbing

### 3.1 ConPTY

Windows has a real pseudoconsole since Windows 10 1809: `CreatePseudoConsole`. The plan uses the
system's, through `windows-sys`, which is already in `Cargo.lock` (0.59 and 0.61). About 400 lines
of our own code; no new crate (section 4.4 prices the crates that would do it).

The sequence, per window (one shell per window in the first release; the code keeps a
"session" type of its own so tabs or splits can hold several later):

1. Two anonymous pipes (`CreatePipe`): one for the shell's input, one for its output.
2. `CreatePseudoConsole(size, input_read, output_write, flags, &hpc)`, with the grid's columns and
   rows. Flags: none at first. `PSEUDOCONSOLE_INHERIT_CURSOR` is not used: with it, ConPTY asks
   the terminal where the cursor is (`ESC[6n`) and blocks until it hears back, and we start with a
   clean screen anyway.
3. Close our copies of the two ends ConPTY now holds.
4. `InitializeProcThreadAttributeList` with `PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE`, then
   `CreateProcessW` with `EXTENDED_STARTUPINFO_PRESENT`, the shell's command line, the starting
   folder and the environment (3.3). The process is created suspended, put in the session's job object
   (3.4), then resumed.
5. A reader thread per session blocks in `ReadFile` on the output pipe. It hands bytes to the parser
   and wakes the window (section 6.2). A blocked `ReadFile` costs no CPU.
6. Keys and pastes are written to the input pipe from the window thread. A write can block if the
   shell does not read (a large paste into a busy program), so writes go through a small writer
   thread with a queue, and the window never waits.
7. **Resize**: `ResizePseudoConsole(hpc, size)` when the grid's size changes, at most once per
   frame while the window is being dragged.

**What ConPTY is, and why it matters.** ConPTY is not a pipe. It is a whole console host
(`conhost.exe` in pseudoconsole mode) that keeps its own screen buffer and *renders* it to us as
VT sequences. Two things follow:

- It sends what it thinks the screen is, not what the program printed. Usually the same; after a
  resize, it repaints, and its idea of the wrapped lines can differ from ours. Windows Terminal
  has carried fixes for this for years. This is the largest risk of the Windows port (section 11).
- It talks to us in its own dialect. On start it may ask for our device attributes
  (`ESC[c`) and turn on **win32-input-mode** (`ESC[?9001h`), in which it wants every key as a
  full Windows key event. We answer the first and support the second (section 7.1).

**Phase 0 checks** (each a line in the spike's report): which sequences ConPTY passes through
unchanged on build 26300 (OSC 8 hyperlinks, OSC 52, OSC 133, OSC 7, `?2026` synchronized output);
how it repaints on resize; its throughput on `type` of a 100 MB file; and the echo delay of one key.
I believe recent Windows 11 builds pass unknown OSC sequences through, and earlier ones did not;
not verified.

**The system's ConPTY, or the one Windows Terminal ships.** Windows Terminal and WezTerm can
carry their own newer `conpty.dll` and `OpenConsole.exe` (MIT, about 1.2 MB together), which fix
resize and passthrough bugs that older Windows builds still have. On your build the system's
should be enough. Plan: the system's. If phase 0 shows a bug that only the newer one fixes, it is
decision 13.7, and it changes the installer, not the code.

### 3.2 Shells

| Shell | Found by | Command line |
|---|---|---|
| PowerShell 7 | `pwsh.exe` on `PATH`, or `%ProgramFiles%\PowerShell\7\pwsh.exe` | `pwsh.exe -NoLogo` |
| Windows PowerShell | Always present | `powershell.exe -NoLogo` |
| Command Prompt | `%ComSpec%` | `cmd.exe` |
| WSL | `wsl.exe --list --quiet` (UTF-16 output), one entry per distribution | `wsl.exe -d <name> --cd ~` |
| Git Bash | `%ProgramFiles%\Git\bin\bash.exe`, or Git's install path from the registry | `bash.exe --login -i` |

The default is PowerShell 7 if present, else Windows PowerShell (your answer to question 3). The others are in
the "New window with" menu (and the settings), read when they open, so nothing is spawned at
launch. One setting holds a
custom command line, for anything else (ssh to a fixed host, nushell). That is the whole of
"profiles".

WSL goes through ConPTY like any console program; nothing special is needed, and `wsl.exe` does
the rest. Its start is slow (a second or more when the VM is cold); that is WSL's.

### 3.3 Environment and the starting folder

- The environment is the terminal's own, plus `TERM_PROGRAM=mdterm`, `TERM_PROGRAM_VERSION`, and
  `COLORTERM=truecolor`. On Windows `TERM` is not set: Windows programs do not read it, and WSL
  sets its own. On Linux and the Mac, `TERM=xterm-256color`.
- A new window starts in the folder given on the command line (`mdterm --cd <dir>`, which is how
  mdedit or Explorer would open it), else in the user's home.
- **A new window (Ctrl+Shift+N) starts in the current window's folder**, if the shell has said
  where it is: OSC 7
  (`file://host/path`) or Windows Terminal's OSC 9;9. PowerShell says neither until a line is added
  to its profile; section 8.4 offers that line, once, and does nothing without it.
- The environment is read once at start. A change to the user's `PATH` reaches new windows, not
  windows opened from an old one. Windows Terminal does the same; it is said in the docs, not
  fixed.

### 3.4 Ctrl+C, closing the window, and the processes left behind

- **Ctrl+C** with no selection writes `0x03` to the input pipe. ConPTY turns it into
  `CTRL_C_EVENT` for every console program attached to that console. With a selection, Ctrl+C
  copies and the selection is cleared (as Windows Terminal does). Ctrl+Shift+C always copies.
- **Ctrl+Break** has no byte. I believe ConPTY maps the win32-input-mode key event for Break to
  `CTRL_BREAK_EVENT`; not verified. Phase 0 checks it.
- **Closing the window**: `ClosePseudoConsole`. It sends `CTRL_CLOSE_EVENT` to every program attached;
  Windows ends any that does not exit within about 5 seconds. **`ClosePseudoConsole` can block
  until the output pipe is drained**, so the reader thread keeps reading until it gets end of
  file, and the close is made from a thread that is not the window's. A well-known deadlock in
  other terminals; it is a test here.
- **A job object per session.** The shell is put in it suspended, before it can start anything, so
  every console program it starts is in the job too. The job is **not** "kill on close": a GUI
  program started from the shell (`notepad`, `code .`) must outlive the window, as it does in
  Windows Terminal. The job is for counting: before a window with programs still running is closed,
  `QueryInformationJobObject` lists them, and the close asks "2 programs are still running: cargo,
  rustc. Close anyway?" (one setting turns the question off). Console programs that ignore the
  close event are ended by Windows; GUI ones live on.
- **Closing the window** closes its session this way, then the window. **The terminal crashing**
  (a panic aborts, rule 11) closes our ends of the pipes and the pseudoconsole handle with the
  process, so ConPTY sends the same close event. Nothing is orphaned that would not be orphaned in
  Windows Terminal.
- **The shell exiting** (`exit`): the reader gets end of file, the window shows "Process exited with
  code 0" and closes itself when the code is 0, stays open with the message otherwise (so a
  failing ssh can be read). No setting.

### 3.5 Linux and the Mac, later

`openpty` (or `posix_openpt`) and `fork`/`exec` through `libc`, already in `Cargo.lock`;
`TIOCSWINSZ` to resize; `SIGHUP` to the process group on close, `SIGKILL` after 2 seconds; the
shell from `$SHELL`, as a login shell on the Mac. About 250 lines. A reader thread, as on Windows.
No ConPTY quirks: the bytes are the program's own.

---

## 4. The VT parser and the screen

### 4.1 What is understood

The model is xterm as most programs use it, which is what Windows Terminal, Alacritty and WezTerm
answer to. The first release understands:

| Kind | Sequences |
|---|---|
| C0 controls | BEL, BS, HT, LF, VT, FF, CR, SO/SI (character sets) |
| Cursor | CUU, CUD, CUF, CUB, CNL, CPL, CHA, CUP, HVP, VPA, HPA, DECSC/DECRC, `CSI s`/`CSI u`, DECSCUSR (cursor shape) |
| Editing | ED, EL, ECH, ICH, DCH, IL, DL, SU, SD, REP, IND, NEL, RI, HTS, TBC, CHT, CBT |
| Scroll region | DECSTBM. Left and right margins (DECSLRM) are out: almost nothing uses them |
| Text attributes (SGR) | Bold, faint, italic, underline (single, double, curly, dotted, dashed, by `4:x`), blink (drawn as bold), inverse, hidden, strike, overline; 16, 256 and 24-bit colour for text, background and underline (`38`, `48`, `58`, both `;` and `:` forms) |
| Modes (DECSET/DECRST) | 1 DECCKM, 6 DECOM, 7 DECAWM, 12 cursor blink, 25 DECTCEM, 47/1047/1049 alternate screen, 1000/1002/1003 mouse, 1004 focus, 1006 SGR mouse, 2004 bracketed paste, 2026 synchronized output, 9001 win32-input-mode; IRM (4) and LNM (20) |
| Character sets | G0/G1 with ASCII and DEC Special Graphics (line drawing in old programs) |
| Replies | DA1, DA2, DSR 5 and 6 (CPR), DECRQM for the modes above, XTVERSION, OSC 10/11/12 colour queries |
| OSC | 0 and 2 (title), 4 (set or query a palette colour), 7 (folder), 8 (hyperlinks), 9;9 (Windows Terminal's folder), 10/11/12 (colours), 52 (clipboard, write only), 104/110/111/112 (reset colours), 133 (command marks: recorded, not used until the blocks) |
| Reset | RIS, DECSTR |

**Out of the first release, on purpose:** sixel, kitty and iTerm images; DECSLRM; DECDHL/DECDWL
(double-size lines, drawn as normal); the VT52 mode; 8-bit C1 controls (read as UTF-8 instead);
DECRQSS beyond SGR and DECSCUSR; XTGETTCAP; title stacks; the kitty keyboard protocol (7.1).

**OSC 52 reads are refused.** A program may put text on the clipboard (so vim over ssh can copy);
no program may read it, because anything that prints a file could otherwise read your clipboard.
Writes over 1 MB are refused.

**Hostile output.** `cat` of a hostile file must not do more than draw. Nothing in the list runs
a command, opens a file or a link without a click, or answers with data the program did not have.
The title is shown, never executed. A link opens only on Ctrl+click, and its address is shown on
hover first.

### 4.2 The screen and the cells

A cell is **16 bytes**:

| Field | Bytes | |
|---|---|---|
| Text | 4 | A `char`, or an index into the session's table of grapheme clusters when the cell holds more than one code point (an emoji with a skin tone, a letter with marks) |
| Text colour | 4 | A tag and 24 bits: default, palette index, or RGB |
| Background | 4 | The same |
| Attributes | 2 | Bold, italic, underline kind, inverse, strike, hidden, faint, overline, wide, wide spacer, wrapped |
| Link and underline colour | 2 | An index into the session's table of hyperlinks and underline colours; 0 for none |

The live screen at 200 x 60 is 192 KB. The alternate screen is a second one, made when a program
first asks for it and freed when it leaves.

**Wide characters.** Width comes from `unicode-width` 0.2, already in `Cargo.lock`. A wide
character takes its cell and a "spacer" cell after it. **Graphemes** are cut with
`unicode-segmentation`, already in `Cargo.lock`: a combining mark joins the cell before it; an
emoji sequence joined by ZWJ is one cluster of width 2.

**The width problem.** The program, ConPTY and we each decide how wide a character is. When two of
them disagree, the cursor drifts and lines are drawn over each other. There is no full fix in any
terminal. The plan: follow ConPTY on Windows (it is the one that moves the cursor we are told
about), keep the width table in one function, and keep recorded tests of the usual offenders
(emoji with variation selector 16, CJK, Powerline symbols). Which table ConPTY uses on build 26300
is a phase 0 check.

### 4.3 Scrollback

Lines that scroll off the top go into the scrollback, **compressed**: the text as UTF-8, plus one
8-byte run for each change of attributes, plus a 16-byte head. A typical 80-column line of
coloured log output is about 100 to 150 bytes, against 1,280 as cells. So:

| | Lines | Memory |
|---|---|---|
| Default | 10,000 | about 1.5 MB typical; at most 24 MB when every cell of 200 columns has its own colour |
| Largest the setting allows | 100,000 | about 15 MB typical |
| Hard cap, whatever the lines | 64 MB per session | The oldest lines go first |

Scrollback lines are stored **unwrapped**: one logical line, however many rows it took. Resizing
the window then costs nothing for the scrollback: the row it is drawn on is computed when it is
drawn, and a small index of row starts is rebuilt lazily for the part that is on screen or being
searched.

**Reflow on resize.** The live screen is rewrapped by its soft-wrap marks when the width changes,
as Windows Terminal and Alacritty do. On Windows, ConPTY then repaints the screen from its own
buffer; the two must agree, and phase 0 sees whether they do. If they do not, the fallback is what
early Windows Terminal did: trust ConPTY's repaint for the live screen and reflow only the
scrollback.

### 4.4 Write the parser, or take one: the crates priced

Rule 7: nothing is added without asking. These were priced for question 6; you chose `vte`. Figures
are from a scratch package: "new in the lock" counts the packages that this workspace's
`Cargo.lock` does not already have; "exe" is a release build that feeds a file through the crate,
against the same program without it (134,656 bytes).

| Crate | Gives | New in the lock | Exe | Licence | Notes |
|---|---|---|---|---|---|
| `vte` 0.15 | The parser only: bytes to "print", "execute", "CSI", "OSC", "ESC" calls | 2 (`vte`, `arrayvec`) | +10 KiB | MIT or Apache-2.0 | Alacritty's own parser, small and fuzzed for years. Our code does every sequence's meaning |
| `alacritty_terminal` 0.26 | Parser, screen, scrollback, reflow, selection, regex search, and its own PTY and event loop | 7 (`alacritty_terminal`, `vte`, `arrayvec`, `home`, `miow`, `rustix-openpty`, `signal-hook`) | +145 KiB | Apache-2.0 | Its cells are its own shape (about 24 bytes) and its scrollback is cells; its PTY and threads would be unused but compiled; API changes with Alacritty's needs |
| `termwiz` 0.23 (WezTerm) | Parser, surface, a terminal model | 47 | not built | MIT | Brings `pest`, `phf`, `nix`, `winapi`, `terminfo`. Too heavy |
| `wezterm-term` | WezTerm's full terminal model | not on crates.io | | MIT | Only from WezTerm's repository by git. Ruled out |
| `portable-pty` 0.9 | PTY on every system, ConPTY on Windows | 11 (with `winapi`, `nix`, `anyhow`, `serial2`) | not built | MIT | No job objects; its Windows code is what we would write ourselves |
| `conpty` 0.7 | A thin ConPTY wrapper | 1 (it uses the `windows` crate, already in the lock) | not built | MIT | Saves perhaps a day; hides the handles the job object and the close need |

**Recommended: `vte` for the parser, everything else our own.**

- The parser is the part with the most ways to be subtly wrong, the least to design, and the
  most years of fuzzing behind `vte`. 2 packages and 10 KiB.
- The screen is where this terminal's budgets live: 16-byte cells, compressed scrollback, the
  damage our drawing needs (section 6), the OSC 133 marks the blocks will need. `alacritty_terminal`
  would make every one of those Alacritty's choice, and its scrollback of cells would cost about
  ten times the memory of section 4.3.
- The PTY layer is small, and the parts that matter (job object, the close, win32-input-mode) are
  exactly the ones the crates do not do.

What it costs: the screen is about 4,000 lines of our own, with its tests (section 10, phase 1).
`alacritty_terminal` would save perhaps 5 to 7 days of that. If you would rather spend the
memory than the days, it was the one alternative worth taking. You chose `vte`.

**A second use of `alacritty_terminal`**, as a test-only oracle fed the same random bytes, was
offered and not taken. The random-bytes test (9.2) checks invariants instead.

---

## 5. What goes where

Rule 8: "whatever can be shared will start shared". The test applied to each piece is the one
`docs/ai-library-plan.md` §7 used: could a second app of the family use it, today or in an app
already on the list in `docs/ecosystem-ideas.md`?

| Piece | Where | Why |
|---|---|---|
| The window loop, regions, damage, timers | Copied from mdview's `app.rs` (2,063 lines) into `mdterm` | `ecosystem-ideas.md`: the third app copies the loop knowingly; the shared frame waits for a fifth |
| Title row, menus, command palette, settings form, text field, notice bars, toasts, scrollbar, corners, frost, snap, shadow | `mdshared-ui`, used as they are | Already shared |
| Fonts, glyph rasterizing and cache | `mdshared-text`, used as it is, plus one new entry point (below) | Already shared |
| Settings file, single instance, crash reports, updater, paths | `mdshared-app`, used as they are | Already shared |
| **New: a fixed-advance glyph path** — draw one glyph by id at a pixel position, with a cmap lookup cache for ASCII, no parley layout | `mdshared-text`, as a new function beside `draw_layout` | mdedit's text view is monospace too and could use it; nothing existing changes |
| **New: box drawing, blocks and Powerline shapes drawn as geometry** (U+2500 to U+259F, U+E0B0 to U+E0B3) | `mdshared-ui` (`gfx`) as a new function | So lines join exactly whatever the font; mdview and mdedit draw these characters from the font today and could opt in |
| **New: the 16 ANSI colours per theme** | `mdshared-ui::theme`, a new field set beside `Palette`, read only by callers that ask | mdview colours ANSI in code blocks of terminal output if it ever wants to |
| VT parsing glue, the screen, scrollback, selection, search | `crates/mdterm-core` in the terminal's workspace: headless, no window, no threads | Only a terminal needs it |
| ConPTY and openpty, the job object, the shell list | `crates/mdterm-pty` | Only a terminal needs it. If mdedit ever runs a program and shows its output, it would move to the shared crates then |
| Key encoding, mouse reporting | `mdterm-core` | Only a terminal |

**Where the code is: a fourth repository**, `Kaan88/markdown-terminal-rs`, beside the other
three, with the same `CLAUDE.md` rules, CI and release workflows copied from this one. Not inside
mdedit's workspace: a terminal commit should not move mdedit's lock or CI. Question 1.

Each shared change follows `docs/CODE.md`: a variant beside the old function, both apps'
`tests/shots.sh` before and after, push, then move each app's `rev` on purpose.

---

## 6. Drawing on winit and softbuffer

### 6.1 The cell grid on the CPU

- **Cell size.** Whole pixels: the font's advance rounded, and the line height from the font's
  metrics. The mock's numbers show why rounding matters: Consolas at 11 px advances 6.445 px, so
  188 columns of 6 px and 188 of 7 px differ by 100 px at 1280 wide. Rounding to whole pixels is
  what every terminal does; glyphs are drawn centred in their cell.
- **The font.** The same list as mdedit: `Consolas, Cascadia Mono, Noto Sans Mono, DejaVu Sans
  Mono, monospace`, with mdedit's emoji fallbacks. One font, one size, one setting each.
- **Ligatures: off, and no setting** (section 8.3). With ligatures every run has to be shaped,
  which is the most expensive thing on the draw path; without them, a cell is one glyph found by
  a table lookup.
- **The fast path.** A cell with a plain character looks its glyph id up in a per-font table
  (filled on first use), and blits the cached raster from `mdshared-text`'s byte-capped cache. A
  cell holding a cluster, or a character the font lacks, goes through parley once and the result
  is cached by the cluster's text.
- **Subpixel text** as in mdedit (RGB masks by default, greyscale where the setting says so).
- **Underlines, strike and the cursor** are rectangles; curly underline is a short cached strip.

### 6.2 Damage, and frames while output streams

- The reader thread parses into the screen under a lock and marks **dirty rows** in a bit set. It
  wakes the window only if no wake is pending.
- The window draws at most one frame per display refresh, and only the dirty rows. A row is drawn
  whole: background runs first, then glyphs.
- **Scrolling is a copy, not a redraw.** When the screen scrolls by n rows and nothing else in
  those rows changed, the frame buffer is moved up by n rows (one `copy_within` of
  `width x rows x 4` bytes) and only the new rows are drawn. `yes` and `cat` scroll every frame;
  this is what keeps them under the frame budget.
- **While output streams**, the parser does not wait for the window. If bytes keep coming, the
  window draws at the display's rate whatever it finds, and every byte is parsed: nothing is
  dropped, but intermediate screens are never drawn. 100 MB of `cat` costs the parse and about
  sixty frames a second, not 100 MB of drawing.
- **Synchronized output** (`?2026`): while a program holds it, nothing is drawn, up to 150 ms.
  Programs that redraw the whole screen (vim, htop) then never tear.
- **Presentation** goes through softbuffer's damage rectangles, as mdedit does.

### 6.3 The numbers it has to hit

A frame of 172 x 40 cells (1280 x 800 at 12 px) all dirty is 6,880 cells: on mdedit's figures for
glyph blits, that is about 2 to 4 ms on this PC. A scroll frame is a memory move of about 4 MB and
one row: under 1 ms. Both are estimates from mdedit's measured scroll frame (p50 0.44 ms), not
measurements of this code.

### 6.4 Latency

A key is written to the pipe on the key event, before anything is drawn. The echo comes back
through ConPTY; §12 of `ecosystem-ideas.md` put that at about 5 to 15 ms, not verified. Our own
part, from the reader waking to the frame presented, must stay under 8 ms. Both are measured and
reported apart (section 9.4).

### 6.5 Idle

- The reader thread blocks in `ReadFile` / `read`. The writer thread blocks on its queue.
- The cursor blinks, as mdedit's caret does, at 530 ms, by redrawing one cell. It **stops blinking
  after 15 seconds without a key** and stays solid, so a window left open costs nothing. An
  unfocused window does not blink at all.
- Text with the "blink" attribute is drawn bold, never blinking.
- The bell flashes the title row once (visual), or does nothing; never a sound by default.

---

## 7. Input

### 7.1 Keys

- **xterm encoding**: printable text as UTF-8; Enter `CR`; Backspace `DEL`; Tab, Shift+Tab
  (`CSI Z`); arrows, Home, End, Insert, Delete, Page Up and Down, F1 to F12 with xterm's modifier
  forms (`CSI 1;5A` for Ctrl+Up); cursor keys in application mode when DECCKM is set; the keypad in
  application mode when DECKPAM is set.
- **Alt** sends ESC before the key. **AltGr** (right Alt on Turkish and other layouts) types what
  the layout says and never sends ESC: winit reports the text the layout makes, and that text wins.
- **Ctrl** with a letter sends the control code; Ctrl+Space sends NUL; Ctrl+[ ESC.
- **win32-input-mode** (`?9001`), on Windows: when ConPTY asks for it, every key goes as a full
  Windows key event (`CSI Vk;Sc;Uc;Kd;Cs;Rc _`). Windows console programs (PSReadLine, far,
  older editors) then see exactly what they would in a console window, Ctrl+Shift combinations
  included. This is what makes PowerShell feel native.
- **The kitty keyboard protocol: not in the first release.** It matters to a few programs
  (recent neovim, helix, fish), and on Windows ConPTY sits in between. Later, if you miss it.
- **The terminal's own shortcuts** are few and use Ctrl+Shift so programs keep everything else:
  Ctrl+Shift+N new window, Ctrl+Shift+W close the window,
  Ctrl+Shift+C / V copy and paste, Ctrl+Shift+F find, Ctrl+Shift+P command palette, Ctrl+= / Ctrl+-
  / Ctrl+0 size, Shift+Page Up/Down scroll, Ctrl+, settings. Ctrl+C copies only when there is a
  selection; Ctrl+V pastes (as Windows Terminal), and Ctrl+Shift+V always pastes. No key-binding
  editor.

### 7.2 IME

winit's `Ime` events, as mdedit already handles them: the text being composed is drawn underlined
at the cursor (`mdshared-ui`'s preedit drawing), the candidate window is placed at the cursor cell,
and the committed text is written to the shell as typed text. Turkish needs no IME; the test is
Japanese and Chinese input in pwsh.

### 7.3 Mouse, selection, copy and paste

- **Selection**: drag selects; double-click a word (with a fixed set of word characters that keeps
  paths and URLs whole); triple-click a line (the logical line, through soft wraps); Alt+drag a
  block. Selecting near the top or bottom edge scrolls.
- **Copy** joins soft-wrapped rows into one line, trims trailing spaces, and puts plain text on
  the clipboard. Copy on select: off by default, one setting.
- **When a program has turned on mouse reporting** (vim, htop), clicks go to it, in SGR form
  (1006) or the old form. Holding **Shift** always selects instead. The wheel scrolls the
  program when it reports the mouse, the scrollback otherwise, and on the alternate screen sends
  arrow keys when the program has not asked for the mouse (as xterm's "alternate scroll").
- **Paste**: with bracketed paste on, the text goes wrapped in `ESC[200~ ... ESC[201~`, with any
  `ESC[201~` inside it removed. Without it, a paste of more than one line asks "Paste 12 lines?"
  first, because each line runs as a command. One setting turns the question off. Line endings
  become `CR`.
- **Right-click**: a menu (Copy, Paste, Select all, Find, Clear scrollback), drawn with the shared
  `Menu`. Not "right-click pastes": that is the old console's habit and a setting nobody needs.

### 7.4 Links

OSC 8 hyperlinks are underlined on hover, show their address in a tooltip, and open on Ctrl+click
through the shell (`ShellExecuteW`), as mdedit opens links. Plain `http://` and `https://` text is
found the same way on hover, in the row under the pointer only, never by scanning the screen. File
paths are not detected in the first release.

### 7.5 Search in scrollback

Ctrl+Shift+F opens a find bar at the top of the grid, with the shared text field, as mdedit's. It
finds plain text, case-insensitive unless the query has a capital, over the scrollback and the
screen, from the bottom up. Matches are tinted, the current one stronger, with "3 of 41" beside
the field; Enter and Shift+Enter move between them. It runs on the window thread in slices of
5,000 lines per frame so a 100,000-line scrollback never holds a frame. Regular expressions: not
in the first release (the `regex` crate is already in the family, so it is cheap later).

---

## 8. The window, settings, themes

### 8.1 The main window

One window, one shell: the shared title row holding the window's title (the program's title, or
the shell's name) where mdedit has its tabs, and the menu button; the grid below it filling the
window; **no command bar and no status strip**. A terminal's space is the grid. The mock's status
strip and the blocks' chrome belong to the block phase. A notice bar appears only when there is
something to say (a program exited with an error, an update is ready).

A scrollbar at the right, thin and drawn only while scrolling or hovered, as in mdedit.

Padding: 8 px around the grid, filled with the background colour, so the cells do not touch the
window's edge.

### 8.2 No tabs, and splits perhaps later

**No tabs in the first release** (your answer). Another shell is another window: Ctrl+Shift+N
opens one with the default shell in this window's folder; the menu's "New window with" lists the
shells found (3.2). Windows' own taskbar and Alt+Tab do the switching. Each window is its own
process: one crash ends one window, and nothing is shared between them but the settings file.

**Splits, perhaps.** Not in the first release. The code is built so they can come without a
rewrite: a window holds a list of **sessions** (pseudoconsole, screen, reader and writer threads,
job object) and draws one, so a split is two sessions drawn side by side, and tabs, if ever
wanted, are the same list drawn as a strip. A split is about 5 to 6 days later, with mdedit's
split view as the model for the divider and focus.

**No single instance.** Starting `mdterm` again opens a new window, as a new process. The shared
`single.rs` is not used: one process per window is simpler and safer for a program whose input
is anything from outside.

### 8.3 Settings

The shared settings form, in the category design mdedit uses (`docs/archive/settings-redesign.md`). The
whole list:

| Category | Setting | Default |
|---|---|---|
| Shell | Default shell (the ones found, and "Custom") | PowerShell 7, else Windows PowerShell |
| | Custom command line | empty |
| | Starting folder | Home |
| | Ask before closing a window with programs running | on |
| | Ask before pasting more than one line | on |
| Appearance | Theme (shared: follow the system, light, dark) | Follow the system |
| | Font, font size | Consolas, 12 |
| | Cursor (block, bar, underline), blink | Block, on |
| | Bell (flash, off) | Flash |
| Scrolling | Scrollback lines (1,000 to 100,000) | 10,000 |
| | Copy on select | off |
| From the shared crates | Frosted window, rounded corners, subpixel text, check for updates | As in mdedit |

That is fourteen rows. Anything more needs a reason you give.

### 8.4 The one offer

If the shell gives no folder (3.3) and the user opens a second window, a notice bar offers once:
"New windows can start in this window's folder if PowerShell reports it. Add one line to your profile?"
with Show the line / Add it / Not again. Adding it appends one commented line to `$PROFILE`; it
is never done unasked. The same mechanism carries the OSC 133 lines when the blocks come.

### 8.5 Themes

The shared palette for the chrome, so the terminal matches mdedit and mdview in both themes.
The grid's 16 ANSI colours are new: one set for dark and one for light, defined once in
`mdshared-ui::theme` (section 5) and looked at in shots. **A light terminal is harder than a light
editor**: programs assume yellow and bright white are readable, and on a white page they are not.
The light set darkens yellow and maps "bright white" text to the page's text colour, as Windows
Terminal's light schemes do. Both themes ship (your answer to question 5).

Programs can still set colours with OSC 4/10/11; a reset puts the theme's back.

---

## 9. Testing

Rule 2: a test per behaviour, failing with its fix reverted. Rule 11: no input can panic.

### 9.1 The screen, headless

`mdterm-core` has no window and no PTY, so almost everything is tested by feeding bytes and
reading the screen back: text, attributes per cell, cursor, modes, scrollback, and the replies
the terminal would send.

- **One test per sequence** of section 4.1, with its edge cases (counts of 0, out-of-range
  positions, the scroll region at the edges, a wide character at the last column).
- **Recorded transcripts**: real output captured once from real programs, on this PC and in WSL —
  pwsh with PSReadLine, `git log --graph --color`, `cargo build` with an error, vim opening and
  quitting, htop for two seconds, `ls --color`, a resize in the middle of each — saved as byte
  files in `fixtures/` with the expected screen as text and attribute dumps. A change that alters
  a dump is looked at, not re-recorded blindly.
- **vttest**: run by hand in WSL in the terminal at each phase end, every menu, and the screens
  that pass and fail written in `docs/STATE.md`. It is interactive and cannot run in CI.
- **esctest** (iTerm2's automated suite, Python, driving the terminal through its replies): run
  in WSL from phase 6, by hand, with its results kept. Its expectations are xterm's; failures we
  choose to keep are listed, not hidden.

### 9.2 No panic

- A **random-bytes test** in the normal suite, with its own small seeded generator (no new
  crate): 10 MB of random bytes and 10 MB of random *valid-looking* sequences (random CSI
  parameters, huge counts, nested OSC, truncated UTF-8), fed at random chunk sizes; after each
  chunk the screen's invariants are checked (cursor inside the grid, no spacer without its wide
  cell, scrollback within its cap). Runs in CI on every push.
- **cargo-fuzz** on the parser glue and the screen, on Linux in CI, nightly once a week. Needs
  `cargo-fuzz` and a nightly toolchain on the runner, not in the lock.
- The arithmetic of sizes and counts is saturating everywhere a count comes from outside;
  clippy's `arithmetic_side_effects` is turned on for `mdterm-core`.

### 9.3 Shots

`tests/shots.sh` as in the other two, both themes: the window empty; pwsh with a prompt and
coloured output; every SGR attribute and the 256 colours; truecolor gradient; box drawing and
Powerline symbols; wide characters, emoji and combining marks; the alternate screen (a recorded
htop); a selection; the find bar with matches; a link hovered; the paste question; the close
question; the "New window with" menu open; settings, each category; the
notice offer. Each shot is drawn from a **recorded transcript fed through `--shot`**, not a live
shell, so it is the same on every run and in CI. Rule 6: you approve them by looking; each is
listed by what it shows.

### 9.4 Budgets

Measured on this PC with release builds, the method of `docs/CODE.md`:

- First frame and memory: the existing method, 15 runs, medians.
- "Prompt visible": time from start to the first byte from the shell, reported, not budgeted.
- Keystroke: a debug counter from key event to present, p50 and p95 over 1,000 keys, and the echo
  delay through ConPTY (time from pipe write to the first byte back) reported apart.
- Throughput: `mdterm-core`'s bench over the recorded transcripts and a 100 MB file, MB/s; end
  to end, `type` of the same file in mdterm and in Windows Terminal, wall clock, three runs each.
- Frame while streaming: the worst frame during that `type`.
- Idle CPU: Process Explorer's cycles for one minute, focused and unfocused, after the blink has
  stopped.

Rule 9 applies to the terminal from its first day: development builds use a scratch settings file
(`MDTERM_SETTINGS`) and never the owner's.

---

## 10. The phases

Days are working days of one agent; a person-week is 5. "Done" means its tests pass, its shots
are taken and looked at, its budgets measured, and `docs/STATE.md` says so.

| Phase | What | Days |
|---|---|---|
| **0. Spike** | Throwaway, in `spikes/`: ConPTY with pwsh, `vte` printing to the console. Answers the phase 0 checks of section 3.1 and 3.4: passthrough of OSC 8/52/133/7 and `?2026`, the resize repaint, ConPTY's width table, its throughput and echo delay, Ctrl+Break, the close deadlock. Writes them down. **A decision point**: if ConPTY's resize or throughput rules out the plan, it says so here | 3 |
| **1. The screen** | `mdterm-core`: cells, the live and alternate screens, every sequence of 4.1, replies, character sets, wide and clustered cells, compressed scrollback, reflow, OSC 133 marks recorded. The random-bytes test. Transcript tests from the spike's recordings | 10 to 12 |
| **2. The PTY** | `mdterm-pty`: ConPTY, the reader and writer threads, the job object and the close, resize, the shell list, environment and folder | 4 to 5 |
| **3. The window** | The loop copied from mdview; the title row; the glyph fast path and the cell size; damage, scroll by copy, frame pacing, synchronized output; cursor and blink; box drawing; IME preedit; the bell. The new shared functions of section 5, with both apps' shots before and after | 8 to 10 |
| **4. Input** | Key encoding with win32-input-mode; mouse reporting; selection, copy, paste and its question; links; the right-click menu | 4 to 5 |
| **5. Find, settings, themes** | The "New window with" menu; find in scrollback; the settings form; the ANSI colours per theme; the folder offer | 3 to 4 |
| **6. Hardening and the release** | esctest and vttest passes and fixes; the budgets measured and written; the installer, zip, updater and manifest copied from mdedit's packaging; CI with shots on Windows | 5 to 6 |
| **First release, Windows** | | **37 to 45 days, about 7.5 to 9 person-weeks** |
| 7. Linux and the Mac | openpty, signals, `$SHELL`; the shots on both in CI; packages | 6 to 8 |
| 8. Later, each only if asked | Splits, the one you said maybe to (5 to 6, with the session type already there); tabs (3); the blocks of `docs/ideas/terminal.md` (15 to 30 days); kitty keyboard (2 to 3); regular expressions in find (1) | |

This is close to `ecosystem-ideas.md` §12's 7 to 11 person-weeks for the plain terminal. It is at
the low end because the scope is narrower than §12's (no splits, few settings, no graphics) and
higher than a toy because of the phase 6 work every terminal needs.

### The first useful slice

**Phases 0 to 3, and the copy and paste of phase 4: about 27 to 32 days.** One window,
PowerShell 7, colours and attributes right, scrollback with the wheel, select and copy, paste,
win32-input-mode so PSReadLine works, resize, both themes. Enough to run git, cargo and ssh in it
all day and find what is wrong. Find and settings come after you have used it.

---

## 11. Risks

| Risk | Why it is real | What the plan does |
|---|---|---|
| **ConPTY's resize repaint** | ConPTY keeps its own screen and repaints after a resize; when its wrapping differs from ours, lines are doubled or lost. Windows Terminal has fought this for years | Phase 0 looks at it first. Fallback: ConPTY's repaint is trusted for the live screen (4.3). Last resort: ship the newer `OpenConsole.exe` (decision 13.7) |
| **Character widths** | The program, ConPTY and we each decide widths; a disagreement drifts the cursor | One width function, ConPTY's table followed on Windows, recorded tests of the usual offenders (4.2) |
| **ConPTY throughput** | Every byte goes through conhost's renderer; "no slower than Windows Terminal" may be the best possible | The end-to-end budget is relative, not absolute (section 2) |
| **The long tail** | Each program uses a sequence nobody else does | vttest and esctest, transcripts of the programs you use; anything else is fixed when you meet it, with a transcript test |
| **A panic aborts the window** (rule 11) | Every byte is from outside | Saturating arithmetic, the random-bytes test in every CI run, cargo-fuzz weekly |
| **A third app to keep** | Installers, manifests, updates, CI, pins to move for every shared change. `ecosystem-ideas.md` warns about the count of binaries | Copy, do not invent: packaging and CI copied from mdedit's |
| **Light theme** | Programs assume a dark background | A tuned light ANSI set; question 5 |
| **Estimates** | Nothing here has been built; ConPTY is the unknown | Phase 0 is three days so the estimate can be redone after it |

---

## 12. Questions, and your answers

| # | Question | Your answer (2026-11-22) |
|---|---|---|
| 1 | Its own app, `mdterm`, in its own repository `markdown-terminal-rs`? | Yes |
| 2 | Tabs in the first release; splits never? | No tabs in the first release; splits maybe |
| 3 | PowerShell 7 by default, Windows PowerShell when it is missing? | "I think so": yes |
| 4 | A third app now, before the mail app the ecosystem review ranked first? | You never asked for mail: the terminal is next |
| 5 | A light terminal at all? | Yes: the family has both themes |
| 6 | `vte`, or `alacritty_terminal`; and `alacritty_terminal` as a test oracle? | `vte`. The oracle is not taken |
| 7 | Bundle Windows Terminal's `OpenConsole.exe` and `conpty.dll` if phase 0 needs it? | "If you think so": decided by phase 0's findings, see 13.7 |

**Question 4, settled.** It came from `docs/ecosystem-ideas.md`, which ranked possible apps and put
a mail app first. You never asked for mail. So nothing comes before the terminal, and the order
of that review binds nothing here. The standing cost of a third exe (its installer, updates, CI,
and one more pin to move per shared change) is still real, and is kept down by copying mdedit's
packaging and CI rather than inventing new ones.

## 13. Decisions

Confirmed by your answers, or following from them. They go into a decisions log in the
terminal's repository when it is made, with one line pointing there in mdedit's
`docs/DECISIONS.md`.

1. The terminal is its own app, `mdterm`, in `Kaan88/markdown-terminal-rs`, on the shared crates;
   not a pane in mdedit. mdedit's "no terminal" stands.
2. The first release is a standard terminal: one window, one shell, about a dozen settings. No
   tabs; another shell is another window. Splits may come later; the session type is built so
   they need no rewrite. The blocks of `docs/ideas/terminal.md` are a later phase; OSC 133 marks are
   recorded from the start.
3. Windows first, on the system's ConPTY through `windows-sys`; Linux and the Mac after the
   Windows release. PowerShell 7 is the default shell, Windows PowerShell when it is missing.
4. `vte` parses (approved: 2 packages, +10 KiB); the screen, scrollback and PTY are our own.
   `alacritty_terminal` is not used, not even in tests.
5. Cells are 16 bytes; scrollback is stored unwrapped and compressed, 10,000 lines by default,
   64 MB per session at most.
6. A job object per session counts the programs still running; it does not kill GUI programs on
   close.
7. The system's ConPTY is used. Windows Terminal's `OpenConsole.exe` and `conpty.dll` are bundled
   only if phase 0 finds a fault on build 26300 that only they fix (the resize repaint, or a
   sequence the system's drops that a listed program needs); phase 0's report says which, and
   that is recorded as its own decision.
8. win32-input-mode is supported; the kitty keyboard protocol is not, at first.
9. OSC 52 may write the clipboard, never read it.
10. No ligatures, no graphics protocols, no tabs, no splits, no key-binding editor in the first
    release.
11. The cursor stops blinking after 15 seconds without a key; idle is zero CPU.
12. Light and dark themes, each with its own 16 ANSI colours.
13. New shared code: a fixed-advance glyph path in `mdshared-text`, geometric box drawing in
    `mdshared-ui`, the ANSI colours in `mdshared-ui::theme`, each opt-in.
14. Shots are drawn from recorded transcripts, never from a live shell.
