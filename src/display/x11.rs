//! Native X11 driver (XCB protocol, no Xlib).
//!
//! Port of `caca/driver/x11.c` using pure-Rust `x11rb`: server-side core
//! fonts (`8x13bold`, then `fixed`), vector box-drawing like the C version,
//! and full event handling (keys, mouse, resize, close).
//!
//! Keyboard input assumes a US/evdev layout for printable keys (the shift and
//! NumLock modifiers are honoured); arrows, navigation and function keys are
//! mapped by keycode. This best-effort mapping is documented because, unlike
//! Xlib, there is no C keysym translator in a pure-Rust stack.
//!
//! Unix-only, behind the `x11` cargo feature. Never autodetected: like the C
//! driver it requires a `DISPLAY`, and requesting it without one fails cleanly
//! instead of popping up unexpectedly. Compile-verified; needs a live X
//! server for a runtime test.

#![cfg(all(feature = "x11", unix))]

use std::time::{Duration, Instant};

use x11rb::connection::{Connection, RequestConnection};
use x11rb::protocol::xproto::{
    Atom, ChangeGCAux, ChangeWindowAttributesAux, Char2b, CreateGCAux, CreateWindowAux, EventMask,
    Font as XFont, Gcontext, Pixmap, Point, PropMode, QueryFontReply, Rectangle, Window,
    WindowClass,
};
use x11rb::protocol::Event as XEvent;
use x11rb::rust_connection::RustConnection;

use crate::attr::Attr;
use crate::canvas::{Canvas, CACA_MAGIC_FULLWIDTH};
use crate::charset::{utf32_is_fullwidth, utf32_to_ascii};
use crate::display::event::{key, Event, EventMask as CacaMask, KeyEvent};
use crate::error::{CacaError, Result};

/// Box-drawing segment map, generated from libcaca's x11 driver.
/// Index is `ch - 0x2500`; a zero entry means "draw as text".
const UDLR: [u8; 109] = [
    0x05, 0x00, 0x50, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x11, 0x00, 0x00, 0x00,
    0x14, 0x00, 0x00, 0x00, 0x41, 0x00, 0x00, 0x00, 0x44, 0x00, 0x00, 0x00, 0x51, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x54, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x15, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x45, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x55, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x0a, 0xa0, 0x12, 0x21, 0x22, 0x18, 0x24, 0x28, 0x42, 0x81, 0x82, 0x48, 0x84, 0x88, 0x52, 0xa1,
    0xa2, 0x58, 0xa4, 0xa8, 0x1a, 0x25, 0x2a, 0x4a, 0x85, 0x8a, 0x5a, 0xa5, 0xaa,
];

/// X11 driver state.
pub struct X11 {
    conn: RustConnection,
    screen: usize,
    window: Window,
    pixmap: Pixmap,
    gc: Gcontext,
    font: XFont,
    font_width: i32,
    font_height: i32,
    font_offset: i32,
    max_char: u32,
    colors: [u32; 4096],
    wm_protocols: Atom,
    wm_delete_window: Atom,
    wm_name: Atom,
    xfixes: bool,
    cols: i32,
    rows: i32,
    mouse: (i32, i32),
    cursor_visible: bool,
    dirty_cursor: Option<(i32, i32)>,
    active: bool,
}

impl X11 {
    /// Open the display and size `canvas` to the window.
    pub fn new(canvas: &mut Canvas) -> Result<X11> {
        let (mut width, mut height) = (canvas.width(), canvas.height());

        if let (Ok(geo), true) = (std::env::var("CACA_GEOMETRY"), true) {
            let mut parts = geo.split('x');
            if let (Some(w), Some(h)) = (parts.next(), parts.next()) {
                if let (Ok(w), Ok(h)) = (w.parse::<i32>(), h.parse::<i32>()) {
                    width = w;
                    height = h;
                }
            }
        }
        if width <= 0 || height <= 0 {
            width = 80;
            height = 32;
        }

        let (conn, screen_num) = x11rb::connect(None).map_err(|_| CacaError::Invalid)?;

        // Cursor hiding needs the XFixes extension; remember availability
        // instead of failing when it is absent.
        let xfixes = conn
            .extension_information("XFIXES")
            .map(|info| info.is_some())
            .unwrap_or(false);

        // Clamp the canvas like the C version, then resize below.
        let _ = canvas.set_size(width, height);
        let (font, reply) = Self::load_font(&conn)?;
        let (font_width, font_height, font_offset, max_char) = Self::font_metrics(&reply, &font.1);

        let setup = conn.setup();
        let screen = &setup.roots[screen_num];
        let (root, depth, visual, colormap) = (
            screen.root,
            screen.root_depth,
            screen.root_visual,
            screen.default_colormap,
        );

        let mut colors = [0u32; 4096];
        for (i, slot) in colors.iter_mut().enumerate() {
            let i = i as u32;
            let r = ((i & 0xf00) >> 8) * 0x1111;
            let g = ((i & 0x0f0) >> 4) * 0x1111;
            let b = (i & 0x00f) * 0x1111;
            *slot =
                x11rb::protocol::xproto::alloc_color(&conn, colormap, r as u16, g as u16, b as u16)
                    .ok()
                    .and_then(|c| c.reply().ok())
                    .map(|r| r.pixel)
                    .unwrap_or(0);
        }

        let wid = conn.generate_id().map_err(|_| CacaError::Invalid)?;
        let aux = CreateWindowAux::new()
            .background_pixel(colors[0])
            .event_mask(EventMask::EXPOSURE | EventMask::STRUCTURE_NOTIFY);
        x11rb::protocol::xproto::create_window(
            &conn,
            depth,
            wid,
            root,
            0,
            0,
            (width * font_width) as u16,
            (height * font_height) as u16,
            0,
            WindowClass::INPUT_OUTPUT,
            visual,
            &aux,
        )
        .map_err(|_| CacaError::Invalid)?;

        let wm_protocols = Self::intern(&conn, b"WM_PROTOCOLS")?;
        let wm_delete_window = Self::intern(&conn, b"WM_DELETE_WINDOW")?;
        // WM_NAME is only needed for the window title.
        let wm_name = Self::intern(&conn, b"WM_NAME")?;
        if wm_protocols != 0 && wm_delete_window != 0 {
            let data = wm_delete_window.to_ne_bytes();
            x11rb::protocol::xproto::change_property(
                &conn,
                PropMode::REPLACE,
                wid,
                wm_protocols,
                4u32,
                32,
                1,
                &data,
            )
            .map_err(|_| CacaError::Invalid)?;
        }
        // STRING = 31, ATOM = 4 by the X11 protocol specification.
        x11rb::protocol::xproto::change_property(
            &conn,
            PropMode::REPLACE,
            wid,
            wm_name,
            31u32,
            8,
            9,
            b"caca for X",
        )
        .map_err(|_| CacaError::Invalid)?;

        // Map and wait for the window like the C version (bounded).
        x11rb::protocol::xproto::map_window(&conn, wid).map_err(|_| CacaError::Invalid)?;
        conn.flush().map_err(|_| CacaError::Invalid)?;
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            match conn.wait_for_event() {
                Ok(XEvent::MapNotify(_)) => break,
                Ok(_) => continue,
                Err(_) => break,
            }
        }

        let gc = conn.generate_id().map_err(|_| CacaError::Invalid)?;
        x11rb::protocol::xproto::create_gc(
            &conn,
            gc,
            wid,
            &CreateGCAux::new().graphics_exposures(0u32).font(font.0),
        )
        .map_err(|_| CacaError::Invalid)?;

        let pixmap = conn.generate_id().map_err(|_| CacaError::Invalid)?;
        x11rb::protocol::xproto::create_pixmap(
            &conn,
            depth,
            pixmap,
            wid,
            (width * font_width) as u16,
            (height * font_height) as u16,
        )
        .map_err(|_| CacaError::Invalid)?;

        // Now listen to everything.
        x11rb::protocol::xproto::change_window_attributes(
            &conn,
            wid,
            &ChangeWindowAttributesAux::new().event_mask(
                EventMask::EXPOSURE
                    | EventMask::STRUCTURE_NOTIFY
                    | EventMask::KEY_PRESS
                    | EventMask::KEY_RELEASE
                    | EventMask::BUTTON_PRESS
                    | EventMask::BUTTON_RELEASE
                    | EventMask::POINTER_MOTION,
            ),
        )
        .map_err(|_| CacaError::Invalid)?;
        conn.flush().map_err(|_| CacaError::Invalid)?;

        canvas.set_size(width, height)?;

        Ok(X11 {
            conn,
            screen: screen_num,
            window: wid,
            pixmap,
            gc,
            font: font.0,
            font_width,
            font_height,
            font_offset,
            max_char,
            colors,
            wm_protocols,
            wm_delete_window,
            wm_name,
            xfixes,
            cols: width,
            rows: height,
            mouse: (width / 2, height / 2),
            cursor_visible: false,
            dirty_cursor: None,
            active: true,
        })
    }

    fn intern(conn: &RustConnection, name: &[u8]) -> Result<Atom> {
        Ok(x11rb::protocol::xproto::intern_atom(conn, true, name)
            .map_err(|_| CacaError::Invalid)?
            .reply()
            .map_err(|_| CacaError::Invalid)?
            .atom)
    }

    /// Try `CACA_FONT`, then `8x13bold`, then `fixed`, like the C version.
    fn load_font(conn: &RustConnection) -> Result<((XFont, String), QueryFontReply)> {
        let mut candidates: Vec<String> = Vec::new();
        if let Ok(name) = std::env::var("CACA_FONT") {
            if !name.is_empty() {
                candidates.push(name);
            }
        }
        candidates.push(String::from("8x13bold"));
        candidates.push(String::from("fixed"));

        let mut last_err = CacaError::Invalid;
        for name in candidates {
            let fid = match conn.generate_id() {
                Ok(fid) => fid,
                Err(_) => {
                    last_err = CacaError::NoMem;
                    continue;
                }
            };
            match x11rb::protocol::xproto::open_font(conn, fid, name.as_bytes()) {
                Ok(cookie) => {
                    if cookie.check().is_err() {
                        continue;
                    }
                }
                Err(_) => continue,
            }
            match x11rb::protocol::xproto::query_font(conn, fid) {
                Ok(cookie) => match cookie.reply() {
                    Ok(reply) => return Ok(((fid, name), reply)),
                    Err(_) => {
                        let _ = x11rb::protocol::xproto::close_font(conn, fid);
                        last_err = CacaError::Invalid;
                    }
                },
                Err(_) => {
                    last_err = CacaError::Invalid;
                }
            }
        }
        Err(last_err)
    }

    fn font_metrics(reply: &QueryFontReply, name: &str) -> (i32, i32, i32, u32) {
        let lower = name.to_ascii_lowercase();
        let mut max_char = if lower.ends_with("-iso10646-1") {
            0xffff
        } else if lower.ends_with("-iso8859-1") {
            0xff
        } else {
            0x7f
        };
        let font_max = ((reply.max_byte1 as u32) << 8) | reply.max_char_or_byte2 as u32;
        if font_max != 0 && font_max < max_char {
            max_char = font_max;
        }

        let mut font_width = 0i32;
        if !reply.char_infos.is_empty()
            && reply.min_byte1 == 0
            && reply.min_char_or_byte2 <= 0x21
            && reply.max_char_or_byte2 >= 0x7e
        {
            for i in 0x21u16..0x7f {
                let cw =
                    reply.char_infos[(i - reply.min_char_or_byte2) as usize].character_width as i32;
                if cw > font_width {
                    font_width = cw;
                }
            }
        }
        if font_width == 0 {
            font_width = reply.max_bounds.character_width as i32;
        }
        let font_height = reply.max_bounds.ascent as i32 + reply.max_bounds.descent as i32;
        let font_offset = reply.max_bounds.descent as i32;

        (font_width.max(1), font_height.max(1), font_offset, max_char)
    }

    /// Shut down: unmap, free resources, close the connection.
    pub fn end(&mut self) {
        if !self.active {
            return;
        }
        self.active = false;
        let _ = x11rb::protocol::xproto::unmap_window(&self.conn, self.window);
        let _ = x11rb::protocol::xproto::free_pixmap(&self.conn, self.pixmap);
        let _ = x11rb::protocol::xproto::close_font(&self.conn, self.font);
        let _ = x11rb::protocol::xproto::free_gc(&self.conn, self.gc);
        let _ = x11rb::protocol::xproto::destroy_window(&self.conn, self.window);
        let _ = self.conn.flush();
    }

    /// Set the window title.
    pub fn set_title(&mut self, title: &str) {
        // WM_NAME with type STRING, Latin-1 approximated.
        let latin1: Vec<u8> = title.chars().map(|c| (c as u32).min(0xff) as u8).collect();
        let _ = x11rb::protocol::xproto::change_property(
            &self.conn,
            PropMode::REPLACE,
            self.window,
            self.wm_name,
            31u32,
            8,
            latin1.len() as u32,
            &latin1,
        );
        let _ = self.conn.flush();
    }

    /// The window width in pixels.
    pub fn display_width(&self) -> i32 {
        self.cols * self.font_width
    }

    /// The window height in pixels.
    pub fn display_height(&self) -> i32 {
        self.rows * self.font_height
    }

    /// The window size in character cells.
    pub fn size(&self) -> (i32, i32) {
        (self.cols, self.rows)
    }

    fn set_foreground(&self, pixel: u32) {
        let _ = x11rb::protocol::xproto::change_gc(
            &self.conn,
            self.gc,
            &ChangeGCAux::new().foreground(pixel),
        );
    }

    fn fill_rect(&self, x: i32, y: i32, w: i32, h: i32) {
        if w <= 0 || h <= 0 {
            return;
        }
        let _ = x11rb::protocol::xproto::poly_fill_rectangle(
            &self.conn,
            self.pixmap,
            self.gc,
            &[Rectangle {
                x: x as i16,
                y: y as i16,
                width: w as u16,
                height: h as u16,
            }],
        );
    }

    /// Render dirty rectangles to the backing pixmap, then copy it over.
    pub fn display(&mut self, canvas: &Canvas) {
        let width = canvas.width();
        let height = canvas.height();
        if width <= 0 || height <= 0 {
            return;
        }
        let chars = canvas.chars();
        let attrs = canvas.attrs();
        let (fw, fh, fo) = (self.font_width, self.font_height, self.font_offset);

        let mut dirty: Vec<(i32, i32, i32, i32)> = Vec::new();
        if let Some((x, y)) = self.dirty_cursor {
            if x >= 0 && y >= 0 && x < width && y < height {
                dirty.push((x, y, 1, 1));
            }
            self.dirty_cursor = None;
        }
        for r in 0..canvas.dirty_rect_count() {
            if let Ok(rect) = canvas.dirty_rect(r) {
                dirty.push(rect);
            }
        }

        for (dx, dy, dw, dh) in dirty {
            // Background pass with run-length merging, like the C version.
            let mut y = dy;
            while y < dy + dh {
                let mut x = dx;
                while x < dx + dw {
                    let bg = Attr::from_raw(attrs[(x + y * width) as usize]).to_rgb12_bg();
                    let mut len = 1;
                    while x + len < dx + dw
                        && Attr::from_raw(attrs[(x + len + y * width) as usize]).to_rgb12_bg() == bg
                    {
                        len += 1;
                    }
                    self.set_foreground(self.colors[bg as usize & 0xfff]);
                    self.fill_rect(x * fw, y * fh, len * fw, fh);
                    x += len;
                }
                y += 1;
            }

            // Foreground pass.
            for y in dy..dy + dh {
                let yoff = (y + 1) * fh - fo;
                for x in dx..dx + dw {
                    let idx = (x + y * width) as usize;
                    self.set_foreground(
                        self.colors[Attr::from_raw(attrs[idx]).to_rgb12_fg() as usize & 0xfff],
                    );
                    self.put_glyph(x * fw, y * fh, yoff, fw, fh, attrs[idx], chars[idx]);
                }
            }
        }

        if self.cursor_visible {
            let x = canvas.wherex();
            let y = canvas.wherey();
            self.set_foreground(self.colors[0xfff]);
            self.fill_rect(x * fw, y * fh, fw, fh);
            self.dirty_cursor = Some((x, y));
        }

        let _ = x11rb::protocol::xproto::copy_area(
            &self.conn,
            self.pixmap,
            self.window,
            self.gc,
            0,
            0,
            0,
            0,
            (width * fw) as u16,
            (height * fh) as u16,
        );
        let _ = self.conn.flush();
    }

    #[allow(clippy::too_many_arguments)]
    fn put_glyph(&self, x: i32, y: i32, yoff: i32, w: i32, h: i32, attr: u32, ch: u32) {
        // Underline.
        if attr & 0x04 != 0 {
            self.fill_rect(x, y + h - 1, w, 1);
        }

        if ch <= 0x20 || ch == CACA_MAGIC_FULLWIDTH {
            return;
        }

        let fw = if utf32_is_fullwidth(ch) { w * 2 } else { w };

        if (0x2500..=0x256c).contains(&ch) {
            let d = UDLR[(ch - 0x2500) as usize];
            if d != 0 {
                self.put_box_segments(x, y, fw, h, d);
                return;
            }
        }

        match ch {
            0xb7 | 0x2219 | 0x30fb => {
                self.fill_rect(x + fw / 2 - 1, y + h / 2 - 1, 2, 2);
                return;
            }
            0x2261 => {
                self.fill_rect(x + 1, y - 2 + h / 2, fw - 1, 1);
                self.fill_rect(x + 1, y + h / 2, fw - 1, 1);
                self.fill_rect(x + 1, y + 2 + h / 2, fw - 1, 1);
                return;
            }
            0x2580 => {
                self.fill_rect(x, y, fw, h / 2);
                return;
            }
            0x2584 => {
                self.fill_rect(x, y + h - h / 2, fw, h / 2);
                return;
            }
            0x2588 | 0x25ae => {
                self.fill_rect(x, y, fw, h);
                return;
            }
            0x258c => {
                self.fill_rect(x, y, fw / 2, h);
                return;
            }
            0x2590 => {
                self.fill_rect(x + fw - fw / 2, y, fw / 2, h);
                return;
            }
            0x25a0 | 0x25ac => {
                self.fill_rect(x, y + h / 4, fw, h / 2);
                return;
            }
            0x2591..=0x2593 => {
                // FIXME: dithered like the C version, which admits it sucks.
                let k = (ch - 0x2591) as i32;
                for j in 0..h {
                    for i in 0..fw {
                        if ((i + 2 * (j & 1)) & 3) > k {
                            continue;
                        }
                        let _ = x11rb::protocol::xproto::poly_point(
                            &self.conn,
                            x11rb::protocol::xproto::CoordMode::ORIGIN,
                            self.pixmap,
                            self.gc,
                            &[Point {
                                x: (x + i) as i16,
                                y: (y + j) as i16,
                            }],
                        );
                    }
                }
                return;
            }
            0x25cb | 0x2022 | 0x25cf => {
                let mut d = fw >> ((ch & 0x1 == 0) as i32);
                if h < fw {
                    d = h;
                }
                if d < 1 {
                    d = 1;
                }
                let (xo, yo) = ((fw - d) / 2, (h - d) / 2);
                let arc = x11rb::protocol::xproto::Arc {
                    x: (x + xo) as i16,
                    y: (y + yo) as i16,
                    width: d as u16,
                    height: d as u16,
                    angle1: 0,
                    angle2: 64 * 360,
                };
                if ch == 0x25cb {
                    let _ =
                        x11rb::protocol::xproto::poly_arc(&self.conn, self.pixmap, self.gc, &[arc]);
                } else {
                    let _ = x11rb::protocol::xproto::poly_fill_arc(
                        &self.conn,
                        self.pixmap,
                        self.gc,
                        &[arc],
                    );
                }
                return;
            }
            _ => {}
        }

        let (b1, b2) = if ch > self.max_char {
            (0u8, utf32_to_ascii(ch) as u8)
        } else {
            ((ch >> 8) as u8, ch as u8)
        };
        let _ = x11rb::protocol::xproto::image_text16(
            &self.conn,
            self.pixmap,
            self.gc,
            x as i16,
            yoff as i16,
            &[Char2b {
                byte1: b1,
                byte2: b2,
            }],
        );
    }

    fn put_box_segments(&self, x: i32, y: i32, fw: i32, h: i32, d: u8) {
        let step = |a: u8, b: u8| -> i32 {
            if d & a != 0 {
                -1
            } else if d & b != 0 {
                1
            } else {
                0
            }
        };

        if d & 0x04 != 0 {
            self.fill_rect(x, y + h / 2, fw / 2 + 1, 1);
        }
        if d & 0x01 != 0 {
            self.fill_rect(x + fw / 2, y + h / 2, (fw + 1) / 2, 1);
        }
        if d & 0x40 != 0 {
            self.fill_rect(x + fw / 2, y, 1, h / 2 + 1);
        }
        if d & 0x10 != 0 {
            self.fill_rect(x + fw / 2, y + h / 2, 1, (h + 1) / 2);
        }
        if d & 0x08 != 0 {
            self.fill_rect(x, y - 1 + h / 2, fw / 2 + 1 + step(0xc0, 0x20), 1);
            self.fill_rect(x, y + 1 + h / 2, fw / 2 + 1 + step(0x30, 0x80), 1);
        }
        if d & 0x02 != 0 {
            self.fill_rect(
                x - step(0xc0, 0x20) + fw / 2,
                y - 1 + h / 2,
                (fw + 1) / 2 + step(0xc0, 0x20),
                1,
            );
            self.fill_rect(
                x - step(0x30, 0x80) + fw / 2,
                y + 1 + h / 2,
                (fw + 1) / 2 + step(0x30, 0x80),
                1,
            );
        }
        if d & 0x80 != 0 {
            self.fill_rect(x - 1 + fw / 2, y, 1, h / 2 + 1 + step(0x0c, 0x02));
            self.fill_rect(x + 1 + fw / 2, y, 1, h / 2 + 1 + step(0x03, 0x08));
        }
        if d & 0x20 != 0 {
            self.fill_rect(
                x - 1 + fw / 2,
                y - step(0x0c, 0x02) + h / 2,
                1,
                (h + 1) / 2 + step(0x0c, 0x02),
            );
            self.fill_rect(
                x + 1 + fw / 2,
                y - step(0x03, 0x08) + h / 2,
                1,
                (h + 1) / 2 + step(0x03, 0x08),
            );
        }
    }

    /// Recreate the backing pixmap after a resize. The caller adds a full
    /// dirty rectangle so the fresh pixmap is fully repainted.
    pub fn handle_resize(&mut self) {
        let pixmap = match self.conn.generate_id() {
            Ok(id) => id,
            Err(_) => return,
        };
        let setup = self.conn.setup();
        let depth = setup.roots[self.screen].root_depth;
        let (w, h) = (self.cols.max(1), self.rows.max(1));
        if x11rb::protocol::xproto::create_pixmap(
            &self.conn,
            depth,
            pixmap,
            self.window,
            (w * self.font_width) as u16,
            (h * self.font_height) as u16,
        )
        .is_err()
        {
            return;
        }
        let _ = x11rb::protocol::xproto::free_pixmap(&self.conn, self.pixmap);
        self.pixmap = pixmap;
        let _ = self.conn.flush();
    }

    /// Show or hide the mouse pointer via the XFixes extension.
    pub fn set_mouse(&mut self, show: bool) -> Result<()> {
        if !self.xfixes {
            return Err(CacaError::NotImplemented);
        }
        if show {
            x11rb::protocol::xfixes::show_cursor(&self.conn, self.window)
                .map_err(|_| CacaError::Invalid)?;
        } else {
            x11rb::protocol::xfixes::hide_cursor(&self.conn, self.window)
                .map_err(|_| CacaError::Invalid)?;
        }
        self.conn.flush().map_err(|_| CacaError::Invalid)?;
        Ok(())
    }

    /// Show or hide the block cursor (drawn on the next refresh).
    pub fn set_cursor(&mut self, show: bool) {
        self.cursor_visible = show;
    }

    /// Poll X events until the deadline, returning the first match.
    pub fn get_event(&mut self, canvas: &Canvas, mask: CacaMask, timeout_us: i64) -> Option<Event> {
        if mask.bits() == 0 || !self.active {
            return None;
        }
        let deadline = if timeout_us >= 0 {
            Some(Instant::now() + Duration::from_micros(timeout_us as u64))
        } else {
            None
        };

        loop {
            while let Some(xev) = self.conn.poll_for_event().ok().flatten() {
                if let Some(ev) = self.translate(canvas, xev) {
                    if ev.matches(mask) {
                        return Some(ev);
                    }
                }
            }
            match deadline {
                Some(d) => {
                    let now = Instant::now();
                    if now >= d {
                        return None;
                    }
                    std::thread::sleep(Duration::from_millis(2).min(d - now));
                }
                None => std::thread::sleep(Duration::from_millis(2)),
            }
        }
    }

    fn translate(&mut self, canvas: &Canvas, xev: XEvent) -> Option<Event> {
        let (fw, fh) = (self.font_width, self.font_height);
        let (width, height) = (canvas.width(), canvas.height());
        match xev {
            XEvent::Expose(_) => {
                let _ = x11rb::protocol::xproto::copy_area(
                    &self.conn,
                    self.pixmap,
                    self.window,
                    self.gc,
                    0,
                    0,
                    0,
                    0,
                    (width * fw) as u16,
                    (height * fh) as u16,
                );
                let _ = self.conn.flush();
                None
            }
            XEvent::ConfigureNotify(ev) => {
                let w = (ev.width as i32 + fw / 3) / fw;
                let h = (ev.height as i32 + fh / 3) / fh;
                if w <= 0 || h <= 0 || (w == width && h == height) {
                    None
                } else {
                    Some(Event::Resize { w, h })
                }
            }
            XEvent::MotionNotify(ev) => {
                let mut x = ev.event_x as i32 / fw;
                let mut y = ev.event_y as i32 / fh;
                if x >= width {
                    x = width - 1;
                }
                if y >= height {
                    y = height - 1;
                }
                if (x, y) == self.mouse {
                    None
                } else {
                    self.mouse = (x, y);
                    Some(Event::MouseMotion { x, y })
                }
            }
            XEvent::ButtonPress(ev) => Some(Event::MousePress {
                x: self.mouse.0,
                y: self.mouse.1,
                button: ev.detail as i32,
            }),
            XEvent::ButtonRelease(ev) => Some(Event::MouseRelease {
                x: self.mouse.0,
                y: self.mouse.1,
                button: ev.detail as i32,
            }),
            XEvent::KeyPress(ev) => self.map_key(ev.detail, u16::from(ev.state), true),
            XEvent::KeyRelease(ev) => self.map_key(ev.detail, u16::from(ev.state), false),
            XEvent::ClientMessage(ev) => {
                if ev.type_ == self.wm_protocols && ev.data.as_data32()[0] == self.wm_delete_window
                {
                    Some(Event::Quit)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn map_key(&self, keycode: u8, state: u16, pressed: bool) -> Option<Event> {
        let shift = state & 0x01 != 0;
        let (ch, utf32) = match keycode {
            9 => (key::ESCAPE, 0x1b),
            23 => (key::TAB, 0x09),
            36 => (key::RETURN, 0x0d),
            22 => (key::BACKSPACE, 0x08),
            119 => (key::DELETE, 0),
            65 => (b' ' as i32, 0x20),
            111 => (key::UP, 0),
            116 => (key::DOWN, 0),
            113 => (key::LEFT, 0),
            114 => (key::RIGHT, 0),
            110 => (key::HOME, 0),
            115 => (key::END, 0),
            112 => (key::PAGEUP, 0),
            117 => (key::PAGEDOWN, 0),
            118 => (key::INSERT, 0),
            67 => (key::F1, 0),
            68 => (key::F2, 0),
            69 => (key::F3, 0),
            70 => (key::F4, 0),
            71 => (key::F5, 0),
            72 => (key::F6, 0),
            73 => (key::F7, 0),
            74 => (key::F8, 0),
            75 => (key::F9, 0),
            76 => (key::F10, 0),
            95 => (key::F11, 0),
            96 => (key::F12, 0),
            191 => (key::F13, 0),
            192 => (key::F14, 0),
            193 => (key::F15, 0),
            code => {
                // Numpad honors NumLock (Mod2); otherwise it navigates.
                if (79..=91).contains(&code) || code == 104 || code == 106 {
                    return self.map_numpad(code, state, pressed);
                }
                let (plain, shifted) = printable(code)?;
                let ch = if shift { shifted } else { plain };
                (ch as i32, ch as u32)
            }
        };
        let key = KeyEvent::new(ch, utf32);
        Some(if pressed {
            Event::KeyPress(key)
        } else {
            Event::KeyRelease(key)
        })
    }

    fn map_numpad(&self, code: u8, state: u16, pressed: bool) -> Option<Event> {
        let numlock = state & 0x10 != 0;
        let (ch, utf32) = match code {
            104 => (key::RETURN, 0x0d),
            106 => (b'/' as i32, 0x2f),
            63 => (b'*' as i32, 0x2a),
            82 => (b'-' as i32, 0x2d),
            86 => (b'+' as i32, 0x2b),
            79 => {
                if numlock {
                    (b'7' as i32, 0x37)
                } else {
                    (key::HOME, 0)
                }
            }
            80 => {
                if numlock {
                    (b'8' as i32, 0x38)
                } else {
                    (key::UP, 0)
                }
            }
            81 => {
                if numlock {
                    (b'9' as i32, 0x39)
                } else {
                    (key::PAGEUP, 0)
                }
            }
            83 => {
                if numlock {
                    (b'4' as i32, 0x34)
                } else {
                    (key::LEFT, 0)
                }
            }
            84 => {
                if numlock {
                    (b'5' as i32, 0x35)
                } else {
                    return None;
                }
            }
            85 => {
                if numlock {
                    (b'6' as i32, 0x36)
                } else {
                    (key::RIGHT, 0)
                }
            }
            87 => {
                if numlock {
                    (b'1' as i32, 0x31)
                } else {
                    (key::END, 0)
                }
            }
            88 => {
                if numlock {
                    (b'2' as i32, 0x32)
                } else {
                    (key::DOWN, 0)
                }
            }
            89 => {
                if numlock {
                    (b'3' as i32, 0x33)
                } else {
                    (key::PAGEDOWN, 0)
                }
            }
            90 => {
                if numlock {
                    (b'0' as i32, 0x30)
                } else {
                    (key::INSERT, 0)
                }
            }
            91 => {
                if numlock {
                    (b'.' as i32, 0x2e)
                } else {
                    (key::DELETE, 0)
                }
            }
            _ => return None,
        };
        let key = KeyEvent::new(ch, utf32);
        Some(if pressed {
            Event::KeyPress(key)
        } else {
            Event::KeyRelease(key)
        })
    }
}

impl Drop for X11 {
    fn drop(&mut self) {
        self.end();
    }
}

/// US-layout printable keycodes: `(unshifted, shifted)`.
fn printable(code: u8) -> Option<(u8, u8)> {
    Some(match code {
        10 => (b'1', b'!'),
        11 => (b'2', b'@'),
        12 => (b'3', b'#'),
        13 => (b'4', b'$'),
        14 => (b'5', b'%'),
        15 => (b'6', b'^'),
        16 => (b'7', b'&'),
        17 => (b'8', b'*'),
        18 => (b'9', b'('),
        19 => (b'0', b')'),
        20 => (b'-', b'_'),
        21 => (b'=', b'+'),
        24 => (b'q', b'Q'),
        25 => (b'w', b'W'),
        26 => (b'e', b'E'),
        27 => (b'r', b'R'),
        28 => (b't', b'T'),
        29 => (b'y', b'Y'),
        30 => (b'u', b'U'),
        31 => (b'i', b'I'),
        32 => (b'o', b'O'),
        33 => (b'p', b'P'),
        34 => (b'[', b'{'),
        35 => (b']', b'}'),
        38 => (b'a', b'A'),
        39 => (b's', b'S'),
        40 => (b'd', b'D'),
        41 => (b'f', b'F'),
        42 => (b'g', b'G'),
        43 => (b'h', b'H'),
        44 => (b'j', b'J'),
        45 => (b'k', b'K'),
        46 => (b'l', b'L'),
        47 => (b';', b':'),
        48 => (b'\'', b'"'),
        49 => (b'`', b'~'),
        51 => (b'\\', b'|'),
        52 => (b'z', b'Z'),
        53 => (b'x', b'X'),
        54 => (b'c', b'C'),
        55 => (b'v', b'V'),
        56 => (b'b', b'B'),
        57 => (b'n', b'N'),
        58 => (b'm', b'M'),
        59 => (b',', b'<'),
        60 => (b'.', b'>'),
        61 => (b'/', b'?'),
        65 => (b' ', b' '),
        _ => return None,
    })
}
