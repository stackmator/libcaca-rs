//! ncurses display driver.
//!
//! Port of `caca/driver/ncurses.c`.
//!
//! The C driver links against the ncurses C library for fullscreen terminal
//! output, palette setup (256 colour pairs arranged by `tools/optipal.c`),
//! keyboard/mouse input and resize handling. There is no pure-Rust ncurses
//! crate without a C dependency, so this port resolves the `"ncurses"`
//! driver name to the shared ANSI/VT [`Terminal`](super::terminal::Terminal)
//! backend: the same escape sequences ncurses itself emits on a modern
//! terminal, with raw-mode input, SGR mouse and resize events. Request with
//! `Display::with_driver(cv, Some("ncurses"))` or `CACA_DRIVER=ncurses`.

/// Driver name as known to [`Driver::from_name`](super::Driver::from_name).
pub const NAME: &str = "ncurses";

/// Human-readable description, matching `DRIVER_LIST` wording.
pub const DESCRIPTION: &str = "ANSI terminal (ncurses-compatible)";
