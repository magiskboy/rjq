//! NEON structural-classification + fused stage-1 index builder (AArch64).

use super::index::StructuralIndex;
use super::stage1::{append_set_bits, blend_block};
use super::StructuralBitmasks;

/// NEON is a baseline AArch64 feature.
#[inline]
pub fn is_available() -> bool {
    true
}

/// See [`super::scalar::compute_bitmasks`].
#[inline]
pub fn compute_bitmasks(input: &[u8]) -> StructuralBitmasks {
    // SAFETY: NEON is a baseline AArch64 feature.
    unsafe { compute_bitmasks_neon(input) }
}

/// Fused classify → blend → flatten (no intermediate bitmask vectors).
#[inline]
pub fn build_structural_index(input: &[u8]) -> StructuralIndex {
    // SAFETY: NEON is a baseline AArch64 feature.
    unsafe { build_structural_index_neon(input) }
}

#[target_feature(enable = "neon")]
unsafe fn build_structural_index_neon(input: &[u8]) -> StructuralIndex {
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
        // SAFETY: `offset + 64 <= len`.
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

#[target_feature(enable = "neon")]
unsafe fn compute_bitmasks_neon(input: &[u8]) -> StructuralBitmasks {
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
#[target_feature(enable = "neon")]
pub(crate) unsafe fn classify_block64(ptr: *const u8) -> (u64, u64, u64, u64) {
    use std::arch::aarch64::*;

    let sp = vdupq_n_u8(b' ');
    let tab = vdupq_n_u8(b'\t');
    let lf = vdupq_n_u8(b'\n');
    let cr = vdupq_n_u8(b'\r');
    let quote_v = vdupq_n_u8(b'"');
    let bs_v = vdupq_n_u8(b'\\');
    let lbrace = vdupq_n_u8(b'{');
    let rbrace = vdupq_n_u8(b'}');
    let lbrack = vdupq_n_u8(b'[');
    let rbrack = vdupq_n_u8(b']');
    let colon = vdupq_n_u8(b':');
    let comma = vdupq_n_u8(b',');

    let mut whitespace = 0u64;
    let mut quote = 0u64;
    let mut backslash = 0u64;
    let mut structural = 0u64;

    for chunk in 0..4 {
        let block = unsafe { vld1q_u8(ptr.add(chunk * 16)) };

        let is_ws = vorrq_u8(
            vorrq_u8(vceqq_u8(block, sp), vceqq_u8(block, tab)),
            vorrq_u8(vceqq_u8(block, lf), vceqq_u8(block, cr)),
        );
        let is_quote = vceqq_u8(block, quote_v);
        let is_bs = vceqq_u8(block, bs_v);
        let is_struct = vorrq_u8(
            vorrq_u8(
                vorrq_u8(vceqq_u8(block, lbrace), vceqq_u8(block, rbrace)),
                vorrq_u8(vceqq_u8(block, lbrack), vceqq_u8(block, rbrack)),
            ),
            vorrq_u8(vceqq_u8(block, colon), vceqq_u8(block, comma)),
        );

        let shift = (chunk * 16) as u32;
        whitespace |= (unsafe { neon_mask16(is_ws) } as u64) << shift;
        quote |= (unsafe { neon_mask16(is_quote) } as u64) << shift;
        backslash |= (unsafe { neon_mask16(is_bs) } as u64) << shift;
        structural |= (unsafe { neon_mask16(is_struct) } as u64) << shift;
    }

    (whitespace, quote, backslash, structural)
}

/// Compact 16 NEON comparison lanes (0x00 / 0xFF) into a 16-bit mask.
#[target_feature(enable = "neon")]
unsafe fn neon_mask16(lanes: std::arch::aarch64::uint8x16_t) -> u32 {
    use std::arch::aarch64::*;
    // Compress via MSB of each lane (cmp results are 0x00 / 0xFF).
    let mut bytes = [0u8; 16];
    unsafe { vst1q_u8(bytes.as_mut_ptr(), lanes) };
    let mut mask = 0u32;
    for (i, &b) in bytes.iter().enumerate() {
        mask |= (u32::from(b) >> 7) << i;
    }
    mask
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::simd::scalar;
    use crate::json::simd::stage1::build_index_portable;

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

    #[test]
    fn fused_index_matches_portable() {
        let samples: &[&[u8]] = &[
            b"",
            br#"{"a":"b\"c","n":[1,2,true]}"#,
            &[b' '; 70],
        ];
        for s in samples {
            assert_eq!(
                build_structural_index(s).indexes,
                build_index_portable(s).indexes,
                "mismatch on {:?}",
                String::from_utf8_lossy(s)
            );
        }
    }
}
