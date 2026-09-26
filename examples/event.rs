//! Port of libcaca's `examples/event.c`.
//!
//! Shows the most recent input event at the top of the screen and a history
//! below it. Type "quit" (or close the window) to exit. When stdout is not a
//! terminal it renders a single frame with a synthetic event and exits.

use libcaca::{Canvas, Color, Display, Driver, Event, EventMask};

fn print_event(cv: &mut Canvas, x: i32, y: i32, ev: &Event) {
    match *ev {
        Event::None => {
            cv.printf(x, y, format_args!("CACA_EVENT_NONE"));
        }
        Event::KeyPress(k) => {
            let c = printable(k.ch);
            cv.printf(
                x,
                y,
                format_args!("CACA_EVENT_KEY_PRESS 0x{:02x} ({})", k.ch, c),
            );
        }
        Event::KeyRelease(k) => {
            let c = printable(k.ch);
            cv.printf(
                x,
                y,
                format_args!("CACA_EVENT_KEY_RELEASE 0x{:02x} ({})", k.ch, c),
            );
        }
        Event::MouseMotion { x: mx, y: my } => {
            cv.printf(x, y, format_args!("CACA_EVENT_MOUSE_MOTION {} {}", mx, my));
        }
        Event::MousePress { button, .. } => {
            cv.printf(x, y, format_args!("CACA_EVENT_MOUSE_PRESS {}", button));
        }
        Event::MouseRelease { button, .. } => {
            cv.printf(x, y, format_args!("CACA_EVENT_MOUSE_RELEASE {}", button));
        }
        Event::Resize { w, h } => {
            cv.printf(x, y, format_args!("CACA_EVENT_RESIZE {} {}", w, h));
        }
        Event::Quit => {
            cv.printf(x, y, format_args!("CACA_EVENT_QUIT"));
        }
    }
}

fn printable(ch: i32) -> char {
    if (0x20..0x80).contains(&ch) {
        char::from_u32(ch as u32).unwrap_or('?')
    } else {
        '?'
    }
}

fn main() -> libcaca::Result<()> {
    let mut dp = Display::new(Canvas::new(80, 24)?)?;

    let h = dp.canvas().height() - 1;
    {
        let cv = dp.canvas_mut();
        let w = cv.width();
        cv.set_color_ansi(Color::White, Color::Blue)?;
        cv.draw_line(0, 0, w - 1, 0, b' ' as u32);
        cv.draw_line(0, h, w - 1, h, b' ' as u32);
        cv.put_str(0, h, "type \"quit\" to exit");
    }
    dp.refresh()?;

    let interactive = matches!(dp.driver(), Driver::Terminal | Driver::Win32);
    if !interactive {
        dp.push_event(Event::Quit);
    }

    let mut events = vec![Event::None; h.max(0) as usize];
    let quit_string = ["", "q", "qu", "qui", "quit"];
    let mut quit = 0usize;

    loop {
        // Block for the next event (or take the synthetic one headlessly).
        let first = match dp.get_event(EventMask::ANY, -1) {
            Some(ev) => ev,
            None => {
                if interactive {
                    continue;
                }
                break;
            }
        };

        // Drain any further pending events like the C version does.
        let mut pending = vec![first];
        while let Some(ev) = dp.get_event(EventMask::ANY, 0) {
            pending.push(ev);
        }

        for ev in &pending {
            if ev.event_type() == EventMask::KEY_PRESS {
                if let Some(k) = ev.key() {
                    let key = k.ch;
                    if (key == b'q' as i32 && quit == 0)
                        || (key == b'u' as i32 && quit == 1)
                        || (key == b'i' as i32 && quit == 2)
                        || (key == b't' as i32 && quit == 3)
                    {
                        quit += 1;
                    } else if key == b'q' as i32 {
                        quit = 1;
                    } else {
                        quit = 0;
                    }
                }
            }

            events.pop();
            events.insert(0, *ev);

            if *ev == Event::Quit {
                quit = 4;
            }
        }

        {
            let cv = dp.canvas_mut();
            let (w, h) = (cv.width(), cv.height() - 1);
            cv.set_color_ansi(Color::LightGray, Color::Black)?;
            cv.clear();

            cv.set_color_ansi(Color::White, Color::Blue)?;
            cv.draw_line(0, 0, w - 1, 0, b' ' as u32);
            print_event(cv, 0, 0, &events[0]);

            cv.draw_line(0, h, w - 1, h, b' ' as u32);
            cv.printf(
                0,
                h,
                format_args!("type \"quit\" to exit: {}", quit_string[quit.min(4)]),
            );

            cv.set_color_ansi(Color::White, Color::Black)?;
            let mut i = 1;
            while i < h && events[i as usize] != Event::None {
                print_event(cv, 0, i, &events[i as usize]);
                i += 1;
            }
        }

        dp.refresh()?;

        if quit >= 4 || !interactive {
            break;
        }
    }

    Ok(())
}
