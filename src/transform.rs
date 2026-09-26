//! Canvas transformations: invert, flip, flop, rotate and stretch.
//!
//! Port of `caca/transform.c`. The large character lookup tables are
//! generated from the C source into the private `transform_tables` module.

use alloc::vec;

use crate::canvas::{Canvas, CACA_MAGIC_FULLWIDTH};
use crate::error::{CacaError, Result};
use crate::transform_tables::{
    FLIP_NOFLIP, FLIP_PAIRS, FLOP_NOFLOP, FLOP_PAIRS, LEFTRIGHT2, LEFTRIGHT2X2, LEFTRIGHT2X4,
    LEFTRIGHT4, ROTATE_NOROTATE, ROTATE_PAIRS,
};

impl Canvas {
    /// Invert the canvas colours (XOR the attribute colour nibbles).
    pub fn invert(&mut self) {
        {
            let f = self.active_mut();
            for a in f.attrs.iter_mut() {
                *a ^= 0x000f_000f;
            }
        }
        self.mark_all_dirty();
    }

    /// Flip the canvas horizontally, mirroring characters where possible.
    pub fn flip(&mut self) {
        let w = self.width();
        let h = self.height();
        let f = self.active_mut();

        for y in 0..h {
            let row = (y * w) as usize;
            let mut left = row;
            let mut right = row + w as usize - 1;

            while left < right {
                f.attrs.swap(left, right);
                let ch = f.chars[right];
                f.chars[right] = flipchar(f.chars[left]);
                f.chars[left] = flipchar(ch);
                left += 1;
                right -= 1;
            }

            if left == right {
                f.chars[left] = flipchar(f.chars[left]);
            }
        }

        // Fix fullwidth characters.
        for y in 0..h {
            let row = (y * w) as usize;
            let mut i = 0usize;
            while i + 1 < w as usize {
                if f.chars[row + i] == CACA_MAGIC_FULLWIDTH {
                    f.chars[row + i] = f.chars[row + i + 1];
                    f.chars[row + i + 1] = CACA_MAGIC_FULLWIDTH;
                    i += 1;
                }
                i += 1;
            }
        }

        self.mark_all_dirty();
    }

    /// Flip the canvas vertically.
    pub fn flop(&mut self) {
        let w = self.width();
        let h = self.height();
        let f = self.active_mut();

        for x in 0..w {
            let mut top = x as usize;
            let mut bottom = x as usize + w as usize * (h as usize - 1);

            while top < bottom {
                f.attrs.swap(top, bottom);
                let ch = f.chars[bottom];
                f.chars[bottom] = flopchar(f.chars[top]);
                f.chars[top] = flopchar(ch);
                top += w as usize;
                bottom -= w as usize;
            }

            if top == bottom {
                f.chars[top] = flopchar(f.chars[top]);
            }
        }

        self.mark_all_dirty();
    }

    /// Rotate the canvas 180 degrees.
    pub fn rotate_180(&mut self) {
        let w = self.width();
        let h = self.height();
        let f = self.active_mut();

        if f.chars.is_empty() {
            return;
        }

        let mut begin = 0usize;
        let mut end = f.chars.len() - 1;

        while begin < end {
            f.attrs.swap(begin, end);
            let ch = f.chars[end];
            f.chars[end] = rotatechar(f.chars[begin]);
            f.chars[begin] = rotatechar(ch);
            begin += 1;
            end -= 1;
        }

        if begin == end {
            f.chars[begin] = rotatechar(f.chars[begin]);
        }

        // Fix fullwidth characters.
        for y in 0..h {
            let row = (y * w) as usize;
            let mut i = 0usize;
            while i + 1 < w as usize {
                if f.chars[row + i] == CACA_MAGIC_FULLWIDTH {
                    f.chars[row + i] = f.chars[row + i + 1];
                    f.chars[row + i + 1] = CACA_MAGIC_FULLWIDTH;
                    i += 1;
                }
                i += 1;
            }
        }

        self.mark_all_dirty();
    }

    /// Rotate the canvas 90 degrees counterclockwise, two cells at a time.
    pub fn rotate_left(&mut self) -> Result<()> {
        if self.refcount != 0 {
            return Err(CacaError::Busy);
        }

        let old_w = self.width();
        let old_h = self.height();
        let w2 = (old_w + 1) / 2;
        let h2 = old_h;
        let new_w = old_h * 2;
        let new_h = (old_w + 1) / 2;

        let (old_chars, old_attrs) = {
            let f = self.active_mut();
            (core::mem::take(&mut f.chars), core::mem::take(&mut f.attrs))
        };

        let size = (new_w as usize) * (new_h as usize);
        let mut newchars = vec![b' ' as u32; size];
        let mut newattrs = vec![0u32; size];

        for y in 0..h2 {
            for x in 0..w2 {
                let i0 = (old_w * y + x * 2) as usize;
                let mut pair = [old_chars[i0], 0u32];
                let mut attr1 = old_attrs[i0];
                let mut attr2;

                if (old_w & 1) != 0 && x == w2 - 1 {
                    pair[1] = b' ' as u32;
                    attr2 = attr1;
                } else {
                    let i1 = (old_w * y + x * 2 + 1) as usize;
                    pair[1] = old_chars[i1];
                    attr2 = old_attrs[i1];
                }

                if pair[0] == b' ' as u32 {
                    attr1 = attr2;
                } else if pair[1] == b' ' as u32 {
                    attr2 = attr1;
                }

                leftpair(&mut pair);

                let idx = ((h2 * (w2 - 1 - x) + y) * 2) as usize;
                newchars[idx] = pair[0];
                newattrs[idx] = attr1;
                newchars[idx + 1] = pair[1];
                newattrs[idx + 1] = attr2;
            }
        }

        {
            let f = self.active_mut();
            let x = f.x;
            let y = f.y;
            f.x = y * 2;
            f.y = (old_w - 1 - x) / 2;

            let hx = f.handlex;
            let hy = f.handley;
            f.handlex = hy * 2;
            f.handley = (old_w - 1 - hx) / 2;

            f.width = new_w;
            f.height = new_h;
            f.chars = newchars;
            f.attrs = newattrs;
        }

        self.mark_all_dirty();
        Ok(())
    }

    /// Rotate the canvas 90 degrees clockwise, two cells at a time.
    pub fn rotate_right(&mut self) -> Result<()> {
        if self.refcount != 0 {
            return Err(CacaError::Busy);
        }

        let old_w = self.width();
        let old_h = self.height();
        let w2 = (old_w + 1) / 2;
        let h2 = old_h;
        let new_w = old_h * 2;
        let new_h = (old_w + 1) / 2;

        let (old_chars, old_attrs) = {
            let f = self.active_mut();
            (core::mem::take(&mut f.chars), core::mem::take(&mut f.attrs))
        };

        let size = (new_w as usize) * (new_h as usize);
        let mut newchars = vec![b' ' as u32; size];
        let mut newattrs = vec![0u32; size];

        for y in 0..h2 {
            for x in 0..w2 {
                let i0 = (old_w * y + x * 2) as usize;
                let mut pair = [old_chars[i0], 0u32];
                let mut attr1 = old_attrs[i0];
                let mut attr2;

                if (old_w & 1) != 0 && x == w2 - 1 {
                    pair[1] = b' ' as u32;
                    attr2 = attr1;
                } else {
                    let i1 = (old_w * y + x * 2 + 1) as usize;
                    pair[1] = old_chars[i1];
                    attr2 = old_attrs[i1];
                }

                if pair[0] == b' ' as u32 {
                    attr1 = attr2;
                } else if pair[1] == b' ' as u32 {
                    attr2 = attr1;
                }

                rightpair(&mut pair);

                let idx = ((h2 * x + h2 - 1 - y) * 2) as usize;
                newchars[idx] = pair[0];
                newattrs[idx] = attr1;
                newchars[idx + 1] = pair[1];
                newattrs[idx + 1] = attr2;
            }
        }

        {
            let f = self.active_mut();
            let x = f.x;
            let y = f.y;
            f.x = (old_h - 1 - y) * 2;
            f.y = x / 2;

            let hx = f.handlex;
            let hy = f.handley;
            f.handlex = (old_h - 1 - hy) * 2;
            f.handley = hx / 2;

            f.width = new_w;
            f.height = new_h;
            f.chars = newchars;
            f.attrs = newattrs;
        }

        self.mark_all_dirty();
        Ok(())
    }

    /// Rotate and stretch the canvas 90 degrees counterclockwise.
    pub fn stretch_left(&mut self) -> Result<()> {
        if self.refcount != 0 {
            return Err(CacaError::Busy);
        }

        let old_w = self.width();
        let old_h = self.height();

        let (old_chars, old_attrs) = {
            let f = self.active_mut();
            (core::mem::take(&mut f.chars), core::mem::take(&mut f.attrs))
        };

        let size = (old_w as usize) * (old_h as usize);
        let mut newchars = vec![b' ' as u32; size];
        let mut newattrs = vec![0u32; size];

        for y in 0..old_h {
            for x in 0..old_w {
                let i = (old_w * y + x) as usize;
                let ch = leftchar(old_chars[i]);
                let attr = old_attrs[i];
                let idx = (old_h * (old_w - 1 - x) + y) as usize;
                newchars[idx] = ch;
                newattrs[idx] = attr;
            }
        }

        {
            let f = self.active_mut();
            let x = f.x;
            let y = f.y;
            f.x = y;
            f.y = old_w - 1 - x;

            let hx = f.handlex;
            let hy = f.handley;
            f.handlex = hy;
            f.handley = old_w - 1 - hx;

            f.width = old_h;
            f.height = old_w;
            f.chars = newchars;
            f.attrs = newattrs;
        }

        let (w, h) = (self.width(), self.height());
        let _ = self.add_dirty_rect(0, 0, w, h);
        Ok(())
    }

    /// Rotate and stretch the canvas 90 degrees clockwise.
    pub fn stretch_right(&mut self) -> Result<()> {
        if self.refcount != 0 {
            return Err(CacaError::Busy);
        }

        let old_w = self.width();
        let old_h = self.height();

        let (old_chars, old_attrs) = {
            let f = self.active_mut();
            (core::mem::take(&mut f.chars), core::mem::take(&mut f.attrs))
        };

        let size = (old_w as usize) * (old_h as usize);
        let mut newchars = vec![b' ' as u32; size];
        let mut newattrs = vec![0u32; size];

        for y in 0..old_h {
            for x in 0..old_w {
                let i = (old_w * y + x) as usize;
                let ch = rightchar(old_chars[i]);
                let attr = old_attrs[i];
                let idx = (old_h * x + old_h - 1 - y) as usize;
                newchars[idx] = ch;
                newattrs[idx] = attr;
            }
        }

        {
            let f = self.active_mut();
            let x = f.x;
            let y = f.y;
            f.x = old_h - 1 - y;
            f.y = x;

            let hx = f.handlex;
            let hy = f.handley;
            f.handlex = old_h - 1 - hy;
            f.handley = hx;

            f.width = old_h;
            f.height = old_w;
            f.chars = newchars;
            f.attrs = newattrs;
        }

        let (w, h) = (self.width(), self.height());
        let _ = self.add_dirty_rect(0, 0, w, h);
        Ok(())
    }

    fn mark_all_dirty(&mut self) {
        if !self.dirty_disabled_is_on() {
            let (w, h) = (self.width(), self.height());
            let _ = self.add_dirty_rect(0, 0, w, h);
        }
    }
}

fn lookup_pairs(ch: u32, table: &[u32]) -> u32 {
    let mut i = 0;
    while table[i] != 0 {
        if ch == table[i] {
            return table[i ^ 1];
        }
        i += 1;
    }
    ch
}

fn flipchar(ch: u32) -> u32 {
    for &n in FLIP_NOFLIP.iter() {
        if ch == n {
            return ch;
        }
    }
    lookup_pairs(ch, &FLIP_PAIRS)
}

fn flopchar(ch: u32) -> u32 {
    for &n in FLOP_NOFLOP.iter() {
        if ch == n {
            return ch;
        }
    }
    lookup_pairs(ch, &FLOP_PAIRS)
}

fn rotatechar(ch: u32) -> u32 {
    for &n in ROTATE_NOROTATE.iter() {
        if ch == n {
            return ch;
        }
    }
    lookup_pairs(ch, &ROTATE_PAIRS)
}

fn leftchar(ch: u32) -> u32 {
    let mut i = 0;
    while LEFTRIGHT2[i] != 0 {
        if ch == LEFTRIGHT2[i] {
            return LEFTRIGHT2[(i & !1) | ((i + 1) & 1)];
        }
        i += 1;
    }
    let mut i = 0;
    while LEFTRIGHT4[i] != 0 {
        if ch == LEFTRIGHT4[i] {
            return LEFTRIGHT4[(i & !3) | ((i + 1) & 3)];
        }
        i += 1;
    }
    ch
}

fn rightchar(ch: u32) -> u32 {
    let mut i = 0;
    while LEFTRIGHT2[i] != 0 {
        if ch == LEFTRIGHT2[i] {
            return LEFTRIGHT2[(i & !1) | ((i.wrapping_sub(1)) & 1)];
        }
        i += 1;
    }
    let mut i = 0;
    while LEFTRIGHT4[i] != 0 {
        if ch == LEFTRIGHT4[i] {
            return LEFTRIGHT4[(i & !3) | ((i.wrapping_sub(1)) & 3)];
        }
        i += 1;
    }
    ch
}

fn leftpair(pair: &mut [u32; 2]) {
    let mut i = 0;
    while LEFTRIGHT2X2[i] != 0 {
        if pair[0] == LEFTRIGHT2X2[i] && pair[1] == LEFTRIGHT2X2[i + 1] {
            let idx = (i & !3) | ((i + 2) & 3);
            pair[0] = LEFTRIGHT2X2[idx];
            pair[1] = LEFTRIGHT2X2[idx + 1];
            return;
        }
        i += 2;
    }

    let mut i = 0;
    while LEFTRIGHT2X4[i] != 0 {
        if pair[0] == LEFTRIGHT2X4[i] && pair[1] == LEFTRIGHT2X4[i + 1] {
            let idx = (i & !7) | ((i + 2) & 7);
            pair[0] = LEFTRIGHT2X4[idx];
            pair[1] = LEFTRIGHT2X4[idx + 1];
            return;
        }
        i += 2;
    }
}

fn rightpair(pair: &mut [u32; 2]) {
    let mut i = 0;
    while LEFTRIGHT2X2[i] != 0 {
        if pair[0] == LEFTRIGHT2X2[i] && pair[1] == LEFTRIGHT2X2[i + 1] {
            let idx = (i & !3) | ((i.wrapping_sub(2)) & 3);
            pair[0] = LEFTRIGHT2X2[idx];
            pair[1] = LEFTRIGHT2X2[idx + 1];
            return;
        }
        i += 2;
    }

    let mut i = 0;
    while LEFTRIGHT2X4[i] != 0 {
        if pair[0] == LEFTRIGHT2X4[i] && pair[1] == LEFTRIGHT2X4[i + 1] {
            let idx = (i & !7) | ((i.wrapping_sub(2)) & 7);
            pair[0] = LEFTRIGHT2X4[idx];
            pair[1] = LEFTRIGHT2X4[idx + 1];
            return;
        }
        i += 2;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flip_mirrors_layout() {
        let mut cv = Canvas::new(3, 2).unwrap();

        cv.put_char(0, 0, b'A' as u32);
        cv.put_char(1, 0, b'H' as u32);
        cv.put_char(2, 0, b'M' as u32);

        cv.put_char(0, 1, b'(' as u32);
        cv.put_char(1, 1, b' ' as u32);
        cv.put_char(2, 1, b' ' as u32);

        cv.flip();

        assert_eq!(cv.get_char(0, 0), b'M' as u32);
        assert_eq!(cv.get_char(1, 0), b'H' as u32);
        assert_eq!(cv.get_char(2, 0), b'A' as u32);

        assert_eq!(cv.get_char(0, 1), b' ' as u32);
        assert_eq!(cv.get_char(1, 1), b' ' as u32);
        assert_eq!(cv.get_char(2, 1), b')' as u32);
    }

    #[test]
    fn flip_is_involutive_ascii() {
        let mut cv = Canvas::new(4, 1).unwrap();
        cv.put_str(0, 0, "abcd");
        cv.flip();
        assert_eq!(cv.get_char(0, 0), flipchar(b'd' as u32));
        cv.flip();
        assert_eq!(cv.get_char(0, 0), b'a' as u32);
    }

    #[test]
    fn invert_swaps_colour_nibbles() {
        let mut cv = Canvas::new(1, 1).unwrap();
        cv.set_color_ansi(crate::attr::Color::Black, crate::attr::Color::White)
            .unwrap();
        let before = cv.get_attr(0, 0).raw();
        cv.invert();
        let after = cv.get_attr(0, 0).raw();
        assert_eq!(after, before ^ 0x000f_000f);
    }

    #[test]
    fn stretch_left_swaps_dimensions() {
        let mut cv = Canvas::new(4, 2).unwrap();
        cv.stretch_left().unwrap();
        assert_eq!(cv.width(), 2);
        assert_eq!(cv.height(), 4);
    }

    #[test]
    fn rotate_left_swaps_dimensions() {
        let mut cv = Canvas::new(4, 3).unwrap();
        cv.rotate_left().unwrap();
        assert_eq!(cv.width(), 6);
        assert_eq!(cv.height(), 2);
    }
}
