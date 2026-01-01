//! Multipart MIME reader.
//!
//! Implements RFC 2046 multipart parsing with async I/O.

use crate::error::{Error, Result};
use pin_project::pin_project;
use std::collections::HashMap;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncRead, BufReader, ReadBuf};

const PEEK_BUFFER_SIZE: usize = 4096;
const MAX_MIME_HEADER_SIZE: usize = 10 << 20; // 10 MB
const MAX_MIME_HEADERS: usize = 10000;

/// MIME header type (similar to HTTP headers).
pub type MimeHeader = HashMap<String, Vec<String>>;

/// A multipart MIME reader.
pub struct Reader<R> {
    buf_reader: BufReader<R>,
    boundary: Vec<u8>,
    nl: Vec<u8>,               // "\r\n" or "\n"
    nl_dash_boundary: Vec<u8>, // nl + "--boundary"
    dash_boundary_dash: Vec<u8>, // "--boundary--"
    dash_boundary: Vec<u8>,    // "--boundary"
    parts_read: usize,
}

impl<R: AsyncRead + Unpin> Reader<R> {
    /// Creates a new multipart reader with the given boundary.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use mime_rs::multipart::Reader;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let data = b"--boundary\r\n...";
    /// let reader = Reader::new(&data[..], "boundary");
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(r: R, boundary: &str) -> Self {
        let b = format!("\r\n--{}--", boundary).into_bytes();
        let nl = b[0..2].to_vec();
        let nl_dash_boundary = b[0..b.len() - 2].to_vec();
        let dash_boundary_dash = b[2..].to_vec();
        let dash_boundary = b[2..b.len() - 2].to_vec();

        Self {
            buf_reader: BufReader::with_capacity(PEEK_BUFFER_SIZE, r),
            boundary: boundary.as_bytes().to_vec(),
            nl,
            nl_dash_boundary,
            dash_boundary_dash,
            dash_boundary,
            parts_read: 0,
        }
    }

    /// Returns the next part in the multipart message.
    ///
    /// Returns `None` when there are no more parts.
    pub async fn next_part(&mut self) -> Result<Option<Part<R>>> {
        self.next_part_internal(false).await
    }

    /// Returns the next part without decoding quoted-printable.
    pub async fn next_raw_part(&mut self) -> Result<Option<Part<R>>> {
        self.next_part_internal(true).await
    }

    async fn next_part_internal(&mut self, raw_part: bool) -> Result<Option<Part<R>>> {
        if self.boundary.is_empty() {
            return Err(Error::Multipart("boundary is empty".to_string()));
        }

        let mut expect_new_part = false;

        loop {
            let mut line = Vec::new();
            match self.buf_reader.read_until(b'\n', &mut line).await {
                Ok(0) => {
                    // EOF
                    if self.is_final_boundary(&line) {
                        return Ok(None);
                    }
                    return Err(Error::Io(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "unexpected EOF",
                    )));
                }
                Ok(_) => {}
                Err(e) => {
                    if e.kind() == io::ErrorKind::UnexpectedEof && self.is_final_boundary(&line) {
                        return Ok(None);
                    }
                    return Err(Error::Io(e));
                }
            }

            if self.is_boundary_delimiter_line(&line) {
                self.parts_read += 1;
                let part = Part::new(
                    &mut self.buf_reader,
                    raw_part,
                    &self.dash_boundary,
                    &self.nl_dash_boundary,
                )
                .await?;
                return Ok(Some(part));
            }

            if self.is_final_boundary(&line) {
                return Ok(None);
            }

            if expect_new_part {
                return Err(Error::Multipart(format!(
                    "expecting a new Part; got line {:?}",
                    String::from_utf8_lossy(&line)
                )));
            }

            if self.parts_read == 0 {
                // Skip preamble
                continue;
            }

            if line == self.nl {
                expect_new_part = true;
                continue;
            }

            return Err(Error::Multipart(format!(
                "unexpected line in next_part: {:?}",
                String::from_utf8_lossy(&line)
            )));
        }
    }

    fn is_final_boundary(&self, line: &[u8]) -> bool {
        if !line.starts_with(&self.dash_boundary_dash) {
            return false;
        }
        let rest = &line[self.dash_boundary_dash.len()..];
        let rest = skip_lwsp_char(rest);
        rest.is_empty() || rest == self.nl
    }

    fn is_boundary_delimiter_line(&mut self, line: &[u8]) -> bool {
        if !line.starts_with(&self.dash_boundary) {
            return false;
        }
        let rest = &line[self.dash_boundary.len()..];
        let rest = skip_lwsp_char(rest);

        // On the first part, check if lines end in \n instead of \r\n
        if self.parts_read == 0 && rest.len() == 1 && rest[0] == b'\n' {
            self.nl = vec![b'\n'];
            self.nl_dash_boundary = [b"\n".as_ref(), &self.dash_boundary].concat();
        }

        rest == self.nl
    }
}

/// A single part in a multipart message.
#[pin_project]
pub struct Part<R> {
    /// The MIME headers of this part.
    pub header: MimeHeader,

    #[pin]
    reader: PartReader<R>,

    disposition: Option<String>,
    disposition_params: Option<HashMap<String, String>>,
}

impl<R: AsyncRead + Unpin> Part<R> {
    async fn new(
        buf_reader: &mut BufReader<R>,
        _raw_part: bool,
        dash_boundary: &[u8],
        nl_dash_boundary: &[u8],
    ) -> Result<Self> {
        // Read headers
        let header = read_mime_header(buf_reader).await?;

        // Read part body into memory until boundary
        let data = read_part_data(buf_reader, dash_boundary, nl_dash_boundary).await?;
        let reader = PartReader::new(data);

        Ok(Self {
            header,
            reader,
            disposition: None,
            disposition_params: None,
        })
    }

    /// Returns the form field name if this part has Content-Disposition: form-data.
    pub fn form_name(&mut self) -> Option<&str> {
        self.parse_content_disposition();
        if self.disposition.as_deref() != Some("form-data") {
            return None;
        }
        self.disposition_params
            .as_ref()
            .and_then(|p| p.get("name"))
            .map(|s| s.as_str())
    }

    /// Returns the filename parameter from Content-Disposition header.
    pub fn file_name(&mut self) -> Option<String> {
        self.parse_content_disposition();
        self.disposition_params
            .as_ref()
            .and_then(|p| p.get("filename"))
            .map(|f| {
                // Extract just the filename (not path)
                std::path::Path::new(f)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(f)
                    .to_string()
            })
    }

    fn parse_content_disposition(&mut self) {
        if self.disposition.is_some() {
            return;
        }

        if let Some(values) = self.header.get("content-disposition") {
            if let Some(v) = values.first() {
                let (disp, params) = parse_disposition(v);
                self.disposition = Some(disp);
                self.disposition_params = Some(params);
                return;
            }
        }

        self.disposition = Some(String::new());
        self.disposition_params = Some(HashMap::new());
    }
}

impl<R: AsyncRead + Unpin> AsyncRead for Part<R> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.project();
        this.reader.poll_read(cx, buf)
    }
}

/// Internal reader for a part's body.
#[pin_project]
struct PartReader<R> {
    data: Vec<u8>,
    pos: usize,
    _phantom: std::marker::PhantomData<R>,
}

impl<R> PartReader<R> {
    fn new(data: Vec<u8>) -> Self {
        Self {
            data,
            pos: 0,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<R: AsyncRead + Unpin> AsyncRead for PartReader<R> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let remaining = &self.data[self.pos..];
        let to_read = remaining.len().min(buf.remaining());

        if to_read == 0 {
            return Poll::Ready(Ok(()));
        }

        buf.put_slice(&remaining[..to_read]);
        self.pos += to_read;

        Poll::Ready(Ok(()))
    }
}

/// Reads MIME headers from a buffered reader.
async fn read_mime_header<R: AsyncBufRead + Unpin>(reader: &mut R) -> Result<MimeHeader> {
    let mut header = HashMap::new();
    let mut total_size = 0;
    let mut header_count = 0;

    loop {
        let mut line = String::new();
        reader.read_line(&mut line).await?;

        total_size += line.len();
        if total_size > MAX_MIME_HEADER_SIZE {
            return Err(Error::MessageTooLarge);
        }

        // Empty line signals end of headers
        if line == "\r\n" || line == "\n" || line.is_empty() {
            break;
        }

        header_count += 1;
        if header_count > MAX_MIME_HEADERS {
            return Err(Error::MessageTooLarge);
        }

        // Parse header line
        if let Some((key, value)) = parse_header_line(&line) {
            header
                .entry(key.to_lowercase())
                .or_insert_with(Vec::new)
                .push(value.to_string());
        }
    }

    Ok(header)
}

/// Parses a single header line.
fn parse_header_line(line: &str) -> Option<(&str, &str)> {
    let line = line.trim_end_matches('\n').trim_end_matches('\r');
    let colon_pos = line.find(':')?;
    let key = line[..colon_pos].trim();
    let value = line[colon_pos + 1..].trim();
    Some((key, value))
}

/// Parses Content-Disposition header value.
/// Format: disposition-type; param1=value1; param2=value2
fn parse_disposition(value: &str) -> (String, HashMap<String, String>) {
    let (disposition, rest) = value.split_once(';').unwrap_or((value, ""));
    let disposition = disposition.trim().to_lowercase();

    let mut params = HashMap::new();
    for param in rest.split(';') {
        let param = param.trim();
        if param.is_empty() {
            continue;
        }

        if let Some((key, val)) = param.split_once('=') {
            let key = key.trim().to_lowercase();
            let val = val.trim();

            // Remove quotes if present
            let val = if val.starts_with('"') && val.ends_with('"') && val.len() >= 2 {
                &val[1..val.len() - 1]
            } else {
                val
            };

            params.insert(key, val.to_string());
        }
    }

    (disposition, params)
}

/// Skips leading whitespace (space and tab).
fn skip_lwsp_char(b: &[u8]) -> &[u8] {
    let mut i = 0;
    while i < b.len() && (b[i] == b' ' || b[i] == b'\t') {
        i += 1;
    }
    &b[i..]
}

/// Reads part data until a boundary is encountered.
///
/// This function reads data line by line, checking each line to see if it's a boundary.
/// When a boundary is found, the boundary line is NOT consumed, so the next call to
/// next_part() will see it.
async fn read_part_data<R: AsyncBufRead + Unpin>(
    reader: &mut R,
    dash_boundary: &[u8],
    nl_dash_boundary: &[u8],
) -> Result<Vec<u8>> {
    use tokio::io::AsyncBufReadExt;

    let mut data = Vec::new();
    let mut total_bytes = 0;
    let mut line_buf = Vec::new();

    loop {
        line_buf.clear();

        // Peek at buffered data to check for boundary without consuming
        let buf = reader.fill_buf().await?;

        if buf.is_empty() {
            // EOF
            break;
        }

        // Find the next newline
        let newline_pos = buf.iter().position(|&b| b == b'\n');

        if let Some(pos) = newline_pos {
            // We have a complete line
            line_buf.extend_from_slice(&buf[..=pos]);

            // Check if this is a boundary line
            // Boundaries should be at the start of the line (possibly with leading \r\n or \n)
            if line_buf.starts_with(dash_boundary)
                || line_buf.starts_with(nl_dash_boundary)
                || (line_buf.starts_with(b"\r\n") && line_buf[2..].starts_with(dash_boundary))
                || (line_buf.starts_with(b"\n") && line_buf[1..].starts_with(dash_boundary))
            {
                // Found boundary - don't consume it, return what we have
                break;
            }

            // Not a boundary, consume the line and add to data
            reader.consume(pos + 1);
            data.extend_from_slice(&line_buf);
            total_bytes += line_buf.len();

            // Limit data size to prevent memory exhaustion (32 MB)
            if total_bytes > 32 * 1024 * 1024 {
                return Err(Error::MessageTooLarge);
            }
        } else {
            // No newline in buffer, consume all buffered data
            let len = buf.len();
            data.extend_from_slice(buf);
            reader.consume(len);
            total_bytes += len;

            // Limit check
            if total_bytes > 32 * 1024 * 1024 {
                return Err(Error::MessageTooLarge);
            }

            // Continue to read more data
        }
    }

    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncReadExt;

    #[tokio::test]
    async fn test_read_mime_header() {
        let data = b"Content-Type: text/plain\r\nContent-Length: 123\r\n\r\n";
        let mut reader = BufReader::new(&data[..]);
        let header = read_mime_header(&mut reader).await.unwrap();

        assert_eq!(header.get("content-type").unwrap()[0], "text/plain");
        assert_eq!(header.get("content-length").unwrap()[0], "123");
    }

    #[tokio::test]
    async fn test_parse_header_line() {
        assert_eq!(
            parse_header_line("Content-Type: text/plain\r\n"),
            Some(("Content-Type", "text/plain"))
        );
        assert_eq!(
            parse_header_line("Content-Length:123\n"),
            Some(("Content-Length", "123"))
        );
    }

    #[tokio::test]
    async fn test_multipart_reader() {
        let data = b"--boundary\r\n\
Content-Type: text/plain\r\n\
\r\n\
Hello World\r\n\
--boundary\r\n\
Content-Type: text/html\r\n\
\r\n\
<html>test</html>\r\n\
--boundary--\r\n";

        let mut reader = Reader::new(&data[..], "boundary");

        // Read first part
        let mut part1 = reader.next_part().await.unwrap().unwrap();
        assert_eq!(part1.header.get("content-type").unwrap()[0], "text/plain");

        let mut body1 = String::new();
        part1.read_to_string(&mut body1).await.unwrap();
        assert_eq!(body1, "Hello World\r\n");

        // Read second part
        let mut part2 = reader.next_part().await.unwrap().unwrap();
        assert_eq!(part2.header.get("content-type").unwrap()[0], "text/html");

        let mut body2 = String::new();
        part2.read_to_string(&mut body2).await.unwrap();
        assert_eq!(body2, "<html>test</html>\r\n");

        // No more parts
        assert!(reader.next_part().await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_form_data() {
        let data = b"--boundary\r\n\
Content-Disposition: form-data; name=\"field1\"\r\n\
\r\n\
value1\r\n\
--boundary\r\n\
Content-Disposition: form-data; name=\"file\"; filename=\"test.txt\"\r\n\
Content-Type: text/plain\r\n\
\r\n\
file content\r\n\
--boundary--\r\n";

        let mut reader = Reader::new(&data[..], "boundary");

        // Read first part (form field)
        let mut part1 = reader.next_part().await.unwrap().unwrap();
        assert_eq!(part1.form_name(), Some("field1"));

        let mut body1 = String::new();
        part1.read_to_string(&mut body1).await.unwrap();
        assert_eq!(body1, "value1\r\n");

        // Read second part (file)
        let mut part2 = reader.next_part().await.unwrap().unwrap();
        assert_eq!(part2.form_name(), Some("file"));
        assert_eq!(part2.file_name(), Some("test.txt".to_string()));

        let mut body2 = String::new();
        part2.read_to_string(&mut body2).await.unwrap();
        assert_eq!(body2, "file content\r\n");

        // No more parts
        assert!(reader.next_part().await.unwrap().is_none());
    }
}
