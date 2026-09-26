//! Dirty rectangle tracking.
//!
//! Port of `caca/dirty.c`. Dirty rectangles are non-overlapping areas that
//! changed since the last display refresh. Display drivers use them to avoid
//! redrawing the whole screen.

use crate::canvas::Canvas;
use crate::error::{CacaError, Result};

/// Maximum number of tracked dirty rectangles.
pub const MAX_DIRTY_COUNT: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DirtyRect {
    pub(crate) xmin: i32,
    pub(crate) ymin: i32,
    pub(crate) xmax: i32,
    pub(crate) ymax: i32,
}

fn int_min(a: i32, b: i32) -> i32 {
    if a < b {
        a
    } else {
        b
    }
}

fn int_max(a: i32, b: i32) -> i32 {
    if a > b {
        a
    } else {
        b
    }
}

impl Canvas {
    /// Disable dirty rectangle handling (recursive).
    pub fn disable_dirty_rect(&mut self) {
        self.dirty_disabled += 1;
    }

    /// Re-enable dirty rectangle handling.
    pub fn enable_dirty_rect(&mut self) -> Result<()> {
        if self.dirty_disabled <= 0 {
            return Err(CacaError::Invalid);
        }
        self.dirty_disabled -= 1;
        Ok(())
    }

    /// Number of dirty rectangles currently tracked.
    pub fn dirty_rect_count(&self) -> usize {
        self.ndirty
    }

    /// Return `(x, y, width, height)` for dirty rectangle `r`.
    pub fn dirty_rect(&self, r: usize) -> Result<(i32, i32, i32, i32)> {
        if r >= self.ndirty {
            return Err(CacaError::Invalid);
        }
        let d = self.dirty[r];
        Ok((
            d.xmin,
            d.ymin,
            d.xmax - d.xmin + 1,
            d.ymax - d.ymin + 1,
        ))
    }

    /// Add an area to the dirty rectangle list.
    pub fn add_dirty_rect(&mut self, x: i32, y: i32, width: i32, height: i32) -> Result<()> {
        let (w, h) = (self.width(), self.height());
        let mut x = x;
        let mut y = y;
        let mut width = width;
        let mut height = height;

        if x < 0 {
            width += x;
            x = 0;
        }
        if x + width > w {
            width = w - x;
        }
        if y < 0 {
            height += y;
            y = 0;
        }
        if y + height > h {
            height = h - y;
        }

        if width <= 0 || height <= 0 {
            return Err(CacaError::Invalid);
        }

        let rect = DirtyRect {
            xmin: x,
            ymin: y,
            xmax: x + width - 1,
            ymax: y + height - 1,
        };
        self.dirty.push(rect);
        self.ndirty += 1;
        let n = self.ndirty - 1;
        self.merge_new_rect(n);
        Ok(())
    }

    /// Mark an area as clean. The reference implementation is a no-op.
    pub fn remove_dirty_rect(&mut self, x: i32, y: i32, width: i32, height: i32) -> Result<()> {
        let (w, h) = (self.width(), self.height());
        let mut x = x;
        let mut y = y;
        let mut width = width;
        let mut height = height;

        if x < 0 {
            width += x;
            x = 0;
        }
        if x + width > w {
            width = w - x;
        }
        if y < 0 {
            height += y;
            y = 0;
        }
        if y + height > h {
            height = h - y;
        }

        if width <= 0 || height <= 0 {
            return Err(CacaError::Invalid);
        }

        Ok(())
    }

    /// Empty the dirty rectangle list.
    pub fn clear_dirty_rect_list(&mut self) {
        self.dirty.clear();
        self.ndirty = 0;
    }

    /// Clip all dirty rectangles to the current canvas size.
    pub(crate) fn clip_dirty_rect_list(&mut self) {
        let (w, h) = (self.width(), self.height());
        for d in self.dirty.iter_mut().take(self.ndirty) {
            if d.xmin < 0 {
                d.xmin = 0;
            }
            if d.ymin < 0 {
                d.ymin = 0;
            }
            if d.xmax >= w {
                d.xmax = w - 1;
            }
            if d.ymax >= h {
                d.ymax = h - 1;
            }
        }
    }

    fn merge_new_rect(&mut self, n: usize) {
        let mut n = n;
        let mut i = 0;

        let mut best: i32 = -1;
        let mut best_score = self.width() * self.height();

        let sn = (self.dirty[n].xmax - self.dirty[n].xmin + 1)
            * (self.dirty[n].ymax - self.dirty[n].ymin + 1);

        while i < self.ndirty {
            if i == n {
                i += 1;
                continue;
            }

            let xmin = int_min(self.dirty[i].xmin, self.dirty[n].xmin);
            let ymin = int_min(self.dirty[i].ymin, self.dirty[n].ymin);
            let xmax = int_max(self.dirty[i].xmax, self.dirty[n].xmax);
            let ymax = int_max(self.dirty[i].ymax, self.dirty[n].ymax);

            let sf = (xmax - xmin + 1) * (ymax - ymin + 1);

            // If the current rectangle is inside the new rectangle, remove
            // the current one and keep trying.
            if sf == sn {
                self.dirty.remove(i);
                self.ndirty -= 1;
                if i < n {
                    n -= 1;
                } else {
                    i -= 1;
                }
                i += 1;
                continue;
            }

            let si = (self.dirty[i].xmax - self.dirty[i].xmin + 1)
                * (self.dirty[i].ymax - self.dirty[i].ymin + 1);

            // If the new rectangle is inside the current one, drop it.
            if sf == si {
                self.dirty.remove(n);
                self.ndirty -= 1;
                return;
            }

            let wasted = sf - si - sn;
            if wasted < best_score {
                best = i as i32;
                best_score = wasted;
            }

            i += 1;
        }

        if best_score > 0 && self.ndirty < MAX_DIRTY_COUNT {
            return;
        }

        // `best` is -1 only when the rectangle list is full and every
        // candidate was already accounted for; merging with index 0 is the
        // conservative fallback, matching the reference behaviour of using
        // the least-wasted candidate.
        let best_idx = if best < 0 { 0 } else { best as usize };

        self.dirty[best_idx].xmin = int_min(self.dirty[best_idx].xmin, self.dirty[n].xmin);
        self.dirty[best_idx].ymin = int_min(self.dirty[best_idx].ymin, self.dirty[n].ymin);
        self.dirty[best_idx].xmax = int_max(self.dirty[best_idx].xmax, self.dirty[n].xmax);
        self.dirty[best_idx].ymax = int_max(self.dirty[best_idx].ymax, self.dirty[n].ymax);

        self.dirty.remove(n);
        self.ndirty -= 1;

        if best_idx < n {
            self.merge_new_rect(best_idx);
        } else {
            self.merge_new_rect(best_idx - 1);
        }
    }
}
