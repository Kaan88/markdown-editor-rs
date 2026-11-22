#![allow(dead_code)]
//! Throwaway mockups of a task manager (`mock/task-manager`), drawn by the app's own painter,
//! fonts, palette and widgets for `--shot` with `--mock <name>`, and never reachable otherwise.
//! Nothing here is the real feature; every process, figure and graph is made up.
//!
//! Names: `tm-procs`, `tm-tree`, `tm-flat`, `tm-search`, `tm-details`, `tm-details-handles`,
//! `tm-details-modules`, `tm-details-net`, `tm-details-denied`, `tm-context`, `tm-endtree`,
//! `tm-affinity`, `tm-elevate`, `tm-perf-cpu`, `tm-perf-cores`, `tm-perf-mem`, `tm-perf-disk`,
//! `tm-perf-net`, `tm-perf-gpu`, `tm-startup`, `tm-services`, `tm-files`, `tm-files-deep`,
//! `tm-settings`, `tm-privacy`, `tm-cost`, `tm-tray`, `tm-compact`.

mod tmdata;
mod tmover;
mod tmpages;
mod tmwin;
mod ui;

use mdshared_ui::gfx::Painter;
use mdshared_ui::theme::Palette;
use tmpages::Res;
use tmwin::{Cols, DTab, Page, Scene, View};

fn page(
    p: &mut Painter,
    pal: &Palette,
    w: f32,
    h: f32,
    pg: Page,
    body: impl FnOnce(&mut Painter, &Palette, mdshared_ui::gfx::Rect),
) {
    let sc = Scene {
        page: pg,
        ..Scene::default()
    };
    tmwin::window(p, pal, w, h, &sc);
    body(p, pal, tmpages::body_rect(w, h));
}

fn procs(p: &mut Painter, pal: &Palette, w: f32, h: f32, sc: Scene) {
    tmwin::window(p, pal, w, h, &sc);
}

pub fn draw(p: &mut Painter, pal: &Palette, name: &str, w: f32, h: f32) {
    let small = w < 1000.0;
    let base = || Scene {
        cols: if small { Cols::Small } else { Cols::Default },
        ..Scene::default()
    };
    match name {
        "tm-procs" => procs(
            p,
            pal,
            w,
            h,
            Scene {
                hover: Some(8004),
                ..base()
            },
        ),
        "tm-tree" => procs(
            p,
            pal,
            w,
            h,
            Scene {
                view: View::Tree,
                cols: if small { Cols::Small } else { Cols::Tree },
                collapsed: vec![10312],
                sel: 12712,
                ..Scene::default()
            },
        ),
        "tm-flat" => procs(
            p,
            pal,
            w,
            h,
            Scene {
                view: View::Flat,
                cols: if small { Cols::Small } else { Cols::Wide },
                sort: "mem",
                sel: 12712,
                ..Scene::default()
            },
        ),
        "tm-search" => procs(
            p,
            pal,
            w,
            h,
            Scene {
                view: View::Flat,
                search: "rust",
                cols: if small { Cols::Small } else { Cols::Tree },
                ..Scene::default()
            },
        ),
        "tm-details" => procs(
            p,
            pal,
            w,
            h,
            Scene {
                details: Some((DTab::Overview, false)),
                ..base()
            },
        ),
        "tm-details-modules" => procs(
            p,
            pal,
            w,
            h,
            Scene {
                details: Some((DTab::Modules, false)),
                ..base()
            },
        ),
        "tm-details-handles" => procs(
            p,
            pal,
            w,
            h,
            Scene {
                details: Some((DTab::Handles, false)),
                ..base()
            },
        ),
        "tm-details-net" => procs(
            p,
            pal,
            w,
            h,
            Scene {
                details: Some((DTab::Network, false)),
                sel: 10788,
                ..base()
            },
        ),
        "tm-details-env" => procs(
            p,
            pal,
            w,
            h,
            Scene {
                details: Some((DTab::Environment, false)),
                ..base()
            },
        ),
        "tm-details-denied" => procs(
            p,
            pal,
            w,
            h,
            Scene {
                details: Some((DTab::Modules, true)),
                sel: 800,
                view: View::Tree,
                cols: if small { Cols::Small } else { Cols::Tree },
                ..Scene::default()
            },
        ),
        "tm-context" => tmover::context(p, pal, w, h),
        "tm-endtree" => tmover::endtree(p, pal, w, h),
        "tm-affinity" => tmover::affinity(p, pal, w, h),
        "tm-elevate" => tmover::elevate(p, pal, w, h),
        "tm-settings" => tmover::settings(p, pal, w, h),
        "tm-privacy" => tmover::privacy(p, pal, w, h),
        "tm-cost" => tmover::cost(p, pal, w, h),
        "tm-tray" => tmover::tray(p, pal, w, h),
        "tm-compact" => tmover::compact(p, pal, w, h),
        "tm-perf-cpu" => page(p, pal, w, h, Page::Performance, |p, pal, b| {
            tmpages::perf_page(p, pal, b, Res::Cpu)
        }),
        "tm-perf-cores" => page(p, pal, w, h, Page::Performance, |p, pal, b| {
            tmpages::perf_page(p, pal, b, Res::Cores)
        }),
        "tm-perf-mem" => page(p, pal, w, h, Page::Performance, |p, pal, b| {
            tmpages::perf_page(p, pal, b, Res::Mem)
        }),
        "tm-perf-disk" => page(p, pal, w, h, Page::Performance, |p, pal, b| {
            tmpages::perf_page(p, pal, b, Res::Disk)
        }),
        "tm-perf-net" => page(p, pal, w, h, Page::Performance, |p, pal, b| {
            tmpages::perf_page(p, pal, b, Res::Net)
        }),
        "tm-perf-gpu" => page(p, pal, w, h, Page::Performance, |p, pal, b| {
            tmpages::perf_page(p, pal, b, Res::Gpu)
        }),
        "tm-startup" => page(p, pal, w, h, Page::Startup, |p, pal, b| {
            tmpages::startup_page(p, pal, b, 0)
        }),
        "tm-startup-admin" => page(p, pal, w, h, Page::Startup, |p, pal, b| {
            tmpages::startup_page(p, pal, b, 4)
        }),
        "tm-services" => page(p, pal, w, h, Page::Services, |p, pal, b| {
            tmpages::services_page(p, pal, b, 9)
        }),
        "tm-files" => page(p, pal, w, h, Page::Files, |p, pal, b| {
            tmpages::files_page(p, pal, b, false)
        }),
        "tm-files-deep" => page(p, pal, w, h, Page::Files, |p, pal, b| {
            tmpages::files_page(p, pal, b, true)
        }),
        other => eprintln!("mdedit: no mock named {other}"),
    }
}
