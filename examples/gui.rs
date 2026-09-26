//! Graphical window demo for the `winit` driver.
//!
//! Requires the `gui` cargo feature: `cargo run --features gui --example gui`.
//! Renders the same scene as `hello` into a real window. Press `q` or Escape,
//! or close the window, to quit.

use libcaca::{key, Canvas, Color, Display, EventMask};

fn main() -> libcaca::Result<()> {
    let canvas = Canvas::new(80, 24)?;
    let mut dp = Display::with_driver(canvas, Some("winit"))?;
    let _ = dp.set_title("libcaca-rs gui demo");

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

        if let Some(ev) = dp.get_event(EventMask::KEY_PRESS | EventMask::QUIT, 100_000) {
            if ev.event_type() == EventMask::QUIT {
                break;
            }
            if let Some(k) = ev.key() {
                if k.ch == b'q' as i32 || k.ch == key::ESCAPE {
                    break;
                }
            }
        }
    }

    Ok(())
}
