//! Display contexts, drivers and event handling.
//!
//! Port of `caca/caca.c`, `caca/graphics.c` and `caca/event.c`. A [`Display`]
//! owns a [`Canvas`] and renders it through a selectable output driver.
//!
//! Available drivers:
//! - `null` — does nothing (useful for tests / headless rendering)
//! - `raw` — writes the native `caca` binary format to stdout
//! - `terminal` — ANSI/VT terminal driver with raw-mode input

pub mod event;
pub mod render;
mod terminal;
#[cfg(windows)]
mod win32;

pub use event::{key, Event, EventMask, KeyEvent};

use std::io::{IsTerminal, Write};
use std::time::{Duration, Instant};

use crate::canvas::Canvas;
use crate::error::{CacaError, Result};

/// The built-in display drivers.
#[cfg(windows)]
pub const DRIVER_LIST: &[(&str, &str)] = &[
    ("win32", "Windows console"),
    ("terminal", "ANSI terminal"),
    ("raw", "raw libcaca output"),
    ("null", "null driver"),
];

/// The built-in display drivers.
#[cfg(not(windows))]
pub const DRIVER_LIST: &[(&str, &str)] = &[
    ("terminal", "ANSI terminal"),
    ("raw", "raw libcaca output"),
    ("null", "null driver"),
];

/// A display driver kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Driver {
    Null,
    Raw,
    Terminal,
    Win32,
}

impl Driver {
    /// The driver's internal name.
    pub fn name(self) -> &'static str {
        match self {
            Driver::Null => "null",
            Driver::Raw => "raw",
            Driver::Terminal => "terminal",
            Driver::Win32 => "win32",
        }
    }

    /// Resolve a driver from its internal name.
    pub fn from_name(name: &str) -> Option<Driver> {
        match name.to_ascii_lowercase().as_str() {
            "null" => Some(Driver::Null),
            "raw" => Some(Driver::Raw),
            "terminal" | "ansi" => Some(Driver::Terminal),
            "win32" => {
                #[cfg(windows)]
                {
                    Some(Driver::Win32)
                }
                #[cfg(not(windows))]
                {
                    None
                }
            }
            _ => None,
        }
    }
}

/// Choose the best available driver for the current process.
fn autodetect_driver() -> Driver {
    #[cfg(windows)]
    {
        if win32::has_console() {
            return Driver::Win32;
        }
    }

    if std::io::stdin().is_terminal() && std::io::stdout().is_terminal() {
        Driver::Terminal
    } else {
        Driver::Null
    }
}

enum Backend {
    Null,
    Raw,
    Terminal(terminal::Terminal),
    #[cfg(windows)]
    Win32(win32::Win32),
}

/// A libcaca display context.
pub struct Display {
    canvas: Canvas,
    driver: Driver,
    backend: Backend,
    delay: i32,
    rendertime: i32,
    lastticks: i64,
    frame_start: Instant,
    mouse: (i32, i32),
    resize_allow: bool,
    event_queue: Vec<Event>,
}

impl Display {
    /// Create a display with a freshly allocated 80×24 canvas.
    pub fn create() -> Result<Display> {
        Display::with_driver(Canvas::new(0, 0)?, None)
    }

    /// Create a display attached to `canvas`, autodetecting the driver.
    pub fn new(canvas: Canvas) -> Result<Display> {
        Display::with_driver(canvas, None)
    }

    /// Create a display attached to `canvas` using a named driver.
    ///
    /// Passing `None` autodetects: the `CACA_DRIVER` environment variable,
    /// then the `terminal` driver if stdin is a TTY, else `null`.
    pub fn with_driver(canvas: Canvas, driver: Option<&str>) -> Result<Display> {
        let name = match driver {
            Some(n) if !n.is_empty() => Some(n.to_string()),
            _ => std::env::var("CACA_DRIVER").ok().filter(|s| !s.is_empty()),
        };

        let driver = match name {
            Some(n) => Driver::from_name(&n).ok_or(CacaError::Invalid)?,
            None => autodetect_driver(),
        };

        Display::build(canvas, driver)
    }

    fn build(mut canvas: Canvas, driver: Driver) -> Result<Display> {
        let backend = install_backend(&mut canvas, driver)?;
        let mouse = (canvas.width() / 2, canvas.height() / 2);

        Ok(Display {
            canvas,
            driver,
            backend,
            delay: 0,
            rendertime: 0,
            lastticks: 0,
            frame_start: Instant::now(),
            mouse,
            resize_allow: false,
            event_queue: Vec::new(),
        })
    }

    /// The current driver.
    pub fn driver(&self) -> Driver {
        self.driver
    }

    /// The current driver's internal name.
    pub fn driver_name(&self) -> &'static str {
        self.driver.name()
    }

    /// Dynamically switch to another driver.
    pub fn set_driver(&mut self, name: &str) -> Result<()> {
        let driver = Driver::from_name(name).ok_or(CacaError::Invalid)?;
        self.end_backend();
        self.driver = driver;
        self.backend = install_backend(&mut self.canvas, driver)?;
        Ok(())
    }

    /// The attached canvas.
    pub fn canvas(&self) -> &Canvas {
        &self.canvas
    }

    /// The attached canvas, mutably.
    pub fn canvas_mut(&mut self) -> &mut Canvas {
        &mut self.canvas
    }

    /// Consume the display and return the canvas.
    pub fn into_canvas(mut self) -> Canvas {
        // Swap the canvas out, then let `self` drop normally so the backend
        // is cleanly torn down.
        let empty = Canvas::new(0, 0).expect("zero-sized canvas is always valid");
        std::mem::replace(&mut self.canvas, empty)
    }

    /// Set the refresh delay in microseconds (0 disables constant framerate).
    pub fn set_display_time(&mut self, usec: i32) -> Result<()> {
        if usec < 0 {
            return Err(CacaError::Invalid);
        }
        self.delay = usec;
        Ok(())
    }

    /// The measured render time in microseconds.
    pub fn display_time(&self) -> i32 {
        self.rendertime
    }

    /// The display width in columns.
    pub fn display_width(&self) -> i32 {
        match &self.backend {
            Backend::Terminal(t) => t.display_width(),
            #[cfg(windows)]
            Backend::Win32(w) => w.display_width(),
            Backend::Null | Backend::Raw => self.canvas.width(),
        }
    }

    /// The display height in rows.
    pub fn display_height(&self) -> i32 {
        match &self.backend {
            Backend::Terminal(t) => t.display_height(),
            #[cfg(windows)]
            Backend::Win32(w) => w.display_height(),
            Backend::Null | Backend::Raw => self.canvas.height(),
        }
    }

    /// Set the window/terminal title.
    pub fn set_title(&mut self, title: &str) -> Result<()> {
        match &mut self.backend {
            Backend::Terminal(t) => {
                t.set_title(title);
                Ok(())
            }
            #[cfg(windows)]
            Backend::Win32(w) => {
                w.set_title(title);
                Ok(())
            }
            _ => Err(CacaError::NotImplemented),
        }
    }

    /// Show or hide the cursor.
    pub fn set_cursor(&mut self, show: bool) -> Result<()> {
        match &mut self.backend {
            Backend::Terminal(t) => {
                t.set_cursor(show);
                Ok(())
            }
            _ => Err(CacaError::NotImplemented),
        }
    }

    /// Show or hide the mouse pointer.
    pub fn set_mouse(&mut self, show: bool) -> Result<()> {
        match &mut self.backend {
            Backend::Terminal(t) => {
                t.set_mouse(show);
                Ok(())
            }
            _ => Err(CacaError::NotImplemented),
        }
    }

    /// The mouse X coordinate.
    pub fn mouse_x(&self) -> i32 {
        let w = self.canvas.width();
        if self.mouse.0 >= w {
            w - 1
        } else {
            self.mouse.0
        }
    }

    /// The mouse Y coordinate.
    pub fn mouse_y(&self) -> i32 {
        let h = self.canvas.height();
        if self.mouse.1 >= h {
            h - 1
        } else {
            self.mouse.1
        }
    }

    fn end_backend(&mut self) {
        if let Backend::Terminal(t) = &mut self.backend {
            t.end();
        }
        #[cfg(windows)]
        if let Backend::Win32(w) = &mut self.backend {
            w.end();
        }
        self.backend = Backend::Null;
    }

    fn apply_resize(&mut self, w: i32, h: i32) {
        if w <= 0 || h <= 0 {
            return;
        }
        if w == self.canvas.width() && h == self.canvas.height() {
            return;
        }

        self.resize_allow = true;
        let _ = self.canvas.set_size(w, h);
        self.resize_allow = false;

        match &mut self.backend {
            Backend::Terminal(t) => t.handle_resize(),
            #[cfg(windows)]
            Backend::Win32(win) => win.handle_resize(&self.canvas),
            _ => {}
        }
    }

    /// Flush pending changes and redraw the screen.
    pub fn refresh(&mut self) -> Result<()> {
        match &mut self.backend {
            Backend::Null => {}
            Backend::Raw => {
                let data = self.canvas.export_to_memory("caca")?;
                let mut out = std::io::stdout();
                let _ = out.write_all(&data);
                let _ = out.flush();
            }
            Backend::Terminal(t) => {
                t.display(&self.canvas);
            }
            #[cfg(windows)]
            Backend::Win32(w) => {
                w.display(&self.canvas);
            }
        }

        self.canvas.clear_dirty_rect_list();

        // Re-synchronise the canvas with the terminal after a resize.
        let terminal_size = match &self.backend {
            Backend::Terminal(t) => Some((t.display_width(), t.display_height())),
            _ => None,
        };
        if let Some((w, h)) = terminal_size {
            if w > 0 && h > 0 && (w != self.canvas.width() || h != self.canvas.height()) {
                self.apply_resize(w, h);
            }
        }

        // The Win32 console reports resizes as events, but also poll its
        // window size so a resize is not missed.
        #[cfg(windows)]
        {
            let console_size = match &self.backend {
                Backend::Win32(w) => Some(w.size()),
                _ => None,
            };
            if let Some((w, h)) = console_size {
                if w > 0 && h > 0 && (w != self.canvas.width() || h != self.canvas.height()) {
                    self.apply_resize(w, h);
                }
            }
        }

        // Constant framerate handling.
        let now = Instant::now();
        let mut ticks = self.lastticks + (now - self.frame_start).as_micros() as i64;
        const IDLE_USEC: i64 = 5000;
        while ticks + IDLE_USEC < self.delay as i64 {
            std::thread::sleep(Duration::from_micros(IDLE_USEC as u64));
            ticks = self.lastticks + (Instant::now() - self.frame_start).as_micros() as i64;
        }

        self.rendertime = ticks as i32;
        self.lastticks = ticks - self.delay as i64;
        if self.lastticks > self.delay as i64 {
            self.lastticks = 0;
        }
        self.frame_start = Instant::now();

        Ok(())
    }

    /// Wait for and return the next matching event.
    ///
    /// `timeout_us` is a timeout in microseconds, or negative to block
    /// indefinitely. Returns `None` if the timeout expired. Unlike the C API
    /// this never allocates and the event is returned by value.
    pub fn get_event(&mut self, mask: EventMask, timeout_us: i32) -> Option<Event> {
        if mask.bits() == 0 {
            return None;
        }

        // Events queued by higher-level code have priority.
        if let Some(pos) = self.event_queue.iter().position(|e| e.matches(mask)) {
            return Some(self.event_queue.remove(pos));
        }

        let ev = match &mut self.backend {
            Backend::Terminal(t) => t.get_event(mask, timeout_us as i64),
            #[cfg(windows)]
            Backend::Win32(w) => w.get_event(&self.canvas, mask, timeout_us as i64),
            Backend::Null | Backend::Raw => None,
        };

        let ev = ev?;
        match ev {
            Event::Resize { .. } => {
                // The canvas will be resized on the next refresh.
                Some(ev)
            }
            Event::MousePress { x, y, .. }
            | Event::MouseRelease { x, y, .. }
            | Event::MouseMotion { x, y } => {
                self.mouse = (x, y);
                Some(ev)
            }
            other => Some(other),
        }
    }

    /// Block until an event matching `mask` is received.
    pub fn wait_event(&mut self, mask: EventMask) -> Event {
        loop {
            if let Some(ev) = self.get_event(mask, -1) {
                return ev;
            }
        }
    }

    /// Push an event into the display's queue (useful for scripting/tests).
    pub fn push_event(&mut self, event: Event) {
        self.event_queue.push(event);
    }

    /// Whether the display driver currently allows resizes (mirrors the
    /// internal `resize.allow` flag).
    pub fn resize_allowed(&self) -> bool {
        self.resize_allow
    }
}

impl Drop for Display {
    fn drop(&mut self) {
        self.end_backend();
    }
}

fn install_backend(canvas: &mut Canvas, driver: Driver) -> Result<Backend> {
    Ok(match driver {
        Driver::Null => Backend::Null,
        Driver::Raw => {
            if canvas.width() == 0 || canvas.height() == 0 {
                canvas.set_size(80, 24)?;
            }
            Backend::Raw
        }
        Driver::Terminal => {
            let mut term = terminal::Terminal::new().map_err(|_| CacaError::Invalid)?;
            term.init();
            // Match the canvas size to the terminal like ncurses does.
            let (w, h) = (term.display_width(), term.display_height());
            if w > 0 && h > 0 && canvas.set_size(w, h).is_err() {
                term.end();
                return Err(CacaError::NoMem);
            }
            Backend::Terminal(term)
        }
        #[cfg(windows)]
        Driver::Win32 => {
            let win = win32::Win32::new(canvas)?;
            Backend::Win32(win)
        }
        #[cfg(not(windows))]
        Driver::Win32 => return Err(CacaError::Invalid),
    })
}

impl std::fmt::Debug for Display {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Display")
            .field("driver", &self.driver_name())
            .field("width", &self.canvas.width())
            .field("height", &self.canvas.height())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attr::Color;

    #[test]
    fn null_driver_roundtrip() {
        let mut cv = Canvas::new(10, 5).unwrap();
        cv.set_color_ansi(Color::Green, Color::Black).unwrap();
        cv.put_str(1, 1, "hi");
        let mut dp = Display::with_driver(cv, Some("null")).unwrap();
        assert_eq!(dp.driver_name(), "null");
        dp.canvas_mut().put_str(0, 0, "yo");
        dp.refresh().unwrap();
        assert_eq!(dp.canvas().get_char(0, 0), b'y' as u32);
    }

    #[test]
    fn push_and_get_event() {
        let dp_canvas = Canvas::new(4, 4).unwrap();
        let mut dp = Display::with_driver(dp_canvas, Some("null")).unwrap();
        dp.push_event(Event::KeyPress(KeyEvent::new(b'q' as i32, b'q' as u32)));
        let ev = dp.get_event(EventMask::KEY_PRESS, 0).unwrap();
        assert_eq!(ev.key().unwrap().ch, b'q' as i32);
        assert!(dp.get_event(EventMask::KEY_PRESS, 0).is_none());
    }

    #[test]
    fn toggle_driver() {
        let cv = Canvas::new(8, 8).unwrap();
        let mut dp = Display::with_driver(cv, Some("null")).unwrap();
        dp.set_driver("raw").unwrap();
        assert_eq!(dp.driver_name(), "raw");
    }
}
