//! Architecture-dispatched byte scanning for the JSON lexer hot paths.
//!
//! Responsibilities are split cleanly so the lexer never needs `cfg`/`unsafe`:
//! - [`scalar`] — portable reference implementations (the semantic spec).
//! - `avx2` / `neon` — SIMD backends per architecture, each verified against
//!   [`scalar`] via `*_dispatch_matches_scalar` tests.
//! - this module — a thin facade exposing [`is_available`],
//!   [`find_string_special`] and [`find_non_whitespace`].
//!
//! Convention for new scans (e.g. a future number-boundary scan): add it to
//! [`scalar`] first, then each SIMD backend, then re-export here, and add a
//! dispatch-vs-scalar test in every backend.

pub(crate) mod scalar;

#[cfg(target_arch = "x86_64")]
mod avx2;
#[cfg(target_arch = "aarch64")]
mod neon;

#[cfg(target_arch = "x86_64")]
use avx2 as backend;
#[cfg(target_arch = "aarch64")]
use neon as backend;

/// True when a vectorized byte scan is available for this target/CPU. When
/// false, callers should take their scalar lexing path (the `find_*` helpers
/// still work, delegating to [`scalar`], but offer no speedup).
#[inline]
pub fn is_available() -> bool {
    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    {
        backend::is_available()
    }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        false
    }
}

/// Index of the first JSON string "special" byte (`"`, `\`, or `< 0x20`).
#[inline]
pub fn find_string_special(haystack: &[u8]) -> Option<usize> {
    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    {
        backend::find_string_special(haystack)
    }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        scalar::find_string_special(haystack)
    }
}

/// Index of the first non-whitespace byte (`space` / `\t` / `\n` / `\r`).
#[inline]
pub fn find_non_whitespace(haystack: &[u8]) -> Option<usize> {
    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    {
        backend::find_non_whitespace(haystack)
    }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        scalar::find_non_whitespace(haystack)
    }
}
