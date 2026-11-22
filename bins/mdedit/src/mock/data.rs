//! The apps of the mockups: the seven of the owner's GitHub repositories whose releases carry
//! installers for a desktop, read from their latest releases on 2026-11-22. Versions, dates, file
//! names and sizes are the real ones; what is installed on "this PC", and every state, is made up.

use mdshared_ui::gfx::Rgb;
use mdshared_ui::glyph;
use mdshared_ui::theme::Palette;

pub struct App {
    pub name: &'static str,
    pub blurb: &'static str,
    pub glyph: &'static str,
    pub tile: Rgb,
    pub repo: &'static str,
    pub latest: &'static str,
    pub date: &'static str,
    /// The Windows installer's size (the file a first install downloads), as the release lists it.
    pub size: &'static str,
    /// The installer's size, for a first install.
    pub setup: &'static str,
    /// The update manifest the release carries, or how the files are checked when there is none.
    pub manifest: &'static str,
    /// The Linux file Hub would use, or none.
    pub linux: Option<&'static str>,
    pub linux_size: &'static str,
}

pub const MDEDIT: usize = 0;
pub const MDVIEW: usize = 1;
pub const SPRAWL: usize = 2;
pub const NEWSQUEUE: usize = 3;
pub const POSTERN: usize = 4;
pub const INBOUND: usize = 5;
pub const CAUSTICA: usize = 6;
pub const HUB: usize = 7;

pub const APPS: [App; 8] = [
    App {
        name: "mdedit",
        blurb: "Light text editor with syntax highlighting",
        glyph: glyph::EDIT,
        tile: [0x2f, 0x6f, 0xd6],
        repo: "markdown-editor-rs",
        latest: "1.2.1",
        date: "21 Nov",
        size: "35.2 MB",
        setup: "12.0 MB",
        manifest: "mdedit-update-2.txt",
        linux: Some("mdedit-1.2.1-linux-x86_64.AppImage"),
        linux_size: "16.7 MB",
    },
    App {
        name: "mdview",
        blurb: "Markdown viewer",
        glyph: glyph::VIEW,
        tile: [0x1d, 0x7d, 0x6b],
        repo: "markdown-viewer-rs",
        latest: "1.0.4",
        date: "21 Nov",
        size: "34.1 MB",
        setup: "11.4 MB",
        manifest: "mdview-update.txt",
        linux: Some("mdview-1.0.4-linux-x86_64.AppImage"),
        linux_size: "14.9 MB",
    },
    App {
        name: "Sprawl",
        blurb: "Disk space analyzer and cleaner",
        glyph: glyph::FOLDER,
        tile: [0xc0, 0x6a, 0x1a],
        repo: "sprawl",
        latest: "1.0.5",
        date: "20 Nov",
        size: "11.7 MB",
        setup: "5.3 MB",
        manifest: "sprawl-update.json (signed)",
        linux: None,
        linux_size: "",
    },
    App {
        name: "NewsQueue",
        blurb: "Usenet downloader",
        glyph: glyph::TASK,
        tile: [0x7a, 0x4f, 0xc4],
        repo: "newsqueue",
        latest: "1.0.10",
        date: "20 Nov",
        size: "5.3 MB",
        setup: "5.3 MB",
        manifest: "SHA256SUMS.txt (no manifest)",
        linux: Some("NewsQueue-1.0.10-linux-amd64.tar.gz"),
        linux_size: "36.8 MB",
    },
    App {
        name: "Postern",
        blurb: "Mail client",
        glyph: glyph::LINK,
        tile: [0xb8, 0x3a, 0x5a],
        repo: "postern",
        latest: "0.3.5",
        date: "18 Nov",
        size: "2.8 MB",
        setup: "2.8 MB",
        manifest: "latest.json (signed)",
        linux: Some("Postern-0.3.5-x64.AppImage"),
        linux_size: "90.4 MB",
    },
    App {
        name: "Inbound",
        blurb: "Download manager: HTTP, torrents, remote files",
        glyph: glyph::DOWNLOAD,
        tile: [0x1d, 0x8a, 0x9e],
        repo: "inbound",
        latest: "0.9.0",
        date: "17 Nov",
        size: "68.7 MB",
        setup: "68.7 MB",
        manifest: "manifest.json + public.pem",
        linux: Some("Inbound-0.9.0-x86_64.AppImage"),
        linux_size: "72.4 MB",
    },
    App {
        name: "Caustica",
        blurb: "Path tracer",
        glyph: glyph::IMAGE,
        tile: [0x55, 0x5c, 0x6e],
        repo: "caustica",
        latest: "0.13.0",
        date: "12 Nov",
        size: "734.5 MB",
        setup: "734.5 MB",
        manifest: "SHA256SUMS (no manifest)",
        linux: Some("Caustica-0.13.0-x86_64.AppImage"),
        linux_size: "725.5 MB",
    },
    App {
        name: "Hub",
        blurb: "This launcher",
        glyph: glyph::SYSTEM,
        tile: [0x4a, 0x55, 0xb8],
        repo: "hub",
        latest: "0.4.0",
        date: "22 Nov",
        size: "3.1 MB",
        setup: "3.1 MB",
        manifest: "hub-update.txt",
        linux: Some("hub-0.4.0-linux-x86_64.AppImage"),
        linux_size: "3.4 MB",
    },
];

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum St {
    UpToDate,
    Update,
    NotInstalled,
    /// Installed, but not by Hub.
    External,
    /// 0..1, and the line under the bar.
    Downloading(f32, &'static str),
    Verifying,
    Waiting(&'static str),
    Failed(&'static str),
    /// Files swapped, the running copy is the old one.
    Restart,
    Done(&'static str),
    NoBuild,
    /// Installed by a package manager.
    Pkg,
}

pub fn green(pal: &Palette) -> Rgb {
    if pal.dark {
        [0x6c, 0xcb, 0x5f]
    } else {
        [0x0f, 0x7b, 0x0f]
    }
}

pub fn red(pal: &Palette) -> Rgb {
    if pal.dark {
        [0xff, 0x6e, 0x6e]
    } else {
        [0xc4, 0x2b, 0x1c]
    }
}

pub fn amber(pal: &Palette) -> Rgb {
    if pal.dark {
        [0xf0, 0xb4, 0x4c]
    } else {
        [0x9a, 0x5b, 0x00]
    }
}

pub fn blue(pal: &Palette) -> Rgb {
    if pal.dark {
        pal.accent
    } else {
        [0x00, 0x5f, 0xb8]
    }
}
