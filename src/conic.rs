//! Circle and ellipse drawing.
//!
//! Port of `caca/conic.c`.

use crate::canvas::Canvas;

impl Canvas {
    /// Draw a circle outline using the given character.
    pub fn draw_circle(&mut self, x: i32, y: i32, r: i32, ch: u32) {
        let mut test = 0;
        let mut dx = 0;
        let mut dy = r;

        while dx <= dy {
            ellipsepoints(self, x, y, dx, dy, ch, false);
            ellipsepoints(self, x, y, dy, dx, ch, false);

            if test > 0 {
                test += dx - dy;
                dy -= 1;
            } else {
                test += dx;
            }
            dx += 1;
        }
    }

    /// Fill an ellipse using the given character.
    pub fn fill_ellipse(&mut self, xo: i32, yo: i32, a: i32, b: i32, ch: u32) {
        let mut x = 0;
        let mut y = b;
        let mut d1 = b * b - (a * a * b) + (a * a / 4);

        while a * a * y - a * a / 2 > b * b * (x + 1) {
            if d1 < 0 {
                d1 += b * b * (2 * x + 1);
            } else {
                d1 += b * b * (2 * x) + a * a * (-2 * y + 2);
                self.draw_line(xo - x, yo - y, xo + x, yo - y, ch);
                self.draw_line(xo - x, yo + y, xo + x, yo + y, ch);
                y -= 1;
            }
            x += 1;
        }

        self.draw_line(xo - x, yo - y, xo + x, yo - y, ch);
        self.draw_line(xo - x, yo + y, xo + x, yo + y, ch);

        let xf = x as f64;
        let yf = y as f64;
        let af = a as f64;
        let bf = b as f64;
        let mut d2 = (bf * bf * (xf + 0.5) * (xf + 0.5) + af * af * (yf - 1.0) * (yf - 1.0)
            - af * af * bf * bf) as i32;

        while y > 0 {
            if d2 < 0 {
                d2 += b * b * (2 * x + 2) + a * a * (-2 * y + 3);
                x += 1;
            } else {
                d2 += a * a * (-2 * y + 3);
            }

            y -= 1;
            self.draw_line(xo - x, yo - y, xo + x, yo - y, ch);
            self.draw_line(xo - x, yo + y, xo + x, yo + y, ch);
        }
    }

    /// Draw an ellipse outline using the given character.
    pub fn draw_ellipse(&mut self, xo: i32, yo: i32, a: i32, b: i32, ch: u32) {
        let mut x = 0;
        let mut y = b;
        let mut d1 = b * b - (a * a * b) + (a * a / 4);

        ellipsepoints(self, xo, yo, x, y, ch, false);

        while a * a * y - a * a / 2 > b * b * (x + 1) {
            if d1 < 0 {
                d1 += b * b * (2 * x + 1);
            } else {
                d1 += b * b * (2 * x) + a * a * (-2 * y + 2);
                y -= 1;
            }
            x += 1;
            ellipsepoints(self, xo, yo, x, y, ch, false);
        }

        let xf = x as f64;
        let yf = y as f64;
        let af = a as f64;
        let bf = b as f64;
        let mut d2 = (bf * bf * (xf + 0.5) * (xf + 0.5) + af * af * (yf - 1.0) * (yf - 1.0)
            - af * af * bf * bf) as i32;

        while y > 0 {
            if d2 < 0 {
                d2 += b * b * (2 * x + 2) + a * a * (-2 * y + 3);
                x += 1;
            } else {
                d2 += a * a * (-2 * y + 3);
            }

            y -= 1;
            ellipsepoints(self, xo, yo, x, y, ch, false);
        }
    }

    /// Draw a thin ASCII-art ellipse.
    pub fn draw_thin_ellipse(&mut self, xo: i32, yo: i32, a: i32, b: i32) {
        let mut x = 0;
        let mut y = b;
        let mut d1 = b * b - (a * a * b) + (a * a / 4);

        ellipsepoints(self, xo, yo, x, y, b'-' as u32, true);

        while a * a * y - a * a / 2 > b * b * (x + 1) {
            if d1 < 0 {
                d1 += b * b * (2 * x + 1);
                ellipsepoints(self, xo, yo, x + 1, y, b'0' as u32, true);
            } else {
                d1 += b * b * (2 * x) + a * a * (-2 * y + 2);
                y -= 1;
                ellipsepoints(self, xo, yo, x + 1, y, b'1' as u32, true);
            }
            x += 1;
        }

        let xf = x as f64;
        let yf = y as f64;
        let af = a as f64;
        let bf = b as f64;
        let mut d2 = (bf * bf * (xf + 0.5) * (xf + 0.5) + af * af * (yf - 1.0) * (yf - 1.0)
            - af * af * bf * bf) as i32;

        while y > 0 {
            if d2 < 0 {
                d2 += b * b * (2 * x + 2) + a * a * (-2 * y + 3);
                x += 1;
                ellipsepoints(self, xo, yo, x, y - 1, b'2' as u32, true);
            } else {
                d2 += a * a * (-2 * y + 3);
                ellipsepoints(self, xo, yo, x, y - 1, b'3' as u32, true);
            }

            y -= 1;
        }
    }
}

fn thin_char(base: u32, quadrant: u8) -> u32 {
    // quadrant: 0 = +x+y, 1 = -x+y, 2 = +x-y, 3 = -x-y
    match base {
        x if x == b'0' as u32 => b'-' as u32,
        x if x == b'1' as u32 => match quadrant {
            0 => b',' as u32,
            1 => b'.' as u32,
            2 => b'`' as u32,
            _ => b'\'' as u32,
        },
        x if x == b'2' as u32 => match quadrant {
            0 => b'/' as u32,
            1 => b'\\' as u32,
            2 => b'\\' as u32,
            _ => b'/' as u32,
        },
        x if x == b'3' as u32 => b'|' as u32,
        _ => base,
    }
}

fn ellipsepoints(cv: &mut Canvas, xo: i32, yo: i32, x: i32, y: i32, ch: u32, thin: bool) {
    let mut b = 0u8;

    if xo + x >= 0 && xo + x < cv.width() {
        b |= 0x1;
    }
    if xo - x >= 0 && xo - x < cv.width() {
        b |= 0x2;
    }
    if yo + y >= 0 && yo + y < cv.height() {
        b |= 0x4;
    }
    if yo - y >= 0 && yo - y < cv.height() {
        b |= 0x8;
    }

    if (b & (0x1 | 0x4)) == (0x1 | 0x4) {
        let c = if thin { thin_char(ch, 0) } else { ch };
        cv.put_char(xo + x, yo + y, c);
    }
    if (b & (0x2 | 0x4)) == (0x2 | 0x4) {
        let c = if thin { thin_char(ch, 1) } else { ch };
        cv.put_char(xo - x, yo + y, c);
    }
    if (b & (0x1 | 0x8)) == (0x1 | 0x8) {
        let c = if thin { thin_char(ch, 2) } else { ch };
        cv.put_char(xo + x, yo - y, c);
    }
    if (b & (0x2 | 0x8)) == (0x2 | 0x8) {
        let c = if thin { thin_char(ch, 3) } else { ch };
        cv.put_char(xo - x, yo - y, c);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn circle_has_extremes() {
        let mut cv = Canvas::new(21, 21).unwrap();
        cv.draw_circle(10, 10, 8, b'#' as u32);
        assert_eq!(cv.get_char(18, 10), b'#' as u32);
        assert_eq!(cv.get_char(2, 10), b'#' as u32);
        assert_eq!(cv.get_char(10, 18), b'#' as u32);
        assert_eq!(cv.get_char(10, 2), b'#' as u32);
    }

    #[test]
    fn filled_ellipse_center() {
        let mut cv = Canvas::new(21, 21).unwrap();
        cv.fill_ellipse(10, 10, 8, 5, b'#' as u32);
        assert_eq!(cv.get_char(10, 10), b'#' as u32);
    }
}
