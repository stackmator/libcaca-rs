//! Port of libcaca's `examples/fullwidth.c`.
//!
//! Exercises fullwidth glyph handling across put, blit and overwrite paths.
//! Press any key to quit when running interactively.

use libcaca::{Canvas, Color, Display, Driver, EventMask};

const CACA: &str = "쫊쫊쫊쫊쫊쫊쫊쫊쫊쫊쫊쫊쫊쫊쫊";

fn main() -> libcaca::Result<()> {
    let mut dp = Display::new(Canvas::new(0, 0)?)?;

    let mut caca = Canvas::new(6, 10)?;
    let mut line = Canvas::new(2, 1)?;

    for i in 0..10i32 {
        caca.set_color_ansi(Color::White, Color::Blue)?;
        caca.put_str(0, i, CACA);
        caca.set_color_ansi(Color::White, Color::Red)?;
        caca.put_char(i - 2, i, b'x' as u32);
    }
    dp.canvas_mut().blit(1, 1, &caca, None)?;

    for i in 0..10i32 {
        caca.set_color_ansi(Color::White, Color::Blue)?;
        caca.put_str(0, i, CACA);
        caca.set_color_ansi(Color::White, Color::Green)?;
        caca.put_str(i - 2, i, "ホ");
    }
    dp.canvas_mut().blit(15, 1, &caca, None)?;

    line.set_color_ansi(Color::White, Color::Magenta)?;
    line.put_str(0, 0, "ほ");
    for i in 0..10i32 {
        caca.set_color_ansi(Color::White, Color::Blue)?;
        caca.put_str(0, i, CACA);
        caca.blit(i - 2, i, &line, None)?;
    }
    dp.canvas_mut().blit(29, 1, &caca, None)?;

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
