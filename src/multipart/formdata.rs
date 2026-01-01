//! Form data handling.

use std::collections::HashMap;

/// A parsed multipart form.
pub struct Form {
    /// The non-file form values.
    pub value: HashMap<String, Vec<String>>,
    /// The file uploads.
    pub file: HashMap<String, Vec<FileHeader>>,
}

/// A file header in a multipart form.
pub struct FileHeader {
    /// The filename.
    pub filename: String,
    /// The file size in bytes.
    pub size: i64,
}
