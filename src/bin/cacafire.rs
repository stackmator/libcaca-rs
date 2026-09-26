//! Port of libcaca's `src/aafire.c` (`cacafire`): fire demo.
//!
//! The classic Hubicka fire effect on an 8-bit paletted dither: heat seeds
//! at the bottom, relaxes upward through a cooling table, and renders in
//! the 256-entry fire palette. Space pauses, `Ctrl-C`/`Ctrl-Z`/Escape
//! quits.
//!
//! The seed rows write two rows past the visible bitmap in the C version,
//! landing in zeroed `malloc` slack; the buffer here is simply allocated
//! two rows taller so every index is in bounds with identical contents.

use libcaca::{key, rand, Canvas, Color, Display, Dither, Event, EventMask};

const MAXTABLE: usize = 256 * 5;

// Fire palette, 256 RGB triples in 0..63 steps.
const PAL: [i32; 768] = [
    0, 0, 0, 0, 0, 6, 0, 0, 6, 0, 0, 7, 0, 0, 8, 0, 0, 8, 0, 0, 9, 0, 0, 10, 2, 0, 10, 4, 0, 9, 6,
    0, 9, 8, 0, 8, 10, 0, 7, 12, 0, 7, 14, 0, 6, 16, 0, 5, 18, 0, 5, 20, 0, 4, 22, 0, 4, 24, 0, 3,
    26, 0, 2, 28, 0, 2, 30, 0, 1, 32, 0, 0, 32, 0, 0, 33, 0, 0, 34, 0, 0, 35, 0, 0, 36, 0, 0, 36,
    0, 0, 37, 0, 0, 38, 0, 0, 39, 0, 0, 40, 0, 0, 40, 0, 0, 41, 0, 0, 42, 0, 0, 43, 0, 0, 44, 0, 0,
    45, 0, 0, 46, 1, 0, 47, 1, 0, 48, 2, 0, 49, 2, 0, 50, 3, 0, 51, 3, 0, 52, 4, 0, 53, 4, 0, 54,
    5, 0, 55, 5, 0, 56, 6, 0, 57, 6, 0, 58, 7, 0, 59, 7, 0, 60, 8, 0, 61, 8, 0, 63, 9, 0, 63, 9, 0,
    63, 10, 0, 63, 10, 0, 63, 11, 0, 63, 11, 0, 63, 12, 0, 63, 12, 0, 63, 13, 0, 63, 13, 0, 63, 14,
    0, 63, 14, 0, 63, 15, 0, 63, 15, 0, 63, 16, 0, 63, 16, 0, 63, 17, 0, 63, 17, 0, 63, 18, 0, 63,
    18, 0, 63, 19, 0, 63, 19, 0, 63, 20, 0, 63, 20, 0, 63, 21, 0, 63, 21, 0, 63, 22, 0, 63, 22, 0,
    63, 23, 0, 63, 24, 0, 63, 24, 0, 63, 25, 0, 63, 25, 0, 63, 26, 0, 63, 26, 0, 63, 27, 0, 63, 27,
    0, 63, 28, 0, 63, 28, 0, 63, 29, 0, 63, 29, 0, 63, 30, 0, 63, 30, 0, 63, 31, 0, 63, 31, 0, 63,
    32, 0, 63, 32, 0, 63, 33, 0, 63, 33, 0, 63, 34, 0, 63, 34, 0, 63, 35, 0, 63, 35, 0, 63, 36, 0,
    63, 36, 0, 63, 37, 0, 63, 38, 0, 63, 38, 0, 63, 39, 0, 63, 39, 0, 63, 40, 0, 63, 40, 0, 63, 41,
    0, 63, 41, 0, 63, 42, 0, 63, 42, 0, 63, 43, 0, 63, 43, 0, 63, 44, 0, 63, 44, 0, 63, 45, 0, 63,
    45, 0, 63, 46, 0, 63, 46, 0, 63, 47, 0, 63, 47, 0, 63, 48, 0, 63, 48, 0, 63, 49, 0, 63, 49, 0,
    63, 50, 0, 63, 50, 0, 63, 51, 0, 63, 52, 0, 63, 52, 0, 63, 52, 0, 63, 52, 0, 63, 52, 0, 63, 53,
    0, 63, 53, 0, 63, 53, 0, 63, 53, 0, 63, 54, 0, 63, 54, 0, 63, 54, 0, 63, 54, 0, 63, 54, 0, 63,
    55, 0, 63, 55, 0, 63, 55, 0, 63, 55, 0, 63, 56, 0, 63, 56, 0, 63, 56, 0, 63, 56, 0, 63, 57, 0,
    63, 57, 0, 63, 57, 0, 63, 57, 0, 63, 57, 0, 63, 58, 0, 63, 58, 0, 63, 58, 0, 63, 58, 0, 63, 59,
    0, 63, 59, 0, 63, 59, 0, 63, 59, 0, 63, 60, 0, 63, 60, 0, 63, 60, 0, 63, 60, 0, 63, 60, 0, 63,
    61, 0, 63, 61, 0, 63, 61, 0, 63, 61, 0, 63, 62, 0, 63, 62, 0, 63, 62, 0, 63, 62, 0, 63, 63, 0,
    63, 63, 1, 63, 63, 2, 63, 63, 3, 63, 63, 4, 63, 63, 5, 63, 63, 6, 63, 63, 7, 63, 63, 8, 63, 63,
    9, 63, 63, 10, 63, 63, 10, 63, 63, 11, 63, 63, 12, 63, 63, 13, 63, 63, 14, 63, 63, 15, 63, 63,
    16, 63, 63, 17, 63, 63, 18, 63, 63, 19, 63, 63, 20, 63, 63, 21, 63, 63, 21, 63, 63, 22, 63, 63,
    23, 63, 63, 24, 63, 63, 25, 63, 63, 26, 63, 63, 27, 63, 63, 28, 63, 63, 29, 63, 63, 30, 63, 63,
    31, 63, 63, 31, 63, 63, 32, 63, 63, 33, 63, 63, 34, 63, 63, 35, 63, 63, 36, 63, 63, 37, 63, 63,
    38, 63, 63, 39, 63, 63, 40, 63, 63, 41, 63, 63, 42, 63, 63, 42, 63, 63, 43, 63, 63, 44, 63, 63,
    45, 63, 63, 46, 63, 63, 47, 63, 63, 48, 63, 63, 49, 63, 63, 50, 63, 63, 51, 63, 63, 52, 63, 63,
    52, 63, 63, 53, 63, 63, 54, 63, 63, 55, 63, 63, 56, 63, 63, 57, 63, 63, 58, 63, 63, 59, 63, 63,
    60, 63, 63, 61, 63, 63, 62, 63, 63, 63,
];

struct Fire {
    xsiz: usize,
    ysiz: usize,
    table: [u32; MAXTABLE],
    bitmap: Vec<u8>,
    dither: Dither,
    height: i32,
    run_loop: i32,
    sloop: i32,
    paused: bool,
}

fn at(bitmap: &[u8], i: usize) -> u32 {
    bitmap.get(i).copied().unwrap_or(0) as u32
}

impl Fire {
    fn gentable(&mut self) {
        let mut minus = 800 / self.ysiz as u32;
        if minus == 0 {
            minus = 1;
        }
        for i in 0..MAXTABLE as u32 {
            self.table[i as usize] = if i > minus { (i - minus) / 5 } else { 0 };
        }
    }

    fn firemain(&mut self) {
        let xsiz = self.xsiz;
        let n = xsiz * self.ysiz;
        // The C loop runs `p <= END`, writing one past the bitmap into
        // zeroed slack; the buffer is taller here, so run it inclusively.
        for p in 0..=n {
            let sum = at(&self.bitmap, p + xsiz - 1)
                + at(&self.bitmap, p + xsiz + 1)
                + at(&self.bitmap, p + xsiz)
                + at(&self.bitmap, p + 2 * xsiz - 1)
                + at(&self.bitmap, p + 2 * xsiz + 1);
            self.bitmap[p] = self.table[sum as usize % MAXTABLE] as u8;
        }
    }

    fn drawfire(&mut self, dp: &mut Display) {
        if !self.paused {
            let xsiz = self.xsiz as i32;
            let ysiz = self.ysiz;
            self.height += 1;
            self.run_loop -= 1;
            if self.run_loop < 0 {
                self.run_loop = rand(0, 3);
                self.sloop += 1;
            }
            let mut i1 = 1i32;
            let mut i2 = 4 * xsiz + 1;
            let row_start = (xsiz as usize) * ysiz;
            let row_end = (xsiz as usize) * (ysiz + 1);
            let mut p = row_start;
            while p < row_end {
                let mut last1 = rand(0, i1.min(i2).min(self.height));
                let mut i = rand(0, 6);
                while p < row_end && i != 0 {
                    // `as u8` wraps exactly like C's unsigned truncation.
                    self.bitmap[p] = last1 as u8;
                    last1 += rand(0, 6) - 2;
                    self.bitmap[p + xsiz as usize] = last1 as u8;
                    last1 += rand(0, 6) - 2;
                    p += 1;
                    i -= 1;
                    i1 += 4;
                    i2 -= 4;
                }
                self.bitmap[p + 2 * xsiz as usize] = last1 as u8;
            }
            self.firemain();
        }

        let cv = dp.canvas_mut();
        let _ = self
            .dither
            .dither_bitmap(cv, 0, 0, cv.width(), cv.height(), &self.bitmap);
        let _ = cv.set_color_ansi(Color::White, Color::Blue);
        if self.sloop < 100 {
            cv.put_str(
                cv.width() - 30,
                cv.height() - 2,
                " -=[ Powered by libcaca ]=- ",
            );
        }
        let _ = dp.refresh();
    }
}

fn main() {
    let cv = Canvas::new(80, 32).unwrap();
    let mut dp = Display::new(cv).unwrap();
    let _ = dp.set_display_time(10_000);

    let (xsiz, ysiz) = (
        dp.canvas().width() as usize * 2,
        dp.canvas().height() as usize * 2 - 4,
    );

    let (mut r, mut g, mut b, mut a) = ([0u32; 256], [0u32; 256], [0u32; 256], [0u32; 256]);
    for i in 0..256 {
        r[i] = (PAL[i * 3] * 64) as u32;
        g[i] = (PAL[i * 3 + 1] * 64) as u32;
        b[i] = (PAL[i * 3 + 2] * 64) as u32;
        a[i] = 0xfff;
    }

    let mut dither = Dither::new(8, xsiz as i32, ysiz as i32 - 2, xsiz as i32, 0, 0, 0, 0).unwrap();
    dither.set_palette(&r, &g, &b, &a).unwrap();

    let mut fire = Fire {
        table: [0; MAXTABLE],
        bitmap: vec![0; xsiz * (ysiz + 3)],
        dither,
        height: 0,
        run_loop: 0,
        sloop: 0,
        paused: false,
        xsiz,
        ysiz,
    };
    fire.gentable();

    loop {
        if let Some(ev) = dp.get_event(EventMask::KEY_PRESS | EventMask::QUIT, 0) {
            match ev {
                Event::Quit => break,
                Event::KeyPress(k) => match k.ch {
                    x if x == key::CTRL_C || x == key::CTRL_Z || x == key::ESCAPE => break,
                    x if x == b' ' as i32 => fire.paused = !fire.paused,
                    _ => {}
                },
                _ => {}
            }
        }
        fire.drawfire(&mut dp);
    }
}
