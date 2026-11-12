//! Command-line arguments: files to open, start-up overrides, and the flags that render a UI state
//! to a PNG without a window (`--shot`) or print timings (`--trace`, `--scroll-steps`). The flags
//! and their meaning are fixed in `docs/DESIGN.md`, section "Command line".

use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShotMenu {
    Main,
    Recent,
    Tab,
    /// The text area's right-click menu.
    Text,
    Find,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShotDialog {
    About,
    Shortcuts,
    Recent,
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
    /// `--find <text>`: put the text in the find box and go to the first match.
    pub find: Option<String>,
    /// `--match-case`: the find box's Match case option on.
    pub match_case: bool,
    /// `--scroll <px>`: scroll the text down this many logical px after loading.
    pub scroll: Option<f32>,
    /// `--hscroll <px>`: scroll the text right this many logical px.
    pub hscroll: Option<f32>,
    /// `--goto <line[:col]>`: put the caret there and scroll it into view (applied before
    /// `--scroll`).
    pub goto: Option<LineCol>,
    /// `--select <line:col>-<line:col>`: select from the first position to the second.
    pub select: Option<(LineCol, LineCol)>,
    /// `--menu main|recent|tab|text|find`.
    pub menu: Option<ShotMenu>,
    /// `--dialog about|shortcuts|recent`.
    pub dialog: Option<ShotDialog>,
    /// `--hover x,y`: logical pointer position for `--shot`.
    pub hover: Option<(f32, f32)>,
    /// `--maximize`: start maximized.
    pub maximize: bool,
    /// `--trace`: print start-up stages, load and first-frame times, memory and frame times on
    /// stderr.
    pub trace: bool,
    /// `--scroll-steps N`: after the first frame, the load and the highlighting of the first
    /// screen, scroll N wheel steps, print frame-time statistics and exit. 0 exits right after
    /// that first state is reached.
    pub scroll_steps: Option<u32>,
}

impl Args {
    /// Parses the arguments after the program name. Unknown flags and bad values are reported on
    /// stderr and ignored, so a stray flag never stops the editor from opening.
    pub fn parse(args: impl IntoIterator<Item = String>) -> Args {
        let _ = args.into_iter();
        todo!()
    }
}
