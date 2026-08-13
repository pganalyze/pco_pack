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

impl<T: PcoSerde + PcoFilter> VecPackable for T
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
        out.extend_from_slice(&cl);
        let all_data: Vec<T> = writer.raw_data.into_iter().flatten().collect();
        let inner_data = T::write(all_data, float_round, time_round)?;
        out.extend_from_slice(&inner_data);
        Ok(())
    }

    fn read_vec(
        src: &mut Cursor<&[u8]>, float_round: u32, time_round: chrono::Duration,
    ) -> anyhow::Result<Self::VecReader> {
        let lengths: Vec<u32> = pco_util::decompress(src)?;
        let inner_reader = T::read(src, float_round, time_round)?;
        Ok(VecNumberReader { lengths: lengths.into(), length_idx: 0, cached_offset: 0, inner_reader })
    }

    fn validate_vec_bounds(reader: &Self::VecReader) -> Result<Option<usize>> {
        Ok(Some(reader.lengths.len()))
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

    fn filter_vec(reader: &Self::VecReader, filter: &Filter, matches: &mut FilterMask) -> Result<()> {
        let lengths = &reader.lengths;
        // Clone to avoid advancing the original reader's cursor.
        let mut elem_rd = reader.inner_reader.clone();
        let mut offset: usize = 0;
        let mut matches_vec: Vec<bool> = Vec::with_capacity(lengths.len());
        for (_, &len) in lengths.iter().enumerate() {
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
        for (_, &len) in lengths.iter().enumerate() {
            let row_len = len as usize;
            let slice = flat_mb.get_range(offset..offset + row_len);
            matches_vec.push(slice.any());
            offset += row_len;
        }
        matches.and_with(&FilterMask::from_bool_slice(&matches_vec));
        Ok(())
    }
}
