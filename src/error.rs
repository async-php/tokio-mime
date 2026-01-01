//! Error types for the mime crate.

use std::io;
use thiserror::Error;

/// The main error type for the mime crate.
#[derive(Error, Debug)]
pub enum Error {
    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    /// MIME type error
    #[error("MIME type error: {0}")]
    MimeType(String),

    /// Media type error
    #[error("Media type error: {0}")]
    MediaType(String),

    /// Encoding error
    #[error("Encoding error: {0}")]
    Encoding(String),

    /// Multipart error
    #[error("Multipart error: {0}")]
    Multipart(String),

    /// Invalid parameter
    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    /// Message too large
    #[error("Message too large")]
    MessageTooLarge,
}

/// Specialized Result type for mime operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Error indicating invalid media parameter (used in media type parsing).
#[derive(Error, Debug)]
#[error("Invalid media parameter")]
pub struct InvalidMediaParameter;

impl From<InvalidMediaParameter> for Error {
    fn from(_: InvalidMediaParameter) -> Self {
        Error::MediaType("invalid media parameter".to_string())
    }
}
