//! The pages of Hub's Settings, as the shared `Row`s the form is made of, and the one thing the
//! form has no row for: the list of what Hub contacts.

use super::engine::{Item, Opts, Page, R};
use mdshared_ui::gfx::{Face, Painter, Rect, mix};
use mdshared_ui::glyph;
use mdshared_ui::settings::Row;
use mdshared_ui::theme::Palette;

pub const PAUSE: &str = "M5.5 3v10M10.5 3v10";

fn choice(id: u32, label: &str, help: &str, options: &[&str], value: usize) -> Row {
    Row::Choice {
        id,
        label: label.into(),
        help: help.into(),
        options: options.iter().map(|s| s.to_string()).collect(),
        value,
    }
}

fn toggle(id: u32, label: &str, help: &str, value: bool) -> Row {
    Row::Toggle {
        id,
        label: label.into(),
        help: help.into(),
        value,
    }
}

fn number(id: u32, label: &str, help: &str, value: i32, min: i32, max: i32, unit: &str) -> Row {
    Row::Number {
        id,
        label: label.into(),
        help: help.into(),
        value,
        min,
        max,
        step: 1,
        unit: unit.into(),
    }
}

fn text(id: u32, label: &str, help: &str, value: &str) -> Row {
    Row::Text {
        id,
        label: label.into(),
        help: help.into(),
        value: value.into(),
        placeholder: String::new(),
        secret: false,
    }
}

fn page(name: &str, items: Vec<Item>) -> Page {
    Page {
        name: name.into(),
        items,
        dynamic: false,
    }
}

pub fn pages() -> Vec<Page> {
    let general = page(
        "General",
        vec![
            Item::Row(
                R::new(text(
                    1,
                    "Install apps into",
                    "Each app gets its own folder here. No administrator rights are needed. Apps that are already installed elsewhere stay where they are.",
                    "%LOCALAPPDATA%\\Programs",
                ))
                .stack()
                .btn("Browse", true),
            ),
            Item::Gap,
            Item::Row(R::new(toggle(
                2,
                "Start Hub with Windows",
                "Hub stays in the tray and checks quietly. Closing its window does not quit it.",
                true,
            ))),
            Item::Row(R::new(toggle(3, "Show an icon in the tray", "", true))),
            Item::Row(R::new(toggle(
                4,
                "Tell me when updates are found",
                "One notification per check, never one per app.",
                true,
            ))),
            Item::Row(R::new(choice(5, "Theme", "", &["Light", "Dark", "System"], 2))),
        ],
    );
    let updates = page(
        "Updates",
        vec![
            Item::Row(R::new(choice(
                10,
                "Look for updates",
                "One request per app, to the address its release publishes.",
                &[
                    "Every hour",
                    "Every 6 hours",
                    "Every day",
                    "Every week",
                    "Never",
                ],
                1,
            ))),
            Item::Row(R::new(toggle(
                11,
                "Include pre-releases (beta channel)",
                "GitHub marks them as pre-releases. None of the desktop apps has one now.",
                false,
            ))),
            Item::Row(R::new(toggle(
                12,
                "Install updates for apps that are closed",
                "Off: Hub tells you and waits. On: a closed app updates by itself; an open one always waits for you.",
                false,
            ))),
            Item::Row(
                R::new(number(
                    13,
                    "Previous versions kept for Roll back",
                    "",
                    1,
                    0,
                    3,
                    "",
                ))
                .child(),
            ),
            Item::Row(R::new(toggle(
                14,
                "Update Hub itself",
                "Hub checks itself first and restarts last, when nothing is installing.",
                true,
            ))),
            Item::Gap,
            Item::Custom(120.0, 1),
        ],
    );
    let rest = |n: &str| page(n, vec![]);
    vec![
        general,
        updates,
        rest("Apps"),
        rest("Network"),
        rest("About"),
    ]
}

pub fn opts(current: usize) -> Opts {
    Opts {
        current,
        nav_focus: false,
        center: false,
    }
}

pub fn custom(p: &mut Painter, pal: &Palette, l: f32, r: f32, y: f32, _id: u8) {
    let rows: [(&str, &str, &str); 3] = [
        (
            "github.com/Kaan88",
            "Release lists, update manifests and release notes of the apps",
            "every 6 hours",
        ),
        (
            "githubusercontent.com",
            "The files themselves, where GitHub sends a download",
            "when you press Update or Install",
        ),
        (
            "Nothing else",
            "No account, no usage data, no crash upload. Each app keeps its own settings.",
            "",
        ),
    ];
    let box_ = Rect::new(l, y + 6.0, r - l, 108.0);
    p.frame(box_, 8.0, mix(pal.popup, pal.text, 0.02), pal.popup_line);
    for (i, (n, sub, when)) in rows.iter().enumerate() {
        let ry = box_.y + i as f32 * 36.0;
        if i > 0 {
            p.fill_a(
                Rect::new(box_.x + 10.0, ry, box_.w - 20.0, 1.0),
                pal.popup_line,
                0.8,
            );
        }
        p.icon(
            if i == 2 { glyph::CHECK } else { glyph::GLOBE },
            13.0,
            box_.x + 20.0,
            ry + 18.0,
            pal.muted,
        );
        p.text(
            Face::Semibold,
            12.5,
            box_.x + 40.0,
            ry + 11.0,
            n,
            pal.text,
            200.0,
        );
        p.text(
            Face::Ui,
            11.5,
            box_.x + 40.0,
            ry + 27.0,
            sub,
            pal.muted,
            box_.w - 190.0,
        );
        if !when.is_empty() {
            p.text_right(
                Face::Ui,
                11.5,
                box_.right() - 14.0,
                ry + 11.0,
                when,
                pal.faint,
            );
        }
    }
}
