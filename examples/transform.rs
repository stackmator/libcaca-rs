//! Port of libcaca's `examples/transform.c`.
//!
//! Blits ASCII-art sprites onto a canvas and shows the `flip`, `flop` and
//! `rotate_180` transforms side by side. Press any key to quit when running
//! interactively.

use libcaca::{Canvas, Color, Display, Driver, EventMask};

const PIG: &str = ",--.   ,--.\n\
                   \\  /-~-\\  /\n\
                   \x20)' o O `(\n\
                   (  ,---.  )\n\
                   \x20`(_o_o_)'\n\
                   \x20  )`-'(\n";

const DUCK: &str = "                ,~~.\n\
                    \x20   __     ,   (  O )>\n\
                    ___( o)>   )`~~'   (\n\
                    \\ <_. )   (  .__)   )\n\
                    \x20`---'     `-.____,'\n";

fn main() -> libcaca::Result<()> {
    let mut dp = Display::new(Canvas::new(0, 0)?)?;

    let mut image = Canvas::new(70, 6)?;
    let mut tmp = Canvas::new(70, 6)?;
    let mut sprite = Canvas::new(0, 0)?;

    sprite.set_color_ansi(Color::LightMagenta, Color::Black)?;
    sprite.import_from_memory(PIG.as_bytes(), "text")?;
    image.blit(55, 0, &sprite, None)?;

    sprite.set_color_ansi(Color::LightGreen, Color::Black)?;
    sprite.import_from_memory(DUCK.as_bytes(), "text")?;
    image.blit(30, 1, &sprite, None)?;

    image.set_color_ansi(Color::LightCyan, Color::Black)?;
    image.put_str(1, 1, "hahaha mais vieux porc immonde !! [⽼ ⾗]");
    image.set_color_ansi(Color::LightRed, Color::Black)?;
    image.put_char(38, 1, b'|' as u32);

    image.set_color_ansi(Color::Yellow, Color::Black)?;
    image.put_str(4, 2, "\\o\\ \\o| _o/ \\o_ |o/ /o/");

    image.set_color_ansi(Color::White, Color::LightRed)?;
    image.put_str(7, 3, "▙▘▌▙▘▞▖▞▖▌ ▞▖▌ ▌▌");
    image.put_str(7, 4, "▛▖▌▛▖▚▘▚▘▚▖▚▘▚▖▖▖");
    image.set_color_ansi(Color::Black, Color::LightRed)?;
    image.put_str(4, 3, "▓▒░");
    image.put_str(4, 4, "▓▒░");
    image.put_str(24, 3, "░▒▓");
    image.put_str(24, 4, "░▒▓");

    {
        let cv = dp.canvas_mut();
        cv.set_color_ansi(Color::White, Color::Blue)?;
        cv.put_str(0, 0, "normal");
        cv.blit(10, 0, &image, None)?;

        cv.put_str(0, 6, "flip");
        tmp.blit(0, 0, &image, None)?;
        tmp.flip();
        cv.blit(10, 6, &tmp, None)?;

        cv.put_str(0, 12, "flop");
        tmp.blit(0, 0, &image, None)?;
        tmp.flop();
        cv.blit(10, 12, &tmp, None)?;

        cv.put_str(0, 18, "rotate");
        tmp.blit(0, 0, &image, None)?;
        tmp.rotate_180();
        cv.blit(10, 18, &tmp, None)?;
    }

    dp.refresh()?;

    // Wait for a key press when attached to a real display.
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
