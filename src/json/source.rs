use crate::Location;

/// Cursor over a contiguous byte slice. Enables zero-copy string lexemes
/// ([`std::borrow::Cow::Borrowed`]) because slices borrow the caller's buffer.
///
/// The hot path tracks only a byte offset (`pos`); advancing is O(1). 1-based
/// line/column is computed on demand via [`location_at`](SliceCursor::location_at)
/// — only needed for error diagnostics — so the common (no-error) path never
/// pays for per-byte position bookkeeping.
#[derive(Debug, Clone)]
pub struct SliceCursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> SliceCursor<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    pub fn position(&self) -> usize {
        self.pos
    }

    /// Compute 1-based line/column for an absolute byte offset. O(offset);
    /// used only on the error path. Column counts bytes (matching historical
    /// behavior and existing span-based diagnostics).
    pub fn location_at(&self, byte: usize) -> Location {
        let end = byte.min(self.data.len());
        let mut line = 1usize;
        let mut column = 1usize;
        for &b in &self.data[..end] {
            if b == b'\n' {
                line += 1;
                column = 1;
            } else {
                column += 1;
            }
        }
        Location { line, column }
    }

    pub fn buffered_end(&self) -> usize {
        self.data.len()
    }

    pub fn remaining(&self) -> &'a [u8] {
        &self.data[self.pos..]
    }

    #[inline]
    pub fn bump_n(&mut self, n: usize) {
        self.pos = (self.pos + n).min(self.data.len());
    }

    pub fn peek_byte(&self) -> Option<u8> {
        self.data.get(self.pos).copied()
    }

    pub fn slice_abs(&self, start: usize, end: usize) -> &'a [u8] {
        &self.data[start..end]
    }

    pub fn peek_bytes(&self, len: usize) -> Option<&'a [u8]> {
        let end = self.pos + len;
        if end > self.data.len() {
            return None;
        }
        Some(&self.data[self.pos..end])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bump_and_slice_are_byte_based() {
        let mut c = SliceCursor::new(b"abc,42");
        assert_eq!(c.peek_byte(), Some(b'a'));
        c.bump_n(3);
        assert_eq!(c.position(), 3);
        assert_eq!(c.peek_byte(), Some(b','));
        assert_eq!(c.slice_abs(0, 3), b"abc");
        // Bumping past the end saturates rather than overflowing.
        c.bump_n(100);
        assert_eq!(c.position(), 6);
        assert_eq!(c.peek_byte(), None);
    }

    #[test]
    fn location_at_counts_lines_and_columns() {
        let c = SliceCursor::new(b"ab\ncd");
        assert_eq!(c.location_at(0), Location { line: 1, column: 1 });
        assert_eq!(c.location_at(2), Location { line: 1, column: 3 });
        // Offset 3 is just past the '\n'.
        assert_eq!(c.location_at(3), Location { line: 2, column: 1 });
        assert_eq!(c.location_at(4), Location { line: 2, column: 2 });
        // Out-of-range offsets clamp to the end.
        assert_eq!(c.location_at(999), Location { line: 2, column: 3 });
    }
}
