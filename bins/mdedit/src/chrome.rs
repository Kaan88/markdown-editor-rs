//! The title row (logo, tabs, window buttons) and the command bar under it, drawn from scratch each
//! time their region is dirty, recording what the pointer can hit. Adapted from the viewer's
//! `bins/mdv-app/src/chrome.rs`; the bar has the editor's buttons.

use crate::commands::{CommandId, hint, strings};
use crate::docs::Docs;
use crate::session::ThemeChoice;
use crate::ui::{BarField, BarItem, Drag, FindOpt, Hit, PopupKind, Ui};
use mdshared_ui::chrome::{
    BAR_H, DRAG_MIN, NEW_TAB_W, TABS_X, TITLE_H, TabState, WIN_BTN_W, WinButton, button_fill,
    draw_new_tab_button, draw_tab, draw_win_button, tab_natural_width, tab_widths, win_button_rect,
};
use mdshared_ui::gfx::{Face, Painter, Rect, Rgb, mix};
pub use mdshared_ui::glyph;
use mdshared_ui::textfield::{FieldColors, TextField};
use mdshared_ui::theme::Palette;

pub const FIND_W: f32 = 260.0;

/// Shared by every drawing function: the painter, colours, and where hits are recorded.
pub struct Ctx<'a, 'b> {
    pub p: &'a mut Painter<'b>,
    pub pal: &'a Palette,
    pub hits: &'a mut Vec<(Hit, Rect)>,
    pub hover: Option<Hit>,
    pub pressed: Option<Hit>,
}

impl Ctx<'_, '_> {
    pub fn hit(&mut self, hit: Hit, r: Rect) {
        self.hits.push((hit, r));
    }

    /// The fill behind a button: pressed, hovered, toggled on, or none.
    pub fn fill_for(&self, hit: Hit, on: bool, open: bool) -> Option<Rgb> {
        button_fill(
            self.pal,
            self.hover == Some(hit),
            self.pressed == Some(hit),
            on,
            open,
        )
    }
}

/// The mark's shapes on the icon's 256-unit canvas, as in `assets/mdedit.svg`: an "M" and a
/// pencil (eraser, then body and point).
const MARK_M: [(f32, f32); 12] = [
    (38.26, 174.08),
    (38.26, 81.92),
    (65.14, 81.92),
    (92.02, 116.48),
    (118.90, 81.92),
    (145.78, 81.92),
    (145.78, 174.08),
    (118.90, 174.08),
    (118.90, 122.24),
    (92.02, 156.80),
    (65.14, 122.24),
    (65.14, 174.08),
];
const MARK_ERASER: [(f32, f32); 4] = [
    (194.01, 82.70),
    (217.74, 95.32),
    (212.33, 105.49),
    (188.60, 92.87),
];
const MARK_PENCIL: [(f32, f32); 5] = [
    (186.34, 97.11),
    (210.08, 109.73),
    (187.99, 151.26),
    (164.41, 166.99),
    (164.26, 138.64),
];
const TILE: Rgb = [0x1d, 0x7d, 0x6b];

/// The app logo, the same drawing as the icon file: a green tile with a lighter top half and the
/// white mark. `r` is the tile.
pub fn logo(p: &mut Painter, r: Rect) {
    // The icon's tile is 240 of its 256 units wide, with corners of 52.
    let radius = r.w * 52.0 / 240.0;
    p.rbox(r, radius, TILE);
    p.rbox_a(
        Rect::new(r.x, r.y, r.w, r.h / 2.0),
        radius,
        [255, 255, 255],
        0.08,
    );
    let place =
        |&(x, y): &(f32, f32)| (r.x + (x - 8.0) / 240.0 * r.w, r.y + (y - 8.0) / 240.0 * r.h);
    let white = [255, 255, 255];
    for shape in [&MARK_M[..], &MARK_ERASER[..], &MARK_PENCIL[..]] {
        p.polygon(&shape.iter().map(place).collect::<Vec<_>>(), white);
    }
}

/// What the command bar shows at a given width.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BarPlan {
    pub labels: bool,
    pub find_w: f32,
    /// The Files toggle.
    pub panes: bool,
    /// Reload and Recent.
    pub recent: bool,
    pub count: bool,
}

/// Label widths that decide the bar's fit, in logical px.
#[derive(Clone, Copy, Debug, Default)]
pub struct BarText {
    pub open: f32,
    pub recent: f32,
    pub files: f32,
    pub theme: f32,
    /// The match counter, 0 when empty.
    pub count: f32,
}

const ICON_BTN: f32 = 36.0;
const GAP: f32 = 2.0;
const SEP: f32 = 13.0;
const MARGIN: f32 = 8.0;

fn labelled(label: f32, on: bool) -> f32 {
    if on { 44.0 + label } else { ICON_BTN }
}

/// The width a plan needs.
fn plan_width(plan: &BarPlan, t: &BarText) -> f32 {
    let l = plan.labels;
    let mut w = 2.0 * MARGIN;
    w += labelled(t.open, l) + GAP;
    if plan.recent {
        w += ICON_BTN + GAP + labelled(t.recent, l) + GAP;
    }
    if plan.panes {
        w += SEP + labelled(t.files, l) + GAP;
    }
    // Right group: more, theme, separator, next, previous, find box, counter.
    w += ICON_BTN + GAP + labelled(t.theme, l) + SEP + 2.0 * (ICON_BTN + GAP) + plan.find_w;
    if plan.count && t.count > 0.0 {
        w += t.count + 12.0;
    }
    w + 16.0
}

/// Drops labels first, then narrows the find box, then hides the Files toggle and Reload/Recent
/// (all of which stay in the main menu and on the keyboard), so nothing overlaps.
pub fn bar_plan(avail: f32, t: &BarText) -> BarPlan {
    let full = BarPlan {
        labels: true,
        find_w: FIND_W,
        panes: true,
        recent: true,
        count: true,
    };
    let steps = [
        full,
        BarPlan {
            labels: false,
            ..full
        },
        BarPlan {
            labels: false,
            find_w: 200.0,
            ..full
        },
        BarPlan {
            labels: false,
            find_w: 180.0,
            panes: false,
            ..full
        },
        BarPlan {
            labels: false,
            find_w: 160.0,
            panes: false,
            recent: false,
            ..full
        },
    ];
    for plan in steps {
        if plan_width(&plan, t) <= avail {
            return plan;
        }
    }
    let mut last = BarPlan {
        labels: false,
        find_w: 0.0,
        panes: false,
        recent: false,
        count: false,
    };
    last.find_w = (avail - plan_width(&last, t)).clamp(96.0, 160.0);
    last
}

fn theme_glyph(choice: ThemeChoice) -> &'static str {
    match choice {
        ThemeChoice::System => glyph::SYSTEM,
        ThemeChoice::Light => glyph::SUN,
        ThemeChoice::Dark => glyph::MOON,
    }
}

pub fn theme_label(choice: ThemeChoice) -> &'static str {
    match choice {
        ThemeChoice::System => "System",
        ThemeChoice::Light => "Light",
        ThemeChoice::Dark => "Dark",
    }
}

/// The tooltip for a chrome hit, if it has one.
pub fn tooltip(hit: Hit, docs: &Docs, maximized: bool) -> Option<String> {
    let with = |text: &str, id: CommandId| {
        let k = hint(id);
        if k.is_empty() {
            text.to_string()
        } else {
            format!("{text} ({k})")
        }
    };
    Some(match hit {
        Hit::Fold(line) | Hit::FoldBox(line) => {
            let folded = docs.active_ready()?.folds.headed(line).is_some();
            with(
                if folded {
                    "Open the block"
                } else {
                    "Fold the block"
                },
                CommandId::ToggleFold,
            )
        }
        Hit::Tab(i) => {
            let doc = docs.tabs.get(i)?;
            doc.path
                .as_ref()
                .map_or_else(|| doc.name(), |p| p.display().to_string())
        }
        Hit::TabClose(_) => with("Close tab", CommandId::CloseTab),
        Hit::NewTab => with("New file", CommandId::NewFile),
        Hit::WinMin => "Minimize".into(),
        Hit::WinMax if maximized => "Restore down".into(),
        Hit::WinMax => "Maximize".into(),
        Hit::WinClose => "Close".into(),
        Hit::Bar(b) => match b {
            BarItem::Open => with("Open files", CommandId::OpenFiles),
            BarItem::Reload => with("Reload", CommandId::Reload),
            BarItem::Recent => "Recent files".into(),
            BarItem::Files => with("Files pane", CommandId::ToggleFiles),
            BarItem::FindPrev => with("Previous match", CommandId::FindPrevious),
            BarItem::FindNext => with("Next match", CommandId::FindNext),
            BarItem::Theme => format!("Theme: {} (click to change)", theme_label(docs.theme)),
            BarItem::More => "More".into(),
        },
        Hit::FindClear => "Clear search (Escape)".into(),
        Hit::FindOpt(FindOpt::Case) => "Match case".into(),
        Hit::FindOpt(FindOpt::Word) => "Whole word".into(),
        Hit::FindOpt(FindOpt::Regex) => "Regular expression".into(),
        Hit::ReplaceOne => "Replace this match and go to the next (Enter)".into(),
        Hit::ReplaceAll => with("Replace every match", CommandId::ReplaceAll),
        Hit::ReplaceClose => "Close (Escape)".into(),
        Hit::InSelection => "Replace only in the selection from when this opened".into(),
        Hit::StatusEol => "Line endings (click to change)".into(),
        Hit::StatusEncoding => "Encoding (click to change)".into(),
        Hit::FilesFolder => with("Open folder", CommandId::OpenFolder),
        _ => return None,
    })
}

// ---- title row -------------------------------------------------------------------------------

/// Draws the title row into `r` (the full window width, `TITLE_H` tall).
pub fn draw_title(c: &mut Ctx, docs: &Docs, ui: &Ui, r: Rect, maximized: bool) {
    let pal = *c.pal;
    c.p.fill(r, pal.title);
    c.hit(Hit::TitleDrag, r);
    logo(c.p, Rect::new(14.0, 9.0, 22.0, 22.0));

    // Window buttons, right to left.
    let mut right = r.right();
    let buttons = [
        (Hit::WinClose, WinButton::Close),
        (Hit::WinMax, WinButton::Max),
        (Hit::WinMin, WinButton::Min),
    ];
    for (hit, kind) in buttons {
        let b = win_button_rect(right, 0);
        right -= WIN_BTN_W;
        draw_win_button(
            c.p,
            &pal,
            kind,
            b,
            maximized,
            c.hover == Some(hit),
            c.pressed == Some(hit),
        );
        c.hit(hit, b);
    }

    // Tabs.
    let labels = docs.labels();
    let avail = (right - DRAG_MIN - NEW_TAB_W - TABS_X).max(0.0);
    let natural: Vec<f32> = labels.iter().map(|l| tab_natural_width(c.p, l)).collect();
    let widths = tab_widths(&natural, avail);
    let active = docs.active();
    let mut x = TABS_X;
    // Tabs that still do not fit at the floor width are cut off rather than drawn over the buttons.
    let clip = c.p.clip(Rect::new(0.0, 0.0, TABS_X + avail + 8.0, TITLE_H));
    let is_hot = |hover: Option<Hit>, i: usize| matches!(hover, Some(Hit::Tab(j) | Hit::TabClose(j)) if j == i);
    for (i, label) in labels.iter().enumerate() {
        let w = widths[i];
        let state = TabState {
            active: active == Some(i),
            hovered: is_hot(c.hover, i),
            dragging: matches!(
                ui.drag,
                Some(Drag::Tab { index, moving: true, .. }) if index == i
            ),
            missing: docs.tabs[i].missing,
            next_quiet: i + 1 < labels.len() && active != Some(i + 1) && !is_hot(c.hover, i + 1),
            close_hovered: c.hover == Some(Hit::TabClose(i)),
            close_pressed: c.pressed == Some(Hit::TabClose(i)),
        };
        let drawn = draw_tab(c.p, &pal, x, w, r.w, label, &state);
        c.hit(Hit::Tab(i), drawn.tab);
        // The tab in the second pane of a split view: a short line under its name.
        if docs.other() == Some(i) {
            let t = drawn.tab;
            c.p.fill(
                Rect::new(t.x + 12.0, t.bottom() - 4.0, (t.w - 24.0).max(0.0), 2.0),
                pal.muted,
            );
        }
        if let Some(b) = drawn.close {
            c.hit(Hit::TabClose(i), b);
        }
        x += w;
    }
    c.p.restore(clip);
    let b = draw_new_tab_button(
        c.p,
        &pal,
        x,
        avail,
        c.hover == Some(Hit::NewTab),
        c.pressed == Some(Hit::NewTab),
    );
    c.hit(Hit::NewTab, b);
}

// ---- command bar -----------------------------------------------------------------------------

/// The counter beside the find box for the active document, and whether it says "No matches".
pub fn find_label(docs: &Docs) -> (String, bool) {
    match docs.active_ready() {
        Some(r) => (
            r.find.label(),
            !r.find.text.is_empty() && !r.find.searching && r.find.matches.is_empty(),
        ),
        None => (String::new(), false),
    }
}

/// The measured text widths the bar's fit depends on.
pub fn bar_text(p: &mut Painter, docs: &Docs) -> BarText {
    let (count, _) = find_label(docs);
    BarText {
        open: p.measure(Face::Ui, 13.0, "Open"),
        recent: p.measure(Face::Ui, 13.0, "Recent"),
        files: p.measure(Face::Ui, 13.0, "Files"),
        theme: p.measure(Face::Ui, 13.0, theme_label(docs.theme)),
        count: if count.is_empty() {
            0.0
        } else {
            p.measure(Face::Ui, 12.5, &count)
        },
    }
}

/// Where the find box's text goes inside its frame, and the clear button if the field has text.
pub fn find_parts(frame: Rect, has_text: bool) -> (Rect, Option<Rect>) {
    let clear =
        has_text.then(|| Rect::new(frame.right() - 30.0, frame.y + 4.0, 24.0, frame.h - 8.0));
    let right = if has_text {
        frame.right() - 32.0
    } else {
        frame.right() - 10.0
    };
    (
        Rect::new(
            frame.x + 32.0,
            frame.y + 1.0,
            (right - frame.x - 32.0).max(0.0),
            frame.h - 2.0,
        ),
        clear,
    )
}

/// Where the text of one of the bar's boxes goes inside its frame, and the clear button if it
/// has one.
pub fn field_parts(which: BarField, frame: Rect, has_text: bool) -> (Rect, Option<Rect>) {
    match which {
        BarField::Find => find_parts(frame, has_text),
        BarField::Replace | BarField::Goto | BarField::ResultsReplace => (
            Rect::new(
                frame.x + 10.0,
                frame.y + 1.0,
                (frame.w - 20.0).max(0.0),
                frame.h - 2.0,
            ),
            None,
        ),
    }
}

/// What the replace strip shows at a width: the box's width and whether the option toggles fit
/// (they are in the find box's menu too).
pub fn replace_plan(avail: f32, buttons: f32, scoped: bool) -> (f32, bool) {
    let fixed = MARGIN * 2.0 + ICON_BTN + buttons + 16.0;
    let toggles = (3.0 + f32::from(u8::from(scoped))) * (TOGGLE_W + GAP) + 8.0;
    if avail >= fixed + toggles + FIND_W {
        (FIND_W, true)
    } else if avail >= fixed + toggles + 140.0 {
        (avail - fixed - toggles, true)
    } else {
        ((avail - fixed).clamp(60.0, FIND_W), false)
    }
}

const TOGGLE_W: f32 = 32.0;

/// Draws the replace strip into `r`: from the right, Close, Replace all, Replace, the
/// replacement's box, and the three search options.
pub fn draw_replace_strip(c: &mut Ctx, docs: &Docs, ui: &mut Ui, r: Rect) {
    let pal = *c.pal;
    c.p.fill(r, pal.bar);
    let y = r.y;
    let has_matches = docs
        .active_ready()
        .is_some_and(|r| !r.find.matches.is_empty() && !r.find.searching);
    let width = |c: &mut Ctx, label: &str| (c.p.measure(Face::Ui, 13.0, label) + 26.0).round();
    let (one_w, all_w) = (
        width(c, strings::REPLACE_ONE),
        width(c, strings::REPLACE_ALL),
    );
    let scope = docs.active_ready().filter(|r| r.replace_scope.is_some());
    let (field_w, toggles) = replace_plan(r.w, one_w + all_w + 6.0, scope.is_some());

    let mut right = r.right() - MARGIN;
    let close = Rect::new(right - ICON_BTN, y, ICON_BTN, 32.0);
    if let Some(f) = c.fill_for(Hit::ReplaceClose, false, false) {
        c.p.rbox(close, 6.0, f);
    }
    c.p.icon(glyph::CLOSE, 10.0, close.cx(), close.cy(), pal.muted);
    c.hit(Hit::ReplaceClose, close);
    right = close.x - 8.0;

    for (hit, label, w) in [
        (Hit::ReplaceAll, strings::REPLACE_ALL, all_w),
        (Hit::ReplaceOne, strings::REPLACE_ONE, one_w),
    ] {
        let b = Rect::new(right - w, y, w, 32.0);
        let fill = c
            .fill_for(hit, false, false)
            .filter(|_| has_matches)
            .map_or(pal.field, |f| mix(pal.field, f, 0.6));
        c.p.frame(b, 6.0, fill, pal.line);
        let color = if has_matches { pal.text } else { pal.faint };
        c.p.text_center(Face::Ui, 13.0, b, label, color);
        if has_matches {
            c.hit(hit, b);
        }
        right = b.x - 6.0;
    }
    right -= 2.0;

    let frame = Rect::new(right - field_w, y, field_w, 32.0);
    let focused = ui.find_focused && ui.field == BarField::Replace;
    draw_field(
        c,
        &mut ui.replace,
        focused,
        ui.caret_on,
        frame,
        BarField::Replace,
        false,
    );
    if !toggles {
        return;
    }
    let mut x = frame.x - 8.0;
    for (opt, label) in [
        (FindOpt::Regex, glyph::REGEX),
        (FindOpt::Word, glyph::WORD),
        (FindOpt::Case, glyph::CASE),
    ] {
        x -= TOGGLE_W;
        let b = Rect::new(x, y, TOGGLE_W, 32.0);
        let on = ui.option(opt);
        let hit = Hit::FindOpt(opt);
        if let Some(f) = c.fill_for(hit, on, false) {
            c.p.rbox(b, 6.0, f);
        }
        let color = if on { pal.accent } else { pal.muted };
        c.p.icon(label, 16.0, b.cx(), b.cy(), color);
        c.hit(hit, b);
        x -= GAP;
    }
    // The selection kept from when the strip opened: Replace stays inside it while this is on.
    if let Some(r) = scope {
        x -= TOGGLE_W;
        let b = Rect::new(x, y, TOGGLE_W, 32.0);
        let on = r.scope_on;
        if let Some(f) = c.fill_for(Hit::InSelection, on, false) {
            c.p.rbox(b, 6.0, f);
        }
        let color = if on { pal.accent } else { pal.muted };
        c.p.icon(glyph::SELECT_ALL, 16.0, b.cx(), b.cy(), color);
        c.hit(Hit::InSelection, b);
    }
}

/// Draws the command bar into `r`.
/// The bar under the command bar that offers a newer version, shows its download, and says
/// when it is in place.
pub fn draw_update_bar(c: &mut Ctx, docs: &Docs, bar: Rect) {
    use mdshared_ui::notice::{self, UpdateBarState};
    let Some(stage) = docs.updates.bar() else {
        return;
    };
    let state = UpdateBarState {
        go_hovered: c.hover == Some(Hit::UpdateGo),
        go_pressed: c.pressed == Some(Hit::UpdateGo),
        close_hovered: c.hover == Some(Hit::UpdateClose),
    };
    let hits = notice::draw_update_bar(c.p, c.pal, bar, &crate::updater::APP, stage, state);
    if let Some(r) = hits.go {
        c.hit(Hit::UpdateGo, r);
    }
    if let Some(r) = hits.close {
        c.hit(Hit::UpdateClose, r);
    }
}

pub fn draw_bar(c: &mut Ctx, docs: &Docs, ui: &mut Ui, r: Rect) {
    let pal = *c.pal;
    c.p.fill(r, pal.bar);
    let t = bar_text(c.p, docs);
    let plan = bar_plan(r.w, &t);
    let y = r.y + (BAR_H - 32.0) / 2.0;
    let cy = y + 16.0;
    let open = ui.popup_kind();

    let has_tab = docs.active().is_some();
    let has_matches = docs
        .active_ready()
        .is_some_and(|r| !r.find.matches.is_empty());
    let button =
        |c: &mut Ctx, x: f32, item: BarItem, g: &str, label: Option<&str>, on: bool| -> f32 {
            // Reload needs a document and match stepping needs matches; without them they show
            // greyed and do nothing.
            let enabled = match item {
                BarItem::Reload => has_tab,
                BarItem::FindPrev | BarItem::FindNext => has_matches,
                _ => true,
            };
            let w = match label {
                Some(l) => 44.0 + c.p.measure(Face::Ui, 13.0, l),
                None => ICON_BTN,
            };
            let b = Rect::new(x, y, w, 32.0);
            let hit = Hit::Bar(item);
            let menu_open = matches!(
                (item, open),
                (BarItem::Recent, Some(PopupKind::Recent)) | (BarItem::More, Some(PopupKind::Main))
            );
            if let Some(fill) = c.fill_for(hit, on, menu_open).filter(|_| enabled) {
                c.p.rbox(b, 6.0, fill);
            }
            let gx = if label.is_some() { x + 18.0 } else { b.cx() };
            let color = if !enabled {
                pal.faint
            } else if on {
                pal.accent
            } else {
                pal.text
            };
            c.p.icon(g, 15.0, gx, cy, color);
            if let Some(l) = label {
                c.p.text(Face::Ui, 13.0, x + 34.0, cy, l, pal.text, f32::INFINITY);
            }
            if enabled {
                c.hit(hit, b);
            }
            w
        };
    let sep = |c: &mut Ctx, x: f32| {
        c.p.fill(
            Rect::new((x + SEP / 2.0).round(), y + 6.0, c.p.px(), 20.0),
            pal.line,
        );
        SEP
    };
    let lab = |s: &'static str| plan.labels.then_some(s);

    // Left group.
    let mut x = r.x + MARGIN;
    x += button(c, x, BarItem::Open, glyph::OPEN, lab("Open"), false) + GAP;
    if plan.recent {
        x += button(c, x, BarItem::Reload, glyph::RELOAD, None, false) + GAP;
        x += button(c, x, BarItem::Recent, glyph::RECENT, lab("Recent"), false) + GAP;
    }
    if plan.panes {
        x += sep(c, x);
        button(
            c,
            x,
            BarItem::Files,
            glyph::FOLDER,
            lab("Files"),
            ui.panes.files,
        );
    }

    // Right group, from the right edge.
    let mut rx = r.right() - MARGIN - ICON_BTN;
    button(c, rx, BarItem::More, glyph::MORE, None, false);
    let choice = docs.theme;
    let tw = labelled(t.theme, plan.labels);
    rx -= GAP + tw;
    button(
        c,
        rx,
        BarItem::Theme,
        theme_glyph(choice),
        lab(theme_label(choice)),
        false,
    );
    rx -= SEP;
    sep(c, rx);
    rx -= ICON_BTN + GAP;
    button(c, rx, BarItem::FindNext, glyph::DOWN, None, false);
    rx -= ICON_BTN + GAP;
    button(c, rx, BarItem::FindPrev, glyph::UP, None, false);
    rx -= plan.find_w + 4.0;
    let frame = Rect::new(rx, y, plan.find_w, 32.0);
    if ui.going_to() {
        draw_field(
            c,
            &mut ui.goto,
            true,
            ui.caret_on,
            frame,
            BarField::Goto,
            ui.naming,
        );
        return;
    }
    let focused = ui.find_focused && ui.field == BarField::Find;
    draw_field(
        c,
        &mut ui.find,
        focused,
        ui.caret_on,
        frame,
        BarField::Find,
        false,
    );
    if plan.count && t.count > 0.0 {
        let (label, none) = find_label(docs);
        c.p.text_right(
            Face::Ui,
            12.5,
            frame.x - 10.0,
            cy,
            &label,
            if none { pal.warn } else { pal.muted },
        );
    }
}

/// One of the bar's two text boxes: the find box with its icon and clear button, or the
/// replacement's.
pub(crate) fn draw_field(
    c: &mut Ctx,
    field: &mut TextField,
    focused: bool,
    caret_on: bool,
    frame: Rect,
    which: BarField,
    naming: bool,
) {
    let pal = *c.pal;
    let find = which == BarField::Find;
    let hovered = c.hover == Some(which.hit()) || (find && c.hover == Some(Hit::FindClear));
    let edge = if hovered && !focused {
        mix(pal.line, pal.text, 0.18)
    } else {
        pal.line
    };
    let fill = if focused || !hovered {
        pal.field
    } else {
        mix(pal.field, pal.text, 0.03)
    };
    c.p.frame(frame, 6.0, fill, edge);
    if focused {
        // The Windows 11 text box: an accent underline that follows the rounded corners.
        let old =
            c.p.clip(Rect::new(frame.x, frame.bottom() - 2.0, frame.w, 2.0));
        c.p.rbox(frame, 6.0, pal.accent);
        c.p.restore(old);
    }
    if find {
        c.p.icon(
            glyph::SEARCH,
            12.0,
            frame.x + 17.0,
            frame.cy(),
            if focused { pal.text } else { pal.muted },
        );
    }
    let (area, clear) = field_parts(which, frame, !field.is_empty());
    let colors = FieldColors {
        text: pal.text,
        placeholder: pal.faint,
        selection: pal.selection,
        caret: pal.text,
    };
    field.draw(
        c.p,
        area,
        Face::Ui,
        13.0,
        &colors,
        match which {
            BarField::Find => strings::FIND_PLACEHOLDER,
            BarField::Replace => strings::REPLACE_PLACEHOLDER,
            BarField::ResultsReplace => strings::RESULTS_REPLACE_PLACEHOLDER,
            BarField::Goto if naming => strings::MACRO_NAME_PLACEHOLDER,
            BarField::Goto => strings::GOTO_PLACEHOLDER,
        },
        focused,
        caret_on,
    );
    c.hit(which.hit(), frame);
    if let Some(b) = clear {
        if let Some(f) = c.fill_for(Hit::FindClear, false, false) {
            c.p.rbox(b, 4.0, f);
        }
        c.p.icon(glyph::CLOSE, 8.5, b.cx(), b.cy(), pal.muted);
        c.hit(Hit::FindClear, b);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_replace_strip_gives_way() {
        // Wide: the full box and the toggles.
        assert_eq!(replace_plan(1280.0, 180.0, false), (FIND_W, true));
        // Narrower: the box shrinks first.
        let (w, toggles) = replace_plan(560.0, 180.0, false);
        assert!(toggles && (140.0..FIND_W).contains(&w));
        // Narrow: the toggles go (the find box's menu has them) and the box keeps a minimum.
        let (w, toggles) = replace_plan(400.0, 180.0, false);
        assert!(!toggles && w >= 60.0);
        let (w, _) = replace_plan(200.0, 180.0, false);
        assert_eq!(w, 60.0);
        // The replacement's text starts at the frame's padding; it has no clear button.
        let frame = Rect::new(100.0, 10.0, 200.0, 32.0);
        let (area, clear) = field_parts(BarField::Replace, frame, true);
        assert_eq!((area.x, area.w, clear), (110.0, 180.0, None));
        assert_eq!(
            field_parts(BarField::Find, frame, true),
            find_parts(frame, true)
        );
    }

    fn text() -> BarText {
        BarText {
            open: 32.0,
            recent: 42.0,
            files: 28.0,
            theme: 44.0,
            count: 50.0,
        }
    }

    #[test]
    fn bar_degrades_in_order() {
        let t = text();
        let wide = bar_plan(1400.0, &t);
        assert!(wide.labels && wide.panes && wide.recent && wide.find_w == FIND_W);
        let mut seen_no_labels = false;
        for avail in (300..1400).rev().step_by(10) {
            let plan = bar_plan(avail as f32, &t);
            if !plan.labels {
                seen_no_labels = true;
            }
            // Items hide only after labels are gone.
            if !plan.panes || !plan.recent {
                assert!(seen_no_labels && !plan.labels);
            }
        }
        let narrow = bar_plan(520.0, &t);
        assert!(plan_width(&narrow, &t) <= 520.0);
    }

    #[test]
    fn bar_fits_at_the_window_minimum() {
        let plan = bar_plan(520.0, &text());
        assert!(plan_width(&plan, &text()) <= 520.0 + 1.0);
        let tiny = bar_plan(300.0, &text());
        assert!(tiny.find_w >= 96.0);
    }
}
