//! Blend stage-1 bitmasks into true structural boundaries.
//!
//! Given the four classification masks from [`super::StructuralBitmasks`], this
//! module applies the simdjson-style string/escape logic:
//!
//! 1. **escaped** — bytes that follow an odd-length backslash run
//! 2. **real_quote** — `"` that are not escaped
//! 3. **in_string** — region between real quotes (includes start `"`, excludes end `"`)
//! 4. **structural** — final structural *starts*: operators `{ } [ ] : ,`,
//!    opening quotes, and scalar starts (`123`, `true`, …), with everything in
//!    the string tail masked out
//!
//! This step is pure bit-parallel arithmetic on `u64` words; no SIMD is needed.

use super::StructuralBitmasks;

/// Result of blending the four classification bitmasks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlendedBitmasks {
    /// Byte at bit `i` is escaped by a preceding backslash.
    pub escaped: Vec<u64>,
    /// Unescaped `"` characters (string delimiters).
    pub real_quote: Vec<u64>,
    /// Inside a JSON string: includes the opening `"`, excludes the closing `"`.
    pub in_string: Vec<u64>,
    /// True structural starts after removing string content / escaped quotes.
    pub structural: Vec<u64>,
    pub input_len: usize,
    /// `true` if a string was still open at end-of-input.
    pub unclosed_string: bool,
}

impl BlendedBitmasks {
    /// Absolute byte positions of the final structural starts.
    pub fn structural_positions(&self) -> Vec<usize> {
        StructuralBitmasks::positions(&self.structural, self.input_len)
    }
}

/// Blend classification bitmasks into true structural boundaries.
pub fn blend(bm: &StructuralBitmasks) -> BlendedBitmasks {
    let n = bm.whitespace.len();
    debug_assert_eq!(n, bm.quote.len());
    debug_assert_eq!(n, bm.backslash.len());
    debug_assert_eq!(n, bm.structural.len());

    let mut escaped = vec![0u64; n];
    let mut real_quote = vec![0u64; n];
    let mut in_string = vec![0u64; n];
    let mut structural = vec![0u64; n];

    // Carry across 64-byte blocks.
    let mut next_is_escaped = false;
    // All-0 or all-1, matching simdjson's `prev_in_string`.
    let mut prev_in_string = 0u64;
    // Bit 0: whether the previous block's last byte was a non-quote scalar.
    let mut prev_scalar = 0u64;

    for i in 0..n {
        let valid = valid_mask(bm.input_len, i);
        let backslash = bm.backslash[i] & valid;
        let quote = bm.quote[i] & valid;
        let whitespace = bm.whitespace[i] & valid;
        let op = bm.structural[i] & valid;

        // 1. Characters escaped by backslash (handles `\\`, `\"`, `\\\"`, …).
        let (esc, carry_esc) = escaped_from_backslash(backslash, next_is_escaped);
        let esc = esc & valid;
        // Carry into the next block only when more input remains.
        next_is_escaped = carry_esc && (i + 1) * 64 < bm.input_len;

        // 2. Quotes that actually delimit strings.
        let rq = quote & !esc;

        // 3. in_string via prefix-XOR (simdjson).
        let ins = prefix_xor(rq) ^ prev_in_string;
        prev_in_string = ((ins as i64) >> 63) as u64;

        // string_tail = in_string ⊕ quote: drops the opening quote, keeps the
        // closing quote and all interior bytes. Structural starts use ~tail.
        let string_tail = ins ^ rq;

        // 4. Scalar = not operator and not whitespace (letters, digits, quotes, …).
        let scalar = !(op | whitespace) & valid;
        let nonquote_scalar = scalar & !rq;

        // follows_scalar[i] ⇔ byte i−1 was a non-quote scalar (with block carry).
        let follows_nonquote_scalar = (nonquote_scalar << 1) | prev_scalar;
        prev_scalar = nonquote_scalar >> 63;

        // First byte of a scalar run (number / literal / opening quote).
        let potential_scalar_start = scalar & !follows_nonquote_scalar;
        let potential = op | potential_scalar_start;

        escaped[i] = esc;
        real_quote[i] = rq;
        in_string[i] = ins & valid;
        structural[i] = (potential & !string_tail) & valid;
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

/// Bits of `backslash` that escape the *next* byte; `prev_escaped` is carry-in
/// (whether bit 0 of this block is already escaped from a trailing `\` ).
///
/// Branchless simdjson algorithm (odd/even backslash runs via subtract + XOR).
#[inline]
pub(crate) fn escaped_from_backslash(backslash: u64, prev_escaped: bool) -> (u64, bool) {
    const ODD_BITS: u64 = 0xAAAA_AAAA_AAAA_AAAA;
    let next_is_escaped = u64::from(prev_escaped);
    // Mask out a leading backslash that is itself escaped by the prior block.
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

/// Prefix-XOR: bit `i` becomes the XOR of bits `0..=i` of `bitmask`.
///
/// For unescaped quotes this lights up the interior of each string (including
/// the opening quote, excluding the closing quote).
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

/// Byte-at-a-time reference for the final structural positions (test oracle).
#[cfg(test)]
pub fn reference_structural_positions(input: &[u8]) -> Vec<usize> {
    let mut out = Vec::new();
    let mut in_string = false;
    let mut escaped = false;
    // Whether the previous emitted byte (outside string logic) was a non-quote scalar.
    let mut prev_nonquote_scalar = false;

    for (i, &b) in input.iter().enumerate() {
        if in_string {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                in_string = false;
                // Closing quote is NOT a structural start (simdjson string_tail).
                prev_nonquote_scalar = false;
            }
            // Interior bytes never start structurals.
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
            // Opening quote = scalar start and structural delimiter.
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
            // whitespace
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
        // quotes at bits 1 and 5 → in_string bits 1..=4
        let quotes = (1u64 << 1) | (1u64 << 5);
        let ins = prefix_xor(quotes);
        assert_eq!(ins & (1 << 1), 1 << 1); // start quote on
        assert_eq!(ins & (1 << 2), 1 << 2);
        assert_eq!(ins & (1 << 3), 1 << 3);
        assert_eq!(ins & (1 << 4), 1 << 4);
        assert_eq!(ins & (1 << 5), 0); // end quote off
        assert_eq!(ins & (1 << 0), 0);
    }

    #[test]
    fn escaped_handles_even_and_odd_backslash_runs() {
        // \ "  → quote escaped
        let bs = 1u64 << 0;
        let (esc, _) = escaped_from_backslash(bs, false);
        assert_ne!(esc & (1 << 1), 0);

        // \ \ " → quote NOT escaped
        let bs = (1u64 << 0) | (1u64 << 1);
        let (esc, _) = escaped_from_backslash(bs, false);
        assert_eq!(esc & (1 << 2), 0);
        assert_ne!(esc & (1 << 1), 0); // second backslash is escaped
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
        // { " x " : " a \" b " }
        // structurals: {, open", :, open", }  — not the escaped quote, not interior
        assert!(positions.contains(&0)); // {
        assert!(!blended.unclosed_string);
        // Escaped quote at the \" inside the value must NOT be a real_quote.
        let esc_quote_at = input.iter().position(|&b| b == b'\\').unwrap() + 1;
        assert!(
            !StructuralBitmasks::bit_set(&blended.real_quote, esc_quote_at),
            "escaped quote at {esc_quote_at} should not be real"
        );
    }

    #[test]
    fn rejects_unclosed_string_flag() {
        let blended = blend(&compute_bitmasks(br#""abc"#));
        assert!(blended.unclosed_string);
    }
}
