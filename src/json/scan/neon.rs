//! NEON byte-scan backend for AArch64 (Apple Silicon et al.).
//!
//! NEON is a baseline feature on AArch64, so [`is_available`] is always true and
//! no runtime detection is required. Falls back to [`super::scalar`] for the
//! sub-16-byte tail. Mirrors [`super::avx2`] (x86_64 counterpart).

use super::scalar;

/// True when NEON vectorized byte scan is available (AArch64 baseline).
#[inline]
pub fn is_available() -> bool {
    true
}

/// See [`super::scalar::find_string_special`].
#[inline]
pub fn find_string_special(haystack: &[u8]) -> Option<usize> {
    // SAFETY: NEON is a baseline AArch64 feature.
    unsafe { find_string_special_neon(haystack) }
}

/// See [`super::scalar::find_non_whitespace`].
#[inline]
pub fn find_non_whitespace(haystack: &[u8]) -> Option<usize> {
    // SAFETY: NEON is a baseline AArch64 feature.
    unsafe { find_non_whitespace_neon(haystack) }
}

#[target_feature(enable = "neon")]
unsafe fn find_string_special_neon(haystack: &[u8]) -> Option<usize> {
    use std::arch::aarch64::*;

    let quote = vdupq_n_u8(b'"');
    let backslash = vdupq_n_u8(b'\\');
    let ctrl_max = vdupq_n_u8(0x1F);

    let mut offset = 0usize;
    while offset + 16 <= haystack.len() {
        // SAFETY: `offset + 16 <= len`.
        let block = unsafe { vld1q_u8(haystack.as_ptr().add(offset)) };
        let is_quote = vceqq_u8(block, quote);
        let is_backslash = vceqq_u8(block, backslash);
        // unsigned `b <= 0x1F` ⇔ `min(b, 0x1F) == b`
        let is_control = vceqq_u8(vminq_u8(block, ctrl_max), block);
        let matched = vorrq_u8(is_quote, vorrq_u8(is_backslash, is_control));
        let mask = unsafe { neon_mask16(matched) };
        if mask != 0 {
            return Some(offset + mask.trailing_zeros() as usize);
        }
        offset += 16;
    }

    scalar::find_string_special(&haystack[offset..]).map(|i| offset + i)
}

#[target_feature(enable = "neon")]
unsafe fn find_non_whitespace_neon(haystack: &[u8]) -> Option<usize> {
    use std::arch::aarch64::*;

    let space = vdupq_n_u8(b' ');
    let tab = vdupq_n_u8(b'\t');
    let lf = vdupq_n_u8(b'\n');
    let cr = vdupq_n_u8(b'\r');

    let mut offset = 0usize;
    while offset + 16 <= haystack.len() {
        let block = unsafe { vld1q_u8(haystack.as_ptr().add(offset)) };
        let is_ws = vorrq_u8(
            vorrq_u8(vceqq_u8(block, space), vceqq_u8(block, tab)),
            vorrq_u8(vceqq_u8(block, lf), vceqq_u8(block, cr)),
        );
        let non_ws = vmvnq_u8(is_ws);
        let mask = unsafe { neon_mask16(non_ws) };
        if mask != 0 {
            return Some(offset + mask.trailing_zeros() as usize);
        }
        offset += 16;
    }

    scalar::find_non_whitespace(&haystack[offset..]).map(|i| offset + i)
}

/// Compact 16 NEON comparison lanes (0x00 / 0xFF) into a 16-bit mask.
#[target_feature(enable = "neon")]
unsafe fn neon_mask16(lanes: std::arch::aarch64::uint8x16_t) -> u32 {
    use std::arch::aarch64::*;
    let mut bytes = [0u8; 16];
    unsafe { vst1q_u8(bytes.as_mut_ptr(), lanes) };
    let mut mask = 0u32;
    for (i, b) in bytes.iter().enumerate() {
        if *b != 0 {
            mask |= 1 << i;
        }
    }
    mask
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_special_dispatch_matches_scalar() {
        let mut long = vec![b'a'; 100];
        long.push(b'"');
        let samples: [&[u8]; 7] = [
            b"",
            b"hello",
            b"hello\"",
            b"hel\\lo",
            b"he\x01lo",
            &[b'x'; 40],
            long.as_slice(),
        ];
        for s in samples {
            assert_eq!(
                find_string_special(s),
                scalar::find_string_special(s),
                "mismatch on {s:?}"
            );
        }
    }

    #[test]
    fn non_whitespace_dispatch_matches_scalar() {
        let mut long = vec![b' '; 100];
        long.push(b'[');
        let samples: [&[u8]; 8] = [
            b"",
            b"   ",
            b"\t\n\r ",
            b"  x",
            b"x",
            b"{\n",
            &[b' '; 40],
            long.as_slice(),
        ];
        for s in samples {
            assert_eq!(
                find_non_whitespace(s),
                scalar::find_non_whitespace(s),
                "mismatch on {s:?}"
            );
        }
    }

    #[test]
    fn available_on_aarch64() {
        assert!(is_available());
    }
}
