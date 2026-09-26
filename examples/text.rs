//! Port of libcaca's `examples/text.c`.
//!
//! Imports ASCII art, shows it mirrored four ways, colourises it, then prints
//! the UTF-8 export — twice, with a 90-degree rotation in between.

use std::io::Write;

use libcaca::{Canvas, Color};

const STRING: &str = "              |_| \n   _,----._   | | \n  (/ @  @ \\)   __ \n   |  OO  |   |_  \n   \\ `--' /   |__ \n    `----'        \n              |_| \n Hello world!  |  \n                  \n";

fn main() -> libcaca::Result<()> {
    let mut pig = Canvas::new(0, 0)?;
    pig.import_from_memory(STRING.as_bytes(), "text")?;

    let (pw, ph) = (pig.width(), pig.height());
    let mut cv = Canvas::new(pw * 2, ph * 2)?;

    cv.blit(0, 0, &pig, None)?;
    pig.flip();
    cv.blit(pw, 0, &pig, None)?;
    pig.flip();
    pig.flop();
    cv.blit(0, ph, &pig, None)?;
    pig.flop();
    pig.rotate_180();
    cv.blit(pw, ph, &pig, None)?;

    for j in 0..cv.height() {
        for i in (0..cv.width()).step_by(2) {
            cv.set_color_ansi(
                Color::from_u8((9 + (i + j) % 6) as u8).unwrap(),
                Color::Default,
            )?;
            let a = cv.get_attr(-1, -1);
            cv.put_attr(i, j, a);
            cv.put_attr(i + 1, j, a);
        }
    }

    let mut out = std::io::stdout();
    let data = cv.export_to_memory("utf8")?;
    let _ = out.write_all(&data);

    cv.rotate_left()?;
    let data = cv.export_to_memory("utf8")?;
    let _ = out.write_all(&data);
    let _ = out.flush();

    Ok(())
}
