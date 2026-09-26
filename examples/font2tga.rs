//! Port of libcaca's `examples/font2tga.c`.
//!
//! Renders every glyph of the first built-in font onto a canvas and exports
//! the result as a TGA image to stdout.

use std::io::Write;

use libcaca::charset::utf32_is_fullwidth;
use libcaca::font::{font_list, load_builtin};
use libcaca::{Canvas, Color};

fn main() -> libcaca::Result<()> {
    let names = font_list();
    let font = load_builtin(names[0])?;
    let blocks = font.blocks();

    let mut cells = 0u32;
    let mut i = 0usize;
    while i + 1 < blocks.len() && blocks[i + 1] != 0 {
        cells += blocks[i + 1] - blocks[i];
        let mut j = blocks[i];
        while j < blocks[i + 1] {
            if utf32_is_fullwidth(j) {
                cells += 1;
            }
            j += 1;
        }
        i += 2;
    }

    let mut width = 64u32;
    while width * width < cells {
        width *= 2;
    }

    let mut cv = Canvas::new(width as i32, ((cells + width - 1) / (width - 1)) as i32)?;
    cv.set_color_ansi(Color::Red, Color::Red)?;
    cv.clear();
    cv.set_color_ansi(Color::Black, Color::White)?;

    let (mut x, mut y) = (0i32, 0i32);
    let mut i = 0usize;
    while i + 1 < blocks.len() && blocks[i + 1] != 0 {
        let mut j = blocks[i];
        while j < blocks[i + 1] {
            cv.put_char(x, y, j);
            if utf32_is_fullwidth(j) {
                x += 1;
            }
            x += 1;
            if x >= width as i32 - 1 {
                x = 0;
                y += 1;
            }
            j += 1;
        }
        i += 2;
    }

    let data = cv.export_to_memory("tga")?;
    let mut out = std::io::stdout();
    let _ = out.write_all(&data);
    let _ = out.flush();

    Ok(())
}
