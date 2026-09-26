//! Error types for the `libcaca` port.
//!
//! The original C library communicates failures through `errno` and `-1` /
//! `NULL` return values. The Rust port uses a proper [`Result`] instead.

use core::fmt;

/// Errors that can be produced by fallible canvas and display operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacaError {
    /// An argument was invalid (`EINVAL` in the C library).
    Invalid,
    /// A size computation overflowed (`EOVERFLOW` in the C library).
    Overflow,
    /// The allocation failed (`ENOMEM` in the C library).
    NoMem,
    /// The resource is in use (`EBUSY` in the C library).
    Busy,
    /// The operation is not supported by the active driver (`ENOSYS`).
    NotImplemented,
}

impl CacaError {
    /// The `errno`-style name historically associated with this error.
    pub fn errno_name(self) -> &'static str {
        match self {
            CacaError::Invalid => "EINVAL",
            CacaError::Overflow => "EOVERFLOW",
            CacaError::NoMem => "ENOMEM",
            CacaError::Busy => "EBUSY",
            CacaError::NotImplemented => "ENOSYS",
        }
    }

    /// The POSIX `errno` value for this error on the current platform.
    pub fn errno(self) -> i32 {
        // Values are stable across the platforms supported by libcaca.
        match self {
            CacaError::Invalid => 22,
            CacaError::Overflow => 75,
            CacaError::NoMem => 12,
            CacaError::Busy => 16,
            CacaError::NotImplemented => 38,
        }
    }
}

impl fmt::Display for CacaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.errno_name())
    }
}

#[cfg(feature = "std")]
impl std::error::Error for CacaError {}

/// Convenience alias used throughout the crate.
pub type Result<T> = core::result::Result<T, CacaError>;
