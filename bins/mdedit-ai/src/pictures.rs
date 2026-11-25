//! The pictures of a request (`proto::VERSION` 2): read from their paths, made ready by
//! `resize`, checked against the digest the owner was shown, and put in the shape each sender
//! needs (`docs/ai-pictures-plan.md` sections 6 and 7).
//!
//! Everything read here is a file nobody has checked: it is read up to [`READ_MAX`], and the
//! decoders behind `resize` have no path that can panic (rule 11).

use std::io::Read;
use std::path::Path;

use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::proto::Fail;
use crate::resize::{self, Options, Prepared};

/// The most of a picture file that is read.
pub const READ_MAX: u64 = 64 * 1024 * 1024;

/// A picture a request names.
#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    pub path: String,
    /// The digest of the bytes as they would be sent, from `prepare`; empty in `prepare` itself.
    #[serde(default)]
    pub sha256: String,
}

/// A picture ready to go into a request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ready {
    pub bytes: Vec<u8>,
    pub mime: &'static str,
    /// Its file name's extension as sent: `jpg`, `png`, `webp`, `gif`.
    pub ext: &'static str,
    pub sha256: String,
}

/// The options of a request: an unknown or missing long side is the default one, and none is
/// larger than [`resize::LONG_SIDE_LARGE`].
pub fn options(long_side: u32, strip: bool) -> Options {
    let long_side = match long_side {
        0 => resize::LONG_SIDE,
        n => n.clamp(64, resize::LONG_SIDE_LARGE),
    };
    Options { long_side, strip }
}

/// The name a message gives a picture: its file name, or the path when it has none.
pub fn name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string())
}

fn image_fail(path: &str, why: impl std::fmt::Display) -> Fail {
    Fail::new("image", format!("{}: {why}", name(path)))
}

/// The file's bytes, at most [`READ_MAX`].
pub fn read(path: &str) -> Result<Vec<u8>, Fail> {
    let f = std::fs::File::open(path).map_err(|e| image_fail(path, read_why(&e)))?;
    let mut b = Vec::new();
    f.take(READ_MAX + 1)
        .read_to_end(&mut b)
        .map_err(|e| image_fail(path, read_why(&e)))?;
    if b.len() as u64 > READ_MAX {
        return Err(image_fail(
            path,
            format!(
                "larger than the {} MB a picture file may be",
                READ_MAX / (1024 * 1024)
            ),
        ));
    }
    Ok(b)
}

fn read_why(e: &std::io::Error) -> String {
    match e.kind() {
        std::io::ErrorKind::NotFound => "it is not there any more".to_string(),
        std::io::ErrorKind::PermissionDenied => "it may not be read".to_string(),
        _ => format!("it could not be read: {e}"),
    }
}

/// The lowercase hex SHA-256 of `bytes`.
pub fn digest(bytes: &[u8]) -> String {
    let d = Sha256::digest(bytes);
    let mut s = String::with_capacity(64);
    for b in d {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// One picture made ready, or why not.
pub fn prepare_one(path: &str, opts: &Options) -> Result<Prepared, Fail> {
    let bytes = read(path)?;
    resize::prepare(&bytes, opts).map_err(|e| image_fail(path, e))
}

/// The `picture` event `prepare` sends for the picture at `index`.
pub fn picture_event(id: u64, index: usize, path: &str, r: &Result<Prepared, Fail>) -> Value {
    match r {
        Ok(p) => {
            let mut v = json!({
                "ev": "picture",
                "id": id,
                "index": index,
                "path": path,
                "format": p.kind.short(),
                "width": p.width,
                "height": p.height,
                "bytes": p.bytes.len(),
                "sha256": digest(&p.bytes),
                "done": p.done(),
                "original_width": p.original_width,
                "original_height": p.original_height,
            });
            if let Some(t) = resize::thumbnail(p) {
                v["thumb"] = base64(&t).into();
            }
            v
        }
        Err(f) => json!({
            "ev": "picture",
            "id": id,
            "index": index,
            "path": path,
            "error": f.message,
        }),
    }
}

/// Each picture of a run made ready and checked: a picture whose bytes as sent are not the ones
/// measured (`sha256`) is refused, and so is the run, before anything is sent.
pub fn for_run(list: &[Picture], opts: &Options) -> Result<Vec<Ready>, Fail> {
    let mut v = Vec::with_capacity(list.len());
    for p in list {
        let ready = prepare_one(&p.path, opts)?;
        let sha256 = digest(&ready.bytes);
        if !sha256.eq_ignore_ascii_case(p.sha256.trim()) {
            return Err(image_fail(&p.path, "changed since it was measured"));
        }
        v.push(Ready {
            mime: ready.kind.mime(),
            ext: ext(ready.kind),
            bytes: ready.bytes,
            sha256,
        });
    }
    Ok(v)
}

fn ext(k: resize::Kind) -> &'static str {
    match k {
        resize::Kind::Jpeg => "jpg",
        resize::Kind::Png | resize::Kind::Bmp => "png",
        resize::Kind::WebP => "webp",
        resize::Kind::Gif => "gif",
    }
}

/// The `sent` event: what went, for the record, before the request is made.
pub fn sent(id: u64, ready: &[Ready], text_bytes: usize) -> Value {
    let images: Vec<Value> = ready
        .iter()
        .map(|r| json!({"bytes": r.bytes.len(), "sha256": r.sha256}))
        .collect();
    json!({"ev": "sent", "id": id, "images": images, "text_bytes": text_bytes})
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard base64 with padding.
pub fn base64(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = (u32::from(c[0]) << 16)
            | (u32::from(c.get(1).copied().unwrap_or(0)) << 8)
            | u32::from(c.get(2).copied().unwrap_or(0));
        s.push(B64[(n >> 18) as usize & 63] as char);
        s.push(B64[(n >> 12) as usize & 63] as char);
        s.push(if c.len() > 1 {
            B64[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        s.push(if c.len() > 2 {
            B64[n as usize & 63] as char
        } else {
            '='
        });
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_is_the_standard_one() {
        for (i, o) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64(i.as_bytes()), o);
        }
        assert_eq!(base64(&[0xFB, 0xFF]), "+/8=");
    }

    #[test]
    fn the_long_side_is_one_the_engine_allows() {
        assert_eq!(options(0, true).long_side, resize::LONG_SIDE);
        assert_eq!(options(9000, true).long_side, resize::LONG_SIDE_LARGE);
        assert_eq!(options(2576, false).long_side, 2576);
        assert!(!options(2576, false).strip);
    }

    fn scratch(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!(
            "mdedit-ai-test-pictures-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn png(w: u32, h: u32) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut e = png::Encoder::new(&mut out, w, h);
            e.set_color(png::ColorType::Rgb);
            let mut wr = e.write_header().unwrap();
            wr.write_image_data(&vec![120u8; (w * h * 3) as usize])
                .unwrap();
        }
        out
    }

    /// Plan 6: what is shown is what is sent, to the byte; a picture that no longer makes the
    /// measured bytes is refused with its name.
    #[test]
    fn a_picture_changed_since_it_was_measured_is_refused() {
        let d = scratch("changed");
        let p = d.join("IMG_4032.png");
        std::fs::write(&p, png(40, 30)).unwrap();
        let path = p.to_string_lossy().into_owned();
        let opts = options(0, true);
        let measured = digest(&prepare_one(&path, &opts).unwrap().bytes);
        let ok = for_run(
            &[Picture {
                path: path.clone(),
                sha256: measured.clone(),
            }],
            &opts,
        )
        .unwrap();
        assert_eq!(ok[0].sha256, measured);
        assert_eq!(ok[0].mime, "image/png");
        std::fs::write(&p, png(41, 30)).unwrap();
        let f = for_run(
            &[Picture {
                path,
                sha256: measured,
            }],
            &opts,
        )
        .unwrap_err();
        assert_eq!(f.kind, "image");
        assert_eq!(f.message, "IMG_4032.png: changed since it was measured");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_missing_or_unreadable_picture_is_named() {
        let d = scratch("missing");
        let f = prepare_one(&d.join("gone.jpg").to_string_lossy(), &options(0, true)).unwrap_err();
        assert_eq!(f.kind, "image");
        assert_eq!(f.message, "gone.jpg: it is not there any more");
        let svg = d.join("a.svg");
        std::fs::write(&svg, "<svg xmlns='http://www.w3.org/2000/svg'/>").unwrap();
        let f = prepare_one(&svg.to_string_lossy(), &options(0, true)).unwrap_err();
        assert!(
            f.message.starts_with("a.svg: SVG: not a picture"),
            "{}",
            f.message
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_picture_event_carries_the_figures_and_a_thumbnail() {
        let d = scratch("event");
        let p = d.join("wide.png");
        std::fs::write(&p, png(3000, 1000)).unwrap();
        let path = p.to_string_lossy().into_owned();
        let r = prepare_one(&path, &options(0, true));
        let v = picture_event(7, 2, &path, &r);
        assert_eq!(v["ev"], "picture");
        assert_eq!(v["index"], 2);
        assert_eq!(v["width"], 1568);
        assert_eq!(v["original_width"], 3000);
        assert_eq!(v["done"][0], "smaller");
        assert_eq!(v["sha256"].as_str().unwrap().len(), 64);
        assert!(
            v["thumb"]
                .as_str()
                .is_some_and(|t| t.starts_with("iVBORw0KGgo"))
        );
        let e = picture_event(
            7,
            3,
            "x/none.png",
            &prepare_one("x/none.png", &options(0, true)),
        );
        assert!(e["error"].as_str().unwrap().starts_with("none.png:"));
        let _ = std::fs::remove_dir_all(&d);
    }
}
