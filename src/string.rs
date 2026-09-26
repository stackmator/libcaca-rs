//! Character, string and attribute drawing functions.
//!
//! Port of `caca/string.c` (minus frame handling, which lives in [`crate::frame`]).

use alloc::{format, vec::Vec};

use crate::attr::{Attr, Color};
use crate::canvas::{Canvas, CACA_MAGIC_FULLWIDTH};
use crate::charset::utf32_is_fullwidth;
use crate::error::{CacaError, Result};

impl Canvas {
    /// Put the cursor at the given coordinates.
    pub fn gotoxy(&mut self, x: i32, y: i32) {
        let f = self.active_mut();
        f.x = x;
        f.y = y;
    }

    /// The cursor X coordinate.
    pub fn wherex(&self) -> i32 {
        self.active().x
    }

    /// The cursor Y coordinate.
    pub fn wherey(&self) -> i32 {
        self.active().y
    }

    /// Print a single UTF-32 character.
    ///
    /// Returns the number of cells the character occupies (2 for fullwidth).
    pub fn put_char(&mut self, x: i32, y: i32, ch: u32) -> i32 {
        if ch == CACA_MAGIC_FULLWIDTH {
            return 1;
        }

        let mut fullwidth = utf32_is_fullwidth(ch);
        let ret = if fullwidth { 2 } else { 1 };

        let width = self.width();
        let height = self.height();

        if x >= width || y < 0 || y >= height {
            return ret;
        }

        let mut x = x;
        let mut ch = ch;

        if x == -1 && fullwidth {
            x = 0;
            ch = b' ' as u32;
            fullwidth = false;
        } else if x < 0 {
            return ret;
        }

        let attr = self.active().curattr;
        let mut xmin = x;
        let mut xmax = x;
        let changed;

        {
            let f = self.active_mut();
            let idx = (x + y * width) as usize;

            // Overwriting the right half of a fullwidth char: clear its left half.
            if x != 0 && f.chars[idx] == CACA_MAGIC_FULLWIDTH {
                f.chars[idx - 1] = b' ' as u32;
                xmin -= 1;
            }

            if fullwidth {
                if x + 1 == width {
                    ch = b' ' as u32;
                } else {
                    xmax += 1;

                    if x + 2 < width && f.chars[idx + 2] == CACA_MAGIC_FULLWIDTH {
                        f.chars[idx + 2] = b' ' as u32;
                        xmax += 1;
                    }

                    f.chars[idx + 1] = CACA_MAGIC_FULLWIDTH;
                    f.attrs[idx + 1] = attr;
                }
            } else if x + 1 != width && f.chars[idx + 1] == CACA_MAGIC_FULLWIDTH {
                f.chars[idx + 1] = b' ' as u32;
                xmax += 1;
            }

            changed = f.chars[idx] != ch || f.attrs[idx] != attr;
            f.chars[idx] = ch;
            f.attrs[idx] = attr;
        }

        if changed && !self.dirty_disabled_is_on() {
            let _ = self.add_dirty_rect(xmin, y, xmax - xmin + 1, 1);
        }

        ret
    }

    /// Get the UTF-32 character at the given coordinates (space if outside).
    pub fn get_char(&self, x: i32, y: i32) -> u32 {
        if x < 0 || x >= self.width() || y < 0 || y >= self.height() {
            return b' ' as u32;
        }
        self.active().chars[(x + y * self.width()) as usize]
    }

    /// Print an UTF-8 string, returning the number of cells written.
    pub fn put_str(&mut self, x: i32, y: i32, s: &str) -> i32 {
        let width = self.width();
        let height = self.height();
        let mut len = 0i32;

        if y < 0 || y >= height || x >= width {
            for ch in s.chars() {
                len += if utf32_is_fullwidth(ch as u32) { 2 } else { 1 };
            }
            return len;
        }

        for ch in s.chars() {
            let c = ch as u32;
            if x + len >= -1 && x + len < width {
                self.put_char(x + len, y, c);
            }
            len += if utf32_is_fullwidth(c) { 2 } else { 1 };
        }

        len
    }

    /// Print a formatted string, returning the number of cells written.
    pub fn printf(&mut self, x: i32, y: i32, args: core::fmt::Arguments<'_>) -> i32 {
        let s = format!("{}", args);
        self.put_str(x, y, &s)
    }

    /// Clear the canvas using the current attribute.
    pub fn clear(&mut self) {
        let attr = self.active().curattr;
        {
            let f = self.active_mut();
            for c in f.chars.iter_mut() {
                *c = b' ' as u32;
            }
            for a in f.attrs.iter_mut() {
                *a = attr;
            }
        }

        if !self.dirty_disabled_is_on() {
            let (w, h) = (self.width(), self.height());
            let _ = self.add_dirty_rect(0, 0, w, h);
        }
    }

    /// Set the canvas handle used by blitting functions.
    pub fn set_handle(&mut self, x: i32, y: i32) {
        let f = self.active_mut();
        f.handlex = x;
        f.handley = y;
    }

    /// The canvas handle X coordinate.
    pub fn handle_x(&self) -> i32 {
        self.active().handlex
    }

    /// The canvas handle Y coordinate.
    pub fn handle_y(&self) -> i32 {
        self.active().handley
    }

    /// Get the current attribute, or the attribute at `(x, y)` when in bounds.
    pub fn get_attr(&self, x: i32, y: i32) -> Attr {
        if x < 0 || x >= self.width() || y < 0 || y >= self.height() {
            return Attr::from_raw(self.active().curattr);
        }
        Attr::from_raw(self.active().attrs[(x + y * self.width()) as usize])
    }

    /// Set the default attribute.
    ///
    /// Values below `0x10` only change the style bits, leaving colours intact.
    pub fn set_attr(&mut self, attr: Attr) {
        let f = self.active_mut();
        if attr.raw() < 0x0000_0010 {
            f.curattr = (f.curattr & 0xffff_fff0) | attr.raw();
        } else {
            f.curattr = attr.raw();
        }
    }

    /// Unset style flags in the default attribute.
    pub fn unset_attr(&mut self, style: Attr) {
        let f = self.active_mut();
        f.curattr &= !(style.raw() & 0x0000_000f);
    }

    /// Toggle style flags in the default attribute.
    pub fn toggle_attr(&mut self, style: Attr) {
        let f = self.active_mut();
        f.curattr ^= style.raw() & 0x0000_000f;
    }

    /// Set the attribute at the given coordinates (without changing the char).
    pub fn put_attr(&mut self, x: i32, y: i32, attr: Attr) {
        let width = self.width();
        let height = self.height();
        if x < 0 || x >= width || y < 0 || y >= height {
            return;
        }

        let mut xmin = x;
        let mut xmax = x;

        {
            let f = self.active_mut();
            let idx = (x + y * width) as usize;

            if attr.raw() < 0x0000_0010 {
                f.attrs[idx] = (f.attrs[idx] & 0xffff_fff0) | attr.raw();
            } else {
                f.attrs[idx] = attr.raw();
            }

            if x != 0 && f.chars[idx] == CACA_MAGIC_FULLWIDTH {
                f.attrs[idx - 1] = f.attrs[idx];
                xmin -= 1;
            } else if x + 1 < width && f.chars[idx + 1] == CACA_MAGIC_FULLWIDTH {
                f.attrs[idx + 1] = f.attrs[idx];
                xmax += 1;
            }
        }

        if !self.dirty_disabled_is_on() {
            let _ = self.add_dirty_rect(xmin, y, xmax - xmin + 1, 1);
        }
    }

    /// Set the default ANSI colour pair.
    pub fn set_color_ansi(&mut self, fg: Color, bg: Color) -> Result<()> {
        let attr = Attr::from_ansi(fg, bg);
        let f = self.active_mut();
        f.curattr = (f.curattr & 0x0000_000f) | (attr.raw() & 0xffff_fff0);
        Ok(())
    }

    /// Set the default ARGB colour pair.
    pub fn set_color_argb(&mut self, fg: u16, bg: u16) {
        let attr = Attr::from_argb(fg, bg);
        let f = self.active_mut();
        f.curattr = (f.curattr & 0x0000_000f) | (attr.raw() & 0xffff_fff0);
    }

    /// Blit `src` onto this canvas at `(x, y)`, optionally masked.
    pub fn blit(&mut self, x: i32, y: i32, src: &Canvas, mask: Option<&Canvas>) -> Result<()> {
        if let Some(m) = mask {
            if src.width() != m.width() || src.height() != m.height() {
                return Err(CacaError::Invalid);
            }
        }

        let srcf = src.active();
        let dst_w = self.width();
        let dst_h = self.height();
        let src_w = srcf.width;
        let src_h = srcf.height;

        let x = x - srcf.handlex;
        let y = y - srcf.handley;

        let starti = if x < 0 { -x } else { 0 };
        let startj = if y < 0 { -y } else { 0 };
        let endi = if x + src_w >= dst_w { dst_w - x } else { src_w };
        let endj = if y + src_h >= dst_h { dst_h - y } else { src_h };
        let stride = endi - starti;

        if starti > src_w || startj > src_h || starti >= endi || startj >= endj {
            return Ok(());
        }

        let maskf = mask.map(|m| m.active());
        let mut dirty_rects: Vec<(i32, i32, i32, i32)> = Vec::new();

        {
            let dstf = self.active_mut();

            for j in startj..endj {
                let dstix = (j + y) * dst_w + starti + x;
                let srcix = j * src_w + starti;

                if (starti + x) != 0 && dstf.chars[dstix as usize] == CACA_MAGIC_FULLWIDTH {
                    dstf.chars[(dstix - 1) as usize] = b' ' as u32;
                }

                if endi + x < dst_w && dstf.chars[(dstix + stride) as usize] == CACA_MAGIC_FULLWIDTH
                {
                    dstf.chars[(dstix + stride) as usize] = b' ' as u32;
                }

                if let Some(mf) = maskf {
                    for i in 0..stride {
                        if mf.chars[(srcix + i) as usize] == b' ' as u32 {
                            continue;
                        }

                        let di = (dstix + i) as usize;
                        let si = (srcix + i) as usize;
                        if dstf.chars[di] != srcf.chars[si] || dstf.attrs[di] != srcf.attrs[si] {
                            dstf.chars[di] = srcf.chars[si];
                            dstf.attrs[di] = srcf.attrs[si];
                            dirty_rects.push((x + starti + i, y + j, 1, 1));
                        }
                    }
                } else {
                    let dst_slice = dstix as usize;
                    let src_slice = srcix as usize;
                    let stride_usize = stride as usize;
                    let chars_differ = dstf.chars[dst_slice..dst_slice + stride_usize]
                        != srcf.chars[src_slice..src_slice + stride_usize];
                    let attrs_differ = dstf.attrs[dst_slice..dst_slice + stride_usize]
                        != srcf.attrs[src_slice..src_slice + stride_usize];

                    if chars_differ || attrs_differ {
                        dstf.chars[dst_slice..dst_slice + stride_usize]
                            .copy_from_slice(&srcf.chars[src_slice..src_slice + stride_usize]);
                        dstf.attrs[dst_slice..dst_slice + stride_usize]
                            .copy_from_slice(&srcf.attrs[src_slice..src_slice + stride_usize]);
                        dirty_rects.push((x + starti, y + j, stride, 1));
                    }
                }

                // Fix split fullwidth chars.
                if srcf.chars[srcix as usize] == CACA_MAGIC_FULLWIDTH {
                    dstf.chars[dstix as usize] = b' ' as u32;
                }

                if endi < src_w && srcf.chars[endi as usize] == CACA_MAGIC_FULLWIDTH {
                    dstf.chars[(dstix + stride - 1) as usize] = b' ' as u32;
                }
            }
        }

        if !self.dirty_disabled_is_on() {
            for (dx, dy, dw, dh) in dirty_rects {
                let _ = self.add_dirty_rect(dx, dy, dw, dh);
            }
        }

        Ok(())
    }

    /// Crop/expand the canvas to new boundaries. All frames are affected.
    pub fn set_boundaries(&mut self, x: i32, y: i32, w: i32, h: i32) -> Result<()> {
        if self.refcount != 0 {
            return Err(CacaError::Busy);
        }
        if w < 0 || h < 0 {
            return Err(CacaError::Invalid);
        }

        let framecount = self.frames.len();
        let saved_f = self.frame;
        let mut new = Canvas::new(w, h)?;

        for f in 0..framecount {
            if f > 0 {
                new.create_frame(framecount as i32)?;
            }
            self.set_frame(f)?;
            new.set_frame(f)?;
            new.blit(-x, -y, self, None)?;
        }

        self.frames = core::mem::take(&mut new.frames);
        self.frame = saved_f;

        if !self.dirty_disabled_is_on() {
            let (cw, ch) = (self.width(), self.height());
            let _ = self.add_dirty_rect(0, 0, cw, ch);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn put_and_get_char() {
        let mut cv = Canvas::new(5, 5).unwrap();
        cv.put_char(1, 2, b'X' as u32);
        assert_eq!(cv.get_char(1, 2), b'X' as u32);
        assert_eq!(cv.get_char(4, 4), b' ' as u32);
    }

    #[test]
    fn fullwidth_marks_second_cell() {
        let mut cv = Canvas::new(8, 1).unwrap();
        assert_eq!(cv.put_char(0, 0, 0x3000), 2);
        assert_eq!(cv.get_char(0, 0), 0x3000);
        assert_eq!(cv.get_char(1, 0), CACA_MAGIC_FULLWIDTH);
    }

    #[test]
    fn put_str_counts_cells() {
        let mut cv = Canvas::new(10, 1).unwrap();
        let n = cv.put_str(0, 0, "ab");
        assert_eq!(n, 2);
        let n = cv.put_str(0, 0, "\u{3000}");
        assert_eq!(n, 2);
    }

    #[test]
    fn clear_uses_current_attr() {
        let mut cv = Canvas::new(3, 3).unwrap();
        cv.set_color_ansi(Color::Red, Color::Blue).unwrap();
        cv.clear();
        assert_eq!(cv.get_attr(0, 0).to_ansi_fg(), Color::Red.as_u8());
        assert_eq!(cv.get_attr(2, 2).to_ansi_bg(), Color::Blue.as_u8());
    }

    #[test]
    fn blit_copies_region() {
        let mut src = Canvas::new(3, 3).unwrap();
        src.put_str(0, 0, "abc");
        let mut dst = Canvas::new(5, 5).unwrap();
        dst.blit(1, 1, &src, None).unwrap();
        assert_eq!(dst.get_char(1, 1), b'a' as u32);
        assert_eq!(dst.get_char(3, 1), b'c' as u32);
        assert_eq!(dst.get_char(0, 0), b' ' as u32);
    }

    #[test]
    fn boundaries_crop() {
        let mut cv = Canvas::new(6, 6).unwrap();
        cv.put_str(0, 0, "hello");
        cv.set_boundaries(2, 0, 3, 1).unwrap();
        assert_eq!(cv.width(), 3);
        assert_eq!(cv.get_char(0, 0), b'l' as u32);
        assert_eq!(cv.get_char(2, 0), b'o' as u32);
    }
}
