//! IME spike: logs every keyboard / modifier / IME event and draws committed text, the
//! preedit (underlined) and a block caret with plain rectangles (no font, no shaping).
use std::fs::File;
use std::io::Write;
use std::num::NonZeroU32;
use std::rc::Rc;
use std::time::{Duration, Instant};

use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, Ime, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;
use winit::window::{Window, WindowId};

const CELL_W: u32 = 14;
const CELL_H: u32 = 24;
const MARGIN: u32 = 10;

struct App {
    window: Option<Rc<Window>>,
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
    log: File,
    t0: Instant,
    deadline: Option<Instant>,
    committed: String,
    preedit: String,
    preedit_cursor: Option<(usize, usize)>,
    last_area: Option<(i32, i32)>,
}

fn is_wide(c: char) -> bool {
    let u = c as u32;
    (0x1100..=0x115F).contains(&u)
        || (0x2E80..=0xA4CF).contains(&u)
        || (0xAC00..=0xD7A3).contains(&u)
        || (0xF900..=0xFAFF).contains(&u)
        || (0xFE30..=0xFE6F).contains(&u)
        || (0xFF00..=0xFF60).contains(&u)
        || (0xFFE0..=0xFFE6).contains(&u)
        || (0x20000..=0x3FFFD).contains(&u)
}

/// Width of a char in cells: CJK and fullwidth take two.
fn cells(c: char) -> u32 {
    if is_wide(c) { 2 } else { 1 }
}

fn advance(c: char, col: &mut u32, row: &mut u32) {
    if c == '\n' {
        *col = 0;
        *row += 1;
    } else {
        *col += cells(c);
    }
}

fn fill_rect(buf: &mut [u32], bw: u32, bh: u32, x: u32, y: u32, w: u32, h: u32, color: u32) {
    for yy in y..(y + h).min(bh) {
        for xx in x..(x + w).min(bw) {
            buf[(yy * bw + xx) as usize] = color;
        }
    }
}

/// Pseudo glyph: a dot pattern derived from the code point, so different characters differ.
fn glyph(buf: &mut [u32], bw: u32, bh: u32, c: char, cx: u32, cy: u32, color: u32) {
    let n = cells(c);
    let mut bits = (c as u32).wrapping_mul(2654435761) | 0x0001_0000;
    for gy in 0..5u32 {
        for gx in 0..(3 * n) {
            if bits & 1 == 1 {
                fill_rect(buf, bw, bh, cx + 2 + gx * 4, cy + 3 + gy * 3, 3, 2, color);
            }
            bits = bits.rotate_right(1);
        }
    }
}

impl App {
    fn log(&mut self, s: String) {
        let line = format!("[{:9.3} ms] {}\n", self.t0.elapsed().as_secs_f64() * 1000.0, s);
        eprint!("{line}");
        let _ = self.log.write_all(line.as_bytes());
        let _ = self.log.flush();
    }

    /// Pixel position of the caret: after the committed text plus the preedit up to its cursor.
    fn caret_xy(&self) -> (u32, u32) {
        let (mut col, mut row) = (0u32, 0u32);
        for c in self.committed.chars() {
            advance(c, &mut col, &mut row);
        }
        let upto = self.preedit_cursor.map(|(a, _)| a).unwrap_or(self.preedit.len());
        for (i, c) in self.preedit.char_indices() {
            if i >= upto {
                break;
            }
            advance(c, &mut col, &mut row);
        }
        (MARGIN + col * CELL_W, MARGIN + row * CELL_H)
    }

    fn update_ime_area(&mut self) {
        let (x, y) = self.caret_xy();
        let key = (x as i32, y as i32);
        if self.last_area == Some(key) {
            return;
        }
        self.last_area = Some(key);
        if let Some(w) = &self.window {
            // Physical client-area pixels: top-left of the caret cell, size of the cell.
            w.set_ime_cursor_area(PhysicalPosition::new(key.0, key.1), PhysicalSize::new(CELL_W, CELL_H));
        }
        self.log(format!("set_ime_cursor_area pos=({}, {}) size=({}, {})", key.0, key.1, CELL_W, CELL_H));
    }

    fn redraw(&mut self) {
        let (x, y) = self.caret_xy();
        let (Some(window), Some(surface)) = (self.window.clone(), self.surface.as_mut()) else { return };
        let size = window.inner_size();
        let (Some(w), Some(h)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) else { return };
        if surface.resize(w, h).is_err() {
            return;
        }
        let Ok(mut buf) = surface.buffer_mut() else { return };
        let (bw, bh) = (size.width, size.height);
        buf.fill(0x00FFFFFF);
        // Block caret first so glyphs draw over it.
        fill_rect(&mut buf, bw, bh, x, y, CELL_W, CELL_H, 0x00A0C8FF);
        let (mut col, mut row) = (0u32, 0u32);
        for c in self.committed.chars() {
            if c == '\n' {
                col = 0;
                row += 1;
                continue;
            }
            if c.is_control() {
                continue;
            }
            glyph(&mut buf, bw, bh, c, MARGIN + col * CELL_W, MARGIN + row * CELL_H, 0x00202020);
            col += cells(c);
        }
        // Preedit: blue glyphs with an underline.
        for c in self.preedit.chars() {
            let n = cells(c);
            let (px, py) = (MARGIN + col * CELL_W, MARGIN + row * CELL_H);
            glyph(&mut buf, bw, bh, c, px, py, 0x001060C0);
            fill_rect(&mut buf, bw, bh, px, py + CELL_H - 3, n * CELL_W, 2, 0x001060C0);
            col += n;
        }
        let _ = buf.present();
    }

    fn request_redraw(&self) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("ime spike")
            .with_inner_size(winit::dpi::LogicalSize::new(800.0, 300.0));
        let window = Rc::new(el.create_window(attrs).expect("window"));
        window.set_ime_allowed(true);
        let ctx = softbuffer::Context::new(window.clone()).expect("ctx");
        self.surface = Some(softbuffer::Surface::new(&ctx, window.clone()).expect("surface"));
        self.window = Some(window);
        self.log("window created, set_ime_allowed(true)".into());
        self.update_ime_area();
        self.request_redraw();
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                self.log("CloseRequested".into());
                el.exit();
            }
            WindowEvent::RedrawRequested => self.redraw(),
            WindowEvent::Resized(_) => self.request_redraw(),
            WindowEvent::ModifiersChanged(m) => {
                self.log(format!("ModifiersChanged {:?}", m.state()));
            }
            WindowEvent::Focused(f) => self.log(format!("Focused({f})")),
            WindowEvent::KeyboardInput { event: ev, is_synthetic, .. } => {
                self.log(format!(
                    "KeyboardInput state={:?} logical={:?} text={:?} location={:?} repeat={} physical={:?} synthetic={} text_all_mods={:?} key_no_mods={:?}",
                    ev.state,
                    ev.logical_key,
                    ev.text,
                    ev.location,
                    ev.repeat,
                    ev.physical_key,
                    is_synthetic,
                    ev.text_with_all_modifiers(),
                    ev.key_without_modifiers()
                ));
                if ev.state == ElementState::Pressed {
                    match &ev.logical_key {
                        Key::Named(NamedKey::Backspace) => {
                            self.committed.pop();
                        }
                        Key::Named(NamedKey::Enter) => self.committed.push('\n'),
                        _ => {
                            if let Some(t) = &ev.text {
                                self.committed.extend(t.chars().filter(|c| !c.is_control()));
                            }
                        }
                    }
                    self.update_ime_area();
                    self.request_redraw();
                }
            }
            WindowEvent::Ime(ime) => {
                match &ime {
                    Ime::Enabled => self.log("Ime::Enabled".into()),
                    Ime::Preedit(s, c) => {
                        self.log(format!("Ime::Preedit({:?}, cursor={:?})", s, c));
                        self.preedit = s.clone();
                        self.preedit_cursor = *c;
                    }
                    Ime::Commit(s) => {
                        self.log(format!("Ime::Commit({:?})", s));
                        self.committed.push_str(s);
                        self.preedit.clear();
                        self.preedit_cursor = None;
                    }
                    Ime::Disabled => {
                        self.log("Ime::Disabled".into());
                        self.preedit.clear();
                        self.preedit_cursor = None;
                    }
                }
                self.update_ime_area();
                self.request_redraw();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        if let Some(d) = self.deadline {
            if Instant::now() >= d {
                self.log("auto-exit".into());
                el.exit();
            } else {
                el.set_control_flow(ControlFlow::WaitUntil(d));
            }
        }
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let (mut auto_ms, mut log_path) = (None::<u64>, String::from("ime-log.txt"));
    while let Some(a) = args.next() {
        match a.as_str() {
            "--auto-exit-ms" => auto_ms = args.next().and_then(|v| v.parse().ok()),
            "--log" => {
                if let Some(p) = args.next() {
                    log_path = p
                }
            }
            _ => {}
        }
    }
    let el = EventLoop::new().expect("event loop");
    el.set_control_flow(ControlFlow::Wait);
    let t0 = Instant::now();
    let mut app = App {
        window: None,
        surface: None,
        log: File::create(&log_path).expect("log file"),
        t0,
        deadline: auto_ms.map(|ms| t0 + Duration::from_millis(ms)),
        committed: String::new(),
        preedit: String::new(),
        preedit_cursor: None,
        last_area: None,
    };
    el.run_app(&mut app).expect("run");
}
