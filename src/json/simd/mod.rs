//! SIMD JSON parser (simdjson-style), built incrementally.
//!
//! 1. **Classify** — SIMD scan → four character-class bitmasks  
//! 2. **Blend** — escape + in-string → true structural-start mask  
//! 3. **Index** — flatten set bits → ascending byte offsets  
//! 4. **Parse** — recursive descent over the index (same hooks as [`super::parser::JsonParser`])
//!
//! Bit layout: for a bitmask covering a 64-byte block, bit `i` is set iff the
//! byte at absolute index `block * 64 + i` belongs to that character class.

pub mod blend;
pub mod index;
pub mod lexer;
pub mod parser;
pub(crate) mod scalar;
pub mod stage1;

#[cfg(target_arch = "x86_64")]
mod avx2;
#[cfg(target_arch = "aarch64")]
mod neon;

pub use blend::BlendedBitmasks;
pub use index::StructuralIndex;
pub use lexer::SIMDLexer;
pub use parser::SIMDParser;

/// Four character-class bitmasks over the input (simdjson stage-1 style).
///
/// Each `Vec<u64>` has `ceil(input_len / 64)` entries. Entry `k` covers bytes
/// `[k*64, k*64+64)`; bit `i` of that word corresponds to byte `k*64 + i`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuralBitmasks {
    /// JSON whitespace: ` `, `\t`, `\n`, `\r`.
    pub whitespace: Vec<u64>,
    /// Double-quote `"` .
    pub quote: Vec<u64>,
    /// Backslash `\` .
    pub backslash: Vec<u64>,
    /// Structural punctuation: `{` `}` `[` `]` `:` `,`.
    pub structural: Vec<u64>,
    /// Original input length in bytes (bits at `>= input_len` are never set).
    pub input_len: usize,
}

impl StructuralBitmasks {
    /// Whether bit `index` is set in `bits` (absolute byte index into the input).
    #[inline]
    pub fn bit_set(bits: &[u64], index: usize) -> bool {
        let word = index / 64;
        let bit = index % 64;
        bits.get(word).is_some_and(|w| (w >> bit) & 1 == 1)
    }

    /// Absolute byte positions where `bits` is set, up to `input_len`.
    pub fn positions(bits: &[u64], input_len: usize) -> Vec<usize> {
        let mut out = Vec::new();
        for (word_idx, &word) in bits.iter().enumerate() {
            let mut w = word;
            while w != 0 {
                let bit = w.trailing_zeros() as usize;
                let index = word_idx * 64 + bit;
                if index >= input_len {
                    break;
                }
                out.push(index);
                w &= w - 1;
            }
        }
        out
    }

    /// Blend these classification masks into true structural starts.
    pub fn blend(&self) -> blend::BlendedBitmasks {
        blend::blend(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitmasks_match_scalar_on_samples() {
        let samples: &[&[u8]] = &[
            b"",
            b" ",
            b"{}",
            b"[1,2]",
            br#"{"a":1}"#,
            br#"{"x":"a\"b"}"#,
            br#"{ "k" : [ 1 , 2 ] }"#,
            b"\t\n\r ",
            &[b'a'; 70],
            br#"{"items":[{"id":1},{"id":2}],"ok":true}"#,
        ];
        for s in samples {
            let got = SIMDParser::classify(s);
            let expect = scalar::compute_bitmasks(s);
            assert_eq!(got, expect, "mismatch on {:?}", String::from_utf8_lossy(s));
        }
    }

    #[test]
    fn positions_reflect_set_bits() {
        let input = br#"{ "a": 1 }"#;
        let bm = SIMDParser::classify(input);
        assert_eq!(
            StructuralBitmasks::positions(&bm.quote, bm.input_len),
            vec![2, 4]
        );
        assert_eq!(
            StructuralBitmasks::positions(&bm.structural, bm.input_len),
            vec![0, 5, 9]
        );
        assert_eq!(
            StructuralBitmasks::positions(&bm.whitespace, bm.input_len),
            vec![1, 6, 8]
        );
        assert!(StructuralBitmasks::positions(&bm.backslash, bm.input_len).is_empty());
    }
}
