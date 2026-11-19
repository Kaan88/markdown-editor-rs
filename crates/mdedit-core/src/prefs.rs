//! The user's preferences as a plain struct: defaults, clamped ranges, and reading and writing as
//! the `key=value` lines of the settings file (the format of `mdshared_app::settings`, as the
//! session file uses it). Preferences live in their own section, `[preferences]`, so they can sit
//! in the same file as the session, whose reader skips sections that are not its own, and the
//! other way round.
//!
//! Reading never fails: unknown keys and sections are ignored, a value that does not parse or is
//! out of range falls back to the default of its key (a number in range but past a limit is
//! clamped when it parses, and a number that does not parse is the default), and the same text
//! read twice gives the same struct. Writing always writes every key, so write, read, write gives
//! the same text.

use crate::ai::Connection;
use crate::buffer::Buffer;
use crate::edit::{Cmd, Outcome};
use crate::history::{History, Kind};
use crate::limits::TAB_WIDTH;
use crate::lines::trim_into;
use crate::selection::Selection;
use crate::typing::{Assist, Indent};
use mdshared_app::backdrop::Backdrop;
use mdshared_app::settings::{self, Writer, escape, unescape};

/// The section of the settings file.
pub const SECTION: &str = "preferences";

/// The text size at zoom 100 in the shell (`FONT_PX` in `view/metrics.rs`: 14 logical px, treated
/// as 14 pt), the default of [`Prefs::font_pt`].
pub const DEFAULT_FONT_PT: f32 = 14.0;
pub const FONT_PT_MIN: f32 = 7.0;
pub const FONT_PT_MAX: f32 = 32.0;
pub const TAB_WIDTH_MAX: usize = 16;
pub const INDENT_SIZE_MAX: usize = 16;
/// The minimap's width, logical px: the default and the range the setting takes.
pub const MINIMAP_WIDTH_DEFAULT: usize = 96;
pub const MINIMAP_WIDTH_MIN: usize = 48;
pub const MINIMAP_WIDTH_MAX: usize = 240;
/// A font family name longer than this many bytes is not a name: the default is used.
pub const FONT_FAMILY_MAX: usize = 256;

#[derive(Clone, Debug, PartialEq)]
pub struct Prefs {
    /// The text font; empty is the shell's default.
    pub font_family: String,
    /// Text size in pt, 7 to 32 (halves are kept, other fractions too; not NaN).
    pub font_pt: f32,
    /// Columns of a tab stop, 1 to 16 (default [`TAB_WIDTH`]).
    pub tab_width: usize,
    /// A file's own indentation ([`crate::indent::detect`]) wins over the two settings below when
    /// it has a clear style.
    pub detect_indent: bool,
    /// Tab inserts spaces rather than a tab character.
    pub indent_with_spaces: bool,
    /// Spaces of one indent level, 1 to 16 (default 4); used when `indent_with_spaces`, and as the
    /// size of a unit of auto-indentation in that case.
    pub indent_size: usize,
    pub auto_pair: bool,
    pub auto_indent: bool,
    pub trim_on_save: bool,
    pub final_newline_on_save: bool,
    pub line_numbers: bool,
    pub highlight_line: bool,
    /// Whether a new window shows the minimap.
    pub minimap: bool,
    /// The minimap column's width, logical px, 48 to 240 (default 96).
    pub minimap_width: usize,
    /// Whether a new window wraps long lines.
    pub word_wrap: bool,
    pub restore_session: bool,
    /// At start, show the pinned tab used last instead of the tab that was showing.
    pub start_on_pinned: bool,
    /// At start, offer to close the tabs not opened in a week of use. On by default.
    pub offer_stale: bool,
    /// mdview shows the text of a previewed document as it is typed, not only when it is saved.
    pub live_preview: bool,
    /// The word list opens by itself after three letters (Ctrl+Space opens it either way).
    pub complete_auto: bool,
    /// A system backdrop behind the chrome (Windows 11); off is the flat colours.
    pub backdrop: Backdrop,
    /// Where AI rewrites go; off by default.
    pub ai_connection: Connection,
    /// A rewrite opens in a review before it goes in (on by default); off puts it in at once.
    pub ai_review: bool,
    /// The review opens on the rendered preview rather than the changes.
    pub ai_review_preview: bool,
    /// The model and effort chosen for each connection but Off, in [`Connection::ALL`] order
    /// from the second; empty is the default.
    pub ai_models: [String; 4],
    pub ai_efforts: [String; 4],
    /// The provider of the OpenAI-compatible API ([`crate::ai::PRESETS`]), and the base URL for
    /// a custom one.
    pub ai_preset: String,
    pub ai_base_url: String,
    /// The Claude Code and Codex programs; empty finds them.
    pub ai_claude_path: String,
    pub ai_codex_path: String,
}

/// A setting's text longer than this many bytes is not a setting: the default is used.
const AI_TEXT_MAX: usize = 2048;

/// The index of a connection's model and effort in [`Prefs::ai_models`].
fn ai_index(c: Connection) -> Option<usize> {
    Connection::ALL[1..].iter().position(|&x| x == c)
}

impl Default for Prefs {
    fn default() -> Prefs {
        Prefs {
            font_family: String::new(),
            font_pt: DEFAULT_FONT_PT,
            tab_width: TAB_WIDTH,
            detect_indent: true,
            indent_with_spaces: false,
            indent_size: 4,
            auto_pair: true,
            auto_indent: true,
            trim_on_save: false,
            final_newline_on_save: false,
            line_numbers: true,
            highlight_line: true,
            minimap: false,
            minimap_width: MINIMAP_WIDTH_DEFAULT,
            word_wrap: false,
            restore_session: true,
            start_on_pinned: false,
            offer_stale: true,
            live_preview: true,
            complete_auto: false,
            backdrop: Backdrop::Off,
            ai_connection: Connection::Off,
            ai_review: true,
            ai_review_preview: false,
            ai_models: Default::default(),
            ai_efforts: Default::default(),
            ai_preset: "openai".into(),
            ai_base_url: String::new(),
            ai_claude_path: String::new(),
            ai_codex_path: String::new(),
        }
    }
}

/// A text setting, or empty when it is too long or holds a line break.
fn ai_text(value: &str) -> String {
    let v = unescape(value);
    if v.len() <= AI_TEXT_MAX && !v.contains(['\n', '\r']) {
        v
    } else {
        String::new()
    }
}

fn flag(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn number(value: &str, min: usize, max: usize) -> Option<usize> {
    value
        .trim()
        .parse::<usize>()
        .ok()
        .map(|n| n.clamp(min, max))
}

impl Prefs {
    /// Puts every field in its range.
    pub fn clamp(&mut self) {
        if !self.font_pt.is_finite() {
            self.font_pt = DEFAULT_FONT_PT;
        }
        self.font_pt = self.font_pt.clamp(FONT_PT_MIN, FONT_PT_MAX);
        self.tab_width = self.tab_width.clamp(1, TAB_WIDTH_MAX);
        self.indent_size = self.indent_size.clamp(1, INDENT_SIZE_MAX);
        self.minimap_width = self
            .minimap_width
            .clamp(MINIMAP_WIDTH_MIN, MINIMAP_WIDTH_MAX);
        if self.font_family.len() > FONT_FAMILY_MAX || self.font_family.contains(['\n', '\r']) {
            self.font_family.clear();
        }
        for t in self
            .ai_models
            .iter_mut()
            .chain(self.ai_efforts.iter_mut())
            .chain([
                &mut self.ai_base_url,
                &mut self.ai_claude_path,
                &mut self.ai_codex_path,
            ])
        {
            if t.len() > AI_TEXT_MAX || t.contains(['\n', '\r']) {
                t.clear();
            }
        }
        if !crate::ai::PRESETS.iter().any(|x| x.key == self.ai_preset) {
            self.ai_preset = "openai".into();
        }
    }

    /// Reads the `[preferences]` section of `text` (text with no section header is read as that
    /// section too).
    pub fn parse(text: &str) -> Prefs {
        let mut p = Prefs::default();
        let d = Prefs::default();
        let mut in_section = true;
        for line in text.lines() {
            let t = line.trim();
            if t.starts_with('[') && t.ends_with(']') {
                in_section = &t[1..t.len() - 1] == SECTION;
                continue;
            }
            if !in_section {
                continue;
            }
            for (key, value) in settings::pairs(line) {
                match key {
                    "font_family" => {
                        let v = unescape(value);
                        p.font_family = if v.len() <= FONT_FAMILY_MAX && !v.contains(['\n', '\r']) {
                            v
                        } else {
                            String::new()
                        };
                    }
                    "font_size" => {
                        p.font_pt = value
                            .trim()
                            .parse::<f32>()
                            .ok()
                            .filter(|v| v.is_finite())
                            .map_or(d.font_pt, |v| v.clamp(FONT_PT_MIN, FONT_PT_MAX))
                    }
                    "tab_width" => {
                        p.tab_width = number(value, 1, TAB_WIDTH_MAX).unwrap_or(d.tab_width)
                    }
                    "detect_indent" => p.detect_indent = flag(value).unwrap_or(d.detect_indent),
                    "indent_spaces" => {
                        p.indent_with_spaces = flag(value).unwrap_or(d.indent_with_spaces)
                    }
                    "indent_size" => {
                        p.indent_size = number(value, 1, INDENT_SIZE_MAX).unwrap_or(d.indent_size)
                    }
                    "auto_pair" => p.auto_pair = flag(value).unwrap_or(d.auto_pair),
                    "auto_indent" => p.auto_indent = flag(value).unwrap_or(d.auto_indent),
                    "trim_on_save" => p.trim_on_save = flag(value).unwrap_or(d.trim_on_save),
                    "final_newline_on_save" => {
                        p.final_newline_on_save = flag(value).unwrap_or(d.final_newline_on_save)
                    }
                    "line_numbers" => p.line_numbers = flag(value).unwrap_or(d.line_numbers),
                    "highlight_line" => p.highlight_line = flag(value).unwrap_or(d.highlight_line),
                    "minimap" => p.minimap = flag(value).unwrap_or(d.minimap),
                    "minimap_width" => {
                        p.minimap_width = number(value, MINIMAP_WIDTH_MIN, MINIMAP_WIDTH_MAX)
                            .unwrap_or(d.minimap_width)
                    }
                    "word_wrap" => p.word_wrap = flag(value).unwrap_or(d.word_wrap),
                    "restore_session" => {
                        p.restore_session = flag(value).unwrap_or(d.restore_session)
                    }
                    "offer_stale" => p.offer_stale = flag(value).unwrap_or(d.offer_stale),
                    "start_on_pinned" => {
                        p.start_on_pinned = flag(value).unwrap_or(d.start_on_pinned)
                    }
                    "live_preview" => p.live_preview = flag(value).unwrap_or(d.live_preview),
                    "complete_auto" => p.complete_auto = flag(value).unwrap_or(d.complete_auto),
                    "backdrop" => p.backdrop = Backdrop::parse(value).unwrap_or(d.backdrop),
                    "ai_connection" => {
                        p.ai_connection = Connection::parse(value).unwrap_or(d.ai_connection)
                    }
                    "ai_review" => p.ai_review = flag(value).unwrap_or(d.ai_review),
                    "ai_review_preview" => {
                        p.ai_review_preview = flag(value).unwrap_or(d.ai_review_preview)
                    }
                    "ai_preset" => {
                        let v = ai_text(value);
                        p.ai_preset = if crate::ai::PRESETS.iter().any(|x| x.key == v) {
                            v
                        } else {
                            d.ai_preset.clone()
                        };
                    }
                    "ai_base_url" => p.ai_base_url = ai_text(value),
                    "ai_claude_path" => p.ai_claude_path = ai_text(value),
                    "ai_codex_path" => p.ai_codex_path = ai_text(value),
                    k => {
                        let per = |prefix: &str| {
                            k.strip_prefix(prefix)
                                .and_then(Connection::parse)
                                .and_then(ai_index)
                        };
                        if let Some(i) = per("ai_model_") {
                            p.ai_models[i] = ai_text(value);
                        } else if let Some(i) = per("ai_effort_") {
                            p.ai_efforts[i] = ai_text(value);
                        }
                    }
                }
            }
        }
        p.clamp();
        p
    }

    /// The `[preferences]` section: every key, one per line, flags as `1` and `0`.
    pub fn serialize(&self) -> String {
        let mut p = self.clone();
        p.clamp();
        let b = |v: bool| if v { "1" } else { "0" };
        let mut w = Writer::new(SECTION);
        w.line("font_family", &escape(&p.font_family));
        w.line("font_size", &p.font_pt.to_string());
        w.line("tab_width", &p.tab_width.to_string());
        w.line("detect_indent", b(p.detect_indent));
        w.line("indent_spaces", b(p.indent_with_spaces));
        w.line("indent_size", &p.indent_size.to_string());
        w.line("auto_pair", b(p.auto_pair));
        w.line("auto_indent", b(p.auto_indent));
        w.line("trim_on_save", b(p.trim_on_save));
        w.line("final_newline_on_save", b(p.final_newline_on_save));
        w.line("line_numbers", b(p.line_numbers));
        w.line("highlight_line", b(p.highlight_line));
        w.line("minimap", b(p.minimap));
        w.line("minimap_width", &p.minimap_width.to_string());
        w.line("word_wrap", b(p.word_wrap));
        w.line("restore_session", b(p.restore_session));
        w.line("start_on_pinned", b(p.start_on_pinned));
        w.line("offer_stale", b(p.offer_stale));
        w.line("live_preview", b(p.live_preview));
        w.line("complete_auto", b(p.complete_auto));
        w.line("backdrop", p.backdrop.key());
        w.line("ai_connection", p.ai_connection.key());
        w.line("ai_review", b(p.ai_review));
        w.line("ai_review_preview", b(p.ai_review_preview));
        for (i, c) in Connection::ALL[1..].iter().enumerate() {
            w.line(&format!("ai_model_{}", c.key()), &escape(&p.ai_models[i]));
            w.line(&format!("ai_effort_{}", c.key()), &escape(&p.ai_efforts[i]));
        }
        w.line("ai_preset", &escape(&p.ai_preset));
        w.line("ai_base_url", &escape(&p.ai_base_url));
        w.line("ai_claude_path", &escape(&p.ai_claude_path));
        w.line("ai_codex_path", &escape(&p.ai_codex_path));
        w.finish()
    }

    /// The model chosen for a connection; empty is its default.
    pub fn ai_model(&self, c: Connection) -> &str {
        ai_index(c).map_or("", |i| self.ai_models[i].as_str())
    }

    pub fn ai_effort(&self, c: Connection) -> &str {
        ai_index(c).map_or("", |i| self.ai_efforts[i].as_str())
    }

    pub fn set_ai_model(&mut self, c: Connection, m: &str) {
        if let Some(i) = ai_index(c) {
            self.ai_models[i] = m.trim().to_string();
        }
    }

    pub fn set_ai_effort(&mut self, c: Connection, e: &str) {
        if let Some(i) = ai_index(c) {
            self.ai_efforts[i] = e.trim().to_string();
        }
    }

    /// One level of indentation: a tab, or `indent_size` spaces.
    pub fn indent(&self) -> Indent {
        if self.indent_with_spaces {
            Indent::Spaces(self.indent_size.clamp(1, INDENT_SIZE_MAX) as u8)
        } else {
            Indent::Tab
        }
    }

    /// The typing help for a language (`typing::Assist::for_language`) with these preferences'
    /// switches, indent unit and tab width.
    pub fn assist(&self, language: &str) -> Assist {
        self.assist_for(language, None)
    }

    /// [`Prefs::assist`] for a document whose indentation was detected (`detected`): with
    /// `detect_indent` on, that unit is used instead of the settings'.
    pub fn assist_for(&self, language: &str, detected: Option<Indent>) -> Assist {
        let unit = detected.filter(|_| self.detect_indent);
        let mut a = Assist::for_language(language, unit.unwrap_or(self.indent()), self.tab_width);
        a.auto_pair = self.auto_pair;
        a.auto_indent = self.auto_indent;
        a
    }
}

/// Applies the save options as one undo step: with `trim_on_save`, removes the spaces and tabs at
/// the end of every line (as [`crate::lines::trim_trailing_whitespace`] does); with
/// `final_newline_on_save`, adds one line break `eol` at the end of a document that is not empty
/// and does not end with a line break (after the trimming). Nothing when no byte would change
/// (no history entry, the selection returned). The selection ends move with the text; one at the
/// very end stays before an added break. `eol` is the document's line ending, "\r\n", "\n" or
/// "\r" (the shell's most frequent one); anything else is taken as "\n".
///
/// Cost as the trimming: one walk over the lines when `trim_on_save` is set, nothing else
/// otherwise.
pub fn apply_on_save(
    buf: &mut Buffer,
    hist: &mut History,
    sel: Selection,
    prefs: &Prefs,
    eol: &str,
) -> Outcome {
    let len = buf.len_bytes();
    let (mut a, mut c) = (sel.anchor.min(len), sel.caret.min(len));
    let mut cmd = Cmd::new(buf);
    if prefs.trim_on_save {
        trim_into(&mut cmd, &mut a, &mut c);
    }
    if prefs.final_newline_on_save {
        let end = cmd.buf.len_bytes();
        if end > 0 && !matches!(cmd.buf.rope().byte(end - 1), b'\n' | b'\r') {
            let eol = if matches!(eol, "\r\n" | "\n" | "\r") {
                eol
            } else {
                "\n"
            };
            cmd.replace(end..end, eol);
        }
    }
    let after = Selection {
        anchor: a,
        caret: c,
        goal_col: None,
    };
    cmd.finish(hist, Kind::Other, sel, after)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::{redo, undo};
    use crate::eol::EolCounts;
    use crate::testutil::{Rng, check_change, check_eols};

    #[test]
    fn defaults() {
        let d = Prefs::default();
        assert_eq!(d.font_pt, 14.0);
        assert_eq!(d.tab_width, 4);
        assert!(d.auto_pair && d.auto_indent && d.line_numbers && d.highlight_line);
        assert!(d.restore_session);
        assert!(!d.trim_on_save && !d.final_newline_on_save && !d.minimap && !d.word_wrap);
        assert!(!d.indent_with_spaces);
        assert_eq!((d.indent_size, d.font_family.as_str()), (4, ""));
        assert_eq!(Prefs::parse(""), d);
        assert_eq!(Prefs::parse("garbage\n\n=\n[x]\n"), d);
    }

    #[test]
    fn parse_clamps_and_falls_back() {
        let p = Prefs::parse(
            "[preferences]\nfont_size=99\ntab_width=0\nindent_size=500\nauto_pair=0\nminimap=yes\n\
             trim_on_save=maybe\nfont_family=Fira%20Code\nunknown=1\nword_wrap=TRUE\n",
        );
        assert_eq!(p.font_pt, 32.0);
        assert_eq!((p.tab_width, p.indent_size), (1, 16));
        assert!(!p.auto_pair && p.minimap && p.word_wrap);
        assert!(!p.trim_on_save, "a bad value is the default");
        assert_eq!(p.font_family, "Fira Code");
        let p = Prefs::parse("font_size=abc\ntab_width=-3\nindent_size=x\nfont_size=nan\n");
        assert_eq!((p.font_pt, p.tab_width, p.indent_size), (14.0, 4, 4));
        assert_eq!(Prefs::parse("font_size=3").font_pt, 7.0);
        assert_eq!(Prefs::parse("font_size=inf").font_pt, 14.0);
        assert_eq!(Prefs::parse("font_size= 10.5 ").font_pt, 10.5);
        // The last of a repeated key wins; spaces around the key are ignored.
        assert_eq!(Prefs::parse("tab_width=2\n tab_width = 8\n").tab_width, 8);
        // A huge family name is dropped.
        let long = format!("font_family={}", "x".repeat(1000));
        assert_eq!(Prefs::parse(&long).font_family, "");
    }

    #[test]
    fn sections_are_respected() {
        let text =
            "[mdedit]\ntab_width=9\nwrap=1\n[preferences]\ntab_width=3\n[other]\ntab_width=7\n";
        assert_eq!(Prefs::parse(text).tab_width, 3);
        let none = "[mdedit]\ntab_width=9\n";
        assert_eq!(Prefs::parse(none).tab_width, 4);
        // Written beside a session in one file, the keys do not collide with the session's.
        let both = format!(
            "[mdedit]\nminimap=1\nzoom=120\n{}",
            Prefs::default().serialize()
        );
        assert!(!Prefs::parse(&both).minimap);
    }

    #[test]
    fn round_trip_is_stable() {
        let p = Prefs {
            font_family: "Cascadia Code, %weird\\".into(),
            font_pt: 11.5,
            tab_width: 8,
            detect_indent: false,
            indent_with_spaces: true,
            indent_size: 2,
            auto_pair: false,
            auto_indent: false,
            trim_on_save: true,
            final_newline_on_save: true,
            line_numbers: false,
            highlight_line: false,
            minimap: true,
            minimap_width: 150,
            word_wrap: true,
            restore_session: false,
            start_on_pinned: true,
            offer_stale: false,
            live_preview: false,
            complete_auto: true,
            backdrop: Backdrop::Acrylic,
            ai_connection: Connection::OpenAi,
            ai_review: false,
            ai_review_preview: true,
            ai_models: [
                "opus".into(),
                "".into(),
                "claude-fable-5-2".into(),
                "llama 4".into(),
            ],
            ai_efforts: ["max".into(), "minimal".into(), "".into(), "low".into()],
            ai_preset: "custom".into(),
            ai_base_url: "http://h:8080/v1?x=%1".into(),
            ai_claude_path: "C:\\Program Files\\claude.exe".into(),
            ai_codex_path: "/opt/codex = 2".into(),
        };
        let t = p.serialize();
        assert_eq!(Prefs::parse(&t), p);
        assert_eq!(Prefs::parse(&t).serialize(), t);
        let d = Prefs::default().serialize();
        assert_eq!(Prefs::parse(&d), Prefs::default());
        assert_eq!(Prefs::parse(&d).serialize(), d);
        // Out-of-range values are written clamped; every line is one line.
        let wild = Prefs {
            font_pt: f32::NAN,
            tab_width: 99,
            minimap_width: 5,
            font_family: "a\nb".into(),
            ai_base_url: "a\nb".into(),
            ..Prefs::default()
        };
        let t = wild.serialize();
        assert_eq!(t.lines().count(), 37);
        assert_eq!(Prefs::parse(&t).ai_base_url, "");
        assert_eq!(
            Prefs::parse("ai_preset=nope\nai_connection=x\n").ai_preset,
            "openai"
        );
        let r = Prefs::parse(&t);
        assert_eq!(
            (r.font_pt, r.tab_width, r.font_family.as_str()),
            (14.0, 16, "")
        );
        assert_eq!(r.minimap_width, MINIMAP_WIDTH_MIN);
        assert_eq!(
            Prefs::parse("[preferences]\nminimap_width=9999\n").minimap_width,
            MINIMAP_WIDTH_MAX
        );
        assert_eq!(
            Prefs::parse("[preferences]\nminimap_width=120\n").minimap_width,
            120
        );
        // Random fractions survive.
        let mut rng = Rng::new(9);
        for _ in 0..200 {
            let p = Prefs {
                font_pt: 7.0 + rng.below(2500) as f32 / 100.0,
                ..Prefs::default()
            };
            let q = Prefs::parse(&p.serialize());
            assert_eq!(q.font_pt, p.font_pt);
        }
    }

    #[test]
    fn hostile_text_never_panics() {
        let mut rng = Rng::new(2);
        let pieces = [
            "font_size",
            "=",
            "\n",
            "[",
            "]",
            "preferences",
            "%",
            "%zz",
            "é",
            "tab_width",
            "9999999999999999999999",
            "-",
            "\r\n",
            "nan",
        ];
        for _ in 0..2000 {
            let t: String = (0..rng.below(20)).map(|_| *rng.pick(&pieces)).collect();
            let p = Prefs::parse(&t);
            assert_eq!(Prefs::parse(&p.serialize()), p, "{t:?}");
        }
    }

    #[test]
    fn indent_and_assist_follow_the_preferences() {
        let mut p = Prefs::default();
        assert_eq!(p.indent(), Indent::Tab);
        p.indent_with_spaces = true;
        p.indent_size = 2;
        p.tab_width = 8;
        p.auto_pair = false;
        let a = p.assist("python");
        assert_eq!(a.indent, Indent::Spaces(2));
        assert_eq!(a.tab_width, 8);
        assert!(!a.auto_pair && a.auto_indent && a.colon_blocks);
        // A detected unit wins while detection is on, and only then.
        let a = p.assist_for("python", Some(Indent::Tab));
        assert_eq!(a.indent, Indent::Tab);
        p.detect_indent = false;
        assert_eq!(
            p.assist_for("python", Some(Indent::Tab)).indent,
            Indent::Spaces(2)
        );
        p.detect_indent = true;
        assert_eq!(p.assist_for("python", None).indent, Indent::Spaces(2));
    }

    fn save(text: &str, sel: Selection, trim: bool, nl: bool, eol: &str) -> (String, Outcome) {
        let mut buf = Buffer::from_text(text);
        let mut hist = History::new();
        let prefs = Prefs {
            trim_on_save: trim,
            final_newline_on_save: nl,
            ..Prefs::default()
        };
        let o = apply_on_save(&mut buf, &mut hist, sel, &prefs, eol);
        let after = buf.rope().to_string();
        match &o.change {
            None => {
                assert_eq!(after, text);
                assert!(!hist.can_undo());
                assert_eq!(o.sel, sel);
            }
            Some(c) => {
                check_change(text, &after, c);
                let mut counts = EolCounts::of(text);
                check_eols(&mut counts, &after, c);
                assert_eq!(hist.steps(), (1, 0), "one undo step");
                undo(&mut buf, &mut hist).unwrap();
                assert_eq!(buf.rope().to_string(), text);
                redo(&mut buf, &mut hist).unwrap();
                assert_eq!(buf.rope().to_string(), after);
            }
        }
        (after, o)
    }

    #[test]
    fn save_options() {
        let s = Selection::at(0);
        assert_eq!(save("a  \nb\t\n", s, true, false, "\n").0, "a\nb\n");
        assert_eq!(save("a\nb", s, false, true, "\n").0, "a\nb\n");
        assert_eq!(save("a \r\nb ", s, true, true, "\r\n").0, "a\r\nb\r\n");
        assert_eq!(save("a\rb", s, false, true, "\r").0, "a\rb\r");
        // Odd eol: LF. Empty document: stays empty. Already fine: nothing.
        assert_eq!(save("a", s, false, true, "xx").0, "a\n");
        assert_eq!(save("", s, true, true, "\n").0, "");
        assert_eq!(save("a\n", s, true, true, "\n").0, "a\n");
        assert_eq!(save("a  ", s, false, true, "\n").0, "a  \n");
        // Both off: nothing, whatever the text.
        assert_eq!(save("a  \nb", s, false, false, "\n").0, "a  \nb");
        // A last line of blanks after a break is trimmed, and the break stays the last thing.
        assert_eq!(save("a\n   ", s, true, true, "\n").0, "a\n");
        // The selection follows the text.
        let (_, o) = save(
            "a  \nb",
            Selection {
                anchor: 5,
                caret: 2,
                goal_col: None,
            },
            true,
            true,
            "\n",
        );
        assert_eq!((o.sel.anchor, o.sel.caret), (3, 1));
        // A caret at the very end stays before the added break.
        let (t, o) = save("ab", Selection::at(2), false, true, "\n");
        assert_eq!((t.as_str(), o.sel), ("ab\n", Selection::at(2)));
    }

    #[test]
    fn save_options_on_random_text() {
        let mut rng = Rng::new(4);
        let pieces = ["a", " ", "\t", "\n", "\r\n", "\r", "é"];
        for _ in 0..500 {
            let text: String = (0..rng.below(14)).map(|_| *rng.pick(&pieces)).collect();
            let (trim, nl) = (rng.chance(60), rng.chance(60));
            let (t, _) = save(
                &text,
                Selection::at(rng.below(text.len() + 1)),
                trim,
                nl,
                "\n",
            );
            if nl && !t.is_empty() {
                assert!(t.ends_with(['\n', '\r']));
            }
            if trim {
                // No blanks before a line end, except that a lone CR then LF cannot appear.
                assert!(!t.contains(" \n") && !t.contains("\t\n") && !t.contains(" \r"));
            }
        }
    }
}
