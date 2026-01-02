//! Unix-specific MIME type loading.
//!
//! Implements loading from:
//! - FreeDesktop Shared MIME-info Database (globs2 format)
//! - Traditional mime.types files

use crate::error::Result;
use crate::mime_type::set_extension_type_skip_existing;
use std::fs::File;
use std::io::{BufRead, BufReader};

/// Paths to FreeDesktop Shared MIME-info Database globs2 files.
const MIME_GLOBS: &[&str] = &[
    "/usr/local/share/mime/globs2",
    "/usr/share/mime/globs2",
];

/// Common locations for mime.types files on Unix.
const TYPE_FILES: &[&str] = &[
    "/etc/mime.types",
    "/etc/apache2/mime.types",
    "/etc/apache/mime.types",
    "/etc/httpd/conf/mime.types",
];

/// Initialize MIME types from Unix system databases.
pub(super) fn init_mime_unix() -> Result<()> {
    // Try globs2 files first (preferred format)
    for filename in MIME_GLOBS {
        if load_mime_globs_file(filename).is_ok() {
            // Stop checking more files if mimetype database is found
            return Ok(());
        }
    }

    // Fallback: load traditional mime.types files
    for filename in TYPE_FILES {
        let _ = load_mime_file(filename);
    }

    Ok(())
}

/// Load MIME types from a globs2 file.
///
/// Format: `weight:mimetype:glob[:morefields...]`
/// Example: `50:text/plain:*.txt`
///
/// See https://specifications.freedesktop.org/shared-mime-info-spec/shared-mime-info-spec-0.21.html
fn load_mime_globs_file(filename: &str) -> Result<()> {
    let file = File::open(filename)?;
    let reader = BufReader::new(file);

    for line in reader.lines() {
        let line = line?;
        
        // Each line should be of format: weight:mimetype:glob[:morefields...]
        let fields: Vec<&str> = line.split(':').collect();
        
        // Need at least 3 fields, and valid weight/glob
        if fields.len() < 3 || fields[0].is_empty() || fields[2].len() < 3 {
            continue;
        }
        
        // Skip comments
        if fields[0].starts_with('#') {
            continue;
        }
        
        // Only process simple extensions (*.ext)
        if !fields[2].starts_with("*.") {
            continue;
        }
        
        let extension = &fields[2][1..]; // Remove leading *
        
        // Skip globs with wildcards (we only handle simple extensions)
        if extension.contains(&['?', '*', '['][..]) {
            continue;
        }
        
        // Add the extension (skip if already exists to preserve builtins)
        let _ = set_extension_type_skip_existing(extension, fields[1]);
    }

    Ok(())
}

/// Load MIME types from a mime.types file.
///
/// Format: `mimetype ext1 ext2 ext3 ...`
/// Example: `text/plain txt text`
fn load_mime_file(filename: &str) -> Result<()> {
    let file = File::open(filename)?;
    let reader = BufReader::new(file);

    for line in reader.lines() {
        let line = line?;
        let fields: Vec<&str> = line.split_whitespace().collect();
        
        // Need at least type and one extension
        if fields.len() <= 1 {
            continue;
        }
        
        // Skip comments
        if fields[0].starts_with('#') {
            continue;
        }
        
        let mime_type = fields[0];
        
        // Process all extensions
        for ext in &fields[1..] {
            // Stop at comments
            if ext.starts_with('#') {
                break;
            }
            
            // Add dot prefix if missing
            let extension = if ext.starts_with('.') {
                ext.to_string()
            } else {
                format!(".{}", ext)
            };
            
            let _ = set_extension_type_skip_existing(&extension, mime_type);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_mime_unix() {
        // Should not panic and complete without error
        let result = init_mime_unix();
        assert!(result.is_ok());
    }
}
