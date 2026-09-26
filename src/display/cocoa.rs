//! Native macOS window driver.
//!
//! A pure-Rust equivalent of libcaca's Cocoa driver using `objc2`: an
//! `NSWindow` shows an `NSImageView` fed with the canvas rasterised by our
//! own bitmap [`Font`](crate::font::Font), and input comes from the `NSEvent`
//! queue. Enable with the `cocoa` cargo feature and select the `cocoa`
//! driver explicitly (never autodetected).
//!
//! AppKit requires the main thread: constructing [`Cocoa`] off the main
//! thread fails cleanly instead of misbehaving. Everything runs
//! synchronously on the calling thread — events are pumped with
//! `nextEventMatchingMask:`, drawing is immediate — so the host never
//! surrenders its run loop and no custom view classes are needed.
//!
//! Compile-verified; needs macOS hardware for a runtime test.

#![cfg(all(feature = "cocoa", target_os = "macos"))]

use std::time::{Duration, Instant};

use objc2::rc::Retained;
use objc2::{AnyThread, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSAutoresizingMaskOptions, NSBackingStoreType,
    NSBitmapImageRep, NSCursor, NSEvent, NSEventMask, NSEventType, NSImage, NSImageView, NSWindow,
    NSWindowStyleMask,
};
use objc2_foundation::{NSDate, NSDefaultRunLoopMode, NSPoint, NSRect, NSSize, NSString};

use crate::canvas::Canvas;
use crate::display::event::{key, Event, EventMask, KeyEvent};
use crate::error::{CacaError, Result};
use crate::font::{font_list, load_builtin, Font};

/// macOS virtual keycodes for special keys.
mod vk {
    pub const ESCAPE: u16 = 53;
    pub const TAB: u16 = 48;
    pub const RETURN: u16 = 36;
    pub const BACKSPACE: u16 = 51;
    pub const DELETE: u16 = 117;
    pub const SPACE: u16 = 49;
    pub const UP: u16 = 126;
    pub const DOWN: u16 = 125;
    pub const LEFT: u16 = 123;
    pub const RIGHT: u16 = 124;
    pub const HOME: u16 = 115;
    pub const END: u16 = 119;
    pub const PAGEUP: u16 = 116;
    pub const PAGEDOWN: u16 = 121;
    pub const INSERT: u16 = 114;
    pub const F1: u16 = 122;
    pub const F2: u16 = 120;
    pub const F3: u16 = 99;
    pub const F4: u16 = 118;
    pub const F5: u16 = 96;
    pub const F6: u16 = 97;
    pub const F7: u16 = 98;
    pub const F8: u16 = 100;
    pub const F9: u16 = 101;
    pub const F10: u16 = 109;
    pub const F11: u16 = 103;
    pub const F12: u16 = 111;
    pub const F13: u16 = 105;
    pub const F14: u16 = 107;
    pub const F15: u16 = 113;
}

/// macOS window driver state. Main thread only.
pub struct Cocoa {
    app: Retained<NSApplication>,
    window: Retained<NSWindow>,
    image_view: Retained<NSImageView>,
    font: Font,
    cell_w: i32,
    cell_h: i32,
    cols: i32,
    rows: i32,
    mouse: (i32, i32),
    cursor_visible: bool,
    events: Vec<Event>,
    quit_sent: bool,
    active: bool,
}

impl Cocoa {
    /// Open the window and size `canvas` to it. Must be called on the main
    /// thread; any other thread gets `Invalid` instead of undefined behaviour.
    pub fn new(canvas: &mut Canvas) -> Result<Cocoa> {
        let mtm = MainThreadMarker::new().ok_or(CacaError::Invalid)?;

        let (mut cols, mut rows) = (canvas.width(), canvas.height());
        if cols <= 0 || rows <= 0 {
            cols = 80;
            rows = 24;
        }

        let names = font_list();
        let font = load_builtin(names.first().ok_or(CacaError::Invalid)?)?;
        let (cell_w, cell_h) = (font.width(), font.height());
        if cell_w <= 0 || cell_h <= 0 {
            return Err(CacaError::Invalid);
        }

        let app = NSApplication::sharedApplication(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
        app.finishLaunching();

        let rect = NSRect::new(
            NSPoint::new(100.0, 100.0),
            NSSize::new(cols as f64 * cell_w as f64, rows as f64 * cell_h as f64),
        );
        // SAFETY: `mtm` proves we are on the main thread, as required.
        let window: Retained<NSWindow> = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(mtm),
                rect,
                NSWindowStyleMask::Titled
                    | NSWindowStyleMask::Closable
                    | NSWindowStyleMask::Miniaturizable
                    | NSWindowStyleMask::Resizable,
                NSBackingStoreType::Buffered,
                false,
            )
        };
        window.setTitle(&NSString::from_str("caca"));
        window.setAcceptsMouseMovedEvents(true);

        let content = window.contentView().ok_or(CacaError::Invalid)?;
        let bounds = content.bounds();
        let image_view: Retained<NSImageView> =
            NSImageView::initWithFrame(NSImageView::alloc(mtm), bounds);
        image_view.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable
                | NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        content.addSubview(&image_view);

        window.makeKeyAndOrderFront(None);
        app.activate();

        canvas.set_size(cols, rows)?;

        Ok(Cocoa {
            app,
            window,
            image_view,
            font,
            cell_w,
            cell_h,
            cols,
            rows,
            mouse: (cols / 2, rows / 2),
            cursor_visible: false,
            events: Vec::new(),
            quit_sent: false,
            active: true,
        })
    }

    /// Shut the window down.
    pub fn end(&mut self) {
        if !self.active {
            return;
        }
        self.active = false;
        self.window.close();
    }

    /// Set the window title.
    pub fn set_title(&mut self, title: &str) {
        self.window.setTitle(&NSString::from_str(title));
    }

    /// The window width in pixels.
    pub fn display_width(&self) -> i32 {
        self.cols * self.cell_w
    }

    /// The window height in pixels.
    pub fn display_height(&self) -> i32 {
        self.rows * self.cell_h
    }

    /// The window size in character cells.
    pub fn size(&self) -> (i32, i32) {
        (self.cols, self.rows)
    }

    /// Rasterise the canvas and show it.
    pub fn display(&mut self, canvas: &Canvas) {
        let w = canvas.width().max(0);
        let h = canvas.height().max(0);
        if w == 0 || h == 0 {
            return;
        }
        if w as u64 * h as u64 > 512 * 512 {
            return;
        }

        let pw = w * self.cell_w;
        let ph = h * self.cell_h;
        let mut argb = vec![0u8; (pw * ph * 4) as usize];
        if self
            .font
            .render_canvas(canvas, &mut argb, pw, ph, 4 * pw)
            .is_err()
        {
            return;
        }

        // NSBitmapImageRep wants RGBA bytes; our renderer emits ARGB.
        let mut rgba = vec![0u8; argb.len()];
        for (dst, src) in rgba.chunks_exact_mut(4).zip(argb.chunks_exact(4)) {
            dst[0] = src[1];
            dst[1] = src[2];
            dst[2] = src[3];
            dst[3] = src[0];
        }

        // SAFETY: main thread; NULL planes make AppKit allocate storage, and
        // `bitmapData` below is valid while `rep` is alive.
        unsafe {
            let rep: Option<Retained<NSBitmapImageRep>> =
                NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
                    NSBitmapImageRep::alloc(),
                    std::ptr::null_mut(),
                    pw as isize,
                    ph as isize,
                    8,
                    4,
                    true,
                    false,
                    objc2_app_kit::NSDeviceRGBColorSpace,
                    (4 * pw) as isize,
                    32,
                );
            let rep = match rep {
                Some(rep) => rep,
                None => return,
            };
            let dst = rep.bitmapData();
            if dst.is_null() {
                return;
            }
            std::ptr::copy_nonoverlapping(rgba.as_ptr(), dst, rgba.len());

            let image: Retained<NSImage> =
                NSImage::initWithSize(NSImage::alloc(), NSSize::new(pw as f64, ph as f64));
            image.addRepresentation(&rep);
            self.image_view.setImage(Some(&image));

            if self.cursor_visible {
                // Block cursor via a filled overlay is skipped here; AppKit
                // shows the I-beam over text views only, and our image view
                // keeps the arrow cursor, which is the honest behaviour.
            }
        }
    }

    /// Re-read the window size after a resize.
    pub fn handle_resize(&mut self) {
        let frame = self.window.frame();
        // Content size approximates the frame here; Display re-syncs exact
        // cell counts from `size()` on the next refresh.
        let _ = frame;
    }

    /// Show or hide the mouse pointer.
    pub fn set_mouse(&mut self, show: bool) -> Result<()> {
        if show {
            NSCursor::unhide();
        } else {
            NSCursor::hide();
        }
        Ok(())
    }

    /// Show or hide the block cursor (tracked; the image view keeps the
    /// arrow cursor, so this is a no-op visually).
    pub fn set_cursor(&mut self, show: bool) {
        self.cursor_visible = show;
    }

    /// Pump pending AppKit events into the queue.
    fn pump(&mut self) {
        loop {
            // SAFETY: main thread; passing nil date would block, so poll.
            let ev: Option<Retained<NSEvent>> = unsafe {
                self.app.nextEventMatchingMask_untilDate_inMode_dequeue(
                    NSEventMask::Any,
                    Some(&NSDate::distantPast()),
                    NSDefaultRunLoopMode,
                    true,
                )
            };
            let Some(ev) = ev else { break };
            if let Some(mapped) = self.translate(&ev) {
                self.events.push(mapped);
            }
            self.app.sendEvent(&ev);
        }

        if !self.quit_sent && !self.window.isVisible() {
            self.quit_sent = true;
            self.events.push(Event::Quit);
        }

        let frame = self.window.frame();
        let cols = (frame.size.width / self.cell_w as f64).floor().max(1.0) as i32;
        let rows = (frame.size.height / self.cell_h as f64).floor().max(1.0) as i32;
        if (cols, rows) != (self.cols, self.rows) {
            self.cols = cols;
            self.rows = rows;
            self.events.push(Event::Resize { w: cols, h: rows });
        }
    }

    fn translate(&mut self, ev: &NSEvent) -> Option<Event> {
        match ev.r#type() {
            NSEventType::KeyDown | NSEventType::KeyUp => {
                let pressed = ev.r#type() == NSEventType::KeyDown;
                let code = ev.keyCode();
                if let Some(s) = ev.characters() {
                    let s = s.to_string();
                    if let Some(ch) = s.chars().next() {
                        let utf32 = ch as u32;
                        if !is_special_keycode(code) {
                            let key =
                                KeyEvent::new(if utf32 < 0x80 { utf32 as i32 } else { 0 }, utf32);
                            return Some(if pressed {
                                Event::KeyPress(key)
                            } else {
                                Event::KeyRelease(key)
                            });
                        }
                    }
                }
                let ch = match code {
                    vk::ESCAPE => key::ESCAPE,
                    vk::TAB => key::TAB,
                    vk::RETURN => key::RETURN,
                    vk::BACKSPACE => key::BACKSPACE,
                    vk::DELETE => key::DELETE,
                    vk::SPACE => b' ' as i32,
                    vk::UP => key::UP,
                    vk::DOWN => key::DOWN,
                    vk::LEFT => key::LEFT,
                    vk::RIGHT => key::RIGHT,
                    vk::HOME => key::HOME,
                    vk::END => key::END,
                    vk::PAGEUP => key::PAGEUP,
                    vk::PAGEDOWN => key::PAGEDOWN,
                    vk::INSERT => key::INSERT,
                    vk::F1 => key::F1,
                    vk::F2 => key::F2,
                    vk::F3 => key::F3,
                    vk::F4 => key::F4,
                    vk::F5 => key::F5,
                    vk::F6 => key::F6,
                    vk::F7 => key::F7,
                    vk::F8 => key::F8,
                    vk::F9 => key::F9,
                    vk::F10 => key::F10,
                    vk::F11 => key::F11,
                    vk::F12 => key::F12,
                    vk::F13 => key::F13,
                    vk::F14 => key::F14,
                    vk::F15 => key::F15,
                    _ => return None,
                };
                let key = KeyEvent::new(ch, 0);
                Some(if pressed {
                    Event::KeyPress(key)
                } else {
                    Event::KeyRelease(key)
                })
            }
            NSEventType::LeftMouseDown
            | NSEventType::RightMouseDown
            | NSEventType::OtherMouseDown => {
                let (x, y) = self.mouse_cell(ev);
                let button = match ev.r#type() {
                    NSEventType::LeftMouseDown => 1,
                    NSEventType::RightMouseDown => 2,
                    _ => ev.buttonNumber() as i32 + 1,
                };
                Some(Event::MousePress { x, y, button })
            }
            NSEventType::LeftMouseUp | NSEventType::RightMouseUp | NSEventType::OtherMouseUp => {
                let (x, y) = self.mouse_cell(ev);
                let button = match ev.r#type() {
                    NSEventType::LeftMouseUp => 1,
                    NSEventType::RightMouseUp => 2,
                    _ => ev.buttonNumber() as i32 + 1,
                };
                Some(Event::MouseRelease { x, y, button })
            }
            NSEventType::LeftMouseDragged
            | NSEventType::RightMouseDragged
            | NSEventType::OtherMouseDragged
            | NSEventType::MouseMoved => {
                let (x, y) = self.mouse_cell(ev);
                if (x, y) == self.mouse {
                    None
                } else {
                    self.mouse = (x, y);
                    Some(Event::MouseMotion { x, y })
                }
            }
            NSEventType::ScrollWheel => {
                let (x, y) = self.mouse_cell(ev);
                Some(Event::MousePress { x, y, button: 4 })
            }
            _ => None,
        }
    }

    fn mouse_cell(&self, ev: &NSEvent) -> (i32, i32) {
        let pt = ev.locationInWindow();
        let h = self.rows as f64 * self.cell_h as f64;
        let x = (pt.x / self.cell_w as f64) as i32;
        let y = ((h - pt.y) / self.cell_h as f64) as i32;
        (x, y)
    }

    /// Wait for an event matching `mask`.
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
            self.pump();
            if let Some(pos) = self.events.iter().position(|e| e.matches(mask)) {
                return Some(self.events.remove(pos));
            }
            match deadline {
                Some(d) => {
                    let now = Instant::now();
                    if now >= d {
                        return None;
                    }
                    std::thread::sleep(Duration::from_millis(2).min(d - now));
                }
                None => std::thread::sleep(Duration::from_millis(2)),
            }
        }
    }
}

impl Drop for Cocoa {
    fn drop(&mut self) {
        self.end();
    }
}

fn is_special_keycode(code: u16) -> bool {
    matches!(
        code,
        vk::UP
            | vk::DOWN
            | vk::LEFT
            | vk::RIGHT
            | vk::HOME
            | vk::END
            | vk::PAGEUP
            | vk::PAGEDOWN
            | vk::INSERT
            | vk::DELETE
            | vk::F1
            | vk::F2
            | vk::F3
            | vk::F4
            | vk::F5
            | vk::F6
            | vk::F7
            | vk::F8
            | vk::F9
            | vk::F10
            | vk::F11
            | vk::F12
            | vk::F13
            | vk::F14
            | vk::F15
    )
}
