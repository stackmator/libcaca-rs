//! The libcaca 0.9 demo (`demo0.c`), kept as a duplicate of [`demo`](demo.rs)
//! written against the deprecated `caca0` API.
//!
//! This exercises the [`Compat`](libcaca::compat::Compat) pre-1.0 shim
//! end to end: packed `u32` events, `DITHERING_*` feature values, colour
//! indices and the old drawing vocabulary. The `#if 0` sprite blocks from
//! the C version are omitted (the sprite was never loaded there either).
//!
//! Requires the `compat` cargo feature.

use libcaca::compat::{Compat, DITHERING_NONE, EVENT_ANY, EVENT_KEY_PRESS, EVENT_MOUSE_MOTION};
use std::f64::consts::PI;
use std::process::exit;

// Old colour indices (caca0.h maps them 1:1 onto the modern palette).
const BLACK: u8 = 0;
const BLUE: u8 = 1;
const GREEN: u8 = 2;
const CYAN: u8 = 3;
const RED: u8 = 4;
const LIGHTGRAY: u8 = 7;
const YELLOW: u8 = 14;
const WHITE: u8 = 15;

const KEY_ESCAPE: i32 = 0x1b;

struct App {
    c: Compat,
    bounds: bool,
    outline: i32,
    dithering: i32,
    frame: i32,
    render_frame: i32,
}

type DemoFn = fn(&mut App);

fn main() {
    let mut app = match Compat::init() {
        Ok(c) => App {
            c,
            bounds: false,
            outline: 0,
            dithering: 0,
            frame: 0,
            render_frame: 0,
        },
        Err(_) => exit(1),
    };

    let _ = app.c.set_delay(40000);

    // Main menu.
    display_menu(&mut app);
    let _ = app.c.refresh();

    // Go!
    let mut demo: Option<DemoFn> = None;
    let mut quit = false;
    while !quit {
        let mut menu = false;
        let mut mouse = false;
        let mut xmouse = 0;
        let mut ymouse = 0;

        loop {
            let event = app.c.get_event(EVENT_ANY, 0);
            if event == 0 {
                break;
            }
            if demo.is_some() && event & EVENT_KEY_PRESS != 0 {
                menu = true;
                demo = None;
            } else if event & EVENT_KEY_PRESS != 0 {
                match (event & 0xffff) as i32 {
                    x if x == b'q' as i32 || x == b'Q' as i32 || x == KEY_ESCAPE => {
                        demo = None;
                        quit = true;
                    }
                    x if x == b'o' as i32 || x == b'O' as i32 => {
                        app.outline = (app.outline + 1) % 3;
                        display_menu(&mut app);
                        let _ = app.c.refresh();
                    }
                    x if x == b'b' as i32 || x == b'B' as i32 => {
                        app.bounds = !app.bounds;
                        display_menu(&mut app);
                        let _ = app.c.refresh();
                    }
                    x if x == b'd' as i32 || x == b'D' as i32 => {
                        app.dithering = (app.dithering + 1) % 5;
                        app.c.set_feature(DITHERING_NONE + app.dithering);
                        display_menu(&mut app);
                        let _ = app.c.refresh();
                    }
                    x if x == b'c' as i32 => demo = Some(demo_color),
                    x if x == b'f' as i32 || x == b'F' as i32 => demo = Some(demo_all),
                    x if x == b'1' as i32 => demo = Some(demo_dots),
                    x if x == b'2' as i32 => demo = Some(demo_lines),
                    x if x == b'3' as i32 => demo = Some(demo_boxes),
                    x if x == b'4' as i32 => demo = Some(demo_triangles),
                    x if x == b'5' as i32 => demo = Some(demo_ellipses),
                    x if x == b'r' as i32 || x == b'R' as i32 => {
                        demo = Some(demo_render);
                    }
                    _ => {}
                }

                if demo.is_some() {
                    app.c.clear();
                }
            } else if event & EVENT_MOUSE_MOTION != 0 {
                mouse = true;
                xmouse = ((event & 0xfff000) >> 12) as i32;
                ymouse = (event & 0xfff) as i32;
            }
        }

        if menu || (mouse && demo.is_none()) {
            display_menu(&mut app);
            if mouse && demo.is_none() {
                let _ = app.c.set_color(RED, BLACK);
                app.c.putstr(xmouse, ymouse, "|\\");
            }
            let _ = app.c.refresh();
        } else if let Some(d) = demo {
            d(&mut app);

            let _ = app.c.set_color(LIGHTGRAY, BLACK);
            app.c
                .draw_thin_box(1, 1, app.c.width() - 2, app.c.height() - 2);
            let rt = app.c.rendertime().max(1);
            app.c.printf(
                4,
                1,
                format_args!("[{}.{} fps]----", 1_000_000 / rt, (10_000_000 / rt) % 10),
            );
            let _ = app.c.refresh();
        }
    }

    // Old `caca_end()`: `Compat` releases the display when dropped.
}

fn display_menu(app: &mut App) {
    let xo = app.c.width() - 2;
    let yo = app.c.height() - 2;

    app.c.clear();
    let _ = app.c.set_color(LIGHTGRAY, BLACK);
    app.c.draw_thin_box(1, 1, xo, yo);

    app.c.putstr((xo - 12) / 2, 3, "libcaca demo");
    app.c.putstr((xo - 14) / 2, 4, "==============");

    app.c.putstr(4, 6, "demos:");
    app.c.putstr(4, 7, "'f': full");
    app.c.putstr(4, 8, "'1': dots");
    app.c.putstr(4, 9, "'2': lines");
    app.c.putstr(4, 10, "'3': boxes");
    app.c.putstr(4, 11, "'4': triangles");
    app.c.putstr(4, 12, "'5': ellipses");
    app.c.putstr(4, 13, "'c': colour");
    app.c.putstr(4, 14, "'r': render");

    app.c.putstr(4, 16, "settings:");
    app.c.printf(
        4,
        17,
        format_args!(
            "'o': outline: {}",
            match app.outline {
                1 => "solid",
                2 => "thin",
                _ => "none",
            }
        ),
    );
    app.c.printf(
        4,
        18,
        format_args!(
            "'b': drawing boundaries: {}",
            if app.bounds { "infinite" } else { "screen" }
        ),
    );
    app.c.printf(
        4,
        19,
        format_args!(
            "'d': dithering ({})",
            Compat::feature_name(DITHERING_NONE + app.dithering)
        ),
    );

    app.c.putstr(4, yo - 2, "'q': quit");
}

#[allow(clippy::too_many_lines)]
fn demo_all(app: &mut App) {
    app.frame = app.frame.wrapping_add(1);
    let i = f64::from(app.frame);
    let w = app.c.width();
    let h = app.c.height();

    app.c.clear();

    // Draw the sun.
    let _ = app.c.set_color(YELLOW, BLACK);
    let xo = w / 4;
    let yo = h / 4 + (5.0 * (0.03 * i).sin()) as i32;

    for j in 0..16 {
        let t = 0.03 * i + PI * f64::from(j) / 8.0;
        let xa = xo - ((30.0 + (0.03 * i).sin() * 8.0) * t.sin()) as i32;
        let ya = yo + ((15.0 + (0.03 * i).sin() * 4.0) * t.cos()) as i32;
        app.c.draw_thin_line(xo, yo, xa, ya);
    }

    let j = (15.0 + (0.03 * i).sin() * 8.0) as i32;
    let _ = app.c.set_color(WHITE, BLACK);
    app.c.fill_ellipse(xo, yo, j, j / 2, b'#' as u32);
    let _ = app.c.set_color(YELLOW, BLACK);
    app.c.draw_ellipse(xo, yo, j, j / 2, b'#' as u32);

    // Draw the pyramid.
    let xo = w * 5 / 8;
    let yo = 2;

    let xa = w / 8 + ((0.03 * i).sin() * 5.0) as i32;
    let ya = h / 2 + ((0.03 * i).cos() * 5.0) as i32;

    let xb = w - 10 - ((0.02 * i).cos() * 10.0) as i32;
    let yb = h * 3 / 4 - 5 + ((0.02 * i).sin() * 5.0) as i32;

    let xc = w / 4 - ((0.02 * i).sin() * 5.0) as i32;
    let yc = h * 3 / 4 + ((0.02 * i).cos() * 5.0) as i32;

    let _ = app.c.set_color(GREEN, BLACK);
    app.c.fill_triangle(xo, yo, xb, yb, xa, ya, b'%' as u32);
    let _ = app.c.set_color(YELLOW, BLACK);
    app.c.draw_thin_triangle(xo, yo, xb, yb, xa, ya);

    let _ = app.c.set_color(RED, BLACK);
    app.c.fill_triangle(xa, ya, xb, yb, xc, yc, b'#' as u32);
    let _ = app.c.set_color(YELLOW, BLACK);
    app.c.draw_thin_triangle(xa, ya, xb, yb, xc, yc);

    let _ = app.c.set_color(BLUE, BLACK);
    app.c.fill_triangle(xo, yo, xb, yb, xc, yc, b'%' as u32);
    let _ = app.c.set_color(YELLOW, BLACK);
    app.c.draw_thin_triangle(xo, yo, xb, yb, xc, yc);

    // Draw a background triangle.
    let xa = 2;
    let ya = 2;

    let xb = w - 3;
    let yb = h / 2;

    let xc = w / 3;
    let yc = h - 3;

    let _ = app.c.set_color(CYAN, BLACK);
    app.c.draw_thin_triangle(xa, ya, xb, yb, xc, yc);

    let xo = w / 2 + ((0.027 * i).cos() * f64::from(w) / 3.0) as i32;
    let yo = h / 2 - ((0.027 * i).sin() * f64::from(h) / 2.0) as i32;

    app.c.draw_thin_line(xa, ya, xo, yo);
    app.c.draw_thin_line(xb, yb, xo, yo);
    app.c.draw_thin_line(xc, yc, xo, yo);

    // Draw a trail behind the foreground sprite.
    for j in app.frame - 60..app.frame {
        let f = f64::from(j);
        let delta = Compat::rand(-5, 5);
        let _ = app
            .c
            .set_color(Compat::rand(0, 15) as u8, Compat::rand(0, 15) as u8);
        app.c.putchar(
            w / 2 + ((0.02 * f).cos() * f64::from(delta + w / 4)) as i32,
            h / 2 + ((0.02 * f).sin() * f64::from(delta + h / 3)) as i32,
            b'#' as u32,
        );
    }
}

fn demo_dots(app: &mut App) {
    let xmax = app.c.width() - 1;
    let ymax = app.c.height() - 1;
    const CHARS: &[u8] = b"+-*#X@%$MW";

    for _ in 0..1000 {
        // Putpixel.
        let _ = app
            .c
            .set_color(Compat::rand(0, 15) as u8, Compat::rand(0, 15) as u8);
        app.c.putchar(
            Compat::rand(0, xmax),
            Compat::rand(0, ymax),
            u32::from(CHARS[Compat::rand(0, 9) as usize]),
        );
    }
}

fn demo_color(app: &mut App) {
    app.c.clear();
    for i in 0..16 {
        app.c.printf(
            4,
            i + if i >= 8 { 4 } else { 3 },
            format_args!(
                "'{}': {} ({})",
                (b'a' + i as u8) as char,
                i,
                Compat::color_name(i as u8)
            ),
        );
        let _ = app.c.set_color(LIGHTGRAY, BLACK);
        for j in 0..16 {
            let _ = app.c.set_color(i as u8, j as u8);
            app.c.putstr(
                (if j >= 8 { 41 } else { 40 }) + j * 2,
                i + if i >= 8 { 4 } else { 3 },
                "# ",
            );
        }
    }
}

fn demo_lines(app: &mut App) {
    let w = app.c.width();
    let h = app.c.height();

    let (xa, ya, xb, yb) = if app.bounds {
        (
            Compat::rand(-w, 2 * w),
            Compat::rand(-h, 2 * h),
            Compat::rand(-w, 2 * w),
            Compat::rand(-h, 2 * h),
        )
    } else {
        (
            Compat::rand(0, w - 1),
            Compat::rand(0, h - 1),
            Compat::rand(0, w - 1),
            Compat::rand(0, h - 1),
        )
    };

    let _ = app.c.set_color(Compat::rand(0, 15) as u8, BLACK);
    if app.outline > 1 {
        app.c.draw_thin_line(xa, ya, xb, yb);
    } else {
        app.c.draw_line(xa, ya, xb, yb, b'#' as u32);
    }
}

fn demo_boxes(app: &mut App) {
    let w = app.c.width();
    let h = app.c.height();

    let (xa, ya, xb, yb) = if app.bounds {
        (
            Compat::rand(-w, 2 * w),
            Compat::rand(-h, 2 * h),
            Compat::rand(-w, 2 * w),
            Compat::rand(-h, 2 * h),
        )
    } else {
        (
            Compat::rand(0, w - 1),
            Compat::rand(0, h - 1),
            Compat::rand(0, w - 1),
            Compat::rand(0, h - 1),
        )
    };

    let _ = app
        .c
        .set_color(Compat::rand(0, 15) as u8, Compat::rand(0, 15) as u8);
    app.c.fill_box(xa, ya, xb, yb, b'#' as u32);

    let _ = app.c.set_color(Compat::rand(0, 15) as u8, BLACK);
    if app.outline == 2 {
        app.c.draw_thin_box(xa, ya, xb, yb);
    } else if app.outline == 1 {
        app.c.draw_box(xa, ya, xb, yb, b'#' as u32);
    }
}

fn demo_ellipses(app: &mut App) {
    let w = app.c.width();
    let h = app.c.height();

    let (x, y, a, b) = if app.bounds {
        (
            Compat::rand(-w, 2 * w),
            Compat::rand(-h, 2 * h),
            Compat::rand(0, w),
            Compat::rand(0, h),
        )
    } else {
        loop {
            let x = Compat::rand(0, w);
            let y = Compat::rand(0, h);
            let a = Compat::rand(0, w);
            let b = Compat::rand(0, h);

            if !(x - a < 0 || x + a >= w || y - b < 0 || y + b >= h) {
                break (x, y, a, b);
            }
        }
    };

    let _ = app
        .c
        .set_color(Compat::rand(0, 15) as u8, Compat::rand(0, 15) as u8);
    app.c.fill_ellipse(x, y, a, b, b'#' as u32);

    let _ = app.c.set_color(Compat::rand(0, 15) as u8, BLACK);
    if app.outline == 2 {
        app.c.draw_thin_ellipse(x, y, a, b);
    } else if app.outline == 1 {
        app.c.draw_ellipse(x, y, a, b, b'#' as u32);
    }
}

fn demo_triangles(app: &mut App) {
    let w = app.c.width();
    let h = app.c.height();

    let (xa, ya, xb, yb, xc, yc) = if app.bounds {
        (
            Compat::rand(-w, 2 * w),
            Compat::rand(-h, 2 * h),
            Compat::rand(-w, 2 * w),
            Compat::rand(-h, 2 * h),
            Compat::rand(-w, 2 * w),
            Compat::rand(-h, 2 * h),
        )
    } else {
        (
            Compat::rand(0, w - 1),
            Compat::rand(0, h - 1),
            Compat::rand(0, w - 1),
            Compat::rand(0, h - 1),
            Compat::rand(0, w - 1),
            Compat::rand(0, h - 1),
        )
    };

    let _ = app
        .c
        .set_color(Compat::rand(0, 15) as u8, Compat::rand(0, 15) as u8);
    app.c.fill_triangle(xa, ya, xb, yb, xc, yc, b'#' as u32);

    let _ = app.c.set_color(Compat::rand(0, 15) as u8, BLACK);
    if app.outline == 2 {
        app.c.draw_thin_triangle(xa, ya, xb, yb, xc, yc);
    } else if app.outline == 1 {
        app.c.draw_triangle(xa, ya, xb, yb, xc, yc, b'#' as u32);
    }
}

fn demo_render(app: &mut App) {
    app.render_frame = app.render_frame.wrapping_add(1);
    let i = f64::from(app.render_frame);

    let mut buffer = vec![0xff000000u32; 256 * 256];

    // Red.
    let xo = (128.0 + 48.0 * (0.02 * i).sin()) as i32;
    let yo = (128.0 + 48.0 * (0.03 * i).cos()) as i32;
    for z in 0..240 {
        draw_circle(&mut buffer, xo, yo, z, 0x00ff0000, 200 << 16);
    }

    // Green.
    let xo = (128.0 + 48.0 * (2.0 + 0.06 * i).sin()) as i32;
    let yo = (128.0 + 48.0 * (2.0 + 0.05 * i).cos()) as i32;
    for z in 0..240 {
        draw_circle(&mut buffer, xo, yo, z, 0x0000ff00, 200 << 8);
    }

    // Blue.
    let xo = (128.0 + 48.0 * (1.0 + 0.04 * i).sin()) as i32;
    let yo = (128.0 + 48.0 * (1.0 + 0.03 * i).cos()) as i32;
    for z in 0..240 {
        draw_circle(&mut buffer, xo, yo, z, 0x000000ff, 200);
    }

    if let Ok(handle) = app.c.create_bitmap(
        32,
        256,
        256,
        4 * 256,
        0x00ff0000,
        0x0000ff00,
        0x000000ff,
        0xff000000,
    ) {
        // Old `(char *)buffer` cast: flatten to native-endian bytes.
        let mut raw = Vec::with_capacity(buffer.len() * 4);
        for pixel in &buffer {
            raw.extend_from_slice(&pixel.to_ne_bytes());
        }
        let _ = app
            .c
            .draw_bitmap(0, 0, app.c.width() - 1, app.c.height() - 1, handle, &raw);
        app.c.free_bitmap(handle);
    }
}

fn draw_circle(buffer: &mut [u32], x: i32, y: i32, r: i32, mask: u32, val: u32) {
    // Old `POINT(X,Y)` macro: set one channel, keep the rest.
    let mut point = |px: i32, py: i32| {
        let idx = (px + 256 * py) as usize;
        buffer[idx] = 0xff000000 | (buffer[idx] & !mask) | val;
    };

    let mut t = 0;
    let mut dy = r;
    let mut dx = 0;
    while dx <= dy {
        point(x - dx / 3, y - dy / 3);
        point(x + dx / 3, y - dy / 3);
        point(x - dx / 3, y + dy / 3);
        point(x + dx / 3, y + dy / 3);

        point(x - dy / 3, y - dx / 3);
        point(x + dy / 3, y - dx / 3);
        point(x - dy / 3, y + dx / 3);
        point(x + dy / 3, y + dx / 3);

        if t > 0 {
            t += dx - dy;
            dy -= 1;
        } else {
            t += dx;
        }
        dx += 1;
    }
}
