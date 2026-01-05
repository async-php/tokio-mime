//! Multipart MIME writer.
//!
//! Implements RFC 2046 multipart message generation with async I/O.

use crate::error::{Error, Result};
use std::collections::HashMap;
use tokio::io::{AsyncWrite, AsyncWriteExt};

/// A multipart MIME writer.
pub struct Writer<W> {
    writer: W,
    boundary: String,
    has_parts: bool,
}

impl<W: AsyncWrite + Unpin> Writer<W> {
    /// Creates a new multipart writer with a random boundary.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio_mime::multipart::Writer;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut output = Vec::new();
    /// let writer = Writer::new(&mut output);
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            boundary: generate_boundary(),
            has_parts: false,
        }
    }

    /// Returns the writer's boundary string.
    pub fn boundary(&self) -> &str {
        &self.boundary
    }

    /// Sets a custom boundary.
    ///
    /// This must be called before creating any parts.
    /// The boundary must be 1-70 characters and contain only valid characters.
    pub fn set_boundary(&mut self, boundary: String) -> Result<()> {
        if self.has_parts {
            return Err(Error::Multipart(
                "cannot set boundary after writing parts".to_string(),
            ));
        }

        // Validate boundary (RFC 2046)
        if boundary.is_empty() || boundary.len() > 70 {
            return Err(Error::Multipart("invalid boundary length".to_string()));
        }

        for (i, ch) in boundary.chars().enumerate() {
            let valid = ch.is_ascii_alphanumeric()
                || matches!(ch, '\'' | '(' | ')' | '+' | '_' | ',' | '-' | '.' | '/' | ':' | '=' | '?')
                || (ch == ' ' && i != boundary.len() - 1);

            if !valid {
                return Err(Error::Multipart(format!(
                    "invalid boundary character: {}",
                    ch
                )));
            }
        }

        self.boundary = boundary;
        Ok(())
    }

    /// Returns the Content-Type header value for multipart/form-data.
    pub fn form_data_content_type(&self) -> String {
        let boundary = if self.boundary.contains(|c| matches!(c, '(' | ')' | '<' | '>' | '@' | ',' | ';' | ':' | '"' | '/' | '[' | ']' | '?' | '=' | ' ')) {
            format!("\"{}\"", self.boundary)
        } else {
            self.boundary.clone()
        };

        format!("multipart/form-data; boundary={}", boundary)
    }

    /// Creates a new part with the given headers.
    ///
    /// Returns a PartWriter that can be used to write the part's body.
    pub async fn create_part(
        &mut self,
        headers: HashMap<String, Vec<String>>,
    ) -> Result<PartWriter<'_, W>> {
        // Write boundary
        if self.has_parts {
            self.writer.write_all(b"\r\n").await?;
        }
        self.writer
            .write_all(format!("--{}\r\n", self.boundary).as_bytes())
            .await?;

        // Write headers (sorted for consistency)
        let mut keys: Vec<_> = headers.keys().collect();
        keys.sort();

        for key in keys {
            if let Some(values) = headers.get(key) {
                for value in values {
                    self.writer
                        .write_all(format!("{}: {}\r\n", key, value).as_bytes())
                        .await?;
                }
            }
        }

        // Empty line after headers
        self.writer.write_all(b"\r\n").await?;

        self.has_parts = true;

        Ok(PartWriter {
            writer: &mut self.writer,
        })
    }

    /// Convenience method to create a form file part.
    pub async fn create_form_file(
        &mut self,
        fieldname: &str,
        filename: &str,
    ) -> Result<PartWriter<'_, W>> {
        let mut headers = HashMap::new();
        headers.insert(
            "Content-Disposition".to_string(),
            vec![format!(
                "form-data; name=\"{}\"; filename=\"{}\"",
                escape_quotes(fieldname),
                escape_quotes(filename)
            )],
        );
        headers.insert(
            "Content-Type".to_string(),
            vec!["application/octet-stream".to_string()],
        );

        self.create_part(headers).await
    }

    /// Convenience method to create a form field part.
    pub async fn create_form_field(&mut self, fieldname: &str) -> Result<PartWriter<'_, W>> {
        let mut headers = HashMap::new();
        headers.insert(
            "Content-Disposition".to_string(),
            vec![format!(
                "form-data; name=\"{}\"",
                escape_quotes(fieldname)
            )],
        );

        self.create_part(headers).await
    }

    /// Writes a complete form field with value.
    pub async fn write_field(&mut self, fieldname: &str, value: &str) -> Result<()> {
        let mut part = self.create_form_field(fieldname).await?;
        part.write_all(value.as_bytes()).await?;
        Ok(())
    }

    /// Closes the writer by writing the final boundary.
    pub async fn close(mut self) -> Result<()> {
        if self.has_parts {
            self.writer.write_all(b"\r\n").await?;
        }
        self.writer
            .write_all(format!("--{}--\r\n", self.boundary).as_bytes())
            .await?;
        self.writer.flush().await?;
        Ok(())
    }
}

/// A writer for a single part's body.
pub struct PartWriter<'a, W> {
    writer: &'a mut W,
}

impl<'a, W: AsyncWrite + Unpin> AsyncWrite for PartWriter<'a, W> {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        std::pin::Pin::new(&mut self.writer).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.writer).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.writer).poll_shutdown(cx)
    }
}

/// Generates a random boundary string.
fn generate_boundary() -> String {
    use getrandom::getrandom;

    let mut buf = [0u8; 30];
    getrandom(&mut buf).expect("failed to generate random boundary");

    // Convert to hex string
    buf.iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>()
}

/// Escapes quotes and backslashes in a string.
fn escape_quotes(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_writer_basic() {
        let mut output = Vec::new();
        let mut writer = Writer::new(&mut output);

        writer.write_field("field1", "value1").await.unwrap();
        writer.write_field("field2", "value2").await.unwrap();
        writer.close().await.unwrap();

        let result = String::from_utf8(output).unwrap();
        assert!(result.contains("Content-Disposition: form-data; name=\"field1\""));
        assert!(result.contains("value1"));
        assert!(result.contains("Content-Disposition: form-data; name=\"field2\""));
        assert!(result.contains("value2"));
        assert!(result.ends_with("--\r\n"));
    }

    #[tokio::test]
    async fn test_form_file() {
        let mut output = Vec::new();
        let mut writer = Writer::new(&mut output);

        let mut part = writer
            .create_form_file("upload", "test.txt")
            .await
            .unwrap();
        part.write_all(b"file content").await.unwrap();
        drop(part);

        writer.close().await.unwrap();

        let result = String::from_utf8(output).unwrap();
        assert!(result.contains("name=\"upload\""));
        assert!(result.contains("filename=\"test.txt\""));
        assert!(result.contains("Content-Type: application/octet-stream"));
        assert!(result.contains("file content"));
    }

    #[test]
    fn test_boundary_validation() {
        let mut output = Vec::new();
        let mut writer = Writer::new(&mut output);

        // Valid boundary
        assert!(writer.set_boundary("simple-boundary".to_string()).is_ok());

        // Too long
        let long = "a".repeat(71);
        assert!(writer.set_boundary(long).is_err());

        // Empty
        assert!(writer.set_boundary(String::new()).is_err());
    }

    #[test]
    fn test_escape_quotes() {
        assert_eq!(escape_quotes("hello"), "hello");
        assert_eq!(escape_quotes("hel\"lo"), "hel\\\"lo");
        assert_eq!(escape_quotes("hel\\lo"), "hel\\\\lo");
        assert_eq!(escape_quotes("hel\\\"lo"), "hel\\\\\\\"lo");
    }
}
