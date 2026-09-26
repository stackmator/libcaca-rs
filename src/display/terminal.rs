//! ANSI/VT terminal driver.
//!
//! This is the pure-Rust equivalent of libcaca's terminal display drivers
//! (ncurses/S-Lang/win32): it puts the terminal in raw mode, renders the
//! canvas with ANSI escape sequences and decodes keyboard/mouse input into
//! [`Event`]s.

use std::io::Write;
use std::time::{Duration, Instant};

use crate::canvas::Canvas;
use crate::display::event::{Event, EventMask};
use crate::display::input::RawInput;
use crate::display::render::{self, AnsiState};

/// Terminal driver state.
pub struct Terminal {
    input: RawInput,
    render_state: AnsiState,
    last_size: (i32, i32),
    cursor_visible: bool,
    mouse_visible: bool,
}

impl Terminal {
    /// Create the driver and put the terminal into raw mode.
    pub fn new() -> std::io::Result<Terminal> {
        let input = RawInput::new()?;
        let last_size = RawInput::terminal_size();
        Ok(Terminal {
            input,
            render_state: AnsiState::default(),
            last_size,
            cursor_visible: true,
            mouse_visible: false,
        })
    }

    /// Enter the alternate screen and start the input thread.
    pub fn init(&mut self) {
        let mut out = std::io::stdout();
        let _ = out.write_all(render::ENTER_SCREEN);
        let _ = out.flush();

        self.input.start();
        self.last_size = RawInput::terminal_size();
    }

    /// Leave the alternate screen and restore the terminal.
    pub fn end(&mut self) {
        let mut out = std::io::stdout();
        let _ = out.write_all(render::LEAVE_SCREEN);
        let _ = out.flush();
        self.input.stop();
    }

    /// The terminal width in cells.
    pub fn display_width(&self) -> i32 {
        RawInput::terminal_size().0
    }

    /// The terminal height in cells.
    pub fn display_height(&self) -> i32 {
        RawInput::terminal_size().1
    }

    /// Set the terminal title.
    pub fn set_title(&mut self, title: &str) {
        let mut out = std::io::stdout();
        let _ = write!(out, "\x1b]0;{}\x07", title);
        let _ = out.flush();
    }

    /// Render the canvas to the terminal.
    pub fn display(&mut self, canvas: &Canvas) {
        let mut buf = Vec::with_capacity((canvas.width() * canvas.height()) as usize * 8 + 64);
        render::render(canvas, &mut self.render_state, &mut buf);
        let mut out = std::io::stdout();
        let _ = out.write_all(&buf);
        let _ = out.flush();
    }

    /// Synchronise the current terminal size into `last_size`.
    pub fn handle_resize(&mut self) {
        self.last_size = RawInput::terminal_size();
    }

    /// Show or hide the cursor.
    pub fn set_cursor(&mut self, show: bool) {
        self.cursor_visible = show;
        let mut out = std::io::stdout();
        let _ = out.write_all(if show { b"\x1b[?25h" } else { b"\x1b[?25l" });
        let _ = out.flush();
    }

    /// Show or hide the mouse pointer / enable mouse reporting.
    pub fn set_mouse(&mut self, show: bool) {
        self.mouse_visible = show;
        let mut out = std::io::stdout();
        if show {
            let _ = out.write_all(b"\x1b[?1000h\x1b[?1006h");
        } else {
            let _ = out.write_all(b"\x1b[?1006l\x1b[?1000l");
        }
        let _ = out.flush();
    }

    fn read_byte(&self, timeout: Option<Duration>) -> Option<u8> {
        self.input.read_byte(timeout)
    }

    /// Wait for an event matching `mask`.
    ///
    /// `timeout_us` is negative for blocking behaviour, zero for a poll.
    pub fn get_event(&mut self, mask: EventMask, timeout_us: i64) -> Option<Event> {
        if mask.bits() == 0 {
            return None;
        }

        let deadline = if timeout_us >= 0 {
            Some(Instant::now() + Duration::from_micros(timeout_us as u64))
        } else {
            None
        };

        loop {
            if let Some(ev) = self.input.next_event() {
                if ev.matches(mask) {
                    return Some(ev);
                }
                continue;
            }

            // Detect a terminal resize.
            let size = RawInput::terminal_size();
            if size != self.last_size && size.0 > 0 && size.1 > 0 {
                self.last_size = size;
                let ev = Event::Resize {
                    w: size.0,
                    h: size.1,
                };
                if ev.matches(mask) {
                    return Some(ev);
                }
                continue;
            }

            let remaining = match deadline {
                Some(d) => {
                    let now = Instant::now();
                    if now >= d {
                        if let Some(ev) = self.input.flush_escape() {
                            if ev.matches(mask) {
                                return Some(ev);
                            }
                        }
                        return None;
                    }
                    Some(d - now)
                }
                None => None,
            };

            match self.read_byte(remaining) {
                Some(b) => self.input.push_bytes(&[b]),
                None => {
                    if let Some(ev) = self.input.flush_escape() {
                        if ev.matches(mask) {
                            return Some(ev);
                        }
                    }
                    if let Some(d) = deadline {
                        if Instant::now() >= d {
                            return None;
                        }
                    } else if !self.input.is_running() {
                        return None;
                    }
                }
            }
        }
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        if self.mouse_visible {
            let _ = std::io::stdout().write_all(b"\x1b[?1006l\x1b[?1000l");
        }
        let _ = std::io::stdout().write_all(render::LEAVE_SCREEN);
        let _ = std::io::stdout().flush();
    }
}
