//! Fused stage-1: classify → blend → flatten in one pass over 64-byte blocks.
//!
//! Only the structural-start index is retained (simdjson-style). Intermediate
//! bitmasks are not stored, which cuts memory traffic vs. the pedagogical
//! classify/blend/index pipeline.

use super::blend::{escaped_from_backslash, prefix_xor};
use super::index::StructuralIndex;

#[cfg(target_arch = "x86_64")]
use super::avx2;
#[cfg(target_arch = "aarch64")]
use super::neon;

/// Build the structural-start index for `input`.
#[inline]
pub fn build_index(input: &[u8]) -> StructuralIndex {
    #[cfg(target_arch = "aarch64")]
    {
        neon::build_structural_index(input)
    }
    #[cfg(target_arch = "x86_64")]
    {
        avx2::build_structural_index(input)
    }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        build_index_portable(input)
    }
}

/// Portable fused index builder (also used as a test oracle / AVX2 fallback).
pub fn build_index_portable(input: &[u8]) -> StructuralIndex {
    let mut indexes = Vec::with_capacity(input.len() / 4 + 8);
    if input.is_empty() {
        return StructuralIndex {
            indexes,
            input_len: 0,
        };
    }

    let mut next_is_escaped = false;
    let mut prev_in_string = 0u64;
    let mut prev_scalar = 0u64;
    let mut offset = 0usize;

    while offset < input.len() {
        let mut buf = [0u8; 64];
        let rem = (input.len() - offset).min(64);
        buf[..rem].copy_from_slice(&input[offset..offset + rem]);
        let (ws, quote, bs, op) = classify_block_scalar(&buf);
        let valid = if rem == 64 {
            !0u64
        } else {
            (1u64 << rem) - 1
        };

        let structural = blend_block(
            ws & valid,
            quote & valid,
            bs & valid,
            op & valid,
            valid,
            &mut next_is_escaped,
            &mut prev_in_string,
            &mut prev_scalar,
        );
        append_set_bits(&mut indexes, structural, offset as u32);
        offset += 64;
    }

    StructuralIndex {
        indexes,
        input_len: input.len(),
    }
}

#[inline]
pub(crate) fn blend_block(
    whitespace: u64,
    quote: u64,
    backslash: u64,
    op: u64,
    valid: u64,
    next_is_escaped: &mut bool,
    prev_in_string: &mut u64,
    prev_scalar: &mut u64,
) -> u64 {
    let (esc, carry) = escaped_from_backslash(backslash, *next_is_escaped);
    let esc = esc & valid;
    *next_is_escaped = carry;

    let real_quote = quote & !esc;
    let in_string = prefix_xor(real_quote) ^ *prev_in_string;
    *prev_in_string = ((in_string as i64) >> 63) as u64;
    let string_tail = in_string ^ real_quote;

    let scalar = !(op | whitespace) & valid;
    let nonquote_scalar = scalar & !real_quote;
    let follows = (nonquote_scalar << 1) | *prev_scalar;
    *prev_scalar = nonquote_scalar >> 63;

    let potential_scalar_start = scalar & !follows;
    let potential = op | potential_scalar_start;
    (potential & !string_tail) & valid
}

#[inline]
pub(crate) fn append_set_bits(out: &mut Vec<u32>, mut bits: u64, base: u32) {
    while bits != 0 {
        let b = bits.trailing_zeros();
        out.push(base + b);
        bits &= bits - 1;
    }
}

fn classify_block_scalar(block: &[u8; 64]) -> (u64, u64, u64, u64) {
    let mut ws = 0u64;
    let mut quote = 0u64;
    let mut bs = 0u64;
    let mut op = 0u64;
    for (i, &b) in block.iter().enumerate() {
        let bit = 1u64 << i;
        match b {
            b' ' | b'\t' | b'\n' | b'\r' => ws |= bit,
            b'"' => quote |= bit,
            b'\\' => bs |= bit,
            b'{' | b'}' | b'[' | b']' | b':' | b',' => op |= bit,
            _ => {}
        }
    }
    (ws, quote, bs, op)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::simd::{blend, index::StructuralIndex, scalar};

    #[test]
    fn fused_matches_blend_flatten() {
        let samples: &[&[u8]] = &[
            b"",
            b"{}",
            b"[1,2,3]",
            br#"{"a":"b\"c","n":12}"#,
            br#"{ "k" : [ true , null ] }"#,
            &[b'x'; 100],
        ];
        for s in samples {
            let expect =
                StructuralIndex::from_blended(&blend::blend(&scalar::compute_bitmasks(s)));
            let got = build_index_portable(s);
            assert_eq!(
                got.indexes,
                expect.indexes,
                "mismatch on {:?}",
                String::from_utf8_lossy(s)
            );
        }
    }

    #[test]
    fn dispatch_matches_portable() {
        let samples: &[&[u8]] = &[
            b"",
            br#"{"a":"b\"c","n":[1,2,true]}"#,
            &[b' '; 70],
            br#"{ "k" : true }"#,
        ];
        for s in samples {
            assert_eq!(
                build_index(s).indexes,
                build_index_portable(s).indexes,
                "mismatch on {:?}",
                String::from_utf8_lossy(s)
            );
        }
    }
}
