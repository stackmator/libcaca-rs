//! FIGlet and TOIlet font handling.
//!
//! Port of `caca/figfont.c`. Unlike the C library, where the font is attached
//! to a canvas through `caca_canvas_set_figfont`, this is an idiomatic
//! standalone [`FigFont`] whose methods take the target [`Canvas`] explicitly.
//!
//! ```no_run
//! use libcaca::{Canvas, FigFont};
//!
//! let mut ff = FigFont::load("slant.flf")?;
//! let mut cv = Canvas::new(1, 1)?;
//! for ch in "hello".chars() {
//!     ff.put_char(&mut cv, ch as u32)?;
//! }
//! ff.flush(&mut cv)?;
//! # Ok::<(), libcaca::CacaError>(())
//! ```

use std::path::Path;

use crate::canvas::Canvas;
use crate::charset::utf8_to_utf32;
use crate::error::{CacaError, Result};

const STD_GLYPHS: i32 = 127 - 32;
const EXT_GLYPHS: i32 = STD_GLYPHS + 7;
const EXT_TAB: [u32; 7] = [196, 214, 220, 228, 246, 252, 223];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HMode {
    Default,
    Kern,
    Smush,
    None,
    Overlap,
}

/// A loaded FIGfont.
pub struct FigFont {
    height: i32,
    baseline: i32,
    max_length: i32,
    hardblank: u32,
    old_layout: i32,
    full_layout: i32,
    glyphs: i32,
    lookup: Vec<(u32, u32)>,
    fontcv: Canvas,
    charcv: Canvas,

    term_width: i32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    lines: i32,
    hmode: HMode,
    hsmushrule: i32,
}

fn split_lines(data: &[u8]) -> Vec<Vec<u8>> {
    let mut v: Vec<Vec<u8>> = Vec::new();
    for line in data.split(|&b| b == b'\n') {
        let mut l = line.to_vec();
        if l.last() == Some(&b'\r') {
            l.pop();
        }
        v.push(l);
    }
    v
}

fn line_codepoints(line: &[u8]) -> Vec<u32> {
    let s = String::from_utf8_lossy(line);
    s.chars().map(|c| c as u32).collect()
}

impl FigFont {
    /// Load a FIGfont from a path, trying the bare path then `.tlf` and `.flf`.
    pub fn load<P: AsRef<Path>>(path: P) -> Result<FigFont> {
        let path = path.as_ref();
        let candidates = [
            path.to_path_buf(),
            path.with_extension("tlf"),
            path.with_extension("flf"),
        ];

        let mut last_err = CacaError::Invalid;
        for candidate in candidates {
            match std::fs::read(&candidate) {
                Ok(data) => return FigFont::from_bytes(&data),
                Err(_) => last_err = CacaError::Invalid,
            }
        }
        Err(last_err)
    }

    /// Parse a FIGfont from its raw bytes.
    pub fn from_bytes(data: &[u8]) -> Result<FigFont> {
        let lines = split_lines(data);
        let header_line = lines.first().map(|l| l.as_slice()).unwrap_or(&[]);
        let header = String::from_utf8_lossy(header_line);

        // The signature is `flf2a` or `tlf2a`, immediately followed by the
        // hardblank character.
        let sig_pos = header.find("lf2a").ok_or(CacaError::Invalid)?;
        let tokens: Vec<&str> = header.split_whitespace().collect();
        if tokens.is_empty() {
            return Err(CacaError::Invalid);
        }

        let hardblank_str = if tokens[0].len() > sig_pos + 4 {
            &tokens[0][sig_pos + 4..]
        } else {
            "$"
        };
        let (hardblank, _) = utf8_to_utf32(hardblank_str.as_bytes());
        let hardblank = if hardblank == 0 {
            b'$' as u32
        } else {
            hardblank
        };

        let nums: Vec<i64> = tokens[1..]
            .iter()
            .filter_map(|t| t.parse::<i64>().ok())
            .collect();
        if nums.len() < 5 {
            return Err(CacaError::Invalid);
        }

        let height = nums[0] as i32;
        let baseline = nums[1] as i32;
        let max_length = nums[2] as i32;
        let old_layout = nums[3] as i32;
        let comment_lines = nums[4] as i32;
        let _print_direction = nums.get(5).copied().unwrap_or(0) as i32;
        let full_layout = nums.get(6).copied().unwrap_or(0) as i32;
        let _codetag_count = nums.get(7).copied().unwrap_or(0) as i32;

        if height <= 0 || max_length <= 0 || comment_lines < 0 {
            return Err(CacaError::Invalid);
        }

        if !(-1..=63).contains(&old_layout)
            || full_layout > 32767
            || ((full_layout & 0x80) != 0 && (full_layout & 0x3f) == 0 && old_layout != 0)
        {
            return Err(CacaError::Invalid);
        }

        let mut cursor = 1 + comment_lines as usize;
        let mut glyph_lines: Vec<Vec<u32>> = Vec::new();
        let mut lookup: Vec<(u32, u32)> = Vec::new();

        let mut glyphs: i32 = 0;
        loop {
            let mut cp;
            let mut skip = false;

            if glyphs < STD_GLYPHS {
                cp = 32 + glyphs as u32;
            } else if glyphs < EXT_GLYPHS {
                cp = EXT_TAB[(glyphs - STD_GLYPHS) as usize];
            } else {
                // Additional glyph: read a code tag line.
                cp = 0;
                let mut got = false;
                while cursor < lines.len() {
                    let line = &lines[cursor];
                    cursor += 1;
                    if line.is_empty() {
                        continue;
                    }
                    if line[0] == b'-' {
                        // Negative index: skip the glyph's rows.
                        cursor += height as usize;
                        skip = true;
                        got = true;
                        break;
                    }
                    if !line[0].is_ascii_digit() {
                        return Err(CacaError::Invalid);
                    }
                    cp = parse_codetag(line);
                    got = true;
                    break;
                }
                if !got {
                    break;
                }
            }

            // Read `height` rows for this glyph (padded with blanks at EOF).
            for _ in 0..height {
                if skip {
                    glyph_lines.push(Vec::new());
                } else if cursor < lines.len() {
                    glyph_lines.push(line_codepoints(&lines[cursor]));
                    cursor += 1;
                } else {
                    glyph_lines.push(Vec::new());
                }
            }

            lookup.push((cp, 0));
            glyphs += 1;

            if cursor >= lines.len() {
                break;
            }
        }

        if glyphs < EXT_GLYPHS {
            return Err(CacaError::Invalid);
        }

        // Build the font canvas from the collected rows.
        let mut fontcv = Canvas::new(max_length, glyphs * height)?;
        for (j, row) in glyph_lines.iter().enumerate() {
            for (i, &ch) in row.iter().enumerate() {
                if (i as i32) < max_length {
                    fontcv.put_char(i as i32, j as i32, ch);
                }
            }
        }

        let mut ff = FigFont {
            height,
            baseline,
            max_length,
            hardblank,
            old_layout,
            full_layout,
            glyphs,
            lookup,
            fontcv,
            charcv: Canvas::new(1, 1)?,
            term_width: 80,
            x: 0,
            y: 0,
            w: 0,
            h: 0,
            lines: 0,
            hmode: HMode::Default,
            hsmushrule: 0,
        };

        // Post-process: strip end-of-line markers and record glyph widths.
        for j in 0..(height * glyphs) {
            let mut oldch: u32 = 0;
            let mut i = max_length - 1;
            while i >= 0 {
                let mut ch = ff.fontcv.get_char(i, j);

                if ch == ff.hardblank {
                    ff.fontcv.put_char(i, j, 0xa0);
                    ch = 0xa0;
                }

                if oldch != 0 && ch != oldch {
                    let idx = (j / height) as usize;
                    if ff.lookup[idx].1 == 0 {
                        ff.lookup[idx].1 = (i + 1) as u32;
                    }
                } else if oldch != 0 && ch == oldch {
                    ff.fontcv.put_char(i, j, b' ' as u32);
                } else if ch != b' ' as u32 {
                    oldch = ch;
                    ff.fontcv.put_char(i, j, b' ' as u32);
                }

                i -= 1;
            }
        }

        ff.update_settings()?;
        Ok(ff)
    }

    fn update_settings(&mut self) -> Result<()> {
        if (self.full_layout & 0x3f) != 0 {
            self.hsmushrule = self.full_layout & 0x3f;
        } else if self.old_layout > 0 {
            self.hsmushrule = self.old_layout;
        }

        if self.hmode == HMode::Default {
            self.hmode = if self.old_layout == -1 {
                HMode::None
            } else if self.old_layout == 0 && (self.full_layout & 0xc0) == 0x40 {
                HMode::Kern
            } else if (self.old_layout & 0x3f) != 0
                && (self.full_layout & 0x3f) != 0
                && (self.full_layout & 0x80) != 0
            {
                self.hsmushrule = self.full_layout & 0x3f;
                HMode::Smush
            } else if self.old_layout == 0 && (self.full_layout & 0xbf) == 0x80 {
                self.hsmushrule = 0x3f;
                HMode::Smush
            } else {
                HMode::Overlap
            };
        }

        let cw = (self.max_length - 2).max(0);
        self.charcv = Canvas::new(cw, self.height)?;
        Ok(())
    }

    /// The font's standard glyph height.
    pub fn height(&self) -> i32 {
        self.height
    }

    /// The font's baseline.
    pub fn baseline(&self) -> i32 {
        self.baseline
    }

    /// The maximum glyph width declared in the header.
    pub fn max_length(&self) -> i32 {
        self.max_length
    }

    /// The font's hardblank character.
    pub fn hardblank(&self) -> u32 {
        self.hardblank
    }

    /// The number of glyphs loaded.
    pub fn glyph_count(&self) -> i32 {
        self.glyphs
    }

    /// Accumulated number of rendered lines.
    pub fn lines(&self) -> i32 {
        self.lines
    }

    /// Set the rendering width (in cells) before wrapping.
    pub fn set_width(&mut self, width: i32) -> Result<()> {
        self.term_width = width.max(1);
        self.update_settings()
    }

    /// Set the horizontal smushing mode.
    ///
    /// One of `"default"`, `"kern"`, `"smush"`, `"none"` or `"overlap"`.
    pub fn set_smush(&mut self, mode: &str) -> Result<()> {
        self.hmode = match mode.to_ascii_lowercase().as_str() {
            "kern" => HMode::Kern,
            "smush" => HMode::Smush,
            "none" => HMode::None,
            "overlap" => HMode::Overlap,
            _ => HMode::Default,
        };
        self.update_settings()
    }

    /// Render one character at the current cursor position, growing the
    /// canvas as needed.
    pub fn put_char(&mut self, cv: &mut Canvas, ch: u32) -> Result<()> {
        if ch == b'\r' as u32 {
            return Ok(());
        }
        if ch == b'\n' as u32 {
            self.x = 0;
            self.y += self.height;
            return Ok(());
        }

        let c = match self.lookup.iter().position(|&(cp, _)| cp == ch) {
            Some(c) => c,
            None => return Ok(()),
        };

        let w = self.lookup[c].1 as i32;
        let h = self.height;

        // Copy the glyph into the working canvas.
        {
            let fontcv = &mut self.fontcv;
            fontcv.set_handle(0, (c as i32) * self.height);
            self.charcv.blit(0, 0, fontcv, None)?;
        }

        // Wrap if we reached the end of the screen.
        if self.x != 0 && self.x + w > self.term_width {
            self.x = 0;
            self.y += h;
        }

        let overlap = self.compute_overlap(cv, w, h)?;

        if self.x + w - overlap > self.w {
            self.w = (self.x + w - overlap).min(self.term_width);
        }
        if self.y + h > self.h {
            self.h = self.y + h;
        }

        cv.set_size(self.w, self.h)?;

        // Render the glyph.
        for yy in 0..h {
            for xx in 0..w {
                let tmpat = self.fontcv.get_attr(xx, yy + (c as i32) * self.height);
                let ch2 = self.charcv.get_char(xx, yy);
                if ch2 == b' ' as u32 {
                    continue;
                }
                let ch1 = cv.get_char(self.x + xx - overlap, self.y + yy);
                if ch1 == b' ' as u32 || self.hmode != HMode::Smush {
                    cv.put_char(self.x + xx - overlap, self.y + yy, ch2);
                } else {
                    let smushed = hsmush(ch1, ch2, self.hsmushrule);
                    cv.put_char(self.x + xx - overlap, self.y + yy, smushed);
                }
                cv.put_attr(self.x + xx, self.y + yy, tmpat);
            }
        }

        self.x += w - overlap;
        Ok(())
    }

    fn compute_overlap(&self, cv: &Canvas, w: i32, h: i32) -> Result<i32> {
        let charcv = &self.charcv;

        match self.hmode {
            HMode::Smush | HMode::Kern | HMode::Overlap => {
                let mut overlap = w;
                for y in 0..h {
                    let mut xright = 0;
                    while xright < overlap && charcv.get_char(xright, y) == b' ' as u32 {
                        xright += 1;
                    }

                    let mut xleft = 0;
                    while xright + xleft < overlap
                        && xleft < self.x
                        && cv.get_char(self.x - 1 - xleft, self.y + y) == b' ' as u32
                    {
                        xleft += 1;
                    }

                    if self.hmode == HMode::Overlap && xleft < self.x {
                        xleft += 1;
                    }

                    if self.hmode == HMode::Smush
                        && xleft < self.x
                        && hsmush(
                            cv.get_char(self.x - 1 - xleft, self.y + y),
                            charcv.get_char(xright, y),
                            self.hsmushrule,
                        ) != 0
                    {
                        xleft += 1;
                    }

                    if xleft + xright < overlap {
                        overlap = xleft + xright;
                    }
                }
                Ok(overlap)
            }
            HMode::None => Ok(0),
            HMode::Default => Err(CacaError::Invalid),
        }
    }

    /// Finish rendering: resize the canvas to the used area, replace
    /// hardblanks with spaces and reset the cursor.
    pub fn flush(&mut self, cv: &mut Canvas) -> Result<()> {
        cv.set_size(self.w, self.h)?;

        for y in 0..self.h {
            for x in 0..self.w {
                if cv.get_char(x, y) == 0xa0 {
                    let attr = cv.get_attr(x, y);
                    cv.put_char(x, y, b' ' as u32);
                    cv.put_attr(x, y, attr);
                }
            }
        }

        self.x = 0;
        self.y = 0;
        self.w = 0;
        self.h = 0;
        self.lines += cv.height();
        Ok(())
    }
}

impl std::fmt::Debug for FigFont {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FigFont")
            .field("height", &self.height)
            .field("baseline", &self.baseline)
            .field("max_length", &self.max_length)
            .field("glyphs", &self.glyphs)
            .finish()
    }
}

fn parse_codetag(line: &[u8]) -> u32 {
    let s = String::from_utf8_lossy(line);
    let s = s.trim();
    let s = s.split_whitespace().next().unwrap_or("0");
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u32::from_str_radix(hex, 16).unwrap_or(0)
    } else {
        s.parse::<u32>().unwrap_or(0)
    }
}

/// FIGfont horizontal smushing rules.
fn hsmush(ch1: u32, ch2: u32, rule: i32) -> u32 {
    const CHARLIST: &[u8] = b"|/\\[]{}()<>";

    // Rule 1: equal character smushing.
    if (rule & 0x01) != 0 && ch1 == ch2 && ch1 != 0xa0 {
        return ch2;
    }

    if ch1 < 0x80 && ch2 < 0x80 {
        // Rule 2: underscore smushing.
        if (rule & 0x02) != 0 {
            if ch1 == b'_' as u32 && CHARLIST.contains(&(ch2 as u8)) {
                return ch2;
            }
            if ch2 == b'_' as u32 && CHARLIST.contains(&(ch1 as u8)) {
                return ch1;
            }
        }

        // Rule 3: hierarchy smushing.
        if (rule & 0x04) != 0 {
            let p1 = CHARLIST.iter().position(|&c| c as u32 == ch1);
            let p2 = CHARLIST.iter().position(|&c| c as u32 == ch2);
            if let (Some(p1), Some(p2)) = (p1, p2) {
                let cl1 = p1.div_ceil(2);
                let cl2 = p2.div_ceil(2);
                if cl1 < cl2 {
                    return ch2;
                }
                if cl1 > cl2 {
                    return ch1;
                }
            }
        }

        // Rule 4: opposite pair smushing.
        if (rule & 0x08) != 0 {
            let s = ch1 + ch2;
            let p = ch1 * ch2;
            if p == 15375 || p == 8463 || (p == 1640 && s == 81) {
                return b'|' as u32;
            }
        }

        // Rule 5: big X smushing.
        if (rule & 0x10) != 0 {
            match (ch1 << 8) | ch2 {
                0x2f5c => return b'|' as u32,
                0x5c2f => return b'Y' as u32,
                0x3e3c => return b'X' as u32,
                _ => {}
            }
        }

        // Rule 6: hardblank smushing.
        if (rule & 0x20) != 0 && ch1 == ch2 && ch1 == 0xa0 {
            return 0xa0;
        }
    }

    0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic_font() -> Vec<u8> {
        let mut s = String::new();
        s.push_str("flf2a$ 1 1 5 -1 0\n");
        // 95 standard glyphs (32..=126).
        for cp in 32u32..=126 {
            if cp == 32 {
                s.push_str("   @@\n");
            } else {
                let ch = char::from_u32(cp).unwrap_or('?');
                s.push_str(&format!("{}@@\n", ch));
            }
        }
        // 7 extended glyphs.
        for _ in 0..7 {
            s.push_str("x@@\n");
        }
        s.into_bytes()
    }

    #[test]
    fn parse_and_render() {
        let ff_data = synthetic_font();
        let mut ff = FigFont::from_bytes(&ff_data).unwrap();
        assert_eq!(ff.height(), 1);
        assert_eq!(ff.glyph_count(), 102);

        let mut cv = Canvas::new(1, 1).unwrap();
        ff.put_char(&mut cv, b'A' as u32).unwrap();
        ff.set_smush("none").unwrap();
        ff.put_char(&mut cv, b'B' as u32).unwrap();
        ff.flush(&mut cv).unwrap();

        assert!(cv.width() >= 2);
        assert_eq!(cv.get_char(0, 0), b'A' as u32);
        assert_eq!(cv.get_char(1, 0), b'B' as u32);
    }

    #[test]
    fn hardblank_replaced_on_flush() {
        let mut s = String::new();
        s.push_str("flf2a$ 1 1 5 -1 0\n");
        for _ in 0..102 {
            s.push_str("@$$\n");
        }
        // Note: the line above has endmark '$'; content is '@'.
        let mut ff = FigFont::from_bytes(s.as_bytes()).unwrap();
        let mut cv = Canvas::new(1, 1).unwrap();
        ff.put_char(&mut cv, b'A' as u32).unwrap();
        ff.flush(&mut cv).unwrap();
        assert_ne!(cv.get_char(0, 0), 0xa0);
    }

    #[test]
    fn bad_header_rejected() {
        assert!(FigFont::from_bytes(b"not a font\n").is_err());
        assert!(FigFont::from_bytes(b"").is_err());
    }

    #[test]
    fn smush_rule_one() {
        assert_eq!(hsmush(b'|' as u32, b'|' as u32, 0x01), b'|' as u32);
        assert_eq!(hsmush(b'/' as u32, b'/' as u32, 0x01), b'/' as u32);
    }
}
