//! Canvas frame handling.
//!
//! Port of `caca/frame.c`.

use alloc::{format, string::ToString};

use crate::canvas::Canvas;
use crate::error::{CacaError, Result};

impl Canvas {
    /// Number of frames in this canvas.
    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    /// Activate a given frame.
    pub fn set_frame(&mut self, id: usize) -> Result<()> {
        if id >= self.frames.len() {
            return Err(CacaError::Invalid);
        }

        if id == self.frame {
            return Ok(());
        }

        self.frame = id;

        if !self.dirty_disabled_is_on() {
            let (w, h) = (self.width(), self.height());
            let _ = self.add_dirty_rect(0, 0, w, h);
        }

        Ok(())
    }

    /// The active frame index.
    pub fn frame(&self) -> usize {
        self.frame
    }

    /// The active frame's name.
    pub fn frame_name(&self) -> &str {
        &self.active().name
    }

    /// Set the active frame's name.
    pub fn set_frame_name(&mut self, name: &str) {
        self.active_mut().name = name.to_string();
    }

    /// Insert a new frame at index `id`, copied from the active frame.
    pub fn create_frame(&mut self, id: i32) -> Result<()> {
        let id = if id < 0 {
            0
        } else if id as usize > self.frames.len() {
            self.frames.len()
        } else {
            id as usize
        };

        let mut frame = self.active().clone();
        self.autoinc = self.autoinc.wrapping_add(1);
        frame.name = format!("frame#{:08x}", self.autoinc);

        self.frames.insert(id, frame);
        if self.frame >= id {
            self.frame += 1;
        }

        Ok(())
    }

    /// Delete a frame.
    pub fn free_frame(&mut self, id: usize) -> Result<()> {
        if id >= self.frames.len() || self.frames.len() == 1 {
            return Err(CacaError::Invalid);
        }

        self.frames.remove(id);

        if self.frame > id {
            self.frame -= 1;
        } else if self.frame == id {
            self.frame = 0;
            if !self.dirty_disabled_is_on() {
                let (w, h) = (self.width(), self.height());
                let _ = self.add_dirty_rect(0, 0, w, h);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_and_switch_frames() {
        let mut cv = Canvas::new(4, 2).unwrap();
        cv.put_str(0, 0, "aaaa");
        cv.create_frame(1).unwrap();
        assert_eq!(cv.frame_count(), 2);
        // The new frame is a copy of the previous one; insertion at index 1
        // leaves the active frame (index 0) unchanged.
        assert_eq!(cv.get_char(0, 0), b'a' as u32);
        cv.set_frame(1).unwrap();
        assert_eq!(cv.get_char(0, 0), b'a' as u32);
        cv.put_str(0, 0, "bbbb");
        cv.set_frame(0).unwrap();
        assert_eq!(cv.get_char(0, 0), b'a' as u32);
        cv.set_frame(1).unwrap();
        assert_eq!(cv.get_char(0, 0), b'b' as u32);
    }

    #[test]
    fn free_frame_last_fails() {
        let mut cv = Canvas::new(4, 2).unwrap();
        assert!(cv.free_frame(0).is_err());
    }

    #[test]
    fn frame_names() {
        let mut cv = Canvas::new(1, 1).unwrap();
        cv.set_frame_name("hello");
        assert_eq!(cv.frame_name(), "hello");
    }
}
