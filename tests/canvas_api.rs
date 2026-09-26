//! Port of libcaca's `examples/canvas.c` API stress test.
//!
//! Creates random canvases and hammers frame-name assignment, asserting the
//! basic canvas contract holds throughout.

use libcaca::{rand, Canvas};

#[test]
fn create_put_get_stress() {
    for _ in 0..128 {
        let w = rand(1, 1000);
        let h = rand(1, 1000);
        let mut cv = Canvas::new(w, h).unwrap();
        cv.put_char(w - 1, h - 1, b'x' as u32);
        assert_eq!(cv.get_char(w - 1, h - 1), b'x' as u32);
    }
}

#[test]
fn frame_name_stress() {
    let mut cv = Canvas::new(1, 1).unwrap();

    for _ in 0..128 {
        cv.create_frame(0).unwrap();
        for _ in 0..128 {
            let w = rand(1, 512) as usize;
            cv.set_frame_name(&"x".repeat(w));
        }
    }

    assert_eq!(cv.frame_count(), 129);
}
