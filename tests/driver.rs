//! Port of libcaca's `caca/t/driver.cpp` CppUnit suite.
//!
//! The C `test_list` only asserts the driver list is non-empty with a valid
//! first entry; the port adds coverage for the pure-Rust `ncurses`/`slang`
//! aliases (see `src/display/ncurses.rs`, `src/display/slang.rs`).

use libcaca::display::DRIVER_LIST;
use libcaca::Driver;

#[test]
fn test_list() {
    assert!(!DRIVER_LIST.is_empty());
    assert!(!DRIVER_LIST[0].0.is_empty());
}

#[test]
fn ncurses_slang_aliases_resolve() {
    assert_eq!(Driver::from_name("ncurses"), Some(Driver::Ncurses));
    assert_eq!(Driver::from_name("curses"), Some(Driver::Ncurses));
    assert_eq!(Driver::from_name("slang"), Some(Driver::Slang));
    assert!(DRIVER_LIST.iter().any(|(n, _)| *n == "ncurses"));
    assert!(DRIVER_LIST.iter().any(|(n, _)| *n == "slang"));
}
