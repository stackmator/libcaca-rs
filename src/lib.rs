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
//!
//! # `no_std` support
//!
//! Without the default `std` feature this crate is `no_std` + `alloc`: the
//! canvas, codec, dither, font, event-parsing and ANSI-rendering core keeps
//! working. The OS-interactive layer — display drivers (`Display`, `Driver`),
//! the `Conio` console, file-based helpers (`import_from_file`,
//! `FigFont::load`) and wall-clock framerate timing — requires `std`.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod attr;
#[path = "box.rs"]
pub mod boxdraw;
pub mod canvas;
pub mod charset;
pub mod codec;
#[cfg(feature = "compat")]
pub mod compat;
pub mod conic;
#[cfg(feature = "std")]
pub mod conio;
pub mod dirty;
pub mod display;
pub mod dither;
pub mod error;
pub mod figfont;
#[cfg(feature = "std")]
pub mod file;
pub mod font;
pub mod frame;
pub mod getopt;
#[cfg(feature = "import")]
pub mod import;
pub mod line;
pub mod string;
pub mod transform;
pub(crate) mod transform_tables;
pub mod triangle;

pub use attr::{Attr, Color, Style};
pub use canvas::{rand, Canvas, CACA_MAGIC_FULLWIDTH};
#[cfg(feature = "compat")]
pub use compat::Compat;
#[cfg(feature = "std")]
pub use conio::Conio;
pub use display::{key, Event, EventMask, KeyEvent};
#[cfg(feature = "std")]
pub use display::{Display, Driver};
pub use dither::Dither;
pub use error::{CacaError, Result};
pub use figfont::FigFont;
#[cfg(feature = "std")]
pub use file::File;
pub use font::Font;
#[cfg(feature = "import")]
pub use import::Image;
