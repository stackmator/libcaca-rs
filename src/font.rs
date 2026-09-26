//! Bitmap font handling and canvas rendering.
//!
//! Port of `caca/font.c`. The libcaca font binary format is a big-endian
//! header followed by a block table, a glyph table and packed glyph data.
//! Two built-in fonts are embedded: `"Monospace 9"` and `"Monospace Bold 12"`.

use alloc::vec::Vec;

use crate::attr::Attr;
use crate::canvas::Canvas;
use crate::error::{CacaError, Result};

const BUILTIN_NAMES: &[&str] = &["Monospace 9", "Monospace Bold 12"];
const MONO9: &[u8] = include_bytes!("../assets/mono9.bin");
const MONOBOLD12: &[u8] = include_bytes!("../assets/monobold12.bin");

#[derive(Debug, Clone, Copy)]
struct Block {
    start: u32,
    stop: u32,
    index: u32,
}

#[derive(Debug, Clone, Copy)]
struct Glyph {
    width: u16,
    height: u16,
    data_offset: u32,
}

/// A loaded bitmap font.
pub struct Font {
    bpp: u16,
    width: u16,
    height: u16,
    #[allow(dead_code)]
    flags: u16,
    blocks: Vec<Block>,
    glyphs: Vec<Glyph>,
    data: Vec<u8>,
}

fn rd_u16(b: &[u8], o: usize) -> u16 {
    u16::from_be_bytes([b[o], b[o + 1]])
}

fn rd_u32(b: &[u8], o: usize) -> u32 {
    u32::from_be_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

/// The list of embedded font names.
pub fn font_list() -> &'static [&'static str] {
    BUILTIN_NAMES
}

/// Load one of the embedded fonts by name.
pub fn load_builtin(name: &str) -> Result<Font> {
    if name.eq_ignore_ascii_case("Monospace 9") {
        Font::from_bytes(MONO9)
    } else if name.eq_ignore_ascii_case("Monospace Bold 12") {
        Font::from_bytes(MONOBOLD12)
    } else {
        Err(CacaError::Invalid)
    }
}

impl Font {
    /// Parse a font from the libcaca binary format.
    pub fn from_bytes(data: &[u8]) -> Result<Font> {
        const HEADER: usize = 28;

        if data.len() < 4 + HEADER {
            return Err(CacaError::Invalid);
        }

        let control_size = rd_u32(data, 4) as usize;
        let data_size = rd_u32(data, 8) as usize;
        let blocks = rd_u16(data, 14) as usize;
        let glyphs = rd_u32(data, 16) as usize;
        let bpp = rd_u16(data, 20);
        let width = rd_u16(data, 22);
        let height = rd_u16(data, 24);
        let maxwidth = rd_u16(data, 26);
        let maxheight = rd_u16(data, 28);
        let flags = rd_u16(data, 30);

        if data.len() != 4 + control_size + data_size
            || !matches!(bpp, 1 | 2 | 4 | 8)
            || (flags & 1) == 0
        {
            return Err(CacaError::Invalid);
        }

        let block_table = 4 + HEADER;
        let glyph_table = block_table + blocks * 12;
        let font_data_off = 4 + control_size;

        if glyph_table + glyphs * 8 > data.len() || font_data_off > data.len() {
            return Err(CacaError::Invalid);
        }

        let mut block_list: Vec<Block> = Vec::with_capacity(blocks);
        let mut user_blocks: Vec<u32> = Vec::with_capacity(blocks * 2 + 2);
        for i in 0..blocks {
            let o = block_table + i * 12;
            let start = rd_u32(data, o);
            let stop = rd_u32(data, o + 4);
            let index = rd_u32(data, o + 8);

            if start > stop || (i > 0 && start < block_list[i - 1].stop) || index as usize >= glyphs
            {
                return Err(CacaError::Invalid);
            }

            block_list.push(Block { start, stop, index });
            user_blocks.push(start);
            user_blocks.push(stop);
        }
        user_blocks.push(0);
        user_blocks.push(0);

        let mut glyph_list: Vec<Glyph> = Vec::with_capacity(glyphs);
        for i in 0..glyphs {
            let o = glyph_table + i * 8;
            let g = Glyph {
                width: rd_u16(data, o),
                height: rd_u16(data, o + 2),
                data_offset: rd_u32(data, o + 4),
            };

            let packed = (g.width as u32 * g.height as u32 * bpp as u32).div_ceil(8);
            if g.data_offset as usize >= data_size
                || g.data_offset as usize + packed as usize > data_size
                || g.width > maxwidth
                || g.height > maxheight
            {
                return Err(CacaError::Invalid);
            }

            glyph_list.push(g);
        }

        Ok(Font {
            bpp,
            width,
            height,
            flags,
            blocks: block_list,
            glyphs: glyph_list,
            data: data[font_data_off..font_data_off + data_size].to_vec(),
        })
    }

    /// The standard glyph width.
    pub fn width(&self) -> i32 {
        self.width as i32
    }

    /// The standard glyph height.
    pub fn height(&self) -> i32 {
        self.height as i32
    }

    /// The supported Unicode blocks as `[start, stop, start, stop, ..., 0, 0]`.
    pub fn blocks(&self) -> Vec<u32> {
        let mut v = Vec::with_capacity(self.blocks.len() * 2 + 2);
        for b in &self.blocks {
            v.push(b.start);
            v.push(b.stop);
        }
        v.push(0);
        v.push(0);
        v
    }

    fn unpack_glyph(&self, g: &Glyph, out: &mut Vec<u8>) {
        let n = g.width as usize * g.height as usize;
        out.clear();
        out.resize(n, 0);

        if self.bpp == 8 {
            let o = g.data_offset as usize;
            out.copy_from_slice(&self.data[o..o + n]);
            return;
        }

        let per_byte = 8 / self.bpp as usize;
        let mask = (1u16 << self.bpp) - 1;
        let scale = 0xffu16 / mask;

        for (i, slot) in out.iter_mut().enumerate() {
            let byte = self.data[g.data_offset as usize + i / per_byte];
            let shift = self.bpp as usize * (per_byte - 1 - (i % per_byte));
            let pixel = ((byte as u16) >> shift) & mask;
            *slot = (pixel * scale) as u8;
        }
    }

    /// Render the canvas into a 32-bit ARGB image buffer.
    ///
    /// `pitch` is the number of bytes per image row. Pixels are written in
    /// ARGB order, matching the C `caca_render_canvas`.
    pub fn render_canvas(
        &self,
        cv: &Canvas,
        buf: &mut [u8],
        width: i32,
        height: i32,
        pitch: i32,
    ) -> Result<()> {
        if width < 0 || height < 0 || pitch < 0 {
            return Err(CacaError::Invalid);
        }
        if (pitch as usize) * (height as usize) > buf.len() {
            return Err(CacaError::Invalid);
        }

        let cv_w = cv.width();
        let cv_h = cv.height();
        let chars = cv.chars();
        let attrs = cv.attrs();

        let xmax = if width < cv_w * self.width as i32 {
            width / self.width as i32
        } else {
            cv_w
        };
        let ymax = if height < cv_h * self.height as i32 {
            height / self.height as i32
        } else {
            cv_h
        };

        let mut glyph: Vec<u8> = Vec::new();

        for y in 0..ymax {
            for x in 0..xmax {
                let starty = y * self.height as i32;
                let startx = x * self.width as i32;
                let ch = chars[(y * cv_w + x) as usize];
                let attr = attrs[(y * cv_w + x) as usize];

                // Find the Unicode block containing this glyph.
                let mut b = 0usize;
                while b < self.blocks.len() {
                    if ch < self.blocks[b].start {
                        b = self.blocks.len();
                        break;
                    }
                    if ch < self.blocks[b].stop {
                        break;
                    }
                    b += 1;
                }
                if b == self.blocks.len() {
                    continue;
                }

                let block = self.blocks[b];
                let gi = (block.index + ch - block.start) as usize;
                if gi >= self.glyphs.len() {
                    continue;
                }
                let g = self.glyphs[gi];

                let argb = Attr::from_raw(attr).to_argb64();
                self.unpack_glyph(&g, &mut glyph);

                let gw = g.width as i32;
                let gh = g.height as i32;

                for j in 0..gh {
                    let line = ((starty + j) as usize) * (pitch as usize) + 4 * startx as usize;
                    for i in 0..gw {
                        let p = glyph[(j * gw + i) as usize] as u32;
                        let q = 0xff - p;
                        let px = line + 4 * i as usize;
                        for t in 0..4 {
                            buf[px + t] =
                                (((q * argb[t] as u32) + (p * argb[4 + t] as u32)) / 0xf) as u8;
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

impl core::fmt::Debug for Font {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Font")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("bpp", &self.bpp)
            .field("glyphs", &self.glyphs.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attr::Color;
    use alloc::vec;

    #[test]
    fn load_and_render_builtin() {
        let font = load_builtin("Monospace 9").unwrap();
        assert!(font.width() > 0);
        assert!(font.height() > 0);
        assert!(!font.blocks().is_empty());

        let mut cv = Canvas::new(4, 2).unwrap();
        cv.set_color_ansi(Color::White, Color::Transparent).unwrap();
        cv.put_str(0, 0, "Hi");

        let w = cv.width() * font.width();
        let h = cv.height() * font.height();
        let mut buf = vec![0u8; (w * h * 4) as usize];
        font.render_canvas(&cv, &mut buf, w, h, w * 4).unwrap();

        // The rendered glyphs must have produced some non-zero pixels.
        assert!(buf.iter().any(|&b| b != 0));
    }

    #[test]
    fn invalid_font_data() {
        assert!(Font::from_bytes(&[0u8; 8]).is_err());
        assert!(load_builtin("No Such Font").is_err());
    }

    #[test]
    fn render_bounds_checked() {
        let font = load_builtin("Monospace Bold 12").unwrap();
        let cv = Canvas::new(1, 1).unwrap();
        let mut buf = [0u8; 4];
        assert!(font.render_canvas(&cv, &mut buf, 100, 100, 400).is_err());
    }
}
