//! Where a text offset is on screen, for what hangs from the caret (the completion list).

use super::*;

impl View {
    /// The place of `offset` in the window as `(x, top, line height)`, logical px, from the
    /// geometry of the last layout: the x is estimated from the advances as `x_of` does. `None`
    /// when the line is not in view.
    pub fn point_of(
        &mut self,
        doc: &mut Doc,
        region: Rect,
        scale: f32,
        offset: usize,
    ) -> Option<(f32, f32, f32)> {
        let m = self.metrics(scale);
        let area = text_area(doc, region);
        let r = doc.ready_mut()?;
        let g = geom(r, area, &m);
        let lh = m.line_h as f32 / scale;
        let offset = offset.min(r.buffer.len_bytes());
        let (rows, x) = if g.wrapped {
            let top = wrap::top(r);
            let p = wrap::pos_of(r, offset);
            let n = wrap::rows_between(r, top, p, g.view_lines.ceil() as usize + 2)?;
            let (start, _) = wrap::row(r, p);
            let mut text = String::new();
            r.buffer
                .text(start.offset..offset.max(start.offset), &mut text);
            let cols = start.cols().after(&text, crate::live::tab_width());
            (
                n as f64,
                cols_x(cols, r.view.advance, r.view.wide_advance) as f32,
            )
        } else {
            let row = r.row_of_line(r.buffer.line_of(offset)) as f64;
            (row - g.top, x_of(r, offset) as f32 - r.view.left as f32)
        };
        if rows < 0.0 || rows >= g.view_lines.ceil() {
            return None;
        }
        Some((
            area.x + g.gutter_w + TEXT_PAD + x,
            area.y + (rows * m.line_h as f64) as f32 / scale,
            lh,
        ))
    }
}
