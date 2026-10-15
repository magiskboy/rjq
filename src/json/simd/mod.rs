//! SIMD JSON parser: classify → blend → index → parse.
//!
//! Hot path fuses classify + blend + index into [`StructuralIndex::build`].
//! [`Bitmask`] stores bits in `u64` buckets; bit `i` marks byte `i`.

pub mod bitmask;
pub mod blend;
pub mod index;
pub mod lexer;
pub mod parser;
pub(crate) mod scalar;

#[cfg(target_arch = "x86_64")]
mod avx2;
#[cfg(target_arch = "aarch64")]
mod neon;

pub use bitmask::Bitmask;
pub use blend::BlendedBitmasks;
pub use index::StructuralIndex;
pub use lexer::SIMDLexer;
pub use parser::SIMDParser;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuralBitmasks {
    pub whitespace: Bitmask,
    pub quote: Bitmask,
    pub backslash: Bitmask,
    pub structural: Bitmask,
    pub input_len: usize,
}

impl StructuralBitmasks {
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
        assert_eq!(bm.quote.positions(), vec![2, 4]);
        assert_eq!(bm.structural.positions(), vec![0, 5, 9]);
        assert_eq!(bm.whitespace.positions(), vec![1, 6, 8]);
        assert!(bm.backslash.positions().is_empty());
    }
}
