//! Shrinking a picture so a model can be sent it (phase V of docs/ai-library-plan.md, spike).
//!
//! A phone photo is 3 to 8 MB, and the providers refuse a picture over about 3.5 MB. This module
//! takes the bytes of a JPEG, PNG, WebP or GIF and, when the file is over [`MAX_BYTES`] or the
//! long side is over [`MAX_LONG_SIDE`] pixels, decodes it, scales it down with an area average and
//! writes it again: a JPEG as a JPEG (quality 85), a PNG as a PNG, a WebP as a PNG (the decoder
//! crate's own encoder is lossless-only, which is the wrong shape for a photo). A GIF is passed
//! through untouched: an animation cannot be scaled by keeping one frame, and a scaled GIF is a
//! different picture from the one the owner picked. Anything else is refused with a plain error.
//!
//! Every byte here comes from a file the owner picked and nothing has checked it, so nothing may
//! panic (CLAUDE.md rule 11: the release build aborts on a panic). Dimensions are read from the
//! header and checked before a decoder allocates, the decoded pixel count is capped, and every
//! slice, length and index below is either computed from a checked size or validated first.
//!
//! Not wired to a request yet: the protocol (`proto::Run` gaining `images`) is phase V's own work.

use std::fmt;
use std::io::Cursor;

/// The largest file a provider takes. 3.5 MB decimal, the plan's number.
pub const MAX_BYTES: usize = 3_500_000;
/// The longest side a picture is scaled to.
pub const MAX_LONG_SIDE: u32 = 2000;
/// The most pixels a file may claim before it is refused: 64 megapixels, 256 MB as RGBA.
pub const MAX_PIXELS: u64 = 64_000_000;
/// The quality a JPEG is written at.
pub const JPEG_QUALITY: u8 = 85;
/// Attempts at getting under [`MAX_BYTES`] when the long-side cap alone did not do it.
const MAX_ATTEMPTS: u8 = 4;

/// What a file's first bytes say it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Jpeg,
    Png,
    WebP,
    Gif,
}

impl Kind {
    /// The type sent with the picture, chosen by what will actually be sent.
    pub fn mime(self) -> &'static str {
        match self {
            Kind::Jpeg => "image/jpeg",
            Kind::Png => "image/png",
            Kind::WebP => "image/webp",
            Kind::Gif => "image/gif",
        }
    }
}

/// A picture ready to be sent.
#[derive(Debug, PartialEq, Eq)]
pub struct Resized {
    /// The bytes to send: the input itself when `resized` is false.
    pub bytes: Vec<u8>,
    /// The type of `bytes`.
    pub mime: &'static str,
    /// Whether anything was changed. A picture that already fits is sent as it came, byte for byte,
    /// so no loss is added to it.
    pub resized: bool,
}

/// Why a picture was not resized.
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    /// Not a format this build reads.
    Unsupported,
    /// The header says more pixels than [`MAX_PIXELS`], or none at all.
    TooLarge { width: u32, height: u32 },
    /// The decoder would not read it: truncated, corrupt, or not what its first bytes claimed.
    Corrupt(Kind),
    /// The encoder failed.
    Encode,
    /// Scanning got the picture under the long-side cap but not under [`MAX_BYTES`].
    StillTooBig,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Unsupported => f.write_str("not a picture this build reads"),
            Error::TooLarge { width, height } => {
                write!(f, "a picture of {width}x{height} pixels is too large")
            }
            Error::Corrupt(kind) => write!(f, "the {} could not be read", kind_name(*kind)),
            Error::Encode => f.write_str("the picture could not be written"),
            Error::StillTooBig => f.write_str("the picture is still too large after shrinking"),
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
    }
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
    } else {
        None
    }
}

/// `bytes` made ready to send: unchanged when it already fits, scaled down and written again when
/// it does not.
pub fn resize(bytes: &[u8]) -> Result<Resized, Error> {
    let kind = kind(bytes).ok_or(Error::Unsupported)?;
    if kind == Kind::Gif {
        return Ok(Resized {
            bytes: bytes.to_vec(),
            mime: Kind::Gif.mime(),
            resized: false,
        });
    }
    let (width, height) = declared(bytes, kind)?;
    fits(width, height)?;
    // An animation cannot be scaled while keeping what it is: one frame is not the picture. Like a
    // GIF, it is sent as it came, and the file-size cap does not apply to it.
    if kind == Kind::WebP && webp_is_animated(bytes) {
        return Ok(Resized {
            bytes: bytes.to_vec(),
            mime: Kind::WebP.mime(),
            resized: false,
        });
    }
    if bytes.len() <= MAX_BYTES && width.max(height) <= MAX_LONG_SIDE {
        return Ok(Resized {
            bytes: bytes.to_vec(),
            mime: kind.mime(),
            resized: false,
        });
    }
    let mut image = decode(bytes, kind, width, height)?;
    let mut limit = MAX_LONG_SIDE;
    for _ in 0..MAX_ATTEMPTS {
        image = scaled_to(&image, limit)?;
        let out = encode(&image, kind)?;
        if out.len() <= MAX_BYTES {
            return Ok(Resized {
                bytes: out,
                mime: written_as(kind).mime(),
                resized: true,
            });
        }
        // The long-side cap was not enough for this picture (a screenshot at 2000 px is mostly
        // flat, so this is rare): try again at the size that would bring the file under the cap,
        // a little smaller for the next write's size.
        let next = (limit as f64 * (MAX_BYTES as f64 / out.len() as f64).sqrt() * 0.95) as u32;
        if next == 0 || next >= limit {
            break;
        }
        limit = next;
    }
    Err(Error::StillTooBig)
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
        Kind::Png => Kind::Png,
        // The WebP decoder's own encoder is lossless (VP8L) only, which would be several times
        // the size of a lossy source for a photo; PNG is the format both the API paths accept and
        // every reader opens.
        Kind::WebP => Kind::Png,
        Kind::Gif => Kind::Gif,
    }
}

/// No pixels at all, or more than [`MAX_PIXELS`].
fn fits(width: u32, height: u32) -> Result<(), Error> {
    if width == 0 || height == 0 {
        return Err(Error::TooLarge { width, height });
    }
    if u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err(Error::TooLarge { width, height });
    }
    Ok(())
}

/// The size the header claims, before anything is decoded. A JPEG's is 16-bit; PNG, WebP and GIF
/// carry 16- or 32-bit fields and are where a hostile size shows up.
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
        // Never reached: a GIF is passed through before this point.
        Kind::Gif => Ok((0, 0)),
    }
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
            let used = buffer
                .get(..frame.buffer_size())
                .ok_or(Error::Corrupt(kind))?
                .to_vec();
            Image::new(width, height, channels, used).ok_or(Error::Corrupt(kind))
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
        Kind::WebP | Kind::Gif => Err(Error::Encode),
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
        let out = resize(&input).expect("resizes");
        assert!(!out.resized);
        assert_eq!(out.mime, "image/jpeg");
        assert_eq!(out.bytes, input, "byte for byte, no second loss");
    }

    #[test]
    fn a_small_png_is_sent_as_it_came() {
        let input = png(40, 30, 3);
        let out = resize(&input).expect("resizes");
        assert!(!out.resized);
        assert_eq!(out.mime, "image/png");
        assert_eq!(out.bytes, input);
    }

    #[test]
    fn a_gif_is_passed_through_untouched() {
        let input = gif();
        let out = resize(&input).expect("passes through");
        assert!(!out.resized);
        assert_eq!(out.mime, "image/gif");
        assert_eq!(out.bytes, input);
    }

    // ---- shrinking ---------------------------------------------------------------------------

    #[test]
    fn a_jpeg_over_the_long_side_is_scaled_down_and_still_decodes() {
        let input = jpeg(2600, 1000, 90, 3);
        assert!(input.len() < MAX_BYTES, "the bytes fit; the shape does not");
        let out = resize(&input).expect("resizes");
        assert!(out.resized);
        assert_eq!(out.mime, "image/jpeg");
        let image = read_back(&out.bytes);
        assert_eq!((image.width, image.height), (2000, 769));
        assert_eq!(image.channels, 3);
    }

    #[test]
    fn a_jpeg_over_the_byte_limit_comes_under_it() {
        let input = jpeg_noise(4000, 3000, 100, 3);
        assert!(input.len() > MAX_BYTES, "fixture is {} bytes", input.len());
        let out = resize(&input).expect("resizes");
        assert!(out.resized);
        assert!(out.bytes.len() <= MAX_BYTES, "{} bytes", out.bytes.len());
        let image = read_back(&out.bytes);
        assert_eq!((image.width, image.height), (2000, 1500));
    }

    #[test]
    fn a_png_over_the_byte_limit_comes_under_it() {
        let input = png_noise(3000, 2000, 3);
        assert!(input.len() > MAX_BYTES, "fixture is {} bytes", input.len());
        let out = resize(&input).expect("resizes");
        assert!(out.resized);
        assert!(out.bytes.len() <= MAX_BYTES, "{} bytes", out.bytes.len());
        let image = read_back(&out.bytes);
        // 2000 px long was still over the cap for an incompressible picture, so it is smaller.
        assert!(image.width < 2000, "{} px long", image.width);
        let expected = image.width as i64 * 2 / 3;
        assert!((image.height as i64 - expected).abs() <= 1);
    }

    #[test]
    fn a_png_stays_a_png_and_shrinks() {
        let input = png(2600, 1000, 3);
        let out = resize(&input).expect("resizes");
        assert!(out.resized);
        assert_eq!(out.mime, "image/png");
        assert!(out.bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
        let image = read_back(&out.bytes);
        assert_eq!((image.width, image.height), (2000, 769));
    }

    #[test]
    fn an_rgba_png_keeps_its_alpha() {
        let input = png(2200, 100, 4);
        let out = resize(&input).expect("resizes");
        let image = read_back(&out.bytes);
        assert_eq!(image.channels, 4);
        assert_eq!((image.width, image.height), (2000, 90));
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
        let out = resize(&input).expect("passes through");
        assert!(!out.resized);
        assert_eq!(out.mime, "image/webp");
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
        let out = resize(&input).expect("resizes");
        assert!(out.bytes.starts_with(&[0xFF, 0xD8, 0xFF]));
        assert_eq!(out.mime, "image/jpeg");
    }

    #[test]
    fn a_webp_is_written_as_a_png() {
        let input = webp(2600, 1000, false);
        assert_eq!(kind(&input), Some(Kind::WebP));
        let out = resize(&input).expect("resizes");
        assert!(out.resized);
        assert_eq!(out.mime, "image/png");
        assert!(out.bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
        let image = read_back(&out.bytes);
        assert_eq!((image.width, image.height), (2000, 769));
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
        let out = scaled_to(&image, 2000).expect("keeps the size");
        assert_eq!((out.width, out.height), (10, 10));
        assert_eq!(out.data, image.data);
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
            let out = resize(&input).expect("resizes");
            let took = started.elapsed();
            let peak = PEAK.load(Ordering::Relaxed);
            let back = read_back(&out.bytes);
            println!(
                "{name}: {} bytes -> {} bytes as {}, {}x{}, {} ms, peak {} MB above the input",
                input.len(),
                out.bytes.len(),
                out.mime,
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
}
