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
