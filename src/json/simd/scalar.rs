//! Portable reference implementation of structural character classification.
//!
//! SIMD backends (`avx2`, `neon`) MUST produce identical bitmasks.

use super::StructuralBitmasks;

#[inline]
fn is_whitespace(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\r')
}

#[inline]
fn is_structural(b: u8) -> bool {
    matches!(b, b'{' | b'}' | b'[' | b']' | b':' | b',')
}

/// Classify `input` into the four bitmask groups (scalar reference).
#[cfg_attr(not(test), allow(dead_code))]
pub fn compute_bitmasks(input: &[u8]) -> StructuralBitmasks {
    // Empty input: one all-zero block keeps indexing simple for callers.
    let n_blocks = if input.is_empty() {
        1
    } else {
        input.len().div_ceil(64)
    };

    let mut whitespace = vec![0u64; n_blocks];
    let mut quote = vec![0u64; n_blocks];
    let mut backslash = vec![0u64; n_blocks];
    let mut structural = vec![0u64; n_blocks];

    for (i, &b) in input.iter().enumerate() {
        let word = i / 64;
        let bit = i % 64;
        let mask = 1u64 << bit;
        if is_whitespace(b) {
            whitespace[word] |= mask;
        }
        if b == b'"' {
            quote[word] |= mask;
        }
        if b == b'\\' {
            backslash[word] |= mask;
        }
        if is_structural(b) {
            structural[word] |= mask;
        }
    }

    StructuralBitmasks {
        whitespace,
        quote,
        backslash,
        structural,
        input_len: input.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_simple_object() {
        // indices: 0:{ 1:sp 2:" 3:a 4:" 5:: 6:1 7:}
        let input = br#"{ "a":1}"#;
        let bm = compute_bitmasks(input);
        assert_eq!(bm.structural[0] & (1 << 0), 1 << 0); // {
        assert_eq!(bm.whitespace[0] & (1 << 1), 1 << 1); // space
        assert_eq!(bm.quote[0] & (1 << 2), 1 << 2); // "
        assert_eq!(bm.quote[0] & (1 << 4), 1 << 4); // "
        assert_eq!(bm.structural[0] & (1 << 5), 1 << 5); // :
        assert_eq!(bm.structural[0] & (1 << 7), 1 << 7); // }
        assert_eq!(bm.backslash[0], 0);
    }

    #[test]
    fn finds_backslash() {
        // indices: 0:" 1:a 2:\ 3:" 4:b 5:"
        let input = br#""a\"b""#;
        let bm = compute_bitmasks(input);
        assert_ne!(bm.backslash[0] & (1 << 2), 0);
    }
}
