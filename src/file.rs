//! Platform-independent file I/O with transparent decompression.
//!
//! Port of `caca/file.c`. Without the `compression` cargo feature this is a
//! thin wrapper over [`std::fs`]; with it, gzip streams and the first file of
//! ZIP archives are decompressed on the fly, mirroring `HAVE_ZLIB_H`.
//! Compressed output (gzip) is produced on write, like `gzopen("wb")`.
//!
//! Requires the `std` feature.
//!
//! ```
//! use libcaca::file::File;
//!
//! let path = std::env::temp_dir().join("libcaca-doctest.txt");
//! let mut f = File::open(&path, "w").unwrap();
//! assert_eq!(f.write(b"hello"), 5);
//! f.close().unwrap();
//!
//! let mut f = File::open(&path, "r").unwrap();
//! let mut buf = [0u8; 5];
//! assert_eq!(f.read(&mut buf), 5);
//! assert_eq!(&buf, b"hello");
//! assert!(f.eof());
//! std::fs::remove_file(&path).unwrap();
//! ```

use std::fs;
use std::io::Write;
#[cfg(feature = "compression")]
use std::io::Read;
use std::path::Path;

use crate::error::{CacaError, Result};

#[cfg(feature = "compression")]
use flate2::read::GzDecoder;
#[cfg(feature = "compression")]
use flate2::write::GzEncoder;
#[cfg(feature = "compression")]
use flate2::{Compression, Decompress, FlushDecompress, Status};

enum Inner {
    Read {
        data: Vec<u8>,
        pos: usize,
    },
    #[cfg(not(feature = "compression"))]
    Write {
        file: fs::File,
        written: u64,
    },
    #[cfg(feature = "compression")]
    WriteGzip {
        enc: GzEncoder<fs::File>,
        written: u64,
    },
}

/// A libcaca file handle.
pub struct File {
    inner: Inner,
    readonly: bool,
}

impl File {
    /// Open a file for reading (`"r"`) or writing. Compressed input is
    /// detected by magic bytes when the `compression` feature is on.
    pub fn open(path: &Path, mode: &str) -> Result<File> {
        let readonly = mode.contains('r');

        if readonly {
            let data = fs::read(path).map_err(|_| CacaError::Invalid)?;
            #[cfg(feature = "compression")]
            let data = decompress(&data);
            return Ok(File {
                inner: Inner::Read { data, pos: 0 },
                readonly: true,
            });
        }

        let file = fs::File::create(path).map_err(|_| CacaError::Invalid)?;
        #[cfg(feature = "compression")]
        {
            Ok(File {
                inner: Inner::WriteGzip {
                    enc: GzEncoder::new(file, Compression::default()),
                    written: 0,
                },
                readonly: false,
            })
        }
        #[cfg(not(feature = "compression"))]
        {
            Ok(File {
                inner: Inner::Write { file, written: 0 },
                readonly: false,
            })
        }
    }

    /// Close the handle, finalising compressed output. Like `fclose`, this
    /// must be called; dropping without closing may truncate gzip output.
    pub fn close(self) -> Result<()> {
        match self.inner {
            Inner::Read { .. } => Ok(()),
            #[cfg(not(feature = "compression"))]
            Inner::Write { mut file, .. } => {
                file.flush().map_err(|_| CacaError::Invalid)?;
                Ok(())
            }
            #[cfg(feature = "compression")]
            Inner::WriteGzip { enc, .. } => {
                enc.finish().map_err(|_| CacaError::Invalid)?;
                Ok(())
            }
        }
    }

    /// The current byte offset.
    pub fn tell(&self) -> u64 {
        match &self.inner {
            Inner::Read { pos, .. } => *pos as u64,
            #[cfg(not(feature = "compression"))]
            Inner::Write { written, .. } => *written,
            #[cfg(feature = "compression")]
            Inner::WriteGzip { written, .. } => *written,
        }
    }

    /// Read up to `buf.len()` bytes, returning the count (0 at EOF).
    pub fn read(&mut self, buf: &mut [u8]) -> usize {
        match &mut self.inner {
            Inner::Read { data, pos } => {
                let n = (data.len() - *pos).min(buf.len());
                buf[..n].copy_from_slice(&data[*pos..*pos + n]);
                *pos += n;
                n
            }
            _ => 0,
        }
    }

    /// Write bytes, returning the count (0 on read-only handles, as in C).
    pub fn write(&mut self, buf: &[u8]) -> usize {
        if self.readonly {
            return 0;
        }
        match &mut self.inner {
            #[cfg(not(feature = "compression"))]
            Inner::Write { file, written } => match file.write(buf) {
                Ok(n) => {
                    *written += n as u64;
                    n
                }
                Err(_) => 0,
            },
            #[cfg(feature = "compression")]
            Inner::WriteGzip { enc, written } => match enc.write(buf) {
                Ok(n) => {
                    *written += n as u64;
                    n
                }
                Err(_) => 0,
            },
            _ => 0,
        }
    }

    /// Read a line (up to `max - 1` bytes, newline included like `fgets`).
    /// Returns `None` at EOF or on write-only handles.
    pub fn gets(&mut self, max: usize) -> Option<Vec<u8>> {
        if max == 0 {
            return None;
        }
        let (data, pos) = match &mut self.inner {
            Inner::Read { data, pos } => (data, pos),
            _ => return None,
        };
        if *pos >= data.len() {
            return None;
        }
        let mut out = Vec::new();
        while out.len() + 1 < max && *pos < data.len() {
            let b = data[*pos];
            *pos += 1;
            out.push(b);
            if b == b'\n' {
                break;
            }
        }
        Some(out)
    }

    /// Whether end of file was reached (always false for writers).
    pub fn eof(&self) -> bool {
        match &self.inner {
            Inner::Read { data, pos } => *pos >= data.len(),
            _ => false,
        }
    }

    /// Whether the handle was opened read-only.
    pub fn is_readonly(&self) -> bool {
        self.readonly
    }
}

/// Read a whole file, decompressing transparently when enabled.
///
/// Used by the canvas importer and the FIGfont loader, mirroring how the C
/// code funnels everything through `caca_file_open`.
pub(crate) fn read_all(path: &Path) -> Result<Vec<u8>> {
    let mut f = File::open(path, "r")?;
    let mut out = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = f.read(&mut buf);
        if n == 0 {
            break;
        }
        out.extend_from_slice(&buf[..n]);
    }
    Ok(out)
}

/// Decompress gzip or first-file-ZIP data; fall back to the raw bytes when
/// the input is plain (or corrupt), like a failed `gzread` surfacing later.
#[cfg(feature = "compression")]
fn decompress(data: &[u8]) -> Vec<u8> {
    if data.len() >= 2 && data[0] == 0x1f && data[1] == 0x8b {
        let mut dec = GzDecoder::new(data);
        let mut out = Vec::new();
        if dec.read_to_end(&mut out).is_ok() {
            return out;
        }
    }

    if data.len() >= 4 && data[0..4] == [b'P', b'K', 3, 4] {
        if let Some(out) = unzip_first(data) {
            return out;
        }
    }

    data.to_vec()
}

/// Extract the first file of a ZIP archive (stored or deflated entries).
#[cfg(feature = "compression")]
fn unzip_first(data: &[u8]) -> Option<Vec<u8>> {
    if data.len() < 30 {
        return None;
    }
    let method = u16::from_le_bytes([data[8], data[9]]);
    let name_len = u16::from_le_bytes([data[26], data[27]]) as usize;
    let extra_len = u16::from_le_bytes([data[28], data[29]]) as usize;
    let start = 30 + name_len + extra_len;
    if start > data.len() {
        return None;
    }
    let payload = &data[start..];

    match method {
        0 => Some(payload.to_vec()),
        8 => {
            // Raw deflate, inflated in a loop like the C `zipread`.
            let mut dec = Decompress::new(false);
            let mut out = Vec::new();
            let mut remaining = payload;
            let mut tmp = [0u8; 32768];
            loop {
                let in_before = dec.total_in();
                let out_before = dec.total_out();
                match dec.decompress(remaining, &mut tmp, FlushDecompress::Finish) {
                    Ok(Status::StreamEnd) => {
                        let produced = (dec.total_out() - out_before) as usize;
                        out.extend_from_slice(&tmp[..produced]);
                        return Some(out);
                    }
                    Ok(_) => {
                        let consumed = (dec.total_in() - in_before) as usize;
                        let produced = (dec.total_out() - out_before) as usize;
                        out.extend_from_slice(&tmp[..produced]);
                        remaining = &remaining[consumed.min(remaining.len())..];
                        if remaining.is_empty() || (consumed == 0 && produced == 0) {
                            return None;
                        }
                    }
                    Err(_) => return None,
                }
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_path(tag: &str) -> std::path::PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "libcaca-file-test-{}-{}-{}.tmp",
            tag,
            std::process::id(),
            n
        ))
    }

    #[test]
    fn plain_roundtrip() {
        let path = temp_path("plain");
        let _ = std::fs::remove_file(&path);

        let mut f = File::open(&path, "w").unwrap();
        assert!(!f.is_readonly());
        #[cfg(not(feature = "compression"))]
        assert_eq!(f.write(b"hello\nworld\n"), 12);
        #[cfg(feature = "compression")]
        assert_eq!(f.write(b"hello\nworld\n"), 12);
        assert_eq!(f.tell(), 12);
        f.close().unwrap();

        let mut f = File::open(&path, "r").unwrap();
        assert!(f.is_readonly());
        assert_eq!(f.write(b"x"), 0);
        assert_eq!(f.gets(100), Some(b"hello\n".to_vec()));
        assert_eq!(f.tell(), 6);
        assert_eq!(f.gets(100), Some(b"world\n".to_vec()));
        assert!(f.eof());
        assert_eq!(f.gets(100), None);

        let mut f = File::open(&path, "r").unwrap();
        let mut buf = [0u8; 4];
        assert_eq!(f.read(&mut buf), 4);
        assert_eq!(&buf, b"hell");
        assert!(!f.eof());

        assert!(File::open(&temp_path("missing"), "r").is_err());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    #[cfg(feature = "compression")]
    fn gzip_roundtrip() {
        let path = temp_path("gzip");
        let _ = std::fs::remove_file(&path);

        let mut f = File::open(&path, "w").unwrap();
        assert_eq!(f.write(b"hello gzip\n"), 11);
        f.close().unwrap();

        // The file on disk really is gzip.
        let raw = std::fs::read(&path).unwrap();
        assert_eq!(&raw[0..2], &[0x1f, 0x8b]);

        let data = read_all(&path).unwrap();
        assert_eq!(data, b"hello gzip\n");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    #[cfg(feature = "compression")]
    fn zip_first_file() {
        use flate2::write::DeflateEncoder;

        // Build a minimal ZIP with one deflated entry by hand.
        let mut payload = Vec::new();
        {
            let mut enc = DeflateEncoder::new(&mut payload, Compression::default());
            use std::io::Write as _;
            enc.write_all(b"zip payload").unwrap();
            enc.finish().unwrap();
        }
        let mut zip = Vec::new();
        zip.extend_from_slice(b"PK\x03\x04");
        zip.extend_from_slice(&[0u8; 22]); // version/flags/method/time/crc/sizes
        zip[8] = 8; // method = deflate
        zip.extend_from_slice(&(4u16.to_le_bytes())); // name len
        zip.extend_from_slice(&(0u16.to_le_bytes())); // extra len
        zip.extend_from_slice(b"name");
        zip.extend_from_slice(&payload);

        assert_eq!(unzip_first(&zip), Some(b"zip payload".to_vec()));
        assert_eq!(unzip_first(b"PK\x03\x04"), None);

        // A stored (uncompressed) entry passes through as-is.
        let mut stored = Vec::new();
        stored.extend_from_slice(b"PK\x03\x04");
        stored.extend_from_slice(&[0u8; 22]);
        stored.extend_from_slice(&(1u16.to_le_bytes()));
        stored.extend_from_slice(&(0u16.to_le_bytes()));
        stored.extend_from_slice(b"n");
        stored.extend_from_slice(b"raw bytes");
        assert_eq!(unzip_first(&stored), Some(b"raw bytes".to_vec()));
    }
}
