//! The Files pane on the left: the folder's tree with every file, a scrollbar and the splitter on
//! its inner edge. Adapted from the viewer's `bins/mdv-app/src/panes.rs`.

use crate::chrome::{Ctx, glyph};
use crate::commands::strings;
use crate::docs::Docs;
use crate::ui::{Drag, Hit, Scroller, Ui};
use mdshared_app::paths::same_path;
use mdshared_ui::gfx::{Face, Rect, mix};
pub use mdshared_ui::pane::ROW;
use mdshared_ui::pane::{HEAD, ROW_H, draw_head, draw_row_fill, draw_splitter};
use mdshared_ui::scroll::{draw_scrollbar, scrollbar};

/// The list of rows below the folder's own row.
pub fn files_list(r: Rect) -> Rect {
    let ry = r.y + HEAD;
    Rect::new(r.x, ry + ROW, r.w, (r.bottom() - ry - ROW).max(0.0))
}

pub fn draw_files(c: &mut Ctx, docs: &Docs, ui: &mut Ui, r: Rect) {
    let pal = *c.pal;
    c.p.fill(r, pal.pane);
    draw_head(c.p, &pal, r, strings::FILES_DOCK);
    let b = Rect::new(r.right() - 38.0, r.y + 6.0, 28.0, 28.0);
    if let Some(fill) = c.fill_for(Hit::FilesFolder, false, false) {
        c.p.rbox(b, 5.0, fill);
    }
    c.p.icon(glyph::FOLDER_OPEN, 14.0, b.cx(), b.cy(), pal.muted);
    c.hit(Hit::FilesFolder, b);

    let dragging = matches!(ui.drag, Some(Drag::Splitter { .. }));
    let Some(root) = ui.tree.root().map(|p| p.to_path_buf()) else {
        let cy = r.y + HEAD + 40.0;
        c.p.text_center(
            Face::Ui,
            13.0,
            Rect::new(r.x, cy - 10.0, r.w, 20.0),
            "No folder open",
            pal.muted,
        );
        let label = "Open folder…";
        let w = c.p.measure(Face::Ui, 13.0, label) + 28.0;
        let b = Rect::new((r.cx() - w / 2.0).round(), cy + 18.0, w, 30.0);
        let fill = c
            .fill_for(Hit::FilesOpenFolder, false, false)
            .unwrap_or(pal.bar);
        c.p.frame(b, 6.0, fill, pal.line);
        c.p.text_center(Face::Ui, 13.0, b, label, pal.text);
        c.hit(Hit::FilesOpenFolder, b);
        ui.panes.files_content = 0.0;
        edge(c, r, dragging);
        return;
    };
    // The folder itself, as the root row.
    let name = root.file_name().map_or_else(
        || root.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    let ry = r.y + HEAD;
    c.p.icon(
        glyph::FOLDER_OPEN,
        14.0,
        r.x + 22.0,
        ry + ROW_H / 2.0,
        pal.folder,
    );
    c.p.text(
        Face::Semibold,
        12.5,
        r.x + 36.0,
        ry + ROW_H / 2.0,
        &name,
        pal.text,
        r.w - 52.0,
    );

    let list = files_list(r);
    let rows = ui.tree.rows();
    ui.panes.files_content = rows.len() as f32 * ROW + 8.0;
    ui.panes.files_view = list.h;
    ui.panes.files_scroll = ui
        .panes
        .files_scroll
        .clamp(0.0, (ui.panes.files_content - list.h).max(0.0));
    let scroll = ui.panes.files_scroll;
    let active = docs.active_doc().and_then(|d| d.path.clone());
    let quiet = mix(pal.text, pal.muted, 0.35);
    let old = c.p.clip(list);
    let first = (scroll / ROW).floor() as usize;
    for (i, row) in rows.iter().enumerate().skip(first) {
        let y = list.y + i as f32 * ROW - scroll;
        if y > list.bottom() {
            break;
        }
        let rr = Rect::new(r.x + 6.0, y, r.w - 12.0, ROW_H);
        let hit = Hit::FilesRow(i);
        let selected = !row.is_dir && active.as_deref().is_some_and(|a| same_path(a, &row.path));
        draw_row_fill(
            c.p,
            &pal,
            rr,
            c.hover == Some(hit),
            c.pressed == Some(hit),
            selected,
        );
        let ix = r.x + 14.0 + (row.depth as f32 + 1.0) * 14.0;
        let cy = rr.cy();
        if row.is_dir {
            let arrow = if row.expanded {
                glyph::DOWN
            } else {
                glyph::RIGHT
            };
            c.p.icon(arrow, 8.0, ix - 8.0, cy, pal.faint);
            let folder = if row.expanded {
                glyph::FOLDER_OPEN
            } else {
                glyph::FOLDER
            };
            c.p.icon(folder, 14.0, ix + 9.0, cy, pal.folder);
        } else {
            let color = if selected { pal.accent } else { pal.muted };
            c.p.icon(glyph::DOC, 13.0, ix + 9.0, cy, color);
        }
        let color = if selected { pal.text } else { quiet };
        c.p.text(
            Face::Ui,
            13.0,
            ix + 23.0,
            cy,
            &row.name,
            color,
            rr.right() - ix - 30.0,
        );
        c.hit(hit, rr);
    }
    c.p.restore(old);
    let thumb_hit = Hit::ScrollBar(Scroller::Files);
    if let Some((track, thumb)) = scrollbar(list, ui.panes.files_content, list.h, scroll) {
        let active = c.hover == Some(thumb_hit)
            || matches!(
                ui.drag,
                Some(Drag::Thumb {
                    target: Scroller::Files,
                    ..
                })
            );
        draw_scrollbar(c.p, &pal, track, thumb, active);
        c.hit(thumb_hit, track);
    }
    edge(c, r, dragging);
}

/// The splitter on the pane's right edge, and the hairline that marks it.
pub fn edge(c: &mut Ctx, r: Rect, dragging: bool) {
    let hot = dragging || c.hover == Some(Hit::Splitter);
    let hit = draw_splitter(c.p, c.pal, r, true, hot);
    c.hit(Hit::Splitter, hit);
}
