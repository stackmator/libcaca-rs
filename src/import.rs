//! Image-file loading for texture dithering.
//!
//! This ports `src/common-image.c`: where the C version decodes through
//! Imlib2 (falling back to a built-in BMP reader), this module decodes
//! through the pure-Rust [`image`](https://crates.io/crates/image) crate
//! and dithers with the same 32-bit layout Imlib2 produced — RGBA bytes,
//! i.e. native words with `R = 0x000000ff`, `G = 0x0000ff00`,
//! `B = 0x00ff0000`, `A = 0xff000000`.
//!
//! Like the C `struct image`, an [`Image`] owns its pixel buffer and its
//! [`Dither`]; both are released automatically on drop (no `unload_image`
//! needed). Enable with the `import` cargo feature (which implies `std`).

use std::path::Path;

use crate::canvas::Canvas;
use crate::dither::Dither;
use crate::error::{CacaError, Result};

/// A decoded image file, ready to dither onto a canvas.
pub struct Image {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
    dither: Dither,
}

impl Image {
    fn from_rgba(width: u32, height: u32, pixels: Vec<u8>) -> Result<Image> {
        let w = i32::try_from(width).map_err(|_| CacaError::Overflow)?;
        let h = i32::try_from(height).map_err(|_| CacaError::Overflow)?;
        let pitch = w
            .checked_mul(4)
            .filter(|&p| p >= 0)
            .ok_or(CacaError::Overflow)?;
        // RGBA byte order: R in the low byte, A in the high byte.
        let dither = Dither::new(
            w, h, 32, pitch, 0x000000ff, 0x0000ff00, 0x00ff0000, 0xff000000,
        )?;
        Ok(Image {
            width,
            height,
            pixels,
            dither,
        })
    }

    /// Decode an image file (format sniffed from magic bytes, like Imlib2).
    ///
    /// Fails with [`CacaError::Invalid`] when the file cannot be read or
    /// decoded (the C `load_image` returned `NULL`).
    pub fn load(path: impl AsRef<Path>) -> Result<Image> {
        let img = image::open(path).map_err(|_| CacaError::Invalid)?;
        let rgba = img.into_rgba8();
        let (width, height) = (rgba.width(), rgba.height());
        Image::from_rgba(width, height, rgba.into_raw())
    }

    /// Decode an image from memory (format sniffed from magic bytes).
    pub fn load_from_memory(data: &[u8]) -> Result<Image> {
        let img = image::load_from_memory(data).map_err(|_| CacaError::Invalid)?;
        let rgba = img.into_rgba8();
        let (width, height) = (rgba.width(), rgba.height());
        Image::from_rgba(width, height, rgba.into_raw())
    }

    /// The image width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// The image height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// The raw RGBA bytes, row-major, top to bottom.
    pub fn pixels_rgba(&self) -> &[u8] {
        &self.pixels
    }

    /// Select the dithering algorithm (e.g. `"random"`, as `trifiller`
    /// picks from the algorithm list).
    pub fn set_algorithm(&mut self, name: &str) -> Result<()> {
        self.dither.set_algorithm(name)
    }

    /// Dither the image onto a `w` × `h` region of the canvas at `(x, y)`
    /// (old `caca_dither_bitmap`).
    pub fn dither_onto(&self, cv: &mut Canvas, x: i32, y: i32, w: i32, h: i32) -> Result<()> {
        self.dither.dither_bitmap(cv, x, y, w, h, &self.pixels)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Encode a tiny PNG in memory so the tests need no fixture files.
    fn red_png() -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(4, 3, image::Rgba([255, 0, 0, 255]));
        let mut buf = Vec::new();
        let encoder = image::codecs::png::PngEncoder::new(&mut buf);
        img.write_with_encoder(encoder).unwrap();
        buf
    }

    #[test]
    fn memory_roundtrip_keeps_size_and_pixels() {
        let png = red_png();
        let im = Image::load_from_memory(&png).unwrap();
        assert_eq!((im.width(), im.height()), (4, 3));
        assert_eq!(im.pixels_rgba().len(), 4 * 3 * 4);
        // First pixel is opaque red in RGBA order.
        assert_eq!(&im.pixels_rgba()[..4], &[255, 0, 0, 255]);
    }

    #[test]
    fn garbage_is_rejected() {
        assert!(Image::load_from_memory(b"not an image").is_err());
        assert!(Image::load_from_memory(&[]).is_err());
    }

    #[test]
    fn dither_onto_paints_canvas() {
        let im = Image::load_from_memory(&red_png()).unwrap();
        let mut cv = Canvas::new(4, 3).unwrap();
        let before = cv.attrs().to_vec();
        im.dither_onto(&mut cv, 0, 0, 4, 3).unwrap();
        assert_ne!(cv.attrs(), before.as_slice());
    }

    #[test]
    fn algorithm_selection() {
        let mut im = Image::load_from_memory(&red_png()).unwrap();
        im.set_algorithm("random").unwrap();
        im.set_algorithm("none").unwrap();
        assert!(im.set_algorithm("bogus").is_err());
    }
}
