#![allow(dead_code)]
//! Throwaway mockups of a file manager (`mock/file-manager`), drawn by the app's own painter,
//! fonts, palette and widgets for `--shot` with `--mock <name>`, and never reachable otherwise.
//! Nothing here is the real feature; the folders, files, sizes and dates are made up.
//!
//! Names: `fm-main`, `fm-split`, `fm-list`, `fm-icons`, `fm-preview`, `fm-menu-tools`,
//! `fm-menu-view`, `fm-context`, `fm-context-shell`, `fm-context-bg`, `fm-queue`, `fm-conflict`,
//! `fm-rename`, `fm-compare`, `fm-find`, `fm-delete`, `fm-settings`; the commander-style
//! `cm-main`, `cm-plugins`, `cm-vfs`, `cm-lister`.

mod cmd;
mod fmdata;
mod fmover;
mod fmwin;
mod ui;

use fmwin::{Content, PaneSt, Prev, Scene, Strip, View};
use mdshared_ui::gfx::Painter;
use mdshared_ui::theme::Palette;

pub fn draw(p: &mut Painter, pal: &Palette, name: &str, w: f32, h: f32) {
    let show = |p: &mut Painter, s: Scene| {
        fmwin::window(p, pal, w, h, &s);
    };
    match name {
        "fm-main" => {
            let mut s = Scene::default();
            s.panes[0].hover = Some(5);
            show(p, s)
        }
        "fm-split" => {
            let mut s = Scene::default();
            s.split = true;
            s.active = 0;
            s.panes[1].sel = vec![0];
            show(p, s)
        }
        "fm-list" => {
            let mut s = Scene::default();
            s.panes[0].view = View::List;
            show(p, s)
        }
        "fm-icons" => {
            let mut s = Scene::default();
            s.panes[0] = PaneSt::new(Content::Pictures);
            s.tab = 2;
            s.nav_sel = 4;
            s.preview = Some(Prev::Img);
            show(p, s)
        }
        "fm-preview" => {
            let mut s = Scene::default();
            s.panes[0].sel = vec![14];
            s.preview = Some(Prev::Md);
            show(p, s)
        }
        "fm-downloads" => {
            let mut s = Scene::default();
            s.panes[0] = PaneSt::new(Content::Downloads);
            s.tab = 1;
            s.nav_sel = 3;
            show(p, s)
        }
        "fm-menu-tools" => fmover::menu_tools(p, pal, w, h),
        "fm-menu-view" => fmover::menu_view(p, pal, w, h),
        "fm-context" => fmover::context(p, pal, w, h, false, false),
        "fm-context-shell" => fmover::context(p, pal, w, h, true, false),
        "fm-context-split" => fmover::context(p, pal, w, h, false, true),
        "fm-context-bg" => fmover::context_bg(p, pal, w, h),
        "fm-queue" => fmover::queue(p, pal, w, h, false),
        "fm-conflict" => fmover::queue(p, pal, w, h, true),
        "fm-rename" => fmover::rename(p, pal, w, h),
        "fm-delete" => fmover::delete(p, pal, w, h),
        "fm-settings" => fmover::settings(p, pal, w, h),
        "fm-compare" => {
            let mut s = Scene::default();
            s.split = true;
            s.panes = [PaneSt::new(Content::Compare), PaneSt::new(Content::Compare)];
            s.strip = Strip::Compare;
            show(p, s)
        }
        "fm-find" => fmover::find(p, pal, w, h),
        n if n.starts_with("cm-") => cmd::draw(p, pal, n, w, h),
        other => eprintln!("mdedit: no mock named {other}"),
    }
}
