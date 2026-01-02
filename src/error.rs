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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[test]
    fn test_error_display() {
        // Test MimeType error
        let err = Error::MimeType("invalid type".to_string());
        assert_eq!(err.to_string(), "MIME type error: invalid type");

        // Test MediaType error
        let err = Error::MediaType("invalid media".to_string());
        assert_eq!(err.to_string(), "Media type error: invalid media");

        // Test Encoding error
        let err = Error::Encoding("invalid encoding".to_string());
        assert_eq!(err.to_string(), "Encoding error: invalid encoding");

        // Test Multipart error
        let err = Error::Multipart("invalid multipart".to_string());
        assert_eq!(err.to_string(), "Multipart error: invalid multipart");

        // Test InvalidParameter error
        let err = Error::InvalidParameter("invalid param".to_string());
        assert_eq!(err.to_string(), "Invalid parameter: invalid param");

        // Test MessageTooLarge error
        let err = Error::MessageTooLarge;
        assert_eq!(err.to_string(), "Message too large");
    }

    #[test]
    fn test_io_error_conversion() {
        // Test IO error conversion
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let err: Error = io_err.into();
        assert!(matches!(err, Error::Io(_)));
        assert!(err.to_string().contains("file not found"));
    }

    #[test]
    fn test_invalid_media_parameter_conversion() {
        // Test InvalidMediaParameter conversion
        let param_err = InvalidMediaParameter;
        let err: Error = param_err.into();
        assert!(matches!(err, Error::MediaType(_)));
        assert_eq!(err.to_string(), "Media type error: invalid media parameter");
    }

    #[test]
    fn test_error_debug() {
        // Test that errors implement Debug
        let err = Error::MimeType("test".to_string());
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("MimeType"));
    }

    #[test]
    fn test_result_type() {
        // Test Result type alias
        let ok_result: Result<i32> = Ok(42);
        assert_eq!(ok_result.unwrap(), 42);

        let err_result: Result<i32> = Err(Error::MimeType("error".to_string()));
        assert!(err_result.is_err());
    }
}
