//! Complete Rust port of Go's mime package with async-first design.
//!
//! This crate provides comprehensive MIME functionality including:
//! - MIME type detection by file extension
//! - Media type parsing and formatting (RFC 2045, RFC 2616, RFC 2231)
//! - RFC 2047 encoded-word encoding and decoding
//! - Multipart MIME parsing and writing (RFC 2046, RFC 2388)
//! - Quoted-printable encoding (RFC 2045)
//!
//! All I/O operations are async-first using tokio.

pub mod error;
pub mod grammar;
pub mod mime_type;
pub mod media_type;
pub mod encoded_word;
pub mod multipart;
pub mod quotedprintable;

#[cfg(unix)]
pub mod platform;

#[cfg(windows)]
pub mod platform;

// Re-export commonly used types
pub use error::{Error, Result};
pub use mime_type::{type_by_extension, extensions_by_type, add_extension_type};
pub use media_type::{parse_media_type, format_media_type};
pub use encoded_word::{WordEncoder, WordDecoder};
