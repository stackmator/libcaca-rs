//! Native libcaca binary import/export and BIN import.

use crate::attr::{Attr, Color};
use crate::canvas::Canvas;
use crate::charset::cp437_to_utf32;
use crate::error::{CacaError, Result};

fn read_u32(buf: &[u8], off: usize) -> u32 {
    u32::from_be_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]])
}

fn read_u16(buf: &[u8], off: usize) -> u16 {
    u16::from_be_bytes([buf[off], buf[off + 1]])
}

pub(super) fn import_caca(cv: &mut Canvas, data: &[u8]) -> Result<usize> {
    let size = data.len();

    if size < 20 {
        return Ok(0);
    }

    if &data[0..4] != b"\xCA\xCACV" {
        return Err(CacaError::Invalid);
    }

    let control_size = read_u32(data, 4) as usize;
    let data_size = read_u32(data, 8) as usize;
    let _version = read_u16(data, 12);
    let frames = read_u32(data, 14) as usize;
    let _flags = read_u16(data, 18);

    if size < 4 + control_size + data_size {
        return Ok(0);
    }

    if control_size < 16 + frames * 32 {
        return Err(CacaError::Invalid);
    }

    let mut xmin: i32 = 0;
    let mut ymin: i32 = 0;
    let mut xmax: i32 = 0;
    let mut ymax: i32 = 0;
    let mut expected_size: u64 = 0;

    for f in 0..frames {
        let base = 4 + 16 + f * 32;
        let width = read_u32(data, base);
        let height = read_u32(data, base + 4);
        let handlex = read_u32(data, base + 24) as i32;
        let handley = read_u32(data, base + 28) as i32;

        expected_size += width as u64 * height as u64 * 8;

        if -handlex < xmin {
            xmin = -handlex;
        }
        if -handley < ymin {
            ymin = -handley;
        }
        if (width as i32) - handlex > xmax {
            xmax = (width as i32) - handlex;
        }
        if (height as i32) - handley > ymax {
            ymax = (height as i32) - handley;
        }
    }

    if expected_size != data_size as u64 {
        return Err(CacaError::Invalid);
    }

    cv.set_size(0, 0)?;
    cv.set_size(xmax - xmin, ymax - ymin)?;

    cv.frames.truncate(1);
    cv.frame = 0;

    let mut offset: usize = 0;

    for f in 0..frames {
        let base = 4 + 16 + f * 32;
        let width = read_u32(data, base);
        let height = read_u32(data, base + 4);
        let attr = read_u32(data, base + 12);
        let fx = read_u32(data, base + 16) as i32;
        let fy = read_u32(data, base + 20) as i32;
        let handlex = read_u32(data, base + 24) as i32;
        let handley = read_u32(data, base + 28) as i32;

        if f > 0 {
            cv.create_frame(f as i32)?;
        }
        cv.set_frame(f)?;

        {
            let frame = &mut cv.frames[f];
            frame.curattr = attr;
            frame.x = fx;
            frame.y = fy;
            frame.handlex = handlex;
            frame.handley = handley;
        }

        let n_cells = width as usize * height as usize;
        let cell_base = 4 + control_size + offset;

        for n in 0..n_cells {
            let x = (n % width as usize) as i32 - handlex - xmin;
            let y = (n / width as usize) as i32 - handley - ymin;
            let ch = read_u32(data, cell_base + 8 * n);
            let at = read_u32(data, cell_base + 8 * n + 4);

            cv.put_char(x, y, ch);
            cv.put_attr(x, y, Attr::from_raw(at));
        }

        offset += n_cells * 8;

        {
            let frame = &mut cv.frames[f];
            frame.x -= handlex;
            frame.y -= handley;
            frame.handlex = -xmin;
            frame.handley = -ymin;
        }
    }

    cv.set_frame(0)?;

    Ok(4 + control_size + data_size)
}

pub(super) fn export_caca(cv: &Canvas) -> Vec<u8> {
    let framecount = cv.frame_count();
    let width = cv.width();
    let height = cv.height();
    let cells = width as usize * height as usize;

    let mut out = Vec::with_capacity(20 + (32 + 8 * cells) * framecount);

    out.extend_from_slice(b"\xCA\xCACV");
    out.extend_from_slice(&(16u32 + 32 * framecount as u32).to_be_bytes());
    out.extend_from_slice(&((cells * 8 * framecount) as u32).to_be_bytes());
    out.extend_from_slice(&1u16.to_be_bytes());
    out.extend_from_slice(&(framecount as u32).to_be_bytes());
    out.extend_from_slice(&0u16.to_be_bytes());

    let curattr = cv.active().curattr;

    for f in 0..framecount {
        out.extend_from_slice(&(width as u32).to_be_bytes());
        out.extend_from_slice(&(height as u32).to_be_bytes());
        out.extend_from_slice(&0u32.to_be_bytes());
        out.extend_from_slice(&curattr.to_be_bytes());
        out.extend_from_slice(&(cv.frames[f].x as u32).to_be_bytes());
        out.extend_from_slice(&(cv.frames[f].y as u32).to_be_bytes());
        out.extend_from_slice(&(cv.frames[f].handlex as u32).to_be_bytes());
        out.extend_from_slice(&(cv.frames[f].handley as u32).to_be_bytes());
    }

    for f in 0..framecount {
        let chars = &cv.frames[f].chars;
        let attrs = &cv.frames[f].attrs;

        for n in 0..cells {
            out.extend_from_slice(&chars[n].to_be_bytes());
            out.extend_from_slice(&attrs[n].to_be_bytes());
        }
    }

    out
}

pub(super) fn import_bin(cv: &mut Canvas, data: &[u8]) -> Result<usize> {
    let mut len = data.len();
    let width = 160i32;

    cv.set_size(0, 0)?;
    cv.set_size(width, (len / width as usize) as i32)?;

    len &= !1usize;

    let mut x = 0i32;
    let mut y = 0i32;
    let mut i = 0usize;

    while i < len {
        let ch = data[i];
        let col = data[i + 1];

        cv.set_color_ansi(
            Color::from_u8(col & 0xf).unwrap(),
            Color::from_u8(col >> 4).unwrap(),
        )?;
        cv.put_char(x, y, cp437_to_utf32(ch));

        x += 1;
        if x >= width {
            x = 0;
            y += 1;
        }

        i += 2;
    }

    Ok(len)
}
