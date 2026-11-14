//! The status strip under the text: position, selection, syntax, line endings, encoding, and the
//! load progress on the left. 26 logical px across the window's bottom.

use crate::chrome::{Ctx, glyph};
use crate::docs::{DocState, Docs};
use crate::ui::{Act, Hit};
use mdedit_core::{Encoding, LineEnding};
use mdshared_ui::gfx::{Face, Rect};
use mdshared_ui::menu::{Item, Menu};

pub const STATUS_H: f32 = 26.0;
/// Between two right-aligned fields.
const GAP: f32 = 24.0;
/// Up to this many bytes a selection is counted in characters, beyond it in bytes.
pub const COUNT_CHARS_UP_TO: usize = 4 << 20;

/// How a selection's length is shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Selected {
    Chars(usize),
    Bytes(usize),
}

/// "12.4 MB", "820 KB", "17 bytes": decimal units, as file managers show sizes.
pub fn size_text(bytes: u64) -> String {
    const K: f64 = 1000.0;
    let b = bytes as f64;
    if bytes < 1000 {
        format!("{bytes} bytes")
    } else if b < K * K {
        format!("{:.0} KB", b / K)
    } else if b < K * K * K {
        format!("{:.1} MB", b / (K * K))
    } else {
        format!("{:.2} GB", b / (K * K * K))
    }
}

/// "Ln 12, Col 5", with " (3 selected)" or " (12.4 MB selected)".
pub fn position_text(line: usize, col: usize, selected: Option<Selected>) -> String {
    let mut s = format!("Ln {line}, Col {col}");
    match selected {
        Some(Selected::Chars(n)) if n > 0 => s.push_str(&format!(" ({n} selected)")),
        Some(Selected::Bytes(n)) if n > 0 => {
            s.push_str(&format!(" ({} selected)", size_text(n as u64)))
        }
        _ => {}
    }
    s
}

/// The language name or "Plain text", with ", not coloured" when the file is over its limit.
pub fn language_text(name: Option<&str>, over_limit: bool) -> String {
    match (name, over_limit) {
        (Some(n), false) => n.to_string(),
        (Some(n), true) => format!("{n}, not coloured"),
        (None, _) => "Plain text".to_string(),
    }
}

/// The encoding's label, with ", invalid bytes replaced" for a lossy file or ", contains NUL"
/// for one with NUL characters (most likely not a text file).
pub fn encoding_text(encoding: Encoding, lossy: bool, nul: bool) -> String {
    let label = encoding.label();
    if lossy {
        format!("{label}, invalid bytes replaced")
    } else if nul {
        format!("{label}, contains NUL")
    } else {
        label
    }
}

pub fn eol_text(eol: LineEnding) -> &'static str {
    eol.label()
}

/// "Loading 42%".
pub fn loading_text(done: u64, total: u64) -> String {
    let pct = (done.min(total) * 100).checked_div(total).unwrap_or(0) as u32;
    format!("Loading {pct}%")
}

/// What the strip shows for the active tab: left text and right-aligned fields, left to right.
pub fn fields(docs: &Docs) -> (String, Vec<String>) {
    let Some(doc) = docs.active_doc() else {
        return (String::new(), Vec::new());
    };
    match &doc.state {
        DocState::Pending => (String::new(), Vec::new()),
        DocState::Loading => {
            let (done, total) = doc.progress();
            (loading_text(done, total), Vec::new())
        }
        DocState::Failed(_) => (String::new(), Vec::new()),
        DocState::Ready(r) => {
            let buf = &r.buffer;
            let pos = buf.position(r.view.sel.caret);
            let range = r.view.sel.range();
            let selected = (!range.is_empty()).then(|| {
                if range.len() <= COUNT_CHARS_UP_TO {
                    let mut n = 0;
                    for piece in buf.chunks(range.clone()) {
                        n += piece.chars().count();
                    }
                    Selected::Chars(n)
                } else {
                    Selected::Bytes(range.len())
                }
            });
            let left = if doc.is_loading() {
                let (done, total) = doc.progress();
                loading_text(done, total)
            } else {
                r.view
                    .align
                    .as_ref()
                    .map(|a| a.summary())
                    .unwrap_or_default()
            };
            let mut right = Vec::new();
            if docs.recording {
                right.push("Recording".to_string());
            }
            if docs.links.contains(doc.id) {
                right.push("Previewing".to_string());
            }
            if docs.zoom != 100 {
                right.push(format!("{}%", docs.zoom));
            }
            if !r.view.extra.is_empty() {
                right.push(format!("{} carets", r.view.extra.len() + 1));
            }
            right.extend([
                position_text(pos.line + 1, pos.col + 1, selected),
                language_text(
                    r.language.map(|l| l.name()),
                    r.language.is_some() && !r.colourable,
                ),
                eol_text(r.info.eol.style()).to_string(),
                encoding_text(r.info.encoding, r.info.lossy, r.info.has_nul),
            ]);
            (left, right)
        }
    }
}

/// The menu of the line-ending field: the three kinds, the document's own checked.
pub fn eol_menu(docs: &Docs, x: f32, y: f32) -> Menu<Act> {
    let style = docs.active_ready().map(|r| r.info.eol.style());
    let item = |kind: LineEnding, eol: &'static str| {
        Item::new(Act::Eol(eol), kind.label())
            .icon(glyph::EOL)
            .checked(style == Some(kind))
    };
    let items = vec![
        item(LineEnding::Crlf, "\r\n"),
        item(LineEnding::Lf, "\n"),
        item(LineEnding::Cr, "\r"),
    ];
    Menu::new(items, x, y, true).above()
}

/// The encodings offered: the Unicode ones and the system's code page.
pub fn encodings(docs: &Docs) -> [Encoding; 5] {
    [
        Encoding::Utf8,
        Encoding::Utf8Bom,
        Encoding::Utf16Le,
        Encoding::Utf16Be,
        docs.legacy(),
    ]
}

/// The menu of the encoding field: save in an encoding (the document's own checked), or read
/// the file again as one.
pub fn encoding_menu(docs: &Docs, x: f32, y: f32) -> Menu<Act> {
    let doc = docs.active_doc();
    let current = docs.active_ready().map(|r| r.info.encoding);
    let lossy = docs.active_ready().is_some_and(|r| r.info.lossy);
    let forced = doc.is_some_and(|d| d.forced.is_some());
    let has_file = doc.is_some_and(|d| d.path.is_some());
    let mut items = Vec::new();
    for e in encodings(docs) {
        items.push(
            Item::new(Act::SaveIn(e), format!("Save in {}", e.label()))
                .icon(glyph::SAVE)
                .checked(current == Some(e))
                .enabled(!lossy),
        );
    }
    for (k, e) in encodings(docs).into_iter().enumerate() {
        let text = format!("Reopen as {}", e.label());
        let item = Item::new(Act::ReadAs(Some(e)), text)
            .icon(glyph::RELOAD)
            .enabled(has_file);
        items.push(if k == 0 { item.sep() } else { item });
    }
    // Back to what the file's bytes suggest, after a choice.
    items.push(
        Item::new(Act::ReadAs(None), "Reopen, detecting the encoding")
            .icon(glyph::SEARCH)
            .enabled(has_file && forced),
    );
    Menu::new(items, x, y, true).above()
}

pub fn draw(c: &mut Ctx, docs: &Docs, r: Rect) {
    let pal = *c.pal;
    c.p.fill(r, pal.bar);
    let px = c.p.px();
    c.p.fill(Rect::new(r.x, r.y, r.w, px), pal.line);
    let (left, right) = fields(docs);
    let cy = r.y + r.h / 2.0 + 0.5;
    let mut x = r.right() - 16.0;
    let mut first = true;
    // The last two fields of a document that can be edited open menus: encoding, line endings.
    // A lossy reading cannot be edited, but can be read again as another encoding.
    let lossy = docs.active_ready().map(|r| r.info.lossy);
    let hits = [
        lossy.is_some().then_some(Hit::StatusEncoding),
        (lossy == Some(false)).then_some(Hit::StatusEol),
    ];
    for (k, f) in right.iter().rev().enumerate() {
        if !first {
            x -= GAP;
        }
        first = false;
        let hit = hits.get(k).copied().flatten();
        let w = c.p.measure(Face::Ui, 12.0, f);
        if let Some(hit) = hit {
            let b = Rect::new(x - w - 8.0, r.y + 3.0, w + 16.0, r.h - 5.0);
            if let Some(fill) = c.fill_for(hit, false, false) {
                c.p.rbox(b, 4.0, fill);
            }
            c.hit(hit, b);
        }
        c.p.text_right(Face::Ui, 12.0, x, cy, f, pal.muted);
        x -= w;
    }
    if !left.is_empty() {
        c.p.text(
            Face::Ui,
            12.0,
            r.x + 16.0,
            cy,
            &left,
            pal.muted,
            (x - r.x - 32.0).max(0.0),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mdedit_core::encoding::legacy_for_code_page;

    #[test]
    fn sizes() {
        assert_eq!(size_text(17), "17 bytes");
        assert_eq!(size_text(820_000), "820 KB");
        assert_eq!(size_text(12_400_000), "12.4 MB");
        assert_eq!(size_text(104_857_685), "104.9 MB");
        assert_eq!(size_text(2_500_000_000), "2.50 GB");
    }

    #[test]
    fn position_and_selection() {
        assert_eq!(position_text(1, 1, None), "Ln 1, Col 1");
        assert_eq!(
            position_text(12, 5, Some(Selected::Chars(3))),
            "Ln 12, Col 5 (3 selected)"
        );
        assert_eq!(
            position_text(1, 1, Some(Selected::Bytes(12_400_000))),
            "Ln 1, Col 1 (12.4 MB selected)"
        );
        assert_eq!(position_text(2, 3, Some(Selected::Chars(0))), "Ln 2, Col 3");
    }

    #[test]
    fn encodings() {
        assert_eq!(encoding_text(Encoding::Utf8, false, false), "UTF-8");
        assert_eq!(
            encoding_text(Encoding::Utf8Bom, false, false),
            "UTF-8 with BOM"
        );
        assert_eq!(encoding_text(Encoding::Utf16Le, false, false), "UTF-16 LE");
        assert_eq!(encoding_text(Encoding::Utf16Be, false, false), "UTF-16 BE");
        assert_eq!(
            encoding_text(Encoding::Legacy(legacy_for_code_page(1254)), false, false),
            "Windows-1254"
        );
        assert_eq!(
            encoding_text(Encoding::Utf16Le, true, false),
            "UTF-16 LE, invalid bytes replaced"
        );
        assert_eq!(
            encoding_text(Encoding::Utf8, false, true),
            "UTF-8, contains NUL"
        );
        assert_eq!(
            encoding_text(Encoding::Utf8, true, true),
            "UTF-8, invalid bytes replaced"
        );
        assert_eq!(
            encoding_text(Encoding::Utf8Bom, true, false),
            "UTF-8 with BOM, invalid bytes replaced"
        );
    }

    #[test]
    fn line_endings() {
        assert_eq!(eol_text(LineEnding::Crlf), "Windows (CRLF)");
        assert_eq!(eol_text(LineEnding::Lf), "Unix (LF)");
        assert_eq!(eol_text(LineEnding::Cr), "Macintosh (CR)");
        assert_eq!(eol_text(LineEnding::Mixed), "Mixed");
        let none = eol_text(LineEnding::None);
        assert!(none == "Windows (CRLF)" || none == "Unix (LF)");
    }

    #[test]
    fn languages_and_loading() {
        assert_eq!(language_text(Some("Rust"), false), "Rust");
        assert_eq!(language_text(Some("Rust"), true), "Rust, not coloured");
        assert_eq!(language_text(None, true), "Plain text");
        assert_eq!(loading_text(50, 200), "Loading 25%");
        assert_eq!(loading_text(5, 0), "Loading 0%");
        assert_eq!(loading_text(500, 200), "Loading 100%");
    }
}
