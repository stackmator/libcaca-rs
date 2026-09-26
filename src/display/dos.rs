//! Emulated DOS `conio` display driver.
//!
//! Port of `caca/driver/conio.c`. The DOS system calls are unavailable
//! outside DOS; what this ports faithfully is everything else:
//!
//! - the full-canvas blit converting UTF-32 to CP437 with `[...]` fullwidth
//!   markers (byte-identical algorithm to the C version),
//! - the event logic: poll for a key, report the press **and synthesize the
//!   matching release**, exactly like the C `_caca_push_event` call,
//! - the fixed geometry, title refusal and resize behaviour.
//!
//! Input comes from the shared stdin reader
//! ([`RawInput`](super::input::RawInput)).

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::attr::Attr;
use crate::canvas::{Canvas, CACA_MAGIC_FULLWIDTH};
use crate::charset::utf32_to_cp437;
use crate::display::event::{Event, EventMask, KeyEvent};
use crate::display::input::RawInput;
use crate::error::{CacaError, Result};

/// Emulated DOS screen dimensions. The C driver sizes to the text mode.
pub const DOS_WIDTH: i32 = 80;
/// Emulated DOS screen dimensions. The C driver sizes to the text mode.
pub const DOS_HEIGHT: i32 = 25;

/// Blit the whole canvas into an emulated `(char, attr)` screen buffer.
///
/// This is the `conio_display` loop: reverse order, CP437 conversion, and
/// `[...]` fullwidth markers when the next cell holds a wide glyph's tail.
pub(crate) fn blit_screen(buf: &mut [u8], canvas: &Canvas) {
    let chars = canvas.chars();
    let attrs = canvas.attrs();
    let n = chars.len();

    let mut i = 0usize;
    let mut bi = 0usize;
    while i < n {
        let mut ch = utf32_to_cp437(chars[i]);
        if i + 1 < n && chars[i + 1] == CACA_MAGIC_FULLWIDTH {
            buf[bi] = b'[';
            buf[bi + 1] = Attr::from_raw(attrs[i]).to_ansi();
            ch = b']';
            i += 1;
            bi += 2;
        }
        buf[bi] = ch;
        buf[bi + 1] = Attr::from_raw(attrs[i]).to_ansi();
        i += 1;
        bi += 2;
    }
}

/// Queue of synthesized key releases. The DOS driver reports a release right
/// after every press; this queue reproduces that behaviour on top of the
/// shared parser (which only yields presses).
#[derive(Default)]
struct ReleaseQueue {
    queue: VecDeque<KeyEvent>,
}

impl ReleaseQueue {
    /// Record a press, queuing its matching release.
    fn push_press(&mut self, key: KeyEvent) {
        self.queue.push_back(key);
    }

    /// Pop the first queued release matching `mask`.
    fn pop_matching(&mut self, mask: EventMask) -> Option<Event> {
        let pos = self
            .queue
            .iter()
            .position(|k| Event::KeyRelease(*k).matches(mask))?;
        self.queue.remove(pos).map(Event::KeyRelease)
    }
}

/// Emulated DOS console state.
pub struct Dos {
    input: RawInput,
    screen: Vec<u8>,
    releases: ReleaseQueue,
    active: bool,
}

impl Dos {
    /// Initialise the emulation: raw input, blank screen, 80x25 canvas.
    pub fn new(canvas: &mut Canvas) -> Result<Dos> {
        let mut input = RawInput::new().map_err(|_| CacaError::Invalid)?;
        input.start();
        canvas.set_size(DOS_WIDTH, DOS_HEIGHT)?;

        Ok(Dos {
            input,
            screen: vec![0; (DOS_WIDTH * DOS_HEIGHT * 2) as usize],
            releases: ReleaseQueue::default(),
            active: true,
        })
    }

    /// Shut down: stop the reader (raw mode restores on drop).
    pub fn end(&mut self) {
        self.input.stop();
        self.active = false;
    }

    /// The C 6x10 font fallback, like the DOS driver.
    pub fn display_width(&self, canvas: &Canvas) -> i32 {
        canvas.width() * 6
    }

    /// The C 6x10 font fallback, like the DOS driver.
    pub fn display_height(&self, canvas: &Canvas) -> i32 {
        canvas.height() * 10
    }

    /// Blit the canvas into the emulated screen.
    pub fn display(&mut self, canvas: &Canvas) {
        let (w, h) = (canvas.width(), canvas.height());
        let need = (w.max(0) * h.max(0) * 2) as usize;
        if self.screen.len() < need {
            self.screen.resize(need, 0);
        }

        blit_screen(&mut self.screen, canvas);
    }

    /// Titles are unsupported, of course.
    pub fn set_title(&mut self, _title: &str) -> Result<()> {
        Err(CacaError::NotImplemented)
    }

    /// Poll for input with press/release synthesis.
    pub fn get_event(&mut self, mask: EventMask, timeout_us: i64) -> Option<Event> {
        if mask.bits() == 0 || !self.active {
            return None;
        }

        let deadline = if timeout_us >= 0 {
            Some(Instant::now() + Duration::from_micros(timeout_us as u64))
        } else {
            None
        };

        loop {
            // Releases queued by earlier presses go first, like the C
            // driver's private event buffer.
            if let Some(ev) = self.releases.pop_matching(mask) {
                return Some(ev);
            }

            if let Some(ev) = self.input.next_event() {
                if let Event::KeyPress(k) = ev {
                    // The DOS driver synthesises the release immediately.
                    self.releases.push_press(k);
                    if ev.matches(mask) {
                        return Some(ev);
                    }
                    continue;
                }
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
                            if let Event::KeyPress(k) = ev {
                                self.releases.push_press(k);
                            }
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

            match self.input.read_byte(remaining) {
                Some(b) => self.input.push_bytes(&[b]),
                None => {
                    if let Some(ev) = self.input.flush_escape() {
                        if let Event::KeyPress(k) = ev {
                            self.releases.push_press(k);
                        }
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

    /// The raw emulated screen buffer.
    pub fn screen(&self) -> &[u8] {
        &self.screen
    }
}

impl Drop for Dos {
    fn drop(&mut self) {
        self.end();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_queue_pairs_presses() {
        let mut q = ReleaseQueue::default();
        let k = KeyEvent::new(b'a' as i32, b'a' as u32);
        q.push_press(k);

        // Releases match release (and any) masks, never press-only masks.
        assert!(q.pop_matching(EventMask::KEY_PRESS).is_none());
        assert_eq!(
            q.pop_matching(EventMask::KEY_RELEASE),
            Some(Event::KeyRelease(k))
        );
        assert_eq!(q.pop_matching(EventMask::ANY), None);
    }

    #[test]
    fn screen_blit_matches_vga_encoding() {
        let mut cv = Canvas::new(4, 1).unwrap();
        cv.clear_dirty_rect_list();
        cv.put_char(0, 0, b'Z' as u32);
        cv.put_char(2, 0, 0x3000);

        let mut buf = vec![0u8; 8];
        blit_screen(&mut buf, &cv);

        assert_eq!(buf[0], b'Z');
        // Fullwidth glyph becomes "[ ]"-style markers like VGA.
        assert_eq!(buf[4], b'[');
        assert_eq!(buf[6], b']');
    }
}
