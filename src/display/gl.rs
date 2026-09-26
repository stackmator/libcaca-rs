//! OpenGL window driver.
//!
//! A pure-Rust equivalent of libcaca's GLUT driver: a `winit` window hosts an
//! OpenGL context (via `glutin`), and the rasterised canvas is uploaded as a
//! texture and drawn as a fullscreen quad (via `glow`). Enable with the `gl`
//! cargo feature and select the `gl` driver explicitly (never autodetected).
//!
//! Like the `winit` driver, the event loop runs on a background thread and
//! frames/events cross channels. Compile-verified; needs a GPU (or software
//! GL) and a display server for a runtime test.

#![cfg(feature = "gl")]

use std::ffi::CString;
use std::num::NonZeroU32;
use std::rc::Rc;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use glow::HasContext;
use glutin::config::ConfigTemplateBuilder;
use glutin::context::{ContextAttributesBuilder, PossiblyCurrentContext};
use glutin::display::GetGlDisplay;
use glutin::prelude::*;
use glutin::surface::{Surface as GlutinSurface, SurfaceAttributesBuilder, WindowSurface};
use raw_window_handle::HasWindowHandle;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, KeyEvent as WinitKeyEvent, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

use crate::canvas::Canvas;
use crate::display::event::{key, Event, EventMask, KeyEvent};
use crate::error::{CacaError, Result};
use crate::font::{font_list, load_builtin, Font};

/// Fullscreen quad: x, y, u, v per vertex (triangle strip). V is flipped
/// because GL textures start at the bottom row while our pixels start at top.
const QUAD: [f32; 16] = [
    -1.0, -1.0, 0.0, 1.0, //
    1.0, -1.0, 1.0, 1.0, //
    -1.0, 1.0, 0.0, 0.0, //
    1.0, 1.0, 1.0, 0.0, //
];

const VERTEX_SRC: &str = "attribute vec2 pos;\n\
                           attribute vec2 uv;\n\
                           varying vec2 v_uv;\n\
                           void main() {\n\
                           \x20  v_uv = uv;\n\
                           \x20  gl_Position = vec4(pos, 0.0, 1.0);\n\
                           }\n";

const FRAGMENT_SRC: &str = "varying vec2 v_uv;\n\
                             uniform sampler2D tex;\n\
                             void main() {\n\
                             \x20  gl_FragColor = texture2D(tex, v_uv);\n\
                             }\n";

/// Messages from the display thread to the window thread.
enum FrameMsg {
    Render {
        pixels: Vec<u8>,
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

/// Window-thread GL state. Never leaves its thread.
struct Backend {
    window: Option<Rc<Window>>,
    gl: Option<glow::Context>,
    gl_context: Option<PossiblyCurrentContext>,
    gl_surface: Option<GlutinSurface<WindowSurface>>,
    gl_config: Option<glutin::config::Config>,
    program: Option<glow::NativeProgram>,
    texture: Option<glow::NativeTexture>,
    tex_w: u32,
    tex_h: u32,
    init: Option<(String, u32, u32, i32, i32)>,
    proxy: Option<EventLoopProxy<UserMsg>>,
    ready_tx: Option<mpsc::Sender<ReadyResult>>,
    frame_rx: mpsc::Receiver<FrameMsg>,
    event_tx: mpsc::Sender<Event>,
    pending: Option<(Vec<u8>, u32, u32)>,
    cell_w: i32,
    cell_h: i32,
    cols: i32,
    rows: i32,
    mouse: (i32, i32),
}

/// Window-creation outcome sent back to the display thread.
type ReadyResult = std::result::Result<(EventLoopProxy<UserMsg>, u32, u32), String>;

impl Backend {
    fn send_event(&self, ev: Event) {
        let _ = self.event_tx.send(ev);
    }

    fn present(&mut self) {
        let (Some(window), Some(gl), Some(program), Some(texture)) = (
            self.window.as_ref(),
            self.gl.as_ref(),
            self.program,
            self.texture,
        ) else {
            return;
        };
        let Some((pixels, pw, ph)) = self.pending.take() else {
            return;
        };

        // SAFETY: GL context is current on this thread (made current at
        // setup and never moved).
        unsafe {
            if pw != self.tex_w || ph != self.tex_h {
                gl.bind_texture(glow::TEXTURE_2D, Some(texture));
                gl.tex_image_2d(
                    glow::TEXTURE_2D,
                    0,
                    glow::RGBA8 as i32,
                    pw as i32,
                    ph as i32,
                    0,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    glow::PixelUnpackData::Slice(Some(&pixels)),
                );
                self.tex_w = pw;
                self.tex_h = ph;
            } else {
                gl.bind_texture(glow::TEXTURE_2D, Some(texture));
                gl.tex_sub_image_2d(
                    glow::TEXTURE_2D,
                    0,
                    0,
                    0,
                    pw as i32,
                    ph as i32,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    glow::PixelUnpackData::Slice(Some(&pixels)),
                );
            }

            let size = window.inner_size();
            gl.viewport(0, 0, size.width as i32, size.height as i32);
            gl.clear_color(0.0, 0.0, 0.0, 1.0);
            gl.clear(glow::COLOR_BUFFER_BIT);
            gl.use_program(Some(program));
            gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
        }

        if let (Some(surface), Some(context)) = (self.gl_surface.as_ref(), self.gl_context.as_ref())
        {
            let _ = surface.swap_buffers(context);
        }
    }

    fn map_key(&self, ev: &WinitKeyEvent, pressed: bool) -> Option<Event> {
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
        if self.window.is_some() {
            return;
        }
        let Some((title, pw, ph, cell_w, cell_h)) = self.init.take() else {
            return;
        };
        self.cell_w = cell_w;
        self.cell_h = cell_h;

        let window_attrs = Window::default_attributes()
            .with_title(title)
            .with_inner_size(PhysicalSize::new(pw.max(1), ph.max(1)));
        let template = ConfigTemplateBuilder::new().with_alpha_size(8);
        let (window_opt, gl_config) = match glutin_winit::DisplayBuilder::new()
            .with_window_attributes(Some(window_attrs))
            .build(event_loop, template, |configs| {
                configs
                    .reduce(|best, config| {
                        if config.num_samples() > best.num_samples() {
                            config
                        } else {
                            best
                        }
                    })
                    .unwrap()
            }) {
            Ok(pair) => pair,
            Err(e) => {
                if let Some(tx) = self.ready_tx.take() {
                    let _ = tx.send(Err(e.to_string()));
                }
                event_loop.exit();
                return;
            }
        };
        let window = match window_opt {
            Some(window) => Rc::new(window),
            None => {
                if let Some(tx) = self.ready_tx.take() {
                    let _ = tx.send(Err(String::from("no window")));
                }
                event_loop.exit();
                return;
            }
        };

        let raw_handle = match window.window_handle() {
            Ok(handle) => handle.as_raw(),
            Err(e) => {
                if let Some(tx) = self.ready_tx.take() {
                    let _ = tx.send(Err(e.to_string()));
                }
                event_loop.exit();
                return;
            }
        };
        let size = window.inner_size();
        let attrs = SurfaceAttributesBuilder::<WindowSurface>::new().build(
            raw_handle,
            NonZeroU32::new(size.width.max(1)).unwrap(),
            NonZeroU32::new(size.height.max(1)).unwrap(),
        );
        // SAFETY: window and config live on this thread for the backend's life.
        unsafe {
            let surface = match gl_config
                .display()
                .create_window_surface(&gl_config, &attrs)
            {
                Ok(surface) => surface,
                Err(e) => {
                    if let Some(tx) = self.ready_tx.take() {
                        let _ = tx.send(Err(e.to_string()));
                    }
                    event_loop.exit();
                    return;
                }
            };
            let context = match gl_config
                .display()
                .create_context(&gl_config, &ContextAttributesBuilder::new().build(None))
            {
                Ok(context) => context,
                Err(e) => {
                    if let Some(tx) = self.ready_tx.take() {
                        let _ = tx.send(Err(e.to_string()));
                    }
                    event_loop.exit();
                    return;
                }
            };
            let context = match context.make_current(&surface) {
                Ok(context) => context,
                Err(e) => {
                    if let Some(tx) = self.ready_tx.take() {
                        let _ = tx.send(Err(e.to_string()));
                    }
                    event_loop.exit();
                    return;
                }
            };

            let gl = glow::Context::from_loader_function(|s| {
                let name = CString::new(s).unwrap_or_default();
                gl_config.display().get_proc_address(&name)
            });

            let (program, texture) = match Self::init_resources(&gl) {
                Ok(handles) => handles,
                Err(e) => {
                    if let Some(tx) = self.ready_tx.take() {
                        let _ = tx.send(Err(e));
                    }
                    event_loop.exit();
                    return;
                }
            };

            self.gl = Some(gl);
            self.program = Some(program);
            self.texture = Some(texture);
            self.gl_context = Some(context);
            self.gl_surface = Some(surface);
            self.gl_config = Some(gl_config);
        }

        self.window = Some(window.clone());

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
                if let (Some(surface), Some(context)) =
                    (self.gl_surface.as_ref(), self.gl_context.as_ref())
                {
                    surface.resize(
                        context,
                        NonZeroU32::new(size.width.max(1)).unwrap(),
                        NonZeroU32::new(size.height.max(1)).unwrap(),
                    );
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

impl Backend {
    /// Compile shaders, upload the quad and create the texture, returning
    /// the program and texture handles for `present`.
    fn init_resources(
        gl: &glow::Context,
    ) -> std::result::Result<(glow::NativeProgram, glow::NativeTexture), String> {
        // SAFETY: context is current on this thread.
        unsafe {
            let program = gl.create_program().map_err(|e| e.to_string())?;
            let vs = gl
                .create_shader(glow::VERTEX_SHADER)
                .map_err(|e| e.to_string())?;
            gl.shader_source(vs, VERTEX_SRC);
            gl.compile_shader(vs);
            if !gl.get_shader_compile_status(vs) {
                return Err(gl.get_shader_info_log(vs));
            }
            let fs = gl
                .create_shader(glow::FRAGMENT_SHADER)
                .map_err(|e| e.to_string())?;
            gl.shader_source(fs, FRAGMENT_SRC);
            gl.compile_shader(fs);
            if !gl.get_shader_compile_status(fs) {
                return Err(gl.get_shader_info_log(fs));
            }
            gl.attach_shader(program, vs);
            gl.attach_shader(program, fs);
            gl.link_program(program);
            if !gl.get_program_link_status(program) {
                return Err(gl.get_program_info_log(program));
            }
            gl.detach_shader(program, vs);
            gl.detach_shader(program, fs);
            gl.delete_shader(vs);
            gl.delete_shader(fs);

            let vao = gl.create_vertex_array().map_err(|e| e.to_string())?;
            gl.bind_vertex_array(Some(vao));
            let vbo = gl.create_buffer().map_err(|e| e.to_string())?;
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
            let quad_bytes: &[u8] =
                std::slice::from_raw_parts(QUAD.as_ptr() as *const u8, QUAD.len() * 4);
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, quad_bytes, glow::STATIC_DRAW);
            gl.enable_vertex_attrib_array(0);
            gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 16, 0);
            gl.enable_vertex_attrib_array(1);
            gl.vertex_attrib_pointer_f32(1, 2, glow::FLOAT, false, 16, 8);
            // VAO/VBO stay bound for the context lifetime; they die with it.

            let tex = gl.create_texture().map_err(|e| e.to_string())?;
            gl.bind_texture(glow::TEXTURE_2D, Some(tex));
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MIN_FILTER,
                glow::NEAREST as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MAG_FILTER,
                glow::NEAREST as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_S,
                glow::CLAMP_TO_EDGE as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_T,
                glow::CLAMP_TO_EDGE as i32,
            );
            if let Some(loc) = gl.get_uniform_location(program, "tex") {
                gl.use_program(Some(program));
                gl.uniform_1_i32(Some(&loc), 0);
            }

            Ok((program, tex))
        }
    }
}

/// Graphical GL window driver state (main-thread side).
pub struct Gl {
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

impl Gl {
    /// Open the window and size `canvas` to it.
    pub fn new(canvas: &mut Canvas) -> Result<Gl> {
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

        Ok(Gl {
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
        let mut backend = Backend {
            window: None,
            gl: None,
            gl_context: None,
            gl_surface: None,
            gl_config: None,
            program: None,
            texture: None,
            tex_w: 0,
            tex_h: 0,
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

        // GL unpacks bottom-row-first RGBA bytes; flip vertically on upload.
        let mut pixels = vec![0u8; pw * ph * 4];
        for (dst_row, src_row) in (0..ph).enumerate() {
            let src = (ph - 1 - src_row) * pw * 4;
            let dst = dst_row * pw * 4;
            pixels[dst..dst + pw * 4].copy_from_slice(&argb[src..src + pw * 4]);
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

impl Drop for Gl {
    fn drop(&mut self) {
        self.end();
    }
}
