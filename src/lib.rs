//! # libcaca — pure-Rust port
//!
//! A from-scratch Rust reimplementation of [libcaca], the Colour ASCII-Art
//! library. It is **not** a binding: no C code is linked, and the public API is
//! idiomatic safe Rust.
//!
//! The core abstraction is [`Canvas`], a grid of character cells with 32-bit
//! foreground/background attributes. On top of it the crate provides drawing
//! primitives, dithering, import/export codecs and terminal display drivers.
//!
//! ```
//! use libcaca::{Canvas, Color};
//!
//! let mut cv = Canvas::new(20, 5).unwrap();
//! cv.set_color_ansi(Color::Yellow, Color::Blue).unwrap();
//! cv.clear();
//! cv.put_str(2, 2, "Hello, caca!");
//! assert_eq!(cv.get_char(2, 2), b'H' as u32);
//! ```
//!
//! [libcaca]: https://github.com/cacalabs/libcaca

pub mod attr;
#[path = "box.rs"]
pub mod boxdraw;
pub mod canvas;
pub mod charset;
pub mod conic;
pub mod codec;
pub mod dirty;
pub mod display;
pub mod dither;
pub mod error;
pub mod font;
pub mod frame;
pub mod line;
pub mod string;
pub mod transform;
pub(crate) mod transform_tables;
pub mod triangle;

pub use attr::{Attr, Color, Style};
pub use canvas::{rand, Canvas, CACA_MAGIC_FULLWIDTH};
pub use display::{key, Display, Driver, Event, EventMask, KeyEvent};
pub use error::{CacaError, Result};
