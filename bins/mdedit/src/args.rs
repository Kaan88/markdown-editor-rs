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
    /// The "Save changes?" question for the active file.
    Unsaved,
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
    /// `--type <text>`: type the text at the caret once the file has loaded (after `--goto` and
    /// `--select`), so a shot can show a file with unsaved changes.
    pub type_text: Option<String>,
    /// `--type-steps N`: after the first frame and the load, type N keystrokes at the caret
    /// (letters, with a line break and a Backspace now and then), one per frame, print the time
    /// from keystroke to pixels and exit without saving.
    pub type_steps: Option<u32>,
    /// `--exit-after-ms N`: close the window N ms after the first frame, as if the user closed
    /// it, so a real window can be started and stopped from a script. Added in package C; not in
    /// DESIGN.md section 12.
    pub exit_after_ms: Option<u64>,
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
                "--match-case" => out.match_case = true,
                "--trace" => out.trace = true,
                "--folder" => out.folder = value(&a).map(PathBuf::from),
                "--shot" => out.shot = value(&a).map(PathBuf::from),
                "--find" => out.find = value(&a).filter(|v| !v.is_empty()),
                "--type" => out.type_text = value(&a).filter(|v| !v.is_empty()),
                "--type-steps" => out.type_steps = checked(&a, value(&a), |v| v.parse().ok()),
                "--size" => {
                    out.size = checked(&a, value(&a), |v| {
                        pair::<u32>(&v.to_ascii_lowercase(), 'x').filter(|&(w, h)| {
                            (100..=16384).contains(&w) && (100..=16384).contains(&h)
                        })
                    })
                }
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
                "--select" => {
                    out.select = checked(&a, value(&a), |v| {
                        let (from, to) = v.split_once('-')?;
                        Some((LineCol::parse(from)?, LineCol::parse(to)?))
                    })
                }
                "--hover" => out.hover = checked(&a, value(&a), |v| pair(v, ',')),
                "--scroll-steps" => out.scroll_steps = checked(&a, value(&a), |v| v.parse().ok()),
                "--exit-after-ms" => out.exit_after_ms = checked(&a, value(&a), |v| v.parse().ok()),
                "--menu" => {
                    out.menu = checked(&a, value(&a), |v| match v {
                        "main" => Some(ShotMenu::Main),
                        "recent" => Some(ShotMenu::Recent),
                        "tab" => Some(ShotMenu::Tab),
                        "text" => Some(ShotMenu::Text),
                        "find" => Some(ShotMenu::Find),
                        _ => None,
                    })
                }
                "--dialog" => {
                    out.dialog = checked(&a, value(&a), |v| match v {
                        "about" => Some(ShotDialog::About),
                        "shortcuts" => Some(ShotDialog::Shortcuts),
                        "recent" => Some(ShotDialog::Recent),
                        "unsaved" => Some(ShotDialog::Unsaved),
                        _ => None,
                    })
                }
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
        assert_eq!(parse("--no-files").files_pane, Some(false));
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
            ("tab", ShotMenu::Tab),
            ("text", ShotMenu::Text),
            ("find", ShotMenu::Find),
        ] {
            assert_eq!(parse(&format!("--menu {name}")).menu, Some(m));
        }
        assert_eq!(
            parse("--dialog shortcuts").dialog,
            Some(ShotDialog::Shortcuts)
        );
        assert_eq!(parse("--dialog recent").dialog, Some(ShotDialog::Recent));
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
