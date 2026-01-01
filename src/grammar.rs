//! Grammar validation helpers for MIME tokens.
//!
//! Based on RFC 1521 and RFC 2045 token definitions.

/// Reports whether the character is in 'tspecials' as defined by RFC 1521 and RFC 2045.
///
/// tspecials := "(" / ")" / "<" / ">" / "@" / "," / ";" / ":" / "\" / <"> / "/" / "[" / "]" / "?" / "="
pub fn is_tspecial(c: char) -> bool {
    matches!(c, '(' | ')' | '<' | '>' | '@' | ',' | ';' | ':' | '\\' | '"' | '/' | '[' | ']' | '?' | '=')
}

/// Reports whether the character is in 'token' as defined by RFC 1521 and RFC 2045.
///
/// token := 1*<any (US-ASCII) CHAR except SPACE, CTLs, or tspecials>
pub fn is_token_char(c: char) -> bool {
    c > '\x20' && c < '\x7f' && !is_tspecial(c)
}

/// Reports whether the character is NOT a token character.
pub fn is_not_token_char(c: char) -> bool {
    !is_token_char(c)
}

/// Reports whether the string is a valid 'token' as defined by RFC 1521 and RFC 2045.
///
/// A token must be non-empty and contain only valid token characters.
pub fn is_token(s: &str) -> bool {
    !s.is_empty() && s.chars().all(is_token_char)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_tspecial() {
        assert!(is_tspecial('('));
        assert!(is_tspecial(')'));
        assert!(is_tspecial('<'));
        assert!(is_tspecial('>'));
        assert!(is_tspecial('@'));
        assert!(is_tspecial(','));
        assert!(is_tspecial(';'));
        assert!(is_tspecial(':'));
        assert!(is_tspecial('\\'));
        assert!(is_tspecial('"'));
        assert!(is_tspecial('/'));
        assert!(is_tspecial('['));
        assert!(is_tspecial(']'));
        assert!(is_tspecial('?'));
        assert!(is_tspecial('='));

        assert!(!is_tspecial('a'));
        assert!(!is_tspecial('Z'));
        assert!(!is_tspecial('0'));
    }

    #[test]
    fn test_is_token_char() {
        assert!(is_token_char('a'));
        assert!(is_token_char('Z'));
        assert!(is_token_char('0'));
        assert!(is_token_char('-'));
        assert!(is_token_char('_'));

        assert!(!is_token_char(' '));
        assert!(!is_token_char('\t'));
        assert!(!is_token_char('('));
        assert!(!is_token_char('\x1f')); // control character
    }

    #[test]
    fn test_is_token() {
        assert!(is_token("text"));
        assert!(is_token("application"));
        assert!(is_token("test-value"));

        assert!(!is_token(""));
        assert!(!is_token("text/plain"));
        assert!(!is_token("with space"));
        assert!(!is_token("with(paren"));
    }
}
