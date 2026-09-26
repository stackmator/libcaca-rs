//! Canvas import/export codecs.
//!
//! Port of `caca/codec/import.c`, `caca/codec/export.c` and `caca/codec/text.c`.

mod html;
mod image;
mod native;
mod text;

use crate::canvas::Canvas;
use crate::error::{CacaError, Result};

pub(super) fn push_utf8(out: &mut Vec<u8>, ch: u32) {
    let (buf, n) = crate::charset::utf32_to_utf8_array(ch);
    out.extend_from_slice(&buf[..n]);
}

impl Canvas {
    /// Import a memory buffer into the current frame.
    pub fn import_from_memory(&mut self, data: &[u8], format: &str) -> Result<usize> {
        match format.to_ascii_lowercase().as_str() {
            "caca" | "import" => native::import_caca(self, data),
            "utf8" => text::import_ansi(self, data, true),
            "text" => text::import_text(self, data),
            "ansi" => text::import_ansi(self, data, false),
            "bin" => native::import_bin(self, data),
            "" => self.autodetect_import(data),
            _ => Err(CacaError::Invalid),
        }
    }

    /// Import a file into the current frame.
    pub fn import_from_file(&mut self, path: &std::path::Path, format: &str) -> Result<usize> {
        let data = std::fs::read(path).map_err(|_| CacaError::Invalid)?;
        self.import_from_memory(&data, format)
    }

    /// Import a memory buffer into the current frame at the given position.
    pub fn import_area_from_memory(
        &mut self,
        x: i32,
        y: i32,
        data: &[u8],
        format: &str,
    ) -> Result<usize> {
        let mut tmp = Canvas::new(0, 0)?;
        let ret = tmp.import_from_memory(data, format)?;

        if ret > 0 {
            let _ = self.blit(x, y, &tmp, None);
        }

        Ok(ret)
    }

    /// Import a file into the current frame at the given position.
    pub fn import_area_from_file(
        &mut self,
        x: i32,
        y: i32,
        path: &std::path::Path,
        format: &str,
    ) -> Result<usize> {
        let data = std::fs::read(path).map_err(|_| CacaError::Invalid)?;
        self.import_area_from_memory(x, y, &data, format)
    }

    /// Export the canvas into the requested format.
    pub fn export_to_memory(&self, format: &str) -> Result<Vec<u8>> {
        match format.to_ascii_lowercase().as_str() {
            "caca" => Ok(native::export_caca(self)),
            "ansi" => Ok(text::export_ansi(self)),
            "utf8" => Ok(text::export_utf8(self, false)),
            "utf8cr" => Ok(text::export_utf8(self, true)),
            "text" => Ok(text::export_text(self)),
            "html" => Ok(html::export_html(self, false)),
            "html3" => Ok(html::export_html3(self)),
            "html5" => Ok(html::export_html(self, true)),
            "bbfr" => Ok(html::export_bbfr(self)),
            "irc" => Ok(text::export_irc(self)),
            "ps" => Ok(image::export_ps(self)),
            "svg" => Ok(image::export_svg(self)),
            "tga" => image::export_tga(self),
            "troff" => Ok(html::export_troff(self)),
            _ => Err(CacaError::Invalid),
        }
    }

    /// Export a rectangular portion of the canvas into the requested format.
    pub fn export_area_to_memory(
        &self,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        format: &str,
    ) -> Result<Vec<u8>> {
        if w < 0 || h < 0 || x < 0 || y < 0 || x + w > self.width() || y + h > self.height() {
            return Err(CacaError::Invalid);
        }

        let mut tmp = Canvas::new(w, h)?;
        tmp.blit(-x, -y, self, None)?;
        tmp.export_to_memory(format)
    }

    fn autodetect_import(&mut self, data: &[u8]) -> Result<usize> {
        if data.len() >= 4
            && data[0] == 0xca
            && data[1] == 0xca
            && data[2] == b'C'
            && data[3] == b'V'
        {
            return native::import_caca(self, data);
        }

        for i in 0..data.len().saturating_sub(1) {
            if data[i] == 0x1b && data[i + 1] == b'[' {
                return text::import_ansi(self, data, false);
            }
        }

        let mut j = 0usize;
        let mut k = 0usize;
        let mut i = 0usize;

        while i < data.len() {
            if data[i] == b' ' {
                j += 1;
            }
            if i + 1 < data.len() && data[i + 1] == b' ' {
                k += 1;
            }
            i += 2;
        }

        if j > 10 && j > data.len() / 40 && k < 10 {
            return native::import_bin(self, data);
        }

        text::import_text(self, data)
    }
}

/// The list of supported import format names.
pub fn import_list() -> &'static [&'static str] {
    &["", "caca", "text", "ansi", "utf8", "bin"]
}

/// The list of supported export format names.
pub fn export_list() -> &'static [&'static str] {
    &[
        "caca", "ansi", "utf8", "utf8cr", "text", "html", "html3", "html5", "bbfr", "irc", "ps",
        "svg", "tga", "troff",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attr::Color;

    #[test]
    fn text_roundtrip() {
        let mut cv = Canvas::new(5, 3).unwrap();
        cv.put_str(0, 0, "hello");
        cv.put_str(0, 1, "world");

        let data = cv.export_to_memory("text").unwrap();
        assert_eq!(&data, b"hello\nworld\n     \n");

        let mut out = Canvas::new(0, 0).unwrap();
        let n = out.import_from_memory(&data, "text").unwrap();
        assert_eq!(n, data.len());
        assert_eq!(out.width(), 5);
        assert_eq!(out.height(), 3);
        assert_eq!(out.get_char(0, 0), b'h' as u32);
        assert_eq!(out.get_char(4, 1), b'd' as u32);
    }

    #[test]
    fn ansi_export_contains_escape_and_reimports() {
        let mut cv = Canvas::new(4, 1).unwrap();
        cv.set_color_ansi(Color::Red, Color::Black).unwrap();
        cv.put_str(0, 0, "test");

        let data = cv.export_to_memory("ansi").unwrap();
        assert!(data.windows(2).any(|w| w[0] == 0x1b && w[1] == b'['));

        let mut out = Canvas::new(0, 0).unwrap();
        out.import_from_memory(&data, "ansi").unwrap();
        assert_eq!(out.get_char(0, 0), b't' as u32);
        assert_eq!(out.get_char(1, 0), b'e' as u32);
        assert_eq!(out.get_char(2, 0), b's' as u32);
        assert_eq!(out.get_char(3, 0), b't' as u32);
    }

    #[test]
    fn caca_roundtrip() {
        let mut cv = Canvas::new(4, 2).unwrap();
        cv.put_char(1, 1, b'Z' as u32);
        cv.set_color_ansi(Color::Black, Color::White).unwrap();
        cv.put_char(3, 0, b'Q' as u32);

        let data = cv.export_to_memory("caca").unwrap();
        let mut out = Canvas::new(0, 0).unwrap();
        let n = out.import_from_memory(&data, "caca").unwrap();
        assert_eq!(n, data.len());
        assert_eq!(out.width(), 4);
        assert_eq!(out.height(), 2);
        assert_eq!(out.get_char(1, 1), b'Z' as u32);
        assert_eq!(out.get_char(3, 0), b'Q' as u32);
    }

    #[test]
    fn utf8_and_html_smoke() {
        let mut cv = Canvas::new(3, 1).unwrap();
        cv.put_str(0, 0, "<&>");

        assert!(!cv.export_to_memory("utf8").unwrap().is_empty());

        let html = cv.export_to_memory("html").unwrap();
        let s = String::from_utf8_lossy(&html);
        assert!(s.contains("&lt;"));
        assert!(s.contains("&amp;"));
        assert!(s.contains("&gt;"));

        assert!(!cv.export_to_memory("html3").unwrap().is_empty());
        assert!(!cv.export_to_memory("html5").unwrap().is_empty());
        assert!(!cv.export_to_memory("bbfr").unwrap().is_empty());
        assert!(!cv.export_to_memory("irc").unwrap().is_empty());
        assert!(!cv.export_to_memory("ps").unwrap().is_empty());
        assert!(!cv.export_to_memory("svg").unwrap().is_empty());

        let tga = cv.export_to_memory("tga").unwrap();
        assert!(tga.len() > 18);
        assert_eq!(tga[2], 2); // uncompressed truecolour
        assert_eq!(tga[16], 32); // pixel depth

        assert!(cv.export_to_memory("nope").is_err());
    }

    #[test]
    fn area_roundtrip() {
        let mut cv = Canvas::new(10, 5).unwrap();
        cv.put_char(7, 3, b'a' as u32);
        cv.put_char(9, 4, b'b' as u32);

        let data = cv.export_area_to_memory(2, 1, 6, 4, "caca").unwrap();

        let mut out = Canvas::new(6, 4).unwrap();
        out.import_area_from_memory(0, 0, &data, "caca").unwrap();
        assert_eq!(out.get_char(5, 2), b'a' as u32);
        assert_eq!(out.get_char(0, 0), b' ' as u32);
        assert_eq!(out.get_char(5, 3), b' ' as u32);
    }

    #[test]
    fn caca_multiframe_roundtrip() {
        let mut cv = Canvas::new(3, 1).unwrap();
        cv.put_str(0, 0, "one");
        cv.create_frame(1).unwrap();
        cv.set_frame(1).unwrap();
        cv.put_str(0, 0, "two");

        let data = cv.export_to_memory("caca").unwrap();
        let mut out = Canvas::new(0, 0).unwrap();
        out.import_from_memory(&data, "import").unwrap();
        assert_eq!(out.frame_count(), 2);
        assert_eq!(out.width(), 3);
        assert_eq!(out.height(), 1);
        assert_eq!(out.get_char(0, 0), b'o' as u32);
        out.set_frame(1).unwrap();
        assert_eq!(out.get_char(0, 0), b't' as u32);
        assert_eq!(out.get_char(2, 0), b'o' as u32);
    }

    #[test]
    fn import_export_lists_nonempty() {
        assert!(import_list().contains(&"caca"));
        assert!(import_list().contains(&"ansi"));
        assert!(import_list().contains(&"utf8"));
        assert!(import_list().contains(&"text"));
        assert!(export_list().contains(&"caca"));
        assert!(export_list().contains(&"html3"));
        assert!(export_list().contains(&"svg"));
        assert!(export_list().contains(&"tga"));
    }
}
