//! Port of libcaca's `examples/figfont.c`.
//!
//! Usage: `figfont <font.flf> <text>` — renders the text with a FIGlet/TOIlet
//! font and prints the UTF-8 export to stdout.

use std::io::Write;
use std::path::Path;

use libcaca::{Canvas, Color, FigFont};

fn main() -> libcaca::Result<()> {
    let argv: Vec<String> = std::env::args().collect();
    if argv.len() < 3 {
        eprintln!("Too few arguments");
        std::process::exit(-1);
    }

    let mut cv = Canvas::new(0, 0)?;
    let mut ff = FigFont::load(Path::new(&argv[1])).map_err(|_| {
        eprintln!("Could not open font");
        libcaca::CacaError::Invalid
    })?;

    let mut color: u8 = 0;
    for ch in argv[2].chars() {
        color = color.wrapping_add(4);
        cv.set_color_ansi(
            Color::from_u8(1 + (color % 15)).unwrap_or(Color::White),
            Color::Transparent,
        )?;
        ff.put_char(&mut cv, ch as u32)?;
    }
    ff.flush(&mut cv)?;

    let data = cv.export_to_memory("utf8")?;
    let mut out = std::io::stdout();
    let _ = out.write_all(&data);
    let _ = out.flush();

    Ok(())
}
