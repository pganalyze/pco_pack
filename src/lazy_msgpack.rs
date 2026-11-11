use serde::Deserialize;
use std::io::Cursor;

/// Chunk size for batched serialization/deserialization. Aligns with u64 word size for FilterMask optimizations.
/// Filters deserialize one chunk at a time into a small Vec<T>, apply predicates, then drop it.
/// This avoids materializing an entire column of large dynamic types.
#[allow(dead_code)]
pub(crate) const CHUNK_SIZE: usize = 128;

/// Format version indicating chunked msgpack arrays. Version byte stored as first byte of decompressed payload.
/// Allows readers to distinguish between new chunked format and legacy flat fallback data.
#[allow(dead_code)]
pub(crate) const FORMAT_VERSION_CHUNKED: u8 = 1;

/// Bytes targets of a text eq/IN filter. Msgpack `str` payloads are the
/// exact UTF-8 bytes of the column's values, so comparing payload bytes
/// against these targets is equivalent to deserializing a row and applying
/// `filter_match`, but allocates nothing.
pub(crate) enum TextTargets<'a> {
    Eq(&'a [u8]),
    In(ahash::HashSet<&'a [u8]>),
}

impl TextTargets<'_> {
    #[inline]
    fn matches(&self, payload: &[u8]) -> bool {
        match self {
            TextTargets::Eq(target) => payload == *target,
            TextTargets::In(set) => set.contains(payload),
        }
    }
}

/// The set behind an `InclusionString` target is built once per filter call,
/// not per chunk; non-text filters get `None` and skip the scan entirely.
#[allow(dead_code)]
fn text_targets(filter: &crate::Filter) -> Option<TextTargets<'_>> {
    match filter {
        crate::Filter::String(s) => Some(TextTargets::Eq(s.as_bytes())),
        crate::Filter::InclusionString(set) => Some(TextTargets::In(set.iter().map(String::as_bytes).collect())),
        _ => None,
    }
}

/// Helper functions for lazy msgpack deserialization: deserialize or skip chunked arrays from decompressed bytes.
#[allow(dead_code)]
pub(crate) mod helpers {
    use super::*;

    /// Deserialize one item starting at `pos` from a flat (non-chunked) msgpack byte stream.
    #[inline]
    pub(crate) fn deserialize_one<T: serde::de::DeserializeOwned>(bytes: &[u8], pos: &mut usize) -> Option<T> {
        if *pos >= bytes.len() {
            return None;
        }
        let mut deserializer = rmp_serde::Deserializer::new(Cursor::new(&bytes[*pos..]));
        let val = T::deserialize(&mut deserializer).ok()?;
        *pos += deserializer.into_inner().position() as usize;
        Some(val)
    }

    /// Deserialize a full chunk (Vec<T>) starting at `pos`. Chunk is expected to be a msgpack array.
    #[inline]
    pub(crate) fn deserialize_chunk<T: serde::de::DeserializeOwned>(bytes: &[u8], pos: &mut usize) -> Option<Vec<T>> {
        if *pos >= bytes.len() {
            return None;
        }
        let mut deserializer = rmp_serde::Deserializer::new(Cursor::new(&bytes[*pos..]));
        let chunk: Vec<T> = Vec::<T>::deserialize(&mut deserializer).ok()?;
        *pos += deserializer.into_inner().position() as usize;
        Some(chunk)
    }

    /// Get item at `offset_in_chunk` from the current chunk in bytes[pos..]. Deserializes whole chunk then returns element.
    #[inline]
    pub(crate) fn get_from_chunk<T: serde::de::DeserializeOwned + Clone>(
        bytes: &[u8], pos: &mut usize, offset_in_chunk: usize,
    ) -> Option<T> {
        if *pos >= bytes.len() {
            return None;
        }
        let mut deserializer = rmp_serde::Deserializer::new(Cursor::new(&bytes[*pos..]));
        let chunk: Vec<T> = Vec::<T>::deserialize(&mut deserializer).ok()?;
        *pos += deserializer.into_inner().position() as usize;
        chunk.get(offset_in_chunk).cloned()
    }

    /// Skip an entire msgpack array chunk by deserialating it as IgnoredAny. Advances pos past this chunk's bytes.
    #[inline]
    pub(crate) fn skip_chunk(bytes: &[u8], pos: &mut usize) -> Option<()> {
        if *pos >= bytes.len() {
            return None;
        }
        let mut deserializer = rmp_serde::Deserializer::new(Cursor::new(&bytes[*pos..]));
        <Vec<serde::de::IgnoredAny>>::deserialize(&mut deserializer).ok()?;
        *pos += deserializer.into_inner().position() as usize;
        Some(())
    }

    /// Two/four-byte big-endian reads with bounds checks, the msgpack
    /// header widths the chunk scan needs.
    #[inline]
    fn be_u16(bytes: &[u8], p: usize) -> Option<u16> {
        let a: [u8; 2] = bytes.get(p..p + 2)?.try_into().ok()?;
        Some(u16::from_be_bytes(a))
    }

    #[inline]
    fn be_u32(bytes: &[u8], p: usize) -> Option<u32> {
        let a: [u8; 4] = bytes.get(p..p + 4)?.try_into().ok()?;
        Some(u32::from_be_bytes(a))
    }

    /// In-place text eq/IN scan of one msgpack array chunk. Reads the array
    /// header, walks every element's length prefix, compares the payload
    /// bytes against `targets`, and ANDs one predicate bit per row into
    /// `matches` without materializing a single row value. `pos` advances
    /// past the chunk on success and returns its row count; any byte the
    /// scan can't interpret (non-array header, element type outside the
    /// nil/str family, truncation) returns `None` with `pos` untouched, and
    /// the caller takes the deserialize-and-predicate path. `row_start` must
    /// be word-aligned, which every chunk-major walk in this module
    /// maintains.
    #[inline]
    pub(crate) fn scan_chunk(
        bytes: &[u8], pos: &mut usize, targets: &TextTargets<'_>, row_start: usize, matches: &mut crate::FilterMask,
    ) -> Option<usize> {
        if *pos >= bytes.len() || row_start >= matches.len() || !row_start.is_multiple_of(64) {
            return None;
        }
        let mut p = *pos;
        let n = match *bytes.get(p)? {
            b if (0x90..=0x9f).contains(&b) => {
                p += 1;
                (b & 0x0f) as usize
            }
            0xdc => {
                let n = be_u16(bytes, p + 1)?;
                p += 3;
                n as usize
            }
            0xdd => {
                let n = be_u32(bytes, p + 1)?;
                p += 5;
                n as usize
            }
            _ => return None,
        };
        if n > CHUNK_SIZE {
            return None;
        }
        // A chunk running past the mask's last row cannot be walked with the
        // word-AND below without leaving `pos` mid-chunk (the loop and `pos`
        // would only cover the clamped prefix). Decline with `pos` untouched
        // and let the deserialize path, which clamps its own writes, take it.
        if n > matches.len() - row_start {
            return None;
        }
        let mut words = [0u64; 2];
        for i in 0..n {
            let mut matched = false;
            match *bytes.get(p)? {
                // Nil is accepted grammar and answers no match: an absent
                // value never satisfies a text equality, which agrees with
                // `Option::filter_match`, whose `None` arm is false.
                0xc0 => p += 1,
                b if (0xa0..=0xbf).contains(&b) => {
                    let len = (b & 0x1f) as usize;
                    let payload = bytes.get(p + 1..p + 1 + len)?;
                    p += 1 + len;
                    matched = targets.matches(payload);
                }
                0xd9 => {
                    let len = *bytes.get(p + 1)? as usize;
                    let payload = bytes.get(p + 2..p + 2 + len)?;
                    p += 2 + len;
                    matched = targets.matches(payload);
                }
                0xda => {
                    let len = be_u16(bytes, p + 1)? as usize;
                    let payload = bytes.get(p + 3..p + 3 + len)?;
                    p += 3 + len;
                    matched = targets.matches(payload);
                }
                0xdb => {
                    let len = be_u32(bytes, p + 1)? as usize;
                    let payload = bytes.get(p + 5..p + 5 + len)?;
                    p += 5 + len;
                    matched = targets.matches(payload);
                }
                _ => return None,
            }
            if matched {
                words[i / 64] |= 1u64 << (i % 64);
            }
        }
        let raw = matches.as_raw_mut_slice();
        let w = row_start / 64;
        let lo_bits = n.min(64);
        let lo_mask = if lo_bits == 64 { u64::MAX } else { (1u64 << lo_bits) - 1 };
        raw[w] = (raw[w] & words[0]) | (raw[w] & !lo_mask);
        if n > 64 {
            let hi_bits = n - 64;
            let hi_mask = if hi_bits == 64 { u64::MAX } else { (1u64 << hi_bits) - 1 };
            raw[w + 1] = (raw[w + 1] & words[1]) | (raw[w + 1] & !hi_mask);
        }
        *pos = p;
        Some(n)
    }

    /// Ascending survivor-row cursor over tail-masked mask words with `peek`,
    /// so the chunk-major gather below can rule a chunk out before deciding
    /// between `skip_chunk` and `deserialize_chunk`.
    pub(crate) struct MaskBits<'a> {
        words: &'a [u64],
        word_idx: usize,
        cur: u64,
    }

    impl<'a> MaskBits<'a> {
        pub(crate) fn new(words: &'a [u64]) -> Self {
            Self { words, word_idx: 0, cur: 0 }
        }

        /// Row index of the next survivor without consuming it.
        #[inline]
        pub(crate) fn peek(&mut self) -> Option<usize> {
            while self.cur == 0 {
                if self.word_idx >= self.words.len() {
                    return None;
                }
                self.cur = self.words[self.word_idx];
                self.word_idx += 1;
            }
            Some((self.word_idx - 1) * 64 + self.cur.trailing_zeros() as usize)
        }

        /// Consume and return the next survivor row.
        #[inline]
        pub(crate) fn next(&mut self) -> Option<usize> {
            let row = self.peek()?;
            self.cur &= self.cur - 1;
            Some(row)
        }

        /// Survivors not yet consumed, including a peeked one.
        #[inline]
        pub(crate) fn count_remaining(&self) -> usize {
            self.cur.count_ones() as usize
                + self.words[self.word_idx..].iter().map(|w| w.count_ones() as usize).sum::<usize>()
        }
    }

    /// Chunk-major gather for lazy-msgpack readers. Walks the set bits of the
    /// tail-masked `mask_words` forward against the chunk cursor: a chunk with
    /// no survivor is `skip_chunk`ed without allocating, a chunk with survivors
    /// is deserialized once and its survivors are moved out with `mem::take`
    /// (leaving `Default` holes that drop with the chunk). This removes the
    /// per-survivor `cloned()` the row-at-a-time `get` fallback pays, which
    /// matters most for text and the deep clone of `serde_json::Value`.
    ///
    /// `cached`/`cached_row_start` mirror the reader's forward-cursor state;
    /// `pos` must point at the byte start of the chunk after `cached` (or at
    /// 0 with no cache). Survivors at or past `total_rows` push `Default`,
    /// matching `get` and `pop_inner` for legitimately short columns, but a
    /// chunk that fails to decode or a stream that runs dry while a row below
    /// `total_rows` is still demanded is corrupt data and returns `Err`.
    #[inline]
    pub(crate) fn gather_chunked<T: serde::de::DeserializeOwned + Default>(
        data: &[u8], total_rows: usize, cached: Option<Vec<T>>, cached_row_start: usize, mut pos: usize,
        mask_words: &[u64], out: &mut Vec<T>,
    ) -> anyhow::Result<()> {
        let mut bits = MaskBits::new(mask_words);
        out.reserve(bits.count_remaining());

        // Drain the reader's cached chunk first; its bytes are behind `pos`.
        // Rows the cursor has already passed push `Default`, the same answer
        // `get` gives once its forward-only cursor is stuck ahead of them.
        let mut scan_row_start = 0usize;
        if let Some(mut chunk) = cached {
            while let Some(row) = bits.peek() {
                if row >= total_rows || row >= cached_row_start + chunk.len() {
                    break;
                }
                if row < cached_row_start {
                    out.push(T::default());
                    bits.next();
                    continue;
                }
                out.push(chunk.get_mut(row - cached_row_start).map(std::mem::take).unwrap_or_default());
                bits.next();
            }
            scan_row_start = cached_row_start + chunk.len();
        }

        loop {
            let first = match bits.peek() {
                Some(row) if row < total_rows => row,
                Some(_) | None => {
                    out.extend((0..bits.count_remaining()).map(|_| T::default()));
                    return Ok(());
                }
            };
            if first >= scan_row_start + CHUNK_SIZE {
                if skip_chunk(data, &mut pos).is_none() {
                    return Err(corrupt_stream_error::<T>("skip", first, total_rows, pos));
                }
                scan_row_start += std::cmp::min(CHUNK_SIZE, total_rows.saturating_sub(scan_row_start));
                continue;
            }
            let mut chunk = match deserialize_chunk::<T>(data, &mut pos) {
                Some(chunk) => chunk,
                None => return Err(corrupt_stream_error::<T>("read", first, total_rows, pos)),
            };
            let end = scan_row_start + chunk.len();
            while let Some(row) = bits.peek() {
                if row >= end {
                    break;
                }
                out.push(chunk.get_mut(row - scan_row_start).map(std::mem::take).unwrap_or_default());
                bits.next();
            }
            scan_row_start = end;
        }
    }

    /// Error for a chunk the column header promised but the byte stream
    /// cannot deliver. The type name identifies the column kind; the row and
    /// byte offsets say where the stream died.
    fn corrupt_stream_error<T>(action: &str, row: usize, total_rows: usize, pos: usize) -> anyhow::Error {
        anyhow::anyhow!(
            "lazy-msgpack column <{}> is corrupt: failed to {action} the chunk holding row {row} \
             (column claims {total_rows} rows, stream broke at byte {pos})",
            std::any::type_name::<T>()
        )
    }
}

/// Generates a lazy-deserialization `PcoSerde` impl for a chunked msgpack+zstd type.
///
/// Format on disk/wire: `[u64 LE total_rows][u64 LE compressed_len][zstd compressed payload]`.
/// Decompressed payload layout: `[u8 format_version][chunk_0][chunk_1]...` where each chunk is a msgpack array of up to CHUNK_SIZE items.
///
/// This enables efficient filtering by deserializing entire chunks as Vec<T> and applying
/// batch u64-word construction instead of per-item operations. get() scans forward-only through chunks.
///
/// **Parameters:**
/// - `$reader`: Reader struct name (e.g., StringReader)
/// - `$writer_ty`: Writer type (usually `()` for lazy types without custom write batching)
/// - `$ty`: Type implementing PcoSerde; caller provides separate PcoFilter impl and FallbackReader impl
#[allow(unused_macros)]
macro_rules! impl_lazy_msgpack {
    ($reader:ident; $writer_ty:ty; $ty:ty; $cheap_to_clone:expr) => {
        pub struct $reader {
            pub msgpack_bytes: std::sync::Arc<[u8]>,
            pub total_rows: usize,
            /// Format version byte indicating serialization layout (1 = chunked arrays).
            pub format_version: u8,
            /// Current position in the decompressed data stream (after format_version byte),
            /// used when advancing between chunks during forward-only get() calls.
            pub(crate) data_pos: usize,
            /// Cached deserialized chunk, valid only while reading within its row range.
            cached_chunk: Option<Vec<$ty>>,
            /// Row index where `cached_chunk` starts (0-based).
            cached_row_start: usize,
            /// Fully decoded values for fallback readers.
            predecoded: Option<std::sync::Arc<[$ty]>>,
        }

        impl Clone for $reader {
            fn clone(&self) -> Self {
                Self {
                    msgpack_bytes: self.msgpack_bytes.clone(),
                    total_rows: self.total_rows,
                    format_version: self.format_version,
                    data_pos: self.data_pos,
                    cached_chunk: None,
                    cached_row_start: self.cached_row_start,
                    predecoded: self.predecoded.clone(),
                }
            }
        }

        impl Default for $reader {
            fn default() -> Self {
                Self {
                    msgpack_bytes: std::sync::Arc::new([]),
                    total_rows: 0,
                    format_version: self::FORMAT_VERSION_CHUNKED,
                    data_pos: 0,
                    cached_chunk: None,
                    cached_row_start: 0,
                    predecoded: None,
                }
            }
        }

        impl super::PcoSerde for $ty {
            type Writer = $writer_ty;
            type Reader = $reader;

            const CHEAP_TO_CLONE: bool = $cheap_to_clone;

            fn write(data: Vec<Self>, _float_round: u32, _time_round: chrono::Duration) -> anyhow::Result<Vec<u8>> {
                use self::{CHUNK_SIZE, FORMAT_VERSION_CHUNKED};
                let len = data.len();
                let mut raw_buffer = Vec::with_capacity(len * 64);
                raw_buffer.push(FORMAT_VERSION_CHUNKED);
                for chunk in data.chunks(CHUNK_SIZE) {
                    rmp_serde::encode::write(&mut raw_buffer, chunk)?;
                }
                let compressed = zstd::encode_all(raw_buffer.as_slice(), 3)?;
                let mut out = Vec::with_capacity(16 + compressed.len());
                out.extend_from_slice(&(len as u64).to_le_bytes());
                out.extend_from_slice(&(compressed.len() as u64).to_le_bytes());
                out.extend_from_slice(&compressed);
                Ok(out)
            }

            /// Encodes `&[&Self]` chunks directly; serde forwards through
            /// references, so the msgpack bytes match owned `write`.
            fn write_borrowed(
                values: &[&Self], _float_round: u32, _time_round: chrono::Duration,
            ) -> anyhow::Result<Vec<u8>> {
                use self::{CHUNK_SIZE, FORMAT_VERSION_CHUNKED};
                let len = values.len();
                let mut raw_buffer = Vec::with_capacity(len * 64);
                raw_buffer.push(FORMAT_VERSION_CHUNKED);
                for chunk in values.chunks(CHUNK_SIZE) {
                    rmp_serde::encode::write(&mut raw_buffer, chunk)?;
                }
                let compressed = zstd::encode_all(raw_buffer.as_slice(), 3)?;
                let mut out = Vec::with_capacity(16 + compressed.len());
                out.extend_from_slice(&(len as u64).to_le_bytes());
                out.extend_from_slice(&(compressed.len() as u64).to_le_bytes());
                out.extend_from_slice(&compressed);
                Ok(out)
            }

            fn read(
                src: &mut std::io::Cursor<&[u8]>, _float_round: u32, _time_round: chrono::Duration,
            ) -> anyhow::Result<Self::Reader> {
                use self::FORMAT_VERSION_CHUNKED;
                let saved_pos = src.position();
                let buf = src.get_ref();
                if saved_pos as usize >= buf.len() || buf.is_empty() {
                    return Ok(Self::Reader {
                        msgpack_bytes: std::sync::Arc::new([]),
                        total_rows: 0,
                        format_version: FORMAT_VERSION_CHUNKED,
                        data_pos: 0,
                        cached_chunk: None,
                        cached_row_start: 0,
                        predecoded: None,
                    });
                }
                let remaining = &buf[saved_pos as usize..];
                let buf_len = buf.len();
                // Try new format: [u64 LE total_rows][u64 LE compressed_len][zstd payload]
                if remaining.len() >= 16 {
                    let total_rows = u64::from_le_bytes((&remaining[0..8]).try_into().unwrap()) as usize;
                    let block_len = u64::from_le_bytes((&remaining[8..16]).try_into().unwrap()) as usize;
                    let compressed_start = 16usize;
                    if compressed_start + block_len <= remaining.len() {
                        let compressed = &remaining[compressed_start..compressed_start + block_len];
                        match zstd::decode_all(Cursor::new(compressed)) {
                            Ok(msgpack_bytes) => {
                                src.set_position(saved_pos + compressed_start as u64 + block_len as u64);
                                let fmt_ver =
                                    if msgpack_bytes.is_empty() { FORMAT_VERSION_CHUNKED } else { msgpack_bytes[0] };
                                return Ok(Self::Reader {
                                    msgpack_bytes: msgpack_bytes.into(),
                                    total_rows,
                                    format_version: fmt_ver,
                                    data_pos: 0,
                                    cached_chunk: None::<Vec<$ty>>,
                                    cached_row_start: 0,
                                    predecoded: None,
                                });
                            }
                            Err(_) => {} // Decompress failed, try fallback
                        }
                    }
                }
                if let Ok(reader) = <Self::Reader as FallbackReader>::read_fallback(remaining) {
                    src.set_position(buf_len as u64);
                    return Ok(reader);
                }
                Err(anyhow::anyhow!(
                    "failed to deserialize {} column: new format and fallback format both failed",
                    stringify!($ty)
                ))
            }

            fn reader_len(reader: &mut Self::Reader) -> anyhow::Result<usize> {
                Ok(reader.total_rows)
            }

            /// Get an item by index. Scans forward-only through chunks; retains the current
            /// deserialized chunk in the reader cache until index advances past that range.
            fn get(reader: &mut Self::Reader, index: usize) -> anyhow::Result<Option<Self>> {
                // Fallback readers keep all values pre-decoded; read them directly.
                if let Some(ref values) = reader.predecoded {
                    return Ok(values.get(index).cloned());
                }
                use self::{CHUNK_SIZE, FORMAT_VERSION_CHUNKED, helpers};
                let bytes = &reader.msgpack_bytes;
                if index >= reader.total_rows || bytes.is_empty() {
                    return Ok(None);
                }
                // First byte is format version.
                let fmt_ver = bytes[0];
                match fmt_ver {
                    FORMAT_VERSION_CHUNKED => {
                        let data_start = &bytes[1..];
                        // If index falls within our cached chunk, return directly (O(1) hit).
                        if let Some(ref cache) = reader.cached_chunk {
                            let end_row_excl = reader.cached_row_start + cache.len();
                            if index >= reader.cached_row_start && index < end_row_excl {
                                return Ok(cache.get(index - reader.cached_row_start).cloned());
                            }
                        }
                        // Need to advance forward from current position. Track row offset relative to start.
                        let mut local_pos = reader.data_pos;
                        let mut current_row_offset: usize = match &reader.cached_chunk {
                            Some(cache) => reader.cached_row_start + cache.len(),
                            None => 0,
                        };
                        loop {
                            if local_pos >= data_start.len() || current_row_offset >= reader.total_rows {
                                break; // Ran out of data.
                            }
                            let chunk_end_excl = std::cmp::min(current_row_offset + CHUNK_SIZE, reader.total_rows);
                            if index < chunk_end_excl {
                                match helpers::deserialize_chunk::<Self>(data_start, &mut local_pos) {
                                    Some(chunk) => {
                                        let value = chunk.get(index - current_row_offset).cloned();
                                        reader.cached_chunk = Some(chunk);
                                        reader.cached_row_start = current_row_offset;
                                        reader.data_pos = local_pos;
                                        return Ok(value);
                                    }
                                    None => break, // Deserialization failed.
                                }
                            }
                            if helpers::skip_chunk(data_start, &mut local_pos).is_none() {
                                break;
                            }
                            let skipped_count = std::cmp::min(CHUNK_SIZE, reader.total_rows - current_row_offset);
                            current_row_offset += skipped_count;
                        }
                        return Ok(None);
                    }
                    _ => {
                        // Flat msgpack format from read_fallback. No version byte prefix; items are serialized sequentially.
                        if index == 0 && !bytes.is_empty() {
                            return Ok(helpers::deserialize_one::<Self>(bytes, &mut 0));
                        }
                        Ok(None)
                    }
                }
            }

            /// Chunk-major survivor gather: text is the dominant cost in the
            /// row-at-a-time fallback (`cache.get().clone()` per survivor),
            /// so this skips untouched chunks outright and moves survivors out
            /// of each deserialized chunk instead of cloning them. A corrupt
            /// or truncated chunk stream returns `Err`; rows past `total_rows`
            /// still answer `Default`.
            fn gather(reader: &mut Self::Reader, mask: &FilterMask, out: &mut Vec<Self>) -> anyhow::Result<()>
            where
                Self: Default,
            {
                // Fallback readers keep every row decoded; clone per
                // survivor, matching `get` byte for byte.
                if let Some(values) = reader.predecoded.clone() {
                    for_each_set_bit(mask, |i| {
                        out.push(values.get(i).cloned().unwrap_or_default());
                        Ok(())
                    })?;
                    return Ok(());
                }
                if reader.format_version != self::FORMAT_VERSION_CHUNKED || reader.msgpack_bytes.is_empty() {
                    return super::gather_via_get::<$ty>(reader, mask, out);
                }
                let total_rows = reader.total_rows;
                let cached = reader.cached_chunk.take();
                let cached_row_start = reader.cached_row_start;
                let data_pos = reader.data_pos;
                let msgpack_bytes = reader.msgpack_bytes.clone();
                let words = mask.gather_words();
                helpers::gather_chunked(
                    &msgpack_bytes[1..],
                    total_rows,
                    cached,
                    cached_row_start,
                    data_pos,
                    &words,
                    out,
                )?;
                // Rewind the cursor to its freshly-read state: survivors were
                // moved out of the drained chunks, so leaving a stale `pos`
                // without a cache would make a later `get` mis-align row
                // offsets against mid-stream bytes. From `(None, 0, 0)` the
                // forward scan re-reads every row correctly.
                reader.cached_chunk = None;
                reader.cached_row_start = 0;
                reader.data_pos = 0;
                Ok(())
            }
        }

        impl $reader {
            /// Chunk-major filter over a whole-frame blob: chunks whose mask
            /// range is already all-zero are skipped without deserialization.
            /// Text eq/IN predicates additionally scan the inflated msgpack
            /// bytes in place (`helpers::scan_chunk`): non-matching rows are
            /// never deserialized, so the inflate pays length checks and
            /// memcmps instead of a `String` allocation per row. Any chunk the
            /// scan can't interpret falls back to deserialize-and-predicate,
            /// byte-for-byte.
            pub(crate) fn filter_bulk<F>(
                &self, filter: &Filter, matches: &mut crate::FilterMask, mut pred: F,
            ) -> anyhow::Result<()>
            where
                F: FnMut(&$ty, &Filter) -> bool,
            {
                // Legacy fallback data is fully decoded; apply the predicate
                // directly.
                if let Some(ref values) = self.predecoded {
                    matches.build_into(0, values.as_ref(), |val| pred(val, filter));
                    return Ok(());
                }
                if self.total_rows == 0 || self.msgpack_bytes.is_empty() {
                    matches.handle_empty_column::<$ty, _>(|val| pred(val, filter));
                    return Ok(());
                }
                let targets = text_targets(filter);
                // Skip format_version byte, iterate chunks sequentially.
                let bytes = &self.msgpack_bytes[1..];
                let mut pos: usize = 0;
                let mut row_idx: usize = 0;
                while row_idx < self.total_rows && pos < bytes.len() {
                    // Skip chunks whose rows are already eliminated.
                    let chunk_end = (row_idx + self::CHUNK_SIZE).min(self.total_rows);
                    if !matches.any_set_in_range(row_idx..chunk_end) {
                        if helpers::skip_chunk(bytes, &mut pos).is_none() {
                            break;
                        }
                        row_idx = chunk_end;
                        continue;
                    }
                    // Raw-byte scan first: a match lands its predicate word
                    // straight into the mask. A decline leaves `pos` untouched
                    // so the deserialize path below re-reads the whole chunk.
                    if let Some(targets) = &targets
                        && let Some(rows) = helpers::scan_chunk(bytes, &mut pos, targets, row_idx, matches)
                    {
                        row_idx += rows;
                        continue;
                    }
                    if let Some(chunk) = helpers::deserialize_chunk::<$ty>(bytes, &mut pos) {
                        matches.build_into(row_idx, &chunk, |val| pred(val, filter));
                        row_idx += chunk.len();
                    } else {
                        break;
                    }
                }
                Ok(())
            }

            /// Deserialize one item starting at `pos` from a flat (non-chunked) msgpack byte stream.
            #[inline]
            #[allow(dead_code)]
            pub(crate) fn deserialize_one(bytes: &[u8], pos: &mut usize) -> Option<$ty> {
                self::helpers::deserialize_one::<$ty>(bytes, pos)
            }

            /// Deserialize a full chunk (Vec<T>) starting at `pos`. Expected to be a msgpack array.
            #[inline]
            #[allow(dead_code)]
            pub(crate) fn deserialize_chunk(bytes: &[u8], pos: &mut usize) -> Option<Vec<$ty>> {
                self::helpers::deserialize_chunk::<$ty>(bytes, pos)
            }

            /// Get item at `offset_in_chunk` from the current chunk in bytes[pos..], deserializing the whole chunk.
            #[inline]
            #[allow(dead_code)]
            pub(crate) fn get_from_chunk(bytes: &[u8], pos: &mut usize, offset_in_chunk: usize) -> Option<$ty> {
                self::helpers::get_from_chunk::<$ty>(bytes, pos, offset_in_chunk)
            }

            /// Skip an entire msgpack array chunk by deserializing it as IgnoredAny, advancing pos without allocation.
            #[inline]
            #[allow(dead_code)]
            pub(crate) fn skip_chunk(bytes: &[u8], pos: &mut usize) -> Option<()> {
                self::helpers::skip_chunk(bytes, pos)
            }
        }
    };
}
