//! Text, ANSI and UTF-8 codecs.

use alloc::{format, vec::Vec};

use crate::attr::{Attr, Color};
use crate::canvas::{Canvas, CACA_MAGIC_FULLWIDTH};
use crate::charset::{cp437_to_utf32, utf32_is_fullwidth, utf32_to_cp437, utf8_to_utf32};
use crate::error::Result;

use super::push_utf8;

struct Import {
    clearattr: u32,
    fg: u8,
    bg: u8,
    dfg: u8,
    dbg: u8,
    bold: bool,
    blink: bool,
    italics: bool,
    negative: bool,
    concealed: bool,
    underline: bool,
    faint: bool,
    strike: bool,
    proportional: bool,
}

const ANSI2CACA: [u8; 8] = [0x00, 0x04, 0x02, 0x06, 0x01, 0x05, 0x03, 0x07];

pub(super) fn import_text(cv: &mut Canvas, data: &[u8]) -> Result<usize> {
    let mut width = 0i32;
    let mut height = 0i32;
    let mut x = 0i32;
    let mut y = 0i32;

    cv.set_size(0, 0)?;

    for &ch in data {
        if ch == b'\r' {
            continue;
        }

        if ch == b'\n' {
            x = 0;
            y += 1;
            continue;
        }

        if x >= width || y >= height {
            if x >= width {
                width = x + 1;
            }

            if y >= height {
                height = y + 1;
            }

            cv.set_size(width, height)?;
        }

        cv.put_char(x, y, ch as u32);
        x += 1;
    }

    if y > height {
        height = y;
        cv.set_size(width, height)?;
    }

    Ok(data.len())
}

pub(super) fn import_ansi(cv: &mut Canvas, data: &[u8], utf8: bool) -> Result<usize> {
    let size = data.len();

    let mut im = Import {
        clearattr: 0,
        fg: 0,
        bg: 0,
        dfg: 0,
        dbg: 0,
        bold: false,
        blink: false,
        italics: false,
        negative: false,
        concealed: false,
        underline: false,
        faint: false,
        strike: false,
        proportional: false,
    };

    let growx;
    let growy;
    let mut width: i32;
    let mut height: i32;
    let mut x: i32;
    let mut y: i32;
    let mut save_x = 0i32;
    let mut save_y = 0i32;

    if utf8 {
        width = cv.width();
        height = cv.height();
        growx = width == 0;
        growy = height == 0;
        x = cv.active().x;
        y = cv.active().y;
        im.dfg = Color::Default.as_u8();
        im.dbg = Color::Transparent.as_u8();
    } else {
        cv.set_size(80, 0)?;
        width = 80;
        height = 0;
        growx = false;
        growy = true;
        x = 0;
        y = 0;
        im.dfg = Color::LightGray.as_u8();
        im.dbg = Color::Black.as_u8();
    }

    cv.set_color_ansi(
        Color::from_u8(im.dfg).unwrap(),
        Color::from_u8(im.dbg).unwrap(),
    )?;
    im.clearattr = cv.get_attr(-1, -1).raw();
    ansi_parse_grcm(cv, &mut im, &[0])?;

    let mut i = 0usize;

    while i < size {
        let mut ch: u32 = 0;
        let mut wch: i32 = 0;
        let mut skip = 1usize;
        let b = data[i];

        if !utf8 && b == 0x1a && i + 7 < size && &data[i + 1..i + 8] == b"SAUCE00" {
            break;
        } else if b == b'\r' {
            x = 0;
        } else if b == b'\n' {
            x = 0;
            y += 1;
        } else if b == b'\t' {
            x = (x + 8) & !7;
        } else if b == 0x08 {
            if x > 0 {
                x -= 1;
            }
        } else if b == 0x1b && i + 2 >= size {
            break;
        } else if b == 0x1b && data[i + 1] == b'(' && data[i + 2] == b'B' {
            skip += 2;
        } else if b == 0x1b && data[i + 1] == b'[' {
            let mut argv = [0u32; 101];
            let mut argc = 0usize;
            let param = 2usize;

            let mut inter = param;
            while i + inter < size && (0x30..=0x3f).contains(&data[i + inter]) {
                inter += 1;
            }

            let mut fin = inter;
            while i + fin < size && (0x20..=0x2f).contains(&data[i + fin]) {
                fin += 1;
            }

            if i + fin >= size || data[i + fin] < 0x40 || data[i + fin] > 0x7e {
                break;
            }

            skip += fin;

            if param < inter && data[i + param] >= 0x3c {
                i += skip;
                continue;
            }

            if fin - param > 100 {
                i += skip;
                continue;
            }

            if param < inter {
                let mut j = param;
                while j < inter {
                    if data[i + j] == b';' {
                        argc += 1;
                        if argc < argv.len() {
                            argv[argc] = 0;
                        }
                    } else if data[i + j].is_ascii_digit() && argc < argv.len() {
                        argv[argc] = 10 * argv[argc] + (data[i + j] - b'0') as u32;
                    }
                    j += 1;
                }
                argc += 1;
            }

            match data[i + fin] {
                b'H' => {
                    x = if argc > 1 && argv[1] > 0 {
                        argv[1] as i32 - 1
                    } else {
                        0
                    };
                    y = if argc > 0 && argv[0] > 0 {
                        argv[0] as i32 - 1
                    } else {
                        0
                    };
                }
                b'A' => {
                    y -= if argc > 0 { argv[0] as i32 } else { 1 };
                    if y < 0 {
                        y = 0;
                    }
                }
                b'B' => {
                    y += if argc > 0 { argv[0] as i32 } else { 1 };
                }
                b'C' => {
                    x += if argc > 0 { argv[0] as i32 } else { 1 };
                }
                b'D' => {
                    x -= if argc > 0 { argv[0] as i32 } else { 1 };
                    if x < 0 {
                        x = 0;
                    }
                }
                b'G' => {
                    x = if argc > 0 && argv[0] > 0 {
                        argv[0] as i32 - 1
                    } else {
                        0
                    };
                }
                b'J' => {
                    let saved = cv.get_attr(-1, -1).raw();
                    cv.set_attr(Attr::from_raw(im.clearattr));

                    if argc == 0 || argv[0] == 0 {
                        cv.draw_line(x, y, width, y, b' ' as u32);
                        cv.fill_box(0, y + 1, width - 1, height - 1, b' ' as u32);
                    } else if argv[0] == 1 {
                        cv.fill_box(0, 0, width - 1, y - 1, b' ' as u32);
                        cv.draw_line(0, y, x, y, b' ' as u32);
                    } else if argv[0] == 2 {
                        cv.fill_box(0, 0, width - 1, height - 1, b' ' as u32);
                    }

                    cv.set_attr(Attr::from_raw(saved));
                }
                b'K' => {
                    if argc == 0 || argv[0] == 0 {
                        cv.draw_line(x, y, width, y, b' ' as u32);
                    } else if argv[0] == 1 {
                        cv.draw_line(0, y, x, y, b' ' as u32);
                    } else if argv[0] == 2 && x < width {
                        cv.draw_line(x, y, width - 1, y, b' ' as u32);
                    }
                }
                b'P' => {
                    let ap = if argc == 0 || argv[0] == 0 {
                        1
                    } else {
                        argv[0] as i32
                    };
                    let mut j = 0i32;

                    while j + ap < width {
                        let c = cv.get_char(j + ap, y);
                        let a = cv.get_attr(j + ap, y);
                        cv.put_char(j, y, c);
                        cv.put_attr(j, y, a);
                        j += 1;
                    }
                }
                b'X' => {
                    if argc > 0 && argv[0] != 0 {
                        let saved = cv.get_attr(-1, -1).raw();
                        cv.set_attr(Attr::from_raw(im.clearattr));
                        cv.draw_line(x, y, x + argv[0] as i32 - 1, y, b' ' as u32);
                        cv.set_attr(Attr::from_raw(saved));
                    }
                }
                b'd' => {
                    y = if argc > 0 && argv[0] > 0 {
                        argv[0] as i32 - 1
                    } else {
                        0
                    };
                }
                b'f' => {
                    x = if argc > 1 && argv[1] > 0 {
                        argv[1] as i32 - 1
                    } else {
                        0
                    };
                    y = if argc > 0 && argv[0] > 0 {
                        argv[0] as i32 - 1
                    } else {
                        0
                    };
                }
                b'h' | b'l' => {}
                b'm' => {
                    if argc > 0 {
                        ansi_parse_grcm(cv, &mut im, &argv[..argc])?;
                    } else {
                        ansi_parse_grcm(cv, &mut im, &[0])?;
                    }
                }
                b's' => {
                    save_x = x;
                    save_y = y;
                }
                b'u' => {
                    x = save_x;
                    y = save_y;
                }
                _ => {}
            }
        } else if b == 0x1b && data[i + 1] == b']' {
            let mut command = 0u32;
            let mut semicolon = 2usize;

            while i + semicolon < size && data[i + semicolon].is_ascii_digit() {
                command = 10 * command + (data[i + semicolon] - b'0') as u32;
                semicolon += 1;
            }

            if i + semicolon >= size || data[i + semicolon] != b';' {
                break;
            }

            let mut fin = semicolon + 1;
            while i + fin < size && data[i + fin] >= 0x20 {
                fin += 1;
            }

            if i + fin >= size || data[i + fin] != 0x07 {
                break;
            }

            let _ = command;
            skip += fin;
        } else if i + 1 < size && b == 0x0c && data[i + 1] == b'\n' {
            let f = cv.frame_count() as i32;
            cv.create_frame(f)?;
            cv.set_frame(f as usize)?;
            x = 0;
            y = 0;
            skip += 1;
        } else if utf8 {
            let bytes: usize;

            if i + 6 < size {
                let (c, n) = utf8_to_utf32(&data[i..]);
                ch = c;
                bytes = n;
            } else {
                let mut tmp = [0u8; 7];
                let n = size - i;
                tmp[..n].copy_from_slice(&data[i..i + n]);
                let (c, bcount) = utf8_to_utf32(&tmp[..=n]);
                ch = c;
                bytes = bcount;
            }

            let bytes = if bytes == 0 {
                ch = data[i] as u32;
                1
            } else {
                bytes
            };

            wch = if utf32_is_fullwidth(ch) { 2 } else { 1 };
            skip += bytes - 1;
        } else {
            ch = cp437_to_utf32(b);
            wch = 1;
        }

        while x + wch > width {
            if growx {
                let saved = cv.get_attr(-1, -1).raw();
                cv.set_attr(Attr::from_raw(im.clearattr));
                width = x + wch;
                cv.set_size(width, height)?;
                cv.set_attr(Attr::from_raw(saved));
            } else {
                x -= width;
                y += 1;
            }
        }

        if y >= height {
            let saved = cv.get_attr(-1, -1).raw();
            cv.set_attr(Attr::from_raw(im.clearattr));

            if growy {
                height = y + 1;
                cv.set_size(width, height)?;
            } else {
                let lines = (y - height) + 1;
                let cw = cv.width() as usize;
                let rows = height - lines;

                if rows > 0 {
                    let frame = cv.active_mut();
                    for j in 0..rows as usize {
                        let src = (j + lines as usize) * cw;
                        let dst = j * cw;
                        for k in 0..cw {
                            frame.chars[dst + k] = frame.chars[src + k];
                            frame.attrs[dst + k] = frame.attrs[src + k];
                        }
                    }
                }

                cv.fill_box(0, height - lines, cv.width() - 1, height - 1, b' ' as u32);
                y -= lines;
            }

            cv.set_attr(Attr::from_raw(saved));
        }

        if wch != 0 {
            cv.put_char(x, y, ch);
            x += wch;
        }

        i += skip;
    }

    if growy && y > height {
        let saved = cv.get_attr(-1, -1).raw();
        cv.set_attr(Attr::from_raw(im.clearattr));
        height = y;
        cv.set_size(width, height)?;
        cv.set_attr(Attr::from_raw(saved));
    }

    cv.active_mut().x = x;
    cv.active_mut().y = y;

    Ok(i)
}

fn ansi_parse_grcm(cv: &mut Canvas, im: &mut Import, argv: &[u32]) -> Result<()> {
    for &a in argv {
        if (30..=37).contains(&a) {
            im.fg = ANSI2CACA[(a - 30) as usize];
        } else if (40..=47).contains(&a) {
            im.bg = ANSI2CACA[(a - 40) as usize];
        } else if (90..=97).contains(&a) {
            im.fg = ANSI2CACA[(a - 90) as usize] + 8;
        } else if (100..=107).contains(&a) {
            im.bg = ANSI2CACA[(a - 100) as usize] + 8;
        } else {
            match a {
                0 => {
                    im.fg = im.dfg;
                    im.bg = im.dbg;
                    im.bold = false;
                    im.blink = false;
                    im.italics = false;
                    im.negative = false;
                    im.concealed = false;
                    im.underline = false;
                    im.faint = false;
                    im.strike = false;
                    im.proportional = false;
                }
                1 => im.bold = true,
                2 => im.faint = true,
                3 => im.italics = true,
                4 | 21 => im.underline = true,
                5 | 6 => im.blink = true,
                7 => im.negative = true,
                8 => im.concealed = true,
                9 => im.strike = true,
                22 => {
                    im.bold = false;
                    im.faint = false;
                }
                23 => im.italics = false,
                24 => im.underline = false,
                25 => im.blink = false,
                26 => im.proportional = true,
                27 => im.negative = false,
                28 => im.concealed = false,
                29 => im.strike = false,
                38 | 48 => {}
                39 => im.fg = im.dfg,
                49 => im.bg = im.dbg,
                50 => im.proportional = false,
                _ => {}
            }
        }
    }

    let mut efg;
    let ebg;

    if im.concealed {
        efg = Color::Transparent.as_u8();
        ebg = Color::Transparent.as_u8();
    } else {
        efg = if im.negative { im.bg } else { im.fg };
        ebg = if im.negative { im.fg } else { im.bg };

        if im.bold {
            if efg < 8 {
                efg += 8;
            } else if efg == Color::Default.as_u8() {
                efg = Color::White.as_u8();
            }
        }
    }

    cv.set_color_ansi(Color::from_u8(efg).unwrap(), Color::from_u8(ebg).unwrap())
}

pub(super) fn export_text(cv: &Canvas) -> Vec<u8> {
    let frame = cv.active();
    let mut out = Vec::new();

    for y in 0..frame.height {
        for x in 0..frame.width {
            let ch = frame.chars[(x + y * frame.width) as usize];

            if ch == CACA_MAGIC_FULLWIDTH {
                continue;
            }

            if ch < 0x20 {
                out.push(b' ');
            } else {
                push_utf8(&mut out, ch);
            }
        }

        out.push(b'\n');
    }

    out
}

pub(super) fn export_utf8(cv: &Canvas, cr: bool) -> Vec<u8> {
    const PALETTE: [u8; 16] = [0, 4, 2, 6, 1, 5, 3, 7, 8, 12, 10, 14, 9, 13, 11, 15];

    let frame = cv.active();
    let mut out = Vec::new();

    for y in 0..frame.height {
        let mut prevfg = 0x10u8;
        let mut prevbg = 0x10u8;

        for x in 0..frame.width {
            let idx = (x + y * frame.width) as usize;
            let attr = Attr::from_raw(frame.attrs[idx]);
            let ch = frame.chars[idx];

            if ch == CACA_MAGIC_FULLWIDTH {
                continue;
            }

            let ansifg = attr.to_ansi_fg();
            let ansibg = attr.to_ansi_bg();
            let fg = if ansifg < 0x10 {
                PALETTE[ansifg as usize]
            } else {
                0x10
            };
            let bg = if ansibg < 0x10 {
                PALETTE[ansibg as usize]
            } else {
                0x10
            };

            if fg != prevfg || bg != prevbg {
                out.extend_from_slice(b"\x1b[0");

                if fg < 8 {
                    out.extend_from_slice(format!(";3{}", fg).as_bytes());
                } else if fg < 16 {
                    out.extend_from_slice(format!(";1;3{};9{}", fg - 8, fg - 8).as_bytes());
                }

                if bg < 8 {
                    out.extend_from_slice(format!(";4{}", bg).as_bytes());
                } else if bg < 16 {
                    out.extend_from_slice(format!(";5;4{};10{}", bg - 8, bg - 8).as_bytes());
                }

                out.push(b'm');
            }

            push_utf8(&mut out, ch);

            prevfg = fg;
            prevbg = bg;
        }

        if prevfg != 0x10 || prevbg != 0x10 {
            out.extend_from_slice(b"\x1b[0m");
        }

        out.extend_from_slice(if cr { b"\r\n" } else { b"\n" });
    }

    out
}

pub(super) fn export_ansi(cv: &Canvas) -> Vec<u8> {
    const PALETTE: [u8; 16] = [0, 4, 2, 6, 1, 5, 3, 7, 8, 12, 10, 14, 9, 13, 11, 15];

    let frame = cv.active();
    let mut out = Vec::new();
    let mut prevfg: i32 = -1;
    let mut prevbg: i32 = -1;

    for y in 0..frame.height {
        for x in 0..frame.width {
            let idx = (x + y * frame.width) as usize;
            let attr = Attr::from_raw(frame.attrs[idx]);
            let ansifg = attr.to_ansi_fg();
            let ansibg = attr.to_ansi_bg();
            let fg = if ansifg < 0x10 {
                PALETTE[ansifg as usize] as i32
            } else {
                Color::LightGray.as_u8() as i32
            };
            let bg = if ansibg < 0x10 {
                PALETTE[ansibg as usize] as i32
            } else {
                Color::Black.as_u8() as i32
            };
            let mut ch = frame.chars[idx];

            if ch == CACA_MAGIC_FULLWIDTH {
                ch = b'?' as u32;
            }

            if fg != prevfg || bg != prevbg {
                out.extend_from_slice(b"\x1b[0;");

                if fg < 8 {
                    if bg < 8 {
                        out.extend_from_slice(format!("3{};4{}m", fg, bg).as_bytes());
                    } else {
                        out.extend_from_slice(format!("5;3{};4{}m", fg, bg - 8).as_bytes());
                    }
                } else if bg < 8 {
                    out.extend_from_slice(format!("1;3{};4{}m", fg - 8, bg).as_bytes());
                } else {
                    out.extend_from_slice(format!("5;1;3{};4{}m", fg - 8, bg - 8).as_bytes());
                }
            }

            out.push(utf32_to_cp437(ch));

            prevfg = fg;
            prevbg = bg;
        }

        if frame.width == 80 {
            out.extend_from_slice(b"\x1b[s\n\x1b[u");
        } else {
            out.extend_from_slice(b"\x1b[0m\r\n");
            prevfg = -1;
            prevbg = -1;
        }
    }

    out
}

pub(super) fn export_irc(cv: &Canvas) -> Vec<u8> {
    const PALETTE: [u8; 16] = [1, 2, 3, 10, 5, 6, 7, 15, 14, 12, 9, 11, 4, 13, 8, 0];

    let frame = cv.active();
    let mut out = Vec::new();

    for y in 0..frame.height {
        let mut prevfg = 0x10u8;
        let mut prevbg = 0x10u8;

        for x in 0..frame.width {
            let idx = (x + y * frame.width) as usize;
            let attr = Attr::from_raw(frame.attrs[idx]);
            let ch = frame.chars[idx];

            if ch == CACA_MAGIC_FULLWIDTH {
                continue;
            }

            let ansifg = attr.to_ansi_fg();
            let ansibg = attr.to_ansi_bg();
            let fg = if ansifg < 0x10 {
                PALETTE[ansifg as usize]
            } else {
                0x10
            };
            let bg = if ansibg < 0x10 {
                PALETTE[ansibg as usize]
            } else {
                0x10
            };

            if bg != prevbg || fg != prevfg {
                let mut need_escape = 0;

                if bg == 0x10 {
                    if fg == 0x10 {
                        out.push(0x0f);
                    } else {
                        if prevbg == 0x10 {
                            out.extend_from_slice(format!("\x03{}", fg).as_bytes());
                        } else {
                            out.extend_from_slice(format!("\x0f\x03{}", fg).as_bytes());
                        }

                        if ch == b',' as u32 {
                            need_escape = 1;
                        }
                    }
                } else if fg == 0x10 {
                    out.extend_from_slice(format!("\x0f\x03,{}", bg).as_bytes());
                } else {
                    out.extend_from_slice(format!("\x03{},{}", fg, bg).as_bytes());
                }

                if (b'0' as u32..=b'9' as u32).contains(&ch) {
                    need_escape = 1;
                }

                if need_escape != 0 {
                    out.extend_from_slice(b"\x02\x02");
                }
            }

            push_utf8(&mut out, ch);
            prevfg = fg;
            prevbg = bg;
        }

        if frame.width == 0 {
            out.push(b' ');
        }

        out.push(b'\r');
        out.push(b'\n');
    }

    out
}
