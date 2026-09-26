//! Port of libcaca's `examples/mouse.c`.
//!
//! Tracks the mouse, draws a crosshair and reports button presses. Any key
//! quits. Headless runs render a few frames with synthetic events.

use libcaca::{Canvas, Display, Driver, Event, EventMask};

fn main() -> libcaca::Result<()> {
    let mut dp = Display::new(Canvas::new(80, 24)?)?;

    dp.set_display_time(40_000)?;
    let _ = dp.set_cursor(false);

    let interactive = matches!(dp.driver(), Driver::Terminal | Driver::Win32);
    if !interactive {
        dp.push_event(Event::MouseMotion { x: 10, y: 5 });
        dp.push_event(Event::MousePress {
            x: 10,
            y: 5,
            button: 1,
        });
    }

    let (mut x, mut y) = (0i32, 0i32);
    let (mut p, mut b) = (0i32, 0i32);
    let mut frames = 0u32;

    loop {
        while let Some(ev) = dp.get_event(EventMask::ANY, 0) {
            match ev {
                Event::KeyPress(_) => {
                    if interactive {
                        return Ok(());
                    }
                }
                Event::MouseMotion { x: mx, y: my } => {
                    x = mx;
                    y = my;
                }
                Event::MousePress { button, .. } => {
                    p = 1;
                    b = button;
                }
                Event::MouseRelease { button, .. } => {
                    p = 0;
                    b = button;
                }
                _ => {}
            }
        }

        {
            let cv = dp.canvas_mut();
            cv.printf(0, 0, format_args!("{},{}", x, y));
            if b != 0 {
                cv.printf(
                    0,
                    1,
                    format_args!(
                        "Mouse button {} {}",
                        b,
                        if p == 1 { "pressed" } else { "released" }
                    ),
                );
            }
            cv.printf(x - 2, y - 1, format_args!("  |"));
            cv.printf(x - 2, y, format_args!("--|--"));
            cv.printf(x - 2, y + 1, format_args!("  |"));
        }

        dp.refresh()?;
        dp.canvas_mut().clear();

        if !interactive {
            frames += 1;
            if frames >= 5 {
                break;
            }
        }
    }

    Ok(())
}
