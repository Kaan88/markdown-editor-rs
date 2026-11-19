//! Pasting an image into a saved Markdown document: the clipboard's PNG is written beside the
//! document and `![](name.png)` goes in at the caret. Only the pure parts are here (names, the
//! link, the bitmap to PNG conversion the Windows clipboard needs); reading the clipboard is in
//! `platform`.

use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// The eight bytes every PNG file starts with.
const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// The most image data a paste takes: a larger clipboard is not an image to put in a text file.
pub const MAX_BYTES: usize = 64 << 20;

/// The toast for an unsaved document: there is no folder to put the image in.
pub const SAVE_FIRST: &str = "Save the document as a Markdown file first, then paste the image";

/// Whether `bytes` start like a PNG file.
pub fn is_png(bytes: &[u8]) -> bool {
    bytes.starts_with(&SIGNATURE)
}

/// The toast after a paste: the file stays when the text is undone.
pub fn saved_toast(name: &str) -> String {
    format!("Image saved as {name}")
}

/// `path` as it goes in a Markdown link: forward slashes, and the characters that would end or
/// change the link written as %XX (a space, brackets, parentheses, `#`, `?`, `%`, control
/// characters). Other characters, accented letters included, stay as they are.
pub fn encode_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for c in path.chars() {
        match c {
            '\\' => out.push('/'),
            ' ' | '%' | '(' | ')' | '[' | ']' | '<' | '>' | '#' | '?' | '"' => escape(&mut out, c),
            c if c.is_control() => escape(&mut out, c),
            c => out.push(c),
        }
    }
    out
}

fn escape(out: &mut String, c: char) {
    let mut buf = [0u8; 4];
    for b in c.encode_utf8(&mut buf).bytes() {
        out.push_str(&format!("%{b:02X}"));
    }
}

/// The link text for an image file at the relative path `rel`.
pub fn markdown_link(rel: &str) -> String {
    format!("![]({})", encode_path(rel))
}

/// The name for the `n`th image of a document called `stem`.
fn image_name(stem: &str, n: u32) -> String {
    format!("{stem}-image-{n}.png")
}

/// A saved image: its file name (in the document's folder), the file, and the text to insert.
pub struct Saved {
    pub name: String,
    pub path: PathBuf,
    pub link: String,
}

/// Writes `png` beside `doc` as `<document name>-image-<n>.png`, the first `n` from 1 whose file
/// does not exist yet (no file is overwritten).
pub fn save_png(doc: &Path, png: &[u8]) -> io::Result<Saved> {
    let dir = match doc.parent() {
        Some(d) if !d.as_os_str().is_empty() => d,
        _ => Path::new("."),
    };
    let stem = doc
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "image".to_string());
    for n in 1..100_000 {
        let name = image_name(&stem, n);
        let path = dir.join(&name);
        let mut file = match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(f) => f,
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        };
        if let Err(e) = file.write_all(png).and_then(|()| file.flush()) {
            drop(file);
            let _ = std::fs::remove_file(&path);
            return Err(e);
        }
        let link = markdown_link(&name);
        return Ok(Saved { name, path, link });
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "too many images with this name",
    ))
}

/// Whether `path` names an image a Markdown page can show, by its extension.
pub fn is_image_file(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        ["png", "jpg", "jpeg", "gif", "webp", "bmp", "svg", "avif"]
            .iter()
            .any(|k| e.eq_ignore_ascii_case(k))
    })
}

/// Where an absolute path starts: a drive (its letter in upper case), a network share (server
/// and share in lower case), or the one root of a system without drives.
#[derive(PartialEq)]
enum Anchor {
    Root,
    Drive(u8),
    Share(String, String),
}

/// Where an absolute path starts and its parts after that. `None` for a path that is not
/// absolute, or one that starts some other way (a device).
fn anchored(path: &Path) -> Option<(Anchor, Vec<&std::ffi::OsStr>)> {
    use std::path::{Component, Prefix};
    let mut drive = Anchor::Root;
    let mut rooted = false;
    let mut parts = Vec::new();
    for c in path.components() {
        match c {
            Component::Prefix(p) => match p.kind() {
                Prefix::Disk(d) | Prefix::VerbatimDisk(d) => {
                    drive = Anchor::Drive(d.to_ascii_uppercase());
                }
                Prefix::UNC(server, share) | Prefix::VerbatimUNC(server, share) => {
                    drive = Anchor::Share(
                        server.to_string_lossy().to_lowercase(),
                        share.to_string_lossy().to_lowercase(),
                    );
                }
                _ => return None,
            },
            Component::RootDir => rooted = true,
            Component::CurDir => {}
            Component::ParentDir => {
                parts.pop()?;
            }
            Component::Normal(s) => parts.push(s),
        }
    }
    rooted.then_some((drive, parts))
}

/// The path of `file` as seen from the folder `dir`, with forward slashes (`../img/a.png`).
/// `None` when there is none: another drive or network share, or a path that is not absolute.
pub fn relative_path(dir: &Path, file: &Path) -> Option<String> {
    let (dir_drive, dir_parts) = anchored(dir)?;
    let (file_drive, file_parts) = anchored(file)?;
    if dir_drive != file_drive {
        return None;
    }
    // Windows names do not differ by case.
    let same = |a: &std::ffi::OsStr, b: &std::ffi::OsStr| {
        if cfg!(windows) {
            a.eq_ignore_ascii_case(b)
        } else {
            a == b
        }
    };
    let common = dir_parts
        .iter()
        .zip(&file_parts)
        .take_while(|(a, b)| same(a, b))
        .count();
    let mut out: Vec<String> = vec!["..".to_string(); dir_parts.len() - common];
    out.extend(
        file_parts[common..]
            .iter()
            .map(|s| s.to_string_lossy().into_owned()),
    );
    (!out.is_empty()).then(|| out.join("/"))
}

/// The image file a pasted text names, when the text is nothing but its full path (in double
/// quotes or not, as Explorer's Copy as path gives it) and the file is there.
pub fn image_path_in_text(text: &str) -> Option<PathBuf> {
    let t = text.trim();
    if t.contains(['\n', '\r']) {
        return None;
    }
    let t = t
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(t);
    let path = PathBuf::from(t);
    (path.is_absolute() && is_image_file(&path) && path.is_file()).then_some(path)
}

/// The link for the image file `file` pasted into the document `doc`: to the file where it is,
/// by its path from the document's folder, or, when there is no such path (another drive), to a
/// copy made beside the document under the file's own name (numbered when that is taken; no
/// file is overwritten). The copy comes back with the link when one was made.
pub fn link_to_file(doc: &Path, file: &Path) -> io::Result<(String, Option<Saved>)> {
    let dir = match doc.parent() {
        Some(d) if !d.as_os_str().is_empty() => d,
        _ => Path::new("."),
    };
    if let Some(rel) = relative_path(dir, file) {
        return Ok((markdown_link(&rel), None));
    }
    let stem = file
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "image".to_string());
    let ext = file
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    let mut from = std::fs::File::open(file)?;
    for n in 0..100_000 {
        let name = if n == 0 {
            format!("{stem}{ext}")
        } else {
            format!("{stem}-{n}{ext}")
        };
        let path = dir.join(&name);
        let mut to = match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(f) => f,
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        };
        if let Err(e) = io::copy(&mut from, &mut to).and_then(|_| to.flush()) {
            drop(to);
            let _ = std::fs::remove_file(&path);
            return Err(e);
        }
        let link = markdown_link(&name);
        return Ok((link.clone(), Some(Saved { name, path, link })));
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "too many images with this name",
    ))
}

/// The file names in a Windows clipboard file list (`CF_HDROP`: a DROPFILES header, then the
/// names one after another, each ended by a zero and the list by one more). Nothing for data
/// that is not one.
#[cfg(any(windows, test))]
pub fn parse_dropfiles(data: &[u8]) -> Vec<String> {
    let u32_at = |o: usize| -> Option<u32> {
        let b = data.get(o..o + 4)?;
        Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    // DROPFILES: the offset of the names, a point, a flag, and whether the names are UTF-16.
    let (Some(start), Some(wide)) = (u32_at(0), u32_at(16)) else {
        return Vec::new();
    };
    let Some(names) = data.get(start as usize..).filter(|_| start >= 20) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if wide != 0 {
        let units: Vec<u16> = names
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| u16::from_le_bytes(*c))
            .collect();
        for name in units.split(|&u| u == 0) {
            if name.is_empty() {
                break;
            }
            out.push(String::from_utf16_lossy(name));
        }
    } else {
        for name in names.split(|&b| b == 0) {
            if name.is_empty() {
                break;
            }
            out.push(String::from_utf8_lossy(name).into_owned());
        }
    }
    out
}

/// The paths in a clipboard list of `file://` URIs (what file managers on Linux offer as
/// `text/uri-list`, and GNOME as `x-special/gnome-copied-files`, whose first line says `copy` or
/// `cut`): one URI to a line, `#` lines are comments, anything that is not a `file:` URI for this
/// machine (no host, or `localhost`) is skipped. Percent escapes are decoded.
#[cfg(any(not(windows), test))]
pub fn parse_uri_list(text: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        let Some(rest) = line
            .get(..7)
            .filter(|p| p.eq_ignore_ascii_case("file://"))
            .map(|_| &line[7..])
        else {
            continue;
        };
        // After the (empty or local) host the path starts at the next slash.
        let path = match rest.find('/') {
            Some(0) => rest,
            Some(i) if rest[..i].eq_ignore_ascii_case("localhost") => &rest[i..],
            _ => continue,
        };
        let mut bytes = Vec::with_capacity(path.len());
        let b = path.as_bytes();
        let mut i = 0;
        while i < b.len() {
            let hex = |c: u8| (c as char).to_digit(16);
            if b[i] == b'%'
                && let (Some(h), Some(l)) = (
                    b.get(i + 1).copied().and_then(hex),
                    b.get(i + 2).copied().and_then(hex),
                )
            {
                bytes.push((h * 16 + l) as u8);
                i += 3;
            } else {
                bytes.push(b[i]);
                i += 1;
            }
        }
        out.push(PathBuf::from(String::from_utf8_lossy(&bytes).into_owned()));
    }
    out
}

/// A Windows clipboard bitmap (`CF_DIB`: a BITMAPINFOHEADER or a later header, then the pixels) as
/// a PNG file. Only 24 and 32 bit colour with the usual channel order is understood; the alpha
/// byte of 32 bit is not trusted (the clipboard's bitmap format leaves it zero), so the picture is
/// opaque. `None` for anything else.
#[cfg(any(windows, test))]
pub fn dib_to_png(dib: &[u8]) -> Option<Vec<u8>> {
    let u16_at = |o: usize| -> Option<u16> {
        let b = dib.get(o..o + 2)?;
        Some(u16::from_le_bytes([b[0], b[1]]))
    };
    let u32_at = |o: usize| -> Option<u32> {
        let b = dib.get(o..o + 4)?;
        Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    let header = u32_at(0)? as usize;
    if header < 40 || dib.len() < header {
        return None;
    }
    let width = u32_at(4)? as i32;
    let height = u32_at(8)? as i32;
    let bits = u16_at(14)?;
    let compression = u32_at(16)?;
    let colours = u32_at(32)? as usize;
    if width <= 0 || height == 0 || !(bits == 24 || bits == 32) {
        return None;
    }
    // 0 is BI_RGB; 3 is BI_BITFIELDS, accepted when the masks are the usual B, G, R bytes.
    let mut data = header.checked_add(colours.checked_mul(4)?)?;
    match compression {
        0 => {}
        3 if bits == 32 => {
            if u32_at(40)? != 0x00FF_0000 || u32_at(44)? != 0x0000_FF00 || u32_at(48)? != 0xFF {
                return None;
            }
            if header == 40 {
                data = data.checked_add(12)?;
            }
        }
        _ => return None,
    }
    let top_down = height < 0;
    let (w, h) = (width as usize, height.unsigned_abs() as usize);
    if w > 32_768 || h > 32_768 {
        return None;
    }
    let bytes_per_pixel = usize::from(bits) / 8;
    let stride = (w * usize::from(bits)).div_ceil(32) * 4;
    let end = data.checked_add(stride.checked_mul(h)?)?;
    if end > dib.len() {
        return None;
    }
    let mut rgb = Vec::with_capacity(w * h * 3);
    for y in 0..h {
        let row = if top_down { y } else { h - 1 - y };
        let line = &dib[data + row * stride..data + row * stride + w * bytes_per_pixel];
        for px in line.chunks_exact(bytes_per_pixel) {
            rgb.extend_from_slice(&[px[2], px[1], px[0]]);
        }
    }
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, w as u32, h as u32);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().ok()?;
        writer.write_image_data(&rgb).ok()?;
        writer.finish().ok()?;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_has_forward_slashes_and_encoded_spaces() {
        assert_eq!(
            markdown_link("my notes-image-1.png"),
            "![](my%20notes-image-1.png)"
        );
        assert_eq!(encode_path("img\\a b.png"), "img/a%20b.png");
        assert_eq!(encode_path("a(1)#2?.png"), "a%281%29%232%3F.png");
        assert_eq!(encode_path("100%.png"), "100%25.png");
        assert_eq!(encode_path("çay.png"), "çay.png");
        assert_eq!(encode_path("a\tb"), "a%09b");
    }

    #[test]
    fn image_files_are_known_by_their_extension() {
        assert!(is_image_file(Path::new("a.PNG")) && is_image_file(Path::new("b/c.jpeg")));
        assert!(!is_image_file(Path::new("a.txt")) && !is_image_file(Path::new("png")));
    }

    #[cfg(windows)]
    #[test]
    fn a_file_is_linked_by_its_path_from_the_document() {
        let dir = Path::new(r"C:\notes\work");
        let rel = |f: &str| relative_path(dir, Path::new(f));
        assert_eq!(rel(r"C:\notes\work\a.png").as_deref(), Some("a.png"));
        assert_eq!(
            rel(r"C:\notes\work\img\a b.png").as_deref(),
            Some("img/a b.png")
        );
        assert_eq!(
            rel(r"C:\notes\pics\a.png").as_deref(),
            Some("../pics/a.png")
        );
        assert_eq!(rel(r"c:\Notes\WORK\a.png").as_deref(), Some("a.png"));
        assert_eq!(rel(r"C:\a.png").as_deref(), Some("../../a.png"));
        // The long form of a path names the same drive.
        assert_eq!(rel(r"\\?\C:\notes\work\a.png").as_deref(), Some("a.png"));
        // Another drive, a network path, a relative path: no path from here.
        assert_eq!(rel(r"D:\notes\work\a.png"), None);
        assert_eq!(rel(r"\\server\share\a.png"), None);
        assert_eq!(rel("a.png"), None);
        // Inside one network share there is a path; from one share to another there is none.
        let share = |f: &str| relative_path(Path::new(r"\\Server\docs\work"), Path::new(f));
        assert_eq!(
            share(r"\\server\Docs\img\a.png").as_deref(),
            Some("../img/a.png")
        );
        assert_eq!(share(r"\\server\other\a.png"), None);
        assert_eq!(share(r"C:\a.png"), None);
    }

    #[cfg(not(windows))]
    #[test]
    fn a_file_is_linked_by_its_path_from_the_document() {
        let dir = Path::new("/notes/work");
        let rel = |f: &str| relative_path(dir, Path::new(f));
        assert_eq!(rel("/notes/work/a.png").as_deref(), Some("a.png"));
        assert_eq!(rel("/notes/pics/a.png").as_deref(), Some("../pics/a.png"));
        assert_eq!(rel("/a.png").as_deref(), Some("../../a.png"));
        assert_eq!(rel("a.png"), None);
    }

    /// A pasted path to an image (from another folder, quoted as Copy as path gives it) is
    /// taken as that image; anything else stays text.
    #[test]
    fn a_pasted_image_path_is_an_image() {
        let dir = temp_dir("pasted-path");
        let file = dir.join("elsewhere").join("my pic.png");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, SIGNATURE).unwrap();
        let shown = file.display().to_string();
        assert_eq!(image_path_in_text(&shown), Some(file.clone()));
        assert_eq!(
            image_path_in_text(&format!("\"{shown}\"\r\n")),
            Some(file.clone())
        );
        // Not an image, not there, not alone, not a full path: text.
        let txt = dir.join("a.txt");
        std::fs::write(&txt, "x").unwrap();
        assert_eq!(image_path_in_text(&txt.display().to_string()), None);
        assert_eq!(
            image_path_in_text(&dir.join("gone.png").display().to_string()),
            None
        );
        assert_eq!(image_path_in_text(&format!("see {shown}")), None);
        assert_eq!(image_path_in_text(&format!("{shown}\n{shown}")), None);
        assert_eq!(image_path_in_text("my pic.png"), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_in_reach_is_linked_where_it_is() {
        let dir = temp_dir("link");
        std::fs::create_dir_all(dir.join("img")).unwrap();
        let file = dir.join("img").join("my pic.png");
        std::fs::write(&file, SIGNATURE).unwrap();
        let (link, copy) = link_to_file(&dir.join("a.md"), &file).unwrap();
        assert_eq!(link, "![](img/my%20pic.png)");
        assert!(copy.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_clipboard_file_list_is_read() {
        let names = [r"C:\a b.png", "D:\\\u{e7}ay.jpg"];
        let mut data = Vec::new();
        data.extend_from_slice(&20u32.to_le_bytes());
        data.extend_from_slice(&[0; 12]);
        data.extend_from_slice(&1u32.to_le_bytes());
        for name in names {
            for u in name.encode_utf16() {
                data.extend_from_slice(&u.to_le_bytes());
            }
            data.extend_from_slice(&[0, 0]);
        }
        data.extend_from_slice(&[0, 0]);
        assert_eq!(parse_dropfiles(&data), names);
        // Not wide: bytes.
        let mut narrow = Vec::new();
        narrow.extend_from_slice(&20u32.to_le_bytes());
        narrow.extend_from_slice(&[0; 16]);
        narrow.extend_from_slice(b"C:\\a.png\0\0");
        assert_eq!(parse_dropfiles(&narrow), [r"C:\a.png"]);
        // Too short, or an offset outside the data.
        assert!(parse_dropfiles(&[0; 8]).is_empty());
        let mut bad = vec![0u8; 20];
        bad[0] = 200;
        assert!(parse_dropfiles(&bad).is_empty());
    }

    #[test]
    fn a_uri_list_from_a_linux_file_manager_is_read() {
        let list = "# a comment\r\nfile:///home/me/My%20Pics/a%C3%A7.png\r\nfile://localhost/tmp/b.jpg\n\
                    file://otherhost/share/c.png\nhttps://example.com/d.png\nfile:///100%25.gif\nfile:///bad%zz%4\n";
        assert_eq!(
            parse_uri_list(list),
            [
                PathBuf::from("/home/me/My Pics/a\u{e7}.png"),
                PathBuf::from("/tmp/b.jpg"),
                PathBuf::from("/100%.gif"),
                PathBuf::from("/bad%zz%4"),
            ]
        );
        // GNOME's list starts with what the file manager will do.
        assert_eq!(
            parse_uri_list("copy\nfile:///a.png\nfile:///b.png"),
            [PathBuf::from("/a.png"), PathBuf::from("/b.png")]
        );
        assert!(parse_uri_list("").is_empty());
        assert!(parse_uri_list("\u{e7}\u{e7}\u{e7}\u{e7}").is_empty());
    }

    #[test]
    fn png_signature_is_checked() {
        assert!(is_png(&SIGNATURE));
        assert!(!is_png(b"GIF89a"));
        assert!(!is_png(&[]));
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mdedit-paste-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn saved_beside_the_document_without_overwriting() {
        let dir = temp_dir("save");
        let doc = dir.join("my notes.md");
        let a = save_png(&doc, &SIGNATURE).unwrap();
        assert_eq!(a.name, "my notes-image-1.png");
        assert_eq!(a.link, "![](my%20notes-image-1.png)");
        assert_eq!(std::fs::read(&a.path).unwrap(), SIGNATURE);
        let b = save_png(&doc, b"second").unwrap();
        assert_eq!(b.name, "my notes-image-2.png");
        // The first file is as it was.
        assert_eq!(std::fs::read(&a.path).unwrap(), SIGNATURE);
        // A gap is not filled before the next free number is looked for from 1: 1 and 2 exist.
        std::fs::remove_file(&a.path).unwrap();
        assert_eq!(
            save_png(&doc, b"third").unwrap().name,
            "my notes-image-1.png"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_folder_is_an_error() {
        let dir = temp_dir("missing");
        let doc = dir.join("gone").join("a.md");
        assert!(save_png(&doc, b"x").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn dib_header(w: i32, h: i32, bits: u16, compression: u32) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&40u32.to_le_bytes());
        v.extend_from_slice(&w.to_le_bytes());
        v.extend_from_slice(&h.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes());
        v.extend_from_slice(&bits.to_le_bytes());
        v.extend_from_slice(&compression.to_le_bytes());
        v.extend_from_slice(&[0; 20]);
        v
    }

    fn decode(png_bytes: &[u8]) -> (u32, u32, Vec<u8>) {
        let mut reader = png::Decoder::new(std::io::Cursor::new(png_bytes))
            .read_info()
            .unwrap();
        let mut buf = vec![0u8; 4096];
        let info = reader.next_frame(&mut buf).unwrap();
        (info.width, info.height, buf[..info.buffer_size()].to_vec())
    }

    #[test]
    fn bottom_up_24_bit_bitmap_becomes_a_png_with_padded_rows_removed() {
        // 3 x 2, rows of 9 bytes padded to 12; the bottom row comes first. Pixels are B, G, R.
        let mut dib = dib_header(3, 2, 24, 0);
        let bottom: [u8; 9] = [1, 2, 3, 4, 5, 6, 7, 8, 9];
        let top: [u8; 9] = [11, 12, 13, 14, 15, 16, 17, 18, 19];
        dib.extend_from_slice(&bottom);
        dib.extend_from_slice(&[0; 3]);
        dib.extend_from_slice(&top);
        dib.extend_from_slice(&[0; 3]);
        let png = dib_to_png(&dib).unwrap();
        assert!(is_png(&png));
        let (w, h, px) = decode(&png);
        assert_eq!((w, h), (3, 2));
        assert_eq!(&px[..9], &[13, 12, 11, 16, 15, 14, 19, 18, 17]);
        assert_eq!(&px[9..], &[3, 2, 1, 6, 5, 4, 9, 8, 7]);
    }

    #[test]
    fn top_down_32_bit_bitmap_ignores_the_alpha_byte() {
        let mut dib = dib_header(1, -2, 32, 0);
        dib.extend_from_slice(&[10, 20, 30, 0]);
        dib.extend_from_slice(&[40, 50, 60, 0]);
        let (w, h, px) = decode(&dib_to_png(&dib).unwrap());
        assert_eq!((w, h), (1, 2));
        assert_eq!(px, vec![30, 20, 10, 60, 50, 40]);
    }

    #[test]
    fn bitmaps_that_are_not_understood_are_refused() {
        // Too short for the pixels, 8 bit colour, run-length coding, no size.
        assert!(dib_to_png(&dib_header(2, 2, 24, 0)).is_none());
        assert!(dib_to_png(&dib_header(1, 1, 8, 0)).is_none());
        assert!(dib_to_png(&dib_header(1, 1, 24, 1)).is_none());
        assert!(dib_to_png(&dib_header(0, 1, 24, 0)).is_none());
        assert!(dib_to_png(&[]).is_none());
        // Sizes that would overflow are refused, not tried.
        assert!(dib_to_png(&dib_header(i32::MAX, 1, 32, 0)).is_none());
    }
}
