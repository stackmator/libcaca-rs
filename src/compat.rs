//! Pre-1.0 compatibility layer.
//!
//! Port of `caca/caca0.c` and `caca/caca0.h`: glue for applications written
//! against the old libcaca API. The C version exposes this through macros and
//! globals; here it is an explicit [`Compat`] value holding the display,
//! colours, features and bitmap registry. New code should use [`Canvas`],
//! [`Display`] and [`Dither`] directly.
//!
//! Opt-in via the `compat` cargo feature (which implies `std`), so programs
//! that do not need the legacy API pay nothing for it.
//!
//! ```no_run
//! use libcaca::compat::Compat;
//!
//! let mut caca = Compat::init()?;
//! caca.set_color(15, 1)?;
//! caca.putstr(1, 1, "hello, old API");
//! caca.refresh()?;
//! # Ok::<(), libcaca::CacaError>(())
//! ```

use alloc::vec::Vec;

use crate::attr::Color;
use crate::canvas::{rand, Canvas};
use crate::display::{Display, Event, EventMask};
use crate::dither::Dither;
use crate::error::{CacaError, Result};

/// Old background feature group.
pub const BACKGROUND: i32 = 0x10;
/// Old black-background value.
pub const BACKGROUND_BLACK: i32 = 0x11;
/// Old solid-background value.
pub const BACKGROUND_SOLID: i32 = 0x12;
/// Old antialiasing feature group.
pub const ANTIALIASING: i32 = 0x20;
/// Old no-antialiasing value.
pub const ANTIALIASING_NONE: i32 = 0x21;
/// Old prefilter value.
pub const ANTIALIASING_PREFILTER: i32 = 0x22;
/// Old dithering feature group.
pub const DITHERING: i32 = 0x30;
/// Old no-dithering value.
pub const DITHERING_NONE: i32 = 0x31;
/// Old 2x2 ordered value.
pub const DITHERING_ORDERED2: i32 = 0x32;
/// Old 4x4 ordered value.
pub const DITHERING_ORDERED4: i32 = 0x33;
/// Old 8x8 ordered value.
pub const DITHERING_ORDERED8: i32 = 0x34;
/// Old random value.
pub const DITHERING_RANDOM: i32 = 0x35;
/// Old unknown-feature value.
pub const FEATURE_UNKNOWN: i32 = 0xffff;

/// Old-style key-press event bit.
pub const EVENT_KEY_PRESS: u32 = 0x01000000;
/// Old-style key-release bit.
pub const EVENT_KEY_RELEASE: u32 = 0x02000000;
/// Old-style mouse-press bit.
pub const EVENT_MOUSE_PRESS: u32 = 0x04000000;
/// Old-style mouse-release bit.
pub const EVENT_MOUSE_RELEASE: u32 = 0x08000000;
/// Old-style mouse-motion bit.
pub const EVENT_MOUSE_MOTION: u32 = 0x10000000;
/// Old-style resize bit.
pub const EVENT_RESIZE: u32 = 0x20000000;
/// Old-style any-event mask.
pub const EVENT_ANY: u32 = 0xff000000;

/// Pre-1.0 libcaca context.
pub struct Compat {
    display: Display,
    fg: u8,
    bg: u8,
    background: i32,
    antialiasing: i32,
    dithering: i32,
    bitmaps: Vec<Dither>,
}

impl Compat {
    /// Initialise the compatibility context (autodetected driver).
    pub fn init() -> Result<Compat> {
        Self::with_driver(None)
    }

    /// Initialise with an explicit driver name (e.g. `"null"` for tests).
    pub fn with_driver(driver: Option<&str>) -> Result<Compat> {
        Ok(Compat {
            display: Display::with_driver(Canvas::new(0, 0)?, driver)?,
            fg: Color::LightGray as u8,
            bg: Color::Black as u8,
            background: BACKGROUND_SOLID,
            antialiasing: ANTIALIASING_PREFILTER,
            dithering: DITHERING_ORDERED4,
            bitmaps: Vec::new(),
        })
    }

    /// The attached canvas.
    pub fn canvas(&self) -> &Canvas {
        self.display.canvas()
    }

    /// The attached canvas, mutably.
    pub fn canvas_mut(&mut self) -> &mut Canvas {
        self.display.canvas_mut()
    }

    /// The underlying display.
    pub fn display_mut(&mut self) -> &mut Display {
        &mut self.display
    }

    /// Set the refresh delay in microseconds (old `caca_set_delay`).
    pub fn set_delay(&mut self, usec: i32) -> Result<()> {
        self.display.set_display_time(usec)
    }

    /// The measured render time in microseconds.
    pub fn rendertime(&self) -> i32 {
        self.display.display_time()
    }

    /// The canvas width in cells (old `caca_get_width`).
    pub fn width(&self) -> i32 {
        self.display.canvas().width()
    }

    /// The canvas height in cells (old `caca_get_height`).
    pub fn height(&self) -> i32 {
        self.display.canvas().height()
    }

    /// Set the window title.
    pub fn set_window_title(&mut self, title: &str) -> Result<()> {
        self.display.set_title(title)
    }

    /// The display width.
    pub fn window_width(&self) -> i32 {
        self.display.display_width()
    }

    /// The display height.
    pub fn window_height(&self) -> i32 {
        self.display.display_height()
    }

    /// Flush and redraw the screen (old `caca_refresh`).
    pub fn refresh(&mut self) -> Result<()> {
        self.display.refresh()
    }

    /// The mouse X coordinate.
    pub fn mouse_x(&self) -> i32 {
        self.display.mouse_x()
    }

    /// The mouse Y coordinate.
    pub fn mouse_y(&self) -> i32 {
        self.display.mouse_y()
    }

    /// Poll for an old-style event, encoded as `0x01000000 | value`.
    pub fn get_event(&mut self, mask: u32, timeout: i32) -> u32 {
        let m = (mask >> 24) & 0x7f;
        let mut new = 0u32;
        if m & 0x01 != 0 {
            new |= EventMask::KEY_PRESS.0;
        }
        if m & 0x02 != 0 {
            new |= EventMask::KEY_RELEASE.0;
        }
        if m & 0x04 != 0 {
            new |= EventMask::MOUSE_PRESS.0;
        }
        if m & 0x08 != 0 {
            new |= EventMask::MOUSE_RELEASE.0;
        }
        if m & 0x10 != 0 {
            new |= EventMask::MOUSE_MOTION.0;
        }
        if m & 0x20 != 0 {
            new |= EventMask::RESIZE.0;
        }

        let ev = match self.display.get_event(EventMask(new), timeout) {
            Some(ev) => ev,
            None => return 0,
        };

        match ev {
            Event::KeyPress(k) => EVENT_KEY_PRESS | (k.ch as u32 & 0xffffff),
            Event::KeyRelease(k) => EVENT_KEY_RELEASE | (k.ch as u32 & 0xffffff),
            Event::MousePress { button, .. } => EVENT_MOUSE_PRESS | (button as u32 & 0xffffff),
            Event::MouseRelease { button, .. } => EVENT_MOUSE_RELEASE | (button as u32 & 0xffffff),
            Event::MouseMotion { x, y } => {
                EVENT_MOUSE_MOTION | (((x & 0xfff) << 12) | (y & 0xfff)) as u32
            }
            Event::Resize { .. } => EVENT_RESIZE,
            _ => 0,
        }
    }

    /// Block for an old-style event (old `caca_wait_event`).
    pub fn wait_event(&mut self, mask: u32) -> u32 {
        self.get_event(mask, -1)
    }

    /// Integer square root (Newton's method, as in the C version).
    pub fn sqrt(a: u32) -> u32 {
        if a == 0 {
            return 0;
        }
        if a < 1_000_000_000 {
            let mut x = if a < 10 {
                1
            } else if a < 1000 {
                10
            } else if a < 100_000 {
                100
            } else if a < 10_000_000 {
                1000
            } else {
                10000
            };
            for _ in 0..4 {
                x = (x * x + a) / x / 2;
            }
            return x;
        }
        2 * Self::sqrt(a / 4)
    }

    /// Get a feature value (`0x10`, `0x20`, `0x30` groups).
    pub fn get_feature(&self, feature: i32) -> i32 {
        match feature {
            BACKGROUND => self.background,
            ANTIALIASING => self.antialiasing,
            DITHERING => self.dithering,
            _ => FEATURE_UNKNOWN,
        }
    }

    /// Set a feature, updating all live bitmaps like the C version.
    pub fn set_feature(&mut self, feature: i32) {
        match feature {
            BACKGROUND => self.set_feature(BACKGROUND_SOLID),
            BACKGROUND_BLACK | BACKGROUND_SOLID => {
                self.background = feature;
                let name = if feature == BACKGROUND_BLACK {
                    "black"
                } else {
                    "full16"
                };
                for d in &mut self.bitmaps {
                    let _ = d.set_color(name);
                }
            }
            ANTIALIASING => self.set_feature(ANTIALIASING_PREFILTER),
            ANTIALIASING_NONE | ANTIALIASING_PREFILTER => {
                self.antialiasing = feature;
                let name = if feature == ANTIALIASING_NONE {
                    "none"
                } else {
                    "prefilter"
                };
                for d in &mut self.bitmaps {
                    let _ = d.set_antialias(name);
                }
            }
            DITHERING => self.set_feature(DITHERING_ORDERED4),
            DITHERING_NONE | DITHERING_ORDERED2 | DITHERING_ORDERED4 | DITHERING_ORDERED8
            | DITHERING_RANDOM => {
                self.dithering = feature;
                let name = match feature {
                    DITHERING_NONE => "none",
                    DITHERING_ORDERED2 => "ordered2",
                    DITHERING_ORDERED4 => "ordered4",
                    DITHERING_ORDERED8 => "ordered8",
                    _ => "random",
                };
                for d in &mut self.bitmaps {
                    let _ = d.set_algorithm(name);
                }
            }
            _ => {}
        }
    }

    /// The human-readable feature name.
    pub fn feature_name(feature: i32) -> &'static str {
        match feature {
            BACKGROUND_BLACK => "black background",
            BACKGROUND_SOLID => "solid background",
            ANTIALIASING_NONE => "no antialiasing",
            ANTIALIASING_PREFILTER => "prefilter antialiasing",
            DITHERING_NONE => "no dithering",
            DITHERING_ORDERED2 => "2x2 ordered dithering",
            DITHERING_ORDERED4 => "4x4 ordered dithering",
            DITHERING_ORDERED8 => "8x8 ordered dithering",
            DITHERING_RANDOM => "random dithering",
            _ => "unknown",
        }
    }

    /// Load a sprite canvas from a file (old `caca_load_sprite`).
    pub fn load_sprite(path: &std::path::Path) -> Result<Canvas> {
        let mut cv = Canvas::new(0, 0)?;
        cv.import_from_file(path, "")?;
        Ok(cv)
    }

    /// Create a bitmap dither. Unlike the C version (which swaps width and
    /// bpp), the arguments are interpreted correctly as `(bpp, w, h, pitch)`.
    /// Returns a handle for [`Compat::draw_bitmap`] / [`Compat::free_bitmap`].
    #[allow(clippy::too_many_arguments)]
    pub fn create_bitmap(
        &mut self,
        bpp: u32,
        w: u32,
        h: u32,
        pitch: u32,
        r: u32,
        g: u32,
        b: u32,
        a: u32,
    ) -> Result<usize> {
        let mut d = Dither::new(w as i32, h as i32, bpp as i32, pitch as i32, r, g, b, a)?;
        let _ = d.set_color(if self.background == BACKGROUND_BLACK {
            "black"
        } else {
            "full16"
        });
        let _ = d.set_antialias(if self.antialiasing == ANTIALIASING_NONE {
            "none"
        } else {
            "prefilter"
        });
        let _ = d.set_algorithm(match self.dithering {
            DITHERING_NONE => "none",
            DITHERING_ORDERED2 => "ordered2",
            DITHERING_ORDERED4 => "ordered4",
            DITHERING_ORDERED8 => "ordered8",
            _ => "random",
        });
        self.bitmaps.push(d);
        Ok(self.bitmaps.len() - 1)
    }

    /// Free a bitmap created by [`Compat::create_bitmap`].
    pub fn free_bitmap(&mut self, handle: usize) {
        if handle < self.bitmaps.len() {
            self.bitmaps.remove(handle);
        }
    }

    /// Dither pixels with a bitmap handle (old `caca_draw_bitmap`).
    pub fn draw_bitmap(
        &mut self,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        handle: usize,
        pixels: &[u8],
    ) -> Result<()> {
        if handle >= self.bitmaps.len() {
            return Err(CacaError::Invalid);
        }
        // Split the borrows: the dither registry and the canvas are
        // disjoint fields of `self`.
        let (display, bitmaps) = (&mut self.display, &self.bitmaps);
        bitmaps[handle].dither_bitmap(display.canvas_mut(), x, y, w, h, pixels)
    }

    /// The colour name for a 0-15 index, or `"unknown"`.
    pub fn color_name(color: u8) -> &'static str {
        match color {
            0 => "black",
            1 => "blue",
            2 => "green",
            3 => "cyan",
            4 => "red",
            5 => "magenta",
            6 => "brown",
            7 => "light gray",
            8 => "dark gray",
            9 => "light blue",
            10 => "light green",
            11 => "light cyan",
            12 => "light red",
            13 => "light magenta",
            14 => "yellow",
            15 => "white",
            _ => "unknown",
        }
    }

    /// Set the default colour pair, remembering the values old-style.
    pub fn set_color(&mut self, fg: u8, bg: u8) -> Result<()> {
        self.fg = fg;
        self.bg = bg;
        let fg = Color::from_u8(fg).ok_or(CacaError::Invalid)?;
        let bg = Color::from_u8(bg).ok_or(CacaError::Invalid)?;
        self.display.canvas_mut().set_color_ansi(fg, bg)
    }

    /// The remembered foreground colour.
    pub fn fg_color(&self) -> u8 {
        self.fg
    }

    /// The remembered background colour.
    pub fn bg_color(&self) -> u8 {
        self.bg
    }

    /// Print a character (old `caca_putchar`).
    pub fn putchar(&mut self, x: i32, y: i32, ch: u32) -> i32 {
        self.display.canvas_mut().put_char(x, y, ch)
    }

    /// Print a string (old `caca_putstr`).
    pub fn putstr(&mut self, x: i32, y: i32, s: &str) -> i32 {
        self.display.canvas_mut().put_str(x, y, s)
    }

    /// Formatted output (old `caca_printf`).
    pub fn printf(&mut self, x: i32, y: i32, args: core::fmt::Arguments<'_>) -> i32 {
        self.display.canvas_mut().printf(x, y, args)
    }

    /// Clear the canvas (old `caca_clear`).
    pub fn clear(&mut self) {
        self.display.canvas_mut().clear();
    }

    /// Old `caca_draw_line`.
    pub fn draw_line(&mut self, x1: i32, y1: i32, x2: i32, y2: i32, ch: u32) {
        self.display.canvas_mut().draw_line(x1, y1, x2, y2, ch);
    }

    /// Old `caca_draw_polyline`.
    pub fn draw_polyline(&mut self, x: &[i32], y: &[i32], n: usize, ch: u32) {
        self.display.canvas_mut().draw_polyline(x, y, n, ch);
    }

    /// Old `caca_draw_thin_line`.
    pub fn draw_thin_line(&mut self, x1: i32, y1: i32, x2: i32, y2: i32) {
        self.display.canvas_mut().draw_thin_line(x1, y1, x2, y2);
    }

    /// Old `caca_draw_thin_polyline`.
    pub fn draw_thin_polyline(&mut self, x: &[i32], y: &[i32], n: usize) {
        self.display.canvas_mut().draw_thin_polyline(x, y, n);
    }

    /// Old `caca_draw_circle`.
    pub fn draw_circle(&mut self, x: i32, y: i32, r: i32, ch: u32) {
        self.display.canvas_mut().draw_circle(x, y, r, ch);
    }

    /// Old `caca_draw_ellipse`.
    pub fn draw_ellipse(&mut self, x: i32, y: i32, a: i32, b: i32, ch: u32) {
        self.display.canvas_mut().draw_ellipse(x, y, a, b, ch);
    }

    /// Old `caca_draw_thin_ellipse`.
    pub fn draw_thin_ellipse(&mut self, x: i32, y: i32, a: i32, b: i32) {
        self.display.canvas_mut().draw_thin_ellipse(x, y, a, b);
    }

    /// Old `caca_fill_ellipse`.
    pub fn fill_ellipse(&mut self, x: i32, y: i32, a: i32, b: i32, ch: u32) {
        self.display.canvas_mut().fill_ellipse(x, y, a, b, ch);
    }

    /// Old `caca_draw_box`.
    pub fn draw_box(&mut self, x: i32, y: i32, w: i32, h: i32, ch: u32) {
        self.display.canvas_mut().draw_box(x, y, w, h, ch);
    }

    /// Old `caca_draw_thin_box`.
    pub fn draw_thin_box(&mut self, x: i32, y: i32, w: i32, h: i32) {
        self.display.canvas_mut().draw_thin_box(x, y, w, h);
    }

    /// Old `caca_fill_box`.
    pub fn fill_box(&mut self, x: i32, y: i32, w: i32, h: i32, ch: u32) {
        self.display.canvas_mut().fill_box(x, y, w, h, ch);
    }

    /// Old `caca_draw_triangle`.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_triangle(&mut self, x1: i32, y1: i32, x2: i32, y2: i32, x3: i32, y3: i32, ch: u32) {
        self.display
            .canvas_mut()
            .draw_triangle(x1, y1, x2, y2, x3, y3, ch);
    }

    /// Old `caca_draw_thin_triangle`.
    pub fn draw_thin_triangle(&mut self, x1: i32, y1: i32, x2: i32, y2: i32, x3: i32, y3: i32) {
        self.display
            .canvas_mut()
            .draw_thin_triangle(x1, y1, x2, y2, x3, y3);
    }

    /// Old `caca_fill_triangle`.
    #[allow(clippy::too_many_arguments)]
    pub fn fill_triangle(&mut self, x1: i32, y1: i32, x2: i32, y2: i32, x3: i32, y3: i32, ch: u32) {
        self.display
            .canvas_mut()
            .fill_triangle(x1, y1, x2, y2, x3, y3, ch);
    }

    /// Old `caca_rand(a, b)` (inclusive upper bound).
    pub fn rand(a: i32, b: i32) -> i32 {
        rand(a, b + 1)
    }

    /// Blit a sprite canvas (old `caca_draw_sprite`, frame always 0).
    pub fn draw_sprite(&mut self, x: i32, y: i32, sprite: &Canvas) -> Result<()> {
        self.display.canvas_mut().blit(x, y, sprite, None)
    }
}

impl core::fmt::Debug for Compat {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Compat")
            .field("width", &self.display.canvas().width())
            .field("height", &self.display.canvas().height())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display::KeyEvent;
    use alloc::vec;

    fn headless() -> Compat {
        Compat::with_driver(Some("null")).unwrap()
    }

    #[test]
    fn old_event_encoding() {
        let mut c = headless();
        c.display_mut()
            .push_event(Event::KeyPress(KeyEvent::new(b'q' as i32, b'q' as u32)));
        assert_eq!(c.get_event(EVENT_ANY, 0), EVENT_KEY_PRESS | b'q' as u32);
        assert_eq!(c.get_event(EVENT_ANY, 0), 0);
    }

    #[test]
    fn sqrt_matches_newton() {
        assert_eq!(Compat::sqrt(0), 0);
        assert_eq!(Compat::sqrt(1), 1);
        assert_eq!(Compat::sqrt(2), 1);
        assert_eq!(Compat::sqrt(144), 12);
        assert_eq!(Compat::sqrt(1_000_000_007), 31622);
    }

    #[test]
    fn features_roundtrip() {
        let mut c = headless();
        assert_eq!(c.get_feature(BACKGROUND), BACKGROUND_SOLID);
        c.set_feature(DITHERING_RANDOM);
        assert_eq!(c.get_feature(DITHERING), DITHERING_RANDOM);
        assert_eq!(
            Compat::feature_name(DITHERING_ORDERED4),
            "4x4 ordered dithering"
        );
        assert_eq!(Compat::feature_name(0xdead), "unknown");
        assert_eq!(Compat::color_name(4), "red");
        assert_eq!(Compat::color_name(99), "unknown");
    }

    #[test]
    fn old_drawing_names_work() {
        let mut c = headless();
        c.canvas_mut().set_size(10, 5).unwrap();
        c.set_color(15, 1).unwrap();
        assert_eq!(c.fg_color(), 15);
        c.putstr(0, 0, "hi");
        assert_eq!(c.canvas().get_char(0, 0), b'h' as u32);
        c.draw_box(0, 0, 4, 3, b'#' as u32);
        assert_eq!(c.canvas().get_char(0, 0), b'#' as u32);
    }

    #[test]
    fn bitmap_registry() {
        let mut c = headless();
        let pixels = vec![0x808080u32; 4 * 4];
        let mut bytes = Vec::new();
        for p in pixels {
            bytes.extend_from_slice(&p.to_le_bytes());
        }
        let h = c
            .create_bitmap(32, 4, 4, 16, 0x00ff0000, 0x0000ff00, 0x000000ff, 0)
            .unwrap();
        c.draw_bitmap(0, 0, 4, 4, h, &bytes).unwrap();
        c.free_bitmap(h);
        assert!(c.draw_bitmap(0, 0, 4, 4, h, &bytes).is_err());
    }
}
