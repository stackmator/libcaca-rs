//! A small interactive demo of the terminal display driver.
//!
//! Run with `cargo run --example hello`. Press `q` or Escape to quit.
//! When stdout is not a terminal the program uses the `null` driver, so it is
//! also safe to run in CI.

use libcaca::{key, Canvas, Color, Display, Driver, EventMask};

fn main() -> libcaca::Result<()> {
    let canvas = Canvas::new(80, 24)?;
    let mut dp = Display::new(canvas)?;
    let _ = dp.set_title("libcaca-rs demo");

    // When there is no terminal (e.g. in CI) run a few frames and exit.
    let interactive = dp.driver() == Driver::Terminal;

    let mut frame = 0u32;
    loop {
        {
            let cv = dp.canvas_mut();
            let (w, h) = (cv.width(), cv.height());

            cv.set_color_ansi(Color::White, Color::Blue)?;
            cv.clear();

            cv.set_color_ansi(Color::LightCyan, Color::Blue)?;
            cv.draw_thin_box(1, 1, w - 2, h - 2);

            cv.set_color_ansi(Color::Yellow, Color::Blue)?;
            cv.put_str(3, 2, "libcaca-rs - a pure-Rust libcaca port");

            cv.set_color_ansi(Color::LightGray, Color::Blue)?;
            cv.put_str(3, 3, &format!("frame #{}", frame));

            cv.set_color_ansi(Color::LightMagenta, Color::Blue)?;
            cv.draw_circle(w / 2, h / 2, 5, b'#' as u32);

            cv.set_color_ansi(Color::LightGreen, Color::Blue)?;
            cv.put_str(3, h - 3, "Press q or Escape to quit");
        }

        dp.refresh()?;
        frame = frame.wrapping_add(1);

        if !interactive {
            if frame >= 30 {
                break;
            }
            continue;
        }

        if let Some(ev) = dp.get_event(EventMask::KEY_PRESS | EventMask::QUIT, 100_000) {
            if let Some(k) = ev.key() {
                if k.ch == b'q' as i32 || k.ch == key::ESCAPE {
                    break;
                }
            }
        }
    }

    Ok(())
}
