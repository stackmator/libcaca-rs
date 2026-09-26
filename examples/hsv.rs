//! Port of libcaca's `examples/hsv.c`.
//!
//! Dithers a 256x256 HSV gradient onto the display canvas. Press any key to
//! quit when running interactively.

use libcaca::{Display, Dither, Driver, EventMask};

fn main() -> libcaca::Result<()> {
    let mut dp = Display::create()?;
    let cv = dp.canvas_mut();

    let mut pixels = Vec::with_capacity(256 * 256 * 4);
    for y in 0..256u32 {
        for x in 0..256u32 {
            let v = y * x / 256;
            pixels.extend_from_slice(&((v << 16) | (v << 8) | x).to_le_bytes());
        }
    }

    // NB: the C demo passes bpp=256; the source is 256x256 32-bit.
    let dither = Dither::new(256, 256, 32, 4 * 256, 0x00ff0000, 0x0000ff00, 0x000000ff, 0)?;
    let (w, h) = (cv.width(), cv.height());
    dither.dither_bitmap(cv, 0, 0, w, h, &pixels)?;

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
