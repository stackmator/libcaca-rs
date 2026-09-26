//! Shared stdin raw-mode input for interactive drivers.
//!
//! [`RawInput`] puts the terminal in raw mode, pumps stdin bytes on a
//! background thread and decodes them with [`EventParser`](super::event::EventParser).
//! Both the ANSI terminal and the DOS-emulation drivers build on it.

use std::io;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

use crate::display::event::{Event, EventParser};

/// Raw stdin reader shared by interactive backends.
pub(crate) struct RawInput {
    _guard: rawmode::RawGuard,
    rx: Option<Receiver<u8>>,
    parser: EventParser,
}

impl RawInput {
    /// Enable raw mode. Fails without an interactive terminal.
    pub fn new() -> io::Result<RawInput> {
        Ok(RawInput {
            _guard: rawmode::RawGuard::new()?,
            rx: None,
            parser: EventParser::new(),
        })
    }

    /// Spawn the stdin reader thread.
    pub fn start(&mut self) {
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
        self.rx = Some(rx);
    }

    /// Stop the reader (the raw-mode guard restores the terminal on drop).
    pub fn stop(&mut self) {
        self.rx = None;
    }

    /// Whether the reader thread is running.
    pub fn is_running(&self) -> bool {
        self.rx.is_some()
    }

    /// Extract the next complete parsed event, if any.
    pub fn next_event(&mut self) -> Option<Event> {
        self.parser.next_event()
    }

    /// Feed raw bytes to the parser.
    pub fn push_bytes(&mut self, bytes: &[u8]) {
        self.parser.push(bytes);
    }

    /// Flush a pending lone ESC as an Escape key press.
    pub fn flush_escape(&mut self) -> Option<Event> {
        self.parser.flush_escape()
    }

    /// Read one stdin byte, waiting up to `timeout` (`None` blocks).
    /// Returns `None` when no reader is running or no byte arrived in time.
    pub fn read_byte(&self, timeout: Option<Duration>) -> Option<u8> {
        let rx = self.rx.as_ref()?;
        match timeout {
            Some(t) => rx.recv_timeout(t).ok(),
            None => rx.recv().ok(),
        }
    }

    /// The current terminal size in cells.
    pub fn terminal_size() -> (i32, i32) {
        rawmode::terminal_size()
    }
}

#[cfg(unix)]
pub(crate) mod rawmode {
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
pub(crate) mod rawmode {
    use std::io;

    use windows_sys::Win32::System::Console::{
        GetConsoleMode, GetConsoleScreenBufferInfo, GetStdHandle, SetConsoleMode,
        SetConsoleOutputCP, CONSOLE_SCREEN_BUFFER_INFO, ENABLE_ECHO_INPUT, ENABLE_LINE_INPUT,
        ENABLE_PROCESSED_INPUT, ENABLE_VIRTUAL_TERMINAL_INPUT, ENABLE_VIRTUAL_TERMINAL_PROCESSING,
        STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
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
pub(crate) mod rawmode {
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
