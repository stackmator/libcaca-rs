//! DOS `conio.h` compatibility layer.
//!
//! Port of `caca/caca_conio.c`. The C version keeps a global canvas and display
//! and exposes free `caca_conio_*` functions; this port provides an idiomatic
//! [`Conio`] value that owns its [`Display`] and canvas. Coordinates are
//! 1-based, like the DOS original.
//!
//! ```no_run
//! use libcaca::conio::Conio;
//!
//! let mut con = Conio::new()?;
//! con.clrscr();
//! con.cputs("Hello, conio!");
//! con.putch(b'!' as i32);
//! # Ok::<(), libcaca::CacaError>(())
//! ```

use std::time::{Duration, Instant};

use crate::attr::Color;
use crate::canvas::Canvas;
use crate::display::{Display, EventMask};
use crate::error::Result;

/// A DOS-style console attached to a libcaca display.
pub struct Conio {
    display: Display,
    unget_ch: Option<i32>,
    kbhit_ch: Option<i32>,
    pass_buffer: String,
    last_refresh: Instant,
}

impl Conio {
    /// Create a console backed by a fresh 80×25 display.
    pub fn new() -> Result<Conio> {
        let display = Display::new(Canvas::new(80, 25)?)?;
        Ok(Conio {
            display,
            unget_ch: None,
            kbhit_ch: None,
            pass_buffer: String::new(),
            last_refresh: Instant::now(),
        })
    }

    /// The underlying display.
    pub fn display(&self) -> &Display {
        &self.display
    }

    /// The underlying display, mutably.
    pub fn display_mut(&mut self) -> &mut Display {
        &mut self.display
    }

    /// Flush the display at most every 10 ms, as the C layer does.
    fn refresh(&mut self) {
        if self.last_refresh.elapsed() > Duration::from_millis(10) {
            let _ = self.display.refresh();
            self.last_refresh = Instant::now();
        }
    }

    fn force_refresh(&mut self) {
        let _ = self.display.refresh();
        self.last_refresh = Instant::now();
    }

    /// Clear the screen and move the cursor to the top-left corner.
    pub fn clrscr(&mut self) {
        self.display.canvas_mut().clear();
        self.display.canvas_mut().gotoxy(0, 0);
        self.force_refresh();
    }

    /// Clear from the cursor to the end of the line.
    pub fn clreol(&mut self) {
        let cv = self.display.canvas_mut();
        let (x, y) = (cv.wherex(), cv.wherey());
        let w = cv.width();
        cv.fill_box(x, y, w - x, 1, b' ' as u32);
        self.refresh();
    }

    /// Move the 1-based cursor to `(x, y)`.
    pub fn gotoxy(&mut self, x: i32, y: i32) {
        self.display.canvas_mut().gotoxy(x - 1, y - 1);
        self.refresh();
    }

    /// The 1-based cursor X coordinate.
    pub fn wherex(&self) -> i32 {
        self.display.canvas().wherex() + 1
    }

    /// The 1-based cursor Y coordinate.
    pub fn wherey(&self) -> i32 {
        self.display.canvas().wherey() + 1
    }

    /// Write one character at the cursor and advance.
    pub fn putch(&mut self, ch: i32) -> i32 {
        let ch = ch as u32;
        let cv = self.display.canvas_mut();
        let (x, y) = (cv.wherex(), cv.wherey());
        cv.put_char(x, y, ch);
        cv.gotoxy(x + 1, y);
        self.refresh();
        ch as i32
    }

    /// Write a string at the cursor.
    pub fn cputs(&mut self, s: &str) -> i32 {
        let mut last = 0i32;
        for ch in s.chars() {
            last = self.putch(ch as i32);
        }
        last
    }

    /// Formatted output at the current cursor position.
    pub fn cprintf(&mut self, args: core::fmt::Arguments<'_>) -> i32 {
        let cv = self.display.canvas_mut();
        let (x, y) = (cv.wherex(), cv.wherey());
        let ret = cv.printf(x, y, args);
        cv.gotoxy(x + ret, y);
        self.refresh();
        ret
    }

    /// Read a character, blocking until one is available.
    pub fn getch(&mut self) -> i32 {
        if let Some(c) = self.unget_ch.take() {
            return c;
        }
        if let Some(c) = self.kbhit_ch.take() {
            return c;
        }

        loop {
            if let Some(ev) = self.display.get_event(EventMask::KEY_PRESS, 1000) {
                let ch = ev.key().map(|k| k.ch).unwrap_or(0);
                self.force_refresh();
                return ch;
            }
            self.refresh();
        }
    }

    /// Read a character and echo it.
    pub fn getche(&mut self) -> i32 {
        let ch = self.getch();
        self.putch(ch);
        ch
    }

    /// Read a password of up to 8 characters without echoing.
    pub fn getpass(&mut self, _prompt: &str) -> &str {
        self.pass_buffer.clear();
        while self.pass_buffer.len() < 8 {
            let ch = self.getch();
            if ch == b'\n' as i32 || ch == b'\r' as i32 {
                break;
            }
            if let Some(c) = char::from_u32(ch as u32) {
                self.pass_buffer.push(c);
            }
        }
        self.force_refresh();
        &self.pass_buffer
    }

    /// Whether a key is available without blocking.
    pub fn kbhit(&mut self) -> bool {
        if self.unget_ch.is_some() || self.kbhit_ch.is_some() {
            return true;
        }
        if let Some(ev) = self.display.get_event(EventMask::KEY_PRESS, 0) {
            self.kbhit_ch = Some(ev.key().map(|k| k.ch).unwrap_or(0));
            true
        } else {
            false
        }
    }

    /// Push a character back onto the input stream.
    pub fn ungetch(&mut self, ch: i32) -> i32 {
        if self.unget_ch.is_some() {
            return -1;
        }
        self.unget_ch = Some(ch);
        ch
    }

    /// Set the foreground colour (0–15, CGA order).
    pub fn textcolor(&mut self, newcolor: u8) {
        let cv = self.display.canvas_mut();
        let bg = cv.get_attr(-1, -1).to_ansi_bg();
        let fg = Color::from_u8(newcolor).unwrap_or(Color::LightGray);
        let bg = Color::from_u8(bg).unwrap_or(Color::Black);
        let _ = cv.set_color_ansi(fg, bg);
    }

    /// Set the background colour (0–15, CGA order).
    pub fn textbackground(&mut self, newcolor: u8) {
        let cv = self.display.canvas_mut();
        let fg = cv.get_attr(-1, -1).to_ansi_fg();
        let fg = Color::from_u8(fg).unwrap_or(Color::LightGray);
        let bg = Color::from_u8(newcolor).unwrap_or(Color::Black);
        let _ = cv.set_color_ansi(fg, bg);
    }

    /// Show (1/2) or hide (0) the cursor.
    pub fn setcursortype(&mut self, cursor_type: i32) -> Result<()> {
        let show = cursor_type != 0;
        self.display.set_cursor(show)
    }

    /// Sleep for a number of milliseconds, refreshing the screen meanwhile.
    pub fn delay(&mut self, milliseconds: u32) {
        self.sleep_usec(milliseconds as u64 * 1000);
    }

    /// Sleep for a number of seconds, refreshing the screen meanwhile.
    pub fn sleep(&mut self, seconds: u32) {
        self.sleep_usec(seconds as u64 * 1_000_000);
    }

    fn sleep_usec(&mut self, mut usec: u64) {
        const SLICE: u64 = 5000;
        while usec > SLICE {
            self.force_refresh();
            std::thread::sleep(Duration::from_micros(SLICE));
            usec -= SLICE;
        }
        if usec > 0 {
            std::thread::sleep(Duration::from_micros(usec));
        }
        self.force_refresh();
    }
}

impl std::fmt::Debug for Conio {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Conio")
            .field("wherex", &self.wherex())
            .field("wherey", &self.wherey())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headless() -> Conio {
        // Force the null driver so tests never touch a real terminal.
        let display = Display::with_driver(Canvas::new(80, 25).unwrap(), Some("null")).unwrap();
        Conio {
            display,
            unget_ch: None,
            kbhit_ch: None,
            pass_buffer: String::new(),
            last_refresh: Instant::now(),
        }
    }

    #[test]
    fn puts_and_cursor() {
        let mut con = headless();
        con.clrscr();
        con.cputs("abc");
        assert_eq!(con.wherex(), 4);
        assert_eq!(con.wherey(), 1);
        assert_eq!(con.display.canvas().get_char(0, 0), b'a' as u32);
        con.gotoxy(1, 2);
        con.putch(b'X' as i32);
        assert_eq!(con.display.canvas().get_char(0, 1), b'X' as u32);
    }

    #[test]
    fn ungetch_and_kbhit() {
        let mut con = headless();
        assert!(!con.kbhit());
        con.ungetch(b'q' as i32);
        assert!(con.kbhit());
        assert_eq!(con.getch(), b'q' as i32);
        assert!(!con.kbhit());
    }

    #[test]
    fn colors() {
        let mut con = headless();
        con.textcolor(4); // red in CGA order
        con.textbackground(1); // blue
        let attr = con.display.canvas().get_attr(-1, -1);
        assert_eq!(attr.to_ansi_fg(), 4);
        assert_eq!(attr.to_ansi_bg(), 1);
    }
}
