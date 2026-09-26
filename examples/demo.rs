//! Port of libcaca's `examples/demo.c`, the flagship demo.
//!
//! A menu of animated primitive demos (dots, lines, boxes, triangles,
//! ellipses, a combined scene and a dithered render). Keys `f`/`1`-`5`/`r`
//! select a demo, `o`/`b` change settings, any other key returns to the menu,
//! `q` quits. Headless runs play each demo briefly and exit.

use std::f64::consts::PI;

use libcaca::{key, rand, Canvas, Color, Display, Dither, Driver, Event, EventMask};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Demo {
    All,
    Dots,
    Lines,
    Boxes,
    Triangles,
    Ellipses,
    Render,
}

struct App {
    display: Display,
    bounds: i32,
    outline: i32,
    demo: Option<Demo>,
    frame: i32,
    render_frame: i32,
}

fn color(n: i32) -> Color {
    Color::from_u8((n & 0x0f) as u8).unwrap_or(Color::White)
}

impl App {
    fn cv(&mut self) -> &mut Canvas {
        self.display.canvas_mut()
    }

    fn display_menu(&mut self) -> libcaca::Result<()> {
        let (xo, yo, w, h);
        {
            let cv = self.cv();
            xo = cv.width() - 2;
            yo = cv.height() - 2;
            w = cv.width();
            h = cv.height();
            cv.set_color_ansi(Color::LightGray, Color::Black)?;
            cv.clear();
            cv.draw_thin_box(1, 1, xo, yo);

            cv.put_str((xo - 12) / 2, 3, "libcaca demo");
            cv.put_str((xo - 14) / 2, 4, "==============");

            cv.put_str(4, 6, "demos:");
            cv.put_str(4, 7, "'f': full");
            cv.put_str(4, 8, "'1': dots");
            cv.put_str(4, 9, "'2': lines");
            cv.put_str(4, 10, "'3': boxes");
            cv.put_str(4, 11, "'4': triangles");
            cv.put_str(4, 12, "'5': ellipses");
            cv.put_str(4, 13, "'c': colour");
            cv.put_str(4, 14, "'r': render");

            cv.put_str(4, 16, "settings:");
            let _ = (w, h);
        }
        let outline = self.outline;
        let bounds = self.bounds;
        {
            let cv = self.cv();
            cv.printf(
                4,
                17,
                format_args!(
                    "'o': outline: {}",
                    match outline {
                        0 => "none",
                        1 => "solid",
                        _ => "thin",
                    }
                ),
            );
            cv.printf(
                4,
                18,
                format_args!(
                    "'b': drawing boundaries: {}",
                    if bounds == 0 { "screen" } else { "infinite" }
                ),
            );
            cv.put_str(4, yo - 2, "'q': quit");
        }
        Ok(())
    }

    fn demo_all(&mut self) -> libcaca::Result<()> {
        let i = self.frame;
        self.frame += 1;
        let (w, h) = {
            let cv = self.cv();
            cv.set_color_ansi(Color::LightGray, Color::Black)?;
            cv.clear();
            (cv.width(), cv.height())
        };

        // Draw the sun.
        let xo = w / 4;
        let yo = h / 4 + (5.0 * (0.03 * i as f64).sin()) as i32;
        {
            let cv = self.cv();
            cv.set_color_ansi(Color::Yellow, Color::Black)?;
            for j in 0..16 {
                let xa = xo
                    - ((30.0 + (0.03 * i as f64).sin() * 8.0)
                        * (0.03 * i as f64 + PI * j as f64 / 8.0).sin())
                        as i32;
                let ya = yo
                    + ((15.0 + (0.03 * i as f64).sin() * 4.0)
                        * (0.03 * i as f64 + PI * j as f64 / 8.0).cos())
                        as i32;
                cv.draw_thin_line(xo, yo, xa, ya);
            }

            let j = (15.0 + (0.03 * i as f64).sin() * 8.0) as i32;
            cv.set_color_ansi(Color::White, Color::Black)?;
            cv.fill_ellipse(xo, yo, j, j / 2, b'#' as u32);
            cv.set_color_ansi(Color::Yellow, Color::Black)?;
            cv.draw_ellipse(xo, yo, j, j / 2, b'#' as u32);
        }

        // Draw the pyramid.
        let xo = w * 5 / 8;
        let yo = 2;
        let xa = w / 8 + (5.0 * (0.03 * i as f64).sin()) as i32;
        let ya = h / 2 + (5.0 * (0.03 * i as f64).cos()) as i32;
        let xb = w - 10 - (10.0 * (0.02 * i as f64).cos()) as i32;
        let yb = h * 3 / 4 - 5 + (5.0 * (0.02 * i as f64).sin()) as i32;
        let xc = w / 4 - (5.0 * (0.02 * i as f64).sin()) as i32;
        let yc = h * 3 / 4 + (5.0 * (0.02 * i as f64).cos()) as i32;
        {
            let cv = self.cv();
            cv.set_color_ansi(Color::Green, Color::Black)?;
            cv.fill_triangle(xo, yo, xb, yb, xa, ya, b'%' as u32);
            cv.set_color_ansi(Color::Yellow, Color::Black)?;
            cv.draw_thin_triangle(xo, yo, xb, yb, xa, ya);

            cv.set_color_ansi(Color::Red, Color::Black)?;
            cv.fill_triangle(xa, ya, xb, yb, xc, yc, b'#' as u32);
            cv.set_color_ansi(Color::Yellow, Color::Black)?;
            cv.draw_thin_triangle(xa, ya, xb, yb, xc, yc);

            cv.set_color_ansi(Color::Blue, Color::Black)?;
            cv.fill_triangle(xo, yo, xb, yb, xc, yc, b'%' as u32);
            cv.set_color_ansi(Color::Yellow, Color::Black)?;
            cv.draw_thin_triangle(xo, yo, xb, yb, xc, yc);
        }

        // Draw a background triangle.
        let xa = 2;
        let ya = 2;
        let xb = w - 3;
        let yb = h / 2;
        let xc = w / 3;
        let yc = h - 3;
        let xo = w / 2 + ((w as f64 / 3.0) * (0.027 * i as f64).cos()) as i32;
        let yo = h / 2 - ((h as f64 / 2.0) * (0.027 * i as f64).sin()) as i32;
        {
            let cv = self.cv();
            cv.set_color_ansi(Color::Cyan, Color::Black)?;
            cv.draw_thin_triangle(xa, ya, xb, yb, xc, yc);
            cv.draw_thin_line(xa, ya, xo, yo);
            cv.draw_thin_line(xb, yb, xo, yo);
            cv.draw_thin_line(xc, yc, xo, yo);
        }

        // Draw a trail of sparks.
        {
            let cv = self.cv();
            for j in (i - 60)..i {
                let delta = rand(-5, 6);
                cv.set_color_ansi(color(rand(0, 16)), color(rand(0, 16)))?;
                cv.put_char(
                    w / 2 + ((delta + w / 4) as f64 * (0.02 * j as f64).cos()) as i32,
                    h / 2 + ((delta + h / 3) as f64 * (0.02 * j as f64).sin()) as i32,
                    b'#' as u32,
                );
            }
        }
        Ok(())
    }

    fn demo_dots(&mut self) -> libcaca::Result<()> {
        const CHARS: &[u8] = b"+-*#X@%$MW";
        let (xmax, ymax) = {
            let cv = self.cv();
            (cv.width(), cv.height())
        };
        // NOTE: the C version draws 1000 dots per frame; keep 200 so the
        // demo stays smooth on software terminals.
        for _ in 0..200 {
            let cv = self.cv();
            cv.set_color_ansi(color(rand(0, 16)), color(rand(0, 16)))?;
            cv.put_char(
                rand(0, xmax),
                rand(0, ymax),
                CHARS[rand(0, 9) as usize] as u32,
            );
        }
        Ok(())
    }

    fn random_line(&mut self) -> (i32, i32, i32, i32) {
        let (w, h, bounds) = {
            let cv = self.cv();
            (cv.width(), cv.height(), self.bounds)
        };
        if bounds != 0 {
            (
                rand(-w, 2 * w),
                rand(-h, 2 * h),
                rand(-w, 2 * w),
                rand(-h, 2 * h),
            )
        } else {
            (rand(0, w), rand(0, h), rand(0, w), rand(0, h))
        }
    }

    fn demo_lines(&mut self) -> libcaca::Result<()> {
        let (xa, ya, xb, yb) = self.random_line();
        let outline = self.outline;
        let cv = self.cv();
        cv.set_color_ansi(color(rand(0, 16)), Color::Black)?;
        if outline > 1 {
            cv.draw_thin_line(xa, ya, xb, yb);
        } else {
            cv.draw_line(xa, ya, xb, yb, b'#' as u32);
        }
        Ok(())
    }

    fn demo_boxes(&mut self) -> libcaca::Result<()> {
        let (xa, ya, xb, yb) = self.random_line();
        let outline = self.outline;
        let cv = self.cv();
        cv.set_color_ansi(color(rand(0, 16)), color(rand(0, 16)))?;
        cv.fill_box(xa, ya, xb, yb, b'#' as u32);
        cv.set_color_ansi(color(rand(0, 16)), Color::Black)?;
        if outline == 2 {
            cv.draw_thin_box(xa, ya, xb, yb);
        } else if outline == 1 {
            cv.draw_box(xa, ya, xb, yb, b'#' as u32);
        }
        Ok(())
    }

    fn demo_ellipses(&mut self) -> libcaca::Result<()> {
        let (w, h, bounds) = {
            let cv = self.cv();
            (cv.width(), cv.height(), self.bounds)
        };
        let (x, y, a, b) = if bounds != 0 {
            (rand(-w, 2 * w), rand(-h, 2 * h), rand(0, w), rand(0, h))
        } else {
            loop {
                let (x, y, a, b) = (rand(0, w), rand(0, h), rand(0, w), rand(0, h));
                if x - a >= 0 && x + a < w && y - b >= 0 && y + b < h {
                    break (x, y, a, b);
                }
            }
        };
        let outline = self.outline;
        let cv = self.cv();
        cv.set_color_ansi(color(rand(0, 16)), color(rand(0, 16)))?;
        cv.fill_ellipse(x, y, a, b, b'#' as u32);
        cv.set_color_ansi(color(rand(0, 16)), Color::Black)?;
        if outline == 2 {
            cv.draw_thin_ellipse(x, y, a, b);
        } else if outline == 1 {
            cv.draw_ellipse(x, y, a, b, b'#' as u32);
        }
        Ok(())
    }

    fn demo_triangles(&mut self) -> libcaca::Result<()> {
        let (w, h, bounds) = {
            let cv = self.cv();
            (cv.width(), cv.height(), self.bounds)
        };
        let (xa, ya, xb, yb, xc, yc) = if bounds != 0 {
            (
                rand(-w, 2 * w),
                rand(-h, 2 * h),
                rand(-w, 2 * w),
                rand(-h, 2 * h),
                rand(-w, 2 * w),
                rand(-h, 2 * h),
            )
        } else {
            (
                rand(0, w),
                rand(0, h),
                rand(0, w),
                rand(0, h),
                rand(0, w),
                rand(0, h),
            )
        };
        let outline = self.outline;
        let cv = self.cv();
        cv.set_color_ansi(color(rand(0, 16)), color(rand(0, 16)))?;
        cv.fill_triangle(xa, ya, xb, yb, xc, yc, b'#' as u32);
        cv.set_color_ansi(color(rand(0, 16)), Color::Black)?;
        if outline == 2 {
            cv.draw_thin_triangle(xa, ya, xb, yb, xc, yc);
        } else if outline == 1 {
            cv.draw_triangle(xa, ya, xb, yb, xc, yc, b'#' as u32);
        }
        Ok(())
    }

    fn demo_render(&mut self) -> libcaca::Result<()> {
        let i = self.render_frame;
        self.render_frame += 1;

        // NB: kept on the heap, not the stack — a 256 KiB array overflows
        // the default 1 MiB Windows main-thread stack in debug builds.
        let mut buffer = vec![0xff000000u32; 256 * 256];

        let draw_circle = |buffer: &mut [u32], xo: i32, yo: i32, r: i32, mask: u32, val: u32| {
            let mut t = 0i32;
            let mut dx = 0i32;
            let mut dy = r;
            let mut point = |x: i32, y: i32| {
                if (0..256).contains(&x) && (0..256).contains(&y) {
                    let idx = (x + 256 * y) as usize;
                    buffer[idx] = 0xff000000 | (buffer[idx] & !mask) | val;
                }
            };
            while dx <= dy {
                point(xo - dx / 3, yo - dy / 3);
                point(xo + dx / 3, yo - dy / 3);
                point(xo - dx / 3, yo + dy / 3);
                point(xo + dx / 3, yo + dy / 3);
                point(xo - dy / 3, yo - dx / 3);
                point(xo + dy / 3, yo - dx / 3);
                point(xo - dy / 3, yo + dx / 3);
                point(xo + dy / 3, yo + dx / 3);
                if t > 0 {
                    t += dx - dy;
                    dy -= 1;
                } else {
                    t += dx;
                }
                dx += 1;
            }
        };

        let xo = (128.0 + 48.0 * (0.02 * i as f64).sin()) as i32;
        let yo = (128.0 + 48.0 * (0.03 * i as f64).cos()) as i32;
        for z in 0..240 {
            draw_circle(&mut buffer, xo, yo, z, 0x00ff0000, 200 << 16);
        }
        let xo = (128.0 + 48.0 * (2.0 + 0.06 * i as f64).sin()) as i32;
        let yo = (128.0 + 48.0 * (2.0 + 0.05 * i as f64).cos()) as i32;
        for z in 0..240 {
            draw_circle(&mut buffer, xo, yo, z, 0x0000ff00, 200 << 8);
        }
        let xo = (128.0 + 48.0 * (1.0 + 0.04 * i as f64).sin()) as i32;
        let yo = (128.0 + 48.0 * (1.0 + 0.03 * i as f64).cos()) as i32;
        for z in 0..240 {
            draw_circle(&mut buffer, xo, yo, z, 0x000000ff, 200);
        }

        let mut pixels = Vec::with_capacity(256 * 256 * 4);
        for p in buffer {
            pixels.extend_from_slice(&p.to_le_bytes());
        }

        let mut dither = Dither::new(
            256,
            256,
            32,
            4 * 256,
            0x00ff0000,
            0x0000ff00,
            0x000000ff,
            0xff000000,
        )?;
        dither.set_gamma(-1.0)?;
        let cv = self.cv();
        let (w, h) = (cv.width().min(256), cv.height().min(256));
        dither.dither_bitmap(cv, 0, 0, w, h, &pixels)?;
        Ok(())
    }

    fn run_demo(&mut self, demo: Demo) -> libcaca::Result<()> {
        match demo {
            Demo::All => self.demo_all(),
            Demo::Dots => self.demo_dots(),
            Demo::Lines => self.demo_lines(),
            Demo::Boxes => self.demo_boxes(),
            Demo::Triangles => self.demo_triangles(),
            Demo::Ellipses => self.demo_ellipses(),
            Demo::Render => self.demo_render(),
        }
    }
}

fn main() -> libcaca::Result<()> {
    let mut app = App {
        display: Display::new(Canvas::new(80, 24)?)?,
        bounds: 0,
        outline: 0,
        demo: None,
        frame: 0,
        render_frame: 0,
    };

    app.display.set_display_time(40_000)?;
    let _ = app.display.set_mouse(false);

    app.display_menu()?;
    app.display.refresh()?;

    let interactive = matches!(app.display.driver(), Driver::Terminal | Driver::Win32);
    let mut headless_demos: Vec<Demo> = Vec::new();
    if !interactive {
        headless_demos = vec![
            Demo::All,
            Demo::Dots,
            Demo::Lines,
            Demo::Boxes,
            Demo::Triangles,
            Demo::Ellipses,
            Demo::Render,
        ];
    }

    let mut quit = false;
    while !quit {
        let mut menu = false;
        let mut mouse = false;
        let (mut xmouse, mut ymouse) = (0i32, 0i32);

        while let Some(ev) = app.display.get_event(EventMask::ANY, 0) {
            match ev {
                Event::KeyPress(k) => {
                    if app.demo.is_some() {
                        menu = true;
                        app.demo = None;
                    } else {
                        match k.ch {
                            c if c == b'q' as i32 || c == b'Q' as i32 || c == key::ESCAPE => {
                                app.demo = None;
                                quit = true;
                            }
                            c if c == b'o' as i32 || c == b'O' as i32 => {
                                app.outline = (app.outline + 1) % 3;
                                app.display_menu()?;
                                app.display.refresh()?;
                            }
                            c if c == b'b' as i32 || c == b'B' as i32 => {
                                app.bounds = (app.bounds + 1) % 2;
                                app.display_menu()?;
                                app.display.refresh()?;
                            }
                            c if c == b'f' as i32 || c == b'F' as i32 => app.demo = Some(Demo::All),
                            c if c == b'1' as i32 => app.demo = Some(Demo::Dots),
                            c if c == b'2' as i32 => app.demo = Some(Demo::Lines),
                            c if c == b'3' as i32 => app.demo = Some(Demo::Boxes),
                            c if c == b'4' as i32 => app.demo = Some(Demo::Triangles),
                            c if c == b'5' as i32 => app.demo = Some(Demo::Ellipses),
                            c if c == b'r' as i32 || c == b'R' as i32 => {
                                app.demo = Some(Demo::Render)
                            }
                            _ => {}
                        }
                        if app.demo.is_some() {
                            let cv = app.display.canvas_mut();
                            cv.set_color_ansi(Color::LightGray, Color::Black)?;
                            cv.clear();
                        }
                    }
                }
                Event::MouseMotion { x, y } => {
                    mouse = true;
                    xmouse = x;
                    ymouse = y;
                }
                Event::Resize { .. } => mouse = true,
                _ => {}
            }
        }

        if menu || (mouse && app.demo.is_none()) {
            app.display_menu()?;
            if mouse && app.demo.is_none() {
                let cv = app.display.canvas_mut();
                cv.set_color_ansi(Color::Red, Color::Black)?;
                cv.put_str(xmouse, ymouse, ".");
                cv.put_str(xmouse, ymouse + 1, "|\\");
            }
            app.display.refresh()?;
        } else if let Some(demo) = app.demo {
            app.run_demo(demo)?;
            let dt = app.display.display_time().max(1);
            {
                let cv = app.display.canvas_mut();
                cv.set_color_ansi(Color::LightGray, Color::Black)?;
                let (w, h) = (cv.width(), cv.height());
                cv.draw_thin_box(1, 1, w - 2, h - 2);
                cv.printf(
                    4,
                    1,
                    format_args!("[{}.{} fps]----", 1000000 / dt, (10000000 / dt) % 10),
                );
            }
            app.display.refresh()?;
        }

        if !interactive {
            if headless_demos.is_empty() {
                break;
            }
            let demo = headless_demos.remove(0);
            app.demo = Some(demo);
            // Render a few frames of each demo, then move on.
            for _ in 0..3 {
                app.run_demo(demo)?;
                app.display.refresh()?;
            }
            app.demo = None;
        }
    }

    Ok(())
}
