//! The libcaca canvas.
//!
//! Port of `caca/canvas.c`, `caca/string.c` and `caca/frame.c`. A [`Canvas`]
//! is a grid of character cells, each with its own 32-bit attribute. Multiple
//! animation frames are supported.

use alloc::{boxed::Box, string::String, vec, vec::Vec};

use crate::attr::{Attr, Color};
use crate::dirty::DirtyRect;
use crate::error::{CacaError, Result};

/// Used to indicate that the previous cell holds a fullwidth glyph.
pub const CACA_MAGIC_FULLWIDTH: u32 = 0x000f_fffe;

const VERSION_STRING: &str = "0.99.beta20-rust";

/// One animation frame of a canvas.
#[derive(Debug, Clone)]
pub(crate) struct Frame {
    pub(crate) width: i32,
    pub(crate) height: i32,
    pub(crate) chars: Vec<u32>,
    pub(crate) attrs: Vec<u32>,
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) handlex: i32,
    pub(crate) handley: i32,
    pub(crate) curattr: u32,
    pub(crate) name: String,
}

impl Frame {
    fn blank(width: i32, height: i32) -> Frame {
        let size = (width as usize) * (height as usize);
        Frame {
            width,
            height,
            chars: vec![b' ' as u32; size],
            attrs: vec![0; size],
            x: 0,
            y: 0,
            handlex: 0,
            handley: 0,
            curattr: 0,
            name: String::new(),
        }
    }
}

/// A libcaca canvas.
pub struct Canvas {
    pub(crate) frame: usize,
    pub(crate) frames: Vec<Frame>,
    pub(crate) autoinc: u32,
    pub(crate) refcount: i32,
    pub(crate) dirty_disabled: i32,
    pub(crate) dirty: Vec<DirtyRect>,
    pub(crate) ndirty: usize,
    pub(crate) resize_callback: Option<Box<dyn FnMut() -> bool>>,
}

impl Canvas {
    /// Create a new canvas with the given dimensions (in character cells).
    pub fn new(width: i32, height: i32) -> Result<Canvas> {
        if width < 0 || height < 0 {
            return Err(CacaError::Invalid);
        }

        let mut first = Frame::blank(0, 0);
        first.curattr = Attr::from_ansi(Color::Default, Color::Transparent).raw();
        first.name = String::from("frame#00000000");

        let mut cv = Canvas {
            frame: 0,
            frames: vec![first],
            autoinc: 0,
            refcount: 0,
            dirty_disabled: 0,
            dirty: Vec::new(),
            ndirty: 0,
            resize_callback: None,
        };

        cv.raw_resize(width, height)?;
        cv.set_color_ansi(Color::Default, Color::Transparent)?;
        Ok(cv)
    }

    /// The libcaca version this port targets.
    pub fn version() -> &'static str {
        VERSION_STRING
    }

    /// The canvas width, in cells.
    pub fn width(&self) -> i32 {
        self.frames[self.frame].width
    }

    /// The canvas height, in cells.
    pub fn height(&self) -> i32 {
        self.frames[self.frame].height
    }

    /// The active frame, read-only.
    pub(crate) fn active(&self) -> &Frame {
        &self.frames[self.frame]
    }

    /// The active frame, mutable.
    pub(crate) fn active_mut(&mut self) -> &mut Frame {
        &mut self.frames[self.frame]
    }

    /// The raw character array of the active frame.
    pub fn chars(&self) -> &[u32] {
        &self.active().chars
    }

    /// The raw attribute array of the active frame.
    pub fn attrs(&self) -> &[u32] {
        &self.active().attrs
    }

    /// Resize the canvas, preserving the top-left contents.
    pub fn set_size(&mut self, width: i32, height: i32) -> Result<()> {
        if width < 0 || height < 0 {
            return Err(CacaError::Invalid);
        }

        if self.refcount > 0 {
            if let Some(cb) = self.resize_callback.as_mut() {
                if !cb() {
                    return Err(CacaError::Busy);
                }
            }
        }

        self.raw_resize(width, height)
    }

    /// Alias for [`Canvas::set_size`].
    pub fn resize(&mut self, width: i32, height: i32) -> Result<()> {
        self.set_size(width, height)
    }

    pub(crate) fn raw_resize(&mut self, width: i32, height: i32) -> Result<()> {
        if width != 0 && height > i32::MAX / width {
            return Err(CacaError::Overflow);
        }

        let new_size = (width as usize) * (height as usize);
        let old_width = self.width();
        let old_height = self.height();

        if width < old_width || height < old_height {
            self.clip_dirty_rect_list();
        }

        for frame in &mut self.frames {
            let old_w = frame.width;
            let old_h = frame.height;

            let mut chars = vec![b' ' as u32; new_size];
            let mut attrs = vec![frame.curattr; new_size];

            let copy_w = width.min(old_w);
            let copy_h = height.min(old_h);

            for y in 0..copy_h {
                for x in 0..copy_w {
                    chars[(y * width + x) as usize] = frame.chars[(y * old_w + x) as usize];
                    attrs[(y * width + x) as usize] = frame.attrs[(y * old_w + x) as usize];
                }
            }

            frame.chars = chars;
            frame.attrs = attrs;
            frame.width = width;
            frame.height = height;

            if frame.x > width {
                frame.x = width;
            }
            if frame.y > height {
                frame.y = height;
            }
        }

        if !self.dirty_disabled_is_on() {
            if width > old_width && old_height > 0 {
                let _ = self.add_dirty_rect(old_width, 0, width - old_width, old_height);
            }
            if height > old_height && old_width > 0 {
                let _ = self.add_dirty_rect(0, old_height, old_width, height - old_height);
            }
            if width > old_width && height > old_height {
                let _ = self.add_dirty_rect(
                    old_width,
                    old_height,
                    width - old_width,
                    height - old_height,
                );
            }
        }

        Ok(())
    }

    pub(crate) fn dirty_disabled_is_on(&self) -> bool {
        self.dirty_disabled > 0
    }

    /// Lock the canvas to prevent resizing. The callback may deny a resize.
    pub fn manage(&mut self, callback: impl FnMut() -> bool + 'static) -> Result<()> {
        if self.refcount != 0 {
            return Err(CacaError::Busy);
        }
        self.resize_callback = Some(Box::new(callback));
        self.refcount = 1;
        Ok(())
    }

    /// Unlock a canvas previously locked by [`Canvas::manage`].
    pub fn unmanage(&mut self) -> Result<()> {
        if self.refcount == 0 {
            return Err(CacaError::Invalid);
        }
        self.refcount = 0;
        self.resize_callback = None;
        Ok(())
    }
}

impl core::fmt::Debug for Canvas {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Canvas")
            .field("width", &self.width())
            .field("height", &self.height())
            .field("frame", &self.frame)
            .field("framecount", &self.frames.len())
            .finish()
    }
}

/// Generate a random integer in `[min, max)`.
///
/// Uses a process-global xorshift generator. Without the `std` feature there
/// is no clock to seed from, so the sequence is deterministic per process
/// (but still varies from call to call).
pub fn rand(min: i32, max: i32) -> i32 {
    use core::sync::atomic::{AtomicU64, Ordering};

    static STATE: AtomicU64 = AtomicU64::new(0x9e37_79b9_7f4a_7c15);

    // Mix in a per-call counter so concurrent callers still advance.
    let mut s = STATE.fetch_add(0x2545_F491_4F6C_DD1D, Ordering::Relaxed);
    if s == 0 {
        s = 0x9e37_79b9_7f4a_7c15;
    }
    // xorshift64*
    s ^= s >> 12;
    s ^= s << 25;
    s ^= s >> 27;
    STATE.store(s, Ordering::Relaxed);
    let r = s.wrapping_mul(0x2545_F491_4F6C_DD1D);

    if max <= min {
        return min;
    }
    min + (r % (max - min) as u64) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_and_dimensions() {
        let cv = Canvas::new(10, 5).unwrap();
        assert_eq!(cv.width(), 10);
        assert_eq!(cv.height(), 5);
        assert_eq!(cv.chars().len(), 50);
    }

    #[test]
    fn resize_preserves_top_left() {
        let mut cv = Canvas::new(2, 2).unwrap();
        cv.put_char(0, 0, b'A' as u32);
        cv.put_char(1, 1, b'B' as u32);
        cv.resize(3, 3).unwrap();
        assert_eq!(cv.get_char(0, 0), b'A' as u32);
        assert_eq!(cv.get_char(1, 1), b'B' as u32);
        assert_eq!(cv.get_char(2, 2), b' ' as u32);
        cv.resize(1, 1).unwrap();
        assert_eq!(cv.get_char(0, 0), b'A' as u32);
    }

    #[test]
    fn invalid_size() {
        assert!(Canvas::new(-1, 0).is_err());
    }
}
