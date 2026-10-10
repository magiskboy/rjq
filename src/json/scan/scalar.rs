//! Scalar byte-scan fallbacks.
//!
//! These functions define the **reference semantics** for byte scanning: every
//! SIMD backend in this module (`avx2`, `neon`) MUST produce identical results.
//! The arch backends enforce this with `*_dispatch_matches_scalar` tests. When
//! adding a new scan (e.g. a future number-boundary scan), add the scalar
//! version here first and treat it as the source of truth.

/// Index of the first JSON string "special" byte in `haystack`, or `None` if
/// every byte is safe unescaped content.
///
/// A special byte is `"`, `\`, or a control byte (`< 0x20`).
#[inline]
pub fn find_string_special(haystack: &[u8]) -> Option<usize> {
    haystack
        .iter()
        .position(|&b| b == b'"' || b == b'\\' || b < 0x20)
}

/// Index of the first non-JSON-whitespace byte (`space` / `\t` / `\n` / `\r`),
/// or `None` if the whole slice is whitespace.
#[inline]
pub fn find_non_whitespace(haystack: &[u8]) -> Option<usize> {
    haystack
        .iter()
        .position(|&b| !matches!(b, b' ' | b'\t' | b'\n' | b'\r'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_special_finds_quote_backslash_control() {
        assert_eq!(find_string_special(b"abc"), None);
        assert_eq!(find_string_special(b"ab\"c"), Some(2));
        assert_eq!(find_string_special(b"a\\b"), Some(1));
        assert_eq!(find_string_special(b"ab\n"), Some(2));
        assert_eq!(find_string_special(b"\txyz"), Some(0));
    }

    #[test]
    fn non_whitespace_finds_first_content_byte() {
        assert_eq!(find_non_whitespace(b"   "), None);
        assert_eq!(find_non_whitespace(b"  x"), Some(2));
        assert_eq!(find_non_whitespace(b"\t\n\r{"), Some(3));
        assert_eq!(find_non_whitespace(b"x"), Some(0));
    }
}
