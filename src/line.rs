//! Line and polyline drawing.
//!
//! Port of `caca/line.c`.

use crate::canvas::Canvas;

#[derive(Clone, Copy)]
struct Line {
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
    ch: u32,
    thin: bool,
}

impl Canvas {
    /// Draw a line using the given character.
    pub fn draw_line(&mut self, x1: i32, y1: i32, x2: i32, y2: i32, ch: u32) {
        let mut s = Line {
            x1,
            y1,
            x2,
            y2,
            ch,
            thin: false,
        };
        clip_line(self, &mut s);
    }

    /// Draw a polyline using the given characters and coordinate arrays.
    ///
    /// `x` and `y` must have at least `n + 1` elements.
    pub fn draw_polyline(&mut self, x: &[i32], y: &[i32], n: usize, ch: u32) {
        for i in 0..n {
            let mut s = Line {
                x1: x[i],
                y1: y[i],
                x2: x[i + 1],
                y2: y[i + 1],
                ch,
                thin: false,
            };
            clip_line(self, &mut s);
        }
    }

    /// Draw a thin ASCII-art line.
    pub fn draw_thin_line(&mut self, x1: i32, y1: i32, x2: i32, y2: i32) {
        let mut s = Line {
            x1,
            y1,
            x2,
            y2,
            ch: 0,
            thin: true,
        };
        clip_line(self, &mut s);
    }

    /// Draw a thin ASCII-art polyline.
    pub fn draw_thin_polyline(&mut self, x: &[i32], y: &[i32], n: usize) {
        for i in 0..n {
            let mut s = Line {
                x1: x[i],
                y1: y[i],
                x2: x[i + 1],
                y2: y[i + 1],
                ch: 0,
                thin: true,
            };
            clip_line(self, &mut s);
        }
    }
}

/// Cohen-Sutherland line clipping.
fn clip_line(cv: &mut Canvas, s: &mut Line) {
    let bits1 = clip_bits(cv, s.x1, s.y1);
    let bits2 = clip_bits(cv, s.x2, s.y2);

    if bits1 & bits2 != 0 {
        return;
    }

    if bits1 == 0 {
        if bits2 == 0 {
            draw(cv, s);
        } else {
            core::mem::swap(&mut s.x1, &mut s.x2);
            core::mem::swap(&mut s.y1, &mut s.y2);
            clip_line(cv, s);
        }
        return;
    }

    if bits1 & (1 << 0) != 0 {
        s.y1 = s.y2 - s.x2 * (s.y2 - s.y1) / (s.x2 - s.x1);
        s.x1 = 0;
    } else if bits1 & (1 << 1) != 0 {
        let xmax = cv.width() - 1;
        s.y1 = s.y2 - (s.x2 - xmax) * (s.y2 - s.y1) / (s.x2 - s.x1);
        s.x1 = xmax;
    } else if bits1 & (1 << 2) != 0 {
        s.x1 = s.x2 - s.y2 * (s.x2 - s.x1) / (s.y2 - s.y1);
        s.y1 = 0;
    } else if bits1 & (1 << 3) != 0 {
        let ymax = cv.height() - 1;
        s.x1 = s.x2 - (s.y2 - ymax) * (s.x2 - s.x1) / (s.y2 - s.y1);
        s.y1 = ymax;
    }

    clip_line(cv, s);
}

fn clip_bits(cv: &Canvas, x: i32, y: i32) -> u8 {
    let mut b = 0u8;

    if x < 0 {
        b |= 1 << 0;
    } else if x >= cv.width() {
        b |= 1 << 1;
    }

    if y < 0 {
        b |= 1 << 2;
    } else if y >= cv.height() {
        b |= 1 << 3;
    }

    b
}

fn draw(cv: &mut Canvas, s: &Line) {
    if s.thin {
        draw_thin_line(cv, s);
    } else {
        draw_solid_line(cv, s);
    }
}

/// Bresenham mid-point line scan-conversion.
fn draw_solid_line(cv: &mut Canvas, s: &Line) {
    let mut x1 = s.x1;
    let mut y1 = s.y1;
    let x2 = s.x2;
    let y2 = s.y2;

    let mut dx = (x2 - x1).abs();
    let mut dy = (y2 - y1).abs();

    let xinc = if x1 > x2 { -1 } else { 1 };
    let yinc = if y1 > y2 { -1 } else { 1 };

    if dx >= dy {
        let dpr = dy << 1;
        let dpru = dpr - (dx << 1);
        let mut delta = dpr - dx;

        while dx >= 0 {
            cv.put_char(x1, y1, s.ch);
            if delta > 0 {
                x1 += xinc;
                y1 += yinc;
                delta += dpru;
            } else {
                x1 += xinc;
                delta += dpr;
            }
            dx -= 1;
        }
    } else {
        let dpr = dx << 1;
        let dpru = dpr - (dy << 1);
        let mut delta = dpr - dy;

        while dy >= 0 {
            cv.put_char(x1, y1, s.ch);
            if delta > 0 {
                x1 += xinc;
                y1 += yinc;
                delta += dpru;
            } else {
                y1 += yinc;
                delta += dpr;
            }
            dy -= 1;
        }
    }
}

/// Thin ASCII-art line drawing.
fn draw_thin_line(cv: &mut Canvas, s: &Line) {
    let mut charmapx = [0u32; 2];
    let mut charmapy = [0u32; 2];
    let (mut x1, mut y1, x2, y2);
    let yinc;

    if s.x2 >= s.x1 {
        charmapx[0] = if s.y1 > s.y2 { b',' as u32 } else { b'`' as u32 };
        charmapx[1] = if s.y1 > s.y2 { b'\'' as u32 } else { b'.' as u32 };
        x1 = s.x1;
        y1 = s.y1;
        x2 = s.x2;
        y2 = s.y2;
    } else {
        charmapx[0] = if s.y1 > s.y2 { b'`' as u32 } else { b'.' as u32 };
        charmapx[1] = if s.y1 > s.y2 { b',' as u32 } else { b'\'' as u32 };
        x1 = s.x2;
        y1 = s.y2;
        x2 = s.x1;
        y2 = s.y1;
    }

    let dx = (x2 - x1).abs();
    let dy = (y2 - y1).abs();

    if y1 > y2 {
        charmapy[0] = b',' as u32;
        charmapy[1] = b'\'' as u32;
        yinc = -1;
    } else {
        yinc = 1;
        charmapy[0] = b'`' as u32;
        charmapy[1] = b'.' as u32;
    }

    if dx >= dy {
        let dpr = dy << 1;
        let dpru = dpr - (dx << 1);
        let mut delta = dpr - dx;
        let mut prev = false;
        let mut rem = dx;

        while rem >= 0 {
            if delta > 0 {
                cv.put_char(x1, y1, charmapy[1]);
                x1 += 1;
                y1 += yinc;
                delta += dpru;
                prev = true;
            } else {
                if prev {
                    cv.put_char(x1, y1, charmapy[0]);
                } else {
                    cv.put_char(x1, y1, b'-' as u32);
                }
                x1 += 1;
                delta += dpr;
                prev = false;
            }
            rem -= 1;
        }
    } else {
        let dpr = dx << 1;
        let dpru = dpr - (dy << 1);
        let mut delta = dpr - dy;
        let mut rem = dy;

        while rem >= 0 {
            if delta > 0 {
                cv.put_char(x1, y1, charmapx[0]);
                cv.put_char(x1 + 1, y1, charmapx[1]);
                x1 += 1;
                y1 += yinc;
                delta += dpru;
            } else {
                cv.put_char(x1, y1, b'|' as u32);
                y1 += yinc;
                delta += dpr;
            }
            rem -= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn horizontal_line() {
        let mut cv = Canvas::new(10, 3).unwrap();
        cv.draw_line(1, 1, 5, 1, b'-' as u32);
        for x in 1..=5 {
            assert_eq!(cv.get_char(x, 1), b'-' as u32);
        }
    }

    #[test]
    fn clipping_outside() {
        let mut cv = Canvas::new(5, 5).unwrap();
        cv.draw_line(-10, 2, 20, 2, b'#' as u32);
        assert_eq!(cv.get_char(0, 2), b'#' as u32);
        assert_eq!(cv.get_char(4, 2), b'#' as u32);
    }

    #[test]
    fn thin_line_uses_ascii_art() {
        let mut cv = Canvas::new(10, 3).unwrap();
        cv.draw_thin_line(0, 0, 9, 0);
        assert_eq!(cv.get_char(0, 0), b'-' as u32);
    }
}
