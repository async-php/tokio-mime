//! Quoted-printable writer.
//!
//! Implements RFC 2045 quoted-printable encoding with async I/O.

use pin_project::pin_project;
use std::io;
use std::pin::Pin;
use std::task::{ready, Context, Poll};
use tokio::io::AsyncWrite;

const LINE_MAX_LEN: usize = 76;
const UPPER_HEX: &[u8] = b"0123456789ABCDEF";

/// A quoted-printable encoder.
///
/// Implements `AsyncWrite` to encode data to quoted-printable on the fly.
#[pin_project]
pub struct Writer<W> {
    #[pin]
    inner: W,
    /// Binary mode treats input as pure binary (doesn't handle line endings specially).
    pub binary: bool,
    line: [u8; 78], // Buffer for current line (76 + CRLF)
    line_len: usize,
    pending_cr: bool,
}

impl<W: AsyncWrite> Writer<W> {
    /// Creates a new quoted-printable writer.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use mime_rs::quotedprintable::Writer;
    /// use tokio::io::AsyncWriteExt;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut output = Vec::new();
    /// let mut writer = Writer::new(&mut output);
    /// writer.write_all(b"Hello World").await?;
    /// writer.close().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(inner: W) -> Self {
        Self {
            inner,
            binary: false,
            line: [0; 78],
            line_len: 0,
            pending_cr: false,
        }
    }

    /// Closes the writer, flushing any buffered data.
    ///
    /// This must be called to ensure all data is written.
    pub async fn close(self) -> io::Result<()> {
        let mut pinned = Box::pin(self);
        futures::future::poll_fn(|cx| {
            pinned.as_mut().poll_shutdown(cx)
        }).await
    }
}

impl<W: AsyncWrite> AsyncWrite for Writer<W> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let mut total_written = 0;
        let mut remaining = buf;

        while !remaining.is_empty() {
            let mut this = self.as_mut().project();

            // If line buffer is full, flush it first
            if *this.line_len > 0 && *this.line_len >= LINE_MAX_LEN - 3 {
                // Try to flush the line buffer
                match this.inner.as_mut().poll_write(cx, &this.line[..*this.line_len]) {
                    Poll::Ready(Ok(n)) if n == *this.line_len => {
                        *this.line_len = 0;
                    }
                    Poll::Ready(Ok(_)) => {
                        return Poll::Ready(Err(io::Error::new(
                            io::ErrorKind::WriteZero,
                            "failed to write whole line",
                        )));
                    }
                    Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                    Poll::Pending => return Poll::Pending,
                }
            }

            let this = self.as_mut().project();

            // Try to write plain bytes
            let plain_written = {
                let mut written = 0;
                for &b in remaining {
                    // Check if we need a soft line break
                    if *this.line_len >= LINE_MAX_LEN - 3 {
                        break;
                    }

                    // Handle line endings in non-binary mode
                    if !*this.binary && (b == b'\n' || b == b'\r') {
                        // Handle CRLF sequence
                        if *this.pending_cr && b == b'\n' {
                            *this.pending_cr = false;
                            written += 1;
                            continue;
                        }

                        if b == b'\r' {
                            *this.pending_cr = true;
                        }

                        // Add CRLF to buffer
                        if *this.line_len + 2 <= LINE_MAX_LEN {
                            this.line[*this.line_len] = b'\r';
                            this.line[*this.line_len + 1] = b'\n';
                            *this.line_len += 2;
                            written += 1;
                            // Mark that we need to flush
                            break;
                        } else {
                            break;
                        }
                    }

                    // Check if byte needs encoding
                    if b >= b'!' && b <= b'~' && b != b'=' {
                        // Can write directly
                        this.line[*this.line_len] = b;
                        *this.line_len += 1;
                        *this.pending_cr = false;
                        written += 1;
                    } else if is_whitespace(b) {
                        // Whitespace - write but may need encoding at end of line
                        this.line[*this.line_len] = b;
                        *this.line_len += 1;
                        *this.pending_cr = false;
                        written += 1;
                    } else {
                        // Need to encode
                        if *this.line_len + 3 <= LINE_MAX_LEN - 1 {
                            this.line[*this.line_len] = b'=';
                            this.line[*this.line_len + 1] = UPPER_HEX[(b >> 4) as usize];
                            this.line[*this.line_len + 2] = UPPER_HEX[(b & 0x0F) as usize];
                            *this.line_len += 3;
                            written += 1;
                        } else {
                            // Need soft line break first
                            break;
                        }
                    }
                }
                written
            };

            remaining = &remaining[plain_written..];
            total_written += plain_written;

            if plain_written == 0 && !remaining.is_empty() {
                // Need to flush and add soft line break
                let this = self.as_mut().project();
                if *this.line_len > 0 {
                    this.line[*this.line_len] = b'=';
                    this.line[*this.line_len + 1] = b'\r';
                    this.line[*this.line_len + 2] = b'\n';
                    *this.line_len += 3;
                }
                // Will flush on next iteration
            }
        }

        Poll::Ready(Ok(total_written))
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let mut this = self.as_mut().project();

        // Flush line buffer first
        if *this.line_len > 0 {
            match this.inner.as_mut().poll_write(cx, &this.line[..*this.line_len]) {
                Poll::Ready(Ok(n)) if n == *this.line_len => {
                    *this.line_len = 0;
                }
                Poll::Ready(Ok(_)) => {
                    return Poll::Ready(Err(io::Error::new(
                        io::ErrorKind::WriteZero,
                        "failed to write whole line",
                    )));
                }
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => return Poll::Pending,
            }
        }

        self.project().inner.poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        // Check last byte for trailing whitespace
        {
            let this = self.as_mut().project();
            if *this.line_len > 0 {
                let last_byte = this.line[*this.line_len - 1];
                if is_whitespace(last_byte) {
                    *this.line_len -= 1;
                    // Encode the whitespace
                    if *this.line_len + 3 <= LINE_MAX_LEN - 1 {
                        this.line[*this.line_len] = b'=';
                        this.line[*this.line_len + 1] = UPPER_HEX[(last_byte >> 4) as usize];
                        this.line[*this.line_len + 2] = UPPER_HEX[(last_byte & 0x0F) as usize];
                        *this.line_len += 3;
                    } else {
                        // Add soft line break
                        this.line[*this.line_len] = b'=';
                        this.line[*this.line_len + 1] = b'\r';
                        this.line[*this.line_len + 2] = b'\n';
                        *this.line_len += 3;
                        // Then add encoded byte in a separate line
                        this.line[*this.line_len] = b'=';
                        this.line[*this.line_len + 1] = UPPER_HEX[(last_byte >> 4) as usize];
                        this.line[*this.line_len + 2] = UPPER_HEX[(last_byte & 0x0F) as usize];
                        *this.line_len += 3;
                    }
                }
            }
        }

        // Flush remaining buffer
        ready!(self.as_mut().poll_flush(cx))?;

        // Shutdown inner writer
        self.project().inner.poll_shutdown(cx)
    }
}

/// Checks if a byte is whitespace (space or tab).
fn is_whitespace(b: u8) -> bool {
    b == b' ' || b == b'\t'
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;

    #[tokio::test]
    async fn test_encode_simple() {
        let mut output = Vec::new();
        let mut writer = Writer::new(&mut output);
        writer.write_all(b"Hello").await.unwrap();
        writer.close().await.unwrap();
        assert_eq!(output, b"Hello");
    }

    #[tokio::test]
    async fn test_encode_with_space() {
        let mut output = Vec::new();
        let mut writer = Writer::new(&mut output);
        writer.write_all(b"Hello World").await.unwrap();
        writer.close().await.unwrap();
        assert_eq!(output, b"Hello World");
    }

    #[tokio::test]
    async fn test_encode_special_chars() {
        let mut output = Vec::new();
        let mut writer = Writer::new(&mut output);
        writer.write_all(b"test=test").await.unwrap();
        writer.close().await.unwrap();
        assert_eq!(output, b"test=3Dtest");
    }

    #[tokio::test]
    async fn test_encode_with_newlines() {
        let mut output = Vec::new();
        let mut writer = Writer::new(&mut output);
        writer.write_all(b"Line1\r\nLine2\r\n").await.unwrap();
        writer.close().await.unwrap();
        assert_eq!(output, b"Line1\r\nLine2\r\n");
    }

    #[tokio::test]
    async fn test_binary_mode() {
        let mut output = Vec::new();
        let mut writer = Writer::new(&mut output);
        writer.binary = true;
        writer.write_all(b"\r\n").await.unwrap();
        writer.close().await.unwrap();
        assert_eq!(output, b"=0D=0A");
    }
}
