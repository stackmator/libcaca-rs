//! Box drawing.
//!
//! Port of `caca/box.c`.

use crate::canvas::Canvas;

impl Canvas {
    /// Draw a box outline using the given character.
    pub fn draw_box(&mut self, x: i32, y: i32, w: i32, h: i32, ch: u32) {
        let x2 = x + w - 1;
        let y2 = y + h - 1;

        self.draw_line(x, y, x, y2, ch);
        self.draw_line(x, y2, x2, y2, ch);
        self.draw_line(x2, y2, x2, y, ch);
        self.draw_line(x2, y, x, y, ch);
    }

    /// Draw an ASCII-art thin box.
    pub fn draw_thin_box(&mut self, x: i32, y: i32, w: i32, h: i32) {
        const CHARS: [u32; 6] = [
            b'-' as u32,
            b'|' as u32,
            b',' as u32,
            b'`' as u32,
            b'.' as u32,
            b'\'' as u32,
        ];
        draw_box_chars(self, x, y, w, h, &CHARS);
    }

    /// Draw a box using CP437 line-drawing characters.
    pub fn draw_cp437_box(&mut self, x: i32, y: i32, w: i32, h: i32) {
        const CHARS: [u32; 6] = [0x2500, 0x2502, 0x250c, 0x2514, 0x2510, 0x2518];
        draw_box_chars(self, x, y, w, h, &CHARS);
    }

    /// Fill a box with the given character.
    pub fn fill_box(&mut self, x: i32, y: i32, w: i32, h: i32, ch: u32) {
        let mut x = x;
        let mut y = y;
        let mut x2 = x + w - 1;
        let mut y2 = y + h - 1;

        if x > x2 {
            core::mem::swap(&mut x, &mut x2);
        }
        if y > y2 {
            core::mem::swap(&mut y, &mut y2);
        }

        let xmax = self.width() - 1;
        let ymax = self.height() - 1;

        if x2 < 0 || y2 < 0 || x > xmax || y > ymax {
            return;
        }

        if x < 0 {
            x = 0;
        }
        if y < 0 {
            y = 0;
        }
        if x2 > xmax {
            x2 = xmax;
        }
        if y2 > ymax {
            y2 = ymax;
        }

        for j in y..=y2 {
            for i in x..=x2 {
                self.put_char(i, j, ch);
            }
        }
    }
}

fn draw_box_chars(cv: &mut Canvas, x: i32, y: i32, w: i32, h: i32, chars: &[u32; 6]) {
    let mut x = x;
    let mut y = y;
    let mut x2 = x + w - 1;
    let mut y2 = y + h - 1;

    if x > x2 {
        core::mem::swap(&mut x, &mut x2);
    }
    if y > y2 {
        core::mem::swap(&mut y, &mut y2);
    }

    let xmax = cv.width() - 1;
    let ymax = cv.height() - 1;

    if x2 < 0 || y2 < 0 || x > xmax || y > ymax {
        return;
    }

    // Top edge.
    if y >= 0 {
        let start = if x < 0 { 1 } else { x + 1 };
        let mut i = start;
        while i < x2 && i < xmax {
            cv.put_char(i, y, chars[0]);
            i += 1;
        }
    }

    // Bottom edge.
    if y2 <= ymax {
        let start = if x < 0 { 1 } else { x + 1 };
        let mut i = start;
        while i < x2 && i < xmax {
            cv.put_char(i, y2, chars[0]);
            i += 1;
        }
    }

    // Left edge.
    if x >= 0 {
        let start = if y < 0 { 1 } else { y + 1 };
        let mut j = start;
        while j < y2 && j < ymax {
            cv.put_char(x, j, chars[1]);
            j += 1;
        }
    }

    // Right edge.
    if x2 <= xmax {
        let start = if y < 0 { 1 } else { y + 1 };
        let mut j = start;
        while j < y2 && j < ymax {
            cv.put_char(x2, j, chars[1]);
            j += 1;
        }
    }

    // Corners.
    cv.put_char(x, y, chars[2]);
    cv.put_char(x, y2, chars[3]);
    cv.put_char(x2, y, chars[4]);
    cv.put_char(x2, y2, chars[5]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cp437_box_corners() {
        let mut cv = Canvas::new(5, 4).unwrap();
        cv.draw_cp437_box(1, 1, 3, 2);
        assert_eq!(cv.get_char(1, 1), 0x250c);
        assert_eq!(cv.get_char(3, 1), 0x2510);
        assert_eq!(cv.get_char(1, 2), 0x2514);
        assert_eq!(cv.get_char(3, 2), 0x2518);
        assert_eq!(cv.get_char(2, 1), 0x2500);
    }

    #[test]
    fn fill_box_clips() {
        let mut cv = Canvas::new(3, 3).unwrap();
        cv.fill_box(-1, -1, 3, 3, b'#' as u32);
        assert_eq!(cv.get_char(0, 0), b'#' as u32);
        assert_eq!(cv.get_char(1, 1), b'#' as u32);
        assert_eq!(cv.get_char(2, 2), b' ' as u32);
    }
}
