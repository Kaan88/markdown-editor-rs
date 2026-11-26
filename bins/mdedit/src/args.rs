//! Command-line arguments: files to open, start-up overrides, and the flags that render a UI state
//! to a PNG without a window (`--shot`) or print timings (`--trace`, `--scroll-steps`). The flags
//! and their meaning are fixed in `docs/DESIGN.md`, section "Command line".

use crate::commands::CommandId;
use mdshared_app::sister::{Rect, Request};
use mdshared_ui::corners::Corners;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShotMenu {
    Macro,
    Eol,
    Encoding,
    View,
    Main,
    /// The main menu scrolled to its last row (visible only when the window is too short for it).
    MainEnd,
    Recent,
    /// The main menu with its View submenu open beside it.
    MainView,
    /// The main menu with its Macro submenu open beside it.
    MainMacro,
    Tab,
    /// The text area's right-click menu.
    Text,
    Find,
    /// The minimap's right-click menu (needs `--minimap`).
    Minimap,
    /// The Macro menu while a macro is being recorded.
    MacroRec,
    /// The completion list at the caret (`--goto` puts the caret after the letters typed).
    Complete,
    /// The same, with its first snippet taken: the text it made, its first stop selected.
    CompleteSnippet,
    /// The text menu with its Lines, Convert, Markdown (or AI) submenu open beside it.
    TextLines,
    TextConvert,
    TextMarkdown,
    TextAi,
    /// The language list above the status bar's language field.
    Language,
    /// The main menu with its notes library submenu open beside it, the list of notes, and the
    /// list of places a note can be put (the library folder is the one the settings folder has).
    MainLibrary,
    Library,
    LibraryAdd,
    /// The notes library in the left pane (`--files` is implied).
    LibraryPane,
    /// The notes library pane with a right-click menu open: on the first note, on the first
    /// section, in the empty space below the rows; a name being typed over the first note; and
    /// the question about removing the first section.
    LibraryRow,
    /// The notes library pane with the keyboard in it, the cursor on the note under "Money".
    LibraryCursor,
    LibrarySection,
    LibraryEmpty,
    LibraryRename,
    /// The box of a new group, with a name typed in it.
    LibraryNewGroup,
    LibraryRemove,
    /// The active document made Markdown as the AI's Format as Markdown does, with the bar
    /// offering its `.md` name.
    RenameBar,
    CrashBar,
    StaleBar,
    /// The menu of the Preview button's arrow, drawn as it is when mdview is found.
    Preview,
    /// The AI menu under the bar's AI edit button (needs `--pref ai_connection=…`).
    BarAi,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShotDialog {
    Macros,
    Settings,
    About,
    Shortcuts,
    Recent,
    /// The "Save changes?" question for the active file.
    Unsaved,
    /// "AI runs…", the question before undoing a run, and the question when the run's source is
    /// in the Recycle Bin, with runs that were put there for the shot.
    Runs,
    RunUndo,
    RunUndoRecycled,
    /// "AI runs…" with record files a later version wrote, listed with nothing to press.
    RunsNewer,
    /// The question when the original the notes were cut from is gone: what undo says instead.
    RunUndoOriginal,
    /// The question for a run that died before its notes, some of which a later run wrote: they
    /// stay (review of the audit 4 fixes, S2).
    RunUndoLater,
    /// The report after that undo: what the later run wrote is listed as kept, with why.
    RunUndoneLater,
    /// The question before undoing a "Move them in" run: each file it puts back, by name.
    RunUndoMoved,
    /// The report of that undo: what was put back, what was kept and why.
    RunUndoneMoved,
    /// The question before undoing a run of Write notes: its notes and the copies of its
    /// pictures go, with their lines (pictures plan, phase V5).
    RunUndoPictures,
    /// The report of that undo when a note changed since is kept: the copy it shows stays with
    /// it, said as kept for that note.
    RunUndonePictures,
    /// The question before a library file is deleted.
    LibraryDelete,
    /// The same question with a group and a file long enough to wrap onto a fourth line.
    LibraryDeleteLong,
    /// The question before a section is taken out, its name long enough to wrap onto a fourth
    /// line (audit 4, F19).
    LibraryRemoveLong,
}

/// A line and column as the user types them: 1-based, `12` or `12:5`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineCol {
    pub line: usize,
    /// 1 when not given.
    pub col: usize,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Args {
    pub files: Vec<PathBuf>,
    /// `--light` (false) or `--dark` (true).
    pub dark: Option<bool>,
    /// `--folder <dir>`: the Files pane's folder.
    pub folder: Option<PathBuf>,
    /// `--files` / `--no-files`: show or hide the Files pane.
    pub files_pane: Option<bool>,
    /// Do not read or write the user's session, and skip the single-instance hand-over.
    pub no_session: bool,
    /// `--shot <out.png>`.
    pub shot: Option<PathBuf>,
    /// `--size WxH`: logical window size for `--shot` (default 1280x800).
    pub size: Option<(u32, u32)>,
    /// `--scale <0.5..4>` for `--shot` (default the system's).
    pub scale: Option<f32>,
    /// `--frame square|small|round`: the shot shows the window as the client-drawn shadow margin
    /// puts it on screen (its shadow and corners, over a flat backdrop) instead of the content
    /// alone.
    pub frame: Option<Corners>,
    /// `--find <text>`: put the text in the find box and go to the first match.
    pub find: Option<String>,
    /// `--match-case`: the find box's Match case option on.
    pub match_case: bool,
    /// `--whole-word` and `--regex`: the other two search options on.
    pub whole_word: bool,
    pub regex: bool,
    /// `--replace <text>`: show the replace strip with this text in its box.
    pub replace: Option<String>,
    /// `--scroll <px>`: scroll the text down this many logical px after loading.
    pub scroll: Option<f32>,
    /// `--hscroll <px>`: scroll the text right this many logical px.
    pub hscroll: Option<f32>,
    /// `--goto <line[:col]>`: put the caret there and scroll it into view (applied before
    /// `--scroll`).
    pub goto: Option<LineCol>,
    /// `--select <line:col>-<line:col>`: select from the first position to the second.
    pub select: Option<(LineCol, LineCol)>,
    /// `--select-column <line:col>-<line:col>`: a column selection between the two corners (visual
    /// columns, from 1, a wide character is two).
    pub select_column: Option<(LineCol, LineCol)>,
    /// `--menu main|recent|tab|text|find`.
    pub menu: Option<ShotMenu>,
    /// `--dialog about|shortcuts|recent`.
    pub dialog: Option<ShotDialog>,
    /// `--hover x,y`: logical pointer position for `--shot`.
    pub hover: Option<(f32, f32)>,
    /// `--menu-at F`: where across the text area (0 to 1) the right-click of the text menu shots
    /// lands; 0.3 when not given.
    pub menu_at: Option<f32>,
    /// `--menu-fade-ms MS`: a `--shot` of the `--menu` this far into its entrance (0 to 1000;
    /// more is at rest). Ignored without `--shot` and `--menu`.
    pub menu_fade_ms: Option<u32>,
    /// `--toast TEXT`, given once or more: the confirmations a `--shot` shows as toasts, the
    /// earlier ones at rest and stacked, the last one coming in under them with `--motion-ms`.
    pub toasts: Vec<String>,
    /// `--tip-ms MS`: a `--shot` of the `--hover` control's tooltip this far into its fade (0 to
    /// 1000; more is at rest). Ignored without `--shot` and `--hover`.
    pub tip_ms: Option<u32>,
    /// `--motion-ms MS`: a `--shot` of what its last `--toast` sets moving, this far into its
    /// motion (0 to 1000; more is at rest). Ignored without `--shot`.
    pub motion_ms: Option<u32>,
    /// `--settings-ms MS`: a `--shot` of the Settings dialog one switch turned, this far into
    /// its slide (0 to 1000; more is at rest). Needs `--settings-section` on a page with a
    /// switch, and is ignored without `--shot`.
    pub settings_ms: Option<u32>,
    /// `--tree-ms MS`: a `--shot` of the Files tree with its first folder opened, this far into
    /// the chevron's turn (0 to 1000; more is at rest). Ignored without `--shot`.
    pub tree_ms: Option<u32>,
    /// `--hover-ms MS`: a `--shot` of what `--hover` sets moving in the title row (a caption
    /// button's or a tab's hover fill), this far into it (0 to 1000; more is at rest). Ignored
    /// without `--shot` and `--hover`.
    pub hover_ms: Option<u32>,
    /// `--motion-clock MS`: a `--shot` of everything else it sets moving (a menu's hover rows,
    /// the command palette, a notice bar) that far into its motion, on a clock of the shot's
    /// own (0 to 1000; more is at rest). Ignored without `--shot`.
    pub motion_clock: Option<u32>,
    /// `--whitespace`: show spaces, tabs and line endings.
    pub whitespace: bool,
    /// `--wrap`: word wrap.
    pub wrap: bool,
    /// `--minimap`: the minimap beside the text.
    pub minimap: bool,
    /// `--find-in-tabs`: the results panel with what `--find` finds in the open tabs.
    pub find_in_tabs: bool,
    /// `--outline`: the left pane with the active document's function list.
    pub outline: bool,
    /// `--bookmark <line>`, as often as wanted: bookmarked lines of the active document.
    pub bookmarks: Vec<usize>,
    /// `--split`: two files side by side, the tab after the active one in the second pane.
    pub split: bool,
    /// `--split-same`: the first file in both panes (the second view starts as a copy of the
    /// first; the `--other-*` flags then place it).
    pub split_same: bool,
    /// `--other-goto <line[:col]>`, `--other-select <a-b>`, `--other-fold <line>` (repeatable) and
    /// `--other-scroll <px>`: the second pane's view, as `--goto`, `--select`, `--fold` and
    /// `--scroll` do the first's.
    pub other_goto: Option<LineCol>,
    pub other_select: Option<(LineCol, LineCol)>,
    pub other_folds: Vec<usize>,
    pub other_scroll: Option<f32>,
    /// `--demo-macros`: three saved macros in memory, for a shot of the Macro menu and dialog.
    pub demo_macros: bool,
    /// `--demo-recovered N`: the toast of a start that recovered N documents, for a shot.
    pub demo_recovered: Option<usize>,
    /// `--pref key=value` (repeatable): a preference for this run, as the settings file has it.
    pub prefs: Vec<String>,
    /// `--macro-prompt <text>`: the box that asks a macro's name, with this text typed in it.
    pub macro_prompt: Option<String>,
    /// `--split-ratio <0.1..0.9>` and `--split-left`: the left pane's share of the width, and the
    /// second pane at the left, for a shot.
    pub split_ratio: Option<f32>,
    pub split_left: bool,
    /// `--fold <line>`, as often as wanted: lines of the active document whose blocks are
    /// folded.
    pub folds: Vec<usize>,
    /// `--zoom <50..400>`: the text size in percent, instead of the session's.
    pub zoom: Option<u32>,
    /// `--maximize`: start maximized.
    pub maximize: bool,
    /// `--behind`: open without taking the focus, under every other window, so that a test run
    /// does not cover what the owner is working in.
    pub behind: bool,
    /// `--trace`: print start-up stages, load and first-frame times, memory and frame times on
    /// stderr.
    pub trace: bool,
    /// `--scroll-steps N`: after the first frame, the load and the highlighting of the first
    /// screen, scroll N wheel steps, print frame-time statistics and exit. 0 exits right after
    /// that first state is reached.
    pub scroll_steps: Option<u32>,
    /// `--type <text>`: type the text at the caret once the file has loaded (after `--goto` and
    /// `--select`), so a shot can show a file with unsaved changes.
    pub type_text: Option<String>,
    /// Typed one key at a time through the typing help ("\n" is Enter), after `--type`.
    pub type_keys: Option<String>,
    /// `--type-steps N`: after the first frame and the load, type N keystrokes at the caret
    /// (letters, with a line break and a Backspace now and then), one per frame, print the time
    /// from keystroke to pixels and exit without saving.
    pub type_steps: Option<u32>,
    /// `--preview-pane`: open the preview pane once the file has loaded, for timing it with
    /// `--trace` and `--type-steps`. Does nothing without mdview.
    pub preview_pane: bool,
    /// `--exit-after-ms N`: close the window N ms after the first frame, as if the user closed
    /// it, so a real window can be started and stopped from a script. Added in package C; not in
    /// DESIGN.md section 12.
    pub exit_after_ms: Option<u64>,
    /// Look for a newer version, print the answer and exit.
    pub check_update: bool,
    /// Install a newer version as the update bar would, start it, and exit. No window.
    pub install_update: bool,
    /// Show the update bar at a stage (`available`, `installing`, `installed`), for a shot.
    pub update_bar: Option<String>,
    /// Open the command palette with this text in its box, for a shot.
    pub palette: Option<String>,
    /// `--ai-demo instruction|review|preview|stale|closed|result-kept|working|bar|settings|review-hunks|review-partial|review-nothing|review-long|review-preview-partial|review-closed`: an AI panel or state
    /// for a shot, with a made-up result and no engine.
    pub ai_demo: Option<String>,
    /// `--plan-demo split|split-gaps|split-rest|split-warn|split-again|split-rename|split-many|split-small|folder|folder-sections|folder-taken|folder-small|folder-over|folder-scheme|folder-scheme-rename|folder-ai|folder-ai-sorting|folder-ai-asking|folder-ai-off|folder-ai-failed|folder-unsorted`:
    /// the plan screen or the import card for a shot, made from a document or a folder built in
    /// the program.
    pub plan_demo: Option<String>,
    /// `--plan-parked`: the notes pane open, with a demonstration plan ticked and left behind, for
    /// a shot of "Plan ready".
    pub plan_parked: bool,
    /// `--move-demo move|results`: the "Move the notes…" card over a plan made in the program,
    /// or the results panel after a move (`move_ui::demo`).
    pub move_demo: Option<String>,
    /// `--task-demo files|files-add|files-from|files-other|files-off|files-plan|files-copy|files-link|files-move|files-long-dest|review|sent|notes|notes-options|notes-looking|notes-failed|notes-arrange-failed|notes-no-vision|notes-limit|notes-off`: the AI task window over
    /// files for a shot, made from `fixtures/task-demo/`, with no engine and nothing sent.
    pub task_demo: Option<String>,
    /// `--note-demo ready|asking|changes|result|blocked`: the Markdown note card over a file and
    /// an answer made in the program (`note_ui::demo`).
    pub note_demo: Option<String>,
    /// `--library-demo drag|ai-menu`: the notes pane with a note held over another heading, or a
    /// section's menu with the AI task at its head, for a shot
    /// (`app/library_drag.rs`). The library is made up; nothing is read or written.
    pub library_demo: Option<String>,
    /// `--compare A B`: open both files, side by side, compared.
    pub compare: Option<(PathBuf, PathBuf)>,
    /// Compare the active document with its file on disk (for a shot: with `--type`).
    pub compare_saved: bool,
    /// Step to the next difference this many times after comparing.
    pub next_difference: usize,
    /// After that, copy the difference the caret is in across (`across`) or take it from the
    /// other side (`take`), for a shot.
    pub copy_change: Option<bool>,
    /// `--find-in-files`: search the folder for the find text now, and wait for the answer.
    pub find_in_files: bool,
    /// `--results-folder` and `--results-kinds`: what the results panel's boxes hold.
    pub results_folder: Option<String>,
    pub results_kinds: Option<String>,
    /// `--results-focus`: the results panel has the keyboard, on its second match.
    pub results_focus: bool,
    /// The results panel's replacement box starts with this text.
    pub results_replace: Option<String>,
    /// Plan the replacement in files and show the confirmation, or apply it and show the report.
    pub replace_dialog: bool,
    pub replace_report: bool,
    /// `--follow`, `--place x,y,w,h`, `--from NAME`: what the sister application asks with a
    /// launch (docs/DESIGN.md, "The hand-off with mdview"). With `--goto` they make the launch a
    /// request; see [`Args::requests`].
    pub follow: bool,
    pub place: Option<Rect>,
    pub from: Option<String>,
    /// `--run <command>`: run one command (its name as the code has it, `PreviewInViewer`) half
    /// a second after the first frame of a window, for tests and scripts.
    pub run: Option<CommandId>,
    /// `--demo-link`: the first document shows as previewed in mdview (side by side, with
    /// `--pref preview_mode=side-by-side`), with nothing started or sent, for a shot.
    pub demo_link: bool,
    /// `--settings-section <name>`: with `--dialog settings`, the form scrolled to that section
    /// (`Preview`), for a shot.
    pub settings_section: Option<String>,
    /// `--new`: start with a new document that has no file, as File > New makes one, after
    /// the files named; for a shot or a script (`--type` then types into it).
    pub new_document: bool,
}

/// `a<sep>b` with both sides parsed.
fn pair<T: std::str::FromStr>(s: &str, sep: char) -> Option<(T, T)> {
    let (a, b) = s.split_once(sep)?;
    Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
}

impl LineCol {
    /// `12` or `12:5`, both 1-based; 0 is refused.
    pub fn parse(s: &str) -> Option<LineCol> {
        let (line, col) = match s.split_once(':') {
            Some((l, c)) => (l.trim().parse().ok()?, c.trim().parse().ok()?),
            None => (s.trim().parse().ok()?, 1),
        };
        (line >= 1 && col >= 1).then_some(LineCol { line, col })
    }
}

impl Args {
    /// What each file asks of a window that is already running, when the launch asked for more
    /// than opening it: see [`Request`]. Empty for an ordinary launch.
    pub fn requests(&self) -> Vec<Request> {
        if self.goto.is_none() && !self.follow && self.place.is_none() && self.from.is_none() {
            return Vec::new();
        }
        self.files
            .iter()
            .map(|path| Request {
                path: path.clone(),
                line: self.goto.map(|g| g.line as u32),
                column: self.goto.map(|g| g.col as u32),
                follow: self.follow,
                place: self.place,
                from: self.from.clone(),
                ..Request::default()
            })
            .collect()
    }

    /// Parses the arguments after the program name. Unknown flags and bad values are reported on
    /// stderr and ignored, so a stray flag never stops the editor from opening.
    pub fn parse(args: impl IntoIterator<Item = String>) -> Args {
        let mut out = Args::default();
        let mut it = args.into_iter();
        while let Some(a) = it.next() {
            let mut value = |name: &str| {
                let v = it.next();
                if v.is_none() {
                    eprintln!("mdedit: {name} needs a value");
                }
                v
            };
            // A value that does not parse is reported and ignored.
            fn checked<T>(
                name: &str,
                raw: Option<String>,
                f: impl FnOnce(&str) -> Option<T>,
            ) -> Option<T> {
                let raw = raw?;
                let v = f(&raw);
                if v.is_none() {
                    eprintln!("mdedit: ignoring {name} {raw}");
                }
                v
            }
            match a.as_str() {
                "--light" => out.dark = Some(false),
                "--dark" => out.dark = Some(true),
                "--no-session" => out.no_session = true,
                "--files" => out.files_pane = Some(true),
                "--no-files" => out.files_pane = Some(false),
                "--maximize" => out.maximize = true,
                "--behind" => out.behind = true,
                "--whitespace" => out.whitespace = true,
                "--wrap" => out.wrap = true,
                "--minimap" => out.minimap = true,
                "--find-in-tabs" => out.find_in_tabs = true,
                "--outline" => out.outline = true,
                "--bookmark" => {
                    if let Some(n) = it.next().and_then(|v| v.parse::<usize>().ok()) {
                        out.bookmarks.push(n);
                    }
                }
                "--split" => out.split = true,
                "--split-same" => out.split_same = true,
                "--other-goto" => out.other_goto = checked(&a, value(&a), LineCol::parse),
                "--other-select" => {
                    out.other_select = checked(&a, value(&a), |v| {
                        let (from, to) = v.split_once('-')?;
                        Some((LineCol::parse(from)?, LineCol::parse(to)?))
                    })
                }
                "--other-fold" => {
                    if let Some(n) = it.next().and_then(|v| v.parse::<usize>().ok()) {
                        out.other_folds.push(n);
                    }
                }
                "--other-scroll" => {
                    out.other_scroll = checked(&a, value(&a), |v| {
                        v.parse::<f32>().ok().filter(|s| s.is_finite())
                    })
                }
                "--split-left" => out.split_left = true,
                "--demo-macros" => out.demo_macros = true,
                "--demo-recovered" => {
                    out.demo_recovered =
                        checked(&a, value(&a), |v| v.parse().ok().filter(|n| *n > 0))
                }
                "--pref" => out.prefs.extend(value(&a).filter(|v| v.contains('='))),
                "--macro-prompt" => {
                    out.macro_prompt = checked(&a, value(&a), |v| Some(v.to_string()))
                }
                "--split-ratio" => {
                    out.split_ratio = checked(&a, value(&a), |v| {
                        v.parse::<f32>().ok().filter(|r| (0.1..=0.9).contains(r))
                    })
                }
                "--fold" => {
                    if let Some(n) = it.next().and_then(|v| v.parse::<usize>().ok()) {
                        out.folds.push(n);
                    }
                }
                "--zoom" => {
                    out.zoom = checked(&a, value(&a), |v| {
                        v.parse().ok().filter(|z| (50..=400).contains(z))
                    })
                }
                "--match-case" => out.match_case = true,
                "--whole-word" => out.whole_word = true,
                "--regex" => out.regex = true,
                "--replace" => out.replace = value(&a),
                "--palette" => out.palette = value(&a),
                "--ai-demo" => out.ai_demo = value(&a),
                "--plan-demo" => out.plan_demo = value(&a),
                "--plan-parked" => out.plan_parked = true,
                "--move-demo" => out.move_demo = value(&a),
                "--task-demo" => out.task_demo = value(&a),
                "--note-demo" => out.note_demo = value(&a),
                "--library-demo" => out.library_demo = value(&a),
                "--compare" => {
                    let pair = (value(&a), value(&a));
                    if let (Some(x), Some(y)) = pair {
                        out.compare = Some((PathBuf::from(x), PathBuf::from(y)));
                    }
                }
                "--compare-saved" => out.compare_saved = true,
                "--next-difference" => {
                    out.next_difference = checked(&a, value(&a), |v| v.parse().ok()).unwrap_or(0)
                }
                "--copy-change" => {
                    out.copy_change = match value(&a).as_deref() {
                        Some("across") => Some(true),
                        Some("take") => Some(false),
                        _ => None,
                    }
                }
                "--find-in-files" => out.find_in_files = true,
                "--results-focus" => out.results_focus = true,
                "--results-folder" => out.results_folder = value(&a),
                "--results-kinds" => out.results_kinds = value(&a),
                "--results-replace" => out.results_replace = value(&a),
                "--replace-dialog" => out.replace_dialog = true,
                "--replace-report" => out.replace_report = true,
                "--trace" => out.trace = true,
                "--preview-pane" => out.preview_pane = true,
                "--folder" => out.folder = value(&a).map(PathBuf::from),
                "--shot" => out.shot = value(&a).map(PathBuf::from),
                "--find" => out.find = value(&a).filter(|v| !v.is_empty()),
                "--type" => out.type_text = value(&a).filter(|v| !v.is_empty()),
                "--type-keys" => out.type_keys = value(&a).filter(|v| !v.is_empty()),
                "--type-steps" => out.type_steps = checked(&a, value(&a), |v| v.parse().ok()),
                "--size" => {
                    out.size = checked(&a, value(&a), |v| {
                        pair::<u32>(&v.to_ascii_lowercase(), 'x').filter(|&(w, h)| {
                            (100..=16384).contains(&w) && (100..=16384).contains(&h)
                        })
                    })
                }
                "--frame" => out.frame = checked(&a, value(&a), Corners::from_name),
                "--scale" => {
                    out.scale = checked(&a, value(&a), |v| {
                        v.parse::<f32>().ok().filter(|s| (0.5..=4.0).contains(s))
                    })
                }
                "--scroll" => {
                    out.scroll = checked(&a, value(&a), |v| {
                        v.parse::<f32>().ok().filter(|s| s.is_finite())
                    })
                }
                "--hscroll" => {
                    out.hscroll = checked(&a, value(&a), |v| {
                        v.parse::<f32>().ok().filter(|s| s.is_finite())
                    })
                }
                "--goto" => out.goto = checked(&a, value(&a), LineCol::parse),
                "--follow" => out.follow = true,
                "--place" => out.place = checked(&a, value(&a), Rect::parse),
                "--from" => out.from = value(&a).filter(|v| !v.is_empty()),
                "--run" => {
                    out.run = checked(&a, value(&a), |v| {
                        crate::commands::COMMANDS
                            .iter()
                            .map(|c| c.id)
                            .find(|id| format!("{id:?}").eq_ignore_ascii_case(v))
                    })
                }
                "--demo-link" => out.demo_link = true,
                "--new" => out.new_document = true,
                "--settings-section" => out.settings_section = value(&a),
                "--select" => {
                    out.select = checked(&a, value(&a), |v| {
                        let (from, to) = v.split_once('-')?;
                        Some((LineCol::parse(from)?, LineCol::parse(to)?))
                    })
                }
                "--select-column" => {
                    out.select_column = checked(&a, value(&a), |v| {
                        let (from, to) = v.split_once('-')?;
                        Some((LineCol::parse(from)?, LineCol::parse(to)?))
                    })
                }
                "--menu-at" => out.menu_at = checked(&a, value(&a), |v| v.parse().ok()),
                "--menu-fade-ms" => out.menu_fade_ms = checked(&a, value(&a), |v| v.parse().ok()),
                "--toast" => out.toasts.extend(value(&a)),
                "--motion-ms" => out.motion_ms = checked(&a, value(&a), |v| v.parse().ok()),
                "--tip-ms" => out.tip_ms = checked(&a, value(&a), |v| v.parse().ok()),
                "--settings-ms" => out.settings_ms = checked(&a, value(&a), |v| v.parse().ok()),
                "--tree-ms" => out.tree_ms = checked(&a, value(&a), |v| v.parse().ok()),
                "--hover-ms" => out.hover_ms = checked(&a, value(&a), |v| v.parse().ok()),
                "--motion-clock" => out.motion_clock = checked(&a, value(&a), |v| v.parse().ok()),
                "--hover" => out.hover = checked(&a, value(&a), |v| pair(v, ',')),
                "--scroll-steps" => out.scroll_steps = checked(&a, value(&a), |v| v.parse().ok()),
                "--exit-after-ms" => out.exit_after_ms = checked(&a, value(&a), |v| v.parse().ok()),
                "--check-update" => out.check_update = true,
                "--install-update" => out.install_update = true,
                "--update-bar" => {
                    out.update_bar = checked(&a, value(&a), |v| {
                        matches!(
                            v,
                            "available" | "installing" | "installed" | "installed-plain"
                        )
                        .then(|| v.to_string())
                    })
                }
                "--menu" => {
                    out.menu = checked(&a, value(&a), |v| match v {
                        "main" => Some(ShotMenu::Main),
                        "main-end" => Some(ShotMenu::MainEnd),
                        "recent" => Some(ShotMenu::Recent),
                        "main-view" => Some(ShotMenu::MainView),
                        "main-macro" => Some(ShotMenu::MainMacro),
                        "view" => Some(ShotMenu::View),
                        "macro" => Some(ShotMenu::Macro),
                        "eol" => Some(ShotMenu::Eol),
                        "encoding" => Some(ShotMenu::Encoding),
                        "tab" => Some(ShotMenu::Tab),
                        "text" => Some(ShotMenu::Text),
                        "find" => Some(ShotMenu::Find),
                        "minimap" => Some(ShotMenu::Minimap),
                        "macro-rec" => Some(ShotMenu::MacroRec),
                        "complete" => Some(ShotMenu::Complete),
                        "complete-snippet" => Some(ShotMenu::CompleteSnippet),
                        "text-lines" => Some(ShotMenu::TextLines),
                        "text-convert" => Some(ShotMenu::TextConvert),
                        "text-ai" => Some(ShotMenu::TextAi),
                        "language" => Some(ShotMenu::Language),
                        "main-library" => Some(ShotMenu::MainLibrary),
                        "library" => Some(ShotMenu::Library),
                        "library-add" => Some(ShotMenu::LibraryAdd),
                        "library-pane" => Some(ShotMenu::LibraryPane),
                        "library-row" => Some(ShotMenu::LibraryRow),
                        "library-cursor" => Some(ShotMenu::LibraryCursor),
                        "library-section" => Some(ShotMenu::LibrarySection),
                        "library-empty" => Some(ShotMenu::LibraryEmpty),
                        "library-rename" => Some(ShotMenu::LibraryRename),
                        "library-newgroup" => Some(ShotMenu::LibraryNewGroup),
                        "library-remove" => Some(ShotMenu::LibraryRemove),
                        "rename-bar" => Some(ShotMenu::RenameBar),
                        "crash-bar" => Some(ShotMenu::CrashBar),
                        "stale-bar" => Some(ShotMenu::StaleBar),
                        "text-markdown" => Some(ShotMenu::TextMarkdown),
                        "preview" => Some(ShotMenu::Preview),
                        "bar-ai" => Some(ShotMenu::BarAi),
                        _ => None,
                    })
                }
                "--dialog" => {
                    out.dialog = checked(&a, value(&a), |v| match v {
                        "about" => Some(ShotDialog::About),
                        "shortcuts" => Some(ShotDialog::Shortcuts),
                        "recent" => Some(ShotDialog::Recent),
                        "macros" => Some(ShotDialog::Macros),
                        "settings" => Some(ShotDialog::Settings),
                        "unsaved" => Some(ShotDialog::Unsaved),
                        "runs" => Some(ShotDialog::Runs),
                        "run-undo" => Some(ShotDialog::RunUndo),
                        "run-undo-recycled" => Some(ShotDialog::RunUndoRecycled),
                        "runs-newer" => Some(ShotDialog::RunsNewer),
                        "run-undo-original" => Some(ShotDialog::RunUndoOriginal),
                        "run-undo-later" => Some(ShotDialog::RunUndoLater),
                        "run-undone-later" => Some(ShotDialog::RunUndoneLater),
                        "run-undo-moved" => Some(ShotDialog::RunUndoMoved),
                        "run-undone-moved" => Some(ShotDialog::RunUndoneMoved),
                        "run-undo-pictures" => Some(ShotDialog::RunUndoPictures),
                        "run-undone-pictures" => Some(ShotDialog::RunUndonePictures),
                        "library-delete" => Some(ShotDialog::LibraryDelete),
                        "library-delete-long" => Some(ShotDialog::LibraryDeleteLong),
                        "library-remove-long" => Some(ShotDialog::LibraryRemoveLong),
                        _ => None,
                    })
                }
                // The mark of a restart after an update, read before this (`relaunch::FLAG`).
                mdshared_app::relaunch::FLAG => {}
                f if f.starts_with("--") => eprintln!("mdedit: ignoring {f}"),
                _ => out.files.push(PathBuf::from(a)),
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Args {
        Args::parse(s.split_whitespace().map(String::from))
    }

    #[test]
    fn the_minimap_switch() {
        assert!(parse("--minimap a.rs").minimap);
        assert!(!parse("a.rs").minimap);
        // It goes with the other switches of the timing runs.
        let a = parse("--minimap --trace --type-steps 5 --scroll-steps 7 --preview-pane a.rs");
        assert!(a.minimap && a.trace);
        assert_eq!((a.type_steps, a.scroll_steps), (Some(5), Some(7)));
        assert!(a.preview_pane);
    }

    #[test]
    fn the_restart_mark_is_not_a_file() {
        let a = parse("--restarted");
        assert!(a.files.is_empty());
        assert_eq!(a, parse(""));
    }

    #[test]
    fn files_and_switches() {
        let a = parse(
            "a.rs --dark b.txt --folder docs --no-session --files --maximize --match-case --trace",
        );
        assert_eq!(a.files, [PathBuf::from("a.rs"), PathBuf::from("b.txt")]);
        assert_eq!(a.dark, Some(true));
        assert_eq!(a.folder, Some(PathBuf::from("docs")));
        assert!(a.no_session && a.maximize && a.match_case && a.trace);
        assert_eq!(a.files_pane, Some(true));
        assert_eq!(parse("--light --no-files").dark, Some(false));
        assert!(parse("--no-session --behind a.rs").behind);
        assert!(!parse("a.rs").behind);
        assert_eq!(parse("--no-files").files_pane, Some(false));
    }

    #[test]
    fn palette_compare_and_replace_flags() {
        assert_eq!(
            parse("--palette >fold a.rs").palette.as_deref(),
            Some(">fold")
        );
        assert!(parse("a.rs").palette.is_none());
        let a = parse("--compare a.rs b.rs --next-difference 3");
        assert_eq!(
            a.compare,
            Some((PathBuf::from("a.rs"), PathBuf::from("b.rs")))
        );
        assert_eq!(a.next_difference, 3);
        // A pair needs both files.
        assert_eq!(parse("--compare a.rs").compare, None);
        assert!(parse("--compare-saved a.rs").compare_saved);
        assert_eq!(parse("--copy-change across a.rs").copy_change, Some(true));
        assert_eq!(parse("--copy-change take a.rs").copy_change, Some(false));
        assert_eq!(parse("--copy-change a.rs").copy_change, None);
        let a = parse(
            "--find-in-files --results-focus --results-replace new --replace-dialog --replace-report a.rs",
        );
        assert!(a.find_in_files && a.results_focus && a.replace_dialog && a.replace_report);
        assert_eq!(a.results_replace.as_deref(), Some("new"));
    }

    #[test]
    fn motion_shot_flags() {
        let a = parse("--toast Copied --toast Saved --motion-ms 67 --menu-fade-ms 40 a.rs");
        assert_eq!(a.toasts, ["Copied", "Saved"]);
        assert_eq!((a.motion_ms, a.menu_fade_ms), (Some(67), Some(40)));
        let b = parse("--motion-ms soon a.rs");
        assert_eq!(b.motion_ms, None);
        assert!(b.toasts.is_empty());
    }

    #[test]
    fn shot_flags() {
        let a = parse(
            "--shot x.png --size 700x500 --scale 1.5 --hover 10,20 --menu recent --dialog about \
             --scroll 300 --hscroll 40.5 --find needle --scroll-steps 300 --exit-after-ms 2000",
        );
        assert_eq!(a.shot, Some(PathBuf::from("x.png")));
        assert_eq!(a.size, Some((700, 500)));
        assert_eq!(a.scale, Some(1.5));
        assert_eq!(a.hover, Some((10.0, 20.0)));
        assert_eq!(a.menu, Some(ShotMenu::Recent));
        assert_eq!(a.dialog, Some(ShotDialog::About));
        assert_eq!(a.scroll, Some(300.0));
        assert_eq!(a.hscroll, Some(40.5));
        assert_eq!(a.find.as_deref(), Some("needle"));
        assert_eq!(a.scroll_steps, Some(300));
        assert_eq!(a.exit_after_ms, Some(2000));
        for (name, m) in [
            ("main", ShotMenu::Main),
            ("main-end", ShotMenu::MainEnd),
            ("main-view", ShotMenu::MainView),
            ("main-macro", ShotMenu::MainMacro),
            ("tab", ShotMenu::Tab),
            ("text", ShotMenu::Text),
            ("find", ShotMenu::Find),
            ("minimap", ShotMenu::Minimap),
            ("macro-rec", ShotMenu::MacroRec),
            ("complete", ShotMenu::Complete),
            ("complete-snippet", ShotMenu::CompleteSnippet),
            ("preview", ShotMenu::Preview),
            ("bar-ai", ShotMenu::BarAi),
        ] {
            assert_eq!(parse(&format!("--menu {name}")).menu, Some(m));
        }
        assert_eq!(
            parse("--dialog shortcuts").dialog,
            Some(ShotDialog::Shortcuts)
        );
        assert_eq!(parse("--dialog recent").dialog, Some(ShotDialog::Recent));
        assert_eq!(
            parse("--dialog runs-newer").dialog,
            Some(ShotDialog::RunsNewer)
        );
        assert_eq!(
            parse("--dialog run-undo-original").dialog,
            Some(ShotDialog::RunUndoOriginal)
        );
        assert_eq!(
            parse("--dialog run-undo-later").dialog,
            Some(ShotDialog::RunUndoLater)
        );
        assert_eq!(
            parse("--dialog run-undone-later").dialog,
            Some(ShotDialog::RunUndoneLater)
        );
        assert_eq!(
            parse("--dialog run-undo-moved").dialog,
            Some(ShotDialog::RunUndoMoved)
        );
        assert_eq!(
            parse("--dialog run-undone-moved").dialog,
            Some(ShotDialog::RunUndoneMoved)
        );
        assert_eq!(
            parse("--dialog run-undo-pictures").dialog,
            Some(ShotDialog::RunUndoPictures)
        );
        assert_eq!(
            parse("--dialog run-undone-pictures").dialog,
            Some(ShotDialog::RunUndonePictures)
        );
        assert_eq!(
            parse("--dialog library-delete").dialog,
            Some(ShotDialog::LibraryDelete)
        );
        assert_eq!(
            parse("--dialog library-delete-long").dialog,
            Some(ShotDialog::LibraryDeleteLong)
        );
        assert_eq!(
            parse("--dialog library-remove-long").dialog,
            Some(ShotDialog::LibraryRemoveLong)
        );
        assert_eq!(parse("--scroll-steps 0").scroll_steps, Some(0));
    }

    #[test]
    fn positions() {
        assert_eq!(parse("--goto 12").goto, Some(LineCol { line: 12, col: 1 }));
        assert_eq!(
            parse("--goto 12:5").goto,
            Some(LineCol { line: 12, col: 5 })
        );
        assert_eq!(
            parse("--select 3:1-5:4").select,
            Some((LineCol { line: 3, col: 1 }, LineCol { line: 5, col: 4 }))
        );
        assert_eq!(
            parse("--select 3-5").select,
            Some((LineCol { line: 3, col: 1 }, LineCol { line: 5, col: 1 }))
        );
        assert_eq!(parse("--goto 0").goto, None);
        assert_eq!(parse("--goto 3:0").goto, None);
        assert_eq!(parse("--goto x").goto, None);
        assert_eq!(parse("--select 3:1").select, None);
        assert_eq!(
            parse("--select-column 3:5-7:9").select_column,
            Some((LineCol { line: 3, col: 5 }, LineCol { line: 7, col: 9 }))
        );
        assert_eq!(parse("--select-column 3:5").select_column, None);
    }

    #[test]
    fn the_hand_off_flags_make_a_launch_a_request() {
        let a = parse("--goto 20:3 --from mdview --follow --place -10,20,960,1000 notes.md");
        assert!(a.follow);
        assert_eq!(a.from.as_deref(), Some("mdview"));
        assert_eq!(
            a.place,
            Some(Rect {
                x: -10,
                y: 20,
                w: 960,
                h: 1000
            })
        );
        assert_eq!(
            a.requests(),
            [Request {
                line: Some(20),
                column: Some(3),
                follow: true,
                place: a.place,
                from: Some("mdview".into()),
                ..Request::new("notes.md")
            }]
        );
        // Each file is handed over with the same options.
        assert_eq!(parse("--goto 7 a.md b.md").requests().len(), 2);
        // An ordinary launch asks nothing: the paths go as they always did.
        assert!(parse("a.md b.md").requests().is_empty());
        assert!(parse("--goto 3").requests().is_empty());
        assert_eq!(parse("--from mdview a.md").requests()[0].line, None);
        // A bad value is dropped, not guessed at.
        let bad = parse("--place 1,2,3 --place 1,2,0,4 a.md");
        assert_eq!(bad.place, None);
        assert_eq!(bad.files, [PathBuf::from("a.md")]);
    }

    #[test]
    fn frame_names_the_corners_of_a_window_shot() {
        assert_eq!(parse("--frame round").frame, Some(Corners::Round));
        assert_eq!(parse("--frame Square").frame, Some(Corners::Square));
        assert_eq!(parse("--frame 4").frame, Some(Corners::Small));
        assert_eq!(parse("--frame pointy").frame, None);
        assert_eq!(parse("a.md").frame, None);
    }

    #[test]
    fn run_and_demo_link_are_for_tests_and_shots() {
        assert_eq!(
            parse("--run PreviewInViewer a.md").run,
            Some(CommandId::PreviewInViewer)
        );
        assert_eq!(
            parse("--run previewsidebyside").run,
            Some(CommandId::PreviewSideBySide)
        );
        assert_eq!(parse("--run nonsense").run, None);
        assert!(parse("--demo-link").demo_link && !parse("a.md").demo_link);
        assert!(parse("--new").new_document && !parse("a.md").new_document);
        assert_eq!(parse("--demo-recovered 2").demo_recovered, Some(2));
        assert_eq!(parse("--demo-recovered 0").demo_recovered, None);
        assert_eq!(parse("a.md").demo_recovered, None);
        assert_eq!(
            parse("--settings-section Preview")
                .settings_section
                .as_deref(),
            Some("Preview")
        );
    }

    #[test]
    fn bad_values_are_ignored() {
        let a = parse(
            "--scale 9 --size big --bogus --menu nope --dialog edit --hover 1 --scroll x \
             --scroll-steps -1 --size 10x10",
        );
        assert_eq!(a, Args::default());
        assert_eq!(parse("--scale 0.4").scale, None);
        assert_eq!(parse("--scale 4").scale, Some(4.0));
        // A flag at the end without its value is reported and ignored.
        assert_eq!(parse("--goto"), Args::default());
        assert_eq!(parse("--find").find, None);
    }
}
