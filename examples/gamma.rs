//! Port of libcaca's `examples/gamma.c`.
//!
//! Dithers grey/R/G/B gradients with and without gamma correction, masked by
//! a moving ellipse. Left/right arrows change the gamma, down resets it,
//! Escape quits. Headless runs render a fixed number of frames.

use libcaca::{key, Canvas, Color, Display, Dither, Driver, EventMask};

fn main() -> libcaca::Result<()> {
    let mut dp = Display::new(Canvas::new(0, 0)?)?;

    let (mut cw, mut mask) = {
        let cv = dp.canvas();
        (
            Canvas::new(cv.width(), cv.height())?,
            Canvas::new(cv.width(), cv.height())?,
        )
    };

    // The C demo keeps a 256x4 stripe buffer; tile it to 256 rows so the
    // whole canvas is covered without ever reading out of bounds.
    let mut pixels = Vec::with_capacity(256 * 256 * 4);
    for row in 0..256u32 {
        for x in 0..256u32 {
            let p = match row % 4 {
                0 => (x << 16) | (x << 8) | x,
                1 => (0xff << 16) | (x << 8),
                2 => (0xff << 8) | x,
                _ => (x << 16) | 0xff,
            };
            pixels.extend_from_slice(&p.to_le_bytes());
        }
    }

    let left = Dither::new(256, 256, 32, 4 * 256, 0x00ff0000, 0x0000ff00, 0x000000ff, 0)?;
    let mut right = Dither::new(256, 256, 32, 4 * 256, 0x00ff0000, 0x0000ff00, 0x000000ff, 0)?;
    let mut gam = 1.0f32;
    dp.set_display_time(20_000)?;

    let interactive = matches!(dp.driver(), Driver::Terminal | Driver::Win32);
    let mut x = 0u32;

    loop {
        if let Some(ev) = dp.get_event(EventMask::KEY_PRESS, 0) {
            if let Some(k) = ev.key() {
                match k.ch {
                    c if c == key::LEFT => gam /= 1.03,
                    c if c == key::RIGHT => gam *= 1.03,
                    c if c == key::DOWN => gam = 1.0,
                    c if c == key::ESCAPE => break,
                    _ => {}
                }
            }
        }

        {
            let cv = dp.canvas_mut();
            let (w, h) = (cv.width().min(256), cv.height().min(256));
            cw.set_size(cv.width(), cv.height())?;
            mask.set_size(cv.width(), cv.height())?;

            left.dither_bitmap(cv, 0, 0, w, h, &pixels)?;

            right.set_gamma(gam)?;
            right.dither_bitmap(&mut cw, 0, 0, w, h, &pixels)?;

            mask.set_color_ansi(Color::LightGray, Color::Black)?;
            mask.clear();
            mask.set_color_ansi(Color::White, Color::White)?;
            mask.fill_ellipse(
                ((1.0 + (0.05 * x as f32).sin()) * 0.5 * mask.width() as f32) as i32,
                ((1.0 + (0.05 * x as f32).cos()) * 0.5 * mask.height() as f32) as i32,
                mask.width() / 2,
                mask.height() / 2,
                b'#' as u32,
            );

            cv.blit(0, 0, &cw, Some(&mask))?;

            cv.set_color_ansi(Color::White, Color::Blue)?;
            cv.printf(
                2,
                1,
                format_args!("gamma={} - use arrows to change, Esc to quit", gam),
            );
        }

        dp.refresh()?;

        x += 1;
        if !interactive && x >= 30 {
            break;
        }
    }

    Ok(())
}
