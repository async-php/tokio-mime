//! Multipart reader implementation.

use crate::error::Result;

/// A multipart reader.
pub struct Reader {
    // TODO: Implement
}

/// A single part in a multipart body.
pub struct Part {
    // TODO: Implement
}

impl Reader {
    /// Creates a new multipart reader.
    pub fn new<R>(r: R, boundary: &str) -> Self
    where
        R: tokio::io::AsyncRead + Unpin,
    {
        // TODO: Implement
        Self {}
    }

    /// Returns the next part in the multipart or an error.
    pub async fn next_part(&mut self) -> Result<Option<Part>> {
        // TODO: Implement
        Ok(None)
    }
}
