//! Quoted-printable writer.

/// A quoted-printable encoder.
pub struct Writer {
    /// Binary mode treats the writer's input as pure binary.
    pub binary: bool,
}

impl Writer {
    /// Creates a new quoted-printable writer.
    pub fn new<W>(w: W) -> Self
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        // TODO: Implement
        Self { binary: false }
    }
}
