//! Port of libcaca's `examples/colors.c`.
//!
//! Displays all 16x16 libcaca colour pairs plus the style attributes.
//! Press any key to quit when running interactively.

use libcaca::{Attr, Canvas, Color, Display, Driver, EventMask, Style};

fn main() -> libcaca::Result<()> {
    let mut dp = Display::new(Canvas::new(80, 24)?)?;

    {
        let cv = dp.canvas_mut();
        cv.set_color_ansi(Color::LightGray, Color::Black)?;
        cv.clear();

        for i in 0..16i32 {
            cv.set_color_ansi(Color::LightGray, Color::Black)?;
            cv.printf(
                3,
                i + if i >= 8 { 3 } else { 2 },
                format_args!("ANSI {}", i),
            );
            for j in 0..16i32 {
                cv.set_color_ansi(
                    Color::from_u8(i as u8).unwrap(),
                    Color::from_u8(j as u8).unwrap(),
                )?;
                cv.put_str(
                    (if j >= 8 { 13 } else { 12 }) + j * 4,
                    i + if i >= 8 { 3 } else { 2 },
                    "Aaホ",
                );
            }
        }

        cv.set_color_ansi(Color::LightGray, Color::Black)?;
        cv.put_str(
            3,
            20,
            "This is bold    This is blink    This is italics    This is underline",
        );
        cv.set_attr(Attr::from_raw(Style::BOLD.bits() as u32));
        cv.put_str(3 + 8, 20, "bold");
        cv.set_attr(Attr::from_raw(Style::BLINK.bits() as u32));
        cv.put_str(3 + 24, 20, "blink");
        cv.set_attr(Attr::from_raw(Style::ITALICS.bits() as u32));
        cv.put_str(3 + 41, 20, "italics");
        cv.set_attr(Attr::from_raw(Style::UNDERLINE.bits() as u32));
        cv.put_str(3 + 60, 20, "underline");
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
