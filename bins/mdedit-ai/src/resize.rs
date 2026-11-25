//! A picture made ready to be sent to a model (`docs/ai-pictures-plan.md` section 7).
//!
//! The bytes of a JPEG, PNG, WebP, GIF or BMP, known by their first bytes and never by a name.
//! A picture inside both limits (its long side, [`LONG_SIDE`] or [`LONG_SIDE_LARGE`]; the bytes,
//! [`MAX_BYTES`]) and stored upright goes as it came, byte for byte, with its location and camera
//! details taken out when [`Options::strip`] asks (D-205): the segments or chunks that hold them
//! are dropped and nothing is decoded. Anything else is decoded, scaled down by an area average,
//! turned upright by its EXIF orientation and written again: a JPEG as a JPEG (quality 85), a PNG
//! as a PNG, a WebP and a BMP as a PNG. An animation (a GIF, an animated WebP) cannot be scaled
//! without becoming a different picture, so it goes as it came or not at all.
//!
//! Every byte here comes from a file nobody has checked, so nothing may panic (CLAUDE.md rule 11:
//! the release build aborts on a panic). Sizes are read from the header and checked before a
//! decoder allocates, the pixel count is capped, and every slice and index is checked first.

use std::fmt;
use std::io::Cursor;

/// The largest picture sent: 3.5 MB, about 4.7 MB as base64, under the 5 MB a provider takes.
pub const MAX_BYTES: usize = 3_500_000;
/// The long side a picture is scaled to.
pub const LONG_SIDE: u32 = 1568;
/// The long side with "Send larger pictures, for small print".
pub const LONG_SIDE_LARGE: u32 = 2576;
/// The most pixels a file may claim before it is refused: 64 megapixels, 256 MB as RGBA.
pub const MAX_PIXELS: u64 = 64_000_000;
/// The quality a JPEG is written at.
pub const JPEG_QUALITY: u8 = 85;
/// The long side of a thumbnail.
pub const THUMB_SIDE: u32 = 160;
/// Attempts at getting under [`MAX_BYTES`] when the long-side cap alone did not do it.
const MAX_ATTEMPTS: u8 = 4;

/// What a file's first bytes say it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Jpeg,
    Png,
    WebP,
    Gif,
    /// Read, never sent: written as a PNG.
    Bmp,
}

impl Kind {
    /// The type sent with the picture.
    pub fn mime(self) -> &'static str {
        match self {
            Kind::Jpeg => "image/jpeg",
            Kind::Png => "image/png",
            Kind::WebP => "image/webp",
            Kind::Gif => "image/gif",
            Kind::Bmp => "image/bmp",
        }
    }

    /// The format's short name, for the event and a file name's extension.
    pub fn short(self) -> &'static str {
        match self {
            Kind::Jpeg => "jpeg",
            Kind::Png => "png",
            Kind::WebP => "webp",
            Kind::Gif => "gif",
            Kind::Bmp => "bmp",
        }
    }
}

/// How a picture is made ready.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    /// The long side it is scaled to when it is over it.
    pub long_side: u32,
    /// Take the location and camera details out.
    pub strip: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            long_side: LONG_SIDE,
            strip: true,
        }
    }
}

/// A picture ready to be sent.
#[derive(Debug, PartialEq, Eq)]
pub struct Prepared {
    /// The bytes to send.
    pub bytes: Vec<u8>,
    /// The type of `bytes`.
    pub kind: Kind,
    /// The pixels of `bytes`.
    pub width: u32,
    pub height: u32,
    /// The original's pixels, upright.
    pub original_width: u32,
    pub original_height: u32,
    /// Scaled down.
    pub smaller: bool,
    /// Turned upright by its orientation.
    pub turned: bool,
    /// Details (EXIF, XMP, text chunks) the original had are not in `bytes`.
    pub stripped: bool,
    /// Written in another format than it came in.
    pub converted: bool,
}

impl Prepared {
    /// What was done, in the words the window shows: `as it is` when nothing.
    pub fn done(&self) -> Vec<&'static str> {
        let mut v = Vec::new();
        if self.smaller {
            v.push("smaller");
        }
        if self.turned {
            v.push("turned upright");
        }
        if self.stripped {
            v.push("details removed");
        }
        if self.converted {
            v.push("made a PNG");
        }
        if v.is_empty() {
            v.push("as it is");
        }
        v
    }
}

/// Why a picture cannot be sent.
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    /// A known format this program does not read (HEIC, AVIF, TIFF, SVG).
    NotRead(&'static str),
    /// Not a picture at all, as far as its first bytes say.
    Unsupported,
    /// The header says more pixels than [`MAX_PIXELS`], or none at all.
    TooLarge { width: u32, height: u32 },
    /// An animation over [`MAX_BYTES`]: it cannot be scaled.
    Animation,
    /// The decoder would not read it: truncated, corrupt, or not what its first bytes claimed.
    Corrupt(Kind),
    /// The encoder failed.
    Encode,
    /// Still over [`MAX_BYTES`] after [`MAX_ATTEMPTS`] tries.
    StillTooBig,
    /// Its details were to be removed, and an animation, which cannot be written again here,
    /// could not be walked to find them (Fable's review of V1, 15).
    DetailsKept(Kind),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotRead(what) => write!(
                f,
                "{what}: not a picture this program can read. Save it as JPEG first."
            ),
            Error::Unsupported => f.write_str("not a picture this program can read"),
            Error::TooLarge { width, height } => write!(
                f,
                "{width} × {height} pixels, more than the {} million a picture may have",
                MAX_PIXELS / 1_000_000
            ),
            Error::Animation => write!(
                f,
                "an animation over {} MB, which cannot be made smaller",
                MAX_BYTES as f64 / 1e6
            ),
            Error::Corrupt(kind) => write!(f, "the {} could not be read", kind_name(*kind)),
            Error::Encode => f.write_str("the picture could not be written again"),
            Error::DetailsKept(kind) => write!(
                f,
                "the {}'s details could not be found to take out: untick \
                 \"Remove location and camera details\" to send it with them",
                kind_name(*kind)
            ),
            Error::StillTooBig => write!(
                f,
                "still over {} MB after {MAX_ATTEMPTS} tries at making it smaller",
                MAX_BYTES as f64 / 1e6
            ),
        }
    }
}

impl std::error::Error for Error {}

fn kind_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Jpeg => "JPEG",
        Kind::Png => "PNG",
        Kind::WebP => "WebP",
        Kind::Gif => "GIF",
        Kind::Bmp => "BMP",
    }
}

fn u16_le(b: &[u8], at: usize) -> Option<u16> {
    let s = b.get(at..at.checked_add(2)?)?;
    Some(u16::from_le_bytes([s[0], s[1]]))
}

fn u32_le(b: &[u8], at: usize) -> Option<u32> {
    let s = b.get(at..at.checked_add(4)?)?;
    Some(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn u16_be(b: &[u8], at: usize) -> Option<u16> {
    let s = b.get(at..at.checked_add(2)?)?;
    Some(u16::from_be_bytes([s[0], s[1]]))
}

fn u32_be(b: &[u8], at: usize) -> Option<u32> {
    let s = b.get(at..at.checked_add(4)?)?;
    Some(u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
}

/// The format from the leading bytes, never the name: a `.jpg` that is really a PNG is a PNG.
pub fn kind(bytes: &[u8]) -> Option<Kind> {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some(Kind::Jpeg)
    } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(Kind::Png)
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some(Kind::Gif)
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some(Kind::WebP)
    } else if bytes.starts_with(b"BM")
        && matches!(u32_le(bytes, 14), Some(40 | 52 | 56 | 108 | 124))
    {
        Some(Kind::Bmp)
    } else {
        None
    }
}

/// The name of a known format that is not read, for the row's reason.
fn not_read(bytes: &[u8]) -> Option<&'static str> {
    if bytes.get(4..8) == Some(b"ftyp") {
        let brand = bytes.get(8..12)?;
        return match brand {
            b"heic" | b"heix" | b"hevc" | b"hevx" | b"heim" | b"heis" | b"hevm" | b"hevs"
            | b"mif1" | b"msf1" => Some("HEIC"),
            b"avif" | b"avis" => Some("AVIF"),
            _ => None,
        };
    }
    if bytes.starts_with(b"II*\0") || bytes.starts_with(b"MM\0*") {
        return Some("TIFF");
    }
    let head = &bytes[..bytes.len().min(1024)];
    let text = String::from_utf8_lossy(head);
    let t = text.trim_start_matches('\u{feff}').trim_start();
    if (t.starts_with("<svg") || t.starts_with("<?xml") || t.starts_with("<!DOCTYPE svg"))
        && t.contains("<svg")
    {
        return Some("SVG");
    }
    None
}

/// `bytes` made ready to send, by `options`.
pub fn prepare(bytes: &[u8], options: &Options) -> Result<Prepared, Error> {
    let kind =
        kind(bytes).ok_or_else(|| not_read(bytes).map_or(Error::Unsupported, Error::NotRead))?;
    let (width, height) = declared(bytes, kind)?;
    fits(width, height)?;
    let long_side = options.long_side.clamp(64, LONG_SIDE_LARGE);
    let stripped_copy = |strip: bool| -> Result<Option<Vec<u8>>, ()> {
        if strip {
            strip_details(bytes, kind)
        } else {
            Ok(None)
        }
    };
    let as_it_came = |out: Option<Vec<u8>>| {
        let stripped = out.is_some();
        Prepared {
            bytes: out.unwrap_or_else(|| bytes.to_vec()),
            kind,
            width,
            height,
            original_width: width,
            original_height: height,
            smaller: false,
            turned: false,
            stripped,
            converted: false,
        }
    };
    // An animation cannot be scaled while keeping what it is: one frame is not the picture. Its
    // details are taken out like a still's; one whose structure cannot be walked to find them is
    // refused while they are to go (Fable's review of V1, 15 and 23: GIFs kept theirs, an
    // unreadable WebP was sent as it came, an animated PNG was flattened to its first frame).
    if kind == Kind::Gif
        || (kind == Kind::WebP && webp_is_animated(bytes))
        || (kind == Kind::Png && png_is_animated(bytes))
    {
        let out = stripped_copy(options.strip).map_err(|()| Error::DetailsKept(kind))?;
        let p = as_it_came(out);
        if p.bytes.len() > MAX_BYTES {
            return Err(Error::Animation);
        }
        return Ok(p);
    }
    let turn = orientation(bytes, kind);
    if kind != Kind::Bmp && turn == 1 && width.max(height) <= long_side {
        // A file whose details cannot be found for certain is written again instead.
        if let Ok(out) = stripped_copy(options.strip) {
            let p = as_it_came(out);
            if p.bytes.len() <= MAX_BYTES {
                return Ok(p);
            }
        }
    }
    let had_details = !matches!(strip_details(bytes, kind), Ok(None));
    let mut image = decode(bytes, kind, width, height)?;
    let (original_width, original_height) = if turn >= 5 {
        (height, width)
    } else {
        (width, height)
    };
    let mut limit = long_side;
    for _ in 0..MAX_ATTEMPTS {
        image = scaled_to(&image, limit)?;
        let upright = turned(&image, turn)?;
        let out = encode(&upright, kind)?;
        if out.len() <= MAX_BYTES {
            let written = written_as(kind);
            return Ok(Prepared {
                bytes: out,
                kind: written,
                width: upright.width,
                height: upright.height,
                original_width,
                original_height,
                smaller: upright.width < original_width || upright.height < original_height,
                turned: turn != 1,
                stripped: had_details,
                converted: written != kind,
            });
        }
        // The long-side cap was not enough for this picture: try again at the size that would
        // bring the file under the cap, a little smaller for the next write's size.
        let next = (limit as f64 * (MAX_BYTES as f64 / out.len() as f64).sqrt() * 0.95) as u32;
        if next == 0 || next >= limit {
            break;
        }
        limit = next;
    }
    Err(Error::StillTooBig)
}

/// A PNG of at most [`THUMB_SIDE`] pixels on its long side, of what is sent; `None` for a GIF
/// (no decoder here) or anything that does not decode.
pub fn thumbnail(p: &Prepared) -> Option<Vec<u8>> {
    if p.kind == Kind::Gif {
        return None;
    }
    let (w, h) = declared(&p.bytes, p.kind).ok()?;
    fits(w, h).ok()?;
    let image = decode(&p.bytes, p.kind, w, h).ok()?;
    let small = scaled_to(&image, THUMB_SIDE).ok()?;
    encode_png(&small).ok()
}

/// Whether a PNG is an animation: an `acTL` chunk before its first `IDAT`.
fn png_is_animated(bytes: &[u8]) -> bool {
    png_chunks(bytes).is_some_and(|(chunks, _)| {
        chunks
            .iter()
            .take_while(|c| &c.0 != b"IDAT")
            .any(|c| &c.0 == b"acTL")
    })
}

/// Whether a WebP is an animation. The still first frame of one decodes fine, which is exactly
/// why it has to be asked before the picture is scaled.
fn webp_is_animated(bytes: &[u8]) -> bool {
    image_webp::WebPDecoder::new(Cursor::new(bytes)).is_ok_and(|decoder| decoder.is_animated())
}

/// The type the bytes are written as.
fn written_as(kind: Kind) -> Kind {
    match kind {
        Kind::Jpeg => Kind::Jpeg,
        // The WebP decoder's own encoder is lossless (VP8L) only, which would be several times
        // the size of a lossy source for a photo; PNG is the format every connection accepts.
        Kind::Png | Kind::WebP | Kind::Bmp => Kind::Png,
        Kind::Gif => Kind::Gif,
    }
}

/// No pixels at all, or more than [`MAX_PIXELS`].
fn fits(width: u32, height: u32) -> Result<(), Error> {
    if width == 0 || height == 0 || u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err(Error::TooLarge { width, height });
    }
    Ok(())
}

/// The size the header claims, before anything is decoded.
fn declared(bytes: &[u8], kind: Kind) -> Result<(u32, u32), Error> {
    match kind {
        Kind::Jpeg => {
            let mut decoder = zune_jpeg::JpegDecoder::new(Cursor::new(bytes));
            decoder.decode_headers().map_err(|_| Error::Corrupt(kind))?;
            let info = decoder.info().ok_or(Error::Corrupt(kind))?;
            Ok((u32::from(info.width), u32::from(info.height)))
        }
        Kind::Png => {
            let decoder = png::Decoder::new(Cursor::new(bytes));
            let reader = decoder.read_info().map_err(|_| Error::Corrupt(kind))?;
            let info = reader.info();
            Ok((info.width, info.height))
        }
        Kind::WebP => {
            let decoder = image_webp::WebPDecoder::new(Cursor::new(bytes))
                .map_err(|_| Error::Corrupt(kind))?;
            Ok(decoder.dimensions())
        }
        // The logical screen: every frame is drawn inside it.
        Kind::Gif => {
            let w = u16_le(bytes, 6).ok_or(Error::Corrupt(kind))?;
            let h = u16_le(bytes, 8).ok_or(Error::Corrupt(kind))?;
            Ok((u32::from(w), u32::from(h)))
        }
        Kind::Bmp => bmp_info(bytes)
            .map(|b| (b.width, b.height))
            .ok_or(Error::Corrupt(kind)),
    }
}

// ---- BMP -------------------------------------------------------------------------------------

/// What a BMP file's headers say: 24 or 32 bits a pixel, uncompressed (or 32 bits with the usual
/// masks), the pixels at `offset`.
struct BmpInfo {
    width: u32,
    height: u32,
    top_down: bool,
    bytes_per_pixel: usize,
    offset: usize,
    stride: usize,
}

fn bmp_info(b: &[u8]) -> Option<BmpInfo> {
    let offset = u32_le(b, 10)? as usize;
    let width = u32_le(b, 18)? as i32;
    let height = u32_le(b, 22)? as i32;
    let bits = u16_le(b, 28)?;
    let compression = u32_le(b, 30)?;
    if width <= 0 || height == 0 || !(bits == 24 || bits == 32) {
        return None;
    }
    match compression {
        0 => {}
        // BI_BITFIELDS: the masks follow a 40-byte header, or sit inside a later one, at the same
        // place in the file.
        3 if bits == 32 => {
            if u32_le(b, 54)? != 0x00FF_0000
                || u32_le(b, 58)? != 0x0000_FF00
                || u32_le(b, 62)? != 0xFF
            {
                return None;
            }
        }
        _ => return None,
    }
    let (w, h) = (width as u32, height.unsigned_abs());
    let stride = (w as usize)
        .checked_mul(usize::from(bits))?
        .div_ceil(32)
        .checked_mul(4)?;
    Some(BmpInfo {
        width: w,
        height: h,
        top_down: height < 0,
        bytes_per_pixel: usize::from(bits) / 8,
        offset,
        stride,
    })
}

/// The pixels as RGB, opaque: the fourth byte of a 32-bit BMP is most often left zero.
fn bmp_pixels(b: &[u8], info: &BmpInfo) -> Option<Vec<u8>> {
    let (w, h) = (info.width as usize, info.height as usize);
    let end = info.offset.checked_add(info.stride.checked_mul(h)?)?;
    if end > b.len() {
        return None;
    }
    let mut rgb = Vec::with_capacity(w.checked_mul(h)?.checked_mul(3)?);
    for y in 0..h {
        let row = if info.top_down { y } else { h - 1 - y };
        let start = info.offset + row * info.stride;
        let line = b.get(start..start + w * info.bytes_per_pixel)?;
        for px in line.chunks_exact(info.bytes_per_pixel) {
            rgb.extend_from_slice(&[px[2], px[1], px[0]]);
        }
    }
    Some(rgb)
}

// ---- the details a file carries --------------------------------------------------------------

/// A JPEG's segments before its scan: (marker, start, data start, end), and where the scan starts.
type Segments = (Vec<(u8, usize, usize, usize)>, usize);

fn jpeg_segments(b: &[u8]) -> Option<Segments> {
    if !b.starts_with(&[0xFF, 0xD8]) {
        return None;
    }
    let mut i = 2;
    let mut segments = Vec::new();
    loop {
        if segments.len() > 4096 || *b.get(i)? != 0xFF {
            return None;
        }
        let mut j = i + 1;
        while *b.get(j)? == 0xFF {
            j += 1;
        }
        let marker = *b.get(j)?;
        // The scan, or the end: everything from here is copied as it is.
        if marker == 0xDA || marker == 0xD9 {
            return Some((segments, i));
        }
        if (0xD0..=0xD7).contains(&marker) || marker == 0x01 {
            segments.push((marker, i, j + 1, j + 1));
            i = j + 1;
            continue;
        }
        let len = usize::from(u16_be(b, j + 1)?);
        if len < 2 {
            return None;
        }
        let end = (j + 1).checked_add(len)?;
        if end > b.len() {
            return None;
        }
        segments.push((marker, i, j + 3, end));
        i = end;
    }
}

/// A PNG's chunks: (type, start, data start, data end, end), up to and with IEND.
type Chunks = Vec<([u8; 4], usize, usize, usize, usize)>;

fn png_chunks(b: &[u8]) -> Option<(Chunks, usize)> {
    if !b.starts_with(b"\x89PNG\r\n\x1a\n") {
        return None;
    }
    let mut i = 8;
    let mut chunks = Vec::new();
    loop {
        let len = u32_be(b, i)? as usize;
        let ty: [u8; 4] = b.get(i + 4..i + 8)?.try_into().ok()?;
        let data = i + 8;
        let data_end = data.checked_add(len)?;
        let end = data_end.checked_add(4)?;
        if end > b.len() {
            return None;
        }
        chunks.push((ty, i, data, data_end, end));
        i = end;
        if &ty == b"IEND" {
            return Some((chunks, i));
        }
    }
}

/// A WebP's chunks after `RIFF....WEBP`, each with its padding.
fn riff_chunks(b: &[u8]) -> Option<Chunks> {
    if b.len() < 12 || &b[..4] != b"RIFF" || &b[8..12] != b"WEBP" {
        return None;
    }
    let total = (u32_le(b, 4)? as usize).checked_add(8)?;
    if total > b.len() {
        return None;
    }
    let mut i = 12;
    let mut chunks = Vec::new();
    while i < total {
        let ty: [u8; 4] = b.get(i..i + 4)?.try_into().ok()?;
        let size = u32_le(b, i + 4)? as usize;
        let data = i + 8;
        let data_end = data.checked_add(size)?;
        if data_end > total {
            return None;
        }
        let end = (data_end + (size & 1)).min(total);
        chunks.push((ty, i, data, data_end, end));
        i = end;
    }
    Some(chunks)
}

/// Whether a JPEG segment holds details: every APPn and comment but those that say how to show
/// the picture: APP0 (JFIF), APP2's colour profile and APP14 (Adobe's colour transform). A list
/// of what is kept, not of what goes, so a segment never seen before goes (Fable's review of V1,
/// 14: APP11's content credentials and the other APPn were kept).
fn jpeg_detail(marker: u8, data: &[u8]) -> bool {
    match marker {
        0xE0 | 0xEE => false,
        0xE2 => !data.starts_with(b"ICC_PROFILE\0"),
        0xE1..=0xEF | 0xFE => true,
        _ => false,
    }
}

/// Whether a PNG chunk holds details: every chunk but the image's own and those that say how to
/// show it (colour, transparency, density, animation). A list of what is kept, as for a JPEG.
fn png_detail(ty: &[u8; 4]) -> bool {
    !matches!(
        ty,
        b"IHDR"
            | b"PLTE"
            | b"IDAT"
            | b"IEND"
            | b"tRNS"
            | b"gAMA"
            | b"cHRM"
            | b"sRGB"
            | b"iCCP"
            | b"sBIT"
            | b"cICP"
            | b"mDCv"
            | b"cLLi"
            | b"bKGD"
            | b"pHYs"
            | b"acTL"
            | b"fcTL"
            | b"fdAT"
    )
}

/// The end of a JPEG's image data, past its EOI, from `scan`: what a phone puts after it (a
/// motion photo's video, a depth map, a second picture with its own EXIF) is not the picture
/// (Fable's review of V1, 13). `None` when there is no EOI: a file cut short.
fn jpeg_end(b: &[u8], scan: usize) -> Option<usize> {
    let rest = b.get(scan..)?;
    // Inside the entropy-coded data a 0xFF is followed by 0x00 or a restart marker, never by
    // 0xD9, so the first FF D9 is the end.
    rest.windows(2)
        .position(|w| w == [0xFF, 0xD9])
        .map(|at| scan + at + 2)
}

/// A GIF without its comments and application blocks (but the loop count's), and without what
/// follows its end: `(bytes, whether anything went)`; `None` when its blocks cannot be walked.
fn gif_stripped(b: &[u8]) -> Option<(Vec<u8>, bool)> {
    if b.len() < 13 || !(b.starts_with(b"GIF87a") || b.starts_with(b"GIF89a")) {
        return None;
    }
    let table = |flags: u8| {
        if flags & 0x80 != 0 {
            3usize << ((flags & 0x07) + 1)
        } else {
            0
        }
    };
    let mut i = 13usize.checked_add(table(b[10]))?;
    let mut out = b.get(..i)?.to_vec();
    let mut dropped = false;
    // The sub-blocks from `at`: the index after their zero length.
    let blocks = |mut at: usize| -> Option<usize> {
        loop {
            let n = usize::from(*b.get(at)?);
            at = at.checked_add(1 + n)?;
            if n == 0 {
                return Some(at);
            }
            if at > b.len() {
                return None;
            }
        }
    };
    loop {
        match *b.get(i)? {
            0x3B => {
                out.push(0x3B);
                return Some((out, dropped || i + 1 != b.len()));
            }
            0x2C => {
                let flags = *b.get(i + 9)?;
                let data = (i + 10).checked_add(table(flags))?;
                // The LZW code size, then the image's sub-blocks.
                let end = blocks(data.checked_add(1)?)?;
                out.extend_from_slice(b.get(i..end)?);
                i = end;
            }
            0x21 => {
                let label = *b.get(i + 1)?;
                let end = blocks(i + 2)?;
                let block = b.get(i..end)?;
                let looping = label == 0xFF
                    && block
                        .get(3..14)
                        .is_some_and(|id| id == b"NETSCAPE2.0" || id == b"ANIMEXTS1.0");
                // Comments and application blocks hold details; graphic control and plain text
                // are the picture.
                if matches!(label, 0xFE | 0xFF) && !looping {
                    dropped = true;
                } else {
                    out.extend_from_slice(block);
                }
                i = end;
            }
            _ => return None,
        }
    }
}

/// `bytes` without its details, `None` when it had none; `Err` when the file's structure could
/// not be walked to the end, so whether it has any is not known.
fn strip_details(bytes: &[u8], kind: Kind) -> Result<Option<Vec<u8>>, ()> {
    match kind {
        Kind::Jpeg => {
            let (segments, scan) = jpeg_segments(bytes).ok_or(())?;
            let end = jpeg_end(bytes, scan).ok_or(())?;
            let detail = |&(marker, _, data, end): &(u8, usize, usize, usize)| {
                jpeg_detail(marker, bytes.get(data..end).unwrap_or(&[]))
            };
            if !segments.iter().any(detail) && end == bytes.len() {
                return Ok(None);
            }
            let mut out = bytes[..2].to_vec();
            for s in &segments {
                if !detail(s) {
                    out.extend_from_slice(&bytes[s.1..s.3]);
                }
            }
            out.extend_from_slice(&bytes[scan..end]);
            Ok(Some(out))
        }
        Kind::Png => {
            let (chunks, end) = png_chunks(bytes).ok_or(())?;
            if !chunks.iter().any(|c| png_detail(&c.0)) && end == bytes.len() {
                return Ok(None);
            }
            // Anything after IEND goes too: no reader shows it, and it can hold anything.
            let mut out = bytes[..8].to_vec();
            for &(ty, start, _, _, end) in &chunks {
                if !png_detail(&ty) {
                    out.extend_from_slice(&bytes[start..end]);
                }
            }
            Ok(Some(out))
        }
        Kind::WebP => {
            let chunks = riff_chunks(bytes).ok_or(())?;
            let detail = |ty: &[u8; 4]| ty == b"EXIF" || ty == b"XMP ";
            if !chunks.iter().any(|c| detail(&c.0)) {
                return Ok(None);
            }
            let mut body = b"WEBP".to_vec();
            for &(ty, start, data, _, end) in &chunks {
                if detail(&ty) {
                    continue;
                }
                let at = body.len();
                body.extend_from_slice(&bytes[start..end]);
                // VP8X says which chunks follow: the EXIF and XMP flags go with them.
                if &ty == b"VP8X"
                    && let Some(flags) = body.get_mut(at + (data - start))
                {
                    *flags &= !(0x08 | 0x04);
                }
            }
            let size = u32::try_from(body.len()).map_err(|_| ())?;
            let mut out = b"RIFF".to_vec();
            out.extend_from_slice(&size.to_le_bytes());
            out.extend_from_slice(&body);
            Ok(Some(out))
        }
        Kind::Gif => {
            let (out, dropped) = gif_stripped(bytes).ok_or(())?;
            Ok(dropped.then_some(out))
        }
        Kind::Bmp => Ok(None),
    }
}

/// The EXIF orientation, 1 to 8; 1 (as stored) when there is none or it cannot be read.
fn orientation(bytes: &[u8], kind: Kind) -> u8 {
    let tiff = match kind {
        Kind::Jpeg => jpeg_segments(bytes).and_then(|(segments, _)| {
            segments.iter().find_map(|&(marker, _, data, end)| {
                let d = bytes.get(data..end)?;
                (marker == 0xE1 && d.starts_with(b"Exif\0\0")).then(|| &d[6..])
            })
        }),
        Kind::Png => png_chunks(bytes).and_then(|(chunks, _)| {
            chunks
                .iter()
                .find(|c| &c.0 == b"eXIf")
                .and_then(|c| bytes.get(c.2..c.3))
        }),
        Kind::WebP => riff_chunks(bytes).and_then(|chunks| {
            chunks.iter().find(|c| &c.0 == b"EXIF").and_then(|c| {
                let d = bytes.get(c.2..c.3)?;
                Some(d.strip_prefix(b"Exif\0\0").unwrap_or(d))
            })
        }),
        Kind::Gif | Kind::Bmp => None,
    };
    tiff.and_then(tiff_orientation).unwrap_or(1)
}

/// Tag 0x0112 of the first directory of a TIFF structure (EXIF's).
fn tiff_orientation(t: &[u8]) -> Option<u8> {
    let be = match t.get(..4)? {
        b"MM\0*" => true,
        b"II*\0" => false,
        _ => return None,
    };
    let u16_at = |at: usize| if be { u16_be(t, at) } else { u16_le(t, at) };
    let u32_at = |at: usize| if be { u32_be(t, at) } else { u32_le(t, at) };
    let ifd = u32_at(4)? as usize;
    let count = usize::from(u16_at(ifd)?);
    for k in 0..count.min(1024) {
        let entry = ifd.checked_add(2 + 12 * k)?;
        if u16_at(entry)? == 0x0112 {
            let v = u16_at(entry + 8)?;
            return (1..=8).contains(&v).then_some(v as u8);
        }
    }
    None
}

/// The picture as it is meant to be seen, by its orientation `turn` (EXIF's 1 to 8).
fn turned(image: &Image, turn: u8) -> Result<Image, Error> {
    if !(2..=8).contains(&turn) {
        return Ok(Image {
            width: image.width,
            height: image.height,
            channels: image.channels,
            data: image.data.clone(),
        });
    }
    let (w, h) = (image.width as usize, image.height as usize);
    let (ow, oh) = if turn >= 5 { (h, w) } else { (w, h) };
    let c = image.channels;
    let len = ow
        .checked_mul(oh)
        .and_then(|p| p.checked_mul(c))
        .ok_or(Error::Encode)?;
    let mut out = vec![0u8; len];
    for y in 0..oh {
        for x in 0..ow {
            let (sx, sy) = match turn {
                2 => (w - 1 - x, y),
                3 => (w - 1 - x, h - 1 - y),
                4 => (x, h - 1 - y),
                5 => (y, x),
                6 => (y, h - 1 - x),
                7 => (w - 1 - y, h - 1 - x),
                _ => (w - 1 - y, x),
            };
            let from = (sy * w + sx) * c;
            let to = (y * ow + x) * c;
            let src = image.data.get(from..from + c).ok_or(Error::Encode)?;
            if let Some(dst) = out.get_mut(to..to + c) {
                dst.copy_from_slice(src);
            }
        }
    }
    Ok(Image {
        width: ow as u32,
        height: oh as u32,
        channels: c,
        data: out,
    })
}

/// A decoded picture, row-major, `channels` bytes a pixel: 1 grey, 2 grey and alpha, 3 RGB, 4 RGBA.
struct Image {
    width: u32,
    height: u32,
    channels: usize,
    data: Vec<u8>,
}

impl Image {
    /// `width * height * channels`, or `None` when the data does not have exactly that length.
    fn new(width: u32, height: u32, channels: usize, data: Vec<u8>) -> Option<Self> {
        let want = (width as usize)
            .checked_mul(height as usize)
            .and_then(|p| p.checked_mul(channels))?;
        if data.len() != want {
            return None;
        }
        Some(Image {
            width,
            height,
            channels,
            data,
        })
    }
}

/// The pixels, with the size the header already gave checked against what the decoder produced.
fn decode(bytes: &[u8], kind: Kind, width: u32, height: u32) -> Result<Image, Error> {
    match kind {
        Kind::Jpeg => {
            let mut decoder = zune_jpeg::JpegDecoder::new(Cursor::new(bytes));
            // Headers first, so a file that lies about its size cannot make `decode` allocate.
            decoder.decode_headers().map_err(|_| Error::Corrupt(kind))?;
            let info = decoder.info().ok_or(Error::Corrupt(kind))?;
            if u32::from(info.width) != width || u32::from(info.height) != height {
                return Err(Error::Corrupt(kind));
            }
            let pixels = decoder.decode().map_err(|_| Error::Corrupt(kind))?;
            let pixels_u64 = u64::from(width) * u64::from(height);
            let channels = match pixels.len() as u64 {
                n if n == pixels_u64 => 1,
                n if n == pixels_u64.saturating_mul(3) => 3,
                n if n == pixels_u64.saturating_mul(4) => 4,
                _ => return Err(Error::Corrupt(kind)),
            };
            Image::new(width, height, channels, pixels).ok_or(Error::Corrupt(kind))
        }
        Kind::Png => {
            let mut decoder = png::Decoder::new(Cursor::new(bytes));
            // 8 bits a channel with any palette or transparency expanded, so there is one shape
            // per colour type below.
            decoder
                .set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
            let mut reader = decoder.read_info().map_err(|_| Error::Corrupt(kind))?;
            let info = reader.info();
            if info.width != width || info.height != height {
                return Err(Error::Corrupt(kind));
            }
            let size = reader.output_buffer_size().ok_or(Error::Corrupt(kind))?;
            let mut buffer = vec![0u8; size];
            let frame = reader
                .next_frame(&mut buffer)
                .map_err(|_| Error::Corrupt(kind))?;
            let channels = match frame.color_type {
                png::ColorType::Grayscale => 1,
                png::ColorType::GrayscaleAlpha => 2,
                png::ColorType::Rgb => 3,
                png::ColorType::Rgba => 4,
                // EXPAND above leaves nothing indexed, and an unknown type must not be guessed.
                png::ColorType::Indexed => return Err(Error::Corrupt(kind)),
            };
            // The frame's part of the buffer, without a copy of it (Fable's review of V1, 22).
            if frame.buffer_size() > buffer.len() {
                return Err(Error::Corrupt(kind));
            }
            buffer.truncate(frame.buffer_size());
            Image::new(width, height, channels, buffer).ok_or(Error::Corrupt(kind))
        }
        Kind::WebP => {
            let mut decoder = image_webp::WebPDecoder::new(Cursor::new(bytes))
                .map_err(|_| Error::Corrupt(kind))?;
            let (w, h) = decoder.dimensions();
            if w != width || h != height {
                return Err(Error::Corrupt(kind));
            }
            // The decoder's own guard, in case a frame tries to grow past the header.
            decoder.set_memory_limit(MAX_PIXELS as usize * 4);
            let alpha = decoder.has_alpha();
            let size = decoder.output_buffer_size().ok_or(Error::Corrupt(kind))?;
            let mut buffer = vec![0u8; size];
            decoder
                .read_image(&mut buffer)
                .map_err(|_| Error::Corrupt(kind))?;
            let channels = if alpha { 4 } else { 3 };
            Image::new(width, height, channels, buffer).ok_or(Error::Corrupt(kind))
        }
        Kind::Bmp => {
            let info = bmp_info(bytes).ok_or(Error::Corrupt(kind))?;
            if info.width != width || info.height != height {
                return Err(Error::Corrupt(kind));
            }
            let rgb = bmp_pixels(bytes, &info).ok_or(Error::Corrupt(kind))?;
            Image::new(width, height, 3, rgb).ok_or(Error::Corrupt(kind))
        }
        Kind::Gif => Err(Error::Unsupported),
    }
}

/// The picture with its long side at most `limit`, keeping its shape. An image already inside the
/// limit is returned as it is: the file was too big on disk, not on screen.
fn scaled_to(image: &Image, limit: u32) -> Result<Image, Error> {
    let (width, height) = (image.width, image.height);
    let long = width.max(height);
    if long <= limit {
        return Ok(Image {
            width,
            height,
            channels: image.channels,
            data: image.data.clone(),
        });
    }
    let (dw, dh) = if width >= height {
        let dh = (u64::from(height) * u64::from(limit) / u64::from(width)) as u32;
        (limit, dh.max(1))
    } else {
        let dw = (u64::from(width) * u64::from(limit) / u64::from(height)) as u32;
        (dw.max(1), limit)
    };
    downscale(image, dw, dh)
}

/// An area-average (box) scale to `dw` by `dh`: every output pixel is the mean of the source
/// pixels under it. Shrinking is what this is for, so it never upsamples.
fn downscale(image: &Image, dw: u32, dh: u32) -> Result<Image, Error> {
    let channels = image.channels;
    let (sw, sh) = (image.width, image.height);
    if dw == 0 || dh == 0 || dw > sw || dh > sh || channels == 0 {
        return Err(Error::Encode);
    }
    let src_row = (sw as usize).checked_mul(channels).ok_or(Error::Encode)?;
    let out_row = (dw as usize).checked_mul(channels).ok_or(Error::Encode)?;
    let out_len = out_row.checked_mul(dh as usize).ok_or(Error::Encode)?;
    let mut out = vec![0u8; out_len];
    // The source span of each output column, worked out once for the whole picture.
    let mut spans = Vec::with_capacity(dw as usize);
    for x in 0..dw {
        let x0 = (u64::from(x) * u64::from(sw) / u64::from(dw)) as usize;
        let x1 = ((u64::from(x) + 1) * u64::from(sw) / u64::from(dw)) as usize;
        let x1 = x1.min(sw as usize).max(x0 + 1);
        spans.push((x0, x1));
    }
    for y in 0..dh {
        let y0 = (u64::from(y) * u64::from(sh) / u64::from(dh)) as usize;
        let y1 = ((u64::from(y) + 1) * u64::from(sh) / u64::from(dh)) as usize;
        let y1 = y1.min(sh as usize).max(y0 + 1);
        let rows = (y1 - y0) as u64;
        for (x, &(x0, x1)) in spans.iter().enumerate() {
            let count = rows * (x1 - x0) as u64;
            let at = y as usize * out_row + x * channels;
            for c in 0..channels {
                let mut sum = 0u64;
                for sy in y0..y1 {
                    let start = sy * src_row + x0 * channels + c;
                    for sx in 0..(x1 - x0) {
                        if let Some(&v) = image.data.get(start + sx * channels) {
                            sum += u64::from(v);
                        }
                    }
                }
                // The two roundings (count + sum / count) keep the mean within one step; a mean
                // of 255 is 255, never 256.
                let mean = (sum + count / 2) / count;
                if let Some(slot) = out.get_mut(at + c) {
                    *slot = mean.min(255) as u8;
                }
            }
        }
    }
    Ok(Image {
        width: dw,
        height: dh,
        channels,
        data: out,
    })
}

/// Writes the picture in the format its kind calls for.
fn encode(image: &Image, kind: Kind) -> Result<Vec<u8>, Error> {
    match written_as(kind) {
        Kind::Jpeg => encode_jpeg(image),
        Kind::Png => encode_png(image),
        // Unreachable: `written_as` maps only these two.
        Kind::WebP | Kind::Gif | Kind::Bmp => Err(Error::Encode),
    }
}

fn encode_jpeg(image: &Image) -> Result<Vec<u8>, Error> {
    let width = u16::try_from(image.width).map_err(|_| Error::Encode)?;
    let height = u16::try_from(image.height).map_err(|_| Error::Encode)?;
    let (color, data) = match image.channels {
        1 => (jpeg_encoder::ColorType::Luma, image.data.as_slice()),
        2 => {
            // JPEG has no alpha: the grey channel alone.
            let grey: Vec<u8> = image
                .data
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pixel| pixel[0])
                .collect();
            let mut out = Vec::new();
            jpeg_encoder::Encoder::new(&mut out, JPEG_QUALITY)
                .encode(&grey, width, height, jpeg_encoder::ColorType::Luma)
                .map_err(|_| Error::Encode)?;
            return Ok(out);
        }
        3 => (jpeg_encoder::ColorType::Rgb, image.data.as_slice()),
        _ => (jpeg_encoder::ColorType::Rgba, image.data.as_slice()),
    };
    let mut out = Vec::new();
    jpeg_encoder::Encoder::new(&mut out, JPEG_QUALITY)
        .encode(data, width, height, color)
        .map_err(|_| Error::Encode)?;
    Ok(out)
}

fn encode_png(image: &Image) -> Result<Vec<u8>, Error> {
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, image.width, image.height);
        encoder.set_color(match image.channels {
            1 => png::ColorType::Grayscale,
            2 => png::ColorType::GrayscaleAlpha,
            3 => png::ColorType::Rgb,
            _ => png::ColorType::Rgba,
        });
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|_| Error::Encode)?;
        writer
            .write_image_data(&image.data)
            .map_err(|_| Error::Encode)?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The spike's call: the default options.
    fn resize(bytes: &[u8]) -> Result<Prepared, Error> {
        prepare(bytes, &Options::default())
    }

    // ---- the allocator, so the measurement test can report a peak without a crate ------------

    use std::alloc::{GlobalAlloc, Layout, System};
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Counting;

    static LIVE: AtomicUsize = AtomicUsize::new(0);
    static PEAK: AtomicUsize = AtomicUsize::new(0);

    // SAFETY: every allocation is handed to the system allocator unchanged and its size is only
    // counted here; nothing is moved, resized or freed by this type itself.
    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let ptr = unsafe { System.alloc(layout) };
            if !ptr.is_null() {
                let live = LIVE.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
                PEAK.fetch_max(live, Ordering::Relaxed);
            }
            ptr
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
            unsafe { System.dealloc(ptr, layout) }
        }
    }

    #[global_allocator]
    static ALLOCATOR: Counting = Counting;

    // ---- pictures made here, so no binary fixture has to be checked in ----------------------

    /// A picture whose bytes are not a flat colour, so a lossy encoder cannot make it tiny: the
    /// value at (x, y) is a cheap hash of the position. Incompressible, so a big one goes over
    /// the byte cap before it goes over the pixel cap.
    fn noise(width: u32, height: u32, channels: usize) -> Vec<u8> {
        let mut out = Vec::with_capacity(width as usize * height as usize * channels);
        let mut state = 0x2545_F491_4F6C_DD1Du64;
        for _ in 0..width as usize * height as usize {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let base = (state & 0xFF) as u8;
            for c in 0..channels {
                out.push(base.wrapping_add((c as u8).wrapping_mul(37)));
            }
        }
        out
    }

    /// A smooth picture (a diagonal gradient with a little structure), the shape a photo or a
    /// screenshot has: it compresses, so it can be over the long-side cap without being over the
    /// byte cap, which is the case a test of the long-side cap needs.
    fn picture(width: u32, height: u32, channels: usize) -> Vec<u8> {
        let mut out = Vec::with_capacity(width as usize * height as usize * channels);
        let span = u64::from(width) + u64::from(height);
        for y in 0..height {
            for x in 0..width {
                let r = (u64::from(x) * 255 / u64::from(width).max(1)) as u8;
                let g = (u64::from(y) * 255 / u64::from(height).max(1)) as u8;
                let b = ((u64::from(x) + u64::from(y)) * 255 / span.max(1)) as u8;
                match channels {
                    1 => out.push(r ^ g),
                    2 => out.extend([r ^ g, 200]),
                    3 => out.extend([r, g, b]),
                    _ => out.extend([r, g, b, 200]),
                }
            }
        }
        out
    }

    fn jpeg_from(data: Vec<u8>, width: u32, height: u32, quality: u8, channels: usize) -> Vec<u8> {
        let color = if channels == 1 {
            jpeg_encoder::ColorType::Luma
        } else {
            jpeg_encoder::ColorType::Rgb
        };
        let mut out = Vec::new();
        jpeg_encoder::Encoder::new(&mut out, quality)
            .encode(&data, width as u16, height as u16, color)
            .expect("test fixture encodes");
        out
    }

    fn jpeg(width: u32, height: u32, quality: u8, channels: usize) -> Vec<u8> {
        jpeg_from(
            picture(width, height, channels),
            width,
            height,
            quality,
            channels,
        )
    }

    fn jpeg_noise(width: u32, height: u32, quality: u8, channels: usize) -> Vec<u8> {
        jpeg_from(
            noise(width, height, channels),
            width,
            height,
            quality,
            channels,
        )
    }

    fn png_from(data: Vec<u8>, width: u32, height: u32, channels: usize) -> Vec<u8> {
        let image = Image {
            width,
            height,
            channels,
            data,
        };
        encode_png(&image).expect("test fixture encodes")
    }

    fn png(width: u32, height: u32, channels: usize) -> Vec<u8> {
        png_from(picture(width, height, channels), width, height, channels)
    }

    fn png_noise(width: u32, height: u32, channels: usize) -> Vec<u8> {
        png_from(noise(width, height, channels), width, height, channels)
    }

    /// A static WebP, through the decoder crate's own lossless encoder.
    fn webp(width: u32, height: u32, alpha: bool) -> Vec<u8> {
        let channels = if alpha { 4 } else { 3 };
        let data = picture(width, height, channels);
        let color = if alpha {
            image_webp::ColorType::Rgba8
        } else {
            image_webp::ColorType::Rgb8
        };
        let mut out = Vec::new();
        image_webp::WebPEncoder::new(&mut out)
            .encode(&data, width, height, color)
            .expect("test fixture encodes");
        out
    }

    fn gif() -> Vec<u8> {
        // One 2x2 frame, two colours, built by hand: `Frame::from_rgb` needs gif's quantiser,
        // which is a package this spike does not need in the lock.
        let mut out = Vec::new();
        {
            let mut encoder = gif::Encoder::new(&mut out, 2, 2, &[0, 0, 0, 255, 255, 255])
                .expect("test fixture encodes");
            let frame = gif::Frame {
                width: 2,
                height: 2,
                buffer: std::borrow::Cow::Owned(vec![0, 1, 1, 0]),
                ..gif::Frame::default()
            };
            encoder.write_frame(&frame).expect("test fixture encodes");
        }
        out
    }

    /// A one-frame animated WebP, hand-built: the still VP8L frame wrapped in a RIFF container
    /// with VP8X (animation flag), ANIM and ANMF chunks.
    fn animated_webp(width: u32, height: u32) -> Vec<u8> {
        fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
            out.extend_from_slice(kind);
            out.extend_from_slice(&(data.len() as u32).to_le_bytes());
            out.extend_from_slice(data);
            if data.len() % 2 == 1 {
                out.push(0);
            }
        }

        let still = webp(width, height, false);
        assert_eq!(&still[12..16], b"VP8L");
        let frame_chunk = &still[12..];
        let mut body = Vec::new();
        let mut vp8x = vec![0x02]; // the animation flag
        vp8x.extend_from_slice(&[0, 0, 0]);
        vp8x.extend_from_slice(&(width - 1).to_le_bytes()[..3]);
        vp8x.extend_from_slice(&(height - 1).to_le_bytes()[..3]);
        chunk(&mut body, b"VP8X", &vp8x);
        chunk(&mut body, b"ANIM", &[0, 0, 0, 0, 0, 0]);
        let mut anmf = Vec::new();
        anmf.extend_from_slice(&[0, 0, 0]); // x / 2
        anmf.extend_from_slice(&[0, 0, 0]); // y / 2
        anmf.extend_from_slice(&(width - 1).to_le_bytes()[..3]);
        anmf.extend_from_slice(&(height - 1).to_le_bytes()[..3]);
        anmf.extend_from_slice(&100u32.to_le_bytes()[..3]); // 100 ms
        anmf.push(0); // no disposal, blend
        anmf.extend_from_slice(frame_chunk);
        chunk(&mut body, b"ANMF", &anmf);
        let mut out = Vec::new();
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&((body.len() + 4) as u32).to_le_bytes());
        out.extend_from_slice(b"WEBP");
        out.extend_from_slice(&body);
        out
    }

    /// CRC-32 of a PNG chunk, written out here so a header can be made without a crate.
    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc = 0xFFFF_FFFFu32;
        for &byte in bytes {
            crc ^= u32::from(byte);
            for _ in 0..8 {
                crc = if crc & 1 != 0 {
                    (crc >> 1) ^ 0xEDB8_8320
                } else {
                    crc >> 1
                };
            }
        }
        !crc
    }

    fn png_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut body = kind.to_vec();
        body.extend_from_slice(data);
        out.extend_from_slice(&body);
        out.extend_from_slice(&crc32(&body).to_be_bytes());
    }

    /// A valid PNG that declares `width` by `height` and carries one empty IDAT: nothing is
    /// decoded, so the header alone is what a reader sees.
    fn png_claiming(width: u32, height: u32) -> Vec<u8> {
        let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&width.to_be_bytes());
        ihdr.extend_from_slice(&height.to_be_bytes());
        ihdr.extend_from_slice(&[8, 2, 0, 0, 0]); // 8 bits, RGB, deflate, no filter, no interlace
        png_chunk(&mut out, b"IHDR", &ihdr);
        png_chunk(&mut out, b"IDAT", &[]);
        png_chunk(&mut out, b"IEND", &[]);
        out
    }

    /// Decodes what the resizer wrote, the way a reader would.
    fn read_back(bytes: &[u8]) -> Image {
        let kind = kind(bytes).expect("a known format");
        let (w, h) = declared(bytes, kind).expect("a readable header");
        decode(bytes, kind, w, h).expect("a decodable picture")
    }

    // ---- pass-through -----------------------------------------------------------------------

    #[test]
    fn a_small_jpeg_is_sent_as_it_came() {
        let input = jpeg(64, 48, 90, 3);
        assert!(input.len() < MAX_BYTES);
        let out = resize(&input).expect("prepares");
        assert_eq!(out.done(), vec!["as it is"]);
        assert_eq!(out.kind.mime(), "image/jpeg");
        assert_eq!(out.bytes, input, "byte for byte, no second loss");
    }

    #[test]
    fn a_small_png_is_sent_as_it_came() {
        let input = png(40, 30, 3);
        let out = resize(&input).expect("prepares");
        assert_eq!(out.done(), vec!["as it is"]);
        assert_eq!(out.kind.mime(), "image/png");
        assert_eq!(out.bytes, input);
    }

    #[test]
    fn a_gif_is_passed_through_untouched() {
        let input = gif();
        let out = resize(&input).expect("prepares");
        assert_eq!(out.done(), vec!["as it is"]);
        assert_eq!(out.kind.mime(), "image/gif");
        assert_eq!(out.bytes, input);
    }

    // ---- shrinking ---------------------------------------------------------------------------

    #[test]
    fn a_jpeg_over_the_long_side_is_scaled_down_and_still_decodes() {
        let input = jpeg(2600, 1000, 90, 3);
        assert!(input.len() < MAX_BYTES, "the bytes fit; the shape does not");
        let out = resize(&input).expect("prepares");
        assert!(out.smaller);
        assert_eq!(out.kind.mime(), "image/jpeg");
        let image = read_back(&out.bytes);
        assert_eq!((image.width, image.height), (1568, 603));
        assert_eq!(image.channels, 3);
    }

    #[test]
    fn a_jpeg_over_the_byte_limit_comes_under_it() {
        let input = jpeg_noise(4000, 3000, 100, 3);
        assert!(input.len() > MAX_BYTES, "fixture is {} bytes", input.len());
        let out = resize(&input).expect("prepares");
        assert!(out.smaller);
        assert!(out.bytes.len() <= MAX_BYTES, "{} bytes", out.bytes.len());
        let image = read_back(&out.bytes);
        assert_eq!((image.width, image.height), (1568, 1176));
    }

    #[test]
    fn a_png_over_the_byte_limit_comes_under_it() {
        let input = png_noise(3000, 2000, 3);
        assert!(input.len() > MAX_BYTES, "fixture is {} bytes", input.len());
        let out = resize(&input).expect("prepares");
        assert!(out.smaller);
        assert!(out.bytes.len() <= MAX_BYTES, "{} bytes", out.bytes.len());
        let image = read_back(&out.bytes);
        // 1568 px long was still over the cap for an incompressible picture, so it is smaller.
        assert!(image.width < 1568, "{} px long", image.width);
        let expected = image.width as i64 * 2 / 3;
        assert!((image.height as i64 - expected).abs() <= 1);
    }

    #[test]
    fn a_png_stays_a_png_and_shrinks() {
        let input = png(2600, 1000, 3);
        let out = resize(&input).expect("prepares");
        assert!(out.smaller);
        assert_eq!(out.kind.mime(), "image/png");
        assert!(out.bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
        let image = read_back(&out.bytes);
        assert_eq!((image.width, image.height), (1568, 603));
    }

    #[test]
    fn an_rgba_png_keeps_its_alpha() {
        let input = png(2200, 100, 4);
        let out = resize(&input).expect("prepares");
        let image = read_back(&out.bytes);
        assert_eq!(image.channels, 4);
        assert_eq!((image.width, image.height), (1568, 71));
        // The fixture's alpha is a flat 200, and an average of equal values is that value.
        assert!(image.data.chunks(4).all(|pixel| pixel[3] == 200));
    }

    #[test]
    fn the_webp_encoder_in_the_lock_can_encode() {
        // `image-webp` 0.2.4 ships a WebPEncoder: lossless (VP8L) only, and its `encode` panics if
        // the data's length does not match the dimensions. It is not used by the resizer (WebP is
        // written as PNG), but the spike asks whether it can encode at all: it can.
        let input = webp(40, 30, true);
        let decoded = read_back(&input);
        assert_eq!(
            (decoded.width, decoded.height, decoded.channels),
            (40, 30, 4)
        );
        assert_eq!(decoded.data, picture(40, 30, 4));
    }

    #[test]
    fn an_animated_webp_is_passed_through_untouched() {
        let input = animated_webp(2600, 1000);
        assert!(webp_is_animated(&input), "fixture is not animated");
        let out = resize(&input).expect("prepares");
        assert_eq!(out.done(), vec!["as it is"]);
        assert_eq!(out.kind.mime(), "image/webp");
        assert_eq!(
            out.bytes, input,
            "an animation is not flattened to one frame"
        );
    }

    #[test]
    fn a_still_webp_is_not_mistaken_for_an_animation() {
        assert!(!webp_is_animated(&webp(40, 30, false)));
    }

    #[test]
    fn a_jpeg_is_written_as_a_jpeg() {
        // A JPEG source goes through the JPEG writer, not the PNG one.
        let input = jpeg(2200, 100, 90, 3);
        let out = resize(&input).expect("prepares");
        assert!(out.bytes.starts_with(&[0xFF, 0xD8, 0xFF]));
        assert_eq!(out.kind.mime(), "image/jpeg");
    }

    #[test]
    fn a_webp_is_written_as_a_png() {
        let input = webp(2600, 1000, false);
        assert_eq!(kind(&input), Some(Kind::WebP));
        let out = resize(&input).expect("prepares");
        assert!(out.smaller);
        assert_eq!(out.kind.mime(), "image/png");
        assert!(out.bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
        let image = read_back(&out.bytes);
        assert_eq!((image.width, image.height), (1568, 603));
    }

    // ---- refusals ----------------------------------------------------------------------------

    #[test]
    fn a_truncated_png_is_an_error() {
        let full = png(2200, 100, 3);
        let half = &full[..full.len() / 2];
        assert_eq!(resize(half), Err(Error::Corrupt(Kind::Png)));
    }

    #[test]
    fn a_truncated_jpeg_is_an_error() {
        // Cut inside the headers. A JPEG whose scan data alone is cut is not an error: zune-jpeg
        // fills the missing blocks with grey and decodes the rest, as mdedit's preview does.
        let full = jpeg(400, 300, 90, 3);
        let head = &full[..40];
        assert_eq!(resize(head), Err(Error::Corrupt(Kind::Jpeg)));
    }

    #[test]
    fn bytes_that_are_not_a_picture_are_refused() {
        assert_eq!(
            resize(b"hello, this is a text file"),
            Err(Error::Unsupported)
        );
        assert_eq!(resize(&[]), Err(Error::Unsupported));
    }

    #[test]
    fn a_png_that_claims_a_hundred_thousand_pixels_each_way_is_refused() {
        assert_eq!(
            resize(&png_claiming(100_000, 100_000)),
            Err(Error::TooLarge {
                width: 100_000,
                height: 100_000
            })
        );
    }

    #[test]
    fn no_pixels_at_all_or_more_than_the_cap_is_refused_before_decoding() {
        assert_eq!(
            fits(0, 10),
            Err(Error::TooLarge {
                width: 0,
                height: 10
            })
        );
        assert_eq!(
            fits(100_000, 100_000),
            Err(Error::TooLarge {
                width: 100_000,
                height: 100_000
            })
        );
        // Just under the cap, and the cap itself, are allowed.
        assert_eq!(fits(8000, 8000), Ok(()));
    }

    // ---- the scaling itself ------------------------------------------------------------------

    #[test]
    fn a_box_scale_averages_the_pixels_under_it() {
        let image = Image {
            width: 2,
            height: 2,
            channels: 1,
            data: vec![0, 100, 200, 255],
        };
        let out = downscale(&image, 1, 1).expect("scales");
        assert_eq!((out.width, out.height), (1, 1));
        // 555 / 4 = 138.75, rounded to the nearest byte.
        assert_eq!(out.data, vec![139]);
    }

    #[test]
    fn a_box_scale_keeps_every_channel_apart() {
        let image = Image {
            width: 2,
            height: 1,
            channels: 3,
            data: vec![
                10, 20, 30, //
                30, 40, 50,
            ],
        };
        let out = downscale(&image, 1, 1).expect("scales");
        assert_eq!(out.data, vec![20, 30, 40]);
    }

    #[test]
    fn a_scale_that_does_not_divide_evenly_covers_every_row() {
        // 5 rows to 2: the first output row covers 2 source rows, the second 3.
        let image = Image {
            width: 1,
            height: 5,
            channels: 1,
            data: vec![10, 20, 30, 40, 50],
        };
        let out = downscale(&image, 1, 2).expect("scales");
        assert_eq!(out.data, vec![(10 + 20) / 2, (30 + 40 + 50) / 3]);
    }

    #[test]
    fn scaling_never_enlarges() {
        let image = Image {
            width: 10,
            height: 10,
            channels: 1,
            data: vec![7; 100],
        };
        let out = scaled_to(&image, LONG_SIDE).expect("keeps the size");
        assert_eq!((out.width, out.height), (10, 10));
        assert_eq!(out.data, image.data);
    }

    // ---- phase V1: the limits, orientation, details, BMP, refusals, thumbnails ---------------

    /// A TIFF structure as EXIF holds it: one directory with the orientation, then `extra`.
    fn exif_tiff(orientation: u16, extra: &[u8]) -> Vec<u8> {
        let mut t = b"MM\0*".to_vec();
        t.extend(8u32.to_be_bytes());
        t.extend(1u16.to_be_bytes());
        t.extend(0x0112u16.to_be_bytes());
        t.extend(3u16.to_be_bytes());
        t.extend(1u32.to_be_bytes());
        t.extend(orientation.to_be_bytes());
        t.extend([0, 0]);
        t.extend(0u32.to_be_bytes());
        t.extend_from_slice(extra);
        t
    }

    /// `jpeg` with a segment of `marker` holding `data` right after its start.
    fn with_segment(jpeg: &[u8], marker: u8, data: &[u8]) -> Vec<u8> {
        let mut out = jpeg[..2].to_vec();
        out.extend([0xFF, marker]);
        out.extend(((data.len() + 2) as u16).to_be_bytes());
        out.extend_from_slice(data);
        out.extend_from_slice(&jpeg[2..]);
        out
    }

    fn app1_exif(orientation: u16, extra: &[u8]) -> Vec<u8> {
        let mut d = b"Exif\0\0".to_vec();
        d.extend(exif_tiff(orientation, extra));
        d
    }

    const WHERE: &[u8] = b"GPS 41.0082N 28.9784E Pixel 9";

    fn contains(hay: &[u8], needle: &[u8]) -> bool {
        hay.windows(needle.len()).any(|w| w == needle)
    }

    /// Left half black, right half white.
    fn halves(width: u32, height: u32) -> Vec<u8> {
        let mut out = Vec::new();
        for _ in 0..height {
            for x in 0..width {
                let v = if x < width / 2 { 0 } else { 255 };
                out.extend([v, v, v]);
            }
        }
        out
    }

    fn grey_at(image: &Image, x: u32, y: u32) -> u8 {
        image.data[((y * image.width + x) as usize) * image.channels]
    }

    #[test]
    fn the_long_side_is_1568_and_2576_when_asked() {
        let input = jpeg(2600, 1000, 90, 3);
        let image = read_back(&resize(&input).unwrap().bytes);
        assert_eq!((image.width, image.height), (1568, 603));
        let large = prepare(
            &input,
            &Options {
                long_side: LONG_SIDE_LARGE,
                strip: true,
            },
        )
        .unwrap();
        let image = read_back(&large.bytes);
        assert_eq!((image.width, image.height), (2576, 990));
        assert_eq!((large.original_width, large.original_height), (2600, 1000));
    }

    #[test]
    fn a_sideways_jpeg_comes_out_upright() {
        // Stored 64 wide and 32 high, meant to be turned a quarter clockwise: its left half is
        // the top half of the picture as seen.
        let stored = jpeg_from(halves(64, 32), 64, 32, 95, 3);
        let input = with_segment(&stored, 0xE1, &app1_exif(6, &[]));
        assert_eq!(orientation(&input, Kind::Jpeg), 6);
        let out = resize(&input).unwrap();
        assert!(out.turned && !out.smaller);
        assert_eq!(out.done(), vec!["turned upright", "details removed"]);
        let image = read_back(&out.bytes);
        assert_eq!((image.width, image.height), (32, 64));
        assert_eq!((out.width, out.height), (32, 64));
        assert_eq!((out.original_width, out.original_height), (32, 64));
        assert!(grey_at(&image, 16, 8) < 40, "the top is the stored left");
        assert!(
            grey_at(&image, 16, 56) > 215,
            "the bottom is the stored right"
        );
    }

    #[test]
    fn every_orientation_turns_the_pixels_its_way() {
        // 3 x 2, one grey value a pixel:  1 2 3 / 4 5 6.
        let image = Image::new(3, 2, 1, vec![1, 2, 3, 4, 5, 6]).unwrap();
        let t = |turn| {
            let o = turned(&image, turn).unwrap();
            (o.width, o.height, o.data)
        };
        assert_eq!(t(1), (3, 2, vec![1, 2, 3, 4, 5, 6]));
        assert_eq!(t(2), (3, 2, vec![3, 2, 1, 6, 5, 4]));
        assert_eq!(t(3), (3, 2, vec![6, 5, 4, 3, 2, 1]));
        assert_eq!(t(4), (3, 2, vec![4, 5, 6, 1, 2, 3]));
        assert_eq!(t(5), (2, 3, vec![1, 4, 2, 5, 3, 6]));
        assert_eq!(t(6), (2, 3, vec![4, 1, 5, 2, 6, 3]));
        assert_eq!(t(7), (2, 3, vec![6, 3, 5, 2, 4, 1]));
        assert_eq!(t(8), (2, 3, vec![3, 6, 2, 5, 1, 4]));
    }

    #[test]
    fn a_jpeg_s_location_is_gone_from_what_is_sent_and_its_pixels_are_unchanged() {
        let plain = jpeg(80, 60, 90, 3);
        let tagged = with_segment(&plain, 0xE1, &app1_exif(1, WHERE));
        let tagged = with_segment(&tagged, 0xFE, b"taken at home");
        let out = resize(&tagged).unwrap();
        assert_eq!(out.done(), vec!["details removed"]);
        assert!(!contains(&out.bytes, WHERE));
        assert!(!contains(&out.bytes, b"taken at home"));
        // Not decoded and written again: the same bytes as without the details.
        assert_eq!(out.bytes, plain);
        // Asked to keep them, it goes as it came.
        let kept = prepare(
            &tagged,
            &Options {
                long_side: LONG_SIDE,
                strip: false,
            },
        )
        .unwrap();
        assert_eq!(kept.bytes, tagged);
        assert_eq!(kept.done(), vec!["as it is"]);
    }

    #[test]
    fn a_png_s_text_and_exif_are_gone_and_its_pixels_are_unchanged() {
        let plain = png(40, 30, 3);
        let mut tagged = plain[..33].to_vec();
        png_chunk(&mut tagged, b"tEXt", b"Location\0Istanbul 41.0082N");
        png_chunk(&mut tagged, b"eXIf", &exif_tiff(1, WHERE));
        tagged.extend_from_slice(&plain[33..]);
        let out = resize(&tagged).unwrap();
        assert!(out.stripped);
        assert!(!contains(&out.bytes, b"Istanbul") && !contains(&out.bytes, WHERE));
        assert_eq!(out.bytes, plain);
        assert_eq!(read_back(&out.bytes).data, read_back(&tagged).data);
    }

    #[test]
    fn a_webp_s_exif_is_gone_and_its_flag_with_it() {
        fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
            out.extend_from_slice(kind);
            out.extend_from_slice(&(data.len() as u32).to_le_bytes());
            out.extend_from_slice(data);
            if data.len() % 2 == 1 {
                out.push(0);
            }
        }
        let still = webp(20, 10, false);
        let mut body = Vec::new();
        let mut vp8x = vec![0x08, 0, 0, 0];
        vp8x.extend_from_slice(&19u32.to_le_bytes()[..3]);
        vp8x.extend_from_slice(&9u32.to_le_bytes()[..3]);
        chunk(&mut body, b"VP8X", &vp8x);
        body.extend_from_slice(&still[12..]);
        chunk(&mut body, b"EXIF", &exif_tiff(1, WHERE));
        let mut tagged = b"RIFF".to_vec();
        tagged.extend(((body.len() + 4) as u32).to_le_bytes());
        tagged.extend_from_slice(b"WEBP");
        tagged.extend_from_slice(&body);
        let before = read_back(&tagged);
        let out = resize(&tagged).unwrap();
        assert_eq!(out.done(), vec!["details removed"]);
        assert_eq!(out.kind, Kind::WebP);
        assert!(!contains(&out.bytes, WHERE) && !contains(&out.bytes, b"EXIF"));
        assert_eq!(out.bytes[20] & 0x08, 0, "the EXIF flag is cleared");
        assert_eq!(read_back(&out.bytes).data, before.data);
    }

    /// A 24-bit BMP file, bottom-up, of `rgb` pixels.
    fn bmp(width: u32, height: u32, rgb: &[u8]) -> Vec<u8> {
        let stride = (width as usize * 3).div_ceil(4) * 4;
        let mut out = b"BM".to_vec();
        out.extend(((54 + stride * height as usize) as u32).to_le_bytes());
        out.extend(0u32.to_le_bytes());
        out.extend(54u32.to_le_bytes());
        out.extend(40u32.to_le_bytes());
        out.extend((width as i32).to_le_bytes());
        out.extend((height as i32).to_le_bytes());
        out.extend(1u16.to_le_bytes());
        out.extend(24u16.to_le_bytes());
        out.extend([0; 24]);
        for y in (0..height as usize).rev() {
            let row = &rgb[y * width as usize * 3..(y + 1) * width as usize * 3];
            for px in row.chunks(3) {
                out.extend([px[2], px[1], px[0]]);
            }
            out.extend(vec![0; stride - width as usize * 3]);
        }
        out
    }

    #[test]
    fn a_bmp_goes_as_a_png() {
        let rgb = picture(5, 3, 3);
        let input = bmp(5, 3, &rgb);
        assert_eq!(kind(&input), Some(Kind::Bmp));
        let out = resize(&input).unwrap();
        assert_eq!(out.kind, Kind::Png);
        assert_eq!(out.done(), vec!["made a PNG"]);
        let image = read_back(&out.bytes);
        assert_eq!((image.width, image.height, image.channels), (5, 3, 3));
        assert_eq!(image.data, rgb);
        // A BMP of a kind not read is refused, not guessed at.
        let mut eight = input.clone();
        eight[28] = 8;
        assert_eq!(resize(&eight), Err(Error::Corrupt(Kind::Bmp)));
        assert_eq!(resize(&input[..60]), Err(Error::Corrupt(Kind::Bmp)));
    }

    #[test]
    fn heic_avif_tiff_and_svg_are_refused_with_their_names() {
        let mut heic = vec![0, 0, 0, 24];
        heic.extend_from_slice(b"ftypheic\0\0\0\0mif1heic");
        let mut avif = vec![0, 0, 0, 24];
        avif.extend_from_slice(b"ftypavif\0\0\0\0");
        assert_eq!(resize(&heic), Err(Error::NotRead("HEIC")));
        assert_eq!(resize(&avif), Err(Error::NotRead("AVIF")));
        assert_eq!(resize(b"II*\0\x08\0\0\0"), Err(Error::NotRead("TIFF")));
        let svg = "\u{feff}<?xml version=\"1.0\"?>\n<svg xmlns=\"http://www.w3.org/2000/svg\"/>";
        assert_eq!(resize(svg.as_bytes()), Err(Error::NotRead("SVG")));
        assert_eq!(
            Error::NotRead("HEIC").to_string(),
            "HEIC: not a picture this program can read. Save it as JPEG first."
        );
    }

    #[test]
    fn an_animation_over_the_byte_limit_is_refused() {
        // Over the limit inside the picture: a plain text block (which is the picture, and
        // stays) of MAX_BYTES before the trailer. Bytes after the trailer are not sent at all.
        let mut big = gif();
        big.pop();
        big.extend_from_slice(&[0x21, 0x01]);
        for _ in 0..MAX_BYTES / 255 + 1 {
            big.push(255);
            big.extend(std::iter::repeat_n(b'x', 255));
        }
        big.extend_from_slice(&[0, 0x3B]);
        assert_eq!(resize(&big), Err(Error::Animation));
        let mut screen = gif();
        screen[6..10].copy_from_slice(&[0, 0, 5, 0]);
        assert_eq!(
            resize(&screen),
            Err(Error::TooLarge {
                width: 0,
                height: 5
            })
        );
    }

    #[test]
    fn a_thumbnail_is_a_small_png_of_what_is_sent() {
        let out = resize(&jpeg(2600, 1000, 90, 3)).unwrap();
        let thumb = thumbnail(&out).unwrap();
        let image = read_back(&thumb);
        assert_eq!((image.width, image.height), (160, 61));
        let small = resize(&png(40, 30, 4)).unwrap();
        let image = read_back(&thumbnail(&small).unwrap());
        assert_eq!((image.width, image.height, image.channels), (40, 30, 4));
        assert_eq!(thumbnail(&resize(&gif()).unwrap()), None);
    }

    #[test]
    fn no_cut_or_broken_picture_panics() {
        let jpeg = with_segment(
            &with_segment(&jpeg(30, 20, 90, 3), 0xE1, &app1_exif(6, WHERE)),
            0xFE,
            b"c",
        );
        let mut png_text = png(20, 10, 3)[..33].to_vec();
        png_chunk(&mut png_text, b"eXIf", &exif_tiff(3, WHERE));
        png_text.extend_from_slice(&png(20, 10, 3)[33..]);
        let inputs = [
            jpeg,
            png_text,
            webp(20, 10, true),
            bmp(4, 4, &picture(4, 4, 3)),
            gif(),
            animated_webp(20, 10),
        ];
        for input in &inputs {
            for cut in 0..input.len().min(400) {
                let _ = resize(&input[..cut]);
            }
            let mut flipped = input.clone();
            for i in (0..flipped.len()).step_by(7) {
                flipped[i] ^= 0x5A;
                let _ = resize(&flipped);
                let _ = orientation(&flipped, Kind::Jpeg);
                let _ = strip_details(&flipped, Kind::Png);
                let _ = strip_details(&flipped, Kind::WebP);
            }
        }
        // A directory that claims more entries than it holds, and offsets past the end.
        let mut t = exif_tiff(6, &[]);
        t[8] = 0xFF;
        t[10] = 0;
        assert_eq!(tiff_orientation(&t), None);
        assert_eq!(tiff_orientation(b"MM\0*\xFF\xFF\xFF\xFF"), None);
    }

    // ---- measurement (run with: cargo test --release -p mdedit-ai -- --ignored --nocapture) --

    /// What a screenshot really looks like: mostly one colour with text-like bars, so it
    /// compresses far under the byte cap and never reaches the resizer at all.
    fn bars(width: u32, height: u32) -> Vec<u8> {
        let mut out = Vec::with_capacity(width as usize * height as usize * 3);
        for y in 0..height {
            let in_line = (y % 24) < 14;
            for x in 0..width {
                let dark = in_line && (16..width.saturating_sub(16)).contains(&x);
                out.extend(if dark { [32, 32, 32] } else { [250, 250, 250] });
            }
        }
        out
    }

    #[test]
    #[ignore = "measurement, not a check: run with --ignored"]
    fn measure_the_shrink() {
        use std::time::Instant;
        // A phone photo is 3 to 8 MB: noise at quality 75 comes out about that, which is what the
        // brief asks to be timed.
        let cases = [
            ("photo 4000x3000 JPEG", jpeg_noise(4000, 3000, 75, 3)),
            ("screenshot 3000x2000 PNG", png_noise(3000, 2000, 3)),
            ("photo 4000x3000 PNG", png_noise(4000, 3000, 3)),
            (
                "screenshot 3000x2000 PNG, flat",
                png_from(bars(3000, 2000), 3000, 2000, 3),
            ),
        ];
        for (name, input) in cases {
            let base = LIVE.load(Ordering::Relaxed);
            PEAK.store(base, Ordering::Relaxed);
            let started = Instant::now();
            let out = resize(&input).expect("prepares");
            let took = started.elapsed();
            let peak = PEAK.load(Ordering::Relaxed);
            let back = read_back(&out.bytes);
            println!(
                "{name}: {} bytes -> {} bytes as {}, {}x{}, {} ms, peak {} MB above the input",
                input.len(),
                out.bytes.len(),
                out.kind.mime(),
                back.width,
                back.height,
                took.as_millis(),
                (peak.saturating_sub(base)) / (1024 * 1024)
            );
        }
        // The decoder crate can encode, losslessly: what that costs next to a PNG, on the same
        // flat screenshot and on an incompressible picture.
        for (name, data, width, height) in [
            ("flat 3000x2000", bars(3000, 2000), 3000, 2000),
            ("noise 1000x667", noise(1000, 667, 3), 1000, 667),
        ] {
            let started = Instant::now();
            let mut lossless = Vec::new();
            image_webp::WebPEncoder::new(&mut lossless)
                .encode(&data, width, height, image_webp::ColorType::Rgb8)
                .expect("encodes");
            println!(
                "{name} as lossless WebP: {} bytes, {} ms (the PNG of it: {} bytes)",
                lossless.len(),
                started.elapsed().as_millis(),
                encode_png(&Image {
                    width,
                    height,
                    channels: 3,
                    data: data.clone(),
                })
                .expect("encodes")
                .len()
            );
        }
    }

    /// Fable's review of V1, 13 and 14: a JPEG goes without what follows its end (a motion
    /// photo's video, a second picture) and without any APPn but JFIF, the colour profile and
    /// Adobe's; the colour profile stays.
    #[test]
    fn a_jpeg_keeps_only_what_shows_it() {
        let plain = jpeg(80, 60, 90, 3);
        let icc = {
            let mut d = b"ICC_PROFILE\0".to_vec();
            d.extend_from_slice(&[1, 1, 9, 9, 9, 9]);
            d
        };
        let with_icc = with_segment(&plain, 0xE2, &icc);
        let mut tagged = with_segment(&with_icc, 0xEB, b"c2pa credentials of the maker");
        tagged = with_segment(&tagged, 0xE2, b"MPF\0second picture index");
        tagged.extend_from_slice(b"motion photo video with GPS 41.0082N");
        let out = resize(&tagged).unwrap();
        assert_eq!(out.done(), vec!["details removed"]);
        assert!(!contains(&out.bytes, b"c2pa"));
        assert!(!contains(&out.bytes, b"MPF"));
        assert!(!contains(&out.bytes, b"motion photo"));
        assert!(contains(&out.bytes, b"ICC_PROFILE"));
        assert_eq!(out.bytes, with_icc);
        // A JPEG with nothing after its end and no detail goes as it came.
        assert_eq!(resize(&with_icc).unwrap().done(), vec!["as it is"]);
    }

    /// Fable's review of V1, 14: a PNG chunk never seen before goes; what says how to show the
    /// picture stays.
    #[test]
    fn a_png_keeps_only_what_shows_it() {
        let plain = png(40, 30, 3);
        let mut shown = plain[..33].to_vec();
        png_chunk(&mut shown, b"pHYs", &[0, 0, 11, 19, 0, 0, 11, 19, 1]);
        shown.extend_from_slice(&plain[33..]);
        let mut tagged = shown[..33].to_vec();
        png_chunk(&mut tagged, b"caBX", b"c2pa credentials of the maker");
        tagged.extend_from_slice(&shown[33..]);
        let out = resize(&tagged).unwrap();
        assert!(out.stripped);
        assert!(!contains(&out.bytes, b"c2pa"));
        assert_eq!(out.bytes, shown);
    }

    /// A GIF with a comment, an application block and a loop count.
    fn gif_tagged() -> (Vec<u8>, Vec<u8>) {
        let plain = gif();
        // After the screen and its two-colour table: 13 + 6 bytes.
        let at = 19;
        let looping = [
            0x21, 0xFF, 11, b'N', b'E', b'T', b'S', b'C', b'A', b'P', b'E', b'2', b'.', b'0', 3, 1,
            0, 0, 0,
        ];
        let mut kept = plain[..at].to_vec();
        kept.extend_from_slice(&looping);
        kept.extend_from_slice(&plain[at..]);
        let mut tagged = plain[..at].to_vec();
        tagged.extend_from_slice(&[0x21, 0xFE, 13]);
        tagged.extend_from_slice(b"taken at home");
        tagged.push(0);
        tagged.extend_from_slice(&looping);
        tagged.extend_from_slice(&[0x21, 0xFF, 11]);
        tagged.extend_from_slice(b"XMP DataXMP");
        tagged.push(9);
        tagged.extend_from_slice(b"GPS 41.0N");
        tagged.push(0);
        tagged.extend_from_slice(&plain[at..]);
        tagged.extend_from_slice(b"after the end");
        (tagged, kept)
    }

    /// Fable's review of V1, 15: a GIF goes without its comments and application blocks (its
    /// loop count stays) and without what follows its end; asked to keep them, as it came.
    #[test]
    fn a_gif_s_details_are_gone_and_its_loop_count_stays() {
        let (tagged, kept) = gif_tagged();
        let out = resize(&tagged).unwrap();
        assert_eq!(out.done(), vec!["details removed"]);
        assert!(!contains(&out.bytes, b"taken at home"));
        assert!(!contains(&out.bytes, b"GPS"));
        assert!(!contains(&out.bytes, b"after the end"));
        assert_eq!(out.bytes, kept);
        let as_came = prepare(
            &tagged,
            &Options {
                long_side: LONG_SIDE,
                strip: false,
            },
        )
        .unwrap();
        assert_eq!(as_came.bytes, tagged);
        // One whose blocks cannot be walked is refused while its details are to go.
        let mut broken = tagged.clone();
        let cut = broken.len() - 30;
        broken.truncate(cut);
        assert_eq!(resize(&broken), Err(Error::DetailsKept(Kind::Gif)));
    }

    /// Fable's review of V1, 23: an animated PNG is not flattened to its first frame: it goes
    /// whole, like a GIF, its details taken out.
    #[test]
    fn an_animated_png_goes_whole() {
        let plain = png(2000, 10, 3);
        let mut apng = plain[..33].to_vec();
        png_chunk(&mut apng, b"acTL", &[0, 0, 0, 1, 0, 0, 0, 0]);
        png_chunk(&mut apng, b"tEXt", b"Location\0Istanbul");
        apng.extend_from_slice(&plain[33..]);
        let out = resize(&apng).unwrap();
        assert_eq!((out.width, out.height), (2000, 10), "not scaled");
        assert!(contains(&out.bytes, b"acTL"));
        assert!(!contains(&out.bytes, b"Istanbul"));
        assert_eq!(out.done(), vec!["details removed"]);
    }
}
