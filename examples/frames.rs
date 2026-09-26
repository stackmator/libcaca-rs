//! Port of libcaca's `examples/frames.c`.
//!
//! Creates a 200-frame canvas, paints the first 16 frames, then animates
//! them until a key is pressed. Headless runs play a fixed number of frames.

use libcaca::{Canvas, Color, Display, Driver, EventMask};

fn main() -> libcaca::Result<()> {
    let mut cv = Canvas::new(0, 0)?;

    for frame in 1..200 {
        cv.create_frame(frame)?;
    }

    eprintln!("canvas created, size is {}x{}", cv.width(), cv.height());

    cv.set_size(150, 80)?;
    eprintln!("canvas expanded, size is {}x{}", cv.width(), cv.height());

    for frame in 0..16i32 {
        cv.set_frame(frame as usize)?;
        cv.set_color_ansi(
            Color::White,
            Color::from_u8(frame as u8).unwrap_or(Color::Black),
        )?;
        cv.fill_box(0, 0, 40, 15, b':' as u32);
        cv.set_color_ansi(Color::White, Color::Blue)?;
        cv.put_str(frame * 5 / 2, frame, "カカ");
        cv.set_color_ansi(Color::Default, Color::Transparent)?;
    }

    cv.set_size(41, 16)?;
    eprintln!("canvas shrinked, size is {}x{}", cv.width(), cv.height());

    let mut dp = Display::new(cv)?;
    dp.set_display_time(50_000)?;
    eprintln!(
        "display attached, size is {}x{}",
        dp.canvas().width(),
        dp.canvas().height()
    );

    let interactive = matches!(dp.driver(), Driver::Terminal | Driver::Win32);
    let mut n = 0u32;
    loop {
        if dp.get_event(EventMask::KEY_PRESS, 0).is_some() {
            break;
        }
        dp.canvas_mut().set_frame((n % 16) as usize)?;
        dp.refresh()?;
        n += 1;
        if !interactive && n >= 32 {
            break;
        }
    }

    Ok(())
}
