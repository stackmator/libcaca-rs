//! Port of libcaca's `examples/driver.c`.
//!
//! Lists the available display drivers, marks the active one, and cycles to
//! the next driver every 5 seconds until a key is pressed. Headless runs
//! render the list once and exit.

use libcaca::display::DRIVER_LIST;
use libcaca::{Color, Display, Driver, EventMask};

fn main() -> libcaca::Result<()> {
    let mut dp = Display::create()?;
    dp.canvas_mut().set_color_ansi(Color::White, Color::Black)?;

    let interactive = matches!(dp.driver(), Driver::Terminal | Driver::Win32);

    let mut cur = 0usize;
    for (i, (name, _)) in DRIVER_LIST.iter().enumerate() {
        if *name == dp.driver_name() {
            cur = i;
        }
    }

    loop {
        let driver = dp.driver_name().to_string();
        {
            let cv = dp.canvas_mut();
            cv.put_str(1, 0, "Available drivers:");
            for (i, (name, desc)) in DRIVER_LIST.iter().enumerate() {
                let matched = *name == driver;
                cv.draw_line(0, i as i32 + 2, 9999, i as i32 + 2, b' ' as u32);
                cv.printf(
                    2,
                    i as i32 + 2,
                    format_args!("{} {} ({})", if matched { '*' } else { ' ' }, name, desc),
                );
            }
            cv.put_str(
                1,
                DRIVER_LIST.len() as i32 + 2,
                "Switching driver in 5 seconds",
            );
        }

        dp.refresh()?;

        if !interactive {
            break;
        }

        if dp.get_event(EventMask::KEY_PRESS, 5_000_000).is_some() {
            break;
        }

        // Cycle to the next driver, skipping raw output, retrying on failure.
        for _ in 0..DRIVER_LIST.len() {
            cur = (cur + 1) % DRIVER_LIST.len();
            if DRIVER_LIST[cur].0 == "raw" {
                continue;
            }
            if dp.set_driver(DRIVER_LIST[cur].0).is_ok() {
                break;
            }
        }
    }

    Ok(())
}
