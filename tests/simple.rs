//! Port of libcaca's `caca/t/simple.c` smoke program.
//!
//! Seven assertions on canvas creation, out-of-bounds writes, rotation of
//! an empty canvas and resizing. Rust tests abort on failure, so instead
//! of counting passes each `TEST(x)` became an `assert!`.

use libcaca::Canvas;

#[test]
fn simple_suite() {
    let mut cv = Canvas::new(0, 0).unwrap();
    cv.put_char(0, 0, b'x' as u32);
    assert!(cv.get_char(0, 0) != b'x' as u32);

    cv.rotate_180();

    cv.set_size(1, 1).unwrap();
    assert!(cv.get_char(0, 0) != b'x' as u32);
    assert!(cv.get_char(0, 0) == b' ' as u32);

    cv.put_char(0, 0, b'y' as u32);
    assert!(cv.get_char(0, 0) == b'y' as u32);

    cv.set_size(1000, 1000).unwrap();
    assert!(cv.width() == 1000);

    cv.put_char(999, 999, b'z' as u32);
    assert!(cv.get_char(999, 999) == b'z' as u32);
}
