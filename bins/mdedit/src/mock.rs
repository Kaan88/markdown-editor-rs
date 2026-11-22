#![allow(dead_code)]
//! Throwaway mockups of a launcher and updater for the owner's apps (`mock/launcher`), drawn by
//! the app's own painter, fonts, palette and widgets for `--shot` with `--mock <name>`, and never
//! reachable otherwise. Like `spikes/`, this exists to make pictures and to measure the layout;
//! nothing here is the real feature. The apps and their versions are the ones on the owner's
//! GitHub releases on 2026-11-22; what is installed on the "PC" and every state is made up.
//!
//! Names: `hub-main`, `hub-menu`, `hub-updating`, `hub-running`, `hub-notes`, `hub-failed`,
//! `hub-rollback`, `hub-first-run`, `hub-tray`, `hub-settings-updates`, `hub-settings-general`,
//! `hub-self-update`, `hub-linux`, and `hub-report`, which prints the layout numbers to stderr.

mod data;
mod dialogs;
mod engine;
mod pages;
mod panes;
mod report;
mod ui;
mod win;

use data::St;
use mdshared_ui::gfx::Painter;
use mdshared_ui::theme::Palette;
use win::{Banner, Bar, Pane, Scene};

fn show(p: &mut Painter, pal: &Palette, w: f32, h: f32, s: Scene) {
    win::window(p, pal, w, h, &s);
}

pub fn draw(p: &mut Painter, pal: &Palette, name: &str, w: f32, h: f32) {
    let d = Scene::default;
    match name {
        "hub-report" => report::print(p, pal, w, h),
        "hub-main" => show(
            p,
            pal,
            w,
            h,
            Scene {
                hover: Some(data::SPRAWL),
                ..d()
            },
        ),
        "hub-menu" => dialogs::row_menu(p, pal, w, h),
        "hub-updating" => {
            let mut s = d();
            s.st[data::MDEDIT] = St::Downloading(0.64, "Downloading \u{b7} 6.2 MB/s");
            s.st[data::SPRAWL] = St::Verifying;
            s.st[data::INBOUND] = St::Waiting("Open: close it, or Hub asks you");
            s.bar = Bar::Busy("2 of 3 updates running, 1 waiting");
            s.pane = Pane::Steps;
            show(p, pal, w, h, s);
        }
        "hub-running" => dialogs::running(p, pal, w, h),
        "hub-notes" => dialogs::notes(p, pal, w, h),
        "hub-failed" => {
            let mut s = d();
            s.st[data::MDEDIT] = St::Done("1.2.1, reopened");
            s.inst[data::MDEDIT] = Some("1.2.1");
            s.st[data::SPRAWL] = St::Failed("SHA-256 does not match");
            s.sel = data::SPRAWL;
            s.pane = Pane::Failure;
            s.bar = Bar::Updates(2);
            s.banner = Some(Banner {
                text: "Sprawl 1.0.5 was not installed: the file does not match its SHA-256. 1.0.3 is unchanged.",
                yes: "Try again",
                no: "Details",
                warn: true,
            });
            show(p, pal, w, h, s);
        }
        "hub-rollback" => dialogs::rollback(p, pal, w, h),
        "hub-first-run" => dialogs::first_run(p, pal, w, h),
        "hub-tray" => dialogs::tray(p, pal, w, h),
        "hub-settings-updates" => dialogs::settings(p, pal, w, h, 1),
        "hub-settings-general" => dialogs::settings(p, pal, w, h, 0),
        "hub-self-update" => {
            let mut s = d();
            s.hub_row = true;
            s.inst[data::HUB] = Some("0.3.0");
            s.st[data::HUB] = St::Restart;
            s.sel = data::HUB;
            s.pane = Pane::HubSelf;
            s.banner = Some(Banner {
                text: "Hub 0.4.0 is ready. Restart Hub to finish updating; your apps are not affected.",
                yes: "Restart Hub",
                no: "Later",
                warn: false,
            });
            show(p, pal, w, h, s);
        }
        "hub-linux" => {
            let mut s = d();
            s.linux = true;
            s.st = [
                St::Pkg,
                St::Update,
                St::NoBuild,
                St::NotInstalled,
                St::NotInstalled,
                St::Update,
                St::NotInstalled,
                St::UpToDate,
            ];
            s.inst = [
                Some("1.2.0"),
                Some("1.0.3"),
                None,
                None,
                None,
                Some("0.8.2"),
                None,
                Some("0.4.0"),
            ];
            s.running = [false; 8];
            s.sel = data::SPRAWL;
            s.pane = Pane::NoBuild;
            s.bar = Bar::Updates(2);
            show(p, pal, w, h, s);
        }
        other => eprintln!("mdedit: no mock named {other}"),
    }
}
