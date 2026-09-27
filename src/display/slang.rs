//! S-Lang display driver.
//!
//! Port of `caca/driver/slang.c`.
//!
//! The C driver links against the S-Lang C library (`slsmg`) for screen
//! management, using the 128-pair palette optimisation from
//! `tools/optipal.c` (S-Lang reserves bit 7 of the colour index for the
//! alternate character set). Like the ncurses driver there is no C-free
//! S-Lang implementation, so this port resolves the `"slang"` driver name
//! to the shared ANSI/VT [`Terminal`](super::terminal::Terminal) backend,
//! which emits the same terminal protocol. Request with
//! `Display::with_driver(cv, Some("slang"))` or `CACA_DRIVER=slang`.

/// Driver name as known to [`Driver::from_name`](super::Driver::from_name).
pub const NAME: &str = "slang";

/// Human-readable description, matching `DRIVER_LIST` wording.
pub const DESCRIPTION: &str = "ANSI terminal (S-Lang-compatible)";
