//! Frosted-window spike (Windows 11): an undecorated, resizable winit window presented through
//! softbuffer (GDI BitBlt), with a DWM system backdrop (Mica, Mica Alt, Acrylic) behind the parts
//! of a mock editor the program leaves transparent. Throwaway code; see RESULTS.md.
//!
//! Pixels are written as premultiplied ARGB: softbuffer documents `0x00RRGGBB`, but the top byte of
//! its 32-bit DIB section goes to the window with the rest, and that byte is what this spike tests.
#![cfg(windows)]

use std::fs::File;
use std::io::Write;
use std::num::NonZeroU32;
use std::rc::Rc;
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{HWND, RECT};
use windows_sys::Win32::Graphics::Dwm::{
    DwmExtendFrameIntoClientArea, DwmFlush, DwmGetWindowAttribute, DwmSetWindowAttribute,
};
use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use windows_sys::Win32::System::ProcessStatus::{
    GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX,
};
use windows_sys::Win32::System::Threading::GetCurrentProcess;
use windows_sys::Win32::UI::Controls::MARGINS;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowLongW, GetWindowRect, GWL_EXSTYLE, GWL_STYLE,
};

use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::platform::windows::WindowAttributesExtWindows;
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::{ResizeDirection, Theme, Window, WindowId, WindowLevel};

#[derive(Default)]
struct Opts {
    /// DWM_SYSTEMBACKDROP_TYPE: 2 Mica, 3 Acrylic (transient), 4 Mica Alt (tabbed). None: no call.
    backdrop: Option<u32>,
    /// DwmExtendFrameIntoClientArea with margins of -1 ("sheet of glass").
    extend: bool,
    /// winit's `with_transparent(true)`: DwmEnableBlurBehindWindow with an empty region.
    blur: bool,
    /// Draw the chrome with its flat colour at alpha 255 (the baseline).
    opaque: bool,
    /// Alpha (0..=255) of the flat chrome colour laid over the backdrop, premultiplied.
    tint: u32,
    /// Write the opaque parts with alpha 0 too, as an app unaware of alpha would (`0x00RRGGBB`).
    alpha0: bool,
    /// Leave out winit's `with_undecorated_shadow(true)` (which keeps a 1 px non-client strip at
    /// the top): the client area is then the whole window.
    no_marker: bool,
    dark: bool,
    inactive: bool,
    top: bool,
    /// A colourful opaque window to put behind the spike window for screenshots.
    pattern: bool,
    resize_test: bool,
    auto_exit: Option<u64>,
    log: Option<String>,
    pos: Option<(i32, i32)>,
    size: (u32, u32),
}

fn opts() -> Opts {
    let mut o = Opts { size: (900, 560), ..Opts::default() };
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut val = || args.next().unwrap_or_default();
        match a.as_str() {
            "--backdrop" => {
                o.backdrop = match val().as_str() {
                    "none" => Some(1),
                    "mica" => Some(2),
                    "acrylic" => Some(3),
                    "alt" => Some(4),
                    "auto" => Some(0),
                    _ => None,
                }
            }
            "--extend" => o.extend = true,
            "--no-marker" => o.no_marker = true,
            "--blur" => o.blur = true,
            "--opaque" => o.opaque = true,
            "--alpha0" => o.alpha0 = true,
            "--tint" => o.tint = val().parse().unwrap_or(0),
            "--dark" => o.dark = true,
            "--light" => o.dark = false,
            "--inactive" => o.inactive = true,
            "--top" => o.top = true,
            "--pattern" => o.pattern = true,
            "--resize-test" => o.resize_test = true,
            "--auto-exit-ms" => o.auto_exit = val().parse().ok(),
            "--log" => o.log = Some(val()),
            "--pos" => {
                let v = val();
                let mut p = v.split(',').filter_map(|s| s.parse().ok());
                if let (Some(x), Some(y)) = (p.next(), p.next()) {
                    o.pos = Some((x, y));
                }
            }
            "--size" => {
                let v = val();
                let mut p = v.split(',').filter_map(|s| s.parse().ok());
                if let (Some(w), Some(h)) = (p.next(), p.next()) {
                    o.size = (w, h);
                }
            }
            other => eprintln!("unknown argument {other}"),
        }
    }
    o
}

struct App {
    o: Opts,
    t0: Instant,
    log: Option<File>,
    window: Option<Rc<Window>>,
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
    first: bool,
    deadline: Option<Instant>,
    cursor: PhysicalPosition<f64>,
    resize_step: u32,
    resize_times: Vec<(f64, f64)>,
    base: PhysicalSize<u32>,
}

fn hwnd(w: &Window) -> HWND {
    match w.window_handle().map(|h| h.as_raw()) {
        Ok(RawWindowHandle::Win32(h)) => h.hwnd.get() as HWND,
        _ => std::ptr::null_mut(),
    }
}

/// (private bytes, working set) of this process, in MiB.
fn memory() -> (f64, f64) {
    let mut c: PROCESS_MEMORY_COUNTERS_EX = unsafe { std::mem::zeroed() };
    c.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
    unsafe {
        GetProcessMemoryInfo(
            GetCurrentProcess(),
            &mut c as *mut _ as *mut PROCESS_MEMORY_COUNTERS,
            c.cb,
        );
    }
    let mib = 1024.0 * 1024.0;
    (c.PrivateUsage as f64 / mib, c.WorkingSetSize as f64 / mib)
}

/// Premultiplied ARGB from a straight colour and an alpha.
fn px(rgb: u32, a: u32) -> u32 {
    let ch = |shift: u32| (((rgb >> shift) & 0xff) * a + 127) / 255;
    (a << 24) | (ch(16) << 16) | (ch(8) << 8) | ch(0)
}

struct Canvas<'a> {
    buf: &'a mut [u32],
    w: u32,
    h: u32,
}

impl Canvas<'_> {
    fn fill(&mut self, x: u32, y: u32, w: u32, h: u32, c: u32) {
        let (x1, y1) = ((x + w).min(self.w), (y + h).min(self.h));
        for yy in y.min(y1)..y1 {
            let row = (yy * self.w) as usize;
            self.buf[row + x.min(x1) as usize..row + x1 as usize].fill(c);
        }
    }

    /// A line of pseudo text: word-like blocks, lengths from a hash of `seed`.
    fn words(&mut self, x: u32, y: u32, max_w: u32, glyph_h: u32, seed: u32, c: u32) {
        let mut s = seed.wrapping_mul(2654435761) | 1;
        let mut cx = x;
        let unit = (glyph_h / 2).max(2);
        let line = 20 + (s >> 8) % 60;
        let end = x + max_w * line.min(80) / 80;
        loop {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            let w = unit * (2 + s % 7);
            if cx + w > end {
                break;
            }
            // Each "letter" is a thin bar, so the block has edges like text does.
            let mut lx = cx;
            while lx + unit <= cx + w {
                self.fill(lx, y, unit - (unit / 3).max(1), glyph_h, c);
                lx += unit;
            }
            cx += w + unit;
        }
    }
}

impl App {
    fn say(&mut self, s: &str) {
        let line = format!("[{:9.3} ms] {s}", self.t0.elapsed().as_secs_f64() * 1000.0);
        eprintln!("{line}");
        if let Some(f) = self.log.as_mut() {
            let _ = writeln!(f, "{line}");
        }
    }

    fn draw(&mut self) {
        let (Some(window), Some(surface)) = (self.window.as_ref(), self.surface.as_mut()) else {
            return;
        };
        let size = window.inner_size();
        let (Some(nw), Some(nh)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
        else {
            return;
        };
        if surface.resize(nw, nh).is_err() {
            return;
        }
        let Ok(mut buffer) = surface.buffer_mut() else {
            return;
        };
        let (w, h) = (size.width, size.height);
        let s = window.scale_factor();
        let u = |l: f64| (l * s).round() as u32;
        let mut c = Canvas { buf: &mut buffer, w, h };
        let o = &self.o;

        if o.pattern {
            // Diagonal colour stripes, a checker and hard black/white bars: something a blur
            // visibly changes.
            let cols = [0xe53935, 0xfdd835, 0x43a047, 0x1e88e5, 0x8e24aa, 0xffffff, 0x000000];
            let band = u(36.0).max(1);
            for y in 0..h {
                for x in 0..w {
                    let i = ((x + y) / band) as usize % cols.len();
                    c.buf[(y * w + x) as usize] = 0xff00_0000 | cols[i];
                }
            }
            let cell = u(12.0).max(1);
            for y in 0..u(120.0).min(h) {
                for x in 0..w {
                    let on = ((x / cell) + (y / cell)) % 2 == 0;
                    c.buf[(y * w + x) as usize] = if on { 0xffff_ffff } else { 0xff00_0000 };
                }
            }
            let _ = buffer.present();
            return;
        }

        let (page, chrome, ink, dim, line, accent) = if o.dark {
            (0x1e1e1e, 0x2b2b2b, 0xd4d4d4, 0x9a9a9a, 0x3c3c3c, 0x4ea1ff)
        } else {
            (0xffffff, 0xf3f3f3, 0x1e1e1e, 0x6a6a6a, 0xd0d0d0, 0x0a64d8)
        };
        // Alpha of everything meant to be opaque: 255, or 0 for an app that never wrote alpha.
        let solid = if o.alpha0 { 0 } else { 255 };
        let frost = if o.opaque { px(chrome, solid) } else { px(chrome, o.tint) };
        let (title_h, side_w, status_h) = (u(40.0), u(220.0), u(24.0));

        // Chrome: title row, side column, status strip. Transparent (or tinted) where frosted.
        c.fill(0, 0, w, h, frost);
        // The text area, opaque.
        let (tx, ty) = (side_w, title_h);
        let (tw, th) = (w.saturating_sub(side_w), h.saturating_sub(title_h + status_h));
        c.fill(tx, ty, tw, th, px(page, solid));
        // Hairlines between chrome and page, opaque.
        c.fill(tx, ty, tw, u(1.0).max(1), px(line, solid));
        c.fill(tx, ty, u(1.0).max(1), th, px(line, solid));
        c.fill(tx, ty + th, tw, u(1.0).max(1), px(line, solid));

        // Title row: three tabs (the active one an opaque page-coloured block) and caption boxes.
        let tab_w = u(150.0);
        for i in 0..3u32 {
            let x = u(12.0) + i * (tab_w + u(4.0));
            if i == 1 {
                c.fill(x, u(6.0), tab_w, title_h - u(6.0), px(page, solid));
            }
            let col = if i == 1 { ink } else { dim };
            c.words(x + u(12.0), u(16.0), tab_w - u(24.0), u(10.0), 7 + i, px(col, 255));
        }
        for i in 0..3u32 {
            let x = w.saturating_sub(u(46.0) * (i + 1)) + u(17.0);
            c.fill(x, u(15.0), u(11.0), u(11.0), px(dim, 255));
        }
        // Side column: file rows; one selected row with a half-transparent highlight.
        let row_h = u(26.0);
        let rows = (h.saturating_sub(title_h + status_h + u(60.0))) / row_h.max(1);
        for i in 0..rows.min(14) {
            let y = title_h + u(10.0) + i * row_h;
            if i == 3 && !o.opaque {
                c.fill(u(6.0), y - u(5.0), side_w - u(12.0), row_h - u(2.0), px(accent, 70));
            } else if i == 3 {
                c.fill(u(6.0), y - u(5.0), side_w - u(12.0), row_h - u(2.0), px(line, solid));
            }
            c.words(u(18.0), y, side_w - u(40.0), u(10.0), 100 + i, px(ink, 255));
        }
        // Alpha ramp: the accent colour from alpha 0 to 255, premultiplied. Shows whether the
        // route carries partial alpha (antialiased text edges over the backdrop need it).
        if !o.opaque {
            let (rx, ry) = (u(12.0), h.saturating_sub(status_h + u(40.0)));
            let (rw, rh) = (side_w - u(24.0), u(24.0));
            for i in 0..rw {
                c.fill(rx + i, ry, 1, rh, px(accent, i * 255 / rw.max(1)));
            }
        }
        // Status strip text.
        c.words(u(12.0), h.saturating_sub(status_h) + u(8.0), u(400.0), u(9.0), 999, px(dim, 255));
        // The page: pseudo text lines with a gutter.
        let lh = u(22.0).max(1);
        let lines = th.saturating_sub(u(12.0)) / lh;
        for i in 0..lines {
            let y = ty + u(12.0) + i * lh;
            c.fill(tx + u(14.0), y, u(18.0), u(10.0), px(dim, solid));
            c.words(tx + u(56.0), y, tw.saturating_sub(u(80.0)), u(11.0), 31 * i + 5, px(ink, solid));
        }
        let _ = buffer.present();
    }

    fn report_window(&mut self) {
        let Some(w) = self.window.clone() else { return };
        let h = hwnd(&w);
        let mut rect: RECT = unsafe { std::mem::zeroed() };
        let mut frame: RECT = unsafe { std::mem::zeroed() };
        let mut backdrop: u32 = 99;
        let (style, ex, hr_frame, hr_bd, fg) = unsafe {
            GetWindowRect(h, &mut rect);
            let hr_frame = DwmGetWindowAttribute(h, 9, &mut frame as *mut _ as *mut _, 16);
            let hr_bd = DwmGetWindowAttribute(h, 38, &mut backdrop as *mut _ as *mut _, 4);
            (
                GetWindowLongW(h, GWL_STYLE) as u32,
                GetWindowLongW(h, GWL_EXSTYLE) as u32,
                hr_frame,
                hr_bd,
                GetForegroundWindow() == h,
            )
        };
        self.say(&format!(
            "window rect {},{},{},{}  frame bounds {},{},{},{} (hr {hr_frame:#x})  style {style:#010x} ex {ex:#010x}  scale {}  backdrop read back {backdrop} (hr {hr_bd:#x})  foreground {fg}",
            rect.left, rect.top, rect.right, rect.bottom,
            frame.left, frame.top, frame.right, frame.bottom,
            w.scale_factor(),
        ));
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let o = &self.o;
        let mut a = Window::default_attributes()
            .with_title(if o.pattern { "frosted-spike pattern" } else { "frosted-spike" })
            .with_decorations(false)
            .with_visible(false)
            .with_active(!o.inactive)
            .with_theme(Some(if o.dark { Theme::Dark } else { Theme::Light }))
            .with_inner_size(LogicalSize::new(o.size.0 as f64, o.size.1 as f64))
            .with_undecorated_shadow(!o.pattern && !o.no_marker)
            .with_transparent(o.blur);
        if o.top {
            a = a.with_window_level(WindowLevel::AlwaysOnTop);
        }
        if let Some((x, y)) = o.pos {
            a = a.with_position(PhysicalPosition::new(x, y));
        }
        let window = Rc::new(el.create_window(a).expect("window"));
        self.say("window created");
        let h = hwnd(&window);
        // As the app's `style_window`: rounded corners (dark mode is set by `with_theme`).
        let corner: u32 = 2;
        let mut notes = Vec::new();
        unsafe {
            let hr = DwmSetWindowAttribute(h, 33, &corner as *const _ as *const _, 4);
            notes.push(format!("corner preference hr {hr:#x}"));
            if self.o.extend {
                let m = MARGINS {
                    cxLeftWidth: -1,
                    cxRightWidth: -1,
                    cyTopHeight: -1,
                    cyBottomHeight: -1,
                };
                let hr = DwmExtendFrameIntoClientArea(h, &m);
                notes.push(format!("DwmExtendFrameIntoClientArea(-1) hr {hr:#x}"));
            }
            if let Some(b) = self.o.backdrop {
                let hr = DwmSetWindowAttribute(h, 38, &b as *const _ as *const _, 4);
                notes.push(format!("DWMWA_SYSTEMBACKDROP_TYPE={b} hr {hr:#x}"));
            }
        }
        for n in notes {
            self.say(&n);
        }
        let ctx = softbuffer::Context::new(window.clone()).expect("context");
        self.surface = Some(softbuffer::Surface::new(&ctx, window.clone()).expect("surface"));
        self.base = window.inner_size();
        self.window = Some(window);
        self.say("surface");
        // As the app: the first frame goes to the hidden window, then the window is shown and the
        // frame is sent again.
        self.draw();
        self.say("first frame presented hidden");
        self.window.as_ref().unwrap().set_visible(true);
        self.draw();
        self.first = false;
        self.say("first frame (window shown, frame presented)");
        unsafe { DwmFlush() };
        self.say("DwmFlush returned (one composition after the first frame)");
        let (p, ws) = memory();
        self.say(&format!("memory at first frame: private {p:.2} MiB, working set {ws:.2} MiB"));
        self.report_window();
        if let Some(ms) = self.o.auto_exit {
            self.deadline = Some(Instant::now() + Duration::from_millis(ms));
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::RedrawRequested => self.draw(),
            WindowEvent::Resized(_) => {
                let t = Instant::now();
                self.draw();
                let d = t.elapsed().as_secs_f64() * 1000.0;
                if self.o.resize_test && self.resize_step > 0 {
                    unsafe { DwmFlush() };
                    self.resize_times.push((d, t.elapsed().as_secs_f64() * 1000.0));
                }
            }
            WindowEvent::Focused(f) => self.say(&format!("Focused({f})")),
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed
                    && event.logical_key == Key::Named(NamedKey::Escape)
                {
                    el.exit();
                }
            }
            WindowEvent::CursorMoved { position, .. } => self.cursor = position,
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                let Some(w) = self.window.as_ref() else { return };
                let size = w.inner_size();
                let s = w.scale_factor();
                let b = 6.0 * s;
                let (x, y) = (self.cursor.x, self.cursor.y);
                let (l, r) = (x < b, x > size.width as f64 - b);
                let (t, bt) = (y < b, y > size.height as f64 - b);
                let dir = match (l, r, t, bt) {
                    (true, _, true, _) => Some(ResizeDirection::NorthWest),
                    (_, true, true, _) => Some(ResizeDirection::NorthEast),
                    (true, _, _, true) => Some(ResizeDirection::SouthWest),
                    (_, true, _, true) => Some(ResizeDirection::SouthEast),
                    (true, ..) => Some(ResizeDirection::West),
                    (_, true, ..) => Some(ResizeDirection::East),
                    (_, _, true, _) => Some(ResizeDirection::North),
                    (_, _, _, true) => Some(ResizeDirection::South),
                    _ => None,
                };
                if let Some(d) = dir {
                    let _ = w.drag_resize_window(d);
                } else if y < 40.0 * s {
                    let _ = w.drag_window();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        // The resize test: 60 programmatic size changes, each drawn and presented.
        if self.o.resize_test && !self.first && self.resize_step < 60 {
            self.resize_step += 1;
            let k = self.resize_step;
            let grow = if k <= 30 { k } else { 60 - k } * 8;
            if let Some(w) = self.window.as_ref() {
                let _ = w.request_inner_size(PhysicalSize::new(
                    self.base.width + grow,
                    self.base.height + grow / 2,
                ));
            }
            if self.resize_step == 60 {
                let n = self.resize_times.len().max(1) as f64;
                let mean_d = self.resize_times.iter().map(|t| t.0).sum::<f64>() / n;
                let max_d = self.resize_times.iter().map(|t| t.0).fold(0.0, f64::max);
                let mean_f = self.resize_times.iter().map(|t| t.1).sum::<f64>() / n;
                let max_f = self.resize_times.iter().map(|t| t.1).fold(0.0, f64::max);
                self.say(&format!(
                    "resize test: {} frames; draw+present mean {mean_d:.2} ms max {max_d:.2} ms; with DwmFlush mean {mean_f:.2} ms max {max_f:.2} ms",
                    self.resize_times.len()
                ));
            }
            el.set_control_flow(ControlFlow::Poll);
            return;
        }
        match self.deadline {
            Some(d) if Instant::now() >= d => {
                let (p, ws) = memory();
                self.say(&format!("memory at exit: private {p:.2} MiB, working set {ws:.2} MiB"));
                self.report_window();
                self.deadline = None;
                el.exit();
            }
            Some(d) => el.set_control_flow(ControlFlow::WaitUntil(d)),
            None => el.set_control_flow(ControlFlow::Wait),
        }
    }
}

fn main() {
    let t0 = Instant::now();
    let o = opts();
    let mut power: SYSTEM_POWER_STATUS = unsafe { std::mem::zeroed() };
    unsafe { GetSystemPowerStatus(&mut power) };
    let log = o.log.as_ref().and_then(|p| File::create(p).ok());
    let mut app = App {
        o,
        t0,
        log,
        window: None,
        surface: None,
        first: true,
        deadline: None,
        cursor: PhysicalPosition::new(0.0, 0.0),
        resize_step: 0,
        resize_times: Vec::new(),
        base: PhysicalSize::new(0, 0),
    };
    app.say(&format!("args: {}", std::env::args().skip(1).collect::<Vec<_>>().join(" ")));
    app.say(&format!(
        "power: AC line {} battery flag {:#x} battery saver {}",
        power.ACLineStatus, power.BatteryFlag, power.SystemStatusFlag
    ));
    let el = EventLoop::new().expect("event loop");
    el.run_app(&mut app).expect("run");
}
