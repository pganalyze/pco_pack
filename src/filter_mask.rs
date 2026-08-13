use bitvec::prelude::*;

/// A bitmask used during filtering to track which rows match filter conditions.
///
/// Wraps `BitVec<u64, Lsb0>` and provides chunked operations for building filters.
/// Constructs full u64 words at a time instead of setting bits individually.
#[derive(Clone)]
pub struct FilterMask(BitVec<u64, Lsb0>);

impl FilterMask {
    /// Creates a new bitmask with all bits set to true (all rows matching).
    #[inline]
    pub fn ones(len: usize) -> Self {
        Self(bitvec![u64, Lsb0; 1; len])
    }

    /// Fill all bits with the given value.
    #[inline]
    pub fn fill(&mut self, val: bool) {
        self.0.fill(val);
    }

    /// Returns a borrowed slice of this bitmask for the given range.
    #[inline]
    pub fn get_range(&self, range: std::ops::Range<usize>) -> &BitSlice<u64, Lsb0> {
        self.0.get(range).unwrap_or_default()
    }

    /// Returns a reference to the underlying bitslice.
    #[inline]
    pub fn as_bitslice(&self) -> &BitSlice<u64, Lsb0> {
        self.0.as_bitslice()
    }

    /// Bitwise AND another mask into this one in-place.
    #[inline]
    pub fn and_with(&mut self, other: &Self) {
        self.0 &= other.0.as_bitslice();
    }

    /// Returns the number of set bits (ones) in this mask.
    #[inline]
    pub fn count_ones(&self) -> usize {
        self.0.count_ones() as usize
    }

    /// Returns a reference to the underlying raw u64 slice for low-level iteration over chunks.
    /// Each word contains 64 bits; the last word may have unused high bits.
    #[inline]
    pub fn as_raw_slice(&self) -> &[u64] {
        self.0.as_raw_slice()
    }

    /// Handle an empty column during filtering (e.g., missing field from old data in schema evolution).
    /// Treats all rows as having the default value; if it doesn't match, excludes them.
    #[inline]
    pub fn handle_empty_column<T, F>(&mut self, mut pred: F)
    where
        T: Default,
        F: FnMut(&T) -> bool,
    {
        if !self.0.is_empty() && !pred(&T::default()) {
            self.fill(false);
        }
    }

    /// Build a predicate mask from `values` and intersect it with this mask in-place.
    /// Builds chunked u64 words directly and ANDs them into existing bits.
    /// This avoids allocating a temporary FilterMask.
    #[inline]
    pub fn build_with_and<T, F>(&mut self, values: &[T], mut pred: F)
    where
        T: Default,
        F: FnMut(&T) -> bool,
    {
        let len = values.len().min(self.0.len());
        if len == 0 && !self.0.is_empty() {
            self.handle_empty_column(&mut pred);
            return;
        }
        let mut words = vec![0; (len + 63) / 64];
        Self::build_words(&values[..len], pred, &mut words);
        for (i, &word) in words.iter().enumerate() {
            let start = i * 64;
            let end = core::cmp::min(start + 64, len);
            let word_bits = self.0.get_mut(start..end).unwrap();
            word_bits.store_le(word_bits.load_le::<u64>() & word);
        }
    }

    /// Specialized builder for boolean arrays. For each chunk of 64 bools, sets bit i IFF the i-th element is true.
    /// Fully unrolled per-chunk loop; produces one word at a time without intermediate Vec allocations.
    #[inline]
    pub fn from_bool_slice(slice: &[bool]) -> Self {
        let mut words = vec![0; (slice.len() + 63) / 64];
        Self::build_words(slice, |&v| v, &mut words);
        Self(BitVec::from_slice(&words))
    }

    /// Builds a mask into the specified range of bits in this FilterMask.
    /// Each value corresponds to one bit starting at `start_bit`, using chunked u64 construction.
    /// Intersects with existing bits so each filter step narrows results.
    #[inline]
    pub fn build_into<T, F>(&mut self, start_bit: usize, values: &[T], mut pred: F)
    where
        T: Default,
        F: FnMut(&T) -> bool,
    {
        if start_bit >= self.0.len() {
            return;
        }
        if values.is_empty() && !self.0.is_empty() {
            self.handle_empty_column(&mut pred);
            return;
        }
        let end = (start_bit + values.len()).min(self.0.len());
        let effective_len = end.saturating_sub(start_bit);
        let mut words = vec![0; (effective_len + 63) / 64];
        // Build predicate words from the relevant slice of values.
        Self::build_words(&values[..effective_len], pred, &mut words);
        // Write constructed words into self at the correct offset,
        // masking out any bits that fall beyond effective_len.
        let view = self.0.get_mut(start_bit..end).unwrap();
        for (i, &word) in words.iter().enumerate() {
            let word_start = i * 64;
            let bits_to_write = core::cmp::min(64usize, effective_len.saturating_sub(word_start));
            if bits_to_write == 0 || word_start >= view.len() {
                continue;
            }
            let mask = if bits_to_write < 64 { (1 << bits_to_write) - 1 } else { u64::MAX };
            let predicate_word = word & mask;
            let slice = view.get_mut(word_start..word_start + bits_to_write).unwrap();
            slice.store_le(slice.load_le::<u64>() & predicate_word);
        }
    }

    /// Core helper: builds chunked u64 predicate words into the provided buffer.
    /// Each word holds up to 64 bits; bit i is set IFF pred(values[i]) is true.
    #[inline(always)]
    fn build_words<T, F>(values: &[T], mut pred: F, out: &mut [u64])
    where
        F: FnMut(&T) -> bool,
    {
        let len = values.len();
        if len == 0 || out.is_empty() {
            return;
        }
        let mut chunk_iter = values.chunks_exact(64);
        for (c, chunk) in chunk_iter.by_ref().enumerate() {
            if c >= out.len() {
                break;
            }
            let mut word: u64 = 0;
            for (bit_pos, val) in chunk.iter().enumerate() {
                if pred(val) {
                    word |= 1 << bit_pos;
                }
            }
            out[c] = word;
        }
        let full_chunks = len / 64;
        if full_chunks < out.len() {
            for (bit_pos, val) in chunk_iter.remainder().iter().enumerate() {
                if pred(val) {
                    out[full_chunks] |= 1 << bit_pos;
                }
            }
        }
    }
}
