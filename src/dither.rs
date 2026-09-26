//! Bitmap dithering.
//!
//! Port of `caca/dither.c`. A [`Dither`] describes the layout of a bitmap
//! (depth, dimensions, pitch and bitmasks, or an 8-bit palette) and knows how
//! to render it onto a [`Canvas`] using one of several dithering algorithms.
//!
//! The numeric logic (fixed-point slopes, Floyd-Steinberg error diffusion,
//! colour quantisation) follows the C implementation closely. Where the C code
//! relies on `unsigned int` wraparound, the equivalent wrapping operations are
//! used explicitly so the behaviour is well-defined in Rust.

use alloc::vec;

use crate::attr::Color;
use crate::canvas::Canvas;
use crate::error::{CacaError, Result};

/// How colours are chosen for the characters and their background.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ColorMode {
    Mono,
    Gray,
    C8,
    C16,
    FullGray,
    Full8,
    Full16,
}

/// The dithering kernel to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Algorithm {
    None,
    Ordered2,
    Ordered4,
    Ordered8,
    Random,
    Fstein,
}

/// RGB palette used for colour quantisation. Values are 11-bit (`0x7ff`).
const RGB_PALETTE: [i32; 48] = [
    0x0, 0x0, 0x0, //
    0x0, 0x0, 0x7ff, //
    0x0, 0x7ff, 0x0, //
    0x0, 0x7ff, 0x7ff, //
    0x7ff, 0x0, 0x0, //
    0x7ff, 0x0, 0x7ff, //
    0x7ff, 0x7ff, 0x0, //
    0xaaa, 0xaaa, 0xaaa, //
    0x555, 0x555, 0x555, //
    0x000, 0x000, 0xfff, //
    0x000, 0xfff, 0x000, //
    0x000, 0xfff, 0xfff, //
    0xfff, 0x000, 0x000, //
    0xfff, 0x000, 0xfff, //
    0xfff, 0xfff, 0x000, //
    0xfff, 0xfff, 0xfff, //
];

/// Weight applied to each palette entry when looking for the nearest colour.
const RGB_WEIGHT: [i32; 16] = [1; 16];

/// Glyphs used by the `"ascii"` charset.
const ASCII_GLYPHS: [u32; 11] = [
    b' ' as u32,
    b'.' as u32,
    b':' as u32,
    b';' as u32,
    b't' as u32,
    b'%' as u32,
    b'S' as u32,
    b'X' as u32,
    b'@' as u32,
    b'8' as u32,
    b'?' as u32,
];

/// Glyphs used by the `"shades"` charset.
const SHADES_GLYPHS: [u32; 5] = [b' ' as u32, 0xb7, 0x2591, 0x2592, b'?' as u32];

/// Glyphs used by the `"blocks"` charset.
const BLOCKS_GLYPHS: [u32; 4] = [b' ' as u32, 0x2598, 0x259a, b'?' as u32];

const ANTIALIAS_LIST: &[&str] = &[
    "none",
    "No antialiasing",
    "prefilter",
    "Prefilter antialiasing",
];

const COLOR_LIST: &[&str] = &[
    "mono",
    "white on black",
    "gray",
    "grayscale on black",
    "8",
    "8 colours on black",
    "16",
    "16 colours on black",
    "fullgray",
    "full grayscale",
    "full8",
    "full 8 colours",
    "full16",
    "full 16 colours",
];

const CHARSET_LIST: &[&str] = &[
    "ascii",
    "plain ASCII",
    "shades",
    "CP437 shades",
    "blocks",
    "Unicode blocks",
];

const ALGORITHM_LIST: &[&str] = &[
    "none",
    "no dithering",
    "ordered2",
    "2x2 ordered dithering",
    "ordered4",
    "4x4 ordered dithering",
    "ordered8",
    "8x8 ordered dithering",
    "random",
    "random dithering",
    "fstein",
    "Floyd-Steinberg dithering",
];

const DITHER2: [i32; 4] = [0x00, 0x80, 0xc0, 0x40];

const DITHER4: [i32; 16] = [
    0x00, 0x80, 0x20, 0xa0, //
    0xc0, 0x40, 0xe0, 0x60, //
    0x30, 0xb0, 0x10, 0x90, //
    0xf0, 0x70, 0xd0, 0x50,
];

const DITHER8: [i32; 64] = [
    0x00, 0x80, 0x20, 0xa0, 0x08, 0x88, 0x28, 0xa8, //
    0xc0, 0x40, 0xe0, 0x60, 0xc8, 0x48, 0xe8, 0x68, //
    0x30, 0xb0, 0x10, 0x90, 0x38, 0xb8, 0x18, 0x98, //
    0xf0, 0x70, 0xd0, 0x50, 0xf8, 0x78, 0xd8, 0x58, //
    0x0c, 0x8c, 0x2c, 0xac, 0x04, 0x84, 0x24, 0xa4, //
    0xcc, 0x4c, 0xec, 0x6c, 0xc4, 0x44, 0xe4, 0x64, //
    0x3c, 0xbc, 0x1c, 0x9c, 0x34, 0xb4, 0x14, 0x94, //
    0xfc, 0x7c, 0xdc, 0x5c, 0xf4, 0x74, 0xd4, 0x54, //
];

/// An internal dither object, mirroring `struct caca_dither`.
pub struct Dither {
    bpp: i32,
    has_palette: bool,
    has_alpha: bool,
    w: i32,
    h: i32,
    pitch: i32,
    rmask: u32,
    gmask: u32,
    bmask: u32,
    amask: u32,
    rright: i32,
    gright: i32,
    bright: i32,
    aright: i32,
    rleft: i32,
    gleft: i32,
    bleft: i32,
    aleft: i32,
    red: [i32; 256],
    green: [i32; 256],
    blue: [i32; 256],
    alpha: [i32; 256],

    gamma: f32,
    brightness: f32,
    contrast: f32,
    gammatab: [i32; 4096],

    antialias_name: &'static str,
    antialias: bool,

    color_name: &'static str,
    color: ColorMode,

    algo_name: &'static str,
    algo: Algorithm,

    glyph_name: &'static str,
    glyphs: &'static [u32],

    invert: bool,
}

impl Dither {
    /// Create a dither object from its layout.
    ///
    /// `width`, `height` and `pitch` are in pixels/bytes and must be
    /// non-negative. `bpp` must be between 8 and 32. For non-8bpp formats the
    /// masks describe the bit fields of each channel; each non-zero mask must be
    /// a contiguous run of at most 12 bits.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        width: i32,
        height: i32,
        bpp: i32,
        pitch: i32,
        rmask: u32,
        gmask: u32,
        bmask: u32,
        amask: u32,
    ) -> Result<Dither> {
        if width < 0 || height < 0 || pitch < 0 || !(8..=32).contains(&bpp) {
            return Err(CacaError::Invalid);
        }

        // Validate that each non-zero mask is a contiguous run that can be
        // normalised to 12 bits (otherwise `mask2shift` would produce an
        // out-of-range index or a negative shift).
        for &mask in &[rmask, gmask, bmask, amask] {
            if mask != 0 {
                if !is_contiguous(mask) {
                    return Err(CacaError::Invalid);
                }
                let run = (mask >> mask.trailing_zeros()).count_ones();
                if run > 12 {
                    return Err(CacaError::Invalid);
                }
            }
        }

        let (rright, rleft) = mask2shift(rmask);
        let (gright, gleft) = mask2shift(gmask);
        let (bright, bleft) = mask2shift(bmask);
        let (aright, aleft) = mask2shift(amask);

        let mut d = Dither {
            bpp,
            has_palette: false,
            has_alpha: amask != 0,
            w: width,
            h: height,
            pitch,
            rmask,
            gmask,
            bmask,
            amask,
            rright,
            gright,
            bright,
            aright,
            rleft,
            gleft,
            bleft,
            aleft,
            red: [0; 256],
            green: [0; 256],
            blue: [0; 256],
            alpha: [0; 256],
            gamma: 1.0,
            brightness: 1.0,
            contrast: 1.0,
            gammatab: [0; 4096],
            antialias_name: "prefilter",
            antialias: true,
            color_name: "full16",
            color: ColorMode::Full16,
            algo_name: "fstein",
            algo: Algorithm::Fstein,
            glyph_name: "ascii",
            glyphs: &ASCII_GLYPHS,
            invert: false,
        };

        // In 8bpp mode, default to a grayscale palette.
        if bpp == 8 {
            d.has_palette = true;
            d.has_alpha = false;
            for i in 0..256 {
                let v = i as i32 * 0xfff / 256;
                d.red[i] = v;
                d.green[i] = v;
                d.blue[i] = v;
            }
        }

        for (i, g) in d.gammatab.iter_mut().enumerate() {
            *g = i as i32;
        }

        Ok(d)
    }

    /// Set the palette of an 8bpp dither object.
    ///
    /// The four slices must each contain at least 256 entries with values in
    /// `0..=0xfff`.
    pub fn set_palette(&mut self, r: &[u32], g: &[u32], b: &[u32], a: &[u32]) -> Result<()> {
        if self.bpp != 8 {
            return Err(CacaError::Invalid);
        }
        if r.len() < 256 || g.len() < 256 || b.len() < 256 || a.len() < 256 {
            return Err(CacaError::Invalid);
        }

        for i in 0..256 {
            if (r[i] | g[i] | b[i] | a[i]) >= 0x1000 {
                return Err(CacaError::Invalid);
            }
        }

        let mut has_alpha = false;
        for i in 0..256 {
            self.red[i] = r[i] as i32;
            self.green[i] = g[i] as i32;
            self.blue[i] = b[i] as i32;
            if a[i] != 0 {
                self.alpha[i] = a[i] as i32;
                has_alpha = true;
            }
        }

        self.has_alpha = has_alpha;
        Ok(())
    }

    /// Set the brightness. This port stores the value; like the C library it is
    /// not applied by the current dithering path.
    pub fn set_brightness(&mut self, value: f32) -> Result<()> {
        self.brightness = value;
        Ok(())
    }

    /// The current brightness.
    pub fn brightness(&self) -> f32 {
        self.brightness
    }

    /// Set the gamma. A negative value causes colour inversion.
    pub fn set_gamma(&mut self, value: f32) -> Result<()> {
        let mut gamma = value;
        if gamma < 0.0 {
            self.invert = true;
            gamma = -gamma;
        } else if gamma == 0.0 {
            return Err(CacaError::Invalid);
        }

        self.gamma = gamma;
        for (i, g) in self.gammatab.iter_mut().enumerate() {
            *g = (4096.0 * gammapow(i as f32 / 4096.0, 1.0 / gamma)) as i32;
        }
        Ok(())
    }

    /// The current gamma.
    pub fn gamma(&self) -> f32 {
        self.gamma
    }

    /// Set the contrast. This port stores the value; like the C library it is
    /// not applied by the current dithering path.
    pub fn set_contrast(&mut self, value: f32) -> Result<()> {
        self.contrast = value;
        Ok(())
    }

    /// The current contrast.
    pub fn contrast(&self) -> f32 {
        self.contrast
    }

    /// Select the antialiasing method (`"none"`, `"prefilter"` or `"default"`).
    pub fn set_antialias(&mut self, name: &str) -> Result<()> {
        if name.eq_ignore_ascii_case("none") {
            self.antialias_name = "none";
            self.antialias = false;
        } else if name.eq_ignore_ascii_case("prefilter") || name.eq_ignore_ascii_case("default") {
            self.antialias_name = "prefilter";
            self.antialias = true;
        } else {
            return Err(CacaError::Invalid);
        }
        Ok(())
    }

    /// The current antialiasing method.
    pub fn antialias(&self) -> &str {
        self.antialias_name
    }

    /// The list of available antialiasing methods (`key`, `description`, ...).
    pub fn antialias_list(&self) -> &'static [&'static str] {
        ANTIALIAS_LIST
    }

    /// Select the colour mode.
    pub fn set_color(&mut self, name: &str) -> Result<()> {
        let mode = if name.eq_ignore_ascii_case("mono") {
            (ColorMode::Mono, "mono")
        } else if name.eq_ignore_ascii_case("gray") {
            (ColorMode::Gray, "gray")
        } else if name.eq_ignore_ascii_case("8") {
            (ColorMode::C8, "8")
        } else if name.eq_ignore_ascii_case("16") {
            (ColorMode::C16, "16")
        } else if name.eq_ignore_ascii_case("fullgray") {
            (ColorMode::FullGray, "fullgray")
        } else if name.eq_ignore_ascii_case("full8") {
            (ColorMode::Full8, "full8")
        } else if name.eq_ignore_ascii_case("full16") || name.eq_ignore_ascii_case("default") {
            (ColorMode::Full16, "full16")
        } else {
            return Err(CacaError::Invalid);
        };
        self.color = mode.0;
        self.color_name = mode.1;
        Ok(())
    }

    /// The current colour mode.
    pub fn color(&self) -> &str {
        self.color_name
    }

    /// The list of available colour modes (`key`, `description`, ...).
    pub fn color_list(&self) -> &'static [&'static str] {
        COLOR_LIST
    }

    /// Select the charset (`"ascii"`, `"shades"`, `"blocks"` or `"default"`).
    pub fn set_charset(&mut self, name: &str) -> Result<()> {
        if name.eq_ignore_ascii_case("shades") {
            self.glyph_name = "shades";
            self.glyphs = &SHADES_GLYPHS;
        } else if name.eq_ignore_ascii_case("blocks") {
            self.glyph_name = "blocks";
            self.glyphs = &BLOCKS_GLYPHS;
        } else if name.eq_ignore_ascii_case("ascii") || name.eq_ignore_ascii_case("default") {
            self.glyph_name = "ascii";
            self.glyphs = &ASCII_GLYPHS;
        } else {
            return Err(CacaError::Invalid);
        }
        Ok(())
    }

    /// The current charset.
    pub fn charset(&self) -> &str {
        self.glyph_name
    }

    /// The list of available charsets (`key`, `description`, ...).
    pub fn charset_list(&self) -> &'static [&'static str] {
        CHARSET_LIST
    }

    /// Select the dithering algorithm.
    pub fn set_algorithm(&mut self, name: &str) -> Result<()> {
        let algo = if name.eq_ignore_ascii_case("none") {
            (Algorithm::None, "none")
        } else if name.eq_ignore_ascii_case("ordered2") {
            (Algorithm::Ordered2, "ordered2")
        } else if name.eq_ignore_ascii_case("ordered4") {
            (Algorithm::Ordered4, "ordered4")
        } else if name.eq_ignore_ascii_case("ordered8") {
            (Algorithm::Ordered8, "ordered8")
        } else if name.eq_ignore_ascii_case("random") {
            (Algorithm::Random, "random")
        } else if name.eq_ignore_ascii_case("fstein") || name.eq_ignore_ascii_case("default") {
            (Algorithm::Fstein, "fstein")
        } else {
            return Err(CacaError::Invalid);
        };
        self.algo = algo.0;
        self.algo_name = algo.1;
        Ok(())
    }

    /// The current dithering algorithm.
    pub fn algorithm(&self) -> &str {
        self.algo_name
    }

    /// The list of available algorithms (`key`, `description`, ...).
    pub fn algorithm_list() -> &'static [&'static str] {
        ALGORITHM_LIST
    }

    /// Dither a bitmap onto the canvas.
    ///
    /// The bitmap is stretched to the requested `w` x `h` rectangle. `pixels`
    /// must be large enough to hold every source pixel that will be read;
    /// otherwise [`CacaError::Invalid`] is returned.
    pub fn dither_bitmap(
        &self,
        cv: &mut Canvas,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        pixels: &[u8],
    ) -> Result<()> {
        let img_w = self.w;
        let img_h = self.h;
        let pitch = self.pitch;

        // An empty source image cannot be sampled.
        if img_w <= 0 || img_h <= 0 {
            return Ok(());
        }

        let x1 = x as i64;
        let x2 = x as i64 + w as i64 - 1;
        let y1 = y as i64;
        let y2 = y as i64 + h as i64 - 1;

        let deltax = w as i64;
        let deltay = h as i64;
        if deltax <= 0 || deltay <= 0 {
            return Ok(());
        }

        // Inclusive clipped loop bounds (matching the C `<= cv->width` checks).
        let x_start = x1.max(0);
        let x_end = x2.min(cv.width() as i64);
        let y_start = y1.max(0);
        let y_end = y2.min(cv.height() as i64);

        if x_start > x_end || y_start > y_end {
            return Ok(());
        }

        let bpp8 = (self.bpp / 8) as usize;

        // Check that the pixel buffer covers the furthest source pixel read.
        let max_sx = src_max(x_end, x1, deltax, img_w, self.antialias);
        let max_sy = src_max(y_end, y1, deltay, img_h, self.antialias);
        let need = bpp8 * max_sx as usize + pitch as usize * max_sy as usize + bpp8;
        if pixels.len() < need {
            return Err(CacaError::Invalid);
        }

        let savedattr = cv.get_attr(-1, -1);

        let fs_len = (x_end + 3) as usize;
        let mut fs_r = vec![0i32; fs_len];
        let mut fs_g = vec![0i32; fs_len];
        let mut fs_b = vec![0i32; fs_len];

        let dchmax = self.glyphs.len() as i32;

        for y in y_start as i32..=y_end as i32 {
            let mut remain_r = 0i32;
            let mut remain_g = 0i32;
            let mut remain_b = 0i32;
            let mut dither_index = 0i32;

            for x in x_start as i32..=x_end as i32 {
                let mut rgba = [0u32; 4];

                // First get RGB(A).
                if self.antialias {
                    let fromx = (x as i64 - x1) * img_w as i64 / deltax;
                    let fromy = (y as i64 - y1) * img_h as i64 / deltay;
                    let mut tox = (x as i64 - x1 + 1) * img_w as i64 / deltax;
                    let mut toy = (y as i64 - y1 + 1) * img_h as i64 / deltay;

                    // We want at least one pixel.
                    if tox == fromx {
                        tox += 1;
                    }
                    if toy == fromy {
                        toy += 1;
                    }

                    let mut dots = 0u32;
                    for myx in fromx..tox {
                        for myy in fromy..toy {
                            dots += 1;
                            self.get_rgba(pixels, myx as i32, myy as i32, &mut rgba);
                        }
                    }

                    rgba[0] /= dots;
                    rgba[1] /= dots;
                    rgba[2] /= dots;
                    rgba[3] /= dots;
                } else {
                    let fromx = (x as i64 - x1) * img_w as i64 / deltax;
                    let fromy = (y as i64 - y1) * img_h as i64 / deltay;
                    let tox = (x as i64 - x1 + 1) * img_w as i64 / deltax;
                    let toy = (y as i64 - y1 + 1) * img_h as i64 / deltay;

                    let myx = (fromx + tox) / 2;
                    let myy = (fromy + toy) / 2;
                    self.get_rgba(pixels, myx as i32, myy as i32, &mut rgba);
                }

                // Hack to force greyscale.
                if self.color == ColorMode::FullGray {
                    let gray = (rgba[0]
                        .wrapping_mul(3)
                        .wrapping_add(rgba[1].wrapping_mul(4))
                        .wrapping_add(rgba[2])
                        .wrapping_add(4))
                        / 8;
                    rgba[0] = gray;
                    rgba[1] = gray;
                    rgba[2] = gray;
                }

                if self.has_alpha && rgba[3] < 0x800 {
                    remain_r = 0;
                    remain_g = 0;
                    remain_b = 0;
                    let xi = x as usize;
                    fs_r[xi + 1] = 0;
                    fs_g[xi + 1] = 0;
                    fs_b[xi + 1] = 0;
                    continue;
                }

                let dither = match self.algo {
                    Algorithm::None | Algorithm::Fstein => 0x80,
                    Algorithm::Ordered2 => DITHER2[((y.rem_euclid(2)) * 2 + dither_index) as usize],
                    Algorithm::Ordered4 => DITHER4[((y.rem_euclid(4)) * 4 + dither_index) as usize],
                    Algorithm::Ordered8 => DITHER8[((y.rem_euclid(8)) * 8 + dither_index) as usize],
                    Algorithm::Random => crate::canvas::rand(0x00, 0x100),
                };

                if self.algo == Algorithm::Fstein {
                    rgba[0] = rgba[0].wrapping_add(remain_r as u32);
                    rgba[1] = rgba[1].wrapping_add(remain_g as u32);
                    rgba[2] = rgba[2].wrapping_add(remain_b as u32);
                } else {
                    let delta = ((dither - 0x80) * 4) as u32;
                    rgba[0] = rgba[0].wrapping_add(delta);
                    rgba[1] = rgba[1].wrapping_add(delta);
                    rgba[2] = rgba[2].wrapping_add(delta);
                }

                // Nearest background colour.
                let mut distmin = i32::MAX;
                let mut outbg = 0i32;
                for (i, p) in RGB_PALETTE.chunks_exact(3).enumerate() {
                    if self.color == ColorMode::FullGray && (p[0] != p[1] || p[0] != p[2]) {
                        continue;
                    }
                    let mut dist = sq(rgba[0].wrapping_sub(p[0] as u32) as i32)
                        .wrapping_add(sq(rgba[1].wrapping_sub(p[1] as u32) as i32))
                        .wrapping_add(sq(rgba[2].wrapping_sub(p[2] as u32) as i32));
                    dist = dist.wrapping_mul(RGB_WEIGHT[i]);
                    if dist < distmin {
                        outbg = i as i32;
                        distmin = dist;
                    }
                }

                let bg_r = RGB_PALETTE[outbg as usize * 3];
                let bg_g = RGB_PALETTE[outbg as usize * 3 + 1];
                let bg_b = RGB_PALETTE[outbg as usize * 3 + 2];

                let mut outfg = 0i32;
                let mut ch = 0i32;
                let mut error = [0i32; 3];
                let outch: u32;

                if self.color == ColorMode::Full16 || self.color == ColorMode::FullGray {
                    // Nearest foreground colour.
                    distmin = i32::MAX;
                    for (i, p) in RGB_PALETTE.chunks_exact(3).enumerate() {
                        if i as i32 == outbg {
                            continue;
                        }
                        if self.color == ColorMode::FullGray && (p[0] != p[1] || p[0] != p[2]) {
                            continue;
                        }
                        let dist = sq(rgba[0].wrapping_sub(p[0] as u32) as i32)
                            .wrapping_add(sq(rgba[1].wrapping_sub(p[1] as u32) as i32))
                            .wrapping_add(sq(rgba[2].wrapping_sub(p[2] as u32) as i32))
                            .wrapping_mul(RGB_WEIGHT[i]);
                        if dist < distmin {
                            outfg = i as i32;
                            distmin = dist;
                        }
                    }
                    let fg_r = RGB_PALETTE[outfg as usize * 3];
                    let fg_g = RGB_PALETTE[outfg as usize * 3 + 1];
                    let fg_b = RGB_PALETTE[outfg as usize * 3 + 2];

                    // Nearest glyph, by interpolating between fg and bg.
                    distmin = i32::MAX;
                    let denom = 2 * dchmax - 1;
                    for i in 0..dchmax - 1 {
                        let newr = i * fg_r + (denom - i) * bg_r;
                        let newg = i * fg_g + (denom - i) * bg_g;
                        let newb = i * fg_b + (denom - i) * bg_b;
                        let dist = (rgba[0].wrapping_mul(denom as u32).wrapping_sub(newr as u32)
                            as i32)
                            .wrapping_abs()
                            .wrapping_add(
                                (rgba[1].wrapping_mul(denom as u32).wrapping_sub(newg as u32)
                                    as i32)
                                    .wrapping_abs(),
                            )
                            .wrapping_add(
                                (rgba[2].wrapping_mul(denom as u32).wrapping_sub(newb as u32)
                                    as i32)
                                    .wrapping_abs(),
                            );
                        if dist < distmin {
                            ch = i;
                            distmin = dist;
                        }
                    }
                    outch = self.glyphs[ch as usize];

                    if self.algo == Algorithm::Fstein {
                        let qr = (fg_r * ch + bg_r * (denom - ch)) / denom;
                        let qg = (fg_g * ch + bg_g * (denom - ch)) / denom;
                        let qb = (fg_b * ch + bg_b * (denom - ch)) / denom;
                        error[0] = rgba[0].wrapping_sub(qr as u32) as i32;
                        error[1] = rgba[1].wrapping_sub(qg as u32) as i32;
                        error[2] = rgba[2].wrapping_sub(qb as u32) as i32;
                    }
                } else {
                    let mut lum = rgba[0];
                    if rgba[1] > lum {
                        lum = rgba[1];
                    }
                    if rgba[2] > lum {
                        lum = rgba[2];
                    }
                    outfg = outbg;
                    outbg = Color::Black as i32;

                    let mut c = (lum.wrapping_mul(dchmax as u32) / 0x1000) as i32;
                    if c < 0 {
                        c = 0;
                    } else if c > dchmax - 1 {
                        c = dchmax - 1;
                    }
                    ch = c;
                    outch = self.glyphs[ch as usize];

                    if self.algo == Algorithm::Fstein {
                        error[0] = rgba[0].wrapping_sub((bg_r * ch / (dchmax - 1)) as u32) as i32;
                        error[1] = rgba[1].wrapping_sub((bg_g * ch / (dchmax - 1)) as u32) as i32;
                        error[2] = rgba[2].wrapping_sub((bg_b * ch / (dchmax - 1)) as u32) as i32;
                    }
                }

                // Floyd-Steinberg error diffusion.
                if self.algo == Algorithm::Fstein {
                    let xi = x as usize;
                    remain_r = fs_r[xi + 2].wrapping_add(error[0].wrapping_mul(7) / 16);
                    remain_g = fs_g[xi + 2].wrapping_add(error[1].wrapping_mul(7) / 16);
                    remain_b = fs_b[xi + 2].wrapping_add(error[2].wrapping_mul(7) / 16);
                    fs_r[xi] = fs_r[xi].wrapping_add(error[0].wrapping_mul(3) / 16);
                    fs_g[xi] = fs_g[xi].wrapping_add(error[1].wrapping_mul(3) / 16);
                    fs_b[xi] = fs_b[xi].wrapping_add(error[2].wrapping_mul(3) / 16);
                    fs_r[xi + 1] = error[0].wrapping_mul(5) / 16;
                    fs_g[xi + 1] = error[1].wrapping_mul(5) / 16;
                    fs_b[xi + 1] = error[2].wrapping_mul(5) / 16;
                    fs_r[xi + 2] = error[0] / 16;
                    fs_g[xi + 2] = error[1] / 16;
                    fs_b[xi + 2] = error[2] / 16;
                }

                if self.invert {
                    outfg = 15 - outfg;
                    outbg = 15 - outbg;
                }

                let fg = Color::from_u8(outfg as u8).unwrap_or(Color::LightGray);
                let bg = Color::from_u8(outbg as u8).unwrap_or(Color::Black);
                cv.set_color_ansi(fg, bg)?;
                cv.put_char(x, y, outch);

                match self.algo {
                    Algorithm::Ordered2 => dither_index = (dither_index + 1) % 2,
                    Algorithm::Ordered4 => dither_index = (dither_index + 1) % 4,
                    Algorithm::Ordered8 => dither_index = (dither_index + 1) % 8,
                    _ => {}
                }
            }
        }

        cv.set_attr(savedattr);
        Ok(())
    }

    /// Extract one pixel as 12-bit RGBA, applying gamma.
    fn get_rgba(&self, pixels: &[u8], x: i32, y: i32, rgba: &mut [u32; 4]) {
        let bpp8 = (self.bpp / 8) as usize;
        let off = bpp8 * x as usize + self.pitch as usize * y as usize;

        let bits: u32 = match bpp8 {
            4 => u32::from_ne_bytes([
                pixels[off],
                pixels[off + 1],
                pixels[off + 2],
                pixels[off + 3],
            ]),
            3 => {
                if cfg!(target_endian = "big") {
                    (pixels[off] as u32) << 16
                        | (pixels[off + 1] as u32) << 8
                        | (pixels[off + 2] as u32)
                } else {
                    (pixels[off + 2] as u32) << 16
                        | (pixels[off + 1] as u32) << 8
                        | (pixels[off] as u32)
                }
            }
            2 => u16::from_ne_bytes([pixels[off], pixels[off + 1]]) as u32,
            _ => pixels[off] as u32,
        };

        if self.has_palette {
            rgba[0] = rgba[0].wrapping_add(self.gammatab[self.red[bits as usize] as usize] as u32);
            rgba[1] =
                rgba[1].wrapping_add(self.gammatab[self.green[bits as usize] as usize] as u32);
            rgba[2] = rgba[2].wrapping_add(self.gammatab[self.blue[bits as usize] as usize] as u32);
            rgba[3] = rgba[3].wrapping_add(self.alpha[bits as usize] as u32);
        } else {
            let r = ((bits & self.rmask) >> self.rright as u32) << self.rleft as u32;
            let g = ((bits & self.gmask) >> self.gright as u32) << self.gleft as u32;
            let b = ((bits & self.bmask) >> self.bright as u32) << self.bleft as u32;
            let a = ((bits & self.amask) >> self.aright as u32) << self.aleft as u32;
            rgba[0] = rgba[0].wrapping_add(self.gammatab[r as usize] as u32);
            rgba[1] = rgba[1].wrapping_add(self.gammatab[g as usize] as u32);
            rgba[2] = rgba[2].wrapping_add(self.gammatab[b as usize] as u32);
            rgba[3] = rgba[3].wrapping_add(a);
        }
    }
}

/// Compute the largest source pixel touched along one axis for a given clipped
/// destination coordinate.
fn src_max(coord_end: i64, start: i64, delta: i64, img: i32, antialias: bool) -> i32 {
    let from = ((coord_end - start) * img as i64 / delta) as i32;
    let mut to = ((coord_end - start + 1) * img as i64 / delta) as i32;
    if antialias {
        if to == from {
            to += 1;
        }
        to - 1
    } else {
        (from + to) / 2
    }
}

/// Convert a mask, eg. `0x0000ff00`, to shift values, eg. `8` and `-4` becomes
/// `(8, 4)` here because the left shift is already normalised to 12 bits.
fn mask2shift(mask: u32) -> (i32, i32) {
    if mask == 0 {
        return (0, 0);
    }

    let right = mask.trailing_zeros() as i32;
    let mut m = mask >> right;

    let mut lshift = 0i32;
    while m & 1 != 0 {
        m >>= 1;
        lshift += 1;
    }

    (right, 12 - lshift)
}

/// Whether `mask` is a single contiguous run of set bits.
fn is_contiguous(mask: u32) -> bool {
    if mask == 0 {
        return true;
    }
    let m = mask >> mask.trailing_zeros();
    m & m.wrapping_add(1) == 0
}

/// `x * x`, with wrapping semantics.
fn sq(x: i32) -> i32 {
    x.wrapping_mul(x)
}

/// Compute `x^y` using the portable series expansion from the C source.
fn gammapow(x: f32, y: f32) -> f32 {
    if x == 0.0 {
        return if y == 0.0 { 1.0 } else { 0.0 };
    }

    let t = (x - 1.0) / (x + 1.0);
    let t2 = t * t;
    let mut r = t;
    let mut acc = t;
    let mut i = 3;
    while i < 20 {
        r *= t2;
        acc += r / i as f32;
        i += 2;
    }

    let e = -y * 2.0 * acc;

    let mut r2 = e;
    let mut acc2 = 1.0 + e;
    let mut i = 2;
    while i < 16 {
        r2 = r2 * e / i as f32;
        acc2 += r2;
        i += 1;
    }

    1.0 / acc2
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    fn dither32() -> Dither {
        Dither::new(
            2,
            2,
            32,
            8,
            0x0000_00ff,
            0x0000_ff00,
            0x00ff_0000,
            0xff00_0000,
        )
        .unwrap()
    }

    #[test]
    fn dither_writes_canvas() {
        let d = dither32();
        let pixels: Vec<u8> = vec![
            255, 0, 0, 255, // red
            0, 255, 0, 255, // green
            0, 0, 255, 255, // blue
            255, 255, 255, 255, // white
        ];
        let mut cv = Canvas::new(4, 4).unwrap();
        let before = cv.get_attr(0, 0);
        d.dither_bitmap(&mut cv, 0, 0, 4, 4, &pixels).unwrap();
        assert!(cv.get_attr(0, 0) != before || cv.get_char(0, 0) != b' ' as u32);
    }

    #[test]
    fn all_algorithms_and_modes_run() {
        let pixels: Vec<u8> = vec![128; 2 * 2 * 4];
        for algo in [
            "none", "ordered2", "ordered4", "ordered8", "random", "fstein",
        ] {
            let mut d = dither32();
            d.set_algorithm(algo).unwrap();
            for color in ["mono", "gray", "8", "16", "fullgray", "full8", "full16"] {
                d.set_color(color).unwrap();
                let mut cv = Canvas::new(4, 4).unwrap();
                d.dither_bitmap(&mut cv, 0, 0, 4, 4, &pixels).unwrap();
            }
        }
    }

    #[test]
    fn palette_setting() {
        let zeros = [0u32; 256];
        let mut d = Dither::new(2, 2, 8, 2, 0, 0, 0, 0).unwrap();
        d.set_palette(&zeros, &zeros, &zeros, &zeros).unwrap();

        let mut high = [0u32; 256];
        high[5] = 0x1000;
        assert!(d.set_palette(&high, &zeros, &zeros, &zeros).is_err());

        // Non-8bpp refuses palettes.
        let mut d32 = dither32();
        assert!(d32.set_palette(&zeros, &zeros, &zeros, &zeros).is_err());
    }

    #[test]
    fn setters_and_getters() {
        let mut d = dither32();
        d.set_antialias("none").unwrap();
        assert_eq!(d.antialias(), "none");
        d.set_antialias("default").unwrap();
        assert_eq!(d.antialias(), "prefilter");
        assert!(d.set_antialias("bogus").is_err());

        d.set_color("mono").unwrap();
        assert_eq!(d.color(), "mono");
        assert!(d.set_color("bogus").is_err());

        d.set_charset("shades").unwrap();
        assert_eq!(d.charset(), "shades");
        assert!(d.set_charset("bogus").is_err());

        d.set_algorithm("ordered4").unwrap();
        assert_eq!(d.algorithm(), "ordered4");
        assert!(d.set_algorithm("bogus").is_err());

        assert!(d.set_gamma(0.0).is_err());
        d.set_gamma(2.2).unwrap();
        assert_eq!(d.gamma(), 2.2);
        d.set_brightness(1.5).unwrap();
        assert_eq!(d.brightness(), 1.5);
        d.set_contrast(0.5).unwrap();
        assert_eq!(d.contrast(), 0.5);

        assert!(!d.antialias_list().is_empty());
        assert!(!d.color_list().is_empty());
        assert!(!d.charset_list().is_empty());
        assert!(!Dither::algorithm_list().is_empty());
    }

    #[test]
    fn invalid_arguments() {
        assert!(Dither::new(-1, 2, 32, 8, 0, 0, 0, 0).is_err());
        assert!(Dither::new(2, -1, 32, 8, 0, 0, 0, 0).is_err());
        assert!(Dither::new(2, 2, 4, 8, 0, 0, 0, 0).is_err());
        assert!(Dither::new(2, 2, 33, 8, 0, 0, 0, 0).is_err());
        assert!(Dither::new(2, 2, 32, -1, 0, 0, 0, 0).is_err());
        // Non-contiguous mask.
        assert!(Dither::new(2, 2, 32, 8, 0x101, 0, 0, 0).is_err());
        // Contiguous but too wide.
        assert!(Dither::new(2, 2, 32, 8, 0xffff, 0, 0, 0).is_err());
    }

    #[test]
    fn too_small_pixel_buffer_is_invalid() {
        let d = dither32();
        let mut cv = Canvas::new(4, 4).unwrap();
        assert!(d.dither_bitmap(&mut cv, 0, 0, 4, 4, &[0u8; 4]).is_err());
    }

    #[test]
    fn off_canvas_region_is_noop() {
        let d = dither32();
        let mut cv = Canvas::new(4, 4).unwrap();
        assert!(d.dither_bitmap(&mut cv, -100, -100, 4, 4, &[]).is_ok());
    }
}
