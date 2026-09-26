//! User events: keyboard, mouse, resize and quit.
//!
//! Port of the public event API in `caca/event.c` and `caca/caca.h`, together
//! with a small incremental ANSI/VT input parser used by the terminal driver.

use alloc::{
    string::{String, ToString},
    vec::Vec,
};

use crate::charset::utf8_to_utf32;

/// Special key values, mirroring `enum caca_key`.
pub mod key {
    pub const UNKNOWN: i32 = 0x00;
    pub const CTRL_A: i32 = 0x01;
    pub const CTRL_B: i32 = 0x02;
    pub const CTRL_C: i32 = 0x03;
    pub const CTRL_D: i32 = 0x04;
    pub const CTRL_E: i32 = 0x05;
    pub const CTRL_F: i32 = 0x06;
    pub const CTRL_G: i32 = 0x07;
    pub const BACKSPACE: i32 = 0x08;
    pub const TAB: i32 = 0x09;
    pub const CTRL_J: i32 = 0x0a;
    pub const CTRL_K: i32 = 0x0b;
    pub const CTRL_L: i32 = 0x0c;
    pub const RETURN: i32 = 0x0d;
    pub const CTRL_N: i32 = 0x0e;
    pub const CTRL_O: i32 = 0x0f;
    pub const CTRL_P: i32 = 0x10;
    pub const CTRL_Q: i32 = 0x11;
    pub const CTRL_R: i32 = 0x12;
    pub const PAUSE: i32 = 0x13;
    pub const CTRL_T: i32 = 0x14;
    pub const CTRL_U: i32 = 0x15;
    pub const CTRL_V: i32 = 0x16;
    pub const CTRL_W: i32 = 0x17;
    pub const CTRL_X: i32 = 0x18;
    pub const CTRL_Y: i32 = 0x19;
    pub const CTRL_Z: i32 = 0x1a;
    pub const ESCAPE: i32 = 0x1b;
    pub const DELETE: i32 = 0x7f;

    pub const UP: i32 = 0x111;
    pub const DOWN: i32 = 0x112;
    pub const LEFT: i32 = 0x113;
    pub const RIGHT: i32 = 0x114;
    pub const INSERT: i32 = 0x115;
    pub const HOME: i32 = 0x116;
    pub const END: i32 = 0x117;
    pub const PAGEUP: i32 = 0x118;
    pub const PAGEDOWN: i32 = 0x119;
    pub const F1: i32 = 0x11a;
    pub const F2: i32 = 0x11b;
    pub const F3: i32 = 0x11c;
    pub const F4: i32 = 0x11d;
    pub const F5: i32 = 0x11e;
    pub const F6: i32 = 0x11f;
    pub const F7: i32 = 0x120;
    pub const F8: i32 = 0x121;
    pub const F9: i32 = 0x122;
    pub const F10: i32 = 0x123;
    pub const F11: i32 = 0x124;
    pub const F12: i32 = 0x125;
    pub const F13: i32 = 0x126;
    pub const F14: i32 = 0x127;
    pub const F15: i32 = 0x128;
}

/// Event type bitmask, mirroring `enum caca_event_type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventMask(pub u32);

impl EventMask {
    pub const NONE: EventMask = EventMask(0x0000);
    pub const KEY_PRESS: EventMask = EventMask(0x0001);
    pub const KEY_RELEASE: EventMask = EventMask(0x0002);
    pub const MOUSE_PRESS: EventMask = EventMask(0x0004);
    pub const MOUSE_RELEASE: EventMask = EventMask(0x0008);
    pub const MOUSE_MOTION: EventMask = EventMask(0x0010);
    pub const RESIZE: EventMask = EventMask(0x0020);
    pub const QUIT: EventMask = EventMask(0x0040);
    pub const ANY: EventMask = EventMask(0xffff);

    pub const fn bits(self) -> u32 {
        self.0
    }

    pub const fn contains(self, other: EventMask) -> bool {
        (self.0 & other.0) != 0
    }
}

impl core::ops::BitOr for EventMask {
    type Output = EventMask;
    fn bitor(self, rhs: EventMask) -> EventMask {
        EventMask(self.0 | rhs.0)
    }
}

/// A key press/release payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEvent {
    /// ASCII value or special [`key`] constant.
    pub ch: i32,
    /// Unicode codepoint when the key maps to a printable character, else 0.
    pub utf32: u32,
}

impl KeyEvent {
    pub fn new(ch: i32, utf32: u32) -> KeyEvent {
        KeyEvent { ch, utf32 }
    }

    /// The UTF-8 representation, or an empty string for special keys.
    pub fn utf8(&self) -> String {
        if self.utf32 == 0 {
            String::new()
        } else {
            char::from_u32(self.utf32)
                .map(|c| c.to_string())
                .unwrap_or_default()
        }
    }
}

/// A user event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// No event.
    None,
    /// A key was pressed.
    KeyPress(KeyEvent),
    /// A key was released.
    KeyRelease(KeyEvent),
    /// A mouse button was pressed.
    MousePress { x: i32, y: i32, button: i32 },
    /// A mouse button was released.
    MouseRelease { x: i32, y: i32, button: i32 },
    /// The mouse moved.
    MouseMotion { x: i32, y: i32 },
    /// The window was resized.
    Resize { w: i32, h: i32 },
    /// The user requested to quit.
    Quit,
}

impl Event {
    /// The event type bit (as an [`EventMask`]).
    pub fn event_type(&self) -> EventMask {
        match self {
            Event::None => EventMask::NONE,
            Event::KeyPress(_) => EventMask::KEY_PRESS,
            Event::KeyRelease(_) => EventMask::KEY_RELEASE,
            Event::MousePress { .. } => EventMask::MOUSE_PRESS,
            Event::MouseRelease { .. } => EventMask::MOUSE_RELEASE,
            Event::MouseMotion { .. } => EventMask::MOUSE_MOTION,
            Event::Resize { .. } => EventMask::RESIZE,
            Event::Quit => EventMask::QUIT,
        }
    }

    /// Whether this event matches a mask.
    pub fn matches(&self, mask: EventMask) -> bool {
        self.event_type().contains(mask)
    }

    /// The key payload, if this is a key event.
    pub fn key(&self) -> Option<KeyEvent> {
        match self {
            Event::KeyPress(k) | Event::KeyRelease(k) => Some(*k),
            _ => None,
        }
    }
}

/// Incremental parser turning raw terminal bytes into [`Event`]s.
#[derive(Default)]
pub struct EventParser {
    buf: Vec<u8>,
}

impl EventParser {
    pub fn new() -> EventParser {
        EventParser { buf: Vec::new() }
    }

    /// Feed raw bytes into the parser.
    pub fn push(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    /// True if the parser holds bytes that may form an incomplete sequence.
    pub fn has_pending(&self) -> bool {
        !self.buf.is_empty()
    }

    /// Try to extract the next complete event.
    pub fn next_event(&mut self) -> Option<Event> {
        if self.buf.is_empty() {
            return None;
        }

        if self.buf[0] == 0x1b {
            return self.parse_escape();
        }

        // Control characters.
        let b = self.buf[0];
        if b < 0x20 || b == 0x7f {
            self.buf.remove(0);
            let ch = b as i32;
            return Some(Event::KeyPress(KeyEvent::new(ch, 0)));
        }

        // Decode UTF-8.
        let (ch, rd) = utf8_to_utf32(&self.buf);
        if rd == 0 {
            // Possibly an incomplete multi-byte sequence.
            if self.buf.len() < 4 {
                return None;
            }
            self.buf.remove(0);
            return Some(Event::KeyPress(KeyEvent::new(b as i32, 0)));
        }
        self.buf.drain(0..rd);
        let ch_i = if ch < 0x80 { ch as i32 } else { 0 };
        Some(Event::KeyPress(KeyEvent::new(ch_i, ch)))
    }

    fn parse_escape(&mut self) -> Option<Event> {
        if self.buf.len() < 2 {
            // A lone ESC: report it only once we are sure no sequence follows.
            // Callers may flush via `flush_escape`.
            return None;
        }

        match self.buf[1] {
            b'[' => self.parse_csi(),
            b'O' => {
                if self.buf.len() < 3 {
                    return None;
                }
                let k = match self.buf[2] {
                    b'P' => key::F1,
                    b'Q' => key::F2,
                    b'R' => key::F3,
                    b'S' => key::F4,
                    b'A' => key::UP,
                    b'B' => key::DOWN,
                    b'C' => key::RIGHT,
                    b'D' => key::LEFT,
                    b'H' => key::HOME,
                    b'F' => key::END,
                    _ => key::UNKNOWN,
                };
                self.buf.drain(0..3);
                Some(Event::KeyPress(KeyEvent::new(k, 0)))
            }
            _ => {
                self.buf.remove(0);
                Some(Event::KeyPress(KeyEvent::new(key::ESCAPE, 0)))
            }
        }
    }

    fn parse_csi(&mut self) -> Option<Event> {
        // Find the final byte (0x40..=0x7e).
        let mut end = None;
        for (i, &b) in self.buf.iter().enumerate().skip(2) {
            if (0x40..=0x7e).contains(&b) {
                end = Some(i);
                break;
            }
        }
        let end = match end {
            Some(e) => e,
            None => {
                if self.buf.len() > 32 {
                    // Give up on malformed input.
                    self.buf.remove(0);
                }
                return None;
            }
        };

        let params: Vec<u8> = self.buf[2..end].to_vec();
        let final_byte = self.buf[end];

        // SGR mouse: ESC [ < b ; x ; y M/m
        if (final_byte == b'M' || final_byte == b'm') && params.first() == Some(&b'<') {
            if let Some((button, x, y)) = parse_mouse_params(&params[1..]) {
                self.buf.drain(0..=end);
                let button = button & 0x7f;
                if final_byte == b'M' {
                    if button & 0x20 != 0 {
                        return Some(Event::MouseMotion { x, y });
                    }
                    return Some(Event::MousePress {
                        x,
                        y,
                        button: button + 1,
                    });
                } else {
                    return Some(Event::MouseRelease {
                        x,
                        y,
                        button: button + 1,
                    });
                }
            }
        }

        let key = match final_byte {
            b'A' => Some(key::UP),
            b'B' => Some(key::DOWN),
            b'C' => Some(key::RIGHT),
            b'D' => Some(key::LEFT),
            b'H' => Some(key::HOME),
            b'F' => Some(key::END),
            b'~' => {
                let n = parse_first_number(&params).unwrap_or(0);
                Some(match n {
                    1 | 7 => key::HOME,
                    2 => key::INSERT,
                    3 => key::DELETE,
                    4 | 8 => key::END,
                    5 => key::PAGEUP,
                    6 => key::PAGEDOWN,
                    11 => key::F1,
                    12 => key::F2,
                    13 => key::F3,
                    14 => key::F4,
                    15 => key::F5,
                    17 => key::F6,
                    18 => key::F7,
                    19 => key::F8,
                    20 => key::F9,
                    21 => key::F10,
                    23 => key::F11,
                    24 => key::F12,
                    _ => key::UNKNOWN,
                })
            }
            _ => None,
        };

        self.buf.drain(0..=end);
        key.map(|k| Event::KeyPress(KeyEvent::new(k, 0)))
    }

    /// Flush a pending lone ESC as an Escape key press.
    pub fn flush_escape(&mut self) -> Option<Event> {
        if self.buf.len() == 1 && self.buf[0] == 0x1b {
            self.buf.clear();
            return Some(Event::KeyPress(KeyEvent::new(key::ESCAPE, 0)));
        }
        None
    }
}

fn parse_first_number(params: &[u8]) -> Option<i32> {
    let mut n: i32 = 0;
    let mut seen = false;
    for &b in params {
        if b.is_ascii_digit() {
            n = n * 10 + (b - b'0') as i32;
            seen = true;
        } else if b == b';' || b == b'?' || b == b'>' || b == b'<' {
            if seen {
                break;
            }
        } else {
            break;
        }
    }
    if seen {
        Some(n)
    } else {
        None
    }
}

fn parse_mouse_params(params: &[u8]) -> Option<(i32, i32, i32)> {
    let parts: Vec<&[u8]> = params.split(|&b| b == b';').collect();
    if parts.len() < 3 {
        return None;
    }
    let button = parse_first_number(parts[0])?;
    let x = parse_first_number(parts[1])?;
    let y = parse_first_number(parts[2])?;
    Some((button, x - 1, y - 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrow_keys() {
        let mut p = EventParser::new();
        p.push(b"\x1b[A");
        assert_eq!(
            p.next_event(),
            Some(Event::KeyPress(KeyEvent::new(key::UP, 0)))
        );
    }

    #[test]
    fn function_keys() {
        let mut p = EventParser::new();
        p.push(b"\x1b[15~");
        assert_eq!(
            p.next_event(),
            Some(Event::KeyPress(KeyEvent::new(key::F5, 0)))
        );
    }

    #[test]
    fn printable_utf8() {
        let mut p = EventParser::new();
        p.push("é".as_bytes());
        let ev = p.next_event().unwrap();
        assert_eq!(ev, Event::KeyPress(KeyEvent::new(0, 0xe9)));
    }

    #[test]
    fn control_key() {
        let mut p = EventParser::new();
        p.push(&[0x03]);
        assert_eq!(
            p.next_event(),
            Some(Event::KeyPress(KeyEvent::new(key::CTRL_C, 0)))
        );
    }

    #[test]
    fn mouse_press() {
        let mut p = EventParser::new();
        p.push(b"\x1b[<0;5;7M");
        assert_eq!(
            p.next_event(),
            Some(Event::MousePress {
                x: 4,
                y: 6,
                button: 1
            })
        );
    }

    #[test]
    fn incomplete_sequence_waits() {
        let mut p = EventParser::new();
        p.push(b"\x1b[");
        assert_eq!(p.next_event(), None);
        assert!(p.has_pending());
        p.push(b"B");
        assert_eq!(
            p.next_event(),
            Some(Event::KeyPress(KeyEvent::new(key::DOWN, 0)))
        );
    }
}
