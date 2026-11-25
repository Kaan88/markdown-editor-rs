//! Drawing the AI task window over files (`docs/ai-library-plan.md` 5.6): the instruction, the
//! saved prompts of the task's kind, the files under their three headings, where they go, and
//! the footer. The state and the layout numbers are `task_ui.rs`; the window's part is
//! `app/task.rs`.

use crate::task_ui::{
    self, DestChoice, FileKind, GROUP_H, Hit, ListRow, ROW_H, TaskKind, TaskScreen,
};
use mdshared_ui::check::{Tick, draw_tick};
use mdshared_ui::gfx::{Face, Painter, Rect, mix};
use mdshared_ui::glyph;
use mdshared_ui::overlay::{
    DIALOG_FOOTER, DIALOG_PAD, dialog_button_w, draw_backdrop, draw_dialog_button,
    draw_dialog_button_disabled, draw_dialog_card,
};
use mdshared_ui::scroll::draw_scrollbar;
use mdshared_ui::segments::{SEGMENT_H, draw_segments, switch_width};
use mdshared_ui::textfield::FieldColors;
use mdshared_ui::theme::Palette;

const PAD: f32 = DIALOG_PAD;

pub fn draw(p: &mut Painter, pal: &Palette, s: &mut TaskScreen, w: f32, h: f32, caret_on: bool) {
    let r = task_ui::card(w, h);
    let g = task_ui::geo(r);
    let mut hits: Vec<(Hit, Rect)> = Vec::new();
    draw_backdrop(p, pal, w, h);
    hits.push((Hit::Card, r));
    let footer = draw_dialog_card(p, pal, r);
    let (hover, pressed) = (s.hover, s.pressed);
    let x = r.x + PAD;
    let inner = r.w - 2.0 * PAD;

    // ---- header ---------------------------------------------------------------------------------
    p.text(
        Face::Semibold,
        17.0,
        x,
        r.y + 30.0,
        "AI task on your files",
        pal.text,
        inner - 130.0,
    );
    p.text(
        Face::Ui,
        12.5,
        x,
        r.y + 50.0,
        "They are read here and sent only to the provider you chose. Nothing is opened in a tab.",
        pal.muted,
        inner - 130.0,
    );
    let add_w = dialog_button_w(p, "Add files\u{2026}");
    let add = Rect::new(r.right() - PAD - add_w, r.y + 19.0, add_w, 30.0);
    draw_dialog_button(
        p,
        pal,
        add,
        "Add files\u{2026}",
        false,
        hover == Some(Hit::AddFiles),
        pressed == Some(Hit::AddFiles),
    );
    hits.push((Hit::AddFiles, add));

    // ---- what to do -----------------------------------------------------------------------------
    let labels: Vec<&str> = TaskKind::ALL.iter().map(|k| k.label()).collect();
    let sw_w = switch_width(p, &labels).min(inner);
    let sw = Rect::new(x, g.task.y, sw_w, SEGMENT_H);
    let selected = TaskKind::ALL.iter().position(|k| *k == s.task);
    let hovered = TaskKind::ALL
        .iter()
        .position(|k| hover == Some(Hit::Kind(*k)));
    let segs = draw_segments(p, pal, sw, &labels, selected, hovered);
    for (k, seg) in TaskKind::ALL.iter().zip(segs) {
        hits.push((Hit::Kind(*k), seg));
    }
    p.text_right(
        Face::Ui,
        12.0,
        x + inner,
        g.task.cy(),
        "WHAT TO DO",
        pal.faint,
    );

    // ---- the instruction ------------------------------------------------------------------------
    let field = g.field;
    p.frame(
        field,
        6.0,
        pal.field,
        if s.asking.is_some() {
            pal.line
        } else {
            pal.accent
        },
    );
    hits.push((Hit::Field, field));
    let colors = FieldColors {
        text: pal.text,
        placeholder: pal.faint,
        selection: pal.selection,
        caret: pal.text,
    };
    let enabled = s.asking.is_none();
    s.instruction.draw(
        p,
        Rect::new(field.x + 12.0, field.y, field.w - 24.0, field.h),
        Face::Ui,
        13.5,
        &colors,
        s.task.placeholder(),
        enabled,
        caret_on && s.focus == crate::task_ui::Focus::Instruction,
    );
    if !enabled {
        // What the request is doing, over the field: the window says it where the instruction was.
        p.text(
            Face::Ui,
            12.5,
            field.x + 12.0,
            field.cy(),
            s.asking.as_ref().map(|a| a.what.as_str()).unwrap_or(""),
            pal.muted,
            field.w - 24.0,
        );
    }

    // ---- the saved prompts ----------------------------------------------------------------------
    let prompts = g.prompts;
    p.text(
        Face::Semibold,
        11.0,
        x,
        prompts.y + 8.0,
        "SAVED PROMPTS",
        pal.muted,
        inner,
    );
    let list = Rect::new(prompts.x, prompts.y + 16.0, prompts.w, prompts.h - 16.0);
    if s.prompts.is_empty() {
        p.text(
            Face::Ui,
            12.0,
            x,
            list.cy(),
            if s.task == TaskKind::File {
                "None of this kind yet. A prompt saved with \u{201c}kind: file\u{201d} is listed here."
            } else if s.task == TaskKind::Split {
                "None of this kind yet. A prompt saved with \u{201c}kind: split\u{201d} is listed here."
            } else {
                "None yet. Settings \u{203A} AI \u{203A} Saved prompts \u{203A} Edit adds some."
            },
            pal.faint,
            inner,
        );
    } else {
        let content = s.prompts.len() as f32 * task_ui::PROMPT_ROW;
        s.prompt_scroll = s.prompt_scroll.clamp(0.0, (content - list.h).max(0.0));
        let old = p.clip(list);
        for (k, sp) in s.prompts.iter().enumerate() {
            let row = Rect::new(
                list.x,
                list.y + k as f32 * task_ui::PROMPT_ROW - s.prompt_scroll,
                list.w,
                task_ui::PROMPT_ROW,
            );
            if row.bottom() < list.y || row.y > list.bottom() {
                continue;
            }
            let on = s.prompt_sel == Some(k) || hover == Some(Hit::Prompt(k));
            if on {
                p.rbox(row.inset(2.0), 6.0, pal.hover);
            }
            p.text(
                Face::Semibold,
                13.0,
                row.x + 12.0,
                row.y + 15.0,
                &sp.name,
                pal.text,
                row.w - 160.0,
            );
            p.text(
                Face::Ui,
                11.5,
                row.x + 12.0,
                row.y + 31.0,
                &sp.summary(),
                pal.muted,
                row.w - 160.0,
            );
            hits.push((Hit::Prompt(k), row));
        }
        p.restore(old);
    }

    // ---- where it goes --------------------------------------------------------------------------
    if let Some(field) = s.new_group.as_mut() {
        p.text(
            Face::Semibold,
            11.0,
            x,
            g.dest.y + 12.0,
            "NEW GROUP",
            pal.muted,
            90.0,
        );
        let f = Rect::new(x + 100.0, g.dest.y + 4.0, 260.0, 30.0);
        p.frame(f, 6.0, pal.field, pal.accent);
        hits.push((Hit::NewGroupField, f));
        let colors = FieldColors {
            text: pal.text,
            placeholder: pal.faint,
            selection: pal.selection,
            caret: pal.text,
        };
        field.draw(
            p,
            Rect::new(f.x + 10.0, f.y, f.w - 20.0, f.h),
            Face::Ui,
            13.0,
            &colors,
            "A name for the group",
            true,
            caret_on && s.focus == crate::task_ui::Focus::NewGroup,
        );
        p.text(
            Face::Ui,
            11.5,
            f.right() + 10.0,
            f.cy(),
            "Enter makes it; Esc leaves it.",
            pal.faint,
            (x + inner - f.right() - 20.0).max(0.0),
        );
    } else if s.task == TaskKind::Rewrite {
        p.text(
            Face::Semibold,
            11.0,
            x,
            g.dest.y + 12.0,
            "WHERE IT GOES",
            pal.muted,
            100.0,
        );
        p.text(
            Face::Ui,
            12.5,
            x + 100.0,
            g.dest.y + 16.0,
            "In place: each file is rewritten where it is, and its original goes to AI runs.",
            pal.muted,
            inner - 100.0,
        );
    } else {
        p.text(
            Face::Semibold,
            11.0,
            x,
            g.dest.y + 12.0,
            "WHERE IT GOES",
            pal.muted,
            100.0,
        );
        let row = task_ui::dest_row(x, inner, g.dest.y, s.task == TaskKind::File);
        let button = row.button;
        let open = s.dest_open;
        let fill = if hover == Some(Hit::Dest) || open {
            mix(pal.accent, pal.popup, 0.82)
        } else {
            pal.field
        };
        p.frame(button, 6.0, fill, if open { pal.accent } else { pal.line });
        p.text(
            Face::Ui,
            13.0,
            button.x + 10.0,
            button.cy(),
            &s.dest_text(),
            pal.text,
            button.w - 30.0,
        );
        p.icon(
            glyph::DOWN,
            12.0,
            button.right() - 18.0,
            button.cy(),
            pal.muted,
        );
        hits.push((Hit::Dest, button));

        if let Some(sec) = row.section {
            p.frame(sec, 6.0, pal.field, pal.line);
            hits.push((Hit::Section, sec));
            let colors = FieldColors {
                text: pal.text,
                placeholder: pal.faint,
                selection: pal.selection,
                caret: pal.text,
            };
            s.section.draw(
                p,
                Rect::new(sec.x + 10.0, sec.y, sec.w - 20.0, sec.h),
                Face::Ui,
                12.5,
                &colors,
                "under a section (none)",
                true,
                caret_on && s.focus == crate::task_ui::Focus::Section,
            );
        }
        // The arrow, then the path; each plan deciding has no path.
        let (lead, folder) = match s.dest_folder() {
            Some(path) => ("\u{2192} ", shown(&path)),
            None => match s.dest {
                DestChoice::Decide => ("", "each plan decides".to_string()),
                _ => ("", String::new()),
            },
        };
        // Right-aligned in the room the row leaves it, the path cut from its left so its end
        // (the group's folder, the section's) stays readable; left out with no room.
        if let (false, Some(room)) = (folder.is_empty(), row.folder) {
            let lead_w = p.measure(Face::Ui, 11.5, lead);
            let fit = crate::library_pane::elide_left(p, 11.5, &folder, room.w - lead_w);
            if !fit.is_empty() {
                let text = format!("{lead}{fit}");
                p.text_right(Face::Ui, 11.5, room.right(), room.cy(), &text, pal.faint);
            }
        }
    }

    // ---- the files ------------------------------------------------------------------------------
    let rows = task_ui::rows(s);
    let content = task_ui::content_h(&rows);
    s.list.area = g.list;
    s.list.view = g.list.h;
    s.list.content = content;
    s.list.scroll = s.list.scroll.clamp(0.0, s.list.max_scroll());
    hits.push((Hit::Body, g.list));
    let old = p.clip(g.list);
    let mut y = g.list.y + 4.0 - s.list.scroll;
    for row in &rows {
        match row {
            ListRow::Heading(kind) => {
                if y + GROUP_H > g.list.y && y < g.list.bottom() {
                    p.text(
                        Face::Semibold,
                        10.5,
                        g.list.x + PAD,
                        y + GROUP_H / 2.0,
                        kind.heading(s.task),
                        pal.muted,
                        g.list.w - 2.0 * PAD,
                    );
                }
                y += GROUP_H;
            }
            ListRow::Placed => {
                if y + GROUP_H > g.list.y && y < g.list.bottom() {
                    p.text(
                        Face::Semibold,
                        10.5,
                        g.list.x + PAD,
                        y + GROUP_H / 2.0,
                        "PLACED \u{00B7} WHAT EACH BECOMES",
                        pal.muted,
                        g.list.w - 2.0 * PAD,
                    );
                }
                y += GROUP_H;
            }
            ListRow::Unplaced => {
                if y + GROUP_H > g.list.y && y < g.list.bottom() {
                    p.fill(
                        Rect::new(g.list.x + 1.0, y + GROUP_H - p.px(), g.list.w - 2.0, p.px()),
                        pal.popup_line,
                    );
                    p.text(
                        Face::Semibold,
                        10.5,
                        g.list.x + PAD,
                        y + GROUP_H / 2.0,
                        "NOT PLACED \u{00B7} THE MODEL DID NOT SAY WHERE THEY GO",
                        pal.muted,
                        g.list.w - 2.0 * PAD,
                    );
                }
                y += GROUP_H;
            }
            ListRow::File(i) => {
                let f = &s.files[*i];
                let row = Rect::new(g.list.x + 8.0, y + 1.0, g.list.w - 16.0, ROW_H - 2.0);
                if row.bottom() > g.list.y && row.y < g.list.bottom() {
                    if hover == Some(Hit::File(*i)) {
                        p.rbox(row, 5.0, mix(pal.popup, pal.text, 0.05));
                    }
                    let reason = f.reason_for(s.task);
                    let enabled = reason.is_none() && !f.done && s.asking.is_none();
                    let tick_at = Rect::new(row.x + 12.0, row.cy() - 8.0, 16.0, 16.0);
                    draw_tick(
                        p,
                        pal,
                        tick_at,
                        Tick {
                            on: (f.ticked && reason.is_none()) || f.done,
                            enabled,
                            focused: false,
                        },
                    );
                    hits.push((Hit::File(*i), row));
                    let name_x = tick_at.right() + 12.0;
                    p.text(
                        Face::Ui,
                        13.0,
                        name_x,
                        row.y + 12.0,
                        &f.name,
                        if enabled || f.done {
                            pal.text
                        } else {
                            pal.muted
                        },
                        (row.w - 260.0).max(80.0),
                    );
                    let sub = if let Some(why) = &reason {
                        why.clone()
                    } else if let Some(with) = f.with {
                        let target = s
                            .files
                            .get(with)
                            .map(|t| t.title.clone().unwrap_or_else(|| t.name.clone()))
                            .unwrap_or_default();
                        format!("with {target}")
                    } else if !f.sections.is_empty() {
                        format!("in {}", f.sections.join(" \u{203A} "))
                    } else if s.planned {
                        "not placed".to_string()
                    } else if f.done {
                        "written".to_string()
                    } else if f.kind == FileKind::Text {
                        format!("{} \u{00B7} read and sent", task_ui::size_text(f.size))
                    } else {
                        task_ui::size_text(f.size)
                    };
                    p.text(
                        Face::Ui,
                        11.0,
                        name_x,
                        row.y + 26.0,
                        &sub,
                        if reason.is_some() {
                            pal.faint
                        } else {
                            pal.muted
                        },
                        (row.w - 260.0).max(80.0),
                    );
                    // Where it goes, on the plan face and after the answers.
                    let dest = if f.done {
                        "written".to_string()
                    } else if s.planned {
                        match (f.title.as_ref(), f.sections.first()) {
                            (Some(t), Some(sec)) => format!("{t} \u{2192} {sec}"),
                            (Some(t), None) => t.clone(),
                            (None, Some(sec)) => format!("\u{2192} {sec}"),
                            (None, None) => "not placed".to_string(),
                        }
                    } else if f.kind == FileKind::Other {
                        if reason.is_some() {
                            String::new()
                        } else {
                            "linked by name".to_string()
                        }
                    } else if f.kind == FileKind::Picture {
                        if s.task == TaskKind::File {
                            "linked".to_string()
                        } else {
                            String::new()
                        }
                    } else {
                        String::new()
                    };
                    if !dest.is_empty() {
                        p.text_right(
                            Face::Ui,
                            11.5,
                            row.right() - 12.0,
                            row.cy(),
                            &dest,
                            if f.done { pal.accent } else { pal.muted },
                        );
                    }
                }
                y += ROW_H;
            }
        }
    }
    p.restore(old);
    if let Some((track, thumb)) = s.list.bar() {
        draw_scrollbar(
            p,
            pal,
            track,
            thumb,
            hover == Some(Hit::Track) || s.list.grab.is_some(),
        );
        hits.push((Hit::Track, track));
        hits.push((Hit::Thumb, thumb));
    }
    if s.files.is_empty() {
        p.text_center(
            Face::Ui,
            13.0,
            Rect::new(g.list.x, g.list.y + 40.0, g.list.w, 24.0),
            "Add files\u{2026} or drop them here from File Explorer.",
            pal.faint,
        );
    }

    // ---- the destination menu -------------------------------------------------------------------
    if s.dest_open {
        draw_dest_menu(p, pal, s, &mut hits, hover);
    }

    // ---- footer --------------------------------------------------------------------------------
    let by = footer.y + (DIALOG_FOOTER - 32.0) / 2.0;
    let mut bx = r.right() - PAD;
    let n = s.count();
    let ai_off = s.task == TaskKind::Rewrite && !s.ai_on;
    let primary_enabled = !ai_off && n > 0 && s.asking.is_none();
    let label = if ai_off {
        "The AI connection is off".to_string()
    } else {
        s.primary_label()
    };
    let pw = dialog_button_w(p, &label);
    bx -= pw;
    let primary = Rect::new(bx, by, pw, 32.0);
    if primary_enabled {
        draw_dialog_button(
            p,
            pal,
            primary,
            &label,
            true,
            hover == Some(Hit::Primary),
            pressed == Some(Hit::Primary),
        );
        hits.push((Hit::Primary, primary));
    } else {
        draw_dialog_button_disabled(p, pal, primary, &label, true);
        if !ai_off && s.blocked.is_none() {
            // Nothing ticked: the button says why when the pointer is on it.
            if hover == Some(Hit::Primary) && n == 0 {
                p.text_right(
                    Face::Ui,
                    11.5,
                    primary.x - 10.0,
                    footer.cy(),
                    "None of these files takes part.",
                    pal.faint,
                );
            }
            hits.push((Hit::Primary, primary));
        }
    }
    bx -= 10.0;
    if ai_off {
        let w = dialog_button_w(p, "Open AI settings");
        bx -= w;
        let b = Rect::new(bx, by, w, 32.0);
        draw_dialog_button(
            p,
            pal,
            b,
            "Open AI settings",
            false,
            hover == Some(Hit::OpenSettings),
            pressed == Some(Hit::OpenSettings),
        );
        hits.push((Hit::OpenSettings, b));
        bx -= 10.0;
    }
    let cw = dialog_button_w(p, "Cancel");
    bx -= cw;
    let cancel = Rect::new(bx, by, cw, 32.0);
    draw_dialog_button(
        p,
        pal,
        cancel,
        "Cancel",
        false,
        hover == Some(Hit::Cancel),
        pressed == Some(Hit::Cancel),
    );
    hits.push((Hit::Cancel, cancel));
    bx -= 16.0;
    // "Show what is sent", a link at the left of the buttons.
    let link_w = p.measure(Face::Ui, 12.5, "Show what is sent");
    let link = Rect::new(bx - link_w, by, link_w + 12.0, 32.0);
    p.text(
        Face::Ui,
        12.5,
        link.x + 6.0,
        footer.cy(),
        "Show what is sent",
        if hover == Some(Hit::ShowSent) {
            pal.accent
        } else {
            mix(pal.accent, pal.text, 0.25)
        },
        link.w,
    );
    hits.push((Hit::ShowSent, link));
    // What goes and what it costs, and why it cannot, at the left.
    let room = (link.x - x - 12.0).max(0.0);
    let sum = s.summary();
    let note = if let Some(why) = &s.blocked {
        why.clone()
    } else if ai_off {
        "The AI connection is off. Rewrite them needs it; File them and Split them do not."
            .to_string()
    } else {
        format!(
            "{} \u{00B7} {}",
            s.via,
            if s.asks_ai() {
                "sent to it"
            } else {
                "no request"
            }
        )
    };
    p.text(
        Face::Semibold,
        12.0,
        x,
        footer.cy() - 8.0,
        &sum,
        pal.text,
        room,
    );
    p.text(
        Face::Ui,
        11.0,
        x,
        footer.cy() + 9.0,
        &note,
        if s.blocked.is_some() || ai_off {
            pal.faint
        } else {
            pal.muted
        },
        room,
    );

    s.hits = hits;
    s.caret_on = caret_on;
}

/// The destination list: the groups, "A new group…", and "Decide on the plan".
fn draw_dest_menu(
    p: &mut Painter,
    pal: &Palette,
    s: &mut TaskScreen,
    hits: &mut Vec<(Hit, Rect)>,
    hover: Option<Hit>,
) {
    let mut items: Vec<(String, bool)> = s.groups.iter().map(|g| (g.name.clone(), true)).collect();
    items.push(("A new group\u{2026}".to_string(), true));
    items.push(("Decide on the plan".to_string(), s.task == TaskKind::Split));
    let row_h = 30.0;
    let h = row_h * items.len() as f32 + 8.0;
    let g = task_ui::geo(task_ui::card(s.win.0, s.win.1));
    let width = 300.0;
    let below = g.dest.bottom() + 2.0;
    let above = g.dest.y - h - 2.0;
    let y = if below + h < g.footer.y {
        below
    } else {
        above.max(g.header.bottom())
    };
    let r = Rect::new(g.dest.x + 100.0, y, width, h);
    p.frame(r, 7.0, pal.popup, pal.popup_line);
    for (k, (label, enabled)) in items.iter().enumerate() {
        let row = Rect::new(r.x + 4.0, r.y + 4.0 + k as f32 * row_h, r.w - 8.0, row_h);
        let hot = hover == Some(Hit::DestItem(k)) && *enabled;
        if hot {
            p.rbox(row, 5.0, pal.hover);
        }
        p.text(
            Face::Ui,
            13.0,
            row.x + 10.0,
            row.cy(),
            label,
            if *enabled { pal.text } else { pal.faint },
            row.w - 20.0,
        );
        if *enabled {
            hits.push((Hit::DestItem(k), row));
        }
    }
}

/// A path as a row shows it: its last three parts, closing with the separator.
fn shown(path: &std::path::Path) -> String {
    let mut parts: Vec<String> = path
        .components()
        .filter_map(|c| match c {
            std::path::Component::Normal(n) => Some(n.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect();
    let sep = std::path::MAIN_SEPARATOR.to_string();
    let cut = parts.len().saturating_sub(3);
    let tail = parts.split_off(cut);
    let lead = if cut > 0 {
        "\u{2026}".to_string()
    } else {
        String::new()
    };
    format!("{lead}{}{}", tail.join(&sep), std::path::MAIN_SEPARATOR)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task_ui::Group;
    use mdshared_text::Bitmap;
    use mdshared_ui::gfx::Gfx;
    use std::path::PathBuf;

    /// File them over a group whose folder is `home`, drawn into a `w` x 800 bitmap: the
    /// pixels, and the button's and the section field's places.
    fn drawn(home: &str, w: u32) -> (Bitmap, Rect, Rect) {
        let mut s = TaskScreen::new(
            TaskKind::File,
            Vec::new(),
            DestChoice::Group(0),
            Vec::new(),
            "Claude".into(),
        );
        s.groups = vec![Group {
            name: "test".into(),
            file: PathBuf::from("test.md"),
            home: PathBuf::from(home),
        }];
        let mut bm = Bitmap::new(w, 800);
        let mut gfx = Gfx::new();
        {
            let mut p = Painter::new(&mut bm, &mut gfx, 1.0);
            draw(
                &mut p,
                &mdshared_ui::theme::LIGHT,
                &mut s,
                w as f32,
                800.0,
                false,
            );
        }
        let at = |h: Hit| {
            s.hits
                .iter()
                .find(|(k, _)| *k == h)
                .map(|(_, r)| *r)
                .unwrap()
        };
        (bm, at(Hit::Dest), at(Hit::Section))
    }

    fn same(a: &Bitmap, b: &Bitmap, r: Rect) -> bool {
        let (x0, y0) = (r.x.floor() as u32, r.y.floor() as u32);
        let (x1, y1) = (r.right().ceil() as u32, r.bottom().ceil() as u32);
        (y0..y1.min(a.height)).all(|y| {
            (x0..x1.min(a.width)).all(|x| {
                let o = ((y * a.width + x) * 4) as usize;
                a.pixels[o..o + 3] == b.pixels[o..o + 3]
            })
        })
    }

    /// The owner's report (2026-11-24): the folder the files go to was drawn over the "under a
    /// section (none)" field. At every width the window allows, the button and the field draw
    /// the same with a long folder beside them as with a one-letter one, and both stay inside the card;
    /// where there is room the folder is drawn.
    #[test]
    fn the_folder_never_draws_over_the_section_field_or_the_button() {
        let long = if cfg!(windows) {
            r"C:\Users\someone\AppData\Local\mdedit\library\Recipes from my grandmother's kitchen"
        } else {
            "/home/someone/.local/share/mdedit/library/Recipes from my grandmother's kitchen"
        };
        let mut shown_somewhere = false;
        for w in (520..=1600).step_by(40) {
            let (with, button, section) = drawn(long, w);
            // A one-letter folder, drawn at the row's right end, far from both.
            let (without, button2, section2) = drawn("x", w);
            assert_eq!((button, section), (button2, section2), "{w}");
            let card = task_ui::card(w as f32, 800.0);
            let right = card.right() - PAD;
            assert!(
                button.right() <= section.x && section.right() <= right,
                "{w}"
            );
            assert!(same(&with, &without, button.inset(-1.0)), "button at {w}");
            assert!(same(&with, &without, section.inset(-1.0)), "section at {w}");
            let folder = Rect::new(
                section.right() + 1.0,
                section.y,
                right - section.right(),
                30.0,
            );
            shown_somewhere |= folder.w > 1.0 && !same(&with, &without, folder);
        }
        assert!(shown_somewhere, "the folder is drawn where there is room");
    }
}
