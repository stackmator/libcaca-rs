//! Native Win32 console driver.
//!
//! Port of `caca/driver/win32.c`. Rather than emitting ANSI escape sequences,
//! this driver writes a `CHAR_INFO` screen buffer with `WriteConsoleOutputW`
//! and reads `INPUT_RECORD`s, giving full Unicode and mouse support on the
//! Windows console. It is only compiled on Windows.

#![cfg(windows)]

use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{
    CloseHandle, GENERIC_READ, GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows_sys::Win32::System::Console::{
    AllocConsole, CreateConsoleScreenBuffer, FreeConsole, GetConsoleCursorInfo, GetConsoleMode,
    GetConsoleScreenBufferInfo, GetConsoleWindow, GetCurrentConsoleFont,
    GetNumberOfConsoleInputEvents, GetStdHandle, ReadConsoleInputW, SetConsoleActiveScreenBuffer,
    SetConsoleCursorInfo, SetConsoleMode, SetConsoleScreenBufferSize, SetConsoleTitleW,
    SetConsoleWindowInfo, WriteConsoleOutputW, BACKGROUND_BLUE, BACKGROUND_GREEN,
    BACKGROUND_INTENSITY, BACKGROUND_RED, CHAR_INFO, CHAR_INFO_0, CONSOLE_CURSOR_INFO,
    CONSOLE_FONT_INFO, CONSOLE_SCREEN_BUFFER_INFO, CONSOLE_TEXTMODE_BUFFER, COORD,
    ENABLE_MOUSE_INPUT, ENABLE_WINDOW_INPUT, FOREGROUND_BLUE, FOREGROUND_GREEN,
    FOREGROUND_INTENSITY, FOREGROUND_RED, INPUT_RECORD, KEY_EVENT, MOUSE_EVENT, MOUSE_HWHEELED,
    MOUSE_MOVED, MOUSE_WHEELED, STD_INPUT_HANDLE, WINDOW_BUFFER_SIZE_EVENT,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    VK_ADD, VK_DECIMAL, VK_DELETE, VK_DIVIDE, VK_DOWN, VK_END, VK_ESCAPE, VK_F1, VK_F10, VK_F11,
    VK_F12, VK_F13, VK_F14, VK_F15, VK_F2, VK_F3, VK_F4, VK_F5, VK_F6, VK_F7, VK_F8, VK_F9,
    VK_HOME, VK_INSERT, VK_LEFT, VK_MULTIPLY, VK_NEXT, VK_NUMPAD0, VK_NUMPAD1, VK_NUMPAD2,
    VK_NUMPAD3, VK_NUMPAD4, VK_NUMPAD5, VK_NUMPAD6, VK_NUMPAD7, VK_NUMPAD8, VK_NUMPAD9, VK_PRIOR,
    VK_RETURN, VK_RIGHT, VK_SEPARATOR, VK_SPACE, VK_SUBTRACT, VK_TAB, VK_UP,
};

use crate::attr::Attr;
use crate::canvas::{Canvas, CACA_MAGIC_FULLWIDTH};
use crate::display::event::{key, Event, EventMask, KeyEvent};
use crate::error::{CacaError, Result};

const FG_PALETTE: [u16; 16] = [
    0,
    FOREGROUND_BLUE,
    FOREGROUND_GREEN,
    FOREGROUND_GREEN | FOREGROUND_BLUE,
    FOREGROUND_RED,
    FOREGROUND_RED | FOREGROUND_BLUE,
    FOREGROUND_RED | FOREGROUND_GREEN,
    FOREGROUND_RED | FOREGROUND_GREEN | FOREGROUND_BLUE,
    FOREGROUND_INTENSITY,
    FOREGROUND_INTENSITY | FOREGROUND_BLUE,
    FOREGROUND_INTENSITY | FOREGROUND_GREEN,
    FOREGROUND_INTENSITY | FOREGROUND_GREEN | FOREGROUND_BLUE,
    FOREGROUND_INTENSITY | FOREGROUND_RED,
    FOREGROUND_INTENSITY | FOREGROUND_RED | FOREGROUND_BLUE,
    FOREGROUND_INTENSITY | FOREGROUND_RED | FOREGROUND_GREEN,
    FOREGROUND_INTENSITY | FOREGROUND_RED | FOREGROUND_GREEN | FOREGROUND_BLUE,
];

const BG_PALETTE: [u16; 16] = [
    0,
    BACKGROUND_BLUE,
    BACKGROUND_GREEN,
    BACKGROUND_GREEN | BACKGROUND_BLUE,
    BACKGROUND_RED,
    BACKGROUND_RED | BACKGROUND_BLUE,
    BACKGROUND_RED | BACKGROUND_GREEN,
    BACKGROUND_RED | BACKGROUND_GREEN | BACKGROUND_BLUE,
    BACKGROUND_INTENSITY,
    BACKGROUND_INTENSITY | BACKGROUND_BLUE,
    BACKGROUND_INTENSITY | BACKGROUND_GREEN,
    BACKGROUND_INTENSITY | BACKGROUND_GREEN | BACKGROUND_BLUE,
    BACKGROUND_INTENSITY | BACKGROUND_RED,
    BACKGROUND_INTENSITY | BACKGROUND_RED | BACKGROUND_BLUE,
    BACKGROUND_INTENSITY | BACKGROUND_RED | BACKGROUND_GREEN,
    BACKGROUND_INTENSITY | BACKGROUND_RED | BACKGROUND_GREEN | BACKGROUND_BLUE,
];

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Whether the process currently owns a console window.
pub fn has_console() -> bool {
    unsafe { !GetConsoleWindow().is_null() }
}

/// Win32 console driver state.
pub struct Win32 {
    hin: HANDLE,
    hout: HANDLE,
    screen: HANDLE,
    buffer: Vec<CHAR_INFO>,
    in_mode: u32,
    cursor_info: CONSOLE_CURSOR_INFO,
    mouse_state: u32,
    new_console: bool,
    active: bool,
}

impl Win32 {
    /// Initialise the driver and size `canvas` to the console window.
    pub fn new(canvas: &mut Canvas) -> Result<Win32> {
        let mut width = canvas.width();
        let mut height = canvas.height();

        let mut w = Win32 {
            hin: std::ptr::null_mut(),
            hout: std::ptr::null_mut(),
            screen: std::ptr::null_mut(),
            buffer: Vec::new(),
            in_mode: 0,
            cursor_info: unsafe { std::mem::zeroed() },
            mouse_state: 0,
            new_console: false,
            active: false,
        };

        unsafe {
            // May fail if we already have a console.
            w.new_console = AllocConsole() != 0;

            w.hin = GetStdHandle(STD_INPUT_HANDLE);

            let conout = wide("CONOUT$");
            w.hout = CreateFileW(
                conout.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null(),
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                std::ptr::null_mut(),
            );
            if w.hout == INVALID_HANDLE_VALUE {
                w.end();
                return Err(CacaError::Invalid);
            }

            GetConsoleCursorInfo(w.hout, &mut w.cursor_info);

            w.screen = CreateConsoleScreenBuffer(
                GENERIC_READ | GENERIC_WRITE,
                0,
                std::ptr::null(),
                CONSOLE_TEXTMODE_BUFFER,
                std::ptr::null(),
            );
            if w.screen == INVALID_HANDLE_VALUE || w.screen.is_null() {
                w.end();
                return Err(CacaError::Invalid);
            }

            let mut size = COORD {
                X: if width > 0 { width as i16 } else { 80 },
                Y: if height > 0 { height as i16 } else { 25 },
            };
            if width <= 0 && height <= 0 {
                let mut info: CONSOLE_SCREEN_BUFFER_INFO = std::mem::zeroed();
                if GetConsoleScreenBufferInfo(w.hout, &mut info) != 0 {
                    size = info.dwSize;
                }
            }

            SetConsoleScreenBufferSize(w.screen, size);
            let rect = windows_sys::Win32::System::Console::SMALL_RECT {
                Left: 0,
                Top: 0,
                Right: size.X - 1,
                Bottom: size.Y - 1,
            };
            SetConsoleWindowInfo(w.screen, 1, &rect);

            let mut csbi: CONSOLE_SCREEN_BUFFER_INFO = std::mem::zeroed();
            if GetConsoleScreenBufferInfo(w.screen, &mut csbi) == 0 {
                w.end();
                return Err(CacaError::Invalid);
            }
            width = (csbi.srWindow.Right - csbi.srWindow.Left + 1) as i32;
            height = (csbi.srWindow.Bottom - csbi.srWindow.Top + 1) as i32;
            if canvas.set_size(width, height).is_err() {
                w.end();
                return Err(CacaError::NoMem);
            }

            SetConsoleMode(w.screen, 0);

            // We want mouse and window resize events.
            GetConsoleMode(w.hin, &mut w.in_mode);
            SetConsoleMode(w.hin, ENABLE_MOUSE_INPUT | ENABLE_WINDOW_INPUT);

            let cci = CONSOLE_CURSOR_INFO {
                dwSize: 1,
                bVisible: 0,
            };
            SetConsoleCursorInfo(w.screen, &cci);

            SetConsoleActiveScreenBuffer(w.screen);

            w.buffer = vec![
                CHAR_INFO {
                    Char: CHAR_INFO_0 {
                        UnicodeChar: b' ' as u16
                    },
                    Attributes: 0,
                };
                (width.max(0) * height.max(0)) as usize
            ];
        }

        w.active = true;
        Ok(w)
    }

    /// Restore the original console state.
    pub fn end(&mut self) {
        if !self.active {
            return;
        }
        self.active = false;

        unsafe {
            if !self.screen.is_null() && self.screen != INVALID_HANDLE_VALUE {
                SetConsoleActiveScreenBuffer(self.hout);
                CloseHandle(self.screen);
            }
            if !self.hin.is_null() {
                SetConsoleMode(self.hin, self.in_mode);
            }
            if !self.hout.is_null() && self.hout != INVALID_HANDLE_VALUE {
                SetConsoleCursorInfo(self.hout, &self.cursor_info);
                CloseHandle(self.hout);
            }
            if self.new_console {
                FreeConsole();
            }
        }
    }

    /// The console title.
    pub fn set_title(&mut self, title: &str) {
        let t = wide(title);
        unsafe {
            SetConsoleTitleW(t.as_ptr());
        }
    }

    /// The console width in pixels.
    pub fn display_width(&self) -> i32 {
        let mut font_width = 6;
        unsafe {
            let mut info: CONSOLE_FONT_INFO = std::mem::zeroed();
            if GetCurrentConsoleFont(self.screen, 0, &mut info) != 0 {
                font_width = info.dwFontSize.X as i32;
            }
        }
        font_width * self.canvas_width()
    }

    /// The console height in pixels.
    pub fn display_height(&self) -> i32 {
        let mut font_height = 10;
        unsafe {
            let mut info: CONSOLE_FONT_INFO = std::mem::zeroed();
            if GetCurrentConsoleFont(self.screen, 0, &mut info) != 0 {
                font_height = info.dwFontSize.Y as i32;
            }
        }
        font_height * self.canvas_height()
    }

    fn canvas_width(&self) -> i32 {
        self.size().0
    }

    fn canvas_height(&self) -> i32 {
        self.size().1
    }

    /// The console window size in character cells.
    pub fn size(&self) -> (i32, i32) {
        unsafe {
            let mut info: CONSOLE_SCREEN_BUFFER_INFO = std::mem::zeroed();
            if GetConsoleScreenBufferInfo(self.screen, &mut info) != 0 {
                return (
                    (info.srWindow.Right - info.srWindow.Left + 1) as i32,
                    (info.srWindow.Bottom - info.srWindow.Top + 1) as i32,
                );
            }
        }
        (0, 0)
    }

    /// Render the canvas to the console screen buffer.
    pub fn display(&mut self, canvas: &Canvas) {
        let width = canvas.width();
        let height = canvas.height();
        if width <= 0 || height <= 0 {
            return;
        }

        let n = (width * height) as usize;
        if self.buffer.len() < n {
            self.buffer.resize(
                n,
                CHAR_INFO {
                    Char: CHAR_INFO_0 {
                        UnicodeChar: b' ' as u16,
                    },
                    Attributes: 0,
                },
            );
        }

        let chars = canvas.chars();
        let attrs = canvas.attrs();

        for i in 0..n {
            let ch = chars[i];
            let attr = Attr::from_raw(attrs[i]);
            let bgfg = attr.to_ansi();
            let fg = bgfg & 0x0f;
            let bg = bgfg >> 4;

            let glyph = if ch == CACA_MAGIC_FULLWIDTH {
                b' ' as u16
            } else if ch > 0x20 && ch < 0x10000 {
                ch as u16
            } else {
                b' ' as u16
            };

            self.buffer[i] = CHAR_INFO {
                Char: CHAR_INFO_0 { UnicodeChar: glyph },
                Attributes: FG_PALETTE[if fg < 0x10 { fg } else { 7 } as usize]
                    | BG_PALETTE[if bg < 0x10 { bg } else { 0 } as usize],
            };
        }

        let size = COORD {
            X: width as i16,
            Y: height as i16,
        };
        let pos = COORD { X: 0, Y: 0 };
        let mut rect = windows_sys::Win32::System::Console::SMALL_RECT {
            Left: 0,
            Top: 0,
            Right: (width - 1) as i16,
            Bottom: (height - 1) as i16,
        };

        unsafe {
            WriteConsoleOutputW(self.screen, self.buffer.as_ptr(), size, pos, &mut rect);
        }
    }

    /// Resize the backing buffer after a console resize.
    pub fn handle_resize(&mut self, canvas: &Canvas) {
        let n = (canvas.width().max(0) * canvas.height().max(0)) as usize;
        self.buffer.clear();
        self.buffer.resize(
            n,
            CHAR_INFO {
                Char: CHAR_INFO_0 {
                    UnicodeChar: b' ' as u16,
                },
                Attributes: 0,
            },
        );
    }

    /// Wait for and return the next console event matching `mask`.
    pub fn get_event(
        &mut self,
        canvas: &Canvas,
        mask: EventMask,
        timeout_us: i64,
    ) -> Option<Event> {
        if mask.bits() == 0 {
            return None;
        }

        let deadline = if timeout_us >= 0 {
            Some(Instant::now() + Duration::from_micros(timeout_us as u64))
        } else {
            None
        };

        loop {
            if let Some(ev) = self.poll_event(canvas) {
                if ev.matches(mask) {
                    return Some(ev);
                }
                continue;
            }

            if let Some(d) = deadline {
                let now = Instant::now();
                if now >= d {
                    return None;
                }
                std::thread::sleep(Duration::from_millis(2).min(d - now));
            } else {
                std::thread::sleep(Duration::from_millis(2));
            }
        }
    }

    fn poll_event(&mut self, canvas: &Canvas) -> Option<Event> {
        unsafe {
            let hin = GetStdHandle(STD_INPUT_HANDLE);
            if hin == INVALID_HANDLE_VALUE {
                return None;
            }

            let mut num: u32 = 0;
            GetNumberOfConsoleInputEvents(hin, &mut num);
            if num == 0 {
                return None;
            }

            let mut rec: INPUT_RECORD = std::mem::zeroed();
            let mut read: u32 = 0;
            if ReadConsoleInputW(hin, &mut rec, 1, &mut read) == 0 || read == 0 {
                return None;
            }

            match rec.EventType as u32 {
                KEY_EVENT => {
                    let k = rec.Event.KeyEvent;
                    let down = k.bKeyDown != 0;
                    let mut key = key_from_vk(k.wVirtualKeyCode);
                    let ch = (k.uChar.AsciiChar as u8) as i32;

                    if k.uChar.UnicodeChar != 0 {
                        let u = k.uChar.UnicodeChar as u32;
                        let ascii = if u < 0x80 { u as i32 } else { 0 };
                        key = KeyEvent::new(ascii, u);
                    } else if key.utf32 == 0 {
                        key = KeyEvent::new(
                            ch,
                            if (0..=0x7f).contains(&ch) && ch > 0 {
                                ch as u32
                            } else {
                                0
                            },
                        );
                    }

                    if down {
                        Some(Event::KeyPress(key))
                    } else {
                        Some(Event::KeyRelease(key))
                    }
                }
                MOUSE_EVENT => {
                    let m = rec.Event.MouseEvent;
                    let flags = m.dwEventFlags;
                    if flags == 0 || flags == 2 {
                        // Press, release or double click.
                        let changed = self.mouse_state ^ m.dwButtonState;
                        let mapping = [1, 3, 2, 8, 9];
                        for button in 1..=5usize {
                            let mask = 1u32 << (button - 1);
                            if changed == mask {
                                self.mouse_state = m.dwButtonState;
                                let x = m.dwMousePosition.X as i32;
                                let y = m.dwMousePosition.Y as i32;
                                return Some(if m.dwButtonState & mask != 0 {
                                    Event::MousePress {
                                        x,
                                        y,
                                        button: mapping[button - 1],
                                    }
                                } else {
                                    Event::MouseRelease {
                                        x,
                                        y,
                                        button: mapping[button - 1],
                                    }
                                });
                            }
                        }
                        None
                    } else if flags == MOUSE_MOVED {
                        let x = m.dwMousePosition.X as i32;
                        let y = m.dwMousePosition.Y as i32;
                        Some(Event::MouseMotion { x, y })
                    } else if flags == MOUSE_WHEELED || flags == MOUSE_HWHEELED {
                        let high = ((m.dwButtonState >> 16) as u16) as i16;
                        let button = match (flags, high > 0) {
                            (MOUSE_WHEELED, true) => 4,
                            (MOUSE_WHEELED, false) => 5,
                            (MOUSE_HWHEELED, true) => 7,
                            _ => 6,
                        };
                        let x = self.size().0 / 2;
                        let y = self.size().1 / 2;
                        Some(Event::MousePress { x, y, button })
                    } else {
                        None
                    }
                }
                WINDOW_BUFFER_SIZE_EVENT => {
                    let s = rec.Event.WindowBufferSizeEvent.dwSize;
                    let w = s.X as i32;
                    let h = s.Y as i32;
                    if w <= 0 || h <= 0 || (w == canvas.width() && h == canvas.height()) {
                        None
                    } else {
                        Some(Event::Resize { w, h })
                    }
                }
                _ => None,
            }
        }
    }
}

impl Drop for Win32 {
    fn drop(&mut self) {
        self.end();
    }
}

fn key_from_vk(vk: u16) -> KeyEvent {
    let ch = match vk {
        VK_TAB => key::TAB,
        VK_RETURN => key::RETURN,
        VK_ESCAPE => key::ESCAPE,
        VK_SPACE => b' ' as i32,
        VK_DELETE => key::DELETE,
        VK_LEFT => key::LEFT,
        VK_RIGHT => key::RIGHT,
        VK_UP => key::UP,
        VK_DOWN => key::DOWN,
        VK_INSERT => key::INSERT,
        VK_HOME => key::HOME,
        VK_END => key::END,
        VK_PRIOR => key::PAGEUP,
        VK_NEXT => key::PAGEDOWN,
        VK_F1 => key::F1,
        VK_F2 => key::F2,
        VK_F3 => key::F3,
        VK_F4 => key::F4,
        VK_F5 => key::F5,
        VK_F6 => key::F6,
        VK_F7 => key::F7,
        VK_F8 => key::F8,
        VK_F9 => key::F9,
        VK_F10 => key::F10,
        VK_F11 => key::F11,
        VK_F12 => key::F12,
        VK_F13 => key::F13,
        VK_F14 => key::F14,
        VK_F15 => key::F15,
        VK_NUMPAD0 => b'0' as i32,
        VK_NUMPAD1 => b'1' as i32,
        VK_NUMPAD2 => b'2' as i32,
        VK_NUMPAD3 => b'3' as i32,
        VK_NUMPAD4 => b'4' as i32,
        VK_NUMPAD5 => b'5' as i32,
        VK_NUMPAD6 => b'6' as i32,
        VK_NUMPAD7 => b'7' as i32,
        VK_NUMPAD8 => b'8' as i32,
        VK_NUMPAD9 => b'9' as i32,
        VK_MULTIPLY => b'*' as i32,
        VK_ADD => b'+' as i32,
        VK_SEPARATOR => b',' as i32,
        VK_SUBTRACT => b'-' as i32,
        VK_DECIMAL => b'.' as i32,
        VK_DIVIDE => b'/' as i32,
        _ => key::UNKNOWN,
    };

    let utf32 = if (0x21..=0x7f).contains(&ch) {
        ch as u32
    } else if ch == b' ' as i32 {
        b' ' as u32
    } else {
        0
    };

    KeyEvent::new(ch, utf32)
}
