//! Quoted-printable reader.
//!
//! Implements RFC 2045 quoted-printable decoding with async I/O.

use crate::error::{Error, Result};
use pin_project::pin_project;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncBufRead, AsyncRead, ReadBuf};

/// A quoted-printable decoder.
///
/// Implements `AsyncRead` to decode quoted-printable data on the fly.
#[pin_project]
pub struct Reader<R> {
    #[pin]
    inner: tokio::io::BufReader<R>,
    line: Vec<u8>,
    line_pos: usize,
    eof: bool,
    error: Option<io::Error>,
}

impl<R: AsyncRead> Reader<R> {
    /// Creates a new quoted-printable reader.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use yamime::quotedprintable::Reader;
    /// use tokio::io::AsyncReadExt;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let data = b"Hello=20World";
    /// let mut reader = Reader::new(&data[..]);
    /// let mut output = String::new();
    /// reader.read_to_string(&mut output).await?;
    /// assert_eq!(output, "Hello World");
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(inner: R) -> Self {
        Self {
            inner: tokio::io::BufReader::new(inner),
            line: Vec::new(),
            line_pos: 0,
            eof: false,
            error: None,
        }
    }
}

impl<R: AsyncRead> AsyncRead for Reader<R> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let mut this = self.project();

        // If we have an error, return it
        if let Some(err) = this.error.take() {
            return Poll::Ready(Err(err));
        }

        let output = buf.initialize_unfilled();
        let mut written = 0;

        while written < output.len() {
            // If we have buffered data, consume it
            if *this.line_pos < this.line.len() {
                let available = this.line.len() - *this.line_pos;
                let to_copy = available.min(output.len() - written);
                output[written..written + to_copy]
                    .copy_from_slice(&this.line[*this.line_pos..*this.line_pos + to_copy]);
                *this.line_pos += to_copy;
                written += to_copy;
                continue;
            }

            // Need to read more data
            this.line.clear();
            *this.line_pos = 0;

            // Read a line from the underlying reader
            let mut line_buf = Vec::new();
            loop {
                let poll_result = this.inner.as_mut().poll_fill_buf(cx);
                match poll_result {
                    Poll::Ready(Ok(chunk)) => {
                        if chunk.is_empty() {
                            *this.eof = true;
                            break;
                        }

                        // Find newline
                        if let Some(pos) = chunk.iter().position(|&b| b == b'\n') {
                            line_buf.extend_from_slice(&chunk[..=pos]);
                            this.inner.as_mut().consume(pos + 1);
                            break;
                        } else {
                            line_buf.extend_from_slice(chunk);
                            let len = chunk.len();
                            this.inner.as_mut().consume(len);
                        }
                    }
                    Poll::Ready(Err(e)) => {
                        *this.error = Some(e);
                        buf.advance(written);
                        return Poll::Ready(Ok(()));
                    }
                    Poll::Pending => {
                        buf.advance(written);
                        return Poll::Pending;
                    }
                }
            }

            // If we reached EOF and have no data, we're done
            if line_buf.is_empty() && *this.eof {
                break;
            }

            // Process the line (even if EOF, we need to process any remaining data)
            if !line_buf.is_empty() {
                match decode_line(&line_buf) {
                    Ok(decoded) => {
                        this.line.extend_from_slice(&decoded);
                    }
                    Err(e) => {
                        *this.error = Some(io::Error::new(io::ErrorKind::InvalidData, e));
                        break;
                    }
                }
            } else if *this.eof {
                // EOF with no more data
                break;
            }
        }

        buf.advance(written);
        Poll::Ready(Ok(()))
    }
}

/// Decodes a single line of quoted-printable data.
fn decode_line(line: &[u8]) -> Result<Vec<u8>> {
    let mut result = Vec::with_capacity(line.len());

    // Check if line ends with CRLF or LF
    let has_lf = line.ends_with(b"\n");
    let has_crlf = line.ends_with(b"\r\n");

    // Trim trailing whitespace
    let mut trimmed = line;
    while !trimmed.is_empty() {
        match trimmed[trimmed.len() - 1] {
            b'\n' | b'\r' | b' ' | b'\t' => trimmed = &trimmed[..trimmed.len() - 1],
            _ => break,
        }
    }

    // Check for soft line break (line ending with =)
    let is_soft_break = trimmed.ends_with(b"=");
    if is_soft_break {
        trimmed = &trimmed[..trimmed.len() - 1];
    }

    // Decode the line content
    let mut i = 0;
    while i < trimmed.len() {
        match trimmed[i] {
            b'=' => {
                if i + 2 < trimmed.len() {
                    // Try to decode =XX
                    match decode_hex_byte(trimmed[i + 1], trimmed[i + 2]) {
                        Ok(byte) => {
                            result.push(byte);
                            i += 3;
                            continue;
                        }
                        Err(_) => {
                            // Not valid hex, treat = as literal
                            result.push(b'=');
                            i += 1;
                        }
                    }
                } else {
                    // = at end without hex digits, treat as literal
                    result.push(b'=');
                    i += 1;
                }
            }
            b => {
                // Regular byte or whitespace
                if b == b'\t' || b == b'\r' || (b >= b' ' && b <= b'~') || b >= 0x80 {
                    result.push(b);
                } else if b < b' ' && b != b'\t' && b != b'\r' && b != b'\n' {
                    return Err(Error::Encoding(format!(
                        "invalid unescaped byte: 0x{:02x}",
                        b
                    )));
                } else {
                    result.push(b);
                }
                i += 1;
            }
        }
    }

    // Add line ending if this wasn't a soft break
    if !is_soft_break && has_lf {
        if has_crlf {
            result.push(b'\r');
            result.push(b'\n');
        } else {
            result.push(b'\n');
        }
    }

    Ok(result)
}

/// Decodes two hex digits into a byte.
fn decode_hex_byte(high: u8, low: u8) -> Result<u8> {
    let h = decode_hex_digit(high)?;
    let l = decode_hex_digit(low)?;
    Ok((h << 4) | l)
}

/// Decodes a single hex digit.
fn decode_hex_digit(digit: u8) -> Result<u8> {
    match digit {
        b'0'..=b'9' => Ok(digit - b'0'),
        b'A'..=b'F' => Ok(digit - b'A' + 10),
        b'a'..=b'f' => Ok(digit - b'a' + 10),
        _ => Err(Error::Encoding(format!("invalid hex digit: 0x{:02x}", digit))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncReadExt;

    #[tokio::test]
    async fn test_decode_simple() {
        let data = b"Hello World";
        let mut reader = Reader::new(&data[..]);
        let mut output = String::new();
        reader.read_to_string(&mut output).await.unwrap();
        assert_eq!(output, "Hello World");
    }

    #[tokio::test]
    async fn test_decode_with_encoding() {
        let data = b"Hello=20World";
        let mut reader = Reader::new(&data[..]);
        let mut output = String::new();
        reader.read_to_string(&mut output).await.unwrap();
        assert_eq!(output, "Hello World");
    }

    #[tokio::test]
    async fn test_decode_soft_line_break() {
        let data = b"Hello=\r\nWorld";
        let mut reader = Reader::new(&data[..]);
        let mut output = String::new();
        reader.read_to_string(&mut output).await.unwrap();
        assert_eq!(output, "HelloWorld");
    }

    #[tokio::test]
    async fn test_decode_with_newlines() {
        let data = b"Line1\r\nLine2\r\n";
        let mut reader = Reader::new(&data[..]);
        let mut output = String::new();
        reader.read_to_string(&mut output).await.unwrap();
        assert_eq!(output, "Line1\r\nLine2\r\n");
    }

    #[tokio::test]
    async fn test_decode_hex() {
        let data = b"=48=65=6C=6C=6F"; // "Hello" in hex
        let mut reader = Reader::new(&data[..]);
        let mut output = String::new();
        reader.read_to_string(&mut output).await.unwrap();
        assert_eq!(output, "Hello");
    }
}
