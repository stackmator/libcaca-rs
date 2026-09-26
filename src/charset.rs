//! Character set conversions.
//!
//! Port of `caca/charset.c`: UTF-8, UTF-32, CP437 and ASCII conversions.

/// Number of trailing bytes that follow a given UTF-8 lead byte.
static TRAILING: [u8; 256] = {
    let mut t = [0u8; 256];
    let mut i = 0x80;
    while i <= 0xbf {
        t[i] = 1;
        i += 1;
    }
    i = 0xc0;
    while i <= 0xdf {
        t[i] = 1;
        i += 1;
    }
    i = 0xe0;
    while i <= 0xef {
        t[i] = 2;
        i += 1;
    }
    i = 0xf0;
    while i <= 0xf7 {
        t[i] = 3;
        i += 1;
    }
    i = 0xf8;
    while i <= 0xfb {
        t[i] = 4;
        i += 1;
    }
    i = 0xfc;
    while i <= 0xfd {
        t[i] = 5;
        i += 1;
    }
    t
};

static OFFSETS: [u32; 6] = [
    0x0000_0000,
    0x0000_3080,
    0x000E_2080,
    0x03C8_2080,
    0xFA08_2080,
    0x8208_2080,
];

static CP437_LOOKUP1: [u32; 31] = [
    // 0x01 - 0x0f
    0x263a, 0x263b, 0x2665, 0x2666, 0x2663, 0x2660, 0x2022, 0x25d8, 0x25cb, 0x25d9, 0x2642,
    0x2640, 0x266a, 0x266b, 0x263c,
    // 0x10 - 0x1f
    0x25ba, 0x25c4, 0x2195, 0x203c, 0xb6, 0xa7, 0x25ac, 0x21a8, 0x2191, 0x2193, 0x2192,
    0x2190, 0x221f, 0x2194, 0x25b2, 0x25bc,
];

static CP437_LOOKUP2: [u32; 129] = [
    // 0x7f
    0x2302, // 0x80 - 0x8f
    0xc7, 0xfc, 0xe9, 0xe2, 0xe4, 0xe0, 0xe5, 0xe7, 0xea, 0xeb, 0xe8, 0xef, 0xee, 0xec,
    0xc4, 0xc5, // 0x90 - 0x9f
    0xc9, 0xe6, 0xc6, 0xf4, 0xf6, 0xf2, 0xfb, 0xf9, 0xff, 0xd6, 0xdc, 0xa2, 0xa3, 0xa5,
    0x20a7, 0x192, // 0xa0 - 0xaf
    0xe1, 0xed, 0xf3, 0xfa, 0xf1, 0xd1, 0xaa, 0xba, 0xbf, 0x2310, 0xac, 0xbd, 0xbc, 0xa1,
    0xab, 0xbb, // 0xb0 - 0xbf
    0x2591, 0x2592, 0x2593, 0x2502, 0x2524, 0x2561, 0x2562, 0x2556, 0x2555, 0x2563, 0x2551,
    0x2557, 0x255d, 0x255c, 0x255b, 0x2510, // 0xc0 - 0xcf
    0x2514, 0x2534, 0x252c, 0x251c, 0x2500, 0x253c, 0x255e, 0x255f, 0x255a, 0x2554, 0x2569,
    0x2566, 0x2560, 0x2550, 0x256c, 0x2567, // 0xd0 - 0xdf
    0x2568, 0x2564, 0x2565, 0x2559, 0x2558, 0x2552, 0x2553, 0x256b, 0x256a, 0x2518, 0x250c,
    0x2588, 0x2584, 0x258c, 0x2590, 0x2580, // 0xe0 - 0xef
    0x3b1, 0xdf, 0x393, 0x3c0, 0x3a3, 0x3c3, 0xb5, 0x3c4, 0x3a6, 0x398, 0x3a9, 0x3b4,
    0x221e, 0x3c6, 0x3b5, 0x2229, // 0xf0 - 0xff
    0x2261, 0xb1, 0x2265, 0x2264, 0x2320, 0x2321, 0xf7, 0x2248, 0xb0, 0x2219, 0xb7, 0x221a,
    0x207f, 0xb2, 0x25a0, 0xa0,
];

/// Convert a UTF-8 sequence to a UTF-32 character.
///
/// Returns the codepoint and the number of bytes consumed. If the sequence is
/// incomplete (terminated by NUL or a short slice), `(0, 0)` is returned, which
/// mirrors `caca_utf8_to_utf32`.
pub fn utf8_to_utf32(s: &[u8]) -> (u32, usize) {
    if s.is_empty() || s[0] == 0 {
        return (0, 0);
    }

    let todo = TRAILING[s[0] as usize] as usize;
    let mut ret: u32 = 0;
    let mut i = 0;

    loop {
        if i >= s.len() || s[i] == 0 {
            return (0, 0);
        }

        ret += (s[i] as u32) << (6 * (todo - i));
        i += 1;

        if todo < i {
            break;
        }
    }

    (ret.wrapping_sub(OFFSETS[todo]), i)
}

/// Convert a UTF-32 character to UTF-8 into `buf`, returning the byte count.
pub fn utf32_to_utf8(buf: &mut [u8], ch: u32) -> usize {
    const MARK: [u8; 7] = [0x00, 0x00, 0xC0, 0xE0, 0xF0, 0xF8, 0xFC];

    if ch < 0x80 {
        if !buf.is_empty() {
            buf[0] = ch as u8;
        }
        return 1;
    }

    let bytes = if ch < 0x800 {
        2
    } else if ch < 0x10000 {
        3
    } else {
        4
    };

    if buf.len() < bytes {
        return bytes;
    }

    let mut ch = ch;
    let mut parser = bytes;
    match bytes {
        4 => {
            parser -= 1;
            buf[parser] = ((ch | 0x80) & 0xbf) as u8;
            ch >>= 6;
            parser -= 1;
            buf[parser] = ((ch | 0x80) & 0xbf) as u8;
            ch >>= 6;
            parser -= 1;
            buf[parser] = ((ch | 0x80) & 0xbf) as u8;
            ch >>= 6;
        }
        3 => {
            parser -= 1;
            buf[parser] = ((ch | 0x80) & 0xbf) as u8;
            ch >>= 6;
            parser -= 1;
            buf[parser] = ((ch | 0x80) & 0xbf) as u8;
            ch >>= 6;
        }
        2 => {
            parser -= 1;
            buf[parser] = ((ch | 0x80) & 0xbf) as u8;
            ch >>= 6;
        }
        _ => unreachable!(),
    }
    buf[0] = (ch | MARK[bytes] as u32) as u8;

    bytes
}

/// Convenience wrapper around [`utf32_to_utf8`].
pub fn utf32_to_utf8_array(ch: u32) -> ([u8; 4], usize) {
    let mut buf = [0u8; 4];
    let n = utf32_to_utf8(&mut buf, ch);
    (buf, n)
}

/// Append the UTF-8 encoding of `ch` to a `String`.
pub fn push_utf32(out: &mut String, ch: u32) {
    if let Some(c) = char::from_u32(ch) {
        out.push(c);
    } else {
        out.push('\u{fffd}');
    }
}

/// Iterate the UTF-32 codepoints of a UTF-8 byte slice.
pub struct Utf8Iter<'a> {
    data: &'a [u8],
}

impl<'a> Utf8Iter<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Utf8Iter { data }
    }
}

impl Iterator for Utf8Iter<'_> {
    type Item = u32;

    fn next(&mut self) -> Option<u32> {
        if self.data.is_empty() {
            return None;
        }
        let (ch, rd) = utf8_to_utf32(self.data);
        if rd == 0 {
            // Skip a byte to guarantee progress on malformed input.
            self.data = &self.data[1..];
            return Some(0xfffd);
        }
        self.data = &self.data[rd..];
        Some(ch)
    }
}

/// Convert a UTF-32 character to CP437, or `b'?'` if not representable.
pub fn utf32_to_cp437(ch: u32) -> u8 {
    if ch < 0x20 {
        return b'?';
    }
    if ch < 0x80 {
        return ch as u8;
    }

    for (i, &v) in CP437_LOOKUP1.iter().enumerate() {
        if v == ch {
            return 0x01 + i as u8;
        }
    }

    for (i, &v) in CP437_LOOKUP2.iter().enumerate() {
        if v == ch {
            return 0x7f + i as u8;
        }
    }

    b'?'
}

/// Convert a CP437 character to UTF-32, or zero for control characters.
pub fn cp437_to_utf32(ch: u8) -> u32 {
    if ch > 0x7f {
        return CP437_LOOKUP2[ch as usize - 0x7f];
    }
    if ch >= 0x20 {
        return ch as u32;
    }
    if ch > 0 {
        return CP437_LOOKUP1[ch as usize - 1];
    }
    0
}

/// Convert a UTF-32 character to an ASCII approximation.
pub fn utf32_to_ascii(ch: u32) -> char {
    if ch < 0x80 {
        return ch as u8 as char;
    }

    // Fullwidth forms.
    if ch > 0x0000_ff00 && ch < 0x0000_ff5f {
        return char::from_u32(b' ' as u32 + (ch - 0x0000_ff00)).unwrap_or('?');
    }

    match ch {
        0x0000_00a0 | 0x0000_3000 => ' ',
        0x0000_00a3 => 'f',
        0x0000_00b0 => '\'',
        0x0000_00b1 => '#',
        0x0000_00b7 | 0x0000_2219 | 0x0000_30fb => '.',
        0x0000_03c0 => '*',
        0x0000_2018 | 0x0000_2019 => '\'',
        0x0000_201c | 0x0000_201d => '"',
        0x0000_2190 => '<',
        0x0000_2191 => '^',
        0x0000_2192 => '>',
        0x0000_2193 => 'v',
        0x0000_2260 => '!',
        0x0000_2261 => '=',
        0x0000_2264 => '<',
        0x0000_2265 => '>',
        0x0000_23ba..=0x0000_23bd | 0x0000_2500 | 0x0000_2550 => '-',
        0x0000_2502 | 0x0000_2551 => '|',
        0x0000_250c
        | 0x0000_2552
        | 0x0000_2553
        | 0x0000_2554
        | 0x0000_2514
        | 0x0000_2558
        | 0x0000_2559
        | 0x0000_255a
        | 0x0000_251c
        | 0x0000_255e
        | 0x0000_255f
        | 0x0000_2560
        | 0x0000_252c
        | 0x0000_2564
        | 0x0000_2565
        | 0x0000_2566
        | 0x0000_2534
        | 0x0000_2567
        | 0x0000_2568
        | 0x0000_2569
        | 0x0000_253c
        | 0x0000_256a
        | 0x0000_256b
        | 0x0000_256c => '+',
        0x0000_2510
        | 0x0000_2555
        | 0x0000_2556
        | 0x0000_2557
        | 0x0000_2518
        | 0x0000_255b
        | 0x0000_255c
        | 0x0000_255d
        | 0x0000_2524
        | 0x0000_2561
        | 0x0000_2562
        | 0x0000_2563 => '+',
        0x0000_2591
        | 0x0000_2592
        | 0x0000_2593
        | 0x0000_2588
        | 0x0000_258c
        | 0x0000_2590
        | 0x0000_25a0
        | 0x0000_25ac
        | 0x0000_25ae => '#',
        0x0000_2580 => '"',
        0x0000_2584 => ',',
        0x0000_25c6 | 0x0000_2666 => '+',
        0x0000_2022 | 0x0000_25cb | 0x0000_25cf | 0x0000_2603 | 0x0000_263c => 'o',
        0x0000_301c => '~',
        _ => '?',
    }
}

/// Whether a UTF-32 character occupies two cells (fullwidth).
pub fn utf32_is_fullwidth(ch: u32) -> bool {
    if ch < 0x2e80 {
        return false;
    }
    if ch < 0xa700 {
        return true;
    }
    if ch < 0xac00 {
        return false;
    }
    if ch < 0xd800 {
        return true;
    }
    if ch < 0xf900 {
        return false;
    }
    if ch < 0xfb00 {
        return true;
    }
    if ch < 0xfe20 {
        return false;
    }
    if ch < 0xfe70 {
        return true;
    }
    if ch < 0xff00 {
        return false;
    }
    if ch < 0xff61 {
        return true;
    }
    if ch < 0xffe0 {
        return false;
    }
    if ch < 0xffe8 {
        return true;
    }
    if ch < 0x20000 {
        return false;
    }
    if ch < 0xe0000 {
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf8_roundtrip() {
        for &ch in &[0x24u32, 0xa2, 0x20ac, 0x1f600] {
            let (buf, n) = utf32_to_utf8_array(ch);
            let (back, rd) = utf8_to_utf32(&buf);
            assert_eq!(n, rd);
            assert_eq!(back, ch);
        }
    }

    #[test]
    fn cp437_roundtrip() {
        assert_eq!(cp437_to_utf32(0x02), 0x263b);
        assert_eq!(utf32_to_cp437(0x263b), 0x02);
        assert_eq!(utf32_to_cp437(0x2500), 0xc4);
        assert_eq!(cp437_to_utf32(0xc4), 0x2500);
    }

    #[test]
    fn fullwidth() {
        assert!(utf32_is_fullwidth(0x3000));
        assert!(!utf32_is_fullwidth(b'a' as u32));
    }

    #[test]
    fn ascii_approx() {
        assert_eq!(utf32_to_ascii(0x2500), '-');
        assert_eq!(utf32_to_ascii(0x2588), '#');
    }
}
