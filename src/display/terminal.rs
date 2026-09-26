//! ANSI/VT terminal driver.
//!
//! This is the pure-Rust equivalent of libcaca's terminal display drivers
//! (ncurses/S-Lang/win32): it puts the terminal in raw mode, renders the
//! canvas with ANSI escape sequences and decodes keyboard/mouse input into
//! [`Event`]s.

use std::io::Write;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use crate::canvas::Canvas;
use crate::display::event::{Event, EventMask, EventParser};
use crate::display::render::{self, AnsiState};

/// Terminal driver state.
pub struct Terminal {
    raw: rawmode::RawGuard,
    input_rx: Option<Receiver<u8>>,
    parser: EventParser,
    render_state: AnsiState,
    last_size: (i32, i32),
    cursor_visible: bool,
    mouse_visible: bool,
}

impl Terminal {
    /// Create the driver and put the terminal into raw mode.
    pub fn new() -> std::io::Result<Terminal> {
        let raw = rawmode::RawGuard::new()?;
        Ok(Terminal {
            raw,
            input_rx: None,
            parser: EventParser::new(),
            render_state: AnsiState::default(),
            last_size: rawmode::terminal_size(),
            cursor_visible: true,
            mouse_visible: false,
        })
    }

    /// Enter the alternate screen and start the input thread.
    pub fn init(&mut self) {
        let mut out = std::io::stdout();
        let _ = out.write_all(render::ENTER_SCREEN);
        let _ = out.flush();

        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            use std::io::Read;
            let mut stdin = std::io::stdin();
            let mut buf = [0u8; 1];
            loop {
                match stdin.read(&mut buf) {
                    Ok(0) => break,
                    Ok(_) => {
                        if tx.send(buf[0]).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });
        self.input_rx = Some(rx);
        self.last_size = rawmode::terminal_size();
    }

    /// Leave the alternate screen and restore the terminal.
    pub fn end(&mut self) {
        let mut out = std::io::stdout();
        let _ = out.write_all(render::LEAVE_SCREEN);
        let _ = out.flush();
        self.input_rx = None;
        // `self.raw` is restored on drop; keep it until then.
        let _ = &self.raw;
    }

    /// The terminal width in cells.
    pub fn display_width(&self) -> i32 {
        rawmode::terminal_size().0
    }

    /// The terminal height in cells.
    pub fn display_height(&self) -> i32 {
        rawmode::terminal_size().1
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
        self.last_size = rawmode::terminal_size();
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
        let rx = self.input_rx.as_ref()?;
        match timeout {
            Some(t) => rx.recv_timeout(t).ok(),
            None => rx.recv().ok(),
        }
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
            if let Some(ev) = self.parser.next_event() {
                if ev.matches(mask) {
                    return Some(ev);
                }
                continue;
            }

            // Detect a terminal resize.
            let size = rawmode::terminal_size();
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
                        if let Some(ev) = self.parser.flush_escape() {
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
                Some(b) => self.parser.push(&[b]),
                None => {
                    if let Some(ev) = self.parser.flush_escape() {
                        if ev.matches(mask) {
                            return Some(ev);
                        }
                    }
                    if let Some(d) = deadline {
                        if Instant::now() >= d {
                            return None;
                        }
                    } else if self.input_rx.is_none() {
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
        let _ = &self.raw;
    }
}

#[cfg(unix)]
mod rawmode {
    use std::io;

    pub struct RawGuard {
        saved: libc::termios,
    }

    impl RawGuard {
        pub fn new() -> io::Result<RawGuard> {
            unsafe {
                let fd = libc::STDIN_FILENO;
                let mut t: libc::termios = std::mem::zeroed();
                if libc::tcgetattr(fd, &mut t) != 0 {
                    return Err(io::Error::last_os_error());
                }
                let saved = t;

                libc::cfmakeraw(&mut t);
                t.c_cc[libc::VMIN] = 1;
                t.c_cc[libc::VTIME] = 0;

                if libc::tcsetattr(fd, libc::TCSANOW, &t) != 0 {
                    return Err(io::Error::last_os_error());
                }
                Ok(RawGuard { saved })
            }
        }
    }

    impl Drop for RawGuard {
        fn drop(&mut self) {
            unsafe {
                libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &self.saved);
            }
        }
    }

    pub fn terminal_size() -> (i32, i32) {
        unsafe {
            let mut ws: libc::winsize = std::mem::zeroed();
            if libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut ws) == 0
                && ws.ws_col > 0
                && ws.ws_row > 0
            {
                return (ws.ws_col as i32, ws.ws_row as i32);
            }
            // Fall back to environment variables.
            let cols = std::env::var("COLUMNS")
                .ok()
                .and_then(|s| s.parse::<i32>().ok());
            let rows = std::env::var("LINES")
                .ok()
                .and_then(|s| s.parse::<i32>().ok());
            if let (Some(c), Some(r)) = (cols, rows) {
                if c > 0 && r > 0 {
                    return (c, r);
                }
            }
            (80, 24)
        }
    }
}

#[cfg(windows)]
mod rawmode {
    use std::io;

    use windows_sys::Win32::System::Console::{
        GetConsoleMode, GetConsoleScreenBufferInfo, GetStdHandle, SetConsoleMode,
        SetConsoleOutputCP, CONSOLE_SCREEN_BUFFER_INFO, ENABLE_ECHO_INPUT,
        ENABLE_LINE_INPUT, ENABLE_PROCESSED_INPUT, ENABLE_VIRTUAL_TERMINAL_INPUT,
        ENABLE_VIRTUAL_TERMINAL_PROCESSING, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
    };

    pub struct RawGuard {
        in_handle: isize,
        in_mode: u32,
        out_handle: isize,
        out_mode: u32,
    }

    impl RawGuard {
        pub fn new() -> io::Result<RawGuard> {
            unsafe {
                let in_handle = GetStdHandle(STD_INPUT_HANDLE);
                let out_handle = GetStdHandle(STD_OUTPUT_HANDLE);

                let mut in_mode: u32 = 0;
                let mut out_mode: u32 = 0;
                if GetConsoleMode(in_handle, &mut in_mode) == 0
                    || GetConsoleMode(out_handle, &mut out_mode) == 0
                {
                    return Err(io::Error::last_os_error());
                }

                let new_in = (in_mode | ENABLE_VIRTUAL_TERMINAL_INPUT)
                    & !(ENABLE_LINE_INPUT | ENABLE_ECHO_INPUT | ENABLE_PROCESSED_INPUT);
                let new_out = out_mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING;

                SetConsoleMode(in_handle, new_in);
                SetConsoleMode(out_handle, new_out);
                SetConsoleOutputCP(65001);

                Ok(RawGuard {
                    in_handle: in_handle as isize,
                    in_mode,
                    out_handle: out_handle as isize,
                    out_mode,
                })
            }
        }
    }

    impl Drop for RawGuard {
        fn drop(&mut self) {
            unsafe {
                SetConsoleMode(self.in_handle as _, self.in_mode);
                SetConsoleMode(self.out_handle as _, self.out_mode);
            }
        }
    }

    pub fn terminal_size() -> (i32, i32) {
        unsafe {
            let out_handle = GetStdHandle(STD_OUTPUT_HANDLE);
            let mut info: CONSOLE_SCREEN_BUFFER_INFO = std::mem::zeroed();
            if GetConsoleScreenBufferInfo(out_handle, &mut info) != 0 {
                let w = (info.srWindow.Right - info.srWindow.Left + 1) as i32;
                let h = (info.srWindow.Bottom - info.srWindow.Top + 1) as i32;
                if w > 0 && h > 0 {
                    return (w, h);
                }
            }
            (80, 24)
        }
    }
}

#[cfg(not(any(unix, windows)))]
mod rawmode {
    use std::io;

    pub struct RawGuard;

    impl RawGuard {
        pub fn new() -> io::Result<RawGuard> {
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "no raw mode support on this platform",
            ))
        }
    }

    pub fn terminal_size() -> (i32, i32) {
        (80, 24)
    }
}
