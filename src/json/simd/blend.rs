//! Blend classification masks into true structural starts (escape / in-string).

use super::{Bitmask, StructuralBitmasks};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlendedBitmasks {
    pub escaped: Bitmask,
    pub real_quote: Bitmask,
    pub in_string: Bitmask,
    pub structural: Bitmask,
    pub input_len: usize,
    pub unclosed_string: bool,
}

impl BlendedBitmasks {
    pub fn structural_positions(&self) -> Vec<usize> {
        self.structural.positions()
    }
}

pub fn blend(bm: &StructuralBitmasks) -> BlendedBitmasks {
    let n = bm.whitespace.bucket_count();
    debug_assert_eq!(n, bm.quote.bucket_count());
    debug_assert_eq!(n, bm.backslash.bucket_count());
    debug_assert_eq!(n, bm.structural.bucket_count());

    let mut escaped = Bitmask::zeros(bm.input_len);
    let mut real_quote = Bitmask::zeros(bm.input_len);
    let mut in_string = Bitmask::zeros(bm.input_len);
    let mut structural = Bitmask::zeros(bm.input_len);

    let mut next_is_escaped = false;
    let mut prev_in_string = 0u64;
    let mut prev_scalar = 0u64;

    for i in 0..n {
        let valid = valid_mask(bm.input_len, i);
        let backslash = bm.backslash.bucket(i) & valid;
        let quote = bm.quote.bucket(i) & valid;
        let whitespace = bm.whitespace.bucket(i) & valid;
        let op = bm.structural.bucket(i) & valid;

        let (esc, carry_esc) = escaped_from_backslash(backslash, next_is_escaped);
        let esc = esc & valid;
        next_is_escaped = carry_esc && (i + 1) * 64 < bm.input_len;

        let rq = quote & !esc;
        let ins = prefix_xor(rq) ^ prev_in_string;
        prev_in_string = ((ins as i64) >> 63) as u64;
        // Drop opening quote; keep closing quote + interior (simdjson string_tail).
        let string_tail = ins ^ rq;

        let scalar = !(op | whitespace) & valid;
        let nonquote_scalar = scalar & !rq;
        let follows_nonquote_scalar = (nonquote_scalar << 1) | prev_scalar;
        prev_scalar = nonquote_scalar >> 63;

        let potential_scalar_start = scalar & !follows_nonquote_scalar;
        let potential = op | potential_scalar_start;

        escaped.set_bucket(i, esc);
        real_quote.set_bucket(i, rq);
        in_string.set_bucket(i, ins & valid);
        structural.set_bucket(i, (potential & !string_tail) & valid);
    }

    BlendedBitmasks {
        escaped,
        real_quote,
        in_string,
        structural,
        input_len: bm.input_len,
        unclosed_string: prev_in_string != 0,
    }
}

/// Branchless simdjson odd/even backslash runs; returns (escaped bits, carry-out).
#[inline]
pub(crate) fn escaped_from_backslash(backslash: u64, prev_escaped: bool) -> (u64, bool) {
    const ODD_BITS: u64 = 0xAAAA_AAAA_AAAA_AAAA;
    let next_is_escaped = u64::from(prev_escaped);
    let potential = backslash & !next_is_escaped;
    let maybe_escaped = potential << 1;
    let maybe_escaped_and_odd = maybe_escaped | ODD_BITS;
    let even_series_codes_and_odd = maybe_escaped_and_odd.wrapping_sub(potential);
    let escape_and_terminal = even_series_codes_and_odd ^ ODD_BITS;
    let escaped = escape_and_terminal ^ (backslash | next_is_escaped);
    let escape = escape_and_terminal & backslash;
    let carry = (escape >> 63) != 0;
    (escaped, carry)
}

/// Prefix-XOR: bit `i` = XOR of bits `0..=i` (string interiors from unescaped quotes).
#[inline]
pub(crate) fn prefix_xor(mut bitmask: u64) -> u64 {
    bitmask ^= bitmask << 1;
    bitmask ^= bitmask << 2;
    bitmask ^= bitmask << 4;
    bitmask ^= bitmask << 8;
    bitmask ^= bitmask << 16;
    bitmask ^= bitmask << 32;
    bitmask
}

#[inline]
fn valid_mask(input_len: usize, block_idx: usize) -> u64 {
    let start = block_idx * 64;
    if start >= input_len {
        return 0;
    }
    let rem = input_len - start;
    if rem >= 64 {
        !0
    } else {
        (1u64 << rem) - 1
    }
}

#[cfg(test)]
pub fn reference_structural_positions(input: &[u8]) -> Vec<usize> {
    let mut out = Vec::new();
    let mut in_string = false;
    let mut escaped = false;
    let mut prev_nonquote_scalar = false;

    for (i, &b) in input.iter().enumerate() {
        if in_string {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                in_string = false;
                prev_nonquote_scalar = false;
            }
            continue;
        }

        let is_ws = matches!(b, b' ' | b'\t' | b'\n' | b'\r');
        let is_op = matches!(b, b'{' | b'}' | b'[' | b']' | b':' | b',');
        let is_quote = b == b'"';
        let is_scalar = !is_ws && !is_op;

        if is_op {
            out.push(i);
            prev_nonquote_scalar = false;
        } else if is_quote {
            if !prev_nonquote_scalar {
                out.push(i);
            }
            in_string = true;
            escaped = false;
            prev_nonquote_scalar = false;
        } else if is_scalar {
            if !prev_nonquote_scalar {
                out.push(i);
            }
            prev_nonquote_scalar = true;
        } else {
            prev_nonquote_scalar = false;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::simd::scalar::compute_bitmasks;

    #[test]
    fn prefix_xor_marks_string_interior() {
        let quotes = (1u64 << 1) | (1u64 << 5);
        let ins = prefix_xor(quotes);
        assert_eq!(ins & (1 << 1), 1 << 1);
        assert_eq!(ins & (1 << 2), 1 << 2);
        assert_eq!(ins & (1 << 3), 1 << 3);
        assert_eq!(ins & (1 << 4), 1 << 4);
        assert_eq!(ins & (1 << 5), 0);
        assert_eq!(ins & (1 << 0), 0);
    }

    #[test]
    fn escaped_handles_even_and_odd_backslash_runs() {
        let bs = 1u64 << 0;
        let (esc, _) = escaped_from_backslash(bs, false);
        assert_ne!(esc & (1 << 1), 0);

        let bs = (1u64 << 0) | (1u64 << 1);
        let (esc, _) = escaped_from_backslash(bs, false);
        assert_eq!(esc & (1 << 2), 0);
        assert_ne!(esc & (1 << 1), 0);
    }

    #[test]
    fn blend_matches_reference_on_samples() {
        let samples: &[&[u8]] = &[
            b"",
            b"{}",
            b"[1,2]",
            br#"{"a":1}"#,
            br#"{"x":"a\"b"}"#,
            br#"{ "k" : [ 1 , 2 ] }"#,
            br#"{"items":[{"id":1},{"id":2}],"ok":true}"#,
            br#""alone""#,
            br#"true"#,
            br#"{"a":"\\\\"}"#,
            br#"{"a":"\\\""}"#,
            &[b' '; 70],
            br#"{"n":123456789012345678901234567890}"#,
        ];
        for s in samples {
            let bm = compute_bitmasks(s);
            let blended = blend(&bm);
            let got = blended.structural_positions();
            let expect = reference_structural_positions(s);
            assert_eq!(
                got,
                expect,
                "mismatch on {:?}\n  got={got:?}\n  exp={expect:?}",
                String::from_utf8_lossy(s)
            );
        }
    }

    #[test]
    fn escaped_quote_does_not_end_string() {
        let input = br#"{"x":"a\"b"}"#;
        let blended = blend(&compute_bitmasks(input));
        let positions = blended.structural_positions();
        assert!(positions.contains(&0));
        assert!(!blended.unclosed_string);
        let esc_quote_at = input.iter().position(|&b| b == b'\\').unwrap() + 1;
        assert!(
            !blended.real_quote.get(esc_quote_at),
            "escaped quote at {esc_quote_at} should not be real"
        );
    }

    #[test]
    fn rejects_unclosed_string_flag() {
        let blended = blend(&compute_bitmasks(br#""abc"#));
        assert!(blended.unclosed_string);
    }
}
