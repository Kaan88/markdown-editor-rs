//! Drawing the AI task window over files (`docs/ai-library-plan.md` 5.6): the instruction, the
//! saved prompts of the task's kind, the files under their three headings, where they go, and
//! the footer. The state and the layout numbers are `task_ui.rs`; the window's part is
//! `app/task.rs`.

use crate::notes_task::{self, Measure};
use crate::task_ui::{
    self, DestChoice, FileKind, GROUP_H, Hit, ListRow, PICTURE_ROW_H, ROW_H, THUMB, TaskKind,
    TaskScreen,
};
use mdshared_text::Bitmap;
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
    // The size the window is drawn at, for what is placed from it later (the group menu, the
    // Add menu's lines): the window may have been resized since it opened.
    s.win = (w, h);
    let r = task_ui::card(w, h);
    let mut g = task_ui::geo(r);
    // The first-time sentence of Write notes takes a band of the list's foot (pictures plan 13).
    let whole = crate::sent_card::shows_whole(s);
    if whole {
        g.list.h = (g.list.h - crate::sent_card::WHOLE_H).max(0.0);
    }
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
        "AI task",
        pal.text,
        inner - 130.0,
    );
    let under = match &s.from {
        Some(from) => format!("{from} Read here and sent only to the provider you chose."),
        // The window opens none of them in a tab; other tabs may well be open (finding 17).
        None => task_ui::HEADER_UNDER.to_string(),
    };
    // The line under the title is drawn with the footer's link, which may take its room.
    let add_w = dialog_button_w(p, "Add files\u{2026}");
    let add = Rect::new(r.right() - PAD - add_w, r.y + 19.0, add_w, 30.0);
    draw_dialog_button(
        p,
        pal,
        add,
        "Add\u{2026}",
        false,
        hover == Some(Hit::AddFiles) || s.add_open,
        pressed == Some(Hit::AddFiles),
    );
    hits.push((Hit::AddFiles, add));

    // ---- what to do -----------------------------------------------------------------------------
    // On File them THE FILES needs room beside it: in a narrow window the tasks lose their
    // "them" first ("File · Split · Rewrite"), so the three ways the files go stay whole.
    let mut labels: Vec<&str> = TaskKind::ALL.iter().map(|k| k.label()).collect();
    let files_need = if s.task == TaskKind::File {
        switch_width(p, &["Link", "Copy", "Move"]) + 16.0
    } else {
        0.0
    };
    if switch_width(p, &labels) + files_need > inner {
        labels = TaskKind::ALL
            .iter()
            .map(|k| k.label().trim_end_matches(" them"))
            .collect();
    }
    // Narrower still (520 px), "Write notes" is "Notes", so "Move" keeps its padding (review of
    // the task fixes, 12).
    if switch_width(p, &labels) + files_need > inner {
        labels = TaskKind::ALL
            .iter()
            .map(|k| match k {
                TaskKind::Notes => "Notes",
                k => k.label().trim_end_matches(" them"),
            })
            .collect();
    }
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
    if s.task == TaskKind::File {
        // THE FILES (pictures plan 4.3, D-209): beside the task, right-aligned in the room the
        // task switch leaves; short words when the full ones do not fit, no caption without
        // room for it.
        use mdedit_core::prefs::FilesGo;
        let room = inner - sw_w - 16.0;
        let full = ["Link them", "Copy them in", "Move them in"];
        let short = ["Link", "Copy in", "Move in"];
        let shortest = ["Link", "Copy", "Move"];
        let labels: &[&str] = if switch_width(p, &full) <= room {
            &full
        } else if switch_width(p, &short) <= room {
            &short
        } else {
            &shortest
        };
        let fw = switch_width(p, labels).min(room.max(0.0));
        let caption = "THE FILES";
        let cw = p.measure(Face::Semibold, 11.0, caption) + 10.0;
        let fr = Rect::new(x + inner - fw, g.task.y, fw, SEGMENT_H);
        if fw + cw <= room {
            p.text_right(
                Face::Semibold,
                11.0,
                fr.x - 10.0,
                g.task.cy(),
                caption,
                pal.muted,
            );
        }
        let selected = FilesGo::ALL.iter().position(|v| *v == s.files_go);
        let hovered = FilesGo::ALL
            .iter()
            .position(|v| hover == Some(Hit::FilesGo(*v)));
        let segs = draw_segments(p, pal, fr, labels, selected, hovered);
        for (v, seg) in FilesGo::ALL.iter().zip(segs) {
            hits.push((Hit::FilesGo(*v), seg));
        }
    }

    // ---- the instruction ------------------------------------------------------------------------
    let field = g.field;
    let busy = s.busy();
    p.frame(
        field,
        6.0,
        pal.field,
        if busy { pal.line } else { pal.accent },
    );
    hits.push((Hit::Field, field));
    let colors = FieldColors {
        text: pal.text,
        placeholder: pal.faint,
        selection: pal.selection,
        caret: pal.text,
    };
    let enabled = !busy;
    // While a request is out the field says what it is doing instead of the instruction: the
    // two drawn together read as neither.
    if enabled {
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
    } else {
        // What the request is doing, over the field: the window says it where the instruction was.
        let what = match (&s.asking, &s.notes_run) {
            (Some(a), _) => a.what.clone(),
            (None, Some(run)) => notes_task::progress(run, &s.via),
            (None, None) => String::new(),
        };
        p.text(
            Face::Ui,
            12.5,
            field.x + 12.0,
            field.cy(),
            &what,
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
        // Right under its label, and on a second line in a narrow window rather than cut.
        let text = task_ui::prompts_empty(s.task);
        let rows = crate::hunks_ui::wrap(&text, inner, &mut |t| p.measure(Face::Ui, 12.0, t));
        for (k, row) in rows.into_iter().take(2).enumerate() {
            let line = text.get(row).unwrap_or("").trim();
            p.text(
                Face::Ui,
                12.0,
                x,
                list.y + 12.0 + k as f32 * 18.0,
                line,
                pal.faint,
                inner,
            );
        }
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
        // Write notes keeps its Options at the row's end (pictures plan 5).
        let options = (s.task == TaskKind::Notes).then(|| {
            let w = dialog_button_w(p, "Options\u{2026}");
            Rect::new(x + inner - w, g.dest.y + 2.0, w, 30.0)
        });
        let row_w = options.map_or(inner, |o| o.x - 12.0 - x);
        let row = task_ui::dest_row(x, row_w, g.dest.y, s.task == TaskKind::File);
        if let Some(o) = options {
            draw_dialog_button(
                p,
                pal,
                o,
                "Options\u{2026}",
                false,
                hover == Some(Hit::Options) || s.options_open,
                pressed == Some(Hit::Options),
            );
            hits.push((Hit::Options, o));
        }
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
                        if *kind == FileKind::Picture && s.task == TaskKind::Notes {
                            notes_task::pictures_heading(s.notes.strip, s.ai_on)
                        } else if *kind == FileKind::Text && s.task == TaskKind::Notes && !s.ai_on {
                            "TEXT \u{00B7} NOT USED WITH THE AI OFF"
                        } else {
                            kind.heading(s.task)
                        },
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
            ListRow::NotSent => {
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
                        if s.answers.is_empty() {
                            "LEFT OUT \u{00B7} EACH ROW SAYS WHY"
                        } else {
                            "NOT SENT \u{00B7} EACH ROW SAYS WHY"
                        },
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
            ListRow::Picture(i) => {
                draw_picture_row(p, pal, s, *i, y, g.list, &mut hits);
                y += PICTURE_ROW_H;
            }
            ListRow::File(i) => {
                let f = &s.files[*i];
                let row = Rect::new(g.list.x + 8.0, y + 1.0, g.list.w - 16.0, ROW_H - 2.0);
                if row.bottom() > g.list.y && row.y < g.list.bottom() {
                    if hover == Some(Hit::File(*i)) {
                        p.rbox(row, 5.0, mix(pal.popup, pal.text, 0.05));
                    }
                    let reason = s.reason_of(*i);
                    let enabled = reason.is_none() && !f.done && !busy;
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
                        match s.copy_note(*i).filter(|_| s.planned && !f.done) {
                            Some(note) => {
                                format!("in {} \u{00B7} {note}", f.sections.join(" \u{203A} "))
                            }
                            None => format!("in {}", f.sections.join(" \u{203A} ")),
                        }
                    } else if s.planned && !s.was_sent(*i) {
                        s.left_out_note().to_string()
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
                            (None, None) if s.was_sent(*i) => "not placed".to_string(),
                            (None, None) => String::new(),
                        }
                    } else if f.kind == FileKind::Other {
                        if reason.is_some() {
                            String::new()
                        } else {
                            "linked by name".to_string()
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
            "Add\u{2026} a file, or drop files here from File Explorer.",
            pal.faint,
        );
    }

    // ---- what the window says, the width of the card, over the list's foot -------------------
    // What was just done in it first (finding 7), then what stopped Write notes.
    let run_notice = s
        .notes_run
        .as_ref()
        .filter(|_| s.task == TaskKind::Notes)
        .and_then(|run| {
            if let Some(why) = run.fault.as_ref() {
                Some(why.clone())
            } else if let Some(why) = run.arrange_failed.as_ref().filter(|_| !s.busy()) {
                Some(format!(
                    "The notes could not be arranged: {why} What was looked at is kept."
                ))
            } else if let Some((b, why)) = run.failed.first().filter(|_| !s.busy()) {
                Some(format!(
                    "Request {} of {} failed: {why} What was looked at is kept.",
                    b + 1,
                    s.notes_run_requests(run)
                ))
            } else {
                None
            }
        });
    if let Some(ask) = s.ask_line() {
        // A folder add of more than ASK_FILES files asks first, where the window says things
        // (D-249): the question, then its two buttons. Nothing is in the window yet.
        let bar = Rect::new(
            g.list.x,
            (g.list.bottom() - 32.0).max(g.list.y),
            g.list.w,
            32.0_f32.min(g.list.h.max(0.0)),
        );
        p.fill(bar, pal.popup);
        p.fill(Rect::new(bar.x, bar.y, bar.w, p.px()), pal.popup_line);
        if bar.h >= 16.0 {
            let add_label = "Add them";
            let leave_label = "Leave them out";
            let aw = dialog_button_w(p, add_label);
            let lw = dialog_button_w(p, leave_label);
            let bh = 24.0_f32.min(bar.h - 6.0);
            let by = bar.cy() - bh / 2.0;
            let add = Rect::new(bar.right() - 8.0 - aw, by, aw, bh);
            let leave = Rect::new(add.x - 8.0 - lw, by, lw, bh);
            p.icon(glyph::WARN, 13.0, x + 7.0, bar.cy(), pal.muted);
            let room = (leave.x - 12.0 - (x + 22.0)).max(0.0);
            p.text(Face::Ui, 12.0, x + 22.0, bar.cy(), &ask, pal.text, room);
            draw_dialog_button(
                p,
                pal,
                leave,
                leave_label,
                false,
                hover == Some(Hit::AskLeave),
                pressed == Some(Hit::AskLeave),
            );
            draw_dialog_button(
                p,
                pal,
                add,
                add_label,
                true,
                hover == Some(Hit::AskAdd),
                pressed == Some(Hit::AskAdd),
            );
            hits.push((Hit::AskLeave, leave));
            hits.push((Hit::AskAdd, add));
        }
    } else if let Some(notice) = s.reading_line().or_else(|| s.notice.clone()).or(run_notice) {
        {
            // In a list lower than the bar, the bar keeps to the list's top: it is never drawn
            // over the rows above the list (review of the task fixes, 13).
            let bar = Rect::new(
                g.list.x,
                (g.list.bottom() - 32.0).max(g.list.y),
                g.list.w,
                32.0_f32.min(g.list.h.max(0.0)),
            );
            p.fill(bar, pal.popup);
            p.fill(Rect::new(bar.x, bar.y, bar.w, p.px()), pal.popup_line);
            // Too low for a line of text, the bar is a band only (Fable's second review, 2).
            if bar.h >= 16.0 {
                p.icon(glyph::WARN, 13.0, x + 7.0, bar.cy(), pal.muted);
                p.text(
                    Face::Ui,
                    12.0,
                    x + 22.0,
                    bar.cy(),
                    &notice,
                    pal.text,
                    inner - 22.0,
                );
            }
        }
    }

    // ---- footer --------------------------------------------------------------------------------
    let by = footer.y + (DIALOG_FOOTER - 32.0) / 2.0;
    let mut bx = r.right() - PAD;
    let ai_off = s.task == TaskKind::Rewrite && !s.ai_on;
    let notes = s.task == TaskKind::Notes;
    let run = s.notes_run.as_ref().filter(|_| notes);
    // Write notes: Stop while it looks or arranges; after a failed look, "Try it again", and
    // "Go on with N" beside it; when the model reads no pictures, the settings.
    let stopping = run.is_some_and(|r| r.looking() || r.arrange.is_some());
    let failed = run.filter(|r| !stopping && !r.failed.is_empty());
    let arrange_failed = run.is_some_and(|r| !stopping && r.arrange_failed.is_some());
    let fault = run.and_then(|r| r.fault.clone()).filter(|_| !stopping);
    let notes_blocked = if notes { s.notes_blocked() } else { None };
    // No file, or no group for the plan face: the button is off and the footer says why.
    let primary_blocked = s.primary_blocked();
    let primary_enabled = if stopping {
        true
    } else {
        !ai_off
            && primary_blocked.is_none()
            && !busy
            && s.ask.is_none()
            && (notes_blocked.is_none() || !s.ai_on)
    };
    let label = if ai_off {
        "The AI connection is off".to_string()
    } else if stopping {
        "Stop".to_string()
    } else if failed.is_some() || arrange_failed {
        "Try it again".to_string()
    } else {
        s.primary_label()
    };
    let primary_hit = if failed.is_some() || arrange_failed {
        Hit::TryAgain
    } else {
        Hit::Primary
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
            hover == Some(primary_hit),
            pressed == Some(primary_hit),
        );
        hits.push((primary_hit, primary));
    } else {
        draw_dialog_button_disabled(p, pal, primary, &label, true);
        if !ai_off && s.blocked.is_none() {
            // Off, it still takes a click: on the plan face the click says in the window what is
            // missing (a group, a placed file); with no file it does nothing. The footer says
            // why either way.
            hits.push((Hit::Primary, primary));
        }
    }
    bx -= 10.0;
    let go_on = if arrange_failed {
        // With text files only, or no picture looked at, there is no picture to make a note of.
        let looked = s
            .notes_parts()
            .0
            .into_iter()
            .any(|i| run.is_some_and(|r| r.look(i).is_some()));
        looked.then(|| "One note per picture instead".to_string())
    } else {
        // Going on with nothing to make notes of is not offered: "Try it again" is the way.
        failed
            .filter(|run| run.can_go_on())
            .map(|run| run.go_on_label())
    };
    if let Some(go_on) = go_on {
        let w = dialog_button_w(p, &go_on);
        bx -= w;
        let b = Rect::new(bx, by, w, 32.0);
        draw_dialog_button(
            p,
            pal,
            b,
            &go_on,
            false,
            hover == Some(Hit::GoOn),
            pressed == Some(Hit::GoOn),
        );
        hits.push((Hit::GoOn, b));
        bx -= 10.0;
    }
    if ai_off || fault.is_some() {
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
    // "Show what is sent", a link at the left of the buttons; in the header, left of Add…,
    // when the buttons leave the footer too little room for it and the summary (finding 20).
    let link_w = p.measure(Face::Ui, 12.5, "Show what is sent");
    let in_footer = bx - link_w - 12.0 >= x + 140.0;
    let link = if in_footer {
        Rect::new(bx - link_w, by, link_w + 12.0, 32.0)
    } else {
        Rect::new(add.x - 12.0 - link_w - 12.0, add.y, link_w + 12.0, 30.0)
    };
    p.text(
        Face::Ui,
        12.5,
        link.x + 6.0,
        link.cy(),
        "Show what is sent",
        if hover == Some(Hit::ShowSent) {
            pal.accent
        } else {
            mix(pal.accent, pal.text, 0.25)
        },
        link.w,
    );
    hits.push((Hit::ShowSent, link));
    // The line under the title, up to the link when the link is in the header, else up to Add…;
    // in words short enough to be read whole when the long ones would be cut (review of the
    // task fixes, 12).
    let under_room = if in_footer {
        add.x - x - 12.0
    } else {
        link.x - x - 4.0
    };
    // With the Add menu open, the line stops short of it too, in shorter words rather than
    // running under it (Fable's second review, 1).
    let under_room = if s.add_open {
        under_room.min(add.right() - ADD_MENU_W - 8.0 - x)
    } else {
        under_room
    }
    .max(0.0);
    let under = if s.from.is_none() {
        let shorter = [
            under.as_str(),
            "Read where they are, sent only to your provider.",
            "Sent only to the provider you chose.",
            "Sent only to your provider.",
        ];
        shorter
            .iter()
            .find(|t| p.measure(Face::Ui, 12.5, t) <= under_room)
            .or(shorter.last())
            .map_or_else(String::new, |t| t.to_string())
    } else {
        under
    };
    p.text(Face::Ui, 12.5, x, r.y + 50.0, &under, pal.muted, under_room);
    // What goes and what it costs, and why it cannot, at the left.
    let room = if in_footer {
        (link.x - x - 12.0).max(0.0)
    } else {
        (bx - x).max(0.0)
    };
    let sum = s.summary();
    let moving = s.task == TaskKind::File && s.files_go == mdedit_core::prefs::FilesGo::Move;
    let note = if let Some(why) = &s.blocked {
        why.clone()
    } else if let Some(why) = primary_blocked.as_ref().filter(|_| !stopping && !ai_off) {
        why.clone()
    } else if fault.is_some() {
        "Choose a model that reads pictures in Settings \u{203A} AI.".to_string()
    } else if let Some(run) = failed {
        s.notes_failed_note(run)
    } else if let Some(why) = notes_blocked.as_ref().filter(|_| s.ai_on) {
        why.clone()
    } else if notes && s.ai_on && s.notes_parts().0.is_empty() {
        format!("{} \u{00B7} sent to it, no picture", s.via)
    } else if notes && s.ai_on {
        format!("{} \u{00B7} the pictures themselves are sent", s.via)
    } else if notes {
        "The AI is off: the notes are made here, and nothing is sent.".to_string()
    } else if moving {
        task_ui::MOVE_WARNING.to_string()
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
        if moving && s.blocked.is_none() {
            pal.text
        } else if s.blocked.is_some()
            || (primary_blocked.is_some() && !stopping)
            || ai_off
            || fault.is_some()
            || failed.is_some()
            || (notes_blocked.is_some() && s.ai_on)
        {
            pal.faint
        } else {
            pal.muted
        },
        room,
    );

    // ---- once ever: a picture is sent whole, above the button --------------------------------
    if whole {
        let band = Rect::new(x, g.list.bottom() + 2.0, inner, crate::sent_card::WHOLE_H);
        let text_w = p.measure(Face::Semibold, 12.0, crate::sent_card::WHOLE);
        let right = band.right().min(primary.right());
        let left = (right - text_w).max(band.x + 18.0);
        p.icon(glyph::WARN, 13.0, left - 11.0, band.cy(), pal.muted);
        p.text(
            Face::Semibold,
            12.0,
            left,
            band.cy(),
            crate::sent_card::WHOLE,
            pal.text,
            right - left,
        );
    }

    // The menus and Options last, over the notice, the footer and the line under the title
    // (drawn with the footer's link), so nothing is drawn over them and their hits come first
    // (finding 15; Fable's second review, 1).
    if s.dest_open {
        draw_dest_menu(p, pal, s, &mut hits, hover);
    }
    if s.add_open {
        draw_add_menu(p, pal, s, add, &mut hits, hover);
    }
    if s.options_open && s.task == TaskKind::Notes {
        draw_options(p, pal, s, &mut hits, hover);
    }

    s.hits = hits;
    s.caret_on = caret_on;

    // ---- "What is sent", over the window ----------------------------------------------------------
    if let Some(card) = s.sent.as_mut() {
        crate::sent_card_draw::draw(p, pal, card, &s.files, w, h);
    }
}

/// A picture's row on Write notes: its tick, its thumbnail as the engine made it, its name, what
/// the model said of it (or its own pixels and size), and at the right what is sent.
fn draw_picture_row(
    p: &mut Painter,
    pal: &Palette,
    s: &TaskScreen,
    i: usize,
    y: f32,
    list: Rect,
    hits: &mut Vec<(Hit, Rect)>,
) {
    let Some(f) = s.files.get(i) else { return };
    let row = Rect::new(list.x + 8.0, y + 1.0, list.w - 16.0, PICTURE_ROW_H - 2.0);
    if row.bottom() <= list.y || row.y >= list.bottom() {
        return;
    }
    let hover = s.hover;
    if hover == Some(Hit::File(i)) {
        p.rbox(row, 5.0, mix(pal.popup, pal.text, 0.05));
    }
    let reason = s.reason_of(i);
    let enabled = reason.is_none() && !f.done && !s.busy();
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
    hits.push((Hit::File(i), row));
    let thumb = Rect::new(tick_at.right() + 12.0, row.cy() - THUMB / 2.0, THUMB, THUMB);
    draw_thumb(p, pal, thumb, f.thumb.as_ref());
    let name_x = thumb.right() + 12.0;
    let right_w = 250.0;
    let text_w = (row.right() - 12.0 - right_w - name_x).max(80.0);
    let run = s.notes_run.as_ref();
    let look = run.and_then(|r| r.look(i));
    p.text(
        Face::Ui,
        13.0,
        name_x,
        row.y + 17.0,
        &f.name,
        if enabled || f.done || s.busy() {
            pal.text
        } else {
            pal.muted
        },
        text_w,
    );
    let sub = match (&reason, look) {
        (Some(why), _) => why.clone(),
        (None, Some(l)) if !l.says.is_empty() => format!("{}: {}", l.title, l.says),
        (None, Some(l)) => l.title.clone(),
        (None, None) => notes_task::picture_line(f, &f.measure),
    };
    p.text(
        Face::Ui,
        11.0,
        name_x,
        row.y + 34.0,
        &sub,
        if reason.is_some() {
            pal.faint
        } else if look.is_some() {
            pal.text
        } else {
            pal.muted
        },
        text_w,
    );
    let out = run.is_some_and(|r| {
        r.in_flight
            .iter()
            .any(|(_, b)| r.batches.get(*b).is_some_and(|ids| ids.contains(&i)))
    });
    let right = if reason.is_some() {
        String::new()
    } else if !s.ai_on {
        "nothing is sent".to_string()
    } else if out {
        "looking\u{2026}".to_string()
    } else if run.is_some_and(|r| r.failed_at(i)) {
        "not looked at: the request failed".to_string()
    } else if look.is_some() {
        "looked at".to_string()
    } else {
        notes_task::sent_as(&f.measure)
    };
    if !right.is_empty() {
        let right = p.elide(Face::Ui, 11.5, &right, right_w);
        p.text_right(
            Face::Ui,
            11.5,
            row.right() - 12.0,
            row.cy(),
            &right,
            if matches!(f.measure, Measure::Waiting) || out {
                pal.faint
            } else {
                pal.muted
            },
        );
    }
}

/// A thumbnail in its square, kept to its shape on the field's colour; an empty square while
/// there is none (measuring, or a GIF the engine makes none of). Drawn through a bitmap of its
/// own so the list's clip holds for it.
pub(crate) fn draw_thumb(
    p: &mut Painter,
    pal: &Palette,
    r: Rect,
    thumb: Option<&(Vec<u8>, u32, u32)>,
) {
    p.frame(r, 4.0, pal.field, pal.line);
    let Some((rgba, w, h)) = thumb.filter(|t| t.1 > 0 && t.2 > 0) else {
        return;
    };
    let (x0, y0, x1, y1) = p.dev_rect(r.inset(2.0));
    let (dw, dh) = (x1 - x0, y1 - y0);
    if dw <= 0 || dh <= 0 {
        return;
    }
    let fit = (dw as f32 / *w as f32).min(dh as f32 / *h as f32);
    let (iw, ih) = (*w as f32 * fit, *h as f32 * fit);
    let mut bmp = Bitmap::new(dw as u32, dh as u32);
    bmp.clear(pal.field);
    bmp.image(
        rgba,
        *w,
        *h,
        (dw as f32 - iw) / 2.0,
        (dh as f32 - ih) / 2.0,
        iw,
        ih,
    );
    p.blit(&bmp, 0, dh, x0, y0);
}

/// Write notes' Options (pictures plan 12): How many notes, then the seven ticks, each with the
/// line under it. Under the Options button, inside the card.
fn draw_options(
    p: &mut Painter,
    pal: &Palette,
    s: &TaskScreen,
    hits: &mut Vec<(Hit, Rect)>,
    hover: Option<Hit>,
) {
    use crate::notes_task::{HOW_MANY, Tick as Opt, how_many_label};
    let g = task_ui::geo(task_ui::card(s.win.0, s.win.1));
    let width = 440.0;
    // Its lines are 40 px, closer (down to 34) in a window too low for them: it is always
    // drawn whole (finding 15).
    let fixed = 16.0 + 18.0 + SEGMENT_H + 14.0 + 8.0;
    let lines = Opt::ALL.len() as f32;
    let row_h = ((s.win.1 - 16.0 - fixed) / lines).clamp(34.0, 40.0);
    let h = fixed + row_h * lines;
    // Under the button while the card has room for it; else as high as the card lets it go,
    // not above the header; and in a low window inside the window, whatever the card.
    let card = task_ui::card(s.win.0, s.win.1);
    let r = Rect::new(
        g.dest.right() - width,
        (g.dest.bottom() + 2.0)
            .min(g.footer.y - h - 4.0)
            .max(card.y + task_ui::HEADER_H)
            .min(s.win.1 - 8.0 - h)
            .max(8.0),
        width,
        h,
    );
    p.frame(r, 8.0, pal.popup, pal.popup_line);
    hits.push((Hit::Popover, r));
    let x = r.x + 16.0;
    p.text(
        Face::Semibold,
        11.0,
        x,
        r.y + 22.0,
        "HOW MANY NOTES",
        pal.muted,
        r.w - 32.0,
    );
    let labels: Vec<&str> = HOW_MANY.iter().map(|h| how_many_label(*h)).collect();
    let sw = Rect::new(
        x,
        r.y + 34.0,
        switch_width(p, &labels).min(r.w - 32.0),
        SEGMENT_H,
    );
    let selected = HOW_MANY.iter().position(|h| *h == s.notes.how_many);
    let hovered = HOW_MANY
        .iter()
        .position(|h| hover == Some(Hit::HowMany(*h)));
    let segs = draw_segments(p, pal, sw, &labels, selected, hovered);
    for (h, seg) in HOW_MANY.iter().zip(segs) {
        hits.push((Hit::HowMany(*h), seg));
    }
    let mut y = sw.bottom() + 14.0;
    for t in Opt::ALL {
        let row = Rect::new(r.x + 4.0, y, r.w - 8.0, row_h);
        let enabled = t.enabled(&s.notes);
        if enabled && hover == Some(Hit::Tick(t)) {
            p.rbox(row, 5.0, pal.hover);
        }
        draw_tick(
            p,
            pal,
            Rect::new(x, row.y + 6.0, 16.0, 16.0),
            Tick {
                on: t.on(&s.notes),
                enabled,
                focused: false,
            },
        );
        p.text(
            Face::Ui,
            13.0,
            x + 28.0,
            row.y + 14.0,
            t.label(),
            if enabled { pal.text } else { pal.faint },
            r.w - 60.0,
        );
        p.text(
            Face::Ui,
            11.0,
            x + 28.0,
            row.y + 30.0,
            t.line(),
            pal.faint,
            r.w - 60.0,
        );
        hits.push((Hit::Tick(t), row));
        y += row_h;
    }
}

/// The Add menu under its button (D-207): the open tabs' files, the one in front first, then
/// "Files…". Its rows are hit before anything under them, being pushed last.
/// The width of the Add menu, under the right of Add….
const ADD_MENU_W: f32 = 320.0;

fn draw_add_menu(
    p: &mut Painter,
    pal: &Palette,
    s: &TaskScreen,
    button: Rect,
    hits: &mut Vec<(Hit, Rect)>,
    hover: Option<Hit>,
) {
    let row_h = task_ui::ADD_ROW_H;
    let h = row_h * s.add_items.len() as f32 + 8.0;
    let width = ADD_MENU_W;
    let r = Rect::new(button.right() - width, button.bottom() + 4.0, width, h);
    p.frame(r, 7.0, pal.popup, pal.popup_line);
    for (k, item) in s.add_items.iter().enumerate() {
        let row = Rect::new(r.x + 4.0, r.y + 4.0 + k as f32 * row_h, r.w - 8.0, row_h);
        if hover == Some(Hit::AddItem(k)) {
            p.rbox(row, 5.0, pal.hover);
        }
        p.text(
            Face::Ui,
            13.0,
            row.x + 10.0,
            row.cy(),
            &item.label(),
            pal.text,
            row.w - 20.0,
        );
        hits.push((Hit::AddItem(k), row));
    }
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

#[cfg(test)]
mod flow_tests {
    use super::*;
    use crate::notes_task::Run;
    use mdshared_text::Bitmap;
    use mdshared_ui::gfx::Gfx;

    fn screen(task: TaskKind) -> TaskScreen {
        TaskScreen::new(
            task,
            Vec::new(),
            DestChoice::Decide,
            Vec::new(),
            "Claude".into(),
        )
    }

    fn draw_at(s: &mut TaskScreen, w: u32, h: u32) {
        let mut bm = Bitmap::new(w, h);
        let mut gfx = Gfx::new();
        let mut p = Painter::new(&mut bm, &mut gfx, 1.0);
        draw(
            &mut p,
            &mdshared_ui::theme::LIGHT,
            s,
            w as f32,
            h as f32,
            false,
        );
    }

    fn at(s: &TaskScreen, hit: Hit) -> Option<Rect> {
        s.hits.iter().find(|(k, _)| *k == hit).map(|(_, r)| *r)
    }

    fn meets(a: Rect, b: Rect) -> bool {
        a.x < b.right() && b.x < a.right() && a.y < b.bottom() && b.y < a.bottom()
    }

    /// Finding 20: "Show what is sent" never overlaps a button, at any width the window allows,
    /// with the most buttons the footer has, and stays in the card.
    #[test]
    fn show_what_is_sent_never_meets_a_button() {
        let mut failed = screen(TaskKind::Notes);
        failed.ai_on = true;
        failed.notes_run = Some(Run {
            batches: vec![vec![0], vec![1]],
            next: 2,
            answered: vec![0],
            failed: vec![(1, "Lost.".into())],
            stopped: true,
            texts: vec![5],
            ..Run::default()
        });
        let off = screen(TaskKind::Rewrite);
        for mut s in [failed, off] {
            for w in (520..=1600).step_by(20) {
                draw_at(&mut s, w, 700);
                let link = at(&s, Hit::ShowSent).expect("drawn");
                let card = task_ui::card(w as f32, 700.0);
                assert!(link.x >= card.x && link.right() <= card.right(), "{w}");
                for hit in [
                    Hit::Primary,
                    Hit::TryAgain,
                    Hit::GoOn,
                    Hit::OpenSettings,
                    Hit::Cancel,
                    Hit::AddFiles,
                ] {
                    if let Some(b) = at(&s, hit) {
                        assert!(!meets(link, b), "{hit:?} at {w}");
                    }
                }
            }
        }
    }

    /// Finding 15: Write notes' Options is drawn whole inside the card, at every height the
    /// window allows, and last, so the hit on it comes before anything under it.
    #[test]
    fn the_options_popover_is_inside_the_card_and_on_top() {
        for h in (360..=1000).step_by(40) {
            let mut s = screen(TaskKind::Notes);
            s.options_open = true;
            s.notice = Some("A line over the foot of the list.".into());
            draw_at(&mut s, 1280, h);
            let pop = at(&s, Hit::Popover).expect("drawn");
            let card = task_ui::card(1280.0, h as f32);
            // Whole in the window always; in the card where the card is tall enough.
            assert!(pop.y >= 0.0 && pop.bottom() <= h as f32, "{h}");
            if card.h >= pop.h + task_ui::HEADER_H {
                assert!(pop.y >= card.y && pop.bottom() <= card.bottom(), "{h}");
            }
            assert!(
                matches!(s.hits.last(), Some((Hit::Tick(_), _))),
                "its lines are the last hit"
            );
        }
    }

    /// Finding 11: the card grows with a large window, within limits, and is as before at the
    /// default size.
    #[test]
    fn the_card_grows_with_the_window() {
        let small = task_ui::card(1280.0, 800.0);
        assert_eq!((small.w, small.h), (task_ui::CARD_W, task_ui::CARD_H));
        let big = task_ui::card(1920.0, 1080.0);
        assert!(big.w > small.w + 300.0 && big.h > small.h + 200.0);
        let huge = task_ui::card(4000.0, 3000.0);
        assert_eq!((huge.w, huge.h), (task_ui::CARD_W_MAX, task_ui::CARD_H_MAX));
    }

    /// Findings 13 and 16: an empty window's footer does not say the summary twice, and the
    /// button says the same with the AI on or off and is off with no file.
    #[test]
    fn an_empty_window_says_what_to_do_and_its_button_is_off() {
        let mut s = screen(TaskKind::Notes);
        s.ai_on = true;
        assert_eq!(s.summary(), "No file takes part.");
        assert_ne!(s.notes_blocked().as_deref(), Some("No file takes part."));
        for task in [TaskKind::File, TaskKind::Split] {
            let mut s = screen(task);
            s.ai_on = true;
            let on = s.primary_label();
            s.ai_on = false;
            assert_eq!(s.primary_label(), on);
            assert_eq!(on, "Make the plan", "no count of nothing");
            assert!(s.primary_blocked().is_some());
        }
    }

    /// Fable's second review, 1: the Add menu is drawn over the line under the title, whatever
    /// that line says: inside the menu the pixels are the same with a short line or a long one.
    #[test]
    fn the_add_menu_is_drawn_over_the_line_under_the_title() {
        let pixels = |from: Option<&str>| {
            let mut s = screen(TaskKind::File);
            s.from = from.map(str::to_string);
            s.add_items = vec![
                task_ui::AddItem::Tab {
                    path: "a.md".into(),
                    name: "a.md".into(),
                    front: true,
                },
                task_ui::AddItem::Files,
            ];
            s.add_open = true;
            let (w, h) = (800u32, 700u32);
            let mut bm = Bitmap::new(w, h);
            let mut gfx = Gfx::new();
            {
                let mut p = Painter::new(&mut bm, &mut gfx, 1.0);
                draw(
                    &mut p,
                    &mdshared_ui::theme::LIGHT,
                    &mut s,
                    w as f32,
                    h as f32,
                    false,
                );
            }
            let rows: Vec<Rect> = s
                .hits
                .iter()
                .filter(|(k, _)| matches!(k, Hit::AddItem(_)))
                .map(|(_, r)| *r)
                .collect();
            assert_eq!(rows.len(), 2, "the menu is drawn");
            let top = rows[0].y.floor() as usize;
            let bottom = rows[1].bottom().ceil() as usize;
            let (left, right) = (rows[0].x.floor() as usize, rows[0].right().ceil() as usize);
            let mut inside = Vec::new();
            for y in top..bottom {
                for x in left..right {
                    let at = (y * w as usize + x) * 4;
                    inside.extend_from_slice(&bm.pixels[at..at + 4]);
                }
            }
            inside
        };
        let long = "The 12 notes of Recipes \u{203A} Pasta \u{203A} Sauces and their many friends, \
                    from the Notes pane.";
        assert!(
            pixels(None) == pixels(Some(long)),
            "no letter of the line under the title is drawn over the menu"
        );
    }
}
