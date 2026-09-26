//! Port of libcaca's `examples/font.c`.
//!
//! Renders a small canvas to a 32-bit ARGB image with the first built-in
//! font, then dithers that image back onto an 80x32 canvas and shows it.
//! Press any key to quit when running interactively.

use libcaca::font::{font_list, load_builtin};
use libcaca::{Canvas, Color, Dither, Display, Driver, EventMask};

fn main() -> libcaca::Result<()> {
    let mut cv = Canvas::new(8, 2)?;

    cv.set_color_ansi(Color::White, Color::Black)?;
    cv.put_str(0, 0, "ABcde");
    cv.set_color_ansi(Color::LightRed, Color::Black)?;
    cv.put_str(5, 0, "\\o/");
    cv.set_color_ansi(Color::White, Color::Blue)?;
    cv.put_str(0, 1, "&$âøÿØ?!");

    let names = font_list();
    let font = load_builtin(names[0])?;

    let w = cv.width() * font.width();
    let h = cv.height() * font.height();
    let mut buf = vec![0u8; (w * h * 4) as usize];
    font.render_canvas(&cv, &mut buf, w, h, 4 * w)?;

    cv.set_size(80, 32)?;
    let mut dp = Display::new(cv)?;

    // The ARGB buffer is [A,R,G,B] in memory order; pick masks for the
    // native endianness like the C version does.
    let (rmask, gmask, bmask, amask) = if cfg!(target_endian = "little") {
        (0xff00u32, 0xff0000, 0xff000000, 0xff)
    } else {
        (0xff0000u32, 0xff00, 0xff, 0xff000000)
    };
    // NB: the C demo passes (32, w, h, ...) i.e. bpp=h, which this port
    // correctly rejects; the source image is w x h 32-bit.
    let dither = Dither::new(w, h, 32, 4 * w, rmask, gmask, bmask, amask)?;

    {
        let cv = dp.canvas_mut();
        // Dither exactly the rendered w x h region. (The C demo dithers the
        // whole 80x32 canvas, reading past the end of its source buffer; this
        // port clamps to the valid region instead.)
        let (cw, ch) = (w.min(cv.width()), h.min(cv.height()));
        dither.dither_bitmap(cv, 0, 0, cw, ch, &buf)?;
    }
    dp.refresh()?;

    if matches!(dp.driver(), Driver::Terminal | Driver::Win32) {
        loop {
            if dp
                .get_event(EventMask::KEY_PRESS | EventMask::QUIT, -1)
                .is_some()
            {
                break;
            }
        }
    }

    Ok(())
}
