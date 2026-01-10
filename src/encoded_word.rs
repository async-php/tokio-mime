//! RFC 2047 encoded-word encoding and decoding.
//!
//! This module implements MIME encoded-word processing as defined in RFC 2047.

use crate::error::{Error, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};

const UPPER_HEX: &[u8] = b"0123456789ABCDEF";
const MAX_ENCODED_WORD_LEN: usize = 75;
const MAX_CONTENT_LEN: usize = MAX_ENCODED_WORD_LEN - "=?UTF-8?q?".len() - "?=".len();

/// An RFC 2047 encoded-word encoder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WordEncoder {
    /// Base64 encoding scheme as defined by RFC 2045.
    BEncoding,
    /// Q-encoding scheme as defined by RFC 2047.
    QEncoding,
}

/// An RFC 2047 encoded-word decoder.
#[derive(Default)]
pub struct WordDecoder {
    /// Custom charset reader function (optional).
    /// For charsets other than UTF-8, ISO-8859-1, and US-ASCII.
    pub charset_reader: Option<Box<dyn Fn(&str, &[u8]) -> Result<String> + Send + Sync>>,
}

impl std::fmt::Debug for WordDecoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WordDecoder")
            .field("charset_reader", &self.charset_reader.as_ref().map(|_| "<function>"))
            .finish()
    }
}

impl WordEncoder {
    /// Returns the encoded-word form of s.
    ///
    /// If s is ASCII without special characters, it is returned unchanged.
    /// The provided charset is the IANA charset name of s (case insensitive).
    ///
    /// # Examples
    ///
    /// ```
    /// use yamine::WordEncoder;
    ///
    /// let encoder = WordEncoder::QEncoding;
    /// let encoded = encoder.encode("UTF-8", "Hello, 世界");
    /// assert!(encoded.starts_with("=?UTF-8?q?"));
    /// ```
    pub fn encode(&self, charset: &str, s: &str) -> String {
        if !needs_encoding(s) {
            return s.to_string();
        }
        self.encode_word(charset, s)
    }

    /// Encodes a string into an encoded-word.
    fn encode_word(&self, charset: &str, s: &str) -> String {
        let mut buf = String::with_capacity(48);

        self.open_word(&mut buf, charset);
        match self {
            WordEncoder::BEncoding => self.b_encode(&mut buf, charset, s),
            WordEncoder::QEncoding => self.q_encode(&mut buf, charset, s),
        }
        close_word(&mut buf);

        buf
    }

    /// Base64 encoding.
    fn b_encode(&self, buf: &mut String, charset: &str, s: &str) {
        let encoded = BASE64.encode(s.as_bytes());

        // If short enough, write it all
        if !is_utf8(charset) || encoded.len() <= MAX_CONTENT_LEN {
            buf.push_str(&encoded);
            return;
        }

        // Need to split for UTF-8 content
        let max_decoded = BASE64.decode(&vec![b'A'; MAX_CONTENT_LEN]).unwrap().len();
        let mut last = 0;
        let mut current_len = 0;

        for (i, ch) in s.char_indices() {
            let char_len = ch.len_utf8();
            if current_len + char_len <= max_decoded {
                current_len += char_len;
            } else {
                // Split here
                let chunk = &s[last..i];
                buf.push_str(&BASE64.encode(chunk.as_bytes()));
                self.split_word(buf, charset);
                last = i;
                current_len = char_len;
            }
        }

        // Write remaining
        if last < s.len() {
            buf.push_str(&BASE64.encode(s[last..].as_bytes()));
        }
    }

    /// Q encoding.
    fn q_encode(&self, buf: &mut String, charset: &str, s: &str) {
        if !is_utf8(charset) {
            write_q_string(buf, s);
            return;
        }

        let mut current_len = 0;

        for (i, ch) in s.char_indices() {
            let b = s.as_bytes()[i];
            let (char_len, enc_len) = if ch.is_ascii()
                && b >= b' '
                && b <= b'~'
                && b != b'='
                && b != b'?'
                && b != b'_'
            {
                (ch.len_utf8(), 1)
            } else {
                (ch.len_utf8(), 3 * ch.len_utf8())
            };

            if current_len + enc_len > MAX_CONTENT_LEN {
                self.split_word(buf, charset);
                current_len = 0;
            }

            write_q_string(buf, &s[i..i + char_len]);
            current_len += enc_len;
        }
    }

    fn open_word(&self, buf: &mut String, charset: &str) {
        buf.push_str("=?");
        buf.push_str(charset);
        buf.push('?');
        buf.push(match self {
            WordEncoder::BEncoding => 'b',
            WordEncoder::QEncoding => 'q',
        });
        buf.push('?');
    }

    fn split_word(&self, buf: &mut String, charset: &str) {
        close_word(buf);
        buf.push(' ');
        self.open_word(buf, charset);
    }
}

impl WordDecoder {
    /// Creates a new WordDecoder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Decodes an RFC 2047 encoded-word.
    ///
    /// # Examples
    ///
    /// ```
    /// use yamine::WordDecoder;
    ///
    /// let decoder = WordDecoder::new();
    /// let decoded = decoder.decode("=?UTF-8?q?Hello?=").unwrap();
    /// assert_eq!(decoded, "Hello");
    /// ```
    pub fn decode(&self, word: &str) -> Result<String> {
        // Validate format: =?charset?encoding?text?=
        if word.len() < 8
            || !word.starts_with("=?")
            || !word.ends_with("?=")
            || word.matches('?').count() != 4
        {
            return Err(Error::Encoding("invalid RFC 2047 encoded-word".to_string()));
        }

        let word = &word[2..word.len() - 2];

        // Split into charset, encoding, text
        let parts: Vec<&str> = word.split('?').collect();
        if parts.len() != 3 || parts[0].is_empty() || parts[1].len() != 1 {
            return Err(Error::Encoding("invalid encoded-word format".to_string()));
        }

        let charset = parts[0];
        let encoding = parts[1].as_bytes()[0];
        let text = parts[2];

        let content = decode_content(encoding, text)?;
        self.convert(charset, &content)
    }

    /// Decodes all encoded-words in the given string.
    ///
    /// # Examples
    ///
    /// ```
    /// use yamine::WordDecoder;
    ///
    /// let decoder = WordDecoder::new();
    /// let decoded = decoder.decode_header("Subject: =?UTF-8?q?Hello?=").unwrap();
    /// assert_eq!(decoded, "Subject: Hello");
    /// ```
    pub fn decode_header(&self, header: &str) -> Result<String> {
        // Quick check if there's anything to decode
        if !header.contains("=?") {
            return Ok(header.to_string());
        }

        let mut result = String::new();
        let mut remaining = header;
        let mut between_words = false;

        while let Some(start) = remaining.find("=?") {
            let mut cur = start + 2;

            // Find charset
            let charset_end = match remaining[cur..].find('?') {
                Some(pos) => cur + pos,
                None => break,
            };
            let charset = &remaining[cur..charset_end];
            cur = charset_end + 1;

            // Check minimum length
            if remaining.len() < cur + 3 {
                break;
            }

            // Get encoding
            let encoding = remaining.as_bytes()[cur];
            cur += 1;

            // Check separator
            if remaining.as_bytes()[cur] != b'?' {
                break;
            }
            cur += 1;

            // Find end
            let end_pos = match remaining[cur..].find("?=") {
                Some(pos) => cur + pos,
                None => break,
            };
            let text = &remaining[cur..end_pos];
            let end = end_pos + 2;

            // Try to decode
            match decode_content(encoding, text) {
                Ok(content) => {
                    // Add text before encoded-word (but skip whitespace between encoded-words)
                    if start > 0 && (!between_words || has_non_whitespace(&remaining[..start])) {
                        result.push_str(&remaining[..start]);
                    }

                    // Add decoded content
                    result.push_str(&self.convert(charset, &content)?);
                    remaining = &remaining[end..];
                    between_words = true;
                    continue;
                }
                Err(_) => {
                    // Failed to decode, skip this and continue
                    result.push_str(&remaining[..start + 2]);
                    remaining = &remaining[start + 2..];
                    between_words = false;
                    continue;
                }
            }
        }

        // Add remaining text
        if !remaining.is_empty() {
            result.push_str(remaining);
        }

        Ok(result)
    }

    /// Converts content from the given charset to UTF-8.
    fn convert(&self, charset: &str, content: &[u8]) -> Result<String> {
        if charset.eq_ignore_ascii_case("utf-8") {
            return String::from_utf8(content.to_vec())
                .map_err(|e| Error::Encoding(format!("invalid UTF-8: {}", e)));
        }

        if charset.eq_ignore_ascii_case("iso-8859-1") {
            // ISO-8859-1 maps directly to Unicode code points
            return Ok(content.iter().map(|&b| b as char).collect());
        }

        if charset.eq_ignore_ascii_case("us-ascii") {
            // US-ASCII - replace non-ASCII with replacement char
            return Ok(content
                .iter()
                .map(|&b| if b < 128 { b as char } else { '\u{FFFD}' })
                .collect());
        }

        // Try custom charset reader
        if let Some(ref reader) = self.charset_reader {
            return reader(&charset.to_lowercase(), content);
        }

        Err(Error::Encoding(format!("unhandled charset: {}", charset)))
    }
}

/// Checks if a string needs encoding.
fn needs_encoding(s: &str) -> bool {
    s.chars()
        .any(|ch| (ch < ' ' || ch > '~') && ch != '\t')
}

/// Writes the closing marker of an encoded-word.
fn close_word(buf: &mut String) {
    buf.push_str("?=");
}

/// Checks if charset is UTF-8.
fn is_utf8(charset: &str) -> bool {
    charset.eq_ignore_ascii_case("UTF-8")
}

/// Encodes a string using Q encoding.
fn write_q_string(buf: &mut String, s: &str) {
    for &b in s.as_bytes() {
        match b {
            b' ' => buf.push('_'),
            b'!' ..= b'~' if b != b'=' && b != b'?' && b != b'_' => buf.push(b as char),
            _ => {
                buf.push('=');
                buf.push(UPPER_HEX[(b >> 4) as usize] as char);
                buf.push(UPPER_HEX[(b & 0x0F) as usize] as char);
            }
        }
    }
}

/// Decodes content based on encoding type.
fn decode_content(encoding: u8, text: &str) -> Result<Vec<u8>> {
    match encoding {
        b'B' | b'b' => BASE64
            .decode(text.as_bytes())
            .map_err(|e| Error::Encoding(format!("base64 decode error: {}", e))),
        b'Q' | b'q' => q_decode(text),
        _ => Err(Error::Encoding("invalid encoding type".to_string())),
    }
}

/// Decodes a Q-encoded string.
fn q_decode(s: &str) -> Result<Vec<u8>> {
    let mut result = Vec::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        match bytes[i] {
            b'_' => {
                result.push(b' ');
                i += 1;
            }
            b'=' => {
                if i + 2 >= bytes.len() {
                    return Err(Error::Encoding("truncated Q encoding".to_string()));
                }
                let high = from_hex(bytes[i + 1])?;
                let low = from_hex(bytes[i + 2])?;
                result.push((high << 4) | low);
                i += 3;
            }
            b' ' ..= b'~' | b'\n' | b'\r' | b'\t' => {
                result.push(bytes[i]);
                i += 1;
            }
            _ => {
                return Err(Error::Encoding(format!(
                    "invalid character in Q encoding: {:02x}",
                    bytes[i]
                )));
            }
        }
    }

    Ok(result)
}

/// Converts a hex digit to its value.
fn from_hex(b: u8) -> Result<u8> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        _ => Err(Error::Encoding(format!("invalid hex digit: {:02x}", b))),
    }
}

/// Checks if a string contains non-whitespace characters.
fn has_non_whitespace(s: &str) -> bool {
    s.bytes().any(|b| !matches!(b, b' ' | b'\t' | b'\n' | b'\r'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_needs_encoding() {
        assert!(!needs_encoding("Hello"));
        assert!(needs_encoding("Hello 世界"));
        assert!(!needs_encoding("test@example.com"));
    }

    #[test]
    fn test_q_encoding() {
        let encoder = WordEncoder::QEncoding;
        let encoded = encoder.encode("UTF-8", "Hello");
        assert_eq!(encoded, "Hello"); // No encoding needed

        // Use non-ASCII to trigger encoding
        let encoded = encoder.encode("UTF-8", "Héllo");
        assert!(encoded.starts_with("=?UTF-8?q?"));
        assert!(encoded.ends_with("?="));
        assert!(encoded.contains("=C3=A9")); // é encoded
    }

    #[test]
    fn test_b_encoding() {
        let encoder = WordEncoder::BEncoding;
        let encoded = encoder.encode("UTF-8", "Hello 世界");
        assert!(encoded.starts_with("=?UTF-8?b?"));
        assert!(encoded.ends_with("?="));
    }

    #[test]
    fn test_decode_simple() {
        let decoder = WordDecoder::new();
        let decoded = decoder.decode("=?UTF-8?q?Hello?=").unwrap();
        assert_eq!(decoded, "Hello");
    }

    #[test]
    fn test_decode_base64() {
        let decoder = WordDecoder::new();
        let decoded = decoder.decode("=?UTF-8?b?SGVsbG8=?=").unwrap();
        assert_eq!(decoded, "Hello");
    }

    #[test]
    fn test_decode_header() {
        let decoder = WordDecoder::new();
        let decoded = decoder
            .decode_header("Subject: =?UTF-8?q?Hello?= World")
            .unwrap();
        assert_eq!(decoded, "Subject: Hello World");
    }

    #[test]
    fn test_decode_multiple_words() {
        let decoder = WordDecoder::new();
        let decoded = decoder
            .decode_header("=?UTF-8?q?Hello?= =?UTF-8?q?World?=")
            .unwrap();
        assert_eq!(decoded, "HelloWorld"); // Whitespace between words is removed
    }

    #[test]
    fn test_roundtrip() {
        let encoder = WordEncoder::QEncoding;
        let decoder = WordDecoder::new();
        let original = "Hello, 世界!";
        let encoded = encoder.encode("UTF-8", original);
        let decoded = decoder.decode(&encoded).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_b_encoding_roundtrip() {
        let encoder = WordEncoder::BEncoding;
        let decoder = WordDecoder::new();
        let original = "Hello, 世界!";
        let encoded = encoder.encode("UTF-8", original);
        let decoded = decoder.decode(&encoded).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_encode_ascii_no_change() {
        // Pure ASCII should not be encoded
        let encoder = WordEncoder::QEncoding;
        let result = encoder.encode("UTF-8", "Hello World");
        assert_eq!(result, "Hello World");
    }

    #[test]
    fn test_encode_with_special_chars() {
        let encoder = WordEncoder::QEncoding;
        let encoded = encoder.encode("UTF-8", "test@example.com");
        // Simple ASCII with @ should not need encoding
        assert_eq!(encoded, "test@example.com");
    }

    #[test]
    fn test_decode_invalid_format() {
        let decoder = WordDecoder::new();
        // Missing closing ?=
        let result = decoder.decode("=?UTF-8?q?Hello");
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_invalid_encoding_type() {
        let decoder = WordDecoder::new();
        // Invalid encoding type 'x'
        let result = decoder.decode("=?UTF-8?x?Hello?=");
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_empty_encoded_word() {
        let decoder = WordDecoder::new();
        let decoded = decoder.decode("=?UTF-8?q??=").unwrap();
        assert_eq!(decoded, "");
    }

    #[test]
    fn test_decode_q_with_underscores() {
        // In Q encoding, underscores represent spaces
        let decoder = WordDecoder::new();
        let decoded = decoder.decode("=?UTF-8?q?Hello_World?=").unwrap();
        assert_eq!(decoded, "Hello World");
    }

    #[test]
    fn test_decode_mixed_text() {
        let decoder = WordDecoder::new();
        let decoded = decoder
            .decode_header("Normal text =?UTF-8?q?encoded?= more text")
            .unwrap();
        assert_eq!(decoded, "Normal text encoded more text");
    }

    #[test]
    fn test_encode_long_string() {
        // Test encoding of moderately long strings
        let encoder = WordEncoder::BEncoding;
        let long_text = "这是一个测试字符串";
        let encoded = encoder.encode("UTF-8", long_text);

        // Should be encoded
        assert!(encoded.starts_with("=?UTF-8?b?"));

        // Should be decodable
        let decoder = WordDecoder::new();
        let decoded = decoder.decode(&encoded).unwrap();
        assert_eq!(decoded, long_text);
    }

    #[test]
    fn test_decode_case_insensitive_charset() {
        let decoder = WordDecoder::new();
        // UTF-8 should work regardless of case
        let decoded1 = decoder.decode("=?utf-8?q?Hello?=").unwrap();
        let decoded2 = decoder.decode("=?UTF-8?q?Hello?=").unwrap();
        let decoded3 = decoder.decode("=?Utf-8?q?Hello?=").unwrap();

        assert_eq!(decoded1, "Hello");
        assert_eq!(decoded2, "Hello");
        assert_eq!(decoded3, "Hello");
    }

    #[test]
    fn test_decode_case_insensitive_encoding() {
        let decoder = WordDecoder::new();
        // Q and q should both work
        let decoded1 = decoder.decode("=?UTF-8?Q?Hello?=").unwrap();
        let decoded2 = decoder.decode("=?UTF-8?q?Hello?=").unwrap();

        assert_eq!(decoded1, decoded2);
    }

    #[test]
    fn test_q_decode_hex_values() {
        let decoder = WordDecoder::new();
        // Test various hex encoded values
        let decoded = decoder.decode("=?UTF-8?q?=C3=A9?=").unwrap();
        assert_eq!(decoded, "é");
    }

    #[test]
    fn test_decode_truncated_q_encoding() {
        let decoder = WordDecoder::new();
        // Truncated hex sequence
        let result = decoder.decode("=?UTF-8?q?test=C?=");
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_invalid_hex() {
        let decoder = WordDecoder::new();
        // Invalid hex characters
        let result = decoder.decode("=?UTF-8?q?test=GG?=");
        assert!(result.is_err());
    }

    #[test]
    fn test_encode_with_tabs() {
        let encoder = WordEncoder::QEncoding;
        let text_with_tab = "Hello\tWorld";
        let encoded = encoder.encode("UTF-8", text_with_tab);
        // Tab is allowed in needs_encoding check, so it won't be encoded
        assert_eq!(encoded, "Hello\tWorld");
    }

    #[test]
    fn test_decode_adjacent_encoded_words() {
        // Adjacent encoded words with whitespace between should have whitespace removed
        let decoder = WordDecoder::new();
        let decoded = decoder
            .decode_header("=?UTF-8?q?Part1?= =?UTF-8?q?Part2?=")
            .unwrap();
        assert_eq!(decoded, "Part1Part2");
    }

    #[test]
    fn test_decode_header_no_encoded_words() {
        let decoder = WordDecoder::new();
        let plain = "This is plain text";
        let decoded = decoder.decode_header(plain).unwrap();
        assert_eq!(decoded, plain);
    }

    #[test]
    fn test_needs_encoding_function() {
        assert!(!needs_encoding("Simple ASCII"));
        assert!(needs_encoding("Non-ASCII: é"));
        assert!(needs_encoding("Chinese: 中文"));
        assert!(needs_encoding("Emoji: 😀"));
        assert!(!needs_encoding("Numbers123"));
        assert!(needs_encoding("Control\x00char"));
    }

    #[test]
    fn test_decode_base64_padding() {
        let decoder = WordDecoder::new();
        // Test base64 with proper padding
        let decoded = decoder.decode("=?UTF-8?b?SGVsbG8gV29ybGQ=?=").unwrap();
        assert_eq!(decoded, "Hello World");
    }

    #[test]
    fn test_encode_empty_string() {
        let encoder = WordEncoder::QEncoding;
        let encoded = encoder.encode("UTF-8", "");
        assert_eq!(encoded, "");
    }

    #[test]
    fn test_charset_us_ascii() {
        let decoder = WordDecoder::new();
        // Test US-ASCII charset
        let decoded = decoder.decode("=?US-ASCII?q?Hello?=").unwrap();
        assert_eq!(decoded, "Hello");
    }
}
