//! Port of libcaca's `examples/blit.c`.
//!
//! Centres a sprite on the canvas using a canvas handle, then shows it.
//! Press any key to quit when running interactively.

use libcaca::{Canvas, Color, Display, Driver, EventMask};

const PIG: &str = "  ,__         __,\n   \\)`\\_..._/`(/\n   .'  _   _  '.\n  /    o\\ /o   \\\n  |    .-.-.    |  _\n  |   /() ()\\   | (,`)\n / \\  '-----'  / \\ .'\n|   '-..___..-'   |\n|                 |\n|                 |\n;                 ;\n \\      / \\      /\n  \\-..-/'-'\\-..-/\njgs\\/\\/     \\/\\/\n";

fn main() -> libcaca::Result<()> {
    let mut dp = Display::new(Canvas::new(80, 24)?)?;

    let mut sprite = Canvas::new(0, 0)?;
    sprite.set_color_ansi(Color::LightRed, Color::Black)?;
    sprite.import_from_memory(PIG.as_bytes(), "text")?;
    let (sw, sh) = (sprite.width(), sprite.height());
    sprite.set_handle(sw / 2, sh / 2);

    {
        let cv = dp.canvas_mut();
        cv.set_color_ansi(Color::White, Color::Blue)?;
        cv.put_str(0, 0, "Centered sprite");
        let (w, h) = (cv.width(), cv.height());
        cv.blit(w / 2, h / 2, &sprite, None)?;
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
