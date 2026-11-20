//! B+ spike (docs/PLAN.md, "Visual editing, one block at a time"). Throwaway.
//!
//! The window shows the rendered page. A click on a top-level block replaces it, in place, by its
//! Markdown source in a plain text box; Esc or a click outside the box puts the text back and the
//! page is rendered again. No renderer change: while a block is open, the text before it and the
//! text after it are rendered as two documents and drawn above and below the box.
//!
//! `blockedit-spike [file.md]` opens the window; `--bench` also opens, types into and closes
//! every block by itself, three rounds, then prints the timings and exits. Each timing runs from
//! the input to the frame presented.

use mdshared_text::Bitmap;
use mdshared_ui::gfx::{Face, Gfx, Painter, Rect};
use mdview_render::{LIGHT, Options, render};
use std::num::NonZeroU32;
use std::rc::Rc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Window, WindowId};

/// The source box, logical px.
const FONT: f32 = 14.0;
const LINE: f32 = 21.0;
const PAD: f32 = 10.0;
/// Space between the box and the page around it.
const GAP: f32 = 6.0;

/// Rows `from..to` of a rendered page, drawn at a place of the composed page.
struct Piece {
    bmp: Bitmap,
    from: i32,
    to: i32,
}

/// One top-level block of the full page: its rows and its source bytes.
#[derive(Clone, Copy)]
struct Block {
    y: f32,
    h: f32,
    s: usize,
    e: usize,
}

struct Edit {
    before: String,
    text: String,
    after: String,
    caret: usize,
    above: Option<Piece>,
    below: Option<Piece>,
    /// Where the block was on the full page (device px).
    was: Block,
    /// Where the block after it was on the full page.
    next_y: f32,
}

struct App {
    window: Option<Rc<Window>>,
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
    gfx: Gfx,
    scale: f32,
    size: (u32, u32),
    source: String,
    original: String,
    full: Option<Piece>,
    blocks: Vec<Block>,
    edit: Option<Edit>,
    scroll: f32,
    mouse: (f32, f32),
    mods: ModifiersState,
    /// The input waiting for its frame: its kind and when it came.
    pending: Option<(&'static str, Instant)>,
    samples: Vec<(&'static str, f64)>,
    /// Logical px the page below (and above) the block moved when it opened.
    jumps: Vec<(f32, f32)>,
    bench: bool,
    /// The file's folder, so relative images load.
    base_dir: Option<std::path::PathBuf>,
}

fn line_starts(s: &str) -> Vec<usize> {
    let mut v = vec![0];
    v.extend(s.match_indices('\n').map(|(i, _)| i + 1));
    v
}

impl App {
    fn options(&self) -> Options {
        Options {
            width: self.size.0 as f32 / self.scale,
            scale: self.scale,
            theme: LIGHT,
            max_height: None,
            base_dir: self.base_dir.clone(),
        }
    }

    /// Renders the whole source and finds its blocks' byte ranges.
    fn render_full(&mut self) {
        let doc = mdview_core::parse(&self.source);
        let r = render(&doc, &self.options());
        let starts = line_starts(&self.source);
        let byte = |line: u32| {
            starts
                .get(line.saturating_sub(1) as usize)
                .copied()
                .unwrap_or(self.source.len())
        };
        let mut blocks = Vec::new();
        for (i, b) in r.blocks.iter().enumerate() {
            let line = doc.blocks[b.block as usize].line;
            let s = byte(line);
            let e = r
                .blocks
                .get(i + 1)
                .map_or(self.source.len(), |n| byte(doc.blocks[n.block as usize].line));
            if e <= s {
                continue; // drawn away from its source: not offered
            }
            let e = s + self.source[s..e].trim_end().len();
            blocks.push(Block { y: b.y, h: b.height, s, e });
        }
        self.blocks = blocks;
        if let Ok(out) = std::env::var("SPIKE_SHOT") {
            let _ = r.bitmap.write_png(std::path::Path::new(&out));
        }
        let to = r.content_height.ceil() as i32;
        self.full = Some(Piece { bmp: r.bitmap, from: 0, to });
    }

    /// Opens block `i` as source.
    fn open(&mut self, i: usize) {
        let was = self.blocks[i];
        let next_y = self.blocks.get(i + 1).map_or(was.y + was.h, |n| n.y);
        let before = self.source[..was.s].to_string();
        let after = self.source[was.e..].to_string();
        let opts = self.options();
        let above = (!before.trim().is_empty()).then(|| {
            let r = render(&mdview_core::parse(&before), &opts);
            let to = r.blocks.last().map_or(0.0, |b| b.y + b.height).ceil() as i32;
            Piece { bmp: r.bitmap, from: 0, to }
        });
        let below = (!after.trim().is_empty()).then(|| {
            let r = render(&mdview_core::parse(&after), &opts);
            let from = r.blocks.first().map_or(0.0, |b| b.y).floor() as i32;
            Piece { bmp: r.bitmap, from, to: r.content_height.ceil() as i32 }
        });
        let text = self.source[was.s..was.e].to_string();
        let caret = text.len();
        self.edit = Some(Edit { before, text, after, caret, above, below, was, next_y });
        let (box_y, below_y) = self.places();
        let e = self.edit.as_ref().unwrap();
        self.jumps.push(((box_y - e.was.y) / self.scale, (below_y - e.next_y) / self.scale));
    }

    /// Puts the edited text back and renders the page again.
    fn close(&mut self) {
        if let Some(e) = self.edit.take() {
            self.source = format!("{}{}{}", e.before, e.text, e.after);
            self.render_full();
        }
    }

    /// The box's top and the page-below's top, device px on the composed page.
    fn places(&self) -> (f32, f32) {
        let Some(e) = &self.edit else { return (0.0, 0.0) };
        let gap = GAP * self.scale;
        let top = match &e.above {
            Some(p) => p.to as f32 + gap,
            None => LIGHT.margin * self.scale,
        };
        (top, top + self.box_h() + gap)
    }

    fn box_h(&self) -> f32 {
        let lines = self.edit.as_ref().map_or(1, |e| e.text.split('\n').count());
        (lines as f32 * LINE + 2.0 * PAD) * self.scale
    }

    fn total(&self) -> f32 {
        match &self.edit {
            None => self.full.as_ref().map_or(0, |p| p.to) as f32,
            Some(e) => {
                let below = e.below.as_ref().map_or(0, |p| p.to - p.from) as f32;
                self.places().1 + below
            }
        }
    }

    /// The box's rectangle in logical window px.
    fn box_rect(&self) -> Rect {
        let w = self.size.0 as f32 / self.scale;
        let cw = (w - 2.0 * LIGHT.margin).min(LIGHT.reading_width).max(100.0);
        let y = (self.places().0 - self.scroll) / self.scale;
        Rect { x: (w - cw) / 2.0 - PAD, y, w: cw + 2.0 * PAD, h: self.box_h() / self.scale }
    }

    fn draw(&mut self) {
        let scroll = self.scroll;
        let rect = self.box_rect();
        let (_, below_y) = self.places();
        let (w, h) = self.size;
        let (Some(window), Some(surface)) = (self.window.clone(), self.surface.as_mut()) else {
            return;
        };
        let (Some(nw), Some(nh)) = (NonZeroU32::new(w), NonZeroU32::new(h)) else { return };
        surface.resize(nw, nh).unwrap();
        let mut canvas = Bitmap::new(w, h);
        canvas.clear(LIGHT.bg);
        let mut p = Painter::new(&mut canvas, &mut self.gfx, self.scale);
        let blit = |p: &mut Painter, piece: &Piece, at: f32| {
            p.blit(&piece.bmp, piece.from, piece.to - piece.from, 0, (at - scroll).round() as i32);
        };
        match &self.edit {
            None => {
                if let Some(full) = &self.full {
                    blit(&mut p, full, 0.0);
                }
            }
            Some(e) => {
                if let Some(a) = &e.above {
                    blit(&mut p, a, 0.0);
                }
                p.frame(rect, 6.0, LIGHT.code_bg, LIGHT.accent);
                let mut at = 0;
                for (i, line) in e.text.split('\n').enumerate() {
                    let cy = rect.y + PAD + LINE * (i as f32 + 0.5);
                    p.text(Face::Ui, FONT, rect.x + PAD, cy, line, LIGHT.text, f32::INFINITY);
                    let end = at + line.len();
                    if (at..=end).contains(&e.caret) {
                        let x = p.measure(Face::Ui, FONT, &line[..e.caret - at]);
                        let r = Rect { x: rect.x + PAD + x, y: cy - LINE / 2.0 + 2.0, w: 1.5, h: LINE - 4.0 };
                        p.fill(r, LIGHT.text);
                    }
                    at = end + 1;
                }
                if let Some(b) = &e.below {
                    blit(&mut p, b, below_y);
                }
            }
        }
        let mut buf = surface.buffer_mut().unwrap();
        for (d, s) in buf.iter_mut().zip(canvas.pixels.chunks_exact(4)) {
            *d = u32::from_le_bytes([s[0], s[1], s[2], s[3]]);
        }
        window.pre_present_notify();
        buf.present().unwrap();
        if let Some((kind, t)) = self.pending.take() {
            self.samples.push((kind, t.elapsed().as_secs_f64() * 1000.0));
        }
    }

    fn click(&mut self, x: f32, y: f32) {
        let py = y + self.scroll;
        if self.edit.is_some() {
            let r = self.box_rect();
            let (lx, ly) = (x / self.scale, y / self.scale);
            if !r.contains(lx, ly) {
                self.pending = Some(("close", Instant::now()));
                self.close();
                return;
            }
            // The caret to the nearest character boundary of the line clicked.
            let line = (((ly - r.y - PAD) / LINE).floor().max(0.0)) as usize;
            let e = self.edit.as_ref().unwrap();
            let lines: Vec<&str> = e.text.split('\n').collect();
            let line = line.min(lines.len() - 1);
            let start: usize = lines[..line].iter().map(|l| l.len() + 1).sum();
            let text = lines[line].to_string();
            let stops = Painter::new(&mut Bitmap::new(1, 1), &mut self.gfx, self.scale)
                .caret_stops(Face::Ui, FONT, &text);
            let want = lx - r.x - PAD;
            let (k, _) = stops
                .iter()
                .enumerate()
                .min_by(|a, b| (a.1 - want).abs().total_cmp(&(b.1 - want).abs()))
                .unwrap();
            let col = text.char_indices().map(|(i, _)| i).chain([text.len()]).nth(k).unwrap();
            self.edit.as_mut().unwrap().caret = start + col;
            self.pending = Some(("caret", Instant::now()));
            return;
        }
        if let Some(i) = self.blocks.iter().position(|b| py >= b.y && py < b.y + b.h) {
            self.pending = Some(("open", Instant::now()));
            self.open(i);
        }
    }

    fn key(&mut self, key: &Key, text: Option<&str>) -> bool {
        let Some(e) = self.edit.as_mut() else { return false };
        let t = &mut e.text;
        let prev = |t: &str, c: usize| t[..c].char_indices().last().map_or(0, |(i, _)| i);
        let next = |t: &str, c: usize| t[c..].chars().next().map_or(c, |ch| c + ch.len_utf8());
        let line_start = |t: &str, c: usize| t[..c].rfind('\n').map_or(0, |i| i + 1);
        let line_end = |t: &str, c: usize| t[c..].find('\n').map_or(t.len(), |i| c + i);
        match key {
            Key::Named(NamedKey::Escape) => {
                self.close();
                return true;
            }
            Key::Named(NamedKey::Enter) => {
                t.insert(e.caret, '\n');
                e.caret += 1;
            }
            Key::Named(NamedKey::Backspace) if e.caret > 0 => {
                let p = prev(t, e.caret);
                t.replace_range(p..e.caret, "");
                e.caret = p;
            }
            Key::Named(NamedKey::Delete) if e.caret < t.len() => {
                let n = next(t, e.caret);
                t.replace_range(e.caret..n, "");
            }
            Key::Named(NamedKey::ArrowLeft) => e.caret = prev(t, e.caret),
            Key::Named(NamedKey::ArrowRight) => e.caret = next(t, e.caret),
            Key::Named(NamedKey::Home) => e.caret = line_start(t, e.caret),
            Key::Named(NamedKey::End) => e.caret = line_end(t, e.caret),
            Key::Named(NamedKey::ArrowUp | NamedKey::ArrowDown) => {
                // By byte column, near enough for a spike.
                let s = line_start(t, e.caret);
                let col = e.caret - s;
                let target = if *key == Key::Named(NamedKey::ArrowUp) {
                    if s == 0 { return true }
                    line_start(t, s - 1)
                } else {
                    let end = line_end(t, e.caret);
                    if end == t.len() { return true }
                    end + 1
                };
                let mut c = (target + col).min(line_end(t, target));
                while !t.is_char_boundary(c) {
                    c -= 1;
                }
                e.caret = c;
            }
            _ => match text {
                Some(s) if !self.mods.control_key() && s.chars().all(|c| !c.is_control()) => {
                    t.insert_str(e.caret, s);
                    e.caret += s.len();
                }
                _ => return false,
            },
        }
        true
    }

    /// Opens, types into and closes every block, three rounds, timing each frame.
    fn run_bench(&mut self) {
        for _ in 0..3 {
            for i in 0..self.blocks.len() {
                self.pending = Some(("open", Instant::now()));
                self.open(i);
                self.draw();
                for c in "typed words ".chars() {
                    self.pending = Some(("key", Instant::now()));
                    let s = c.to_string();
                    self.key(&Key::Character(s.as_str().into()), Some(&s));
                    self.draw();
                }
                self.pending = Some(("enter", Instant::now()));
                self.key(&Key::Named(NamedKey::Enter), None);
                self.draw();
                self.pending = Some(("close", Instant::now()));
                self.close();
                self.draw();
                self.source = self.original.clone();
                self.render_full();
            }
        }
        self.report();
    }

    fn report(&self) {
        let mut out = String::new();
        for kind in ["open", "close", "key", "enter", "caret"] {
            let mut v: Vec<f64> = self.samples.iter().filter(|s| s.0 == kind).map(|s| s.1).collect();
            if v.is_empty() {
                continue;
            }
            v.sort_by(f64::total_cmp);
            let at = |q: f64| v[((v.len() - 1) as f64 * q).round() as usize];
            out += &format!(
                "{kind:6} n={:4}  p50 {:6.2} ms  p95 {:6.2} ms  max {:6.2} ms\n",
                v.len(),
                at(0.5),
                at(0.95),
                at(1.0)
            );
        }
        if !self.jumps.is_empty() {
            let top = self.jumps.iter().map(|j| j.0.abs()).fold(0.0, f32::max);
            let below: Vec<f32> = self.jumps.iter().map(|j| j.1).collect();
            out += &format!(
                "on open: the box's top moved up to {top:.0} px from the block's; the page below moved {:.0} to {:.0} px\n",
                below.iter().cloned().fold(f32::MAX, f32::min),
                below.iter().cloned().fold(f32::MIN, f32::max)
            );
        }
        out += &format!(
            "window {}x{} at scale {}, {} blocks\n",
            self.size.0,
            self.size.1,
            self.scale,
            self.blocks.len()
        );
        eprint!("{out}");
        let _ = std::fs::write("timings.txt", out);
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("B+ spike: click a block")
            .with_inner_size(winit::dpi::LogicalSize::new(900.0, 800.0));
        let window = Rc::new(el.create_window(attrs).unwrap());
        let ctx = softbuffer::Context::new(window.clone()).unwrap();
        self.surface = Some(softbuffer::Surface::new(&ctx, window.clone()).unwrap());
        self.scale = window.scale_factor() as f32;
        let s = window.inner_size();
        self.size = (s.width, s.height);
        self.window = Some(window);
        self.render_full();
        self.draw();
        if self.bench {
            self.run_bench();
            el.exit();
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let redraw = match event {
            WindowEvent::CloseRequested => {
                self.report();
                el.exit();
                false
            }
            WindowEvent::Resized(s) => {
                self.size = (s.width, s.height);
                self.close();
                self.render_full();
                true
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.scale = scale_factor as f32;
                false
            }
            WindowEvent::RedrawRequested => {
                self.draw();
                false
            }
            WindowEvent::ModifiersChanged(m) => {
                self.mods = m.state();
                false
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.mouse = (position.x as f32, position.y as f32);
                false
            }
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. } => {
                let (x, y) = self.mouse;
                self.click(x, y);
                true
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let by = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y * 60.0 * self.scale,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32,
                };
                let max = (self.total() - self.size.1 as f32).max(0.0);
                self.scroll = (self.scroll - by).clamp(0.0, max);
                true
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                let t = Instant::now();
                let kind = if event.logical_key == Key::Named(NamedKey::Escape) { "close" } else { "key" };
                let done = self.key(&event.logical_key, event.text.as_deref());
                if done {
                    self.pending = Some((kind, t));
                }
                done
            }
            _ => false,
        };
        if redraw {
            if let Some(w) = &self.window {
                w.request_redraw();
            }
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let bench = args.iter().any(|a| a == "--bench");
    let path = args.iter().find(|a| !a.starts_with("--")).cloned().unwrap_or_else(|| {
        concat!(env!("CARGO_MANIFEST_DIR"), "/sample.md").to_string()
    });
    let source = std::fs::read_to_string(&path).expect("read the file").replace("\r\n", "\n");
    // Formulas and diagrams: the installed engine, as mdedit finds it.
    if let Ok(dir) = std::env::var("LOCALAPPDATA") {
        let engine = std::path::Path::new(&dir).join("Programs/mdedit/mdview-engine.exe");
        if engine.exists() {
            mdview_render::set_engine_path(engine);
        }
    }
    let mut app = App {
        window: None,
        surface: None,
        gfx: Gfx::new(),
        scale: 1.0,
        size: (900, 800),
        original: source.clone(),
        source,
        full: None,
        blocks: Vec::new(),
        edit: None,
        scroll: 0.0,
        mouse: (0.0, 0.0),
        mods: ModifiersState::empty(),
        pending: None,
        samples: Vec::new(),
        jumps: Vec::new(),
        bench,
        base_dir: std::path::Path::new(&path).canonicalize().ok().and_then(|p| p.parent().map(|d| d.to_path_buf())),
    };
    EventLoop::new().unwrap().run_app(&mut app).unwrap();
}
