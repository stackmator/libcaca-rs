//! Port of libcaca's `examples/input.c`.
//!
//! Five editable single-line text entries with full Unicode support.
//! Tab/Return cycles, arrows/Home/End move, Delete/Backspace edit,
//! Escape quits. Headless runs exit immediately.

use libcaca::charset::utf8_to_utf32;
use libcaca::{key, Canvas, Color, Display, Driver, Event, EventMask, CACA_MAGIC_FULLWIDTH};

const BUFFER_SIZE: usize = 75;
const TEXT_ENTRIES: usize = 5;

struct TextEntry {
    buffer: Vec<u32>,
    cursor: usize,
    changed: bool,
}

fn main() -> libcaca::Result<()> {
    let mut dp = Display::new(Canvas::new(0, 0)?)?;
    let _ = dp.set_cursor(true);

    dp.canvas_mut().set_color_ansi(Color::White, Color::Blue)?;
    dp.canvas_mut()
        .put_str(1, 1, "Text entries - press tab to cycle");

    let mut entries: Vec<TextEntry> = (0..TEXT_ENTRIES)
        .map(|_| TextEntry {
            buffer: Vec::new(),
            cursor: 0,
            changed: true,
        })
        .collect();

    for i in 0..TEXT_ENTRIES {
        dp.canvas_mut()
            .printf(3, 3 * i as i32 + 4, format_args!("[entry {}]", i + 1));
    }

    // Seed the last entry with assorted Unicode, like the C version.
    {
        let last = &mut entries[TEXT_ENTRIES - 1];
        last.buffer = vec![
            b'A' as u32,
            b'b' as u32,
            utf8_to_utf32("Ç".as_bytes()).0,
            utf8_to_utf32("đ".as_bytes()).0,
            utf8_to_utf32("ボ".as_bytes()).0,
            CACA_MAGIC_FULLWIDTH,
            utf8_to_utf32("♥".as_bytes()).0,
        ];
    }

    let interactive = matches!(dp.driver(), Driver::Terminal | Driver::Win32);
    if !interactive {
        dp.push_event(Event::KeyPress(libcaca::KeyEvent::new(key::ESCAPE, 0)));
    }

    let mut e = 0usize;
    let mut running = true;
    while running {
        for (i, entry) in entries.iter_mut().enumerate() {
            if !entry.changed {
                continue;
            }
            let cv = dp.canvas_mut();
            cv.set_color_ansi(Color::Black, Color::LightGray)?;
            cv.fill_box(2, 3 * i as i32 + 5, BUFFER_SIZE as i32 + 1, 1, b' ' as u32);
            for (j, &ch) in entry.buffer.iter().enumerate() {
                cv.put_char(2 + j as i32, 3 * i as i32 + 5, ch);
            }
            entry.changed = false;
        }

        {
            let cursor = entries[e].cursor;
            dp.canvas_mut().gotoxy(2 + cursor as i32, 3 * e as i32 + 5);
        }
        dp.refresh()?;

        let ev = match dp.get_event(EventMask::KEY_PRESS, -1) {
            Some(ev) => ev,
            None => {
                if interactive {
                    continue;
                }
                break;
            }
        };
        let ch = ev.key().map(|k| k.ch).unwrap_or(0);

        if ch == key::ESCAPE {
            running = false;
        } else if ch == key::TAB || ch == key::RETURN {
            e = (e + 1) % TEXT_ENTRIES;
        } else if ch == key::HOME {
            entries[e].cursor = 0;
        } else if ch == key::END {
            entries[e].cursor = entries[e].buffer.len();
        } else if ch == key::LEFT {
            if entries[e].cursor > 0 {
                entries[e].cursor -= 1;
            }
        } else if ch == key::RIGHT {
            if entries[e].cursor < entries[e].buffer.len() {
                entries[e].cursor += 1;
            }
        } else if ch == key::DELETE {
            if entries[e].cursor < entries[e].buffer.len() {
                let cursor = entries[e].cursor;
                entries[e].buffer.remove(cursor);
                entries[e].changed = true;
            }
        } else if ch == key::BACKSPACE {
            if entries[e].cursor > 0 {
                entries[e].cursor -= 1;
                let cursor = entries[e].cursor;
                entries[e].buffer.remove(cursor);
                entries[e].changed = true;
            }
        } else if entries[e].buffer.len() < BUFFER_SIZE {
            let entry = &mut entries[e];
            let utf32 = ev.key().map(|k| k.utf32).unwrap_or(0);
            entry.buffer.insert(entry.cursor, utf32);
            entry.cursor += 1;
            entry.changed = true;
        }

        if !interactive {
            break;
        }
    }

    Ok(())
}
