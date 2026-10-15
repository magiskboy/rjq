//! AVX2 structural-classification backend for x86_64.
//!
//! Processes the input in 64-byte blocks (2×32-byte AVX2 loads). Falls back to
//! [`super::scalar`] when AVX2 is unavailable, and uses a zero-padded temporary
//! block for the sub-64-byte tail.

use super::index::StructuralIndex;
use super::scalar;
use super::stage1::{append_set_bits, blend_block, build_index_portable};
use super::StructuralBitmasks;

/// True when AVX2 vectorized classification is available on this CPU.
#[inline]
pub fn is_available() -> bool {
    std::is_x86_feature_detected!("avx2")
}

/// See [`super::scalar::compute_bitmasks`].
#[inline]
pub fn compute_bitmasks(input: &[u8]) -> StructuralBitmasks {
    if is_available() {
        // SAFETY: guarded by runtime AVX2 detection.
        unsafe { compute_bitmasks_avx2(input) }
    } else {
        scalar::compute_bitmasks(input)
    }
}

/// Fused classify → blend → flatten.
#[inline]
pub fn build_structural_index(input: &[u8]) -> StructuralIndex {
    if is_available() {
        // SAFETY: guarded by runtime AVX2 detection.
        unsafe { build_structural_index_avx2(input) }
    } else {
        build_index_portable(input)
    }
}

#[target_feature(enable = "avx2")]
unsafe fn build_structural_index_avx2(input: &[u8]) -> StructuralIndex {
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

    while offset + 64 <= input.len() {
        let (ws, quote, bs, op) = unsafe { classify_block64(input.as_ptr().add(offset)) };
        let structural = blend_block(
            ws,
            quote,
            bs,
            op,
            !0u64,
            &mut next_is_escaped,
            &mut prev_in_string,
            &mut prev_scalar,
        );
        append_set_bits(&mut indexes, structural, offset as u32);
        offset += 64;
    }

    if offset < input.len() {
        let mut buf = [0u8; 64];
        let rem = input.len() - offset;
        buf[..rem].copy_from_slice(&input[offset..]);
        let (ws, quote, bs, op) = unsafe { classify_block64(buf.as_ptr()) };
        let valid = (1u64 << rem) - 1;
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
    }

    StructuralIndex {
        indexes,
        input_len: input.len(),
    }
}

#[target_feature(enable = "avx2")]
unsafe fn compute_bitmasks_avx2(input: &[u8]) -> StructuralBitmasks {
    let n_blocks = if input.is_empty() {
        1
    } else {
        input.len().div_ceil(64)
    };

    let mut whitespace = vec![0u64; n_blocks];
    let mut quote = vec![0u64; n_blocks];
    let mut backslash = vec![0u64; n_blocks];
    let mut structural = vec![0u64; n_blocks];

    let mut offset = 0usize;
    let mut block_idx = 0usize;

    while offset + 64 <= input.len() {
        // SAFETY: `offset + 64 <= len`.
        let (w, q, b, s) = unsafe { classify_block64(input.as_ptr().add(offset)) };
        whitespace[block_idx] = w;
        quote[block_idx] = q;
        backslash[block_idx] = b;
        structural[block_idx] = s;
        offset += 64;
        block_idx += 1;
    }

    if offset < input.len() || input.is_empty() {
        let mut buf = [0u8; 64];
        let rem = input.len() - offset;
        if rem > 0 {
            buf[..rem].copy_from_slice(&input[offset..]);
        }
        let (w, q, b, s) = unsafe { classify_block64(buf.as_ptr()) };
        whitespace[block_idx] = w;
        quote[block_idx] = q;
        backslash[block_idx] = b;
        structural[block_idx] = s;
    }

    StructuralBitmasks {
        whitespace,
        quote,
        backslash,
        structural,
        input_len: input.len(),
    }
}

/// Classify 64 bytes at `ptr` into `(whitespace, quote, backslash, structural)`.
#[target_feature(enable = "avx2")]
unsafe fn classify_block64(ptr: *const u8) -> (u64, u64, u64, u64) {
    use std::arch::x86_64::*;

    let sp = _mm256_set1_epi8(b' ' as i8);
    let tab = _mm256_set1_epi8(b'\t' as i8);
    let lf = _mm256_set1_epi8(b'\n' as i8);
    let cr = _mm256_set1_epi8(b'\r' as i8);
    let quote_v = _mm256_set1_epi8(b'"' as i8);
    let bs_v = _mm256_set1_epi8(b'\\' as i8);
    let lbrace = _mm256_set1_epi8(b'{' as i8);
    let rbrace = _mm256_set1_epi8(b'}' as i8);
    let lbrack = _mm256_set1_epi8(b'[' as i8);
    let rbrack = _mm256_set1_epi8(b']' as i8);
    let colon = _mm256_set1_epi8(b':' as i8);
    let comma = _mm256_set1_epi8(b',' as i8);

    let mut whitespace = 0u64;
    let mut quote = 0u64;
    let mut backslash = 0u64;
    let mut structural = 0u64;

    for chunk in 0..2 {
        // SAFETY: caller guarantees 64 readable bytes; unaligned load is OK.
        let block = unsafe { _mm256_loadu_si256(ptr.add(chunk * 32) as *const __m256i) };

        let is_ws = _mm256_or_si256(
            _mm256_or_si256(_mm256_cmpeq_epi8(block, sp), _mm256_cmpeq_epi8(block, tab)),
            _mm256_or_si256(_mm256_cmpeq_epi8(block, lf), _mm256_cmpeq_epi8(block, cr)),
        );
        let is_quote = _mm256_cmpeq_epi8(block, quote_v);
        let is_bs = _mm256_cmpeq_epi8(block, bs_v);
        let is_struct = _mm256_or_si256(
            _mm256_or_si256(
                _mm256_or_si256(
                    _mm256_cmpeq_epi8(block, lbrace),
                    _mm256_cmpeq_epi8(block, rbrace),
                ),
                _mm256_or_si256(
                    _mm256_cmpeq_epi8(block, lbrack),
                    _mm256_cmpeq_epi8(block, rbrack),
                ),
            ),
            _mm256_or_si256(
                _mm256_cmpeq_epi8(block, colon),
                _mm256_cmpeq_epi8(block, comma),
            ),
        );

        let shift = (chunk * 32) as u32;
        whitespace |= (_mm256_movemask_epi8(is_ws) as u32 as u64) << shift;
        quote |= (_mm256_movemask_epi8(is_quote) as u32 as u64) << shift;
        backslash |= (_mm256_movemask_epi8(is_bs) as u32 as u64) << shift;
        structural |= (_mm256_movemask_epi8(is_struct) as u32 as u64) << shift;
    }

    (whitespace, quote, backslash, structural)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::simd::scalar;

    #[test]
    fn dispatch_matches_scalar() {
        let samples: &[&[u8]] = &[
            b"",
            b"{}",
            br#"{"a":[1,2],"b":"x\"y"}"#,
            &[b' '; 100],
            br#"{ "k" : true }"#,
        ];
        for s in samples {
            assert_eq!(
                compute_bitmasks(s),
                scalar::compute_bitmasks(s),
                "mismatch on {:?}",
                String::from_utf8_lossy(s)
            );
        }
    }
}
