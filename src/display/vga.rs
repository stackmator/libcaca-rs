//! Emulated VGA text hardware.
//!
//! Port of `caca/driver/vga.c`. The physical accesses (the `0xB8000` screen,
//! palette and cursor ports) need real mode or a kernel driver, so here the
//! hardware is plain state: an 80x25 character/attribute buffer, the palette
//! registers and the cursor flag. Everything else is a direct translation:
//! fixed 80x25 geometry, the palette table, the dirty-rectangle blit with
//! CP437 conversion and `[...]` fullwidth markers, title refusal, the
//! 320x200 fallback dimensions and eventless behaviour.

use alloc::{vec, vec::Vec};

use crate::attr::Attr;
use crate::canvas::{Canvas, CACA_MAGIC_FULLWIDTH};
use crate::charset::utf32_to_cp437;
use crate::display::event::{Event, EventMask};
use crate::error::{CacaError, Result};

/// Text-mode width. The C driver hard-codes 80x25.
pub const VGA_WIDTH: i32 = 80;
/// Text-mode height. The C driver hard-codes 80x25.
pub const VGA_HEIGHT: i32 = 25;

/// VGA palette: `(register, r, g, b)` with 6-bit components, straight from
/// `vga.c`. Note the unusual register numbers (`0x14`, `0x38`–`0x3f`).
pub const VGA_PALETTE: [(u8, u8, u8, u8); 16] = [
    (0x00, 0x00, 0x00, 0x00),
    (0x01, 0x00, 0x00, 0x1f),
    (0x02, 0x00, 0x1f, 0x00),
    (0x03, 0x00, 0x1f, 0x1f),
    (0x04, 0x1f, 0x00, 0x00),
    (0x05, 0x1f, 0x00, 0x1f),
    (0x14, 0x1f, 0x1f, 0x00),
    (0x07, 0x1f, 0x1f, 0x1f),
    (0x38, 0x0f, 0x0f, 0x0f),
    (0x39, 0x0f, 0x0f, 0x3f),
    (0x3a, 0x0f, 0x3f, 0x0f),
    (0x3b, 0x0f, 0x3f, 0x3f),
    (0x3c, 0x3f, 0x0f, 0x0f),
    (0x3d, 0x3f, 0x0f, 0x3f),
    (0x3e, 0x3f, 0x3f, 0x0f),
    (0x3f, 0x3f, 0x3f, 0x3f),
];

/// Blit dirty rectangles into a `(char, attr)` byte buffer.
///
/// This is the `vga_display` loop: a fullwidth glyph occupies two cells
/// rendered as `[` and `]`.
///
/// One deliberate fix: the C version advances the destination with
/// `screen += dy * width + dx` (bytes, missing the `× 2` for the
/// char/attribute pairs) and only strides correctly per row, so partial
/// rects away from the origin land at the wrong offset. The buffer index
/// here uses the intended `(dx + dy * width) * 2`.
pub(crate) fn blit_buffer(buf: &mut [u8], width: i32, canvas: &Canvas) {
    let chars = canvas.chars();
    let attrs = canvas.attrs();

    for r in 0..canvas.dirty_rect_count() {
        let Ok((dx, dy, dw, dh)) = canvas.dirty_rect(r) else {
            continue;
        };

        let mut ci = (dx + dy * width) as usize;
        let mut bi = ((dx + dy * width) * 2) as usize;

        let mut y = dy;
        while y < dy + dh {
            let mut x = dx;
            while x < dx + dw {
                let mut ch = utf32_to_cp437(chars[ci]);
                if x < dx + dw - 1 && chars[ci + 1] == CACA_MAGIC_FULLWIDTH {
                    buf[bi] = b'[';
                    buf[bi + 1] = Attr::from_raw(attrs[ci]).to_ansi();
                    ch = b']';
                    ci += 1;
                    bi += 2;
                    x += 1;
                }
                buf[bi] = ch;
                buf[bi + 1] = Attr::from_raw(attrs[ci]).to_ansi();
                ci += 1;
                bi += 2;
                x += 1;
            }
            ci += (width - dw) as usize;
            bi += ((width - dw) * 2) as usize;
            y += 1;
        }
    }
}

/// Emulated VGA text hardware state.
pub struct Vga {
    buffer: Vec<u8>,
    cursor_visible: bool,
}

impl Vga {
    /// Initialise: blank buffer, hidden cursor, 80x25 canvas.
    pub fn new(canvas: &mut Canvas) -> Result<Vga> {
        canvas.set_size(VGA_WIDTH, VGA_HEIGHT)?;
        let _ = canvas.add_dirty_rect(0, 0, VGA_WIDTH, VGA_HEIGHT);

        Ok(Vga {
            buffer: vec![0; (VGA_WIDTH * VGA_HEIGHT * 2) as usize],
            cursor_visible: false,
        })
    }

    /// Shut down: show the cursor again, like the C version.
    pub fn end(&mut self) {
        self.cursor_visible = true;
    }

    /// The C fallback: a 320-pixel wide screen.
    pub fn display_width(&self) -> i32 {
        320
    }

    /// The C fallback: a 200-pixel high screen.
    pub fn display_height(&self) -> i32 {
        200
    }

    /// Blit dirty rectangles into the emulated buffer.
    pub fn display(&mut self, canvas: &Canvas) {
        let (w, h) = (canvas.width(), canvas.height());
        let need = (w.max(0) * h.max(0) * 2) as usize;
        if self.buffer.len() < need {
            self.buffer.resize(need, 0);
        }

        blit_buffer(&mut self.buffer, w, canvas);
    }

    /// Titles are unsupported, of course.
    pub fn set_title(&mut self, _title: &str) -> Result<()> {
        Err(CacaError::NotImplemented)
    }

    /// Report the window size. The C version knows nothing about its
    /// window, so this is always the canvas size.
    pub fn handle_resize(canvas: &Canvas) -> (i32, i32) {
        (canvas.width(), canvas.height())
    }

    /// The C version always reports no events (`FIXME`).
    pub fn get_event(&mut self, _mask: EventMask, _timeout_us: i64) -> Option<Event> {
        None
    }

    /// Read back an emulated `(char, attr)` cell.
    pub fn cell(&self, x: i32, y: i32, width: i32) -> Option<(u8, u8)> {
        if x < 0 || y < 0 {
            return None;
        }
        let idx = ((x + y * width) * 2) as usize;
        self.buffer.get(idx..idx + 2).map(|c| (c[0], c[1]))
    }

    /// The raw emulated text buffer.
    pub fn buffer(&self) -> &[u8] {
        &self.buffer
    }

    /// Whether the emulated cursor is currently visible.
    pub fn cursor_visible(&self) -> bool {
        self.cursor_visible
    }
}

impl Drop for Vga {
    fn drop(&mut self) {
        self.end();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attr::Color;

    fn headless_canvas() -> Canvas {
        let mut cv = Canvas::new(VGA_WIDTH, VGA_HEIGHT).unwrap();
        cv.clear_dirty_rect_list();
        cv
    }

    #[test]
    fn palette_registers_match_hardware() {
        assert_eq!(VGA_PALETTE[0], (0x00, 0x00, 0x00, 0x00));
        assert_eq!(VGA_PALETTE[6], (0x14, 0x1f, 0x1f, 0x00));
        assert_eq!(VGA_PALETTE[8], (0x38, 0x0f, 0x0f, 0x0f));
        assert_eq!(VGA_PALETTE[15], (0x3f, 0x3f, 0x3f, 0x3f));
    }

    #[test]
    fn blit_writes_char_and_attr_bytes() {
        let mut cv = headless_canvas();
        cv.set_color_ansi(Color::Red, Color::Blue).unwrap();
        cv.put_char(2, 1, b'A' as u32);

        let mut buf = vec![0u8; (VGA_WIDTH * VGA_HEIGHT * 2) as usize];
        blit_buffer(&mut buf, VGA_WIDTH, &cv);

        let idx = ((2 + VGA_WIDTH) * 2) as usize;
        assert_eq!(buf[idx], b'A');
        assert_eq!(
            buf[idx + 1],
            (Color::Blue.as_u8() << 4) | Color::Red.as_u8()
        );
        // Untouched cells stay zeroed.
        assert_eq!(buf[0], 0);
    }

    #[test]
    fn blit_marks_fullwidth_with_brackets() {
        let mut cv = headless_canvas();
        cv.put_char(0, 0, 0x3000); // fullwidth ideographic space

        let mut buf = vec![0u8; (VGA_WIDTH * VGA_HEIGHT * 2) as usize];
        blit_buffer(&mut buf, VGA_WIDTH, &cv);

        assert_eq!(buf[0], b'[');
        assert_eq!(buf[2], b']');
    }

    #[test]
    fn lifecycle_without_hardware() {
        let mut cv = Canvas::new(10, 5).unwrap();
        let mut vga = Vga::new(&mut cv).unwrap();
        assert_eq!((cv.width(), cv.height()), (80, 25));
        assert!(!vga.cursor_visible());
        assert_eq!(vga.display_width(), 320);
        assert!(vga.get_event(EventMask::ANY, 0).is_none());
        vga.end();
        assert!(vga.cursor_visible());
        assert!(vga.set_title("x").is_err());
    }

    #[test]
    fn resize_reports_canvas_size() {
        let cv = Canvas::new(40, 10).unwrap();
        assert_eq!(Vga::handle_resize(&cv), (40, 10));
    }
}
