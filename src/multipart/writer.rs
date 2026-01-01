//! Multipart writer implementation.

/// A multipart writer.
pub struct Writer {
    // TODO: Implement
}

impl Writer {
    /// Creates a new multipart writer.
    pub fn new<W>(w: W) -> Self
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        // TODO: Implement
        Self {}
    }

    /// Returns the writer's boundary.
    pub fn boundary(&self) -> &str {
        // TODO: Implement
        ""
    }
}
