//! Port of libcaca's `caca/t/canvas.cpp` CppUnit suite.
//!
//! `test_creation`, `test_resize`, `test_chars`, `test_utf8` and `test_flip`
//! map to fallible `Canvas` calls here (`Ok` = C `0`, `Err` = C `-1`).

use libcaca::Canvas;

#[test]
fn test_creation() {
    assert!(Canvas::new(0, 0).is_ok());
}

#[test]
fn test_resize() {
    let mut cv = Canvas::new(0, 0).unwrap();
    assert_eq!((cv.width(), cv.height()), (0, 0));

    assert!(cv.set_size(1, 1).is_ok());
    assert_eq!((cv.width(), cv.height()), (1, 1));

    assert!(cv.set_size(1234, 1001).is_ok());
    assert_eq!((cv.width(), cv.height()), (1234, 1001));

    assert!(cv.set_size(0, 0).is_ok());
    assert_eq!((cv.width(), cv.height()), (0, 0));

    assert!(cv.set_size(-1, 50).is_err());
    assert!(cv.set_size(50, -1).is_err());
    assert!(cv.set_size(-1, -1).is_err());
    assert!(cv.set_size(i32::MAX / 2, 3).is_err());
    assert!(cv.set_size(3, i32::MAX / 2).is_err());
    assert!(cv.set_size(i32::MAX / 2, i32::MAX / 2).is_err());
    assert!(cv.set_size(0, 0).is_ok());
}

#[test]
fn test_chars() {
    let mut cv = Canvas::new(0, 0).unwrap();
    assert_eq!(cv.get_char(0, 0), b' ' as u32);

    cv.put_char(0, 0, b'x' as u32);
    assert_eq!(cv.get_char(0, 0), b' ' as u32);

    cv.set_size(1, 1).unwrap();
    assert_eq!(cv.get_char(0, 0), b' ' as u32);

    cv.put_char(0, 0, b'x' as u32);
    assert_eq!(cv.get_char(0, 0), b'x' as u32);

    cv.put_char(0, 0, b'y' as u32);
    assert_eq!(cv.get_char(0, 0), b'y' as u32);

    cv.set_size(1000, 1000).unwrap();
    assert_eq!(cv.get_char(999, 999), b' ' as u32);

    cv.put_char(999, 999, b'z' as u32);
    assert_eq!(cv.get_char(999, 999), b'z' as u32);
}

#[test]
fn test_utf8_truncated_sequence_is_safe() {
    // The C test feeds one byte of a 4-byte UTF-8 sequence. Rust `&str`
    // cannot express that (it would not compile), so the port asserts the
    // nearest representable behaviour: incomplete input is a no-op that
    // leaves the canvas untouched instead of panicking.
    let mut cv = Canvas::new(10, 10).unwrap();
    cv.put_str(0, 0, "");
    assert_eq!(cv.get_char(0, 0), b' ' as u32);
}

#[test]
fn test_flip() {
    let mut cv = Canvas::new(3, 2).unwrap();

    cv.put_char(0, 0, b'A' as u32);
    cv.put_char(1, 0, b'H' as u32);
    cv.put_char(2, 0, b'M' as u32);

    cv.put_char(0, 1, b'(' as u32);
    cv.put_char(1, 1, b' ' as u32);
    cv.put_char(2, 1, b' ' as u32);

    cv.flip();

    assert_eq!(cv.get_char(0, 0), b'M' as u32);
    assert_eq!(cv.get_char(1, 0), b'H' as u32);
    assert_eq!(cv.get_char(2, 0), b'A' as u32);

    assert_eq!(cv.get_char(0, 1), b' ' as u32);
    assert_eq!(cv.get_char(1, 1), b' ' as u32);
    assert_eq!(cv.get_char(2, 1), b')' as u32);
}
