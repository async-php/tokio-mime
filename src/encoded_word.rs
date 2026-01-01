//! RFC 2047 encoded-word encoding and decoding.

use crate::error::Result;

/// An RFC 2047 encoded-word encoder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WordEncoder {
    /// Base64 encoding scheme as defined by RFC 2045.
    BEncoding,
    /// Q-encoding scheme as defined by RFC 2047.
    QEncoding,
}

/// An RFC 2047 encoded-word decoder.
#[derive(Debug, Default)]
pub struct WordDecoder {
    // TODO: Add charset reader field
}

impl WordEncoder {
    /// Returns the encoded-word form of s.
    pub fn encode(&self, charset: &str, s: &str) -> String {
        // TODO: Implement
        String::new()
    }
}

impl WordDecoder {
    /// Creates a new WordDecoder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Decodes an RFC 2047 encoded-word.
    pub fn decode(&self, word: &str) -> Result<String> {
        // TODO: Implement
        Ok(String::new())
    }

    /// Decodes all encoded-words of the given string.
    pub fn decode_header(&self, header: &str) -> Result<String> {
        // TODO: Implement
        Ok(String::new())
    }
}
