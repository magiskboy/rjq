//! AVX2 byte-scan backend for `x86_64`.
//!
//! Runtime-detects AVX2; falls back to [`super::scalar`] when unavailable or
//! for the sub-32-byte tail. Mirrors [`super::neon`] (AArch64 counterpart).

use super::scalar;

/// True when AVX2 vectorized byte scan is available on this CPU/process.
#[inline]
pub fn is_available() -> bool {
    std::is_x86_feature_detected!("avx2")
}

/// See [`super::scalar::find_string_special`].
#[inline]
pub fn find_string_special(haystack: &[u8]) -> Option<usize> {
    if is_available() {
        // SAFETY: guarded by runtime AVX2 detection.
        unsafe { find_string_special_avx2(haystack) }
    } else {
        scalar::find_string_special(haystack)
    }
}

/// See [`super::scalar::find_non_whitespace`].
#[inline]
pub fn find_non_whitespace(haystack: &[u8]) -> Option<usize> {
    if is_available() {
        // SAFETY: guarded by runtime AVX2 detection.
        unsafe { find_non_whitespace_avx2(haystack) }
    } else {
        scalar::find_non_whitespace(haystack)
    }
}

#[target_feature(enable = "avx2")]
unsafe fn find_string_special_avx2(haystack: &[u8]) -> Option<usize> {
    use std::arch::x86_64::*;

    let quote = _mm256_set1_epi8(b'"' as i8);
    let backslash = _mm256_set1_epi8(b'\\' as i8);
    let ctrl_max = _mm256_set1_epi8(0x1F_u8 as i8);

    let mut offset = 0usize;
    while offset + 32 <= haystack.len() {
        // SAFETY: `offset + 32 <= len`, unaligned load is allowed.
        let block = unsafe { _mm256_loadu_si256(haystack.as_ptr().add(offset) as *const __m256i) };
        let is_quote = _mm256_cmpeq_epi8(block, quote);
        let is_backslash = _mm256_cmpeq_epi8(block, backslash);
        // unsigned `b <= 0x1F` ⇔ `min_epu8(b, 0x1F) == b`
        let is_control = _mm256_cmpeq_epi8(_mm256_min_epu8(block, ctrl_max), block);
        let matched = _mm256_or_si256(is_quote, _mm256_or_si256(is_backslash, is_control));
        let mask = _mm256_movemask_epi8(matched) as u32;
        if mask != 0 {
            return Some(offset + mask.trailing_zeros() as usize);
        }
        offset += 32;
    }

    scalar::find_string_special(&haystack[offset..]).map(|i| offset + i)
}

#[target_feature(enable = "avx2")]
unsafe fn find_non_whitespace_avx2(haystack: &[u8]) -> Option<usize> {
    use std::arch::x86_64::*;

    let space = _mm256_set1_epi8(b' ' as i8);
    let tab = _mm256_set1_epi8(b'\t' as i8);
    let lf = _mm256_set1_epi8(b'\n' as i8);
    let cr = _mm256_set1_epi8(b'\r' as i8);

    let mut offset = 0usize;
    while offset + 32 <= haystack.len() {
        let block = unsafe { _mm256_loadu_si256(haystack.as_ptr().add(offset) as *const __m256i) };
        let is_ws = _mm256_or_si256(
            _mm256_or_si256(_mm256_cmpeq_epi8(block, space), _mm256_cmpeq_epi8(block, tab)),
            _mm256_or_si256(_mm256_cmpeq_epi8(block, lf), _mm256_cmpeq_epi8(block, cr)),
        );
        // Bits set where byte is NOT whitespace.
        let mask = (!_mm256_movemask_epi8(is_ws)) as u32;
        if mask != 0 {
            return Some(offset + mask.trailing_zeros() as usize);
        }
        offset += 32;
    }

    scalar::find_non_whitespace(&haystack[offset..]).map(|i| offset + i)
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
    fn available_matches_cpu_feature() {
        assert_eq!(is_available(), std::is_x86_feature_detected!("avx2"));
    }
}
