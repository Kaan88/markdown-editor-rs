//! The layout engine and the drawing of the mocked Settings dialog. It lays out the shared
//! `Row`s (the real data model of the form) page by page and draws them with the same shapes,
//! colours and fonts as `mdshared_ui::settings`; the control drawing below is a copy of that
//! file's private code, so a control here is the one the app draws today.
//!
//! Every x and y in the dialog comes from `Geo` and `layout`, and the audit in `fit` reads the
//! same rectangles the drawing uses, so a column rule that the report says holds is the one on
//! the picture.

use mdshared_ui::gfx::{Face, Painter, Rect, Rgb, mix};
use mdshared_ui::glyph;
use mdshared_ui::overlay::draw_dialog_button;
use mdshared_ui::settings::Row;
use mdshared_ui::textfield::{FieldColors, TextField};
use mdshared_ui::theme::Palette;

// ---- the numbers the design proposes ----------------------------------------------------------

/// The card at its largest, and the margin between it and the window.
pub const CARD_W: f32 = 820.0;
pub const CARD_H: f32 = 460.0;
pub const MARGIN: f32 = 6.0;
/// The strip above both columns: the title at the left, the close button at the right.
pub const HEADER_H: f32 = 40.0;
/// The one inset of the card: the title, the list's text, the page's text and the controls' right
/// edge are all this far from the card's left or right edge (the list's text and the page's text
/// from their own column's edge).
pub const PAD: f32 = 24.0;
/// The category list: the highlighted row's width, the gap on each side of it, the item height.
pub const NAV_W: f32 = 168.0;
pub const NAV_W_SMALL: f32 = 112.0;
pub const NAV_GAP: f32 = 8.0;
pub const NAV_NARROW_BELOW: f32 = 640.0;
pub const NAV_ITEM: f32 = 34.0;
pub const NAV_ITEM_SMALL: f32 = 28.0;
/// The accent mark: its inset from the highlight's left edge, its size; the text's inset.
pub const MARK_INSET: f32 = 4.0;
pub const MARK_W: f32 = 3.0;
pub const MARK_H: f32 = 16.0;
pub const NAV_TEXT_INSET: f32 = 16.0;
/// Space above the first and below the last row of a page.
pub const PAGE_TOP: f32 = 4.0;
pub const PAGE_BOTTOM: f32 = 4.0;
/// The row heights. A page uses `ROW_H` for rows without help, then `BESIDE_H` for rows with
/// help when every help line of the page fits beside its label, else `UNDER_H` for all of them
/// (the page decides, never the row). Each further help line adds `HELP_LINE`.
pub const ROW_H: f32 = 34.0;
pub const BESIDE_H: f32 = 40.0;
pub const UNDER_H: f32 = 45.0;
pub const HELP_LINE: f32 = 14.0;
/// A text field on its own line (paths, addresses, keys): the label, the field with its button at
/// the right on the field's line, then the help.
pub const STACK_LABEL_H: f32 = 24.0;
pub const STACK_GAP: f32 = 2.0;
pub const STACK_BOTTOM: f32 = 6.0;
pub const GAP_H: f32 = 8.0;
pub const CHILD_INDENT: f32 = 16.0;
/// The control column: the width of a segmented group, a dropdown, an inline text field. 240 in a
/// card at least 640 wide, else 192. Right-aligned: its right edge is the one every control ends at.
pub const COL_W: f32 = 240.0;
pub const COL_W_SMALL: f32 = 192.0;
pub const CONTROL_H: f32 = 28.0;
pub const TOGGLE_W: f32 = 40.0;
pub const TOGGLE_H: f32 = 20.0;
pub const STEPPER_W: f32 = 164.0;
pub const BUTTON_W: f32 = 96.0;
pub const ICON_W: f32 = 28.0;
pub const ACC_GAP: f32 = 8.0;
const LABEL_GAP: f32 = 12.0;

// ---- the data ---------------------------------------------------------------------------------

/// A row with the few things the design adds to the shared `Row`.
#[derive(Clone)]
pub struct R {
    pub row: Row,
    /// Indented under the row above it (a setting that only matters while that one is on).
    pub child: bool,
    /// Disabled; the reason takes the place of the help line.
    pub off: Option<String>,
    /// A text field on its own line, full width (paths, addresses, keys).
    pub stack: bool,
    /// A button belonging to the control: a label, or `icon:delete` / `icon:reload`.
    pub button: Option<(String, bool)>,
    /// The keyboard focus is here: a ring, or the accent underline and a caret in a text field.
    pub focus: bool,
    /// The help line of a row that has none of its own (an action).
    pub note: Option<String>,
}

impl R {
    pub fn new(row: Row) -> R {
        R {
            row,
            child: false,
            off: None,
            stack: false,
            button: None,
            focus: false,
            note: None,
        }
    }
    pub fn note(mut self, t: &str) -> R {
        self.note = Some(t.to_string());
        self
    }
    pub fn child(mut self) -> R {
        self.child = true;
        self
    }
    pub fn off(mut self, why: &str) -> R {
        self.off = Some(why.to_string());
        self
    }
    pub fn stack(mut self) -> R {
        self.stack = true;
        self
    }
    pub fn btn(mut self, label: &str, enabled: bool) -> R {
        self.button = Some((label.to_string(), enabled));
        self
    }
    pub fn focus(mut self) -> R {
        self.focus = true;
        self
    }
}

#[derive(Clone)]
pub enum Item {
    /// A gap between two groups.
    Gap,
    /// Something the page draws itself, this tall.
    Custom(f32, u8),
    Row(R),
}

#[derive(Clone)]
pub struct Page {
    pub name: String,
    pub items: Vec<Item>,
    /// Some of its help is written at run time (what the server said, where the text goes):
    /// the page always puts help under the row, whatever it says now.
    pub dynamic: bool,
}

pub struct Opts {
    pub current: usize,
    /// The keyboard is in the category list: its current item has a ring.
    pub nav_focus: bool,
    /// A page shorter than the area is centred in it (the alternative to the top).
    pub center: bool,
}

// ---- geometry ---------------------------------------------------------------------------------

#[derive(Clone, Copy)]
pub struct Geo {
    pub card: Rect,
    /// The highlighted row of the list, its first item's top and the item height.
    pub nav: Rect,
    pub item_h: f32,
    pub divider_x: f32,
    pub page: Rect,
    /// The page's text starts here, its controls end at `r`.
    pub l: f32,
    pub r: f32,
    pub col: f32,
}

pub fn shape(w: f32, h: f32) -> Geo {
    let cw = (w - 2.0 * MARGIN).min(CARD_W);
    let ch = (h - 2.0 * MARGIN).min(CARD_H);
    let card = Rect::new(((w - cw) / 2.0).round(), ((h - ch) / 2.0).round(), cw, ch);
    let wide = cw >= NAV_NARROW_BELOW;
    let nav_w = if wide { NAV_W } else { NAV_W_SMALL };
    let item_h = if ch < 400.0 { NAV_ITEM_SMALL } else { NAV_ITEM };
    let top = card.y + HEADER_H;
    // The first item is centred on the first plain row of the page.
    let nav = Rect::new(
        card.x + NAV_GAP,
        top + PAGE_TOP + (ROW_H - item_h) / 2.0,
        nav_w,
        card.bottom() - top,
    );
    let divider_x = nav.right() + NAV_GAP;
    let px = divider_x + 1.0;
    let page = Rect::new(px, top, card.right() - px, card.bottom() - top);
    Geo {
        card,
        nav,
        item_h,
        divider_x,
        page,
        l: px + PAD,
        r: card.right() - PAD,
        col: if wide { COL_W } else { COL_W_SMALL },
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Kind {
    Toggle,
    Seg,
    Drop,
    Text,
    Step,
    Action,
    Stack,
}

/// A laid-out row: every rectangle absolute in x, relative to the page's top in y.
#[derive(Clone)]
pub struct Placed {
    pub y: f32,
    pub h: f32,
    pub kind: Option<Kind>,
    pub label_x: f32,
    pub label_cy: f32,
    pub label_w: f32,
    pub ctrl: Rect,
    pub acc: Option<Rect>,
    pub help: Vec<String>,
    pub help_x: f32,
    pub help_y: f32,
    pub help_w: f32,
    #[allow(dead_code)]
    pub under: bool,
    pub cut: bool,
}

fn wrap(p: &mut Painter, text: &str, w: f32, max: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut used = 0;
    for (i, word) in words.iter().enumerate() {
        let next = if cur.is_empty() {
            word.to_string()
        } else {
            format!("{cur} {word}")
        };
        if cur.is_empty() || p.measure(Face::Ui, 11.5, &next) <= w {
            cur = next;
            used = i + 1;
        } else if lines.len() + 1 == max {
            let rest = words[used - cur.split(' ').count()..].join(" ");
            lines.push(p.elide(Face::Ui, 11.5, &rest, w));
            return lines;
        } else {
            lines.push(std::mem::take(&mut cur));
            cur = word.to_string();
            used = i + 1;
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

fn kind_of(r: &R) -> Kind {
    if r.stack {
        return Kind::Stack;
    }
    match &r.row {
        Row::Toggle { .. } => Kind::Toggle,
        // Up to three options sit in a segmented group, more in a dropdown: the number of
        // options, not their text, decides, so a page does not change shape with a translation
        // or a model that offers one more level.
        Row::Choice { options, .. } if options.len() <= 3 => Kind::Seg,
        Row::Choice { .. } => Kind::Drop,
        Row::Number { .. } => Kind::Step,
        Row::Text { .. } => Kind::Text,
        Row::Action { .. } => Kind::Action,
        Row::Header(_) => Kind::Toggle,
    }
}

pub fn number_text(v: i32, unit: &str) -> String {
    if unit.is_empty() {
        v.to_string()
    } else {
        format!("{v} {unit}")
    }
}

fn help_text(r: &R) -> String {
    match &r.row {
        Row::Toggle { help, .. }
        | Row::Choice { help, .. }
        | Row::Number { help, .. }
        | Row::Text { help, .. } => help.clone(),
        _ => r.note.clone().unwrap_or_default(),
    }
}

fn help_of(r: &R) -> String {
    match &r.off {
        Some(why) => why.clone(),
        None => match &r.row {
            Row::Toggle { help, .. }
            | Row::Choice { help, .. }
            | Row::Number { help, .. }
            | Row::Text { help, .. } => help.clone(),
            _ => r.note.clone().unwrap_or_default(),
        },
    }
}

pub fn label_of(row: &Row) -> &str {
    match row {
        Row::Header(t) => t,
        Row::Toggle { label, .. }
        | Row::Choice { label, .. }
        | Row::Number { label, .. }
        | Row::Text { label, .. }
        | Row::Action { label, .. } => label,
    }
}

fn acc_w(label: &str) -> f32 {
    if label.starts_with("icon:") {
        ICON_W
    } else {
        BUTTON_W
    }
}

/// The control of a row, right-aligned, vertically centred on `cy`; and its button, if any.
fn control_rects(r: &R, g: &Geo, cy: f32) -> (Rect, Option<Rect>) {
    let kind = kind_of(r);
    let h = if kind == Kind::Toggle {
        TOGGLE_H
    } else {
        CONTROL_H
    };
    let w = match kind {
        Kind::Toggle => TOGGLE_W,
        Kind::Step => STEPPER_W,
        Kind::Action => BUTTON_W,
        _ => g.col,
    };
    let acc = r.button.as_ref().map(|(l, _)| {
        let aw = acc_w(l);
        Rect::new(g.r - aw, (cy - CONTROL_H / 2.0).round(), aw, CONTROL_H)
    });
    // With a button the control gives up room for it and stays in the column's left edge.
    let w_used = match (&acc, kind) {
        (Some(a), Kind::Seg | Kind::Drop | Kind::Text) => g.col - a.w - ACC_GAP,
        _ => w,
    };
    let x = match (&acc, kind) {
        (Some(_), Kind::Seg | Kind::Drop | Kind::Text) => g.r - g.col,
        _ => g.r - w,
    };
    (
        Rect::new(x.round(), (cy - h / 2.0).round(), w_used.round(), h),
        acc,
    )
}

/// Every item's rectangles for a page; and the page's content height. `under` is the page's
/// help rule: help under its row (true) or beside its label (false).
pub fn layout(p: &mut Painter, page: &Page, g: &Geo) -> (Vec<Placed>, f32, bool) {
    // The page's rule first: help goes beside its label only when every line of the page fits
    // there, because the page decides, not the row.
    let mut under = page.dynamic;
    for it in &page.items {
        let Item::Row(r) = it else { continue };
        if r.stack {
            continue;
        }
        let indent = if r.child { CHILD_INDENT } else { 0.0 };
        let (ctrl, acc) = control_rects(r, g, 0.0);
        let left = acc.map_or(ctrl.x, |_| ctrl.x).min(ctrl.x);
        let room = left - (g.l + indent) - LABEL_GAP;
        // Both what the row says and what it says while disabled, so switching a setting does
        // not re-lay the page.
        for help in [help_text(r), r.off.clone().unwrap_or_default()] {
            if p.measure(Face::Ui, 11.5, &help) > room {
                under = true;
            }
        }
    }
    let mut out: Vec<Placed> = Vec::new();
    let mut y = PAGE_TOP;
    for it in &page.items {
        let Item::Row(r) = it else {
            out.push(Placed {
                y,
                h: gap_of(it),
                kind: None,
                label_x: 0.0,
                label_cy: 0.0,
                label_w: 0.0,
                ctrl: Rect::new(0.0, 0.0, 0.0, 0.0),
                acc: None,
                help: Vec::new(),
                help_x: 0.0,
                help_y: 0.0,
                help_w: 0.0,
                under: false,
                cut: false,
            });
            y += gap_of(it);
            continue;
        };
        let kind = kind_of(r);
        let indent = if r.child { CHILD_INDENT } else { 0.0 };
        let lx = g.l + indent;
        let help_text = help_of(r);
        let label = label_of(&r.row);
        let lw = p.measure(Face::Ui, 13.5, label);
        let pl = if kind == Kind::Stack {
            let w = g.r - lx;
            let help = wrap(p, &help_text, w, 3);
            let n = help.len() as f32;
            let h = STACK_LABEL_H
                + CONTROL_H
                + if help.is_empty() {
                    0.0
                } else {
                    STACK_GAP + HELP_LINE * n
                }
                + STACK_BOTTOM;
            let btn = r.button.as_ref().map(|(l, _)| {
                let aw = acc_w(l);
                Rect::new(g.r - aw, y + STACK_LABEL_H, aw, CONTROL_H)
            });
            let fw = btn.map_or(w, |b| b.x - ACC_GAP - lx);
            Placed {
                y,
                h,
                kind: Some(kind),
                label_x: lx,
                label_cy: y + STACK_LABEL_H / 2.0 - 1.0,
                label_w: w,
                ctrl: Rect::new(lx, y + STACK_LABEL_H, fw, CONTROL_H),
                acc: btn,
                help,
                help_x: lx,
                help_y: y + STACK_LABEL_H + CONTROL_H + STACK_GAP + HELP_LINE / 2.0,
                help_w: w,
                under: false,
                cut: lw > w,
            }
        } else if help_text.is_empty() {
            let (ctrl, acc) = control_rects(r, g, y + ROW_H / 2.0);
            let room = ctrl.x - lx - LABEL_GAP;
            Placed {
                y,
                h: ROW_H,
                kind: Some(kind),
                label_x: lx,
                label_cy: y + ROW_H / 2.0,
                label_w: room,
                ctrl,
                acc,
                help: Vec::new(),
                help_x: lx,
                help_y: 0.0,
                help_w: 0.0,
                under: false,
                cut: lw > room,
            }
        } else if !under {
            let (ctrl, acc) = control_rects(r, g, y + BESIDE_H / 2.0);
            let room = ctrl.x - lx - LABEL_GAP;
            Placed {
                y,
                h: BESIDE_H,
                kind: Some(kind),
                label_x: lx,
                label_cy: y + 13.5,
                label_w: room,
                ctrl,
                acc,
                help: vec![help_text.clone()],
                help_x: lx,
                help_y: y + 29.5,
                help_w: room,
                under: false,
                cut: lw > room,
            }
        } else {
            let (ctrl, acc) = control_rects(r, g, y + 16.0);
            let room = ctrl.x - lx - LABEL_GAP;
            let help = wrap(p, &help_text, g.r - lx, 2);
            let n = help.len() as f32;
            Placed {
                y,
                h: UNDER_H + HELP_LINE * (n - 1.0),
                kind: Some(kind),
                label_x: lx,
                label_cy: y + 16.0,
                label_w: room,
                ctrl,
                acc,
                help,
                help_x: lx,
                help_y: y + 36.0,
                help_w: g.r - lx,
                under: true,
                cut: lw > room,
            }
        };
        y += pl.h;
        out.push(pl);
    }
    (out, y + PAGE_BOTTOM, under)
}

// ---- drawing ----------------------------------------------------------------------------------

pub fn draw(p: &mut Painter, pal: &Palette, pages: &[Page], o: &Opts, w: f32, h: f32) {
    let g = shape(w, h);
    let r = g.card;
    p.shadow(r, 10.0, pal.dark);
    if pal.dark {
        p.shadow(r, 10.0, true);
    }
    p.frame(r, 10.0, pal.popup, pal.popup_line);
    let px = p.px().max(1.0);

    // The header: the title, the close button, a hairline under both.
    p.text(
        Face::Semibold,
        17.0,
        r.x + PAD,
        r.y + HEADER_H / 2.0,
        "Settings",
        pal.text,
        300.0,
    );
    let close = Rect::new(
        r.right() - 12.0 - 28.0,
        r.y + (HEADER_H - 28.0) / 2.0,
        28.0,
        28.0,
    );
    p.icon(glyph::CLOSE, 13.0, close.cx(), close.cy(), pal.muted);
    p.fill(
        Rect::new(r.x + 1.0, r.y + HEADER_H, r.w - 2.0, px),
        pal.popup_line,
    );
    p.fill(
        Rect::new(g.divider_x, r.y + HEADER_H, px, r.h - HEADER_H - 1.0),
        pal.popup_line,
    );

    // The category list.
    for (c, pg) in pages.iter().enumerate() {
        let it = Rect::new(g.nav.x, g.nav.y + c as f32 * g.item_h, g.nav.w, g.item_h);
        let cur = c == o.current;
        if cur {
            if o.nav_focus {
                ring(p, pal, it, 6.0, pal.popup);
            }
            p.rbox(it, 6.0, pal.on);
            p.rbox(
                Rect::new(it.x + MARK_INSET, it.cy() - MARK_H / 2.0, MARK_W, MARK_H),
                MARK_W / 2.0,
                pal.accent,
            );
        }
        p.text(
            Face::Ui,
            13.0,
            it.x + NAV_TEXT_INSET,
            it.cy(),
            &pg.name,
            if cur { pal.text } else { pal.muted },
            it.w - NAV_TEXT_INSET - 8.0,
        );
    }

    // The page.
    let page = &pages[o.current];
    let (placed, total, _) = layout(p, page, &g);
    let oy = g.page.y
        + if o.center {
            ((g.page.h - total) / 2.0).floor().max(0.0)
        } else {
            0.0
        };
    let old = p.clip(g.page);
    for (i, it) in page.items.iter().enumerate() {
        let Item::Row(rr) = it else {
            if let Item::Custom(_, id) = it {
                custom(p, pal, &g, placed[i].y + oy, *id);
            }
            continue;
        };
        let pl = &placed[i];
        let next_is_row = matches!(page.items.get(i + 1), Some(Item::Row(_)));
        draw_row(p, pal, &g, rr, pl, oy, next_is_row);
    }
    p.restore(old);

    // A page that does not fit: a red bar where it is cut, so no mockup can hide it.
    if total > g.page.h {
        p.fill(
            Rect::new(g.page.x, g.page.bottom() - 3.0, g.page.w, 3.0),
            [220, 40, 40],
        );
    }
}

fn dy(r: Rect, oy: f32) -> Rect {
    Rect::new(r.x, r.y + oy, r.w, r.h)
}

fn draw_row(
    p: &mut Painter,
    pal: &Palette,
    g: &Geo,
    r: &R,
    pl: &Placed,
    oy: f32,
    next_is_row: bool,
) {
    let off = r.off.is_some();
    let y = pl.y + oy;
    let lx = pl.label_x;
    if r.child {
        p.fill(
            Rect::new(g.l + 3.0, y + 1.0, 2.0 * p.px().max(1.0), pl.h - 2.0),
            mix(pal.popup, pal.text, 0.14),
        );
    }
    if next_is_row {
        let px = p.px().max(1.0);
        p.fill_a(
            Rect::new(lx, y + pl.h - px, g.r - lx, px),
            pal.popup_line,
            0.6,
        );
    }
    let label = label_of(&r.row);
    let label_col = if off { pal.faint } else { pal.text };
    let bg = pal.popup;
    let ctrl = dy(pl.ctrl, oy);
    p.text(
        Face::Ui,
        13.5,
        lx,
        pl.label_cy + oy,
        label,
        label_col,
        pl.label_w,
    );
    for (k, l) in pl.help.iter().enumerate() {
        p.text(
            Face::Ui,
            11.5,
            pl.help_x,
            pl.help_y + oy + HELP_LINE * k as f32,
            l,
            pal.muted,
            pl.help_w,
        );
    }
    if let (Some(acc), Some((b, enabled))) = (pl.acc, &r.button) {
        button(p, pal, dy(acc, oy), b, *enabled);
    }

    let ring_on = r.focus;
    match (&r.row, pl.kind) {
        (Row::Toggle { value, .. }, _) => {
            if ring_on {
                ring(p, pal, ctrl, TOGGLE_H / 2.0, bg);
            }
            draw_toggle(p, pal, ctrl, *value);
            if off {
                p.rbox_a(ctrl.inset(-1.0), TOGGLE_H / 2.0, pal.popup, 0.55);
            }
        }
        (Row::Choice { options, value, .. }, Some(Kind::Seg)) => {
            if ring_on {
                ring(p, pal, ctrl, 6.0, bg);
            }
            p.frame(ctrl, 6.0, pal.field, pal.line);
            let n = options.len() as f32;
            let sw = (ctrl.w - 4.0) / n;
            for (k, o) in options.iter().enumerate() {
                let sr = Rect::new(ctrl.x + 2.0 + sw * k as f32, ctrl.y + 2.0, sw, ctrl.h - 4.0);
                let on = k == *value;
                if on {
                    p.rbox(
                        sr,
                        4.0,
                        if off {
                            mix(pal.accent, pal.popup, 0.55)
                        } else {
                            pal.accent
                        },
                    );
                }
                p.text_center(
                    if on { Face::Semibold } else { Face::Ui },
                    13.0,
                    sr,
                    o,
                    if on {
                        pal.on_accent
                    } else if off {
                        pal.faint
                    } else {
                        pal.text
                    },
                );
            }
        }
        (Row::Choice { options, value, .. }, _) => {
            if ring_on {
                ring(p, pal, ctrl, 6.0, bg);
            }
            p.frame(ctrl, 6.0, pal.field, pal.line);
            let shown = options.get(*value).map_or("", String::as_str);
            p.text(
                Face::Ui,
                13.0,
                ctrl.x + 11.0,
                ctrl.cy(),
                shown,
                if off { pal.faint } else { pal.text },
                ctrl.w - 36.0,
            );
            p.icon(glyph::DOWN, 10.0, ctrl.right() - 15.0, ctrl.cy(), pal.muted);
        }
        (
            Row::Number {
                value,
                min,
                max,
                unit,
                ..
            },
            _,
        ) => {
            if ring_on {
                ring(p, pal, ctrl, 6.0, bg);
            }
            let m = Rect::new(ctrl.x, ctrl.y, ICON_W, ctrl.h);
            let pl_ = Rect::new(ctrl.right() - ICON_W, ctrl.y, ICON_W, ctrl.h);
            let v = Rect::new(
                m.right() + 4.0,
                ctrl.y,
                pl_.x - 4.0 - m.right() - 4.0,
                ctrl.h,
            );
            for (rect, gl, enabled) in [
                (m, glyph::MINUS, value > min && !off),
                (pl_, glyph::PLUS, value < max && !off),
            ] {
                p.frame(rect, 6.0, mix(pal.popup, pal.text, 0.04), pal.popup_line);
                p.icon(
                    gl,
                    11.0,
                    rect.cx(),
                    rect.cy(),
                    if enabled { pal.text } else { pal.faint },
                );
            }
            p.frame(v, 6.0, pal.field, pal.line);
            p.text_center(
                Face::Ui,
                13.0,
                v,
                &number_text(*value, unit),
                if off { pal.faint } else { pal.text },
            );
        }
        (Row::Text { .. }, _) => draw_text(p, pal, &r.row, ctrl, r.focus, off),
        (Row::Action { button: b, .. }, _) => {
            if ring_on {
                ring(p, pal, ctrl, 6.0, bg);
            }
            button(p, pal, ctrl, b, !off);
        }
        _ => {}
    }
}

/// A button, all of them one look and one height: the shared dialog button, with a glyph on it
/// for the icon ones.
fn button(p: &mut Painter, pal: &Palette, r: Rect, label: &str, enabled: bool) {
    if let Some(name) = label.strip_prefix("icon:") {
        let gl = if name == "delete" {
            glyph::DELETE
        } else {
            glyph::RELOAD
        };
        draw_dialog_button(p, pal, r, "", false, false, false);
        p.icon(
            gl,
            14.0,
            r.cx(),
            r.cy(),
            if enabled { pal.text } else { pal.faint },
        );
        return;
    }
    if enabled {
        draw_dialog_button(p, pal, r, label, false, false, false);
    } else {
        p.frame(r, 6.0, pal.field, pal.line);
        p.text_center(Face::Ui, 13.0, r, label, pal.faint);
    }
}

fn draw_text(p: &mut Painter, pal: &Palette, row: &Row, ctrl: Rect, focused: bool, off: bool) {
    let Row::Text {
        value,
        placeholder,
        secret,
        ..
    } = row
    else {
        return;
    };
    p.frame(ctrl, 6.0, pal.field, pal.line);
    if focused {
        let old = p.clip(Rect::new(ctrl.x, ctrl.bottom() - 2.0, ctrl.w, 2.0));
        p.rbox(ctrl, 6.0, pal.accent);
        p.restore(old);
    }
    let colors = FieldColors {
        text: if off { pal.faint } else { pal.text },
        placeholder: pal.faint,
        selection: pal.selection,
        caret: pal.text,
    };
    let mut f = TextField::new();
    f.set_secret(*secret);
    f.set_text(value);
    f.draw(
        p,
        Rect::new(ctrl.x + 10.0, ctrl.y, ctrl.w - 20.0, ctrl.h),
        Face::Ui,
        13.0,
        &colors,
        placeholder,
        focused,
        true,
    );
}

fn ring(p: &mut Painter, pal: &Palette, r: Rect, radius: f32, bg: Rgb) {
    p.rbox(r.inset(-3.0), radius + 3.0, pal.accent);
    p.rbox(r.inset(-1.5), radius + 1.5, bg);
}

fn draw_toggle(p: &mut Painter, pal: &Palette, r: Rect, on: bool) {
    let knob = 12.0;
    if on {
        p.rbox(r, r.h / 2.0, pal.accent);
        p.rbox(
            Rect::new(r.right() - 4.0 - knob, r.cy() - knob / 2.0, knob, knob),
            knob / 2.0,
            pal.on_accent,
        );
    } else {
        p.frame(r, r.h / 2.0, pal.field, mix(pal.muted, pal.popup, 0.35));
        p.rbox(
            Rect::new(r.x + 4.0, r.cy() - knob / 2.0, knob, knob),
            knob / 2.0,
            pal.muted,
        );
    }
}

fn gap_of(it: &Item) -> f32 {
    match it {
        Item::Custom(h, _) => *h,
        _ => GAP_H,
    }
}

/// What a page draws itself: the downloader's server list, scheduler grid and so on.
fn custom(p: &mut Painter, pal: &Palette, g: &Geo, y: f32, id: u8) {
    super::pages::custom(p, pal, g.l, g.r, y, id);
}
