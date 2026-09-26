//! Cross-platform graphical window driver.
//!
//! A pure-Rust equivalent of libcaca's X11/GL window drivers, built only from
//! pure-Rust crates: `winit` opens the window and feeds input, `softbuffer`
//! presents the rasterised canvas. Enable with the `gui` cargo feature and
//! select the `winit` driver explicitly (it is never autodetected, so a GUI
//! window cannot pop up unexpectedly).
//!
//! The canvas is rasterised with the first built-in bitmap font. The winit
//! event loop runs on a background thread; frames and events cross over
//! channels, so the [`Display`](super::Display) API stays imperative.
//!
//! Note: on macOS winit requires the event loop on the main thread, so this
//! driver is primarily intended for Windows and Linux.

#![cfg(feature = "gui")]

use std::num::NonZeroU32;
use std::rc::Rc;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use softbuffer::{Context, Surface};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, KeyEvent as WinitKeyEvent, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy, OwnedDisplayHandle};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

use crate::canvas::Canvas;
use crate::display::event::{key, Event, EventMask, KeyEvent};
use crate::error::{CacaError, Result};
use crate::font::{font_list, load_builtin, Font};

/// Messages from the display thread to the window thread.
enum FrameMsg {
    Render {
        pixels: Vec<u32>,
        width: u32,
        height: u32,
    },
    SetTitle(String),
    Shutdown,
}

/// Wake-up ping for the window thread.
#[derive(Debug)]
enum UserMsg {
    Wake,
}

/// Window-creation outcome sent back to the display thread.
type ReadyResult = std::result::Result<(EventLoopProxy<UserMsg>, u32, u32), String>;

/// Window-thread state. Never leaves its thread.
struct Backend {
    window: Option<Rc<Window>>,
    context: Option<Context<OwnedDisplayHandle>>,
    surface: Option<Surface<OwnedDisplayHandle, Rc<Window>>>,
    init: Option<(String, u32, u32, i32, i32)>,
    proxy: Option<EventLoopProxy<UserMsg>>,
    ready_tx: Option<mpsc::Sender<ReadyResult>>,
    frame_rx: mpsc::Receiver<FrameMsg>,
    event_tx: mpsc::Sender<Event>,
    pending: Option<(Vec<u32>, u32, u32)>,
    cell_w: i32,
    cell_h: i32,
    cols: i32,
    rows: i32,
    mouse: (i32, i32),
}

impl Backend {
    fn send_event(&self, ev: Event) {
        let _ = self.event_tx.send(ev);
    }

    fn present(&mut self) {
        let (Some(window), Some(surface)) = (self.window.as_ref(), self.surface.as_mut()) else {
            return;
        };
        let Some((pixels, pw, ph)) = self.pending.take() else {
            return;
        };

        let size = window.inner_size();
        if size.width == 0 || size.height == 0 {
            return;
        }
        if surface
            .resize(
                NonZeroU32::new(size.width).unwrap(),
                NonZeroU32::new(size.height).unwrap(),
            )
            .is_err()
        {
            return;
        }

        let Ok(mut buffer) = surface.buffer_mut() else {
            return;
        };

        // Letterbox the frame onto the surface so transient size races
        // cannot corrupt the output.
        let (sw, sh) = (size.width as usize, size.height as usize);
        for px in buffer.iter_mut().take(sw * sh) {
            *px = 0;
        }
        let ox = (size.width.saturating_sub(pw) / 2) as usize;
        let oy = (size.height.saturating_sub(ph) / 2) as usize;
        for y in 0..ph as usize {
            if oy + y >= sh {
                break;
            }
            for x in 0..pw as usize {
                if ox + x >= sw {
                    break;
                }
                if let Some(p) = pixels.get(y * pw as usize + x) {
                    buffer[(oy + y) * sw + (ox + x)] = *p;
                }
            }
        }

        let _ = buffer.present();
    }

    fn map_key(&self, ev: &WinitKeyEvent, pressed: bool) -> Option<Event> {
        // Printable input wins over physical position.
        if let Some(text) = ev.text.as_deref() {
            if let Some(ch) = text.chars().next() {
                let utf32 = ch as u32;
                let key = KeyEvent::new(if utf32 < 0x80 { utf32 as i32 } else { 0 }, utf32);
                return Some(if pressed {
                    Event::KeyPress(key)
                } else {
                    Event::KeyRelease(key)
                });
            }
        }

        let code = match ev.physical_key {
            PhysicalKey::Code(code) => code,
            PhysicalKey::Unidentified(_) => return None,
        };
        let ch = match code {
            KeyCode::Escape => key::ESCAPE,
            KeyCode::Tab => key::TAB,
            KeyCode::Enter | KeyCode::NumpadEnter => key::RETURN,
            KeyCode::Backspace => key::BACKSPACE,
            KeyCode::Delete => key::DELETE,
            KeyCode::Space => b' ' as i32,
            KeyCode::ArrowUp => key::UP,
            KeyCode::ArrowDown => key::DOWN,
            KeyCode::ArrowLeft => key::LEFT,
            KeyCode::ArrowRight => key::RIGHT,
            KeyCode::Home => key::HOME,
            KeyCode::End => key::END,
            KeyCode::PageUp => key::PAGEUP,
            KeyCode::PageDown => key::PAGEDOWN,
            KeyCode::Insert => key::INSERT,
            KeyCode::F1 => key::F1,
            KeyCode::F2 => key::F2,
            KeyCode::F3 => key::F3,
            KeyCode::F4 => key::F4,
            KeyCode::F5 => key::F5,
            KeyCode::F6 => key::F6,
            KeyCode::F7 => key::F7,
            KeyCode::F8 => key::F8,
            KeyCode::F9 => key::F9,
            KeyCode::F10 => key::F10,
            KeyCode::F11 => key::F11,
            KeyCode::F12 => key::F12,
            KeyCode::F13 => key::F13,
            KeyCode::F14 => key::F14,
            KeyCode::F15 => key::F15,
            _ => return None,
        };
        let utf32 = if (0x20..0x80).contains(&ch) {
            ch as u32
        } else {
            0
        };
        let key = KeyEvent::new(ch, utf32);
        Some(if pressed {
            Event::KeyPress(key)
        } else {
            Event::KeyRelease(key)
        })
    }
}

impl ApplicationHandler<UserMsg> for Backend {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // `resumed` can fire more than once (e.g. Android); only the first
        // call creates the window.
        if self.window.is_some() {
            return;
        }
        let Some((title, pw, ph, cell_w, cell_h)) = self.init.take() else {
            return;
        };
        self.cell_w = cell_w;
        self.cell_h = cell_h;

        let window = match event_loop.create_window(
            Window::default_attributes()
                .with_title(title)
                .with_inner_size(PhysicalSize::new(pw.max(1), ph.max(1))),
        ) {
            Ok(window) => Rc::new(window),
            Err(e) => {
                if let Some(tx) = self.ready_tx.take() {
                    let _ = tx.send(Err(e.to_string()));
                }
                event_loop.exit();
                return;
            }
        };

        // The context was built before `run_app` and moved in with us.
        let context = match self.context.take() {
            Some(context) => context,
            None => {
                if let Some(tx) = self.ready_tx.take() {
                    let _ = tx.send(Err(String::from("no softbuffer context")));
                }
                event_loop.exit();
                return;
            }
        };
        let surface = match Surface::new(&context, window.clone()) {
            Ok(surface) => surface,
            Err(e) => {
                if let Some(tx) = self.ready_tx.take() {
                    let _ = tx.send(Err(e.to_string()));
                }
                event_loop.exit();
                return;
            }
        };

        // Keep the context alive for as long as the surface exists.
        self.context = Some(context);
        self.window = Some(window);
        self.surface = Some(surface);

        if let (Some(window), Some(tx), Some(proxy)) = (
            self.window.as_ref(),
            self.ready_tx.take(),
            self.proxy.take(),
        ) {
            let size = window.inner_size();
            let _ = tx.send(Ok((proxy, size.width, size.height)));
            window.request_redraw();
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserMsg) {
        let UserMsg::Wake = event;
        let mut shutdown = false;
        while let Ok(msg) = self.frame_rx.try_recv() {
            match msg {
                FrameMsg::Render {
                    pixels,
                    width,
                    height,
                } => {
                    self.pending = Some((pixels, width, height));
                }
                FrameMsg::SetTitle(title) => {
                    if let Some(window) = self.window.as_ref() {
                        window.set_title(&title);
                    }
                }
                FrameMsg::Shutdown => {
                    shutdown = true;
                }
            }
        }
        if shutdown {
            event_loop.exit();
            return;
        }
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested | WindowEvent::Destroyed => {
                self.send_event(Event::Quit);
                event_loop.exit();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let pressed = event.state == ElementState::Pressed;
                if let Some(ev) = self.map_key(&event, pressed) {
                    self.send_event(ev);
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if self.cell_w <= 0 || self.cell_h <= 0 {
                    return;
                }
                let x = (position.x / self.cell_w as f64) as i32;
                let y = (position.y / self.cell_h as f64) as i32;
                if (x, y) != self.mouse {
                    self.mouse = (x, y);
                    self.send_event(Event::MouseMotion { x, y });
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let pressed = state == ElementState::Pressed;
                let number = match button {
                    MouseButton::Left => 1,
                    MouseButton::Right => 2,
                    MouseButton::Middle => 3,
                    MouseButton::Back => 8,
                    MouseButton::Forward => 9,
                    MouseButton::Other(n) => n as i32,
                };
                let (x, y) = self.mouse;
                self.send_event(if pressed {
                    Event::MousePress {
                        x,
                        y,
                        button: number,
                    }
                } else {
                    Event::MouseRelease {
                        x,
                        y,
                        button: number,
                    }
                });
            }
            WindowEvent::MouseWheel { delta, .. } => {
                use winit::event::MouseScrollDelta;
                let up = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y > 0.0,
                    MouseScrollDelta::PixelDelta(pos) => pos.y > 0.0,
                };
                let (x, y) = self.mouse;
                self.send_event(Event::MousePress {
                    x,
                    y,
                    button: if up { 4 } else { 5 },
                });
            }
            WindowEvent::Resized(size) => {
                if self.cell_w <= 0 || self.cell_h <= 0 {
                    return;
                }
                let cols = (size.width / self.cell_w as u32).max(1) as i32;
                let rows = (size.height / self.cell_h as u32).max(1) as i32;
                if (cols, rows) != (self.cols, self.rows) {
                    self.cols = cols;
                    self.rows = rows;
                    self.send_event(Event::Resize { w: cols, h: rows });
                }
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                self.present();
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }
}

/// Graphical window driver state (main-thread side).
pub struct Winit {
    frame_tx: mpsc::Sender<FrameMsg>,
    event_rx: mpsc::Receiver<Event>,
    proxy: EventLoopProxy<UserMsg>,
    worker: Option<thread::JoinHandle<()>>,
    font: Font,
    cols: i32,
    rows: i32,
    cell_w: i32,
    cell_h: i32,
    active: bool,
}

impl Winit {
    /// Open the window and size `canvas` to it.
    pub fn new(canvas: &mut Canvas) -> Result<Winit> {
        let names = font_list();
        let font = load_builtin(names.first().ok_or(CacaError::Invalid)?)?;
        let (cell_w, cell_h) = (font.width(), font.height());
        if cell_w <= 0 || cell_h <= 0 {
            return Err(CacaError::Invalid);
        }

        let (mut cols, mut rows) = (canvas.width(), canvas.height());
        if cols <= 0 || rows <= 0 {
            cols = 80;
            rows = 24;
        }

        let (frame_tx, frame_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::channel();

        let title = String::from("libcaca");
        let init = (
            title,
            (cols * cell_w) as u32,
            (rows * cell_h) as u32,
            cell_w,
            cell_h,
        );
        let worker = thread::spawn(move || Self::worker(init, frame_rx, event_tx, ready_tx));

        // Wait for the window (or a clean failure when no display server
        // is reachable).
        let (proxy, pw, ph) = match ready_rx.recv_timeout(Duration::from_secs(15)) {
            Ok(Ok((proxy, pw, ph))) => (proxy, pw, ph),
            Ok(Err(_)) | Err(_) => {
                let _ = worker.join();
                return Err(CacaError::Invalid);
            }
        };

        if pw > 0 && ph > 0 {
            cols = (pw / cell_w as u32).max(1) as i32;
            rows = (ph / cell_h as u32).max(1) as i32;
        }
        canvas.set_size(cols, rows)?;

        Ok(Winit {
            frame_tx,
            event_rx,
            proxy,
            worker: Some(worker),
            font,
            cols,
            rows,
            cell_w,
            cell_h,
            active: true,
        })
    }

    fn worker(
        init: (String, u32, u32, i32, i32),
        frame_rx: mpsc::Receiver<FrameMsg>,
        event_tx: mpsc::Sender<Event>,
        ready_tx: mpsc::Sender<ReadyResult>,
    ) {
        let Ok(event_loop) = EventLoop::<UserMsg>::with_user_event().build() else {
            let _ = ready_tx.send(Err(String::from("cannot create event loop")));
            return;
        };
        let proxy = event_loop.create_proxy();
        let context = match Context::new(event_loop.owned_display_handle()) {
            Ok(context) => context,
            Err(e) => {
                let _ = ready_tx.send(Err(e.to_string()));
                return;
            }
        };
        let mut backend = Backend {
            window: None,
            context: Some(context),
            surface: None,
            init: Some(init),
            proxy: Some(proxy),
            ready_tx: Some(ready_tx),
            frame_rx,
            event_tx,
            pending: None,
            cell_w: 1,
            cell_h: 1,
            cols: 0,
            rows: 0,
            mouse: (0, 0),
        };

        let _ = event_loop.run_app(&mut backend);
    }

    /// Shut the window down and join the window thread.
    pub fn end(&mut self) {
        if !self.active {
            return;
        }
        self.active = false;
        let _ = self.frame_tx.send(FrameMsg::Shutdown);
        let _ = self.proxy.send_event(UserMsg::Wake);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
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

    /// Set the window title.
    pub fn set_title(&mut self, title: &str) {
        if self
            .frame_tx
            .send(FrameMsg::SetTitle(String::from(title)))
            .is_ok()
        {
            let _ = self.proxy.send_event(UserMsg::Wake);
        }
    }

    /// Rasterise and present the canvas.
    pub fn display(&mut self, canvas: &Canvas) {
        let w = canvas.width().max(0);
        let h = canvas.height().max(0);
        if w == 0 || h == 0 {
            return;
        }
        // Clamp absurd sizes before allocating the pixel buffer.
        if w as u64 * h as u64 > 512 * 512 {
            return;
        }

        let pw = (w * self.cell_w) as usize;
        let ph = (h * self.cell_h) as usize;
        let mut argb = vec![0u8; pw * ph * 4];
        if self
            .font
            .render_canvas(
                canvas,
                &mut argb,
                w * self.cell_w,
                h * self.cell_h,
                (4 * pw) as i32,
            )
            .is_err()
        {
            return;
        }

        // softbuffer expects 0x00RRGGBB pixels.
        let mut pixels = Vec::with_capacity(pw * ph);
        for px in argb.chunks_exact(4) {
            pixels.push(((px[1] as u32) << 16) | ((px[2] as u32) << 8) | (px[3] as u32));
        }

        let ok = self
            .frame_tx
            .send(FrameMsg::Render {
                pixels,
                width: pw as u32,
                height: ph as u32,
            })
            .is_ok();
        if ok {
            let _ = self.proxy.send_event(UserMsg::Wake);
        } else {
            self.active = false;
        }
    }

    /// No-op: the window size drives the canvas, not the other way around.
    pub fn handle_resize(&mut self) {}

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
            let remaining = match deadline {
                Some(d) => {
                    let now = Instant::now();
                    if now >= d {
                        return None;
                    }
                    Some(d - now)
                }
                None => None,
            };

            let next = match remaining {
                Some(left) => match self.event_rx.recv_timeout(left) {
                    Ok(ev) => Some(ev),
                    Err(RecvTimeoutError::Timeout) => return None,
                    Err(RecvTimeoutError::Disconnected) => {
                        self.active = false;
                        return None;
                    }
                },
                None => self.event_rx.recv().ok(),
            };

            match next {
                Some(ev) if ev.matches(mask) => {
                    if let Event::Resize { w, h } = ev {
                        self.cols = w;
                        self.rows = h;
                    }
                    return Some(ev);
                }
                Some(_) => continue,
                None => {
                    self.active = false;
                    return None;
                }
            }
        }
    }
}

impl Drop for Winit {
    fn drop(&mut self) {
        self.end();
    }
}
