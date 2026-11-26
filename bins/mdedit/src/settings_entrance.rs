//! Fade the Settings card as a group without copying the pixels outside it.

use mdshared_ui::anim::Snapshot;
use mdshared_ui::gfx::{Painter, Rect};
use mdshared_ui::overlay::dialog_bounds;
use mdshared_ui::settings::pages::Form;
use mdshared_ui::theme::Palette;

pub(crate) fn backing(p: &mut Painter, form: &mut Form, window: Rect, t: f32) -> Option<Snapshot> {
    if t >= 1.0 {
        return None;
    }
    let card = form.layout(p, window);
    (t > 0.0).then(|| p.snapshot(dialog_bounds(card)))
}

pub(crate) fn fade(p: &mut Painter, pal: &Palette, backing: &Snapshot, t: f32) {
    let (x0, y0, x1, y1) = backing.rect();
    let stride = p.bmp.width as usize * 4;
    let lut = dim_levels(pal.dark, t);
    let dim = |bytes: &mut [u8]| {
        for px in bytes.as_chunks_mut::<4>().0 {
            for c in &mut px[..3] {
                *c = lut[usize::from(*c)];
            }
        }
    };
    for (y, row) in p.bmp.pixels.chunks_exact_mut(stride).enumerate() {
        if (y0 as usize..y1 as usize).contains(&y) {
            dim(&mut row[..x0 as usize * 4]);
            dim(&mut row[x1 as usize * 4..]);
        } else {
            dim(row);
        }
    }
    p.fade(backing, t);
}

fn dim_levels(dark: bool, t: f32) -> [u8; 256] {
    // Undo part of the already drawn dim: y * (1 - a*t) / (1 - a).
    // Build its byte lookup using draw_backdrop/span's quantized alpha and Snapshot's
    // rounded blend. A dimmed byte can have two possible originals; either produces
    // a result within one level of the full-copy method. Alpha/marks are unchanged.
    let a = (if dark { 0.45_f32 } else { 0.28 } * 256.0) as u32;
    let t = (t * 256.0).round() as u32;
    let mut out = [0; 256];
    for src in 0..256 {
        let dim = (src * (256 - a)) >> 8;
        out[dim as usize] = ((src * (256 - t) + dim * t + 128) >> 8) as u8;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use mdshared_text::Bitmap;
    use mdshared_ui::{gfx::Gfx, theme};

    #[test]
    fn the_card_only_fade_matches_the_whole_window_and_needs_no_endpoint_copy() {
        for pal in [theme::LIGHT, theme::DARK] {
            for scale in [1.0_f32, 1.25, 1.5] {
                let window = Rect::new(0.0, 0.0, 1280.0, 800.0);
                let mut form = Form::new("Settings", Vec::new());
                let mut gfx = Gfx::new();
                let mut screen = Bitmap::new((1280.0 * scale) as u32, (800.0 * scale) as u32);
                for (i, px) in screen.pixels.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                    let value = i as u8;
                    px.copy_from_slice(&[value, value.wrapping_mul(37), 255 - value, 255]);
                }
                let mut expected = screen.clone();
                let mut reference = Painter::new(&mut expected, &mut gfx, scale);
                let before = reference.snapshot(window);
                form.draw(&mut reference, &pal, window, true);
                let at_rest = reference.bmp.clone();
                let t = 0.57;
                reference.fade(&before, t);
                let mut p = Painter::new(&mut screen, &mut gfx, scale);
                assert!(backing(&mut p, &mut form, window, 0.0).is_none());
                let card = backing(&mut p, &mut form, window, t).unwrap();
                assert_eq!(card.rect(), p.dev_rect(dialog_bounds(form.rect())));
                assert!(card.rect().0 > 0, "never copy the whole window");
                form.draw(&mut p, &pal, window, true);
                fade(&mut p, &pal, &card, t);
                let error = p
                    .bmp
                    .pixels
                    .iter()
                    .zip(expected.pixels)
                    .map(|(a, b)| a.abs_diff(b))
                    .max();
                assert!(
                    error <= Some(1),
                    "mid-fade error {error:?} at scale {scale}"
                );
                before.restore(p.bmp);
                assert!(backing(&mut p, &mut form, window, 1.0).is_none());
                form.draw(&mut p, &pal, window, true);
                assert!(p.bmp.pixels == at_rest.pixels, "exact at rest");
            }
        }
    }
}
