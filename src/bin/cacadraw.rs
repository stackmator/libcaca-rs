//! Port of libcaca's `src/cacadraw.c`: ANSI art file scroller.
//!
//! Shows `ansi` files passed on the command line, scrolling with the arrow
//! keys (`PageUp`/`PageDown` jump) and switching files with `n`/`p`.
//! `q` or Escape quits. Only `ansi` files can be displayed, like the C
//! version.

use std::path::Path;

use libcaca::{key, Canvas, Color, Display, Event, EventMask};

fn refresh_screen(dp: &mut Display, image: &Canvas, x: i32, y: i32) {
    let cv = dp.canvas_mut();
    let _ = cv.set_color_ansi(Color::Default, Color::Default);
    cv.clear();
    let _ = cv.blit(-x, -y, image, None);
    let _ = dp.refresh();
}

fn main() {
    let argv: Vec<String> = std::env::args().collect();
    if argv.len() < 2 {
        eprintln!("{}: missing argument (filename).", argv[0]);
        std::process::exit(1);
    }

    let mut dp = match Canvas::new(0, 0)
        .map_err(|_| ())
        .and_then(|cv| Display::new(cv).map_err(|_| ()))
    {
        Ok(dp) => dp,
        Err(()) => std::process::exit(1),
    };

    let mut image: Option<Canvas> = None;
    let (mut x, mut y) = (0, 0);
    let mut file = 1usize;
    let mut refresh = true;

    loop {
        let (iw, ih) = match image.as_ref() {
            Some(image) => (image.width(), image.height()),
            None => (0, 0),
        };

        if image.is_none() {
            let mut img = Canvas::new(0, 0).unwrap();
            if img
                .import_from_file(Path::new(&argv[file]), "ansi")
                .is_err()
            {
                // The C version always names `argv[1]` here, even when a
                // later file fails; name the file that actually failed.
                eprintln!("{}: invalid file `{}`.", argv[0], argv[file]);
                std::process::exit(1);
            }
            x = 0;
            y = 0;
            let _ = dp.set_title(&argv[file]);
            image = Some(img);
            refresh = true;
            continue;
        }

        if refresh {
            refresh_screen(&mut dp, image.as_ref().unwrap(), x, y);
            refresh = false;
        }

        let mut dx = 0;
        let mut dy = 0;
        let mut quit = false;
        let mut stopevents = false;
        while let Some(ev) = dp.get_event(EventMask::ANY, -1) {
            match ev {
                Event::Quit => {
                    quit = true;
                    break;
                }
                Event::KeyPress(k) => match k.ch {
                    x if x == key::LEFT => dx -= 2,
                    x if x == key::RIGHT => dx += 2,
                    x if x == key::UP => dy -= 1,
                    x if x == key::DOWN => dy += 1,
                    x if x == key::PAGEUP => dy -= 12,
                    x if x == key::PAGEDOWN => dy += 12,
                    x if x == key::ESCAPE || x == b'q' as i32 => {
                        quit = true;
                        break;
                    }
                    x if x == b'n' as i32 => {
                        file = if file + 1 < argv.len() { file + 1 } else { 1 };
                        image = None;
                        stopevents = true;
                        break;
                    }
                    x if x == b'p' as i32 => {
                        file = if file > 1 { file - 1 } else { argv.len() - 1 };
                        image = None;
                        stopevents = true;
                        break;
                    }
                    _ => {}
                },
                Event::Resize { .. } => {
                    refresh = true;
                    stopevents = true;
                    break;
                }
                _ => {}
            }
        }
        if quit {
            break;
        }
        if stopevents {
            continue;
        }

        let (w, h) = (dp.canvas().width(), dp.canvas().height());

        if dx != 0 || dy != 0 {
            refresh = true;
            x += dx;
            y += dy;

            if x < 0 {
                x = 0;
            } else if x + w > iw {
                x = if iw > w { iw - w } else { 0 };
            }
            if y < 0 {
                y = 0;
            } else if y + h > ih {
                y = if ih > h { ih - h } else { 0 };
            }
        }
    }
}
