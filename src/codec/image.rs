//! PostScript and SVG exports.

use crate::attr::Attr;
use crate::canvas::{Canvas, CACA_MAGIC_FULLWIDTH};
use crate::error::{CacaError, Result};

use super::push_utf8;

/// Export the canvas as an uncompressed 32-bit TGA image, rasterised with the
/// first built-in bitmap font (like the C `export_tga`).
pub(super) fn export_tga(cv: &Canvas) -> Result<Vec<u8>> {
    let names = crate::font::font_list();
    let name = names.first().ok_or(CacaError::Invalid)?;
    let font = crate::font::load_builtin(name)?;

    let w = cv.width() * font.width();
    let h = cv.height() * font.height();
    let pix_len = (w.max(0) * h.max(0) * 4) as usize;

    let mut out = Vec::with_capacity(18 + pix_len);

    // TGA header: 18 bytes.
    out.push(0); // ID length
    out.push(0); // colour map type
    out.push(2); // image type: uncompressed truecolour
    out.extend_from_slice(&[0u8; 5]); // colour map specification
    out.extend_from_slice(&[0, 0]); // X origin
    out.extend_from_slice(&[0, 0]); // Y origin
    out.push((w & 0xff) as u8);
    out.push(((w >> 8) & 0xff) as u8);
    out.push((h & 0xff) as u8);
    out.push(((h >> 8) & 0xff) as u8);
    out.push(32); // pixel depth
    out.push(40); // image descriptor

    let mut pixels = vec![0u8; pix_len];
    if w > 0 && h > 0 {
        font.render_canvas(cv, &mut pixels, w, h, 4 * w)?;
    }

    // render_canvas writes ARGB; TGA expects BGRA.
    for px in pixels.chunks_exact_mut(4) {
        px.swap(0, 3);
        px.swap(1, 2);
    }

    out.extend_from_slice(&pixels);
    Ok(out)
}

pub(super) fn export_ps(cv: &Canvas) -> Vec<u8> {
    const PS_HEADER: &str = "%!\n\
%% libcaca PDF export\n\
%%LanguageLevel: 2\n\
%%Pages: 1\n\
%%DocumentData: Clean7Bit\n\
/csquare {\n\
  newpath\n\
  0 0 moveto\n\
  0 1 rlineto\n\
  1 0 rlineto\n\
  0 -1 rlineto\n\
  closepath\n\
  setrgbcolor\n\
  fill\n\
} def\n\
/S {\n\
  Show\n\
} bind def\n\
/Courier-Bold findfont\n\
8 scalefont\n\
setfont\n\
gsave\n\
6 10 scale\n";

    let frame = cv.active();
    let width = frame.width;
    let height = frame.height;
    let stride = width;

    let mut out = String::new();
    out.push_str(PS_HEADER);
    out.push_str(&format!("0 {} translate\n", height));

    for y in (0..height).rev() {
        for x in 0..width {
            let argb = Attr::from_raw(frame.attrs[(x + y * stride) as usize]).to_argb64();
            out.push_str(&format!(
                "1 0 translate\n {} {} {} csquare\n",
                argb[1] as f64 / 15.0,
                argb[2] as f64 / 15.0,
                argb[3] as f64 / 15.0
            ));
        }

        out.push_str(&format!("-{} 1 translate\n", width));
    }

    out.push_str("grestore\n");
    out.push_str(&format!("0 {} translate\n", height * 10));

    for y in (0..height).rev() {
        let row = height - y - 1;

        for x in 0..width {
            let idx = (x + row * stride) as usize;
            let argb = Attr::from_raw(frame.attrs[idx]).to_argb64();
            let ch = frame.chars[idx];

            out.push_str("newpath\n");
            out.push_str(&format!("{} {} moveto\n", (x + 1) * 6, y * 10 + 2));
            out.push_str(&format!(
                "{} {} {} setrgbcolor\n",
                argb[5] as f64 / 15.0,
                argb[6] as f64 / 15.0,
                argb[7] as f64 / 15.0
            ));

            if !(0x20..0x80).contains(&ch) {
                out.push_str("(?) show\n");
            } else {
                let c = ch as u8 as char;
                if c == '\\' || c == '(' || c == ')' {
                    out.push_str(&format!("(\\{}) show\n", c));
                } else {
                    out.push_str(&format!("({}) show\n", c));
                }
            }
        }
    }

    out.push_str("showpage\n");

    out.into_bytes()
}

pub(super) fn export_svg(cv: &Canvas) -> Vec<u8> {
    let frame = cv.active();
    let width = frame.width;
    let height = frame.height;
    let stride = width;

    let mut out = String::new();

    out.push_str(&format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<svg width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\"\
 xmlns=\"http://www.w3.org/2000/svg\"\
 xmlns:xlink=\"http://www.w3.org/1999/xlink\"\
 xml:space=\"preserve\" version=\"1.1\"  baseProfile=\"full\">\n",
        w = width * 6,
        h = height * 10
    ));

    out.push_str(
        " <g id=\"mainlayer\" font-size=\"10\" style=\"font-family: monospace\">\n",
    );

    for y in 0..height {
        for x in 0..width {
            let attr = Attr::from_raw(frame.attrs[(x + y * stride) as usize]);
            out.push_str(&format!(
                "<rect style=\"fill:#{:03x}\" x=\"{}\" y=\"{}\" width=\"6\" height=\"10\"/>\n",
                attr.to_rgb12_bg(),
                x * 6,
                y * 10
            ));
        }
    }

    for y in 0..height {
        for x in 0..width {
            let idx = (x + y * stride) as usize;
            let attr = Attr::from_raw(frame.attrs[idx]);
            let ch = frame.chars[idx];

            if ch == b' ' as u32 || ch == CACA_MAGIC_FULLWIDTH {
                continue;
            }

            let bold = if attr.raw() & 0x1 != 0 {
                " font-weight=\"bold\""
            } else {
                ""
            };
            let italic = if attr.raw() & 0x2 != 0 {
                " font-style=\"italic\""
            } else {
                ""
            };

            out.push_str(&format!(
                "<text style=\"fill:#{:03x}\"{}{} x=\"{}\" y=\"{}\">",
                attr.to_rgb12_fg(),
                bold,
                italic,
                x * 6,
                y * 10 + 8
            ));

            if ch < 0x20 {
                out.push('?');
            } else if ch > 0x7f {
                let mut buf = Vec::new();
                push_utf8(&mut buf, ch);
                out.push_str(&String::from_utf8_lossy(&buf));
            } else {
                match ch as u8 {
                    b'>' => out.push_str("&gt;"),
                    b'<' => out.push_str("&lt;"),
                    b'&' => out.push_str("&amp;"),
                    c => out.push(c as char),
                }
            }

            out.push_str("</text>\n");
        }
    }

    out.push_str(" </g>\n</svg>\n");

    out.into_bytes()
}
