//! Character attributes and colourspace conversions.
//!
//! Port of `caca/attr.c`. An attribute is a 32-bit value with the following
//! layout (MSB to LSB):
//!
//! ```text
//! 3 bits  background alpha
//! 4 bits  background red
//! 4 bits  background green
//! 3 bits  background blue
//! 3 bits  foreground alpha
//! 4 bits  foreground red
//! 4 bits  foreground green
//! 3 bits  foreground blue
//! 4 bits  bold, italics, underline and blink flags
//! ```

use core::ops::{BitOr, BitOrAssign, BitAnd, BitXor, Not};

/// A `libcaca` colour keyword.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Color {
    Black = 0x00,
    Blue = 0x01,
    Green = 0x02,
    Cyan = 0x03,
    Red = 0x04,
    Magenta = 0x05,
    Brown = 0x06,
    LightGray = 0x07,
    DarkGray = 0x08,
    LightBlue = 0x09,
    LightGreen = 0x0a,
    LightCyan = 0x0b,
    LightRed = 0x0c,
    LightMagenta = 0x0d,
    Yellow = 0x0e,
    White = 0x0f,
    /// The output driver's default colour.
    Default = 0x10,
    /// The transparent colour.
    Transparent = 0x20,
}

impl Color {
    /// Build a colour from its raw byte value, or `None` if out of range.
    pub const fn from_u8(v: u8) -> Option<Color> {
        Some(match v {
            0x00 => Color::Black,
            0x01 => Color::Blue,
            0x02 => Color::Green,
            0x03 => Color::Cyan,
            0x04 => Color::Red,
            0x05 => Color::Magenta,
            0x06 => Color::Brown,
            0x07 => Color::LightGray,
            0x08 => Color::DarkGray,
            0x09 => Color::LightBlue,
            0x0a => Color::LightGreen,
            0x0b => Color::LightCyan,
            0x0c => Color::LightRed,
            0x0d => Color::LightMagenta,
            0x0e => Color::Yellow,
            0x0f => Color::White,
            0x10 => Color::Default,
            0x20 => Color::Transparent,
            _ => return None,
        })
    }

    /// The raw byte value of this colour.
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    /// The 12-bit RGB value for the 16 base ANSI colours.
    pub const fn rgb12(self) -> u16 {
        ansitab16()[(self as u8 & 0x0f) as usize] & 0x0fff
    }
}

/// A style bitmask (`BOLD`, `ITALICS`, `UNDERLINE`, `BLINK`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Style(u8);

impl Style {
    pub const NONE: Style = Style(0);
    pub const BOLD: Style = Style(0x01);
    pub const ITALICS: Style = Style(0x02);
    pub const UNDERLINE: Style = Style(0x04);
    pub const BLINK: Style = Style(0x08);
    pub const ALL: Style = Style(0x0f);

    /// Raw bit representation.
    pub const fn bits(self) -> u8 {
        self.0
    }

    /// Whether this style contains all bits of `other`.
    pub const fn contains(self, other: Style) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Whether this style contains no bits.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl BitOr for Style {
    type Output = Style;
    fn bitor(self, rhs: Style) -> Style {
        Style(self.0 | rhs.0)
    }
}

impl BitOrAssign for Style {
    fn bitor_assign(&mut self, rhs: Style) {
        self.0 |= rhs.0;
    }
}

impl BitAnd for Style {
    type Output = Style;
    fn bitand(self, rhs: Style) -> Style {
        Style(self.0 & rhs.0)
    }
}

impl BitXor for Style {
    type Output = Style;
    fn bitxor(self, rhs: Style) -> Style {
        Style(self.0 ^ rhs.0)
    }
}

impl Not for Style {
    type Output = Style;
    fn not(self) -> Style {
        Style(!self.0 & 0x0f)
    }
}

/// A 32-bit character attribute.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(transparent)]
pub struct Attr(pub u32);

impl Attr {
    /// The default attribute (black foreground / black background ANSI).
    pub const NONE: Attr = Attr(0);

    /// Wrap a raw 32-bit attribute value.
    pub const fn from_raw(v: u32) -> Attr {
        Attr(v)
    }

    /// The raw 32-bit value.
    pub const fn raw(self) -> u32 {
        self.0
    }

    /// The style bits (low nibble).
    pub const fn style(self) -> Style {
        Style((self.0 & 0x0f) as u8)
    }

    /// Return a copy with the style bits replaced.
    pub const fn with_style(self, style: Style) -> Attr {
        Attr((self.0 & 0xffff_fff0) | style.0 as u32)
    }

    /// Set (add) style bits without touching the colours.
    pub const fn set_style(self, style: Style) -> Attr {
        Attr(self.0 | style.0 as u32)
    }

    /// Clear the given style bits.
    pub const fn unset_style(self, style: Style) -> Attr {
        Attr(self.0 & !(style.0 as u32))
    }

    /// Toggle the given style bits.
    pub const fn toggle_style(self, style: Style) -> Attr {
        Attr(self.0 ^ (style.0 as u32))
    }

    /// The raw 14-bit foreground value.
    pub const fn fg14(self) -> u16 {
        ((self.0 >> 4) & 0x3fff) as u16
    }

    /// The raw 14-bit background value.
    pub const fn bg14(self) -> u16 {
        (self.0 >> 18) as u16
    }

    /// Build an attribute from ANSI foreground and background colours.
    pub fn from_ansi(fg: Color, bg: Color) -> Attr {
        let fg = fg.as_u8() as u32;
        let bg = bg.as_u8() as u32;
        Attr(((bg | 0x40) << 18) | ((fg | 0x40) << 4))
    }

    /// Build an attribute from 16-bit ARGB foreground and background colours.
    pub fn from_argb(fg: u16, bg: u16) -> Attr {
        let fg = encode_argb14(fg);
        let bg = encode_argb14(bg);
        Attr(((bg as u32) << 18) | ((fg as u32) << 4))
    }

    /// The DOS ANSI value (background in high nibble, foreground in low).
    pub fn to_ansi(self) -> u8 {
        let fg = nearest_ansi(self.fg14());
        let bg = nearest_ansi(self.bg14());

        (if fg < 0x10 { fg } else { Color::LightGray.as_u8() })
            | ((if bg < 0x10 { bg } else { Color::Black.as_u8() }) << 4)
    }

    /// The ANSI foreground value.
    pub fn to_ansi_fg(self) -> u8 {
        nearest_ansi(self.fg14())
    }

    /// The ANSI background value.
    pub fn to_ansi_bg(self) -> u8 {
        nearest_ansi(self.bg14())
    }

    /// The 12-bit RGB foreground value.
    pub fn to_rgb12_fg(self) -> u16 {
        let fg = self.fg14();

        if fg < (0x10 | 0x40) {
            return ansitab16()[(fg ^ 0x40) as usize] & 0x0fff;
        }

        if fg == (Color::Default.as_u8() as u16 | 0x40) {
            return ansitab16()[Color::LightGray.as_u8() as usize] & 0x0fff;
        }

        if fg == (Color::Transparent.as_u8() as u16 | 0x40) {
            return ansitab16()[Color::LightGray.as_u8() as usize] & 0x0fff;
        }

        (fg << 1) & 0x0fff
    }

    /// The 12-bit RGB background value.
    pub fn to_rgb12_bg(self) -> u16 {
        let bg = self.bg14();

        if bg < (0x10 | 0x40) {
            return ansitab16()[(bg ^ 0x40) as usize] & 0x0fff;
        }

        if bg == (Color::Default.as_u8() as u16 | 0x40) {
            return ansitab16()[Color::Black.as_u8() as usize] & 0x0fff;
        }

        if bg == (Color::Transparent.as_u8() as u16 | 0x40) {
            return ansitab16()[Color::Black.as_u8() as usize] & 0x0fff;
        }

        (bg << 1) & 0x0fff
    }

    /// The 64-bit ARGB values, in `[bg.a, bg.r, bg.g, bg.b, fg.a, fg.r, fg.g, fg.b]`
    /// order.
    pub fn to_argb64(self) -> [u8; 8] {
        let fg = self.fg14();
        let bg = self.bg14();

        let bg = if bg < (0x10 | 0x40) {
            ansitab16()[(bg ^ 0x40) as usize]
        } else if bg == (Color::Default.as_u8() as u16 | 0x40) {
            ansitab16()[Color::Black.as_u8() as usize]
        } else if bg == (Color::Transparent.as_u8() as u16 | 0x40) {
            0x0fff
        } else {
            ((bg << 2) & 0xf000) | ((bg << 1) & 0x0fff)
        };

        let fg = if fg < (0x10 | 0x40) {
            ansitab16()[(fg ^ 0x40) as usize]
        } else if fg == (Color::Default.as_u8() as u16 | 0x40) {
            ansitab16()[Color::LightGray.as_u8() as usize]
        } else if fg == (Color::Transparent.as_u8() as u16 | 0x40) {
            0x0fff
        } else {
            ((fg << 2) & 0xf000) | ((fg << 1) & 0x0fff)
        };

        [
            (bg >> 12) as u8,
            ((bg >> 8) & 0xf) as u8,
            ((bg >> 4) & 0xf) as u8,
            (bg & 0xf) as u8,
            (fg >> 12) as u8,
            ((fg >> 8) & 0xf) as u8,
            ((fg >> 4) & 0xf) as u8,
            (fg & 0xf) as u8,
        ]
    }

    /// The 24-bit RGB foreground value.
    pub fn to_rgb24_fg(self) -> u32 {
        rgb12_to_24(self.to_rgb12_fg())
    }

    /// The 24-bit RGB background value.
    pub fn to_rgb24_bg(self) -> u32 {
        rgb12_to_24(self.to_rgb12_bg())
    }
}

/// RGB colours for the ANSI palette (gnome-terminal values).
pub const fn ansitab16() -> [u16; 16] {
    [
        0xf000, 0xf00a, 0xf0a0, 0xf0aa, 0xfa00, 0xfa0a, 0xfa50, 0xfaaa, 0xf555, 0xf55f,
        0xf5f5, 0xf5ff, 0xff55, 0xff5f, 0xfff5, 0xffff,
    ]
}

/// Same palette on 14 bits (3-4-4-3).
pub const fn ansitab14() -> [u16; 16] {
    [
        0x3800, 0x3805, 0x3850, 0x3855, 0x3d00, 0x3d05, 0x3d28, 0x3d55, 0x3aaa, 0x3aaf,
        0x3afa, 0x3aff, 0x3faa, 0x3faf, 0x3ffa, 0x3fff,
    ]
}

fn encode_argb14(color: u16) -> u16 {
    // `if(color < 0x100) color += 0x100;`
    let color = if color < 0x100 { color + 0x100 } else { color };
    ((color >> 1) & 0x7ff) | ((color >> 13) << 11)
}

fn nearest_ansi(argb14: u16) -> u8 {
    if argb14 < (0x10 | 0x40) {
        return (argb14 ^ 0x40) as u8;
    }

    if argb14 == (Color::Default.as_u8() as u16 | 0x40)
        || argb14 == (Color::Transparent.as_u8() as u16 | 0x40)
    {
        return (argb14 ^ 0x40) as u8;
    }

    if argb14 < 0x0fff {
        return Color::Transparent.as_u8();
    }

    let mut best: u8 = Color::Default.as_u8();
    let mut dist: i32 = 0x3fff;
    let tab = ansitab14();

    for (i, &entry) in tab.iter().enumerate() {
        let mut d: i32 = 0;
        let a = ((entry >> 7) & 0xf) as i32;
        let b = ((argb14 >> 7) & 0xf) as i32;
        d += (a - b) * (a - b);

        let a = ((entry >> 3) & 0xf) as i32;
        let b = ((argb14 >> 3) & 0xf) as i32;
        d += (a - b) * (a - b);

        let a = ((entry << 1) & 0xf) as i32;
        let b = ((argb14 << 1) & 0xf) as i32;
        d += (a - b) * (a - b);

        if d < dist {
            dist = d;
            best = i as u8;
        }
    }

    best
}

fn rgb12_to_24(i: u16) -> u32 {
    (((i as u32 & 0xf00) >> 8) * 0x11_0000)
        | (((i as u32 & 0x0f0) >> 4) * 0x00_1100)
        | ((i as u32 & 0x00f) * 0x00_0011)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ansi_roundtrip() {
        let a = Attr::from_ansi(Color::LightRed, Color::Blue);
        assert_eq!(a.to_ansi_fg(), Color::LightRed.as_u8());
        assert_eq!(a.to_ansi_bg(), Color::Blue.as_u8());
        assert_eq!(a.to_ansi(), (Color::Blue.as_u8() << 4) | Color::LightRed.as_u8());
    }

    #[test]
    fn style_ops() {
        let a = Attr::from_ansi(Color::White, Color::Black);
        let b = a.set_style(Style::BOLD | Style::UNDERLINE);
        assert!(b.style().contains(Style::BOLD));
        assert!(b.style().contains(Style::UNDERLINE));
        assert_eq!(b.to_ansi_fg(), Color::White.as_u8());
        assert_eq!(b.unset_style(Style::BOLD).style().bits(), 0x04);
    }

    #[test]
    fn argb_defaults() {
        let a = Attr::from_ansi(Color::Default, Color::Default);
        assert_eq!(a.to_rgb12_fg(), ansitab16()[Color::LightGray.as_u8() as usize] & 0x0fff);
        assert_eq!(a.to_rgb12_bg(), ansitab16()[Color::Black.as_u8() as usize] & 0x0fff);
    }
}
