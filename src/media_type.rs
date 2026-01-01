//! Media type parsing and formatting.
//!
//! Implements RFC 2045, RFC 2616, and RFC 2231 media type handling.

use crate::error::{Error, Result};
use crate::grammar::{is_token, is_token_char, is_tspecial};
use std::collections::HashMap;

const UPPER_HEX: &[u8] = b"0123456789ABCDEF";

/// Parses a media type value and any optional parameters, per RFC 1521.
///
/// Media types are the values in Content-Type and Content-Disposition headers (RFC 2183).
/// Returns the media type converted to lowercase and a map of parameters.
///
/// # Examples
///
/// ```
/// use mime_rs::parse_media_type;
///
/// let (media_type, params) = parse_media_type("text/html; charset=utf-8").unwrap();
/// assert_eq!(media_type, "text/html");
/// assert_eq!(params.get("charset"), Some(&"utf-8".to_string()));
/// ```
pub fn parse_media_type(v: &str) -> Result<(String, HashMap<String, String>)> {
    // Split on first semicolon to get base type
    let (base, rest) = v.split_once(';').unwrap_or((v, ""));
    let mediatype = base.trim().to_lowercase();

    // Validate media type format
    if let Some((major, sub)) = mediatype.split_once('/') {
        if !is_token(major) || !is_token(sub) {
            return Err(Error::MediaType("invalid media type format".to_string()));
        }
    } else {
        return Err(Error::MediaType("no media type".to_string()));
    }

    let mut params = HashMap::new();

    // Simple parameter parsing (TODO: implement RFC 2231 continuation)
    if !rest.is_empty() {
        for param in rest.split(';') {
            let param = param.trim();
            if param.is_empty() {
                continue;
            }

            if let Some((key, value)) = param.split_once('=') {
                let key = key.trim().to_lowercase();
                let value = value.trim();

                // Remove quotes if present
                let value = if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
                    &value[1..value.len()-1]
                } else {
                    value
                };

                params.insert(key, value.to_string());
            }
        }
    }

    Ok((mediatype, params))
}

/// Serializes a media type and parameters as a media type conforming to RFC 2045 and RFC 2616.
///
/// The type and parameter names are written in lower-case.
///
/// # Examples
///
/// ```
/// use mime_rs::format_media_type;
/// use std::collections::HashMap;
///
/// let mut params = HashMap::new();
/// params.insert("charset".to_string(), "utf-8".to_string());
/// let formatted = format_media_type("text/html", &params);
/// assert_eq!(formatted, "text/html; charset=utf-8");
/// ```
pub fn format_media_type(t: &str, params: &HashMap<String, String>) -> String {
    let mut result = String::new();

    // Validate and format the media type
    if let Some((major, sub)) = t.split_once('/') {
        if !is_token(major) || !is_token(sub) {
            return String::new();
        }
        result.push_str(&major.to_lowercase());
        result.push('/');
        result.push_str(&sub.to_lowercase());
    } else {
        if !is_token(t) {
            return String::new();
        }
        result.push_str(&t.to_lowercase());
    }

    // Sort parameters for consistent output
    let mut keys: Vec<_> = params.keys().collect();
    keys.sort();

    for key in keys {
        let value = &params[key];

        if !is_token(key) {
            return String::new();
        }

        result.push_str("; ");
        result.push_str(&key.to_lowercase());

        // Check if value needs encoding
        let needs_encoding = needs_encoding(value);

        if needs_encoding {
            // RFC 2231 encoding
            result.push_str("*=utf-8''");
            for &b in value.as_bytes() {
                if b <= b' ' || b >= 0x7F || b == b'*' || b == b'\'' || b == b'%' || is_tspecial(b as char) {
                    result.push('%');
                    result.push(UPPER_HEX[(b >> 4) as usize] as char);
                    result.push(UPPER_HEX[(b & 0x0F) as usize] as char);
                } else {
                    result.push(b as char);
                }
            }
        } else if is_token(value) {
            result.push('=');
            result.push_str(value);
        } else {
            // Quote the value
            result.push_str("=\"");
            for ch in value.chars() {
                if ch == '"' || ch == '\\' {
                    result.push('\\');
                }
                result.push(ch);
            }
            result.push('"');
        }
    }

    result
}

/// Checks if a string needs encoding per RFC 2231.
fn needs_encoding(s: &str) -> bool {
    for ch in s.chars() {
        if (ch < ' ' || ch > '~') && ch != '\t' {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_media_type_simple() {
        let (media_type, params) = parse_media_type("text/html").unwrap();
        assert_eq!(media_type, "text/html");
        assert!(params.is_empty());
    }

    #[test]
    fn test_parse_media_type_with_charset() {
        let (media_type, params) = parse_media_type("text/html; charset=utf-8").unwrap();
        assert_eq!(media_type, "text/html");
        assert_eq!(params.get("charset"), Some(&"utf-8".to_string()));
    }

    #[test]
    fn test_parse_media_type_quoted_value() {
        let (media_type, params) = parse_media_type("text/html; charset=\"utf-8\"").unwrap();
        assert_eq!(media_type, "text/html");
        assert_eq!(params.get("charset"), Some(&"utf-8".to_string()));
    }

    #[test]
    fn test_format_media_type_simple() {
        let params = HashMap::new();
        let formatted = format_media_type("text/html", &params);
        assert_eq!(formatted, "text/html");
    }

    #[test]
    fn test_format_media_type_with_params() {
        let mut params = HashMap::new();
        params.insert("charset".to_string(), "utf-8".to_string());
        let formatted = format_media_type("text/html", &params);
        assert_eq!(formatted, "text/html; charset=utf-8");
    }

    #[test]
    fn test_format_media_type_quoted() {
        // Test with a value that needs quoting (contains spaces)
        let mut params = HashMap::new();
        params.insert("name".to_string(), "hello world".to_string());
        let formatted = format_media_type("text/plain", &params);
        assert_eq!(formatted, "text/plain; name=\"hello world\"");
    }

    #[test]
    fn test_format_media_type_boundary() {
        let mut params = HashMap::new();
        params.insert("boundary".to_string(), "----boundary".to_string());
        let formatted = format_media_type("multipart/form-data", &params);
        // "----boundary" is a valid token, doesn't need quotes
        assert_eq!(formatted, "multipart/form-data; boundary=----boundary");
    }
}
