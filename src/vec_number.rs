use super::*;

pub struct VecNumberWriter<T> {
    pub lengths: Vec<u32>,
    pub raw_data: Vec<Vec<T>>,
    pub _marker: PhantomData<T>,
}

impl<T> Default for VecNumberWriter<T> {
    fn default() -> Self {
        Self { lengths: Vec::new(), raw_data: Vec::new(), _marker: PhantomData }
    }
}

#[derive(Clone)]
pub struct VecNumberReader<T: PcoSerde>
where
    T::Reader: Clone + Default,
{
    pub lengths: Arc<[u32]>,
    pub length_idx: usize,
    pub inner_reader: T::Reader,
    pub cached_offset: usize,
}

impl<T: PcoSerde> Default for VecNumberReader<T>
where
    T::Reader: Clone + Default,
{
    fn default() -> Self {
        Self { lengths: Arc::new([]), length_idx: 0, inner_reader: T::Reader::default(), cached_offset: 0 }
    }
}

impl<T: PcoSerde + PcoFilter + Default> VecPackable for T
where
    T::Writer: Default,
    T::Reader: Clone + Default,
{
    type VecWriter = VecNumberWriter<T>;
    type VecReader = VecNumberReader<T>;

    fn pack_vec(data: Vec<T>, writer: &mut Self::VecWriter) {
        writer.lengths.push(data.len() as u32);
        writer.raw_data.push(data);
    }

    fn write_vec(
        writer: Self::VecWriter, float_round: u32, time_round: chrono::Duration, out: &mut Vec<u8>,
    ) -> anyhow::Result<()> {
        let config = pco_util::config();
        let cl = pco::standalone::simple_compress::<u32>(&writer.lengths, &config)?;
        let all_data: Vec<T> = writer.raw_data.into_iter().flatten().collect();
        let (header, body) = bounds::split_header(T::write(all_data, float_round, time_round)?);
        out.reserve(bounds::BOUNDS_PREFIX_LEN + cl.len() + body.len());
        out.extend_from_slice(&header);
        out.extend_from_slice(&cl);
        out.extend_from_slice(&body);
        Ok(())
    }

    fn read_vec(
        src: &mut Cursor<&[u8]>, float_round: u32, time_round: chrono::Duration,
    ) -> anyhow::Result<Self::VecReader> {
        let buf = *src.get_ref();
        src.set_position(bounds::strip_prefix(buf, src.position() as usize) as u64);
        let lengths: Vec<u32> = pco_util::decompress(src)?;
        let inner_reader = if (src.position() as usize) < src.get_ref().len() {
            T::read(src, float_round, time_round)?
        } else {
            T::Reader::default()
        };
        Ok(VecNumberReader { lengths: lengths.into(), length_idx: 0, cached_offset: 0, inner_reader })
    }

    fn reader_len(reader: &Self::VecReader) -> Result<usize> {
        Ok(reader.lengths.len())
    }

    fn get_vec(reader: &mut Self::VecReader, index: usize) -> Result<Option<Vec<T>>> {
        let target_len = reader.lengths.get(index).copied().context("length missing")? as usize;
        let mut offset = reader.cached_offset;
        for i in reader.length_idx..index {
            offset += reader.lengths[i] as usize;
        }
        reader.cached_offset = offset + target_len;
        reader.length_idx = index + 1;
        let mut row = Vec::with_capacity(target_len);
        for i in 0..target_len {
            row.push(T::get(&mut reader.inner_reader, offset + i)?.context("inner missing")?);
        }
        Ok(Some(row))
    }

    /// Chunk-major survivor rows: project every surviving row's inner span
    /// `[offset, offset + len)` onto the flat element stream, gather all of
    /// them in one inner pass, then move the owned elements into rows.
    ///
    /// The scan clamps to the caller's mask word span, which covers every
    /// row below `mask.len()`: rows past the last survivor but below that
    /// length select nothing, and the cursor advances past them anyway.
    /// Surviving rows the forward cursor already passed read as empty rows;
    /// that is the documented gather leniency for out-of-contract cursor
    /// reuse, where `get_vec` behind a stuck cursor mis-slices or errors.
    fn gather_vec(reader: &mut Self::VecReader, mask: &FilterMask, out: &mut Vec<Vec<T>>) -> Result<()> {
        let mask_words = mask.gather_words();
        let rows = reader.lengths.len().min(mask_words.len() * 64);
        let mut inner_mask: Vec<u64> = Vec::new();
        // (row, start, len) per surviving row, ascending.
        let mut spans: Vec<(usize, usize, usize)> = Vec::new();
        let mut offset = reader.cached_offset;
        for i in reader.length_idx..rows {
            let len = reader.lengths[i] as usize;
            if (mask_words[i >> 6] >> (i & 63)) & 1 == 1 {
                crate::set_bits(&mut inner_mask, offset, len);
                spans.push((i, offset, len));
            }
            offset += len;
        }
        reader.cached_offset = offset;
        reader.length_idx = rows;
        let mut inner: Vec<T> = Vec::new();
        if !inner_mask.is_empty() {
            let inner_mask = FilterMask::from_raw_words(inner_mask);
            T::gather(&mut reader.inner_reader, &inner_mask, &mut inner)?;
        }
        let mut it = inner.into_iter();
        let mut next = spans.into_iter();
        let mut pending = next.next();
        for_each_set_bit(mask, |row| match pending {
            Some((span_row, _, len)) if span_row == row => {
                out.push(it.by_ref().take(len).collect());
                pending = next.next();
            }
            Some(_) | None => {
                // Surviving rows before the forward cursor, and rows past
                // the lengths column, both read empty: leniency for
                // out-of-contract reads, not parity with `get_vec`.
                out.push(Vec::new());
            }
        });
        Ok(())
    }

    fn filter_vec(reader: &Self::VecReader, filter: &Filter, matches: &mut FilterMask) -> Result<()> {
        let lengths = &reader.lengths;
        // Clone to avoid advancing the original reader's cursor.
        let mut elem_rd = reader.inner_reader.clone();
        let mut offset: usize = 0;
        let mut matches_vec: Vec<bool> = Vec::with_capacity(lengths.len());
        for &len in lengths.iter() {
            let row_len = len as usize;
            let mut matched = false;
            for i in 0..row_len {
                if let Some(val) = T::get(&mut elem_rd, offset + i)?
                    && T::filter_match(&val, filter)
                {
                    matched = true;
                    break;
                }
            }
            matches_vec.push(matched);
            offset += row_len;
        }
        matches.and_with(&FilterMask::from_bool_slice(&matches_vec));
        Ok(())
    }

    fn filter_vec_match(value: &Self, filter: &Filter) -> bool {
        T::filter_match(value, filter)
    }

    fn filter_vec_nested(
        reader: &mut Self::VecReader, path: &[usize], filter: &Filter, matches: &mut FilterMask,
    ) -> Result<()> {
        let lengths = &reader.lengths;
        let total_elements: usize = lengths.iter().map(|&l| l as usize).sum();
        let mut flat_mb = FilterMask::ones(total_elements);
        T::filter_nested(&mut reader.inner_reader, path, filter, &mut flat_mb)?;
        // Aggregate per-element matches into per-row (row matches if any element matched)
        let mut offset: usize = 0;
        let mut matches_vec: Vec<bool> = Vec::with_capacity(lengths.len());
        for &len in lengths.iter() {
            let row_len = len as usize;
            let slice = flat_mb.get_range(offset..offset + row_len);
            matches_vec.push(slice.any());
            offset += row_len;
        }
        matches.and_with(&FilterMask::from_bool_slice(&matches_vec));
        Ok(())
    }
}
