//! Portable classify reference; SIMD backends must match exactly.

use super::{Bitmask, StructuralBitmasks};

#[inline]
fn is_whitespace(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\r')
}

#[inline]
fn is_structural(b: u8) -> bool {
    matches!(b, b'{' | b'}' | b'[' | b']' | b':' | b',')
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn compute_bitmasks(input: &[u8]) -> StructuralBitmasks {
    let mut whitespace = Bitmask::zeros(input.len());
    let mut quote = Bitmask::zeros(input.len());
    let mut backslash = Bitmask::zeros(input.len());
    let mut structural = Bitmask::zeros(input.len());

    for (i, &b) in input.iter().enumerate() {
        if is_whitespace(b) {
            whitespace.set(i);
        }
        if b == b'"' {
            quote.set(i);
        }
        if b == b'\\' {
            backslash.set(i);
        }
        if is_structural(b) {
            structural.set(i);
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
        let input = br#"{ "a":1}"#;
        let bm = compute_bitmasks(input);
        assert!(bm.structural.get(0));
        assert!(bm.whitespace.get(1));
        assert!(bm.quote.get(2));
        assert!(bm.quote.get(4));
        assert!(bm.structural.get(5));
        assert!(bm.structural.get(7));
        assert!(bm.backslash.positions().is_empty());
    }

    #[test]
    fn finds_backslash() {
        let input = br#""a\"b""#;
        let bm = compute_bitmasks(input);
        assert!(bm.backslash.get(2));
    }
}
