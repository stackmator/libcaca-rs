//! Port of libcaca's `examples/canvas.c` full-API stress test.
//!
//! Creates random canvases and hammers `put_char`/`get_char`, then creates
//! 128 frames while assigning random-length frame names. Failures are
//! reported on stderr like the C version; success prints `all tests passed`.

use libcaca::{rand, Canvas};

const ITER: u32 = 128;

fn main() -> libcaca::Result<()> {
    eprintln!("testing caca_create_canvas()");
    for _ in 0..ITER {
        let w = rand(1, 1000);
        let h = rand(1, 1000);
        let mut cv = Canvas::new(w, h)?;
        cv.put_char(w - 1, h - 1, b'x' as u32);
        if cv.get_char(w - 1, h - 1) != b'x' as u32 {
            eprintln!("  failed ({w}x{h})");
        }
    }

    eprintln!("testing caca_set_frame_name()");
    let mut cv = Canvas::new(1, 1)?;
    for _ in 0..ITER {
        cv.create_frame(0)?;
        for _ in 0..ITER {
            let w = rand(1, 8191) as usize;
            cv.set_frame_name(&"x".repeat(w));
        }
    }

    eprintln!("all tests passed");
    Ok(())
}
