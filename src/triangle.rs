//! Triangle drawing.
//!
//! Port of `caca/triangle.c`.
#![allow(clippy::too_many_arguments)]

use crate::canvas::Canvas;

impl Canvas {
    /// Draw a triangle outline using the given character.
    pub fn draw_triangle(
        &mut self,
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        x3: i32,
        y3: i32,
        ch: u32,
    ) {
        self.draw_line(x1, y1, x2, y2, ch);
        self.draw_line(x2, y2, x3, y3, ch);
        self.draw_line(x3, y3, x1, y1, ch);
    }

    /// Draw a thin ASCII-art triangle.
    pub fn draw_thin_triangle(
        &mut self,
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        x3: i32,
        y3: i32,
    ) {
        self.draw_thin_line(x1, y1, x2, y2);
        self.draw_thin_line(x2, y2, x3, y3);
        self.draw_thin_line(x3, y3, x1, y1);
    }

    /// Fill a triangle using the given character.
    #[allow(clippy::too_many_arguments)]
    pub fn fill_triangle(
        &mut self,
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        x3: i32,
        y3: i32,
        ch: u32,
    ) {
        // Bubble-sort y1 <= y2 <= y3.
        if y1 > y2 {
            self.fill_triangle(x2, y2, x1, y1, x3, y3, ch);
            return;
        }
        if y2 > y3 {
            self.fill_triangle(x1, y1, x3, y3, x2, y2, ch);
            return;
        }

        let sl21 = if y2 == y1 {
            0
        } else {
            (x2 - x1) * 0x10000 / (y2 - y1)
        };
        let sl31 = if y3 == y1 {
            0
        } else {
            (x3 - x1) * 0x10000 / (y3 - y1)
        };
        let sl32 = if y3 == y2 {
            0
        } else {
            (x3 - x2) * 0x10000 / (y3 - y2)
        };

        let x1 = x1 * 0x10000;
        let x2 = x2 * 0x10000;
        let x3 = x3 * 0x10000;

        let ymin = if y1 < 0 { 0 } else { y1 };
        let ymax = if y3 + 1 < self.height() {
            y3 + 1
        } else {
            self.height()
        };

        let mut xa;
        let mut xb;

        if ymin < y2 {
            xa = x1 + sl21 * (ymin - y1);
            xb = x1 + sl31 * (ymin - y1);
        } else if ymin == y2 {
            xa = x2;
            xb = if y1 == y3 {
                x3
            } else {
                x1 + sl31 * (ymin - y1)
            };
        } else {
            xa = x3 + sl32 * (ymin - y3);
            xb = x3 + sl31 * (ymin - y3);
        }

        let mut y = ymin;
        while y < ymax {
            let (xx1, xx2) = if xa < xb {
                ((xa + 0x800) / 0x10000, (xb + 0x801) / 0x10000)
            } else {
                ((xb + 0x800) / 0x10000, (xa + 0x801) / 0x10000)
            };

            let xmin = if xx1 < 0 { 0 } else { xx1 };
            let xmax = if xx2 + 1 < self.width() {
                xx2 + 1
            } else {
                self.width()
            };

            let mut x = xmin;
            while x < xmax {
                self.put_char(x, y, ch);
                x += 1;
            }

            xa += if y < y2 { sl21 } else { sl32 };
            xb += sl31;
            y += 1;
        }
    }

    /// Fill a triangle using an arbitrary texture, with per-vertex UVs.
    #[allow(clippy::too_many_arguments)]
    pub fn fill_triangle_textured(
        &mut self,
        coords: [i32; 6],
        tex: &Canvas,
        uv: [f32; 6],
    ) {
        fill_triangle_textured_l(
            self,
            coords[0], coords[1], coords[2], coords[3], coords[4], coords[5], tex, uv[0],
            uv[1], uv[2], uv[3], uv[4], uv[5],
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn fill_triangle_textured_l(
    cv: &mut Canvas,
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
    x3: i32,
    y3: i32,
    tex: &Canvas,
    u1: f32,
    v1: f32,
    u2: f32,
    v2: f32,
    u3: f32,
    v3: f32,
) {
    // Bubble-sort y1 <= y2 <= y3.
    if y1 > y2 {
        fill_triangle_textured_l(
            cv, x2, y2, x1, y1, x3, y3, tex, u2, v2, u1, v1, u3, v3,
        );
        return;
    }
    if y2 > y3 {
        fill_triangle_textured_l(
            cv, x1, y1, x3, y3, x2, y2, tex, u1, v1, u3, v3, u2, v2,
        );
        return;
    }

    let savedattr = cv.get_attr(-1, -1);

    let clamp = |v: f32| v.clamp(0.0, 1.0);
    let (mut u1, mut v1) = (clamp(u1), clamp(v1));
    let (mut u2, mut v2) = (clamp(u2), clamp(v2));
    let (mut u3, mut v3) = (clamp(u3), clamp(v3));

    let tw = tex.width() as f32;
    let th = tex.height() as f32;

    u1 *= tw;
    u2 *= tw;
    u3 *= tw;
    v1 *= th;
    v2 *= th;
    v3 *= th;

    let y2y1 = (y2 - y1) as f32;
    let y3y1 = (y3 - y1) as f32;
    let y3y2 = (y3 - y2) as f32;

    let mut sl12 = ((x2 - x1) as f32) / (if y2y1 == 0.0 { 1.0 } else { y2y1 });
    let mut sl13 = ((x3 - x1) as f32) / (if y3y1 == 0.0 { 1.0 } else { y3y1 });
    let mut sl23 = ((x3 - x2) as f32) / (if y3y2 == 0.0 { 1.0 } else { y3y2 });

    let mut usl12 = (u2 - u1) / (if y2y1 == 0.0 { 1.0 } else { y2y1 });
    let mut usl13 = (u3 - u1) / (if y3y1 == 0.0 { 1.0 } else { y3y1 });
    let mut usl23 = (u3 - u2) / (if y3y2 == 0.0 { 1.0 } else { y3y2 });
    let mut vsl12 = (v2 - v1) / (if y2y1 == 0.0 { 1.0 } else { y2y1 });
    let mut vsl13 = (v3 - v1) / (if y3y1 == 0.0 { 1.0 } else { y3y1 });
    let mut vsl23 = (v3 - v2) / (if y3y2 == 0.0 { 1.0 } else { y3y2 });

    let mut xa = x1 as f32;
    let mut xb = x1 as f32;
    let mut ua = u1;
    let mut ub = u1;
    let mut va = v1;
    let mut vb = v1;

    let mut s = false;

    // Top part.
    let mut y = y1;
    while y < y2 {
        if xb < xa {
            core::mem::swap(&mut xb, &mut xa);
            core::mem::swap(&mut sl13, &mut sl12);
            core::mem::swap(&mut ua, &mut ub);
            core::mem::swap(&mut va, &mut vb);
            core::mem::swap(&mut usl13, &mut usl12);
            core::mem::swap(&mut vsl13, &mut vsl12);
            s = true;
        }

        let tus = (ub - ua) / (xb - xa);
        let tvs = (vb - va) / (xb - xa);
        let mut v = va;
        let mut u = ua;

        let mut x = xa as i32;
        while (x as f32) < xb {
            u += tus;
            v += tvs;
            let attr = tex.get_attr(u as i32, v as i32);
            let c = tex.get_char(u as i32, v as i32);
            cv.set_attr(attr);
            cv.put_char(x, y, c);
            x += 1;
        }

        xa += sl13;
        xb += sl12;

        ua += usl13;
        va += vsl13;
        ub += usl12;
        vb += vsl12;

        y += 1;
    }

    if s {
        core::mem::swap(&mut xb, &mut xa);
        core::mem::swap(&mut sl13, &mut sl12);
        core::mem::swap(&mut ua, &mut ub);
        core::mem::swap(&mut va, &mut vb);
        core::mem::swap(&mut usl13, &mut usl12);
        core::mem::swap(&mut vsl13, &mut vsl12);
    }

    // Bottom part.
    xb = x2 as f32;

    if y1 == y2 {
        ua = u1;
        ub = u2;
        va = v1;
        vb = v2;
    }

    let mut y = y2;
    while y < y3 {
        if xb <= xa {
            core::mem::swap(&mut xb, &mut xa);
            core::mem::swap(&mut sl13, &mut sl23);
            core::mem::swap(&mut ua, &mut ub);
            core::mem::swap(&mut va, &mut vb);
            core::mem::swap(&mut usl13, &mut usl23);
            core::mem::swap(&mut vsl13, &mut vsl23);
        }

        let tus = (ub - ua) / (xb - xa);
        let tvs = (vb - va) / (xb - xa);
        let mut u = ua;
        let mut v = va;

        let mut x = xa as i32;
        while (x as f32) < xb {
            u += tus;
            v += tvs;
            let attr = tex.get_attr(u as i32, v as i32);
            let c = tex.get_char(u as i32, v as i32);
            cv.set_attr(attr);
            cv.put_char(x, y, c);
            x += 1;
        }

        xa += sl13;
        xb += sl23;

        ua += usl13;
        va += vsl13;
        ub += usl23;
        vb += vsl23;

        y += 1;
    }

    cv.set_attr(savedattr);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attr::Color;

    #[test]
    fn filled_triangle_nonempty() {
        let mut cv = Canvas::new(10, 10).unwrap();
        cv.fill_triangle(1, 1, 8, 1, 4, 8, b'#' as u32);
        assert_eq!(cv.get_char(4, 1), b'#' as u32);
        assert_eq!(cv.get_char(4, 5), b'#' as u32);
    }

    #[test]
    fn textured_triangle_copies_texture() {
        let mut tex = Canvas::new(2, 1).unwrap();
        tex.set_color_ansi(Color::Red, Color::Black).unwrap();
        tex.put_str(0, 0, "XY");

        let mut cv = Canvas::new(4, 2).unwrap();
        cv.fill_triangle_textured([0, 0, 3, 0, 0, 1], &tex, [0.0, 0.0, 1.0, 0.0, 0.0, 1.0]);
        // At least some texture cell should have been copied.
        let mut found = false;
        for y in 0..2 {
            for x in 0..4 {
                let c = cv.get_char(x, y);
                if c == b'X' as u32 || c == b'Y' as u32 {
                    found = true;
                }
            }
        }
        assert!(found);
    }
}
