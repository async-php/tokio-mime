//! MIME type detection by file extension.
//!
//! Provides functions to map file extensions to MIME types and vice versa.
//!
//! The built-in table is small but on Unix it is augmented by the local system's
//! MIME-info database or mime.types file(s) if available under one or more of these names:
//! - /usr/local/share/mime/globs2
//! - /usr/share/mime/globs2
//! - /etc/mime.types
//! - /etc/apache2/mime.types
//! - /etc/apache/mime.types
//!
//! On Windows, MIME types are extracted from the registry.
//!
//! Text types have the charset parameter set to "utf-8" by default.

use crate::error::{Error, Result};
use crate::media_type::{format_media_type, parse_media_type};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::RwLock;

/// Built-in MIME type mappings (all lowercase extensions).
static BUILTIN_TYPES_LOWER: &[(&str, &str)] = &[
    (".avif", "image/avif"),
    (".css", "text/css; charset=utf-8"),
    (".gif", "image/gif"),
    (".htm", "text/html; charset=utf-8"),
    (".html", "text/html; charset=utf-8"),
    (".jpeg", "image/jpeg"),
    (".jpg", "image/jpeg"),
    (".js", "text/javascript; charset=utf-8"),
    (".json", "application/json"),
    (".mjs", "text/javascript; charset=utf-8"),
    (".pdf", "application/pdf"),
    (".png", "image/png"),
    (".svg", "image/svg+xml"),
    (".wasm", "application/wasm"),
    (".webp", "image/webp"),
    (".xml", "text/xml; charset=utf-8"),
];

/// Maps file extensions to MIME types (case-sensitive).
/// Example: ".Z" => "application/x-compress"
static MIME_TYPES: Lazy<RwLock<HashMap<String, String>>> = Lazy::new(|| {
    RwLock::new(HashMap::new())
});

/// Maps lowercase file extensions to MIME types (case-insensitive).
/// Example: ".z" => "application/x-compress"
static MIME_TYPES_LOWER: Lazy<RwLock<HashMap<String, String>>> = Lazy::new(|| {
    RwLock::new(HashMap::new())
});

/// Maps MIME types to lists of file extensions.
/// Example: "image/jpeg" => [".jpg", ".jpeg"]
static EXTENSIONS: Lazy<RwLock<HashMap<String, Vec<String>>>> = Lazy::new(|| {
    RwLock::new(HashMap::new())
});

/// Ensures MIME types are initialized exactly once.
static INIT: Lazy<()> = Lazy::new(|| {
    init_mime();
});

/// Initializes the MIME type maps with builtin types and platform-specific types.
fn init_mime() {
    // Set builtin types
    set_mime_types_internal(BUILTIN_TYPES_LOWER, BUILTIN_TYPES_LOWER);

    // Load platform-specific types (errors are ignored)
    #[cfg(any(unix, windows))]
    let _ = crate::platform::init_mime();
}

/// Internal function to set MIME type mappings.
fn set_mime_types_internal(lower_ext: &[(&str, &str)], mix_ext: &[(&str, &str)]) {
    let mut mime_types = MIME_TYPES.write().unwrap();
    let mut mime_types_lower = MIME_TYPES_LOWER.write().unwrap();
    let mut extensions = EXTENSIONS.write().unwrap();

    // Clear existing mappings
    mime_types.clear();
    mime_types_lower.clear();
    extensions.clear();

    // Set lowercase mappings
    for (ext, mime) in lower_ext {
        mime_types_lower.insert(ext.to_string(), mime.to_string());
    }

    // Set case-sensitive mappings
    for (ext, mime) in mix_ext {
        mime_types.insert(ext.to_string(), mime.to_string());
    }

    // Build reverse mapping (MIME type -> extensions)
    for (ext, mime) in lower_ext {
        // Parse media type to get just the type without parameters
        if let Ok((just_type, _)) = parse_media_type(mime) {
            extensions
                .entry(just_type)
                .or_insert_with(Vec::new)
                .push(ext.to_string());
        }
    }
}

/// Returns the MIME type associated with the file extension ext.
///
/// The extension ext should begin with a leading dot, as in ".html".
/// When ext has no associated type, returns None.
///
/// Extensions are looked up first case-sensitively, then case-insensitively.
///
/// # Examples
///
/// ```
/// use tokio_mime::type_by_extension;
///
/// assert_eq!(type_by_extension(".html"), Some("text/html; charset=utf-8".to_string()));
/// assert_eq!(type_by_extension(".HTML"), Some("text/html; charset=utf-8".to_string()));
/// assert_eq!(type_by_extension(".jpg"), Some("image/jpeg".to_string()));
/// assert_eq!(type_by_extension(".unknown"), None);
/// ```
pub fn type_by_extension(ext: &str) -> Option<String> {
    // Ensure initialization
    Lazy::force(&INIT);

    // Case-sensitive lookup
    {
        let mime_types = MIME_TYPES.read().unwrap();
        if let Some(mime) = mime_types.get(ext) {
            return Some(mime.clone());
        }
    }

    // Case-insensitive lookup
    // Optimistically assume a short ASCII extension and be allocation-free in that case
    let lower = if ext.is_ascii() {
        // Fast path: use stack buffer for ASCII
        ext.to_ascii_lowercase()
    } else {
        // Slow path: handle UTF-8
        ext.to_lowercase()
    };

    let mime_types_lower = MIME_TYPES_LOWER.read().unwrap();
    mime_types_lower.get(&lower).cloned()
}

/// Returns the extensions known to be associated with the MIME type typ.
///
/// The returned extensions will each begin with a leading dot, as in ".html".
/// When typ has no associated extensions, returns an empty vector.
///
/// # Examples
///
/// ```
/// use tokio_mime::extensions_by_type;
///
/// let exts = extensions_by_type("image/jpeg").unwrap();
/// assert!(exts.contains(&".jpg".to_string()));
/// assert!(exts.contains(&".jpeg".to_string()));
/// ```
pub fn extensions_by_type(mime_type: &str) -> Result<Vec<String>> {
    // Parse media type to get just the type without parameters
    let (just_type, _) = parse_media_type(mime_type)?;

    // Ensure initialization
    Lazy::force(&INIT);

    let extensions = EXTENSIONS.read().unwrap();
    if let Some(exts) = extensions.get(&just_type) {
        let mut ret = exts.clone();
        ret.sort();
        Ok(ret)
    } else {
        Ok(Vec::new())
    }
}

/// Sets the MIME type associated with the extension ext to typ.
///
/// The extension should begin with a leading dot, as in ".html".
///
/// # Examples
///
/// ```
/// use tokio_mime::add_extension_type;
///
/// add_extension_type(".foo", "application/foo").unwrap();
/// ```
pub fn add_extension_type(ext: &str, mime_type: &str) -> Result<()> {
    if !ext.starts_with('.') {
        return Err(Error::MimeType(format!(
            "extension {:?} missing leading dot",
            ext
        )));
    }

    // Ensure initialization
    Lazy::force(&INIT);

    set_extension_type(ext, mime_type)
}

/// Internal function to set an extension type mapping.
/// This is public for use by platform modules during initialization.
/// If skip_if_exists is true, the extension will not be overwritten if it already exists.
pub(crate) fn set_extension_type(extension: &str, mime_type: &str) -> Result<()> {
    set_extension_type_internal(extension, mime_type, false)
}

/// Internal function to set an extension type mapping, used during platform initialization.
/// If skip_if_exists is true, the extension will not be overwritten if it already exists.
pub(crate) fn set_extension_type_skip_existing(extension: &str, mime_type: &str) -> Result<()> {
    set_extension_type_internal(extension, mime_type, true)
}

fn set_extension_type_internal(extension: &str, mime_type: &str, skip_if_exists: bool) -> Result<()> {
    let ext_lower = extension.to_lowercase();

    // Check if extension already exists (for platform loading)
    if skip_if_exists {
        let mime_types_lower = MIME_TYPES_LOWER.read().unwrap();
        if mime_types_lower.contains_key(&ext_lower) {
            return Ok(());
        }
    }

    // Parse media type
    let (just_type, mut params) = parse_media_type(mime_type)?;

    // Add charset=utf-8 for text/* types if not present
    let final_mime_type = if mime_type.starts_with("text/") && !params.contains_key("charset") {
        params.insert("charset".to_string(), "utf-8".to_string());
        format_media_type(&just_type, &params)
    } else {
        mime_type.to_string()
    };

    // Update MIME type mappings
    {
        let mut mime_types = MIME_TYPES.write().unwrap();
        mime_types.insert(extension.to_string(), final_mime_type.clone());
    }
    {
        let mut mime_types_lower = MIME_TYPES_LOWER.write().unwrap();
        mime_types_lower.insert(ext_lower.clone(), final_mime_type.clone());
    }

    // Update reverse mapping (extensions)
    {
        let mut extensions = EXTENSIONS.write().unwrap();
        let exts = extensions.entry(just_type).or_insert_with(Vec::new);

        // Only add if not already present
        if !exts.contains(&ext_lower) {
            exts.push(ext_lower);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_by_extension() {
        assert_eq!(
            type_by_extension(".html"),
            Some("text/html; charset=utf-8".to_string())
        );
        assert_eq!(
            type_by_extension(".HTML"),
            Some("text/html; charset=utf-8".to_string())
        );
        assert_eq!(type_by_extension(".jpg"), Some("image/jpeg".to_string()));
        assert_eq!(type_by_extension(".jpeg"), Some("image/jpeg".to_string()));
        assert_eq!(type_by_extension(".png"), Some("image/png".to_string()));
        assert_eq!(type_by_extension(".unknown"), None);
    }

    #[test]
    fn test_extensions_by_type() {
        let exts = extensions_by_type("image/jpeg").unwrap();
        // Check that at least the builtin extensions are present
        assert!(exts.contains(&".jpg".to_string()));
        assert!(exts.contains(&".jpeg".to_string()));
        // Platform-specific databases may add more extensions (e.g., .jpe, .jfif)
        assert!(exts.len() >= 2);
    }

    #[test]
    fn test_add_extension_type() {
        // Test error case
        let result = add_extension_type("foo", "application/foo");
        assert!(result.is_err());

        // Test success case
        add_extension_type(".test", "application/test").unwrap();
        assert_eq!(
            type_by_extension(".test"),
            Some("application/test".to_string())
        );
    }
}
