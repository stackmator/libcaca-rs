//! Port of libcaca's `examples/spritedit.c`.
//!
//! Builds a 4-frame sprite, round-trips it through the native export format,
//! then prints each frame as UTF-8.

use std::io::Write;

use libcaca::Canvas;

const GUY: &[&str] = &[
    "  O_,= \n  |    \n  /\\   \n / /   \n",
    "  O_,= \n  |    \n  /|   \n / |   \n",
    "  O_,= \n  |    \n  |\\   \n  |/   \n",
    "  O_,= \n  |    \n  |\\   \n  | \\  \n",
];

fn main() -> libcaca::Result<()> {
    let mut sprite = Canvas::new(0, 0)?;
    for _ in 0..3 {
        sprite.create_frame(0)?;
    }

    for (i, frame) in GUY.iter().enumerate() {
        sprite.set_frame(i)?;
        sprite.import_from_memory(frame.as_bytes(), "utf8")?;
    }

    let buffer = sprite.export_to_memory("caca")?;

    let mut sprite = Canvas::new(0, 0)?;
    sprite.import_from_memory(&buffer, "caca")?;

    let mut out = std::io::stdout();
    for i in 0..4 {
        sprite.set_frame(i)?;
        println!("Frame #{}", i);
        let data = sprite.export_to_memory("utf8")?;
        let _ = out.write_all(&data);
    }
    let _ = out.flush();

    Ok(())
}
