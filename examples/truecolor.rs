//! Port of libcaca's `examples/truecolor.c`.
//!
//! Shows a 16x16 grid of ARGB colour pairs. Press any key to quit when
//! running interactively.

use libcaca::{Canvas, Color, Display, Driver, EventMask};

fn main() -> libcaca::Result<()> {
    let mut dp = Display::new(Canvas::new(32, 16)?)?;

    {
        let cv = dp.canvas_mut();
        for y in 0..16i32 {
            for x in 0..16i32 {
                let bgcolor = 0xff00u16 | ((y as u16) << 4) | (x as u16);
                let fgcolor = 0xf000u16 | (((15 - y) as u16) << 4) | (((15 - x) as u16) << 8);
                cv.set_color_argb(fgcolor, bgcolor);
                cv.put_str(x * 2, y, "CA");
            }
        }

        cv.set_color_ansi(Color::White, Color::LightBlue)?;
        cv.put_str(2, 1, " truecolor libcaca ");
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
