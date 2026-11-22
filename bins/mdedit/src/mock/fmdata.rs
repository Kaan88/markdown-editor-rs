//! Made-up folders and files for the file manager mockups. The names are the shape of this
//! repository and of a downloads folder; every size and date is invented.

use mdshared_ui::gfx::Rgb;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum K {
    Dir,
    Md,
    Rs,
    Toml,
    Txt,
    Img,
    Zip,
    Exe,
    Pdf,
    Sh,
    Json,
    Mp3,
    Mp4,
    Doc,
}

pub const NOT_COMPUTED: i64 = -1;
pub const COMPUTING: i64 = -2;

#[derive(Clone, Copy)]
pub struct E {
    pub name: &'static str,
    pub k: K,
    /// Bytes; for a folder `NOT_COMPUTED`, `COMPUTING` or the computed total.
    pub size: i64,
    pub date: &'static str,
    /// A cloud placeholder (not on this PC), a read-only mark or none.
    pub flag: Flag,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Flag {
    None,
    Cloud,
    ReadOnly,
    Link,
    Hidden,
}

const fn e(name: &'static str, k: K, size: i64, date: &'static str) -> E {
    E {
        name,
        k,
        size,
        date,
        flag: Flag::None,
    }
}
const fn ef(name: &'static str, k: K, size: i64, date: &'static str, flag: Flag) -> E {
    E {
        name,
        k,
        size,
        date,
        flag,
    }
}

const MB: i64 = 1024 * 1024;
const KB: i64 = 1024;

pub fn human(b: i64) -> String {
    if b < 0 {
        return String::new();
    }
    let f = b as f64;
    if b < KB {
        format!("{b} B")
    } else if b < MB {
        format!("{:.1} KB", f / KB as f64)
    } else if b < 1024 * MB {
        format!("{:.1} MB", f / MB as f64)
    } else {
        format!("{:.2} GB", f / (1024.0 * MB as f64))
    }
}

pub fn type_name(k: K) -> &'static str {
    match k {
        K::Dir => "File folder",
        K::Md => "Markdown",
        K::Rs => "Rust source",
        K::Toml => "TOML",
        K::Txt => "Text",
        K::Img => "PNG image",
        K::Zip => "ZIP archive",
        K::Exe => "Application",
        K::Pdf => "PDF document",
        K::Sh => "Shell script",
        K::Json => "JSON",
        K::Mp3 => "MP3 audio",
        K::Mp4 => "MP4 video",
        K::Doc => "Word document",
    }
}

/// The tint of a type's icon; mid-tones that read on both pages.
pub fn tint(k: K) -> Rgb {
    match k {
        K::Dir => [0xe0, 0xb0, 0x4a],
        K::Md => [0x4a, 0x9e, 0xe0],
        K::Rs => [0xe0, 0x7a, 0x4a],
        K::Toml => [0x9a, 0x8a, 0xd0],
        K::Txt => [0x8a, 0x95, 0xa0],
        K::Img => [0x4a, 0xb0, 0x7a],
        K::Zip => [0xd0, 0xa0, 0x3a],
        K::Exe => [0x8a, 0x6a, 0xd8],
        K::Pdf => [0xd8, 0x5a, 0x5a],
        K::Sh => [0x6a, 0xb8, 0x5a],
        K::Json => [0xc8, 0xb0, 0x4a],
        K::Mp3 => [0xd0, 0x6a, 0xa8],
        K::Mp4 => [0xd0, 0x6a, 0xa8],
        K::Doc => [0x4a, 0x7a, 0xd8],
    }
}

/// `C:\Users\you\Projects\mdedit`.
pub fn project() -> Vec<E> {
    vec![
        e("bins", K::Dir, 3 * MB + 700 * KB, "2026-11-22 09:41"),
        e("crates", K::Dir, 2 * MB + 120 * KB, "2026-11-21 18:02"),
        e("docs", K::Dir, 1 * MB + 380 * KB, "2026-11-22 13:55"),
        e("fixtures", K::Dir, 214 * KB, "2026-11-17 11:20"),
        e("packaging", K::Dir, 18 * MB + 400 * KB, "2026-11-15 20:08"),
        e("shots", K::Dir, 148 * MB + 600 * KB, "2026-11-22 14:07"),
        e("spikes", K::Dir, COMPUTING, "2026-11-13 10:30"),
        e("target", K::Dir, NOT_COMPUTED, "2026-11-22 14:02"),
        e("tests", K::Dir, 61 * KB, "2026-11-21 16:44"),
        e("tools", K::Dir, 44 * KB, "2026-11-19 12:12"),
        ef(".gitattributes", K::Txt, 112, "2026-11-12 09:00", Flag::Hidden),
        ef(".gitignore", K::Txt, 86, "2026-11-12 09:00", Flag::Hidden),
        e("Cargo.lock", K::Toml, 118 * KB + 300, "2026-11-22 09:41"),
        e("Cargo.toml", K::Toml, 7 * KB + 400, "2026-11-22 09:41"),
        e("CLAUDE.md", K::Md, 9 * KB + 200, "2026-11-22 13:55"),
        e("LICENSE", K::Txt, 1 * KB + 80, "2026-11-12 09:00"),
        e("README.md", K::Md, 4 * KB + 800, "2026-11-21 18:02"),
    ]
}

pub fn project_sel() -> Vec<usize> {
    vec![13, 14, 16]
}

/// The same folder on a backup drive, for the compare shot: (entry, what compare says).
#[derive(Clone, Copy, PartialEq)]
pub enum Cmp {
    Same,
    LeftOnly,
    RightOnly,
    LeftNewer,
    RightNewer,
    Differs,
}

pub fn compare_left() -> Vec<(Option<E>, Cmp)> {
    let l = [
        (e("bins", K::Dir, 3 * MB + 700 * KB, "2026-11-22 09:41"), Cmp::Differs),
        (e("crates", K::Dir, 2 * MB + 120 * KB, "2026-11-21 18:02"), Cmp::Same),
        (e("docs", K::Dir, 1 * MB + 380 * KB, "2026-11-22 13:55"), Cmp::LeftNewer),
        (e("fixtures", K::Dir, 214 * KB, "2026-11-17 11:20"), Cmp::Same),
        (e("packaging", K::Dir, 18 * MB + 400 * KB, "2026-11-15 20:08"), Cmp::Same),
        (e("tests", K::Dir, 61 * KB, "2026-11-21 16:44"), Cmp::RightNewer),
        (e("Cargo.lock", K::Toml, 118 * KB + 300, "2026-11-22 09:41"), Cmp::LeftNewer),
        (e("Cargo.toml", K::Toml, 7 * KB + 400, "2026-11-22 09:41"), Cmp::LeftNewer),
        (e("CLAUDE.md", K::Md, 9 * KB + 200, "2026-11-22 13:55"), Cmp::LeftNewer),
        (e("LICENSE", K::Txt, 1 * KB + 80, "2026-11-12 09:00"), Cmp::Same),
        (e("notes-draft.md", K::Md, 2 * KB + 600, "2026-11-22 12:30"), Cmp::LeftOnly),
        (e("README.md", K::Md, 4 * KB + 800, "2026-11-21 18:02"), Cmp::Same),
    ];
    l.into_iter().map(|(x, c)| (Some(x), c)).collect()
}

pub fn compare_right() -> Vec<Option<E>> {
    vec![
        Some(e("bins", K::Dir, 3 * MB + 640 * KB, "2026-11-20 09:12")),
        Some(e("crates", K::Dir, 2 * MB + 120 * KB, "2026-11-21 18:02")),
        Some(e("docs", K::Dir, 1 * MB + 310 * KB, "2026-11-20 17:40")),
        Some(e("fixtures", K::Dir, 214 * KB, "2026-11-17 11:20")),
        Some(e("packaging", K::Dir, 18 * MB + 400 * KB, "2026-11-15 20:08")),
        Some(e("tests", K::Dir, 63 * KB, "2026-11-22 08:15")),
        Some(e("Cargo.lock", K::Toml, 117 * KB, "2026-11-20 09:12")),
        Some(e("Cargo.toml", K::Toml, 7 * KB + 100, "2026-11-20 09:12")),
        Some(e("CLAUDE.md", K::Md, 8 * KB + 900, "2026-11-20 17:40")),
        Some(e("LICENSE", K::Txt, 1 * KB + 80, "2026-11-12 09:00")),
        None,
        Some(e("README.md", K::Md, 4 * KB + 800, "2026-11-21 18:02")),
    ]
}

/// `C:\Users\you\Downloads`, newest first, to be grouped by day.
pub fn downloads() -> (Vec<E>, Vec<(usize, &'static str)>) {
    (
        vec![
            e("mdedit-1.3.0-setup.exe", K::Exe, 12 * MB + 300 * KB, "2026-11-22 14:20"),
            e("invoice-2026-11.pdf", K::Pdf, 188 * KB, "2026-11-22 10:02"),
            ef("tax-papers.zip", K::Zip, 24 * MB, "2026-11-22 09:30", Flag::Cloud),
            e("screenshot-0412.png", K::Img, 640 * KB, "2026-11-21 21:17"),
            e("meeting-notes.docx", K::Doc, 64 * KB, "2026-11-21 16:05"),
            e("podcast-ep-41.mp3", K::Mp3, 38 * MB, "2026-11-20 19:44"),
            e("build-logs.zip", K::Zip, 5 * MB + 100 * KB, "2026-11-19 12:01"),
            e("holiday.mp4", K::Mp4, 420 * MB, "2026-11-14 08:30"),
            e("sprawl-1.0.5.exe", K::Exe, 9 * MB, "2026-11-12 13:12"),
        ],
        vec![(0, "Today"), (3, "Yesterday"), (5, "Earlier this week"), (7, "Earlier this month")],
    )
}

/// `C:\Users\you\Pictures\Trip`, for the thumbnails.
pub fn pictures() -> Vec<E> {
    vec![
        e("IMG_0412.png", K::Img, 3 * MB + 100 * KB, "2026-10-04 11:20"),
        e("IMG_0413.png", K::Img, 2 * MB + 900 * KB, "2026-10-04 11:21"),
        e("IMG_0414.png", K::Img, 3 * MB + 400 * KB, "2026-10-04 11:23"),
        e("harbour.png", K::Img, 4 * MB + 200 * KB, "2026-10-04 12:02"),
        e("market.png", K::Img, 2 * MB + 600 * KB, "2026-10-04 13:48"),
        e("sunset.png", K::Img, 5 * MB + 800 * KB, "2026-10-04 19:15"),
        e("panorama.png", K::Img, 11 * MB, "2026-10-05 08:40"),
        e("map.pdf", K::Pdf, 840 * KB, "2026-10-03 22:10"),
        e("notes.md", K::Md, 1 * KB + 200, "2026-10-06 09:00"),
        e("clip-01.mp4", K::Mp4, 84 * MB, "2026-10-05 10:11"),
        e("clip-02.mp4", K::Mp4, 62 * MB, "2026-10-05 10:40"),
        ef("raw", K::Dir, NOT_COMPUTED, "2026-10-06 21:00", Flag::Cloud),
    ]
}

/// A short list of search hits: (name, folder, line, snippet with the match).
pub struct Hit {
    pub name: &'static str,
    pub path: &'static str,
    pub k: K,
    pub line: Option<u32>,
    pub snippet: &'static str,
    pub at: (usize, usize),
}

pub fn hits() -> Vec<Hit> {
    vec![
        Hit {
            name: "recycle.rs",
            path: "mdshared-ui\\src",
            k: K::Rs,
            line: Some(41),
            snippet: "pub fn recycle(paths: &[PathBuf]) -> Result<(), String> {",
            at: (7, 14),
        },
        Hit {
            name: "STATE.md",
            path: "docs",
            k: K::Md,
            line: Some(88),
            snippet: "the Recycle Bin approved, issues 7 and 8 to a branch",
            at: (4, 15),
        },
        Hit {
            name: "DECISIONS.md",
            path: "docs",
            k: K::Md,
            line: Some(951),
            snippet: "D-095: the Recycle Bin, not a permanent delete, is what Delete does",
            at: (11, 22),
        },
        Hit {
            name: "DESIGN.md",
            path: "docs",
            k: K::Md,
            line: Some(1604),
            snippet: "A deleted note goes to the Recycle Bin and the pane says so",
            at: (27, 38),
        },
        Hit {
            name: "recycle-bin-notes.txt",
            path: "docs\\captures",
            k: K::Txt,
            line: None,
            snippet: "name matches",
            at: (0, 0),
        },
        Hit {
            name: "library.rs",
            path: "mdshared-app\\src",
            k: K::Rs,
            line: Some(1207),
            snippet: "// the note is sent to the Recycle Bin, never removed outright",
            at: (27, 38),
        },
    ]
}
