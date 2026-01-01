//! Platform-specific MIME type loading.

#[cfg(unix)]
pub mod unix;

#[cfg(windows)]
pub mod windows;

#[cfg(unix)]
pub use unix::init_mime;

#[cfg(windows)]
pub use windows::init_mime;
