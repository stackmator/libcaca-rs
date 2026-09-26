//! Port of libcaca's `examples/trifiller.c`.
//!
//! Renders a rotating textured square with `fill_triangle_textured`. Keys:
//! arrows move it, `a`/`s` rotate it, `q` or Escape quits. The texture is the
//! generated colour ramp (the C version can alternatively dither an image
//! file, which needs an image decoder this port does not ship).

use std::f32::consts::PI;

use libcaca::{key, Canvas, Color, Display, Driver, Event, EventMask};

const SQUARE_SIZE: f32 = 20.0;

fn main() -> libcaca::Result<()> {
    let mut dp = Display::new(Canvas::new(0, 0)?)?;
    let mut tex = Canvas::new(16, 16)?;

    let _ = dp.set_title("trifiller");
    dp.set_display_time(10_000)?;

    for i in 0..16 {
        tex.set_color_ansi(
            Color::from_u8(((i + 1) % 0x0f) as u8).unwrap_or(Color::White),
            Color::from_u8((i % 0x0f) as u8).unwrap_or(Color::Black),
        )?;
        tex.put_str(0, i, "0123456789ABCDEF");
    }

    let square = [
        [-SQUARE_SIZE, -SQUARE_SIZE],
        [SQUARE_SIZE, -SQUARE_SIZE],
        [SQUARE_SIZE, SQUARE_SIZE],
        [-SQUARE_SIZE, SQUARE_SIZE],
    ];
    let uv1 = [0.0f32, 0.0, 1.0, 0.0, 1.0, 1.0];
    let uv2 = [0.0f32, 0.0, 1.0, 1.0, 0.0, 1.0];

    let interactive = matches!(dp.driver(), Driver::Terminal | Driver::Win32);
    let mut quit = false;
    let mut px = 0i32;
    let mut py = 0i32;
    let mut angle = 0.0f32;
    let mut frames = 0u32;

    while !quit {
        let mask = EventMask::KEY_PRESS | EventMask::RESIZE | EventMask::QUIT;
        let mut event = dp.get_event(mask, 0);
        while let Some(ev) = event {
            match ev {
                Event::KeyPress(k) => match k.ch {
                    c if c == b'q' as i32 || c == b'Q' as i32 || c == key::ESCAPE => {
                        quit = true;
                    }
                    c if c == key::UP => py -= 1,
                    c if c == key::DOWN => py += 1,
                    c if c == key::LEFT => px -= 1,
                    c if c == key::RIGHT => px += 1,
                    c if c == b'a' as i32 => angle += 1.0,
                    c if c == b's' as i32 => angle -= 1.0,
                    _ => {}
                },
                Event::Resize { .. } => {
                    dp.refresh()?;
                }
                Event::Quit => quit = true,
                _ => {}
            }
            event = dp.get_event(EventMask::KEY_PRESS, 0);
        }

        let (ww, wh) = {
            let cv = dp.canvas();
            (cv.width(), cv.height())
        };

        // Rotate the square around the screen centre.
        let mut rotated = [[0.0f32; 2]; 4];
        for (p, corner) in square.iter().enumerate() {
            let rad = angle * PI / 180.0;
            rotated[p][0] = corner[0] * rad.cos() - corner[1] * rad.sin() + ww as f32 / 2.0
                + px as f32;
            rotated[p][1] = corner[0] * rad.sin() + corner[1] * rad.cos() + wh as f32 / 2.0
                + py as f32;
        }
        angle += 1.0;

        let coords1 = [
            rotated[0][0] as i32,
            rotated[0][1] as i32,
            rotated[1][0] as i32,
            rotated[1][1] as i32,
            rotated[2][0] as i32,
            rotated[2][1] as i32,
        ];
        let coords2 = [
            rotated[0][0] as i32,
            rotated[0][1] as i32,
            rotated[2][0] as i32,
            rotated[2][1] as i32,
            rotated[3][0] as i32,
            rotated[3][1] as i32,
        ];

        {
            let cv = dp.canvas_mut();
            cv.fill_triangle_textured(coords1, &tex, uv1);
            cv.fill_triangle_textured(coords2, &tex, uv2);
        }

        dp.refresh()?;
        dp.canvas_mut().clear();

        if !interactive {
            frames += 1;
            if frames >= 30 {
                break;
            }
        }
    }

    Ok(())
}
