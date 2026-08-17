use bitvec::prelude::*;

/// A bitmask used during filtering to track which rows match filter conditions.
///
/// Wraps `BitVec<u64, Lsb0>` and provides chunked operations for building filters.
/// Constructs full u64 words at a time instead of setting bits individually.
#[derive(Clone)]
pub struct FilterMask(pub(crate) BitVec<u64, Lsb0>);

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

    /// Returns true if any valid bit is set, i.e. some row still matches.
    #[inline]
    pub fn any_set(&self) -> bool {
        self.any_set_in_range(0..self.0.len())
    }

    /// Returns true if any bit in `range` is set (padding ignored).
    #[inline]
    pub fn any_set_in_range(&self, range: std::ops::Range<usize>) -> bool {
        let len = self.0.len();
        if range.start >= len || range.end <= range.start {
            return false;
        }
        let start = range.start.min(len);
        let end = range.end.min(len);
        let raw = self.0.as_raw_slice();
        let first_word = start / 64;
        let last_word = (end - 1) / 64;
        if first_word == last_word {
            let off = start - first_word * 64;
            return raw[first_word] & Self::in_word_mask(off, end - first_word * 64) != 0;
        }
        if raw[first_word] & Self::in_word_mask(start - first_word * 64, 64) != 0 {
            return true;
        }
        // Middle words need no masking.
        if raw[first_word + 1..last_word].iter().any(|&w| w != 0) {
            return true;
        }
        raw[last_word] & Self::in_word_mask(0, end - last_word * 64) != 0
    }

    /// Mask for bits `[start, end)` within one 64-bit word (0 <= start <= end <= 64).
    #[inline]
    fn in_word_mask(start: usize, end: usize) -> u64 {
        let hi = if end >= 64 { u64::MAX } else { (1u64 << end) - 1 };
        let lo = if start == 0 { 0 } else { (1u64 << start) - 1 };
        hi & !lo
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
        // Skip groups with no set bits — ANDing them is a no-op and pred need not run.
        let raw = self.0.as_raw_mut_slice();
        for (w, chunk) in values[..len].chunks(64).enumerate() {
            let mask = if chunk.len() == 64 { u64::MAX } else { (1u64 << chunk.len()) - 1 };
            let word = raw[w];
            if word & mask == 0 {
                continue;
            }
            let mut pred_word: u64 = 0;
            for (i, val) in chunk.iter().enumerate() {
                pred_word |= (pred(val) as u64) << i;
            }
            // AND the predicate in, preserving any bits above the valid range.
            raw[w] = (word & pred_word) | (word & !mask);
        }
    }

    #[inline]
    pub fn from_bool_slice(slice: &[bool]) -> Self {
        let len = slice.len();
        if len == 0 {
            return Self(BitVec::new());
        }
        let mut words = Vec::with_capacity((len + 63) / 64);
        for chunk in slice.chunks(64) {
            let mut word = 0u64;
            for (i, &b) in chunk.iter().enumerate() {
                word |= (b as u64) << i;
            }
            words.push(word);
        }
        let mut bits = BitVec::from_vec(words);
        bits.truncate(len);
        Self(bits)
    }

    /// Intersects a predicate mask built from `values` into bits starting at `start_bit`.
    /// Skips groups with no set bits; uses raw-word writes when aligned, bit-slice otherwise.
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
        if effective_len == 0 {
            return;
        }
        // Skip groups with no set bits — ANDing them is a no-op and pred need not run.
        if (start_bit & 63) == 0 {
            // Word-aligned fast path: every group is a single backing word.
            let raw = self.0.as_raw_mut_slice();
            let mut w = start_bit / 64;
            let mut i = 0usize;
            while i < effective_len {
                let bits = core::cmp::min(64usize, effective_len - i);
                let word = raw[w];
                let mask = if bits == 64 { u64::MAX } else { (1u64 << bits) - 1 };
                if word & mask != 0 {
                    let mut pred_word: u64 = 0;
                    for (j, val) in values[i..i + bits].iter().enumerate() {
                        pred_word |= (pred(val) as u64) << j;
                    }
                    // AND the predicate in, preserving any bits above the valid range.
                    raw[w] = (word & pred_word) | (word & !mask);
                }
                w += 1;
                i += 64;
            }
        } else {
            // Unaligned start: each group may straddle two words; bit-slice stores handle it.
            let mut i = 0usize;
            while i < effective_len {
                let bits = core::cmp::min(64usize, effective_len - i);
                let abs_start = start_bit + i;
                if self.any_set_in_range(abs_start..abs_start + bits) {
                    let mut word: u64 = 0;
                    for (j, val) in values[i..i + bits].iter().enumerate() {
                        word |= (pred(val) as u64) << j;
                    }
                    let mask = if bits < 64 { (1u64 << bits) - 1 } else { u64::MAX };
                    let slice = self.0.get_mut(abs_start..abs_start + bits).unwrap();
                    slice.store_le(slice.load_le::<u64>() & (word & mask));
                }
                i += 64;
            }
        }
    }
}
