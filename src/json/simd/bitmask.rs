//! Dense bit vector backed by `u64` buckets (bit `i` → bucket `i/64`, lane `i%64`).

use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not};

pub const BITS_PER_BUCKET: usize = 64;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Bitmask {
    buckets: Vec<u64>,
    len: usize,
}

impl Bitmask {
    /// Zero mask of `bit_len` bits (at least one bucket).
    #[inline]
    pub fn zeros(bit_len: usize) -> Self {
        let n = if bit_len == 0 {
            1
        } else {
            bit_len.div_ceil(BITS_PER_BUCKET)
        };
        Self {
            buckets: vec![0u64; n],
            len: bit_len,
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
    pub fn bucket_count(&self) -> usize {
        self.buckets.len()
    }

    #[inline]
    pub fn buckets(&self) -> &[u64] {
        &self.buckets
    }

    #[inline]
    pub fn buckets_mut(&mut self) -> &mut [u64] {
        &mut self.buckets
    }

    #[inline]
    pub fn bucket(&self, i: usize) -> u64 {
        self.buckets.get(i).copied().unwrap_or(0)
    }

    #[inline]
    pub fn set_bucket(&mut self, i: usize, value: u64) {
        if let Some(slot) = self.buckets.get_mut(i) {
            *slot = value;
        }
    }

    #[inline]
    pub fn get(&self, index: usize) -> bool {
        if index >= self.len {
            return false;
        }
        let bucket = index / BITS_PER_BUCKET;
        let bit = index % BITS_PER_BUCKET;
        (self.buckets[bucket] >> bit) & 1 == 1
    }

    #[inline]
    pub fn set(&mut self, index: usize) {
        if index >= self.len {
            return;
        }
        let bucket = index / BITS_PER_BUCKET;
        let bit = index % BITS_PER_BUCKET;
        self.buckets[bucket] |= 1u64 << bit;
    }

    #[inline]
    pub fn unset(&mut self, index: usize) {
        if index >= self.len {
            return;
        }
        let bucket = index / BITS_PER_BUCKET;
        let bit = index % BITS_PER_BUCKET;
        self.buckets[bucket] &= !(1u64 << bit);
    }

    #[inline]
    pub fn set_to(&mut self, index: usize, value: bool) {
        if value {
            self.set(index);
        } else {
            self.unset(index);
        }
    }

    #[inline]
    pub fn and_with(&mut self, other: &Self) {
        debug_assert_eq!(self.len, other.len);
        debug_assert_eq!(self.buckets.len(), other.buckets.len());
        for (a, &b) in self.buckets.iter_mut().zip(other.buckets.iter()) {
            *a &= b;
        }
    }

    #[inline]
    pub fn or_with(&mut self, other: &Self) {
        debug_assert_eq!(self.len, other.len);
        for (a, &b) in self.buckets.iter_mut().zip(other.buckets.iter()) {
            *a |= b;
        }
    }

    #[inline]
    pub fn xor_with(&mut self, other: &Self) {
        debug_assert_eq!(self.len, other.len);
        for (a, &b) in self.buckets.iter_mut().zip(other.buckets.iter()) {
            *a ^= b;
        }
    }

    #[inline]
    pub fn and_not_with(&mut self, other: &Self) {
        debug_assert_eq!(self.len, other.len);
        for (a, &b) in self.buckets.iter_mut().zip(other.buckets.iter()) {
            *a &= !b;
        }
        self.clear_tail();
    }

    #[inline]
    pub fn not_in_place(&mut self) {
        for b in &mut self.buckets {
            *b = !*b;
        }
        self.clear_tail();
    }

    #[inline]
    fn clear_tail(&mut self) {
        if self.len == 0 {
            self.buckets.fill(0);
            return;
        }
        let rem = self.len % BITS_PER_BUCKET;
        if rem != 0 {
            let last = self.len / BITS_PER_BUCKET;
            if let Some(slot) = self.buckets.get_mut(last) {
                *slot &= (1u64 << rem) - 1;
            }
        }
        let used = self.len.div_ceil(BITS_PER_BUCKET);
        for slot in self.buckets.iter_mut().skip(used) {
            *slot = 0;
        }
    }

    pub fn positions(&self) -> Vec<usize> {
        let mut out = Vec::new();
        self.for_each_set(|i| out.push(i));
        out
    }

    #[inline]
    pub fn for_each_set(&self, mut f: impl FnMut(usize)) {
        for (bucket_idx, &bucket) in self.buckets.iter().enumerate() {
            let mut w = bucket;
            while w != 0 {
                let bit = w.trailing_zeros() as usize;
                let index = bucket_idx * BITS_PER_BUCKET + bit;
                if index >= self.len {
                    return;
                }
                f(index);
                w &= w - 1;
            }
        }
    }

    pub fn flatten_u32(&self) -> Vec<u32> {
        let mut out = Vec::new();
        self.for_each_set(|i| out.push(i as u32));
        out
    }
}

impl BitAnd for &Bitmask {
    type Output = Bitmask;

    #[inline]
    fn bitand(self, rhs: Self) -> Bitmask {
        let mut out = self.clone();
        out.and_with(rhs);
        out
    }
}

impl BitAndAssign<&Bitmask> for Bitmask {
    #[inline]
    fn bitand_assign(&mut self, rhs: &Bitmask) {
        self.and_with(rhs);
    }
}

impl BitOr for &Bitmask {
    type Output = Bitmask;

    #[inline]
    fn bitor(self, rhs: Self) -> Bitmask {
        let mut out = self.clone();
        out.or_with(rhs);
        out
    }
}

impl BitOrAssign<&Bitmask> for Bitmask {
    #[inline]
    fn bitor_assign(&mut self, rhs: &Bitmask) {
        self.or_with(rhs);
    }
}

impl BitXor for &Bitmask {
    type Output = Bitmask;

    #[inline]
    fn bitxor(self, rhs: Self) -> Bitmask {
        let mut out = self.clone();
        out.xor_with(rhs);
        out
    }
}

impl BitXorAssign<&Bitmask> for Bitmask {
    #[inline]
    fn bitxor_assign(&mut self, rhs: &Bitmask) {
        self.xor_with(rhs);
    }
}

impl Not for &Bitmask {
    type Output = Bitmask;

    #[inline]
    fn not(self) -> Bitmask {
        let mut out = self.clone();
        out.not_in_place();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_get_across_bit_positions() {
        let mut bm = Bitmask::zeros(70);
        bm.set(0);
        bm.set(31);
        bm.set(32);
        bm.set(63);
        bm.set(64);
        bm.set(69);
        assert!(bm.get(0));
        assert!(bm.get(31));
        assert!(bm.get(32));
        assert!(bm.get(63));
        assert!(bm.get(64));
        assert!(bm.get(69));
        assert!(!bm.get(1));
        assert!(!bm.get(65));
        assert_eq!(bm.positions(), vec![0, 31, 32, 63, 64, 69]);
    }

    #[test]
    fn bucket_roundtrip() {
        let mut bm = Bitmask::zeros(70);
        let word = (1u64 << 5) | (1u64 << 40) | (1u64 << 63);
        bm.set_bucket(0, word);
        assert_eq!(bm.bucket(0), word);
        assert!(bm.get(5));
        assert!(bm.get(40));
        assert!(bm.get(63));
        assert!(!bm.get(64));
    }

    #[test]
    fn bit_ops() {
        let mut a = Bitmask::zeros(16);
        let mut b = Bitmask::zeros(16);
        a.set(1);
        a.set(3);
        b.set(3);
        b.set(5);

        let and = &a & &b;
        assert_eq!(and.positions(), vec![3]);

        let or = &a | &b;
        assert_eq!(or.positions(), vec![1, 3, 5]);

        let xor = &a ^ &b;
        assert_eq!(xor.positions(), vec![1, 5]);

        let mut a2 = a.clone();
        a2.and_not_with(&b);
        assert_eq!(a2.positions(), vec![1]);
    }

    #[test]
    fn not_clears_tail_bits() {
        let mut bm = Bitmask::zeros(5);
        bm.not_in_place();
        assert_eq!(bm.positions(), vec![0, 1, 2, 3, 4]);
        assert_eq!(bm.bucket(0) & !0b1_1111, 0);
    }
}
