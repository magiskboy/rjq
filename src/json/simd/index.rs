//! Flatten blended structural bitmasks into a compact index (simdjson-style).
//!
//! Each entry is the absolute byte offset of a structural *start*: operators
//! `{ } [ ] : ,`, opening quotes, and scalar starts (`123`, `true`, …).

use super::blend::BlendedBitmasks;

/// Compact list of structural-start byte offsets derived from [`BlendedBitmasks`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuralIndex {
    pub indexes: Vec<u32>,
    pub input_len: usize,
}

impl StructuralIndex {
    pub fn from_blended(blended: &BlendedBitmasks) -> Self {
        Self {
            indexes: flatten(&blended.structural, blended.input_len),
            input_len: blended.input_len,
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.indexes.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.indexes.is_empty()
    }

    #[inline]
    pub fn get(&self, i: usize) -> Option<usize> {
        self.indexes.get(i).map(|&x| x as usize)
    }
}

/// Collect set-bit positions from a `Vec<u64>` bitmask into ascending offsets.
pub fn flatten(bits: &[u64], input_len: usize) -> Vec<u32> {
    let mut out = Vec::new();
    for (word_idx, &word) in bits.iter().enumerate() {
        let mut w = word;
        while w != 0 {
            let bit = w.trailing_zeros() as usize;
            let index = word_idx * 64 + bit;
            if index >= input_len {
                break;
            }
            out.push(index as u32);
            w &= w - 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::simd::{blend, scalar};

    #[test]
    fn flattens_simple_object() {
        let input = br#"{"a":1}"#;
        let idx = StructuralIndex::from_blended(&blend::blend(&scalar::compute_bitmasks(input)));
        // { " : 1 }
        assert_eq!(idx.indexes, vec![0, 1, 4, 5, 6]);
    }
}
