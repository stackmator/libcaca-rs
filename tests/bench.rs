//! Port of libcaca's `caca/t/bench.c` benchmark program.
//!
//! The C version runs 1M blits / 50M putchars against a null display and
//! reports `caca_get_display_time`. Loop counts are scaled down so the test
//! finishes in well under a second; it asserts functional correctness (blit
//! lands, putchars land) and exercises the same display-time measurement
//! path rather than asserting on wall-clock speed.

use libcaca::{Canvas, Display};

const BLIT_LOOPS: i32 = 200;
const PUTCHAR_LOOPS: i32 = 20_000;

fn blit(mask: bool, clear: bool) {
    let mut cv = Canvas::new(40, 40).unwrap();
    let mut cv2 = Canvas::new(16, 16).unwrap();
    cv2.fill_box(0, 0, 16, 16, b'x' as u32);
    let mut mask_cv = Canvas::new(16, 16).unwrap();
    mask_cv.fill_box(0, 0, 16, 16, b'x' as u32);
    for _ in 0..BLIT_LOOPS {
        if clear {
            cv.clear();
        }
        if mask {
            cv.blit(1, 1, &cv2, Some(&mask_cv)).unwrap();
        } else {
            cv.blit(1, 1, &cv2, None).unwrap();
        }
    }
    assert_eq!(cv.get_char(1, 1), b'x' as u32);
}

fn putchars(optim: bool) {
    let mut cv = Canvas::new(40, 40).unwrap();
    if optim {
        cv.disable_dirty_rect();
    }
    for _ in 0..PUTCHAR_LOOPS {
        cv.put_char(1, 1, b'x' as u32);
        cv.put_char(1, 1, b'o' as u32);
    }
    if optim {
        cv.enable_dirty_rect().unwrap();
        cv.add_dirty_rect(1, 1, 1, 1).unwrap();
    }
    assert_eq!(cv.get_char(1, 1), b'o' as u32);
}

fn timed(desc: &str, f: impl FnOnce()) {
    let mut dp = Display::with_driver(Canvas::new(0, 0).unwrap(), Some("null")).unwrap();
    dp.refresh().unwrap();
    f();
    dp.refresh().unwrap();
    eprintln!("{:25}: {}ms", desc, dp.display_time() / 1000);
}

#[test]
fn bench_blit_and_putchars() {
    timed("blit no mask, no clear", || blit(false, false));
    timed("blit no mask, clear", || blit(false, true));
    timed("blit mask, no clear", || blit(true, false));
    timed("blit mask, clear", || blit(true, true));
    timed("putchars, no optim", || putchars(false));
    timed("putchars, optim", || putchars(true));
}
