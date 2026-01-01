//! Quoted-printable reader.

/// A quoted-printable decoder.
pub struct Reader {
    // TODO: Implement
}

impl Reader {
    /// Creates a new quoted-printable reader.
    pub fn new<R>(r: R) -> Self
    where
        R: tokio::io::AsyncRead + Unpin,
    {
        // TODO: Implement
        Self {}
    }
}
