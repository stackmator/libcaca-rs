//! Port of libcaca's `examples/dithering.c`.
//!
//! A procedural dithering test: every cell of a 100x100 field picks the
//! nearer of two palette colours for six fuzzy Voronoi sites (black, 40%,
//! 70%, white, dark red, light red), mixing in uniform noise so the
//! boundary dissolves into a density ramp. Shows one frame, waits for a
//! keypress, exits.

use libcaca::{rand, Canvas, Color, Display, EventMask};

const XRATIO: i32 = 100 * 100;
const YRATIO: i32 = 70 * 70;
const FUZZY: i32 = 5_000_000;

const POINTS: [Color; 6] = [
    Color::Black,
    Color::DarkGray,
    Color::LightGray,
    Color::White,
    Color::Red,
    Color::LightRed,
];

const DENSITY: &[u8] = b" ',+:;o&%w$W@#";

fn main() -> libcaca::Result<()> {
    let mut dp = Display::new(Canvas::new(80, 24)?)?;

    for x in 0..100 {
        for y in 0..100 {
            let mut ch = b'?';

            // Distance to black.
            let mut dista = XRATIO * x * x;
            let mut neara = 0;

            // Distance to 40%.
            let mut dist = XRATIO * (x - 40) * (x - 40) + YRATIO * y * y;
            let (mut nearb, mut distb);
            if rand(-FUZZY, FUZZY + 1) + dist < dista {
                nearb = neara;
                distb = dista;
                neara = 1;
                dista = dist;
            } else {
                nearb = 1;
                distb = dist;
            }

            // Distance to 70%.
            dist = XRATIO * (x - 70) * (x - 70) + YRATIO * y * y;
            if rand(-FUZZY, FUZZY + 1) + dist < dista {
                nearb = neara;
                distb = dista;
                neara = 2;
                dista = dist;
            } else if rand(-FUZZY, FUZZY + 1) + dist < distb {
                nearb = 2;
                distb = dist;
            }

            // Distance to white.
            dist = XRATIO * (x - 100) * (x - 100) + YRATIO * y * y;
            if rand(-FUZZY, FUZZY + 1) + dist < dista {
                nearb = neara;
                distb = dista;
                neara = 3;
                dista = dist;
            } else if rand(-FUZZY, FUZZY + 1) + dist < distb {
                nearb = 3;
                distb = dist;
            }

            // Distance to dark (weighted 12/16).
            dist = XRATIO * (x - 40) * (x - 40) + YRATIO * (y - 100) * (y - 100);
            dist = dist * 12 / 16;
            if rand(-FUZZY, FUZZY + 1) + dist < dista {
                nearb = neara;
                distb = dista;
                neara = 4;
                dista = dist;
            } else if rand(-FUZZY, FUZZY + 1) + dist < distb {
                nearb = 4;
                distb = dist;
            }

            // Distance to light (weighted 8/16).
            dist = XRATIO * (x - 100) * (x - 100) + YRATIO * (y - 100) * (y - 100);
            dist = dist * 8 / 16;
            if rand(-FUZZY, FUZZY + 1) + dist < dista {
                nearb = neara;
                distb = dista;
                neara = 5;
                dista = dist;
            } else if rand(-FUZZY, FUZZY + 1) + dist < distb {
                nearb = 5;
                distb = dist;
            }

            // `dista` can exceed `distb` because of the fuzziness. The C
            // version multiplies in 32-bit `int` (wrapping at the far
            // corners) and indexes past the string; compute in 64 bits and
            // clamp to the ramp instead.
            let (near, far) = if dista > distb {
                (i64::from(distb), i64::from(dista))
            } else {
                (i64::from(dista), i64::from(distb))
            };
            if far > 0 {
                let idx = (near * 2 * 13 / (near + far)).clamp(0, 12) as usize;
                ch = DENSITY[idx];
            }
            let cv = dp.canvas_mut();
            cv.set_color_ansi(POINTS[nearb as usize], POINTS[neara as usize])?;
            cv.put_char(
                x * cv.width() / 100,
                (100 - y) * cv.height() / 100,
                u32::from(ch),
            );
        }
    }

    dp.refresh()?;

    dp.get_event(EventMask::KEY_PRESS, -1);

    Ok(())
}
