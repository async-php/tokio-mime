//! Form data handling for multipart messages.
//!
//! Implements RFC 2388 multipart/form-data processing.

use crate::error::Result;
use crate::multipart::reader::MimeHeader;
use std::collections::HashMap;
use std::io::Cursor;
use tokio::fs::File;
use tokio::io::AsyncRead;

#[cfg(test)]
use tokio::io::AsyncReadExt;

#[allow(dead_code)]
const MAX_MEMORY_DEFAULT: usize = 32 << 20; // 32 MB
#[allow(dead_code)]
const MAX_PARTS_DEFAULT: usize = 1000;

/// A parsed multipart form.
///
/// Contains both regular form values and file uploads.
pub struct Form {
    /// The non-file form values.
    pub value: HashMap<String, Vec<String>>,
    /// The file uploads.
    pub file: HashMap<String, Vec<FileHeader>>,
}

impl Form {
    /// Creates a new empty form.
    pub fn new() -> Self {
        Self {
            value: HashMap::new(),
            file: HashMap::new(),
        }
    }

    /// Removes all temporary files created during form parsing.
    pub async fn remove_all(&mut self) -> Result<()> {
        for files in self.file.values_mut() {
            for file_header in files {
                file_header.remove().await?;
            }
        }
        Ok(())
    }
}

impl Default for Form {
    fn default() -> Self {
        Self::new()
    }
}

/// A file header in a multipart form.
///
/// Contains metadata about an uploaded file and methods to access its content.
pub struct FileHeader {
    /// The filename.
    pub filename: String,
    /// The file size in bytes.
    pub size: i64,
    /// The MIME headers for this file part.
    pub header: MimeHeader,
    /// In-memory content (if file is small enough).
    content: Option<Vec<u8>>,
    /// Temporary file path (if file was written to disk).
    tmpfile: Option<String>,
}

impl FileHeader {
    /// Creates a new FileHeader with in-memory content.
    pub fn new(filename: String, content: Vec<u8>, header: MimeHeader) -> Self {
        let size = content.len() as i64;
        Self {
            filename,
            size,
            header,
            content: Some(content),
            tmpfile: None,
        }
    }

    /// Creates a new FileHeader with temporary file.
    pub fn from_file(filename: String, size: i64, tmpfile: String, header: MimeHeader) -> Self {
        Self {
            filename,
            size,
            header,
            content: None,
            tmpfile: Some(tmpfile),
        }
    }

    /// Opens the file for reading.
    ///
    /// Returns a reader that can be used to read the file contents.
    pub async fn open(&self) -> Result<Box<dyn AsyncRead + Unpin + Send>> {
        if let Some(content) = &self.content {
            // File is in memory
            Ok(Box::new(Cursor::new(content.clone())))
        } else if let Some(path) = &self.tmpfile {
            // File is on disk
            let file = File::open(path).await?;
            Ok(Box::new(file))
        } else {
            // No content available
            Ok(Box::new(Cursor::new(Vec::new())))
        }
    }

    /// Removes the temporary file if it exists.
    async fn remove(&mut self) -> Result<()> {
        if let Some(path) = self.tmpfile.take() {
            tokio::fs::remove_file(&path).await?;
        }
        Ok(())
    }
}

impl Drop for FileHeader {
    fn drop(&mut self) {
        // Note: We can't await in Drop, so temporary files are cleaned up via remove_all()
        // or when the Form is dropped if the user didn't call remove_all()
        if let Some(path) = &self.tmpfile {
            // Best effort cleanup (may fail if async runtime is gone)
            let _ = std::fs::remove_file(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_form_new() {
        let form = Form::new();
        assert!(form.value.is_empty());
        assert!(form.file.is_empty());
    }

    #[tokio::test]
    async fn test_file_header_in_memory() {
        let content = b"test content".to_vec();
        let header = MimeHeader::new();
        let file_header = FileHeader::new("test.txt".to_string(), content.clone(), header);

        assert_eq!(file_header.filename, "test.txt");
        assert_eq!(file_header.size, 12);

        let mut reader = file_header.open().await.unwrap();
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf).await.unwrap();
        assert_eq!(buf, content);
    }

    #[tokio::test]
    async fn test_file_header_from_disk() {
        use tokio::io::AsyncWriteExt;

        // Create a temporary file
        let tmpfile = "/tmp/test_multipart_rs.txt";
        let content = b"file content";
        let mut file = File::create(tmpfile).await.unwrap();
        file.write_all(content).await.unwrap();
        file.flush().await.unwrap();
        drop(file);

        let header = MimeHeader::new();
        let mut file_header = FileHeader::from_file(
            "test.txt".to_string(),
            content.len() as i64,
            tmpfile.to_string(),
            header,
        );

        assert_eq!(file_header.filename, "test.txt");
        assert_eq!(file_header.size, 12);

        let mut reader = file_header.open().await.unwrap();
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf).await.unwrap();
        assert_eq!(buf, content);

        // Clean up
        file_header.remove().await.unwrap();
    }
}
