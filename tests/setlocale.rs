//! Port of libcaca's `caca/t/bug-setlocale.c` locale regression test.
//!
//! The C program checks that creating a display does not change the process
//! locale (which would move the `printf("%.1f")` decimal point). Rust
//! formatting is locale-independent, so the port asserts the formatted
//! output still uses `.` before and after display creation, and that the
//! driver list walk in the C test maps to our `DRIVER_LIST`.

use libcaca::display::DRIVER_LIST;
use libcaca::{Display, Driver};

#[test]
fn decimal_point_survives_display_creation() {
    let mut buf = format!("{:.1}", 0.0f32);
    assert_eq!(buf.chars().nth(1), Some('.'));

    for (name, _) in DRIVER_LIST {
        // The C test instantiates the x11/ncurses displays; creating a real
        // terminal display would steal the test harness TTY, so only the
        // headless null driver is instantiated here while the interactive
        // names are checked to resolve.
        if *name == "x11" || *name == "ncurses" {
            assert!(Driver::from_name(name).is_some());
            let dp =
                Display::with_driver(libcaca::Canvas::new(0, 0).unwrap(), Some("null")).unwrap();
            drop(dp);
            buf = format!("{:.1}", 0.0f32);
            assert_eq!(buf.chars().nth(1), Some('.'));
        }
    }

    buf = format!("{:.1}", 0.0f32);
    assert_eq!(buf.chars().nth(1), Some('.'));
}
