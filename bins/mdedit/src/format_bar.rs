//! The format bar: a row under the command bar, for Markdown documents, with a button for each
//! Markdown command, mdview's features included (alerts, math, diagrams, footnotes, collapsible
//! sections). A button shows pressed when the caret is in what it makes: a bold span, a heading
//! of its level, a list of its kind, a quote. Where the row is too narrow the groups at its end
//! give way to a button that opens the Markdown menu, which has every command.

use crate::chrome::Ctx;
use crate::commands::{CommandId as C, command, hint};
use crate::docs::Docs;
use crate::ui::{Hit, PopupKind};
use mdedit_core::blocks::{self, ListKind};
use mdedit_core::markdown;
use mdshared_ui::gfx::{Face, Rect};
use mdshared_ui::glyph;

/// The row's height.
pub const FORMAT_H: f32 = 36.0;
const BTN: f32 = 30.0;
const BTN_H: f32 = 28.0;
const GAP: f32 = 2.0;
const SEP: f32 = 13.0;
const MARGIN: f32 = 8.0;

/// The icons the shared set does not have, on its 16-unit grid.
mod icon {
    pub const NUMBERED: &str =
        "M2.6 3.2l.9-.7v3.5M2.2 9.8a.9 .9 0 1 1 1.6 .6L2.2 12.5h1.9M6.5 4h7M6.5 8h7M6.5 12h7";
    pub const QUOTE: &str = "M2.5 3v10M5.5 4.5h8M5.5 8h8M5.5 11.5h5";
    pub const CODE_BLOCK: &str = "M2 2.5h12a.5 .5 0 0 1 .5 .5v10a.5 .5 0 0 1-.5 .5H2a.5 .5 0 0 1-.5-.5V3a.5 .5 0 0 1 .5-.5zM6.2 6L4.5 8l1.7 2M9.8 6l1.7 2-1.7 2";
    pub const MATH: &str = "M12 3.5H4l4 4.5-4 4.5h8";
    pub const MATH_BLOCK: &str = "M2 2.5h12a.5 .5 0 0 1 .5 .5v10a.5 .5 0 0 1-.5 .5H2a.5 .5 0 0 1-.5-.5V3a.5 .5 0 0 1 .5-.5zM10.5 5H5.5l2.5 3-2.5 3h5";
    pub const DIAGRAM: &str = "M2 2.5h4v3H2zM10 2.5h4v3h-4zM6 10.5h4v3H6zM4 5.5V8h8V5.5M8 8v2.5";
    pub const FOOTNOTE: &str = "M2.5 6l5 6.5M7.5 6l-5 6.5M10.8 3.6l1.4-1.1v4.5";
    pub const DETAILS: &str = "M2.5 3.5l2.5 2-2.5 2zM7 5.5h6.5M4.5 9.5h9M4.5 12.5h6";
    pub const RULE: &str = "M1.5 8h13M4 4.5h8M4 11.5h8";
}

/// What a button shows: an icon or a short text.
#[derive(Clone, Copy)]
enum Look {
    Icon(&'static str),
    Text(&'static str),
}

/// The buttons, in groups; a separator stands between groups.
const GROUPS: &[&[(C, Look)]] = &[
    &[
        (C::MdBold, Look::Icon(glyph::BOLD)),
        (C::MdItalic, Look::Icon(glyph::ITALIC)),
        (C::MdStrike, Look::Icon(glyph::STRIKE)),
        (C::MdCode, Look::Icon(glyph::CODE)),
    ],
    &[
        (C::MdHeading1, Look::Text("H1")),
        (C::MdHeading2, Look::Text("H2")),
        (C::MdHeading3, Look::Text("H3")),
    ],
    &[
        (C::MdBulletList, Look::Icon(glyph::OUTLINE)),
        (C::MdNumberedList, Look::Icon(icon::NUMBERED)),
        (C::MdTaskList, Look::Icon(glyph::TASK)),
        (C::MdQuote, Look::Icon(icon::QUOTE)),
        (C::MdAlert, Look::Icon(glyph::INFO)),
    ],
    &[
        (C::MdLink, Look::Icon(glyph::LINK)),
        (C::MdImage, Look::Icon(glyph::IMAGE)),
        (C::MdTable, Look::Icon(glyph::TABLE)),
        (C::MdCodeBlock, Look::Icon(icon::CODE_BLOCK)),
    ],
    &[
        (C::MdMath, Look::Icon(icon::MATH)),
        (C::MdMathBlock, Look::Icon(icon::MATH_BLOCK)),
        (C::MdDiagram, Look::Icon(icon::DIAGRAM)),
        (C::MdFootnote, Look::Icon(icon::FOOTNOTE)),
        (C::MdDetails, Look::Icon(icon::DETAILS)),
        (C::MdRule, Look::Icon(icon::RULE)),
    ],
];

/// What shows pressed: from the caret of the active document.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct State {
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub code: bool,
    pub math: bool,
    pub line: blocks::LineState,
}

impl State {
    fn on(&self, id: C) -> bool {
        let l = &self.line;
        match id {
            C::MdBold => self.bold,
            C::MdItalic => self.italic,
            C::MdStrike => self.strike,
            C::MdCode => self.code,
            C::MdMath => self.math,
            C::MdHeading1 => l.heading == 1,
            C::MdHeading2 => l.heading == 2,
            C::MdHeading3 => l.heading == 3,
            C::MdBulletList => l.list == Some(ListKind::Bullet),
            C::MdNumberedList => l.list == Some(ListKind::Numbered),
            C::MdTaskList => l.list == Some(ListKind::Task),
            C::MdQuote => l.quote,
            _ => false,
        }
    }
}

/// The bar's state when it shows: it is on, and the active document is Markdown and loaded.
pub fn shown(docs: &Docs) -> Option<State> {
    if !docs.format_bar {
        return None;
    }
    let r = docs.active_ready().filter(|r| r.assist.markdown)?;
    let (buf, caret) = (&r.buffer, r.view.sel.caret);
    Some(State {
        bold: markdown::in_span(buf, caret, "**"),
        italic: markdown::in_span(buf, caret, "*") || markdown::in_span(buf, caret, "_"),
        strike: markdown::in_span(buf, caret, "~~"),
        code: markdown::in_span(buf, caret, "`"),
        math: markdown::in_span(buf, caret, "$"),
        line: blocks::line_state(buf, caret),
    })
}

/// How many groups fit in `w`, and whether the overflow button is needed.
fn fit(w: f32) -> (usize, bool) {
    let group_w = |g: &[(C, Look)]| g.len() as f32 * (BTN + GAP);
    let total: f32 = GROUPS.iter().map(|g| group_w(g)).sum::<f32>()
        + SEP * (GROUPS.len() - 1) as f32
        + 2.0 * MARGIN;
    if total <= w {
        return (GROUPS.len(), false);
    }
    let room = w - 2.0 * MARGIN - BTN - SEP;
    let mut used = 0.0;
    let mut n = 0;
    for (i, g) in GROUPS.iter().enumerate() {
        let need = group_w(g) + if i > 0 { SEP } else { 0.0 };
        if used + need > room {
            break;
        }
        used += need;
        n += 1;
    }
    (n, true)
}

/// Draws the bar into `r`.
pub fn draw(c: &mut Ctx, state: &State, r: Rect) {
    let pal = *c.pal;
    c.p.fill(r, pal.bar);
    let y = r.y + (FORMAT_H - BTN_H) / 2.0 - 2.0;
    let cy = y + BTN_H / 2.0;
    let (groups, more) = fit(r.w);
    let mut x = r.x + MARGIN;
    for (i, g) in GROUPS.iter().take(groups).enumerate() {
        if i > 0 {
            c.p.fill(
                Rect::new((x + SEP / 2.0).round(), y + 5.0, c.p.px(), BTN_H - 10.0),
                pal.line,
            );
            x += SEP;
        }
        for &(id, face) in g.iter() {
            let b = Rect::new(x, y, BTN, BTN_H);
            let hit = Hit::Format(id);
            let on = state.on(id);
            if let Some(f) = c.fill_for(hit, on, false) {
                c.p.rbox(b, 6.0, f);
            }
            let color = if on { pal.accent } else { pal.text };
            match face {
                Look::Icon(g) => c.p.icon(g, 15.0, b.cx(), cy, color),
                Look::Text(t) => c.p.text_center(Face::Semibold, 12.5, b, t, color),
            }
            c.hit(hit, b);
            x += BTN + GAP;
        }
    }
    if more {
        let b = Rect::new(r.right() - MARGIN - BTN, y, BTN, BTN_H);
        let hit = Hit::FormatMore;
        if let Some(f) = c.fill_for(hit, false, false) {
            c.p.rbox(b, 6.0, f);
        }
        c.p.icon(glyph::MORE, 15.0, b.cx(), cy, pal.text);
        c.hit(hit, b);
    }
    // A hairline under the row, as under the command bar.
    c.p.fill(
        Rect::new(r.x, r.bottom() - c.p.px(), r.w, c.p.px()),
        pal.line,
    );
}

/// The tooltip of a button: the command's name and its shortcut.
pub fn tooltip(id: C) -> String {
    let label = match id {
        C::MdAlert => "Alert: again for the next kind (note, tip, important, warning, caution)",
        C::MdFootnote => "Footnote: a number here, its text at the end",
        C::MdDiagram => "Diagram (Mermaid)",
        C::MdMath => "Math ($\u{2026}$)",
        C::MdMathBlock => "Math block ($$)",
        C::MdDetails => "Collapsible section",
        C::MdTable => "Align table",
        id => command(id).label,
    };
    let k = hint(id);
    if k.is_empty() {
        label.to_string()
    } else {
        format!("{label} ({k})")
    }
}

/// The overflow button's menu.
pub const MORE_MENU: PopupKind = PopupKind::Markdown;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn narrow_rows_keep_the_first_groups_and_an_overflow_button() {
        assert_eq!(fit(2000.0), (GROUPS.len(), false));
        let (n, more) = fit(500.0);
        assert!(more && n >= 2 && n < GROUPS.len());
        // The window's minimum width still has the inline marks.
        assert!(fit(520.0).0 >= 1);
    }

    #[test]
    fn every_icon_draws_inside_its_square() {
        use mdshared_ui::gfx::{Gfx, Painter};
        let mut gfx = Gfx::new();
        for g in [
            icon::NUMBERED,
            icon::QUOTE,
            icon::CODE_BLOCK,
            icon::MATH,
            icon::MATH_BLOCK,
            icon::DIAGRAM,
            icon::FOOTNOTE,
            icon::DETAILS,
            icon::RULE,
        ] {
            // 16 units at 4x, centred in 96 px: ink only inside the middle 64.
            let mut bmp = mdshared_text::Bitmap::new(96, 96);
            Painter::new(&mut bmp, &mut gfx, 4.0).icon(g, 16.0, 12.0, 12.0, [255, 255, 255]);
            let ink = |x: usize, y: usize| bmp.pixels[(y * 96 + x) * 4] > 0;
            let mut any = false;
            for y in 0..96 {
                for x in 0..96 {
                    if ink(x, y) {
                        any = true;
                        assert!(
                            (16..80).contains(&x) && (16..80).contains(&y),
                            "{g} at {x},{y}"
                        );
                    }
                }
            }
            assert!(any, "{g} has no ink");
        }
    }
}
