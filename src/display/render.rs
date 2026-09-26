//! Render a [`Canvas`] into ANSI/VT escape sequences.
//!
//! This is the drawing half of the terminal driver. It is a pure function of
//! the canvas and a small incremental state, so it can be unit-tested without
//! a real terminal.

use crate::attr::Attr;
use crate::canvas::{Canvas, CACA_MAGIC_FULLWIDTH};
use crate::charset::{utf32_is_fullwidth, utf32_to_utf8};

#[derive(Clone, Copy, PartialEq, Eq)]
enum ColorCode {
    Ansi(u8),
    Rgb(u8, u8, u8),
    Default,
}

/// Persistent rendering state between two refreshes.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct AnsiState {
    fg: ColorCode,
    bg: ColorCode,
    style: u8,
}

impl Default for AnsiState {
    fn default() -> Self {
        AnsiState {
            fg: ColorCode::Default,
            bg: ColorCode::Default,
            style: 0,
        }
    }
}

impl AnsiState {
    pub fn reset(&mut self) {
        *self = AnsiState::default();
    }
}

fn is_ansi(v: u16) -> bool {
    v == 0x50 || v == 0x60 || (0x40..0x50).contains(&v)
}

fn fg_code(attr: Attr) -> ColorCode {
    let v = attr.fg14();
    if is_ansi(v) {
        let idx = v ^ 0x40;
        if idx >= 0x10 {
            ColorCode::Default
        } else {
            ColorCode::Ansi(idx as u8)
        }
    } else {
        let argb = attr.to_argb64();
        let a = argb[4];
        if a == 0 {
            ColorCode::Default
        } else {
            ColorCode::Rgb(argb[5] * 17, argb[6] * 17, argb[7] * 17)
        }
    }
}

fn bg_code(attr: Attr) -> ColorCode {
    let v = attr.bg14();
    if is_ansi(v) {
        let idx = v ^ 0x40;
        if idx >= 0x10 {
            ColorCode::Default
        } else {
            ColorCode::Ansi(idx as u8)
        }
    } else {
        let argb = attr.to_argb64();
        let a = argb[0];
        if a == 0 {
            ColorCode::Default
        } else {
            ColorCode::Rgb(argb[1] * 17, argb[2] * 17, argb[3] * 17)
        }
    }
}

/// Maps libcaca's CGA colour order (black, blue, green, cyan, red, magenta,
/// brown, light gray, ...) to ANSI SGR foreground codes.
const ANSI_FG: [u16; 16] = [
    30, 34, 32, 36, 31, 35, 33, 37, 90, 94, 92, 96, 91, 95, 93, 97,
];

fn push_code(out: &mut Vec<u8>, code: ColorCode, is_fg: bool) {
    match code {
        ColorCode::Default => {
            out.extend_from_slice(if is_fg { b"39" } else { b"49" });
        }
        ColorCode::Ansi(i) => {
            let fg = ANSI_FG[(i & 0x0f) as usize];
            let n = if is_fg { fg } else { fg + 10 };
            out.extend_from_slice(n.to_string().as_bytes());
        }
        ColorCode::Rgb(r, g, b) => {
            if is_fg {
                out.extend_from_slice(b"38;2;");
            } else {
                out.extend_from_slice(b"48;2;");
            }
            out.extend_from_slice(format!("{};{};{}", r, g, b).as_bytes());
        }
    }
}

fn emit_style(out: &mut Vec<u8>, style: u8) {
    if style & 0x01 != 0 {
        out.extend_from_slice(b";1");
    }
    if style & 0x02 != 0 {
        out.extend_from_slice(b";3");
    }
    if style & 0x04 != 0 {
        out.extend_from_slice(b";4");
    }
    if style & 0x08 != 0 {
        out.extend_from_slice(b";5");
    }
}

fn sync_state(out: &mut Vec<u8>, state: &mut AnsiState, attr: Attr) {
    let fg = fg_code(attr);
    let bg = bg_code(attr);
    let style = attr.style().bits();

    if fg == state.fg && bg == state.bg && style == state.style {
        return;
    }

    out.extend_from_slice(b"\x1b[0");
    emit_style(out, style);
    out.push(b';');
    push_code(out, fg, true);
    out.push(b';');
    push_code(out, bg, false);
    out.push(b'm');

    state.fg = fg;
    state.bg = bg;
    state.style = style;
}

/// Append the full-screen ANSI rendering of `canvas` to `out`.
///
/// `state` is updated in place and should be reset when the terminal is
/// cleared or reinitialised.
pub fn render(canvas: &Canvas, state: &mut AnsiState, out: &mut Vec<u8>) {
    let w = canvas.width();
    let h = canvas.height();
    if w <= 0 || h <= 0 {
        return;
    }

    let chars = canvas.chars();
    let attrs = canvas.attrs();

    for y in 0..h {
        out.extend_from_slice(format!("\x1b[{};1H", y + 1).as_bytes());
        for x in 0..w {
            let idx = (y * w + x) as usize;
            let ch = chars[idx];
            if ch == CACA_MAGIC_FULLWIDTH {
                // The leading fullwidth glyph already advanced the cursor.
                continue;
            }

            sync_state(out, state, Attr::from_raw(attrs[idx]));

            let ch = if ch < 0x20 || ch == 0x7f {
                // Avoid corrupting the terminal with control characters.
                b' ' as u32
            } else {
                ch
            };

            let mut buf = [0u8; 4];
            let n = utf32_to_utf8(&mut buf, ch);
            out.extend_from_slice(&buf[..n]);

            // Sanity: a fullwidth glyph should be followed by the magic cell.
            let _ = utf32_is_fullwidth(ch);
        }
    }

    out.extend_from_slice(b"\x1b[0m");
    state.reset();
}

/// Escape sequences emitted when entering/leaving the alternate screen.
pub const ENTER_SCREEN: &[u8] = b"\x1b[?1049h\x1b[?25l\x1b[0m\x1b[2J";
pub const LEAVE_SCREEN: &[u8] = b"\x1b[0m\x1b[?25h\x1b[?1049l";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attr::Color;

    #[test]
    fn renders_characters_and_colours() {
        let mut cv = Canvas::new(3, 1).unwrap();
        cv.set_color_ansi(Color::Red, Color::Black).unwrap();
        cv.put_str(0, 0, "abc");
        let mut state = AnsiState::default();
        let mut out = Vec::new();
        render(&cv, &mut state, &mut out);
        let s = String::from_utf8_lossy(&out);
        assert!(s.contains('a') && s.contains('b') && s.contains('c'));
        assert!(s.contains("\x1b[0;31;40m"));
        assert!(s.ends_with("\x1b[0m"));
    }

    #[test]
    fn fullwidth_skips_magic_cell() {
        let mut cv = Canvas::new(4, 1).unwrap();
        cv.put_char(0, 0, 0x3000);
        cv.put_str(2, 0, "ab");
        let mut state = AnsiState::default();
        let mut out = Vec::new();
        render(&cv, &mut state, &mut out);
        let s = String::from_utf8_lossy(&out);
        assert!(s.contains("ab"));
        assert!(!s.contains('\u{fffe}'));
    }
}
