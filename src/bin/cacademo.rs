//! Port of libcaca's `src/cacademo.c`: various demo effects.
//!
//! Cycles through plasma, metaballs, moiré, matrix and rotozoom effects
//! with star/square/circle/line transitions. Space pauses, Enter skips to
//! the next effect, Escape (or `Ctrl-C`/`Ctrl-Z`) quits.
//!
//! Two effects differ from C in out-of-bounds territory only: the matrix
//! effect indexes its drop strings with a possibly-negative offset (C
//! reads stack garbage; this uses `rem_euclid`), and the metaball ball
//! positions saturate instead of wrapping huge. The `langton` effect is
//! absent exactly like in C, where it is commented out.

use std::f64::consts::PI;

use libcaca::{key, rand, Canvas, Color, Display, Dither, Event, EventMask};

#[path = "../cademo_texture.rs"]
mod texture;
use texture::TEXTURE256X256;

const XSIZ: usize = 256;
const YSIZ: usize = 256;

const DEMO_COUNT: usize = 5;
const TRANSITION_FRAMES: i32 = 40;
const TRANSITION_COUNT: i32 = 5;
const TRANSITION_CIRCLE: i32 = 0;
const TRANSITION_STAR: i32 = 1;
const TRANSITION_SQUARE: i32 = 2;
const TRANSITION_VLINES: i32 = 3;
const TRANSITION_HLINES: i32 = 4;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Action {
    Prepare,
    Init,
    Update,
    Render,
    Free,
}

// Plasma -----------------------------------------------------------------

struct Plasma {
    dither: Option<Dither>,
    screen: Vec<u8>,
    table: Vec<u8>,
    red: [u32; 256],
    green: [u32; 256],
    blue: [u32; 256],
    alpha: [u32; 256],
    r: [f64; 3],
    big_r: [f64; 6],
}

impl Plasma {
    fn new() -> Plasma {
        Plasma {
            dither: None,
            screen: Vec::new(),
            table: Vec::new(),
            red: [0; 256],
            green: [0; 256],
            blue: [0; 256],
            alpha: [0; 256],
            r: [0.0; 3],
            big_r: [0.0; 6],
        }
    }

    fn do_plasma(&mut self, x: [f64; 3], y: [f64; 3]) {
        const TX: usize = XSIZ * 2;
        let x1 = (x[0] * (TX / 2) as f64) as usize;
        let y1 = (y[0] * (TX / 2) as f64) as usize;
        let x2 = (x[1] * (TX / 2) as f64) as usize;
        let y2 = (y[1] * (TX / 2) as f64) as usize;
        let x3 = (x[2] * (TX / 2) as f64) as usize;
        let y3 = (y[2] * (TX / 2) as f64) as usize;
        for yy in 0..YSIZ {
            let mut tmp = yy * YSIZ;
            let mut ty = yy * TX;
            let tmax = ty + XSIZ;
            while ty < tmax {
                self.screen[tmp] = self.table[x1 + y1 * TX + ty]
                    .wrapping_add(self.table[x2 + y2 * TX + ty])
                    .wrapping_add(self.table[x3 + y3 * TX + ty]);
                tmp += 1;
                ty += 1;
            }
        }
    }

    fn run(&mut self, action: Action, cv: &mut Canvas, frame: i32) {
        match action {
            Action::Prepare => {
                for i in 0..3 {
                    self.r[i] = rand(1, 1000) as f64 / 60000.0 * PI;
                }
                for i in 0..6 {
                    self.big_r[i] = rand(1, 1000) as f64 / 10000.0;
                }
                const TX: usize = XSIZ * 2;
                const TY: usize = YSIZ * 2;
                self.table = vec![0; TX * TY];
                for y in 0..TY {
                    for x in 0..TX {
                        let dx = x as i32 - TX as i32 / 2;
                        let dy = y as i32 - TX as i32 / 2;
                        let tmp = (dx * dx + dy * dy) as f64 * (PI / (TX * TX + TY * TY) as f64);
                        self.table[x + y * TX] =
                            ((1.0 + (12.0 * tmp.sqrt()).sin()) * 256.0 / 6.0) as u8;
                    }
                }
            }
            Action::Init => {
                self.screen = vec![0; XSIZ * YSIZ];
                self.dither = Some(
                    Dither::new(8, XSIZ as i32, YSIZ as i32, XSIZ as i32, 0, 0, 0, 0).unwrap(),
                );
            }
            Action::Update => {
                let f = frame as f64;
                for i in 0..256 {
                    let z = i as f64 / 256.0 * 6.0 * PI;
                    self.red[i] = ((1.0 + (z + self.r[1] * f).sin()) / 2.0 * 0xfff as f64) as u32;
                    self.blue[i] =
                        ((1.0 + (z + self.r[0] * (f + 100.0)).cos()) / 2.0 * 0xfff as f64) as u32;
                    self.green[i] =
                        ((1.0 + (z + self.r[2] * (f + 200.0)).cos()) / 2.0 * 0xfff as f64) as u32;
                }
                let dither = self.dither.as_mut().unwrap();
                let _ = dither.set_palette(&self.red, &self.green, &self.blue, &self.alpha);
                let r = self.big_r;
                self.do_plasma(
                    [
                        (1.0 + (f * r[0]).sin()) / 2.0,
                        (1.0 + (f * r[1]).sin()) / 2.0,
                        (1.0 + (f * r[2]).sin()) / 2.0,
                    ],
                    [
                        (1.0 + (f * r[3]).sin()) / 2.0,
                        (1.0 + (f * r[4]).sin()) / 2.0,
                        (1.0 + (f * r[5]).sin()) / 2.0,
                    ],
                );
            }
            Action::Render => {
                let dither = self.dither.as_ref().unwrap();
                let _ = dither.dither_bitmap(cv, 0, 0, cv.width(), cv.height(), &self.screen);
            }
            Action::Free => {
                self.screen.clear();
                self.dither = None;
            }
        }
    }
}

// Metaballs ---------------------------------------------------------------

const METASIZE: usize = XSIZ / 2;
const METABALLS: usize = 12;

struct Metaballs {
    dither: Option<Dither>,
    screen: Vec<u8>,
    ball: Vec<u8>,
    r: [u32; 256],
    g: [u32; 256],
    b: [u32; 256],
    a: [u32; 256],
    dd: [f32; METABALLS],
    di: [f32; METABALLS],
    dj: [f32; METABALLS],
    dk: [f32; METABALLS],
    x: [u32; METABALLS],
    y: [u32; METABALLS],
    fi: f32,
    fj: f32,
    fk: f32,
    offset: [f64; 440],
    angleoff: i32,
}

impl Metaballs {
    fn new() -> Metaballs {
        Metaballs {
            dither: None,
            screen: Vec::new(),
            ball: vec![0; METASIZE * METASIZE],
            r: [0; 256],
            g: [0; 256],
            b: [0; 256],
            a: [0; 256],
            dd: [0.0; METABALLS],
            di: [0.0; METABALLS],
            dj: [0.0; METABALLS],
            dk: [0.0; METABALLS],
            x: [0; METABALLS],
            y: [0; METABALLS],
            fi: 10.0,
            fj: 17.0,
            fk: 11.0,
            offset: [0.0; 440],
            angleoff: 0,
        }
    }

    fn create_ball(&mut self) {
        for y in 0..METASIZE {
            for x in 0..METASIZE {
                let dx = METASIZE as i32 / 2 - x as i32;
                let dy = METASIZE as i32 / 2 - y as i32;
                let distance = ((dx * dx + dy * dy) as f32).sqrt() * 64.0 / METASIZE as f32;
                // Values above 255 wrap to the low byte on x86 C builds;
                // reproduce the wrap instead of saturating.
                self.ball[x + y * METASIZE] = if distance > 15.0 {
                    0
                } else {
                    (((255.0 - distance) * 15.0) as i32 & 0xff) as u8
                };
            }
        }
    }

    fn draw_ball(&mut self, bx: usize, by: usize) {
        let mut b = by * XSIZ + bx;
        let mut e = 0usize;
        for i in 0..METASIZE * METASIZE {
            // Balls at the extreme edge address past the buffer in C;
            // skip those writes instead of scribbling the heap.
            if b < self.screen.len() {
                let color = self.screen[b] as u16 + self.ball[i] as u16;
                self.screen[b] = color.min(255) as u8;
            }
            if e == METASIZE {
                e = 0;
                b += XSIZ - METASIZE;
            }
            b += 1;
            e += 1;
        }
    }

    fn run(&mut self, action: Action, cv: &mut Canvas, frame: i32) {
        match action {
            Action::Prepare => {
                self.r[255] = 0xfff;
                self.g[255] = 0xfff;
                self.b[255] = 0xfff;
                self.create_ball();
                for n in 0..METABALLS {
                    self.dd[n] = rand(0, 100) as f32;
                    self.di[n] = rand(500, 4000) as f32 / 6000.0;
                    self.dj[n] = rand(500, 4000) as f32 / 6000.0;
                    self.dk[n] = rand(500, 4000) as f32 / 6000.0;
                }
                self.angleoff = rand(0, 360);
                for n in 0..440 {
                    self.offset[n] = 1.0 + (n as f64 * PI / 60.0).sin();
                }
            }
            Action::Init => {
                self.screen = vec![0; XSIZ * YSIZ];
                // A dither smaller than the pixel buffer, showing only
                // the interesting part of it.
                self.dither = Some(
                    Dither::new(
                        8,
                        (XSIZ - METASIZE) as i32,
                        (YSIZ - METASIZE) as i32,
                        XSIZ as i32,
                        0,
                        0,
                        0,
                        0,
                    )
                    .unwrap(),
                );
            }
            Action::Update => {
                let angle = (frame + self.angleoff) % 360;
                for n in 200..255 {
                    let c1 = self.offset[angle as usize];
                    let c2 = self.offset[angle as usize + 40];
                    let c3 = self.offset[angle as usize + 80];
                    let t1 = if n < 0x40 {
                        0
                    } else if n < 0xc0 {
                        (n - 0x40) * 0x20
                    } else {
                        0xfff
                    };
                    let t2 = if n < 0xe0 { 0 } else { (n - 0xe0) * 0x80 };
                    let t3 = if n < 0x40 { n * 0x40 } else { 0xfff };
                    self.r[n as usize] =
                        ((c1 * t1 as f64 + c2 * t2 as f64 + c3 * t3 as f64) / 4.0) as u32;
                    self.g[n as usize] =
                        ((c1 * t2 as f64 + c2 * t3 as f64 + c3 * t1 as f64) / 4.0) as u32;
                    self.b[n as usize] =
                        ((c1 * t3 as f64 + c2 * t1 as f64 + c3 * t2 as f64) / 4.0) as u32;
                }
                let dither = self.dither.as_mut().unwrap();
                let _ = dither.set_palette(&self.r, &self.g, &self.b, &self.a);

                for n in 0..METABALLS {
                    let u = self.di[n] * self.fi
                        + self.dj[n] * self.fj
                        + self.dk[n] * (self.di[n] * self.fk).sin();
                    let v = self.dd[n]
                        + self.di[n] * self.fj
                        + self.dj[n] * self.fk
                        + self.dk[n] * (self.dk[n] * self.fi).sin();
                    let u = (self.fi + u * 2.1).sin() * (1.0 + u.sin());
                    let v = (self.fj + v * 1.9).sin() * (1.0 + v.sin());
                    // Positions stay inside [0, edge]; `as u32` saturates
                    // the unrepresentable float dust at the boundary.
                    self.x[n] = ((XSIZ - METASIZE) as f32 / 2.0
                        + u * (XSIZ - METASIZE) as f32 / 4.0)
                        as u32;
                    self.y[n] = ((YSIZ - METASIZE) as f32 / 2.0
                        + v * (YSIZ - METASIZE) as f32 / 4.0)
                        as u32;
                }

                self.fi += 0.011;
                self.fj += 0.017;
                self.fk += 0.019;

                self.screen.fill(0);
                for n in 0..METABALLS {
                    self.draw_ball(self.x[n] as usize, self.y[n] as usize);
                }
            }
            Action::Render => {
                let dither = self.dither.as_ref().unwrap();
                let off = (METASIZE / 2) * (1 + XSIZ);
                let _ =
                    dither.dither_bitmap(cv, 0, 0, cv.width(), cv.height(), &self.screen[off..]);
            }
            Action::Free => {
                self.screen.clear();
                self.dither = None;
            }
        }
    }
}

// Moiré -------------------------------------------------------------------

const DISCSIZ: usize = XSIZ * 2;
const DISCTHICKNESS: i32 = XSIZ as i32 * 15 / 40;

struct Moire {
    dither: Option<Dither>,
    screen: Vec<u8>,
    disc: Vec<u8>,
    d: [f32; 6],
    red: [u32; 256],
    green: [u32; 256],
    blue: [u32; 256],
    alpha: [u32; 256],
}

impl Moire {
    fn new() -> Moire {
        Moire {
            dither: None,
            screen: Vec::new(),
            disc: vec![0; DISCSIZ * DISCSIZ],
            d: [0.0; 6],
            red: [0; 256],
            green: [0; 256],
            blue: [0; 256],
            alpha: [0; 256],
        }
    }

    fn draw_line(&mut self, x: i32, y: i32, color: u8) {
        if x == 0 || y == 0 || y > DISCSIZ as i32 / 2 {
            return;
        }
        let x = x.min(DISCSIZ as i32 / 2);
        let half = DISCSIZ as i32 / 2;
        for row in [half - y, half + y - 1] {
            let start = (half - x + DISCSIZ as i32 * row) as usize;
            for i in 0..(2 * x - 1) as usize {
                self.disc[start + i] = color;
            }
        }
    }

    fn put_disc(&mut self, x: i32, y: i32) {
        for j in 0..YSIZ as i32 {
            for i in 0..XSIZ as i32 {
                let sx = DISCSIZ as i32 / 2 - x + i;
                let sy = DISCSIZ as i32 / 2 - y + j;
                // Negative source offsets read out of bounds in C;
                // treat them as zeroes.
                let v = if sx >= 0 && sy >= 0 && sx < DISCSIZ as i32 && sy < DISCSIZ as i32 {
                    self.disc[sx as usize + DISCSIZ * sy as usize]
                } else {
                    0
                };
                let idx = i as usize + XSIZ * j as usize;
                self.screen[idx] ^= v;
            }
        }
    }

    fn run(&mut self, action: Action, cv: &mut Canvas, frame: i32) {
        match action {
            Action::Prepare => {
                for i in 0..6 {
                    self.d[i] = rand(50, 70) as f32 / 1000.0;
                }
                self.red[0] = 0x777;
                self.green[0] = 0x777;
                self.blue[0] = 0x777;
                self.red[1] = 0xfff;
                self.green[1] = 0xfff;
                self.blue[1] = 0xfff;

                let mut i = DISCSIZ as i32 * 2;
                while i > 0 {
                    let (mut t, mut dx, mut dy) = (0, 0, i);
                    while dx <= dy {
                        let c = ((i / DISCTHICKNESS) % 2) as u8;
                        self.draw_line(dx / 3, dy / 3, c);
                        self.draw_line(dy / 3, dx / 3, c);
                        if t > 0 {
                            t += dx - dy;
                            dy -= 1;
                        } else {
                            t += dx;
                        }
                        dx += 1;
                    }
                    i -= DISCTHICKNESS;
                }
            }
            Action::Init => {
                self.screen = vec![0; XSIZ * YSIZ];
                self.dither = Some(
                    Dither::new(8, XSIZ as i32, YSIZ as i32, XSIZ as i32, 0, 0, 0, 0).unwrap(),
                );
            }
            Action::Update => {
                self.screen.fill(0);
                let f = frame as f64;
                let d = self.d;
                self.red[0] =
                    (0.5 * (1.0 + (d[0] as f64 * (f + 1000.0)).sin()) * 0xfff as f64) as u32;
                self.green[0] = (0.5 * (1.0 + (d[1] as f64 * f).cos()) * 0xfff as f64) as u32;
                self.blue[0] =
                    (0.5 * (1.0 + (d[2] as f64 * (f + 3000.0)).cos()) * 0xfff as f64) as u32;
                self.red[1] =
                    (0.5 * (1.0 + (d[3] as f64 * (f + 2000.0)).sin()) * 0xfff as f64) as u32;
                self.green[1] = (0.5 * (1.0 + (d[4] as f64 * f + 5.0).cos()) * 0xfff as f64) as u32;
                self.blue[1] =
                    (0.5 * (1.0 + (d[5] as f64 * (f + 4000.0)).cos()) * 0xfff as f64) as u32;
                let dither = self.dither.as_mut().unwrap();
                let _ = dither.set_palette(&self.red, &self.green, &self.blue, &self.alpha);

                let x = (d[0] as f64 * (f + 1000.0)).cos() * 128.0 + (XSIZ / 2) as f64;
                let y = (0.11 * f).sin() * 128.0 + (YSIZ / 2) as f64;
                self.put_disc(x as i32, y as i32);

                let x = (0.13 * f + 2.0).cos() * 64.0 + (XSIZ / 2) as f64;
                let y = (d[1] as f64 * (f + 2000.0)).sin() * 64.0 + (YSIZ / 2) as f64;
                self.put_disc(x as i32, y as i32);
            }
            Action::Render => {
                let dither = self.dither.as_ref().unwrap();
                let _ = dither.dither_bitmap(cv, 0, 0, cv.width(), cv.height(), &self.screen);
            }
            Action::Free => {
                self.screen.clear();
                self.dither = None;
            }
        }
    }
}

// Matrix ------------------------------------------------------------------

const MAXDROPS: usize = 500;
const MINLEN: usize = 15;
const MAXLEN: usize = 30;

struct Drop {
    x: i32,
    y: i32,
    speed: i32,
    len: usize,
    str_: [u8; MAXLEN],
}

struct Matrix {
    drops: Vec<Drop>,
}

impl Matrix {
    fn new() -> Matrix {
        let mut drops = Vec::with_capacity(MAXDROPS);
        for _ in 0..MAXDROPS {
            let mut str_ = [0u8; MAXLEN];
            for slot in str_.iter_mut() {
                *slot = rand(b'0' as i32, b'z' as i32) as u8;
            }
            drops.push(Drop {
                x: rand(0, 1000),
                y: rand(0, 1000),
                speed: 5 + rand(0, 30),
                len: MINLEN + rand(0, (MAXLEN - MINLEN) as i32) as usize,
                str_,
            });
        }
        Matrix { drops }
    }

    fn run(&mut self, action: Action, cv: &mut Canvas, _frame: i32) {
        match action {
            Action::Prepare => {}
            Action::Init => {}
            Action::Update => {
                let (w, h) = (cv.width(), cv.height());
                for drop_ in self
                    .drops
                    .iter_mut()
                    .take(MAXDROPS.min((w * h / 32).max(0) as usize))
                {
                    drop_.y += drop_.speed;
                    if drop_.y > 1000 {
                        drop_.y -= 1000;
                        drop_.x = rand(0, 1000);
                    }
                }
            }
            Action::Render => {
                use Color::{Black, DarkGray, Green, LightGreen, White};
                let (w, h) = (cv.width(), cv.height());
                let _ = cv.set_color_ansi(Black, Black);
                cv.clear();
                for drop_ in self
                    .drops
                    .iter()
                    .take(MAXDROPS.min((w * h / 32).max(0) as usize))
                {
                    let x = drop_.x * w / 1000 / 2 * 2;
                    let y = drop_.y * (h + MAXLEN as i32) / 1000;
                    for j in 0..drop_.len as i32 {
                        let fg = if j < 2 {
                            White
                        } else if j < drop_.len as i32 / 4 {
                            LightGreen
                        } else if j < drop_.len as i32 * 4 / 5 {
                            Green
                        } else {
                            DarkGray
                        };
                        let _ = cv.set_color_ansi(fg, Black);
                        // `(y - j)` goes negative near the top in C,
                        // reading stack garbage; wrap instead.
                        let ch = drop_.str_[(y - j).rem_euclid(drop_.len as i32) as usize];
                        cv.put_char(x, y - j, ch as u32);
                    }
                }
            }
            Action::Free => {}
        }
    }
}

// Rotozoom -----------------------------------------------------------------

const TEXTURE_SIZE: usize = 256;
const TABLE_SIZE: usize = 65536;

fn fmul(a: i32, b: i32) -> i32 {
    a.wrapping_mul(b) >> 8
}
fn tofix(d: f64) -> i32 {
    (d * 256.0) as i32
}

struct Rotozoom {
    dither: Option<Dither>,
    screen: Vec<u32>,
    raw: Vec<u8>,
    cos_tab: Vec<i32>,
    sin_tab: Vec<i32>,
    y_tab: [u32; TEXTURE_SIZE],
    alpha_f: i32,
    t_f: i32,
}

impl Rotozoom {
    fn new() -> Rotozoom {
        Rotozoom {
            dither: None,
            screen: Vec::new(),
            raw: Vec::new(),
            cos_tab: Vec::new(),
            sin_tab: Vec::new(),
            y_tab: [0; TEXTURE_SIZE],
            alpha_f: 0,
            t_f: 0,
        }
    }

    fn run(&mut self, action: Action, cv: &mut Canvas, _frame: i32) {
        match action {
            Action::Prepare => {
                self.cos_tab = (0..TABLE_SIZE)
                    .map(|x| tofix((x as f64 * 360.0 / TABLE_SIZE as f64).cos()))
                    .collect();
                self.sin_tab = (0..TABLE_SIZE)
                    .map(|x| tofix((x as f64 * 360.0 / TABLE_SIZE as f64).sin()))
                    .collect();
                for x in 0..TEXTURE_SIZE {
                    self.y_tab[x] = (x * TEXTURE_SIZE) as u32;
                }
            }
            Action::Init => {
                self.dither = Some(
                    Dither::new(
                        32,
                        XSIZ as i32,
                        YSIZ as i32,
                        XSIZ as i32 * 4,
                        0x00FF0000,
                        0x0000FF00,
                        0x000000FF,
                        0x00000000,
                    )
                    .unwrap(),
                );
                self.screen = vec![0; XSIZ * YSIZ];
                self.raw = vec![0; XSIZ * YSIZ * 4];
            }
            Action::Update => {
                self.alpha_f = self.alpha_f.wrapping_add(4);
                self.t_f = self.t_f.wrapping_add(3);
                let scale_f =
                    fmul(self.sin_tab[(self.t_f & 0xFFFF) as usize], tofix(3.0)) + tofix(4.0);
                let xxf = fmul(self.cos_tab[(self.alpha_f & 0xFFFF) as usize], scale_f);
                let yyf = fmul(self.sin_tab[(self.alpha_f & 0xFFFF) as usize], scale_f);
                let (mut uf, mut vf, mut uf_, mut vf_) = (0u32, 0u32, 0u32, 0u32);
                let mut p = 0usize;
                for _ in (0..YSIZ).rev() {
                    for _ in (0..XSIZ).rev() {
                        uf = uf.wrapping_add(xxf as u32);
                        vf = vf.wrapping_add(yyf as u32);
                        let vu = (uf >> 8) & 0xFF;
                        let vv = (vf >> 8) & 0xFF;
                        self.screen[p] = TEXTURE256X256[(vu + self.y_tab[vv as usize]) as usize];
                        p += 1;
                    }
                    uf_ = uf_.wrapping_sub(yyf as u32);
                    uf = uf_;
                    vf_ = vf_.wrapping_add(xxf as u32);
                    vf = vf_;
                }
            }
            Action::Render => {
                for (i, pixel) in self.screen.iter().enumerate() {
                    self.raw[i * 4..i * 4 + 4].copy_from_slice(&pixel.to_ne_bytes());
                }
                let dither = self.dither.as_ref().unwrap();
                let _ = dither.dither_bitmap(cv, 0, 0, cv.width(), cv.height(), &self.raw);
            }
            Action::Free => {
                self.screen.clear();
                self.raw.clear();
                self.dither = None;
            }
        }
    }
}

// Transitions ---------------------------------------------------------------

const STAR: [f32; 20] = [
    0.000, -1.000, 0.308, -0.349, 0.992, -0.244, 0.500, 0.266, 0.632, 0.998, 0.008, 0.659, -0.601,
    0.995, -0.496, 0.275, -0.997, -0.244, -0.313, -0.349,
];
const SQUARE: [f32; 8] = [-1.0, -1.0, 1.0, -1.0, 1.0, 1.0, -1.0, 1.0];

fn transition(mask: &mut Canvas, tmode: i32, completed: i32) {
    let (w, h) = (mask.width(), mask.height());
    let (w2, h2) = (w / 2, h / 2);
    let (mut mulx, mut muly) = (
        0.0075 * completed as f32 * w as f32,
        0.0075 * completed as f32 * h as f32,
    );
    // The C version spells out 3.14 here; keep the literal.
    #[allow(clippy::approx_constant)]
    let angle = (0.0075 * completed as f32 * 360.0) * 3.14 / 180.0;

    match tmode {
        TRANSITION_SQUARE => {
            let mut rot = [0.0f32; 8];
            for i in 0..4 {
                let (x, y) = (SQUARE[i * 2], SQUARE[i * 2 + 1]);
                rot[i * 2] = x * angle.cos() - y * angle.sin();
                rot[i * 2 + 1] = y * angle.cos() + x * angle.sin();
            }
            mulx *= 1.8;
            muly *= 1.8;
            let tri = |m: &mut Canvas, a: usize, b: usize, c: usize| {
                m.fill_triangle(
                    (rot[a * 2] * mulx + w2 as f32) as i32,
                    (rot[a * 2 + 1] * muly + h2 as f32) as i32,
                    (rot[b * 2] * mulx + w2 as f32) as i32,
                    (rot[b * 2 + 1] * muly + h2 as f32) as i32,
                    (rot[c * 2] * mulx + w2 as f32) as i32,
                    (rot[c * 2 + 1] * muly + h2 as f32) as i32,
                    b'#' as u32,
                );
            };
            tri(mask, 0, 1, 2);
            tri(mask, 0, 2, 3);
        }
        TRANSITION_STAR => {
            let mut rot = [0.0f32; 20];
            for i in 0..10 {
                let (x, y) = (STAR[i * 2], STAR[i * 2 + 1]);
                rot[i * 2] = x * angle.cos() - y * angle.sin();
                rot[i * 2 + 1] = y * angle.cos() + x * angle.sin();
            }
            mulx *= 1.8;
            muly *= 1.8;
            let tri = |m: &mut Canvas, a: usize, b: usize, c: usize| {
                m.fill_triangle(
                    (rot[a * 2] * mulx + w2 as f32) as i32,
                    (rot[a * 2 + 1] * muly + h2 as f32) as i32,
                    (rot[b * 2] * mulx + w2 as f32) as i32,
                    (rot[b * 2 + 1] * muly + h2 as f32) as i32,
                    (rot[c * 2] * mulx + w2 as f32) as i32,
                    (rot[c * 2 + 1] * muly + h2 as f32) as i32,
                    b'#' as u32,
                );
            };
            tri(mask, 0, 1, 9);
            tri(mask, 1, 2, 3);
            tri(mask, 3, 4, 5);
            tri(mask, 5, 6, 7);
            tri(mask, 7, 8, 9);
            tri(mask, 9, 1, 5);
            tri(mask, 9, 5, 7);
            tri(mask, 1, 3, 5);
        }
        TRANSITION_CIRCLE => {
            mask.fill_ellipse(w2, h2, mulx as i32, muly as i32, b'#' as u32);
        }
        TRANSITION_VLINES => {
            for i in 0..8 {
                let z = (if i & 1 != 0 { w } else { -w / 2 }) * (100 - completed) / 100;
                mask.fill_box(i * w / 8, z, w / 8 + 1, z + h, b'#' as u32);
            }
        }
        TRANSITION_HLINES => {
            for i in 0..6 {
                let z = (if i & 1 != 0 { w } else { -w / 2 }) * (100 - completed) / 100;
                mask.fill_box(z, i * h / 6, z + w, h / 6 + 1, b'#' as u32);
            }
        }
        _ => {}
    }
}

// Driver --------------------------------------------------------------------

enum Effect {
    Plasma(Box<Plasma>),
    Metaballs(Box<Metaballs>),
    Moire(Box<Moire>),
    Matrix(Box<Matrix>),
    Rotozoom(Box<Rotozoom>),
}

impl Effect {
    fn run(&mut self, action: Action, cv: &mut Canvas, frame: i32) {
        match self {
            Effect::Plasma(e) => e.run(action, cv, frame),
            Effect::Metaballs(e) => e.run(action, cv, frame),
            Effect::Moire(e) => e.run(action, cv, frame),
            Effect::Matrix(e) => e.run(action, cv, frame),
            Effect::Rotozoom(e) => e.run(action, cv, frame),
        }
    }
}

fn main() {
    let mut back = Canvas::new(0, 0).unwrap();
    let mut mask = Canvas::new(0, 0).unwrap();

    let mut dp = match Display::new(Canvas::new(0, 0).unwrap()) {
        Ok(dp) => dp,
        Err(_) => return,
    };
    let _ = back.set_size(dp.canvas().width(), dp.canvas().height());
    let _ = mask.set_size(dp.canvas().width(), dp.canvas().height());
    let _ = dp.set_display_time(20_000);

    let mut effects = [
        Effect::Plasma(Box::new(Plasma::new())),
        Effect::Metaballs(Box::new(Metaballs::new())),
        Effect::Moire(Box::new(Moire::new())),
        Effect::Matrix(Box::new(Matrix::new())),
        Effect::Rotozoom(Box::new(Rotozoom::new())),
    ];

    // Initialise all demos' lookup tables.
    for effect in effects.iter_mut() {
        effect.run(Action::Prepare, dp.canvas_mut(), 0);
    }

    // Choose a demo at random.
    let mut demo = rand(0, DEMO_COUNT as i32) as usize;
    let mut frame = 0i32;
    let mut next: Option<usize> = None;
    let mut next_transition = rand(500, 1000);
    let mut tmode = rand(0, TRANSITION_COUNT);
    let mut paused = false;

    effects[demo].run(Action::Init, dp.canvas_mut(), frame);

    loop {
        // Handle events.
        let mut end = false;
        while let Some(ev) = dp.get_event(EventMask::KEY_PRESS | EventMask::QUIT, 0) {
            match ev {
                Event::Quit => {
                    end = true;
                    break;
                }
                Event::KeyPress(k) => match k.ch {
                    x if x == key::ESCAPE || x == key::CTRL_C || x == key::CTRL_Z => {
                        end = true;
                        break;
                    }
                    x if x == b' ' as i32 => paused = !paused,
                    x if x == b'\r' as i32 && next.is_none() => {
                        next_transition = frame;
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        if end {
            break;
        }

        // Resize the spare canvases, just in case the main one changed.
        let (w, h) = (dp.canvas().width(), dp.canvas().height());
        let _ = back.set_size(w, h);
        let _ = mask.set_size(w, h);

        if !paused {
            // Update demo's data.
            effects[demo].run(Action::Update, dp.canvas_mut(), frame);

            // Handle transitions.
            if frame == next_transition {
                let mut n = rand(0, DEMO_COUNT as i32) as usize;
                if n == demo {
                    n = (n + 1) % DEMO_COUNT;
                }
                effects[n].run(Action::Init, &mut back, frame);
                next = Some(n);
            } else if next.is_some() && frame == next_transition + TRANSITION_FRAMES {
                effects[demo].run(Action::Free, dp.canvas_mut(), frame);
                demo = next.unwrap();
                next = None;
                next_transition = frame + rand(500, 1000);
                tmode = rand(0, TRANSITION_COUNT);
            }

            if let Some(n) = next {
                effects[n].run(Action::Update, &mut back, frame);
            }

            frame = frame.wrapping_add(1);
        }

        // Render main demo's canvas.
        effects[demo].run(Action::Render, dp.canvas_mut(), frame);

        // If a transition is on its way, render it.
        if let Some(n) = next {
            effects[n].run(Action::Render, &mut back, frame);
            let _ = mask.set_color_ansi(Color::LightGray, Color::Black);
            mask.clear();
            let _ = mask.set_color_ansi(Color::White, Color::White);
            transition(
                &mut mask,
                tmode,
                100 * (frame - next_transition) / TRANSITION_FRAMES,
            );
            let _ = dp.canvas_mut().blit(0, 0, &back, Some(&mask));
        }

        let _ = dp.canvas_mut().set_color_ansi(Color::White, Color::Blue);
        if frame < 100 {
            let cv = dp.canvas_mut();
            cv.put_str(
                cv.width() - 30,
                cv.height() - 2,
                " -=[ Powered by libcaca ]=- ",
            );
        }
        let _ = dp.refresh();
    }

    if let Some(n) = next {
        effects[n].run(Action::Free, &mut back, frame);
    }
    effects[demo].run(Action::Free, dp.canvas_mut(), frame);
}
