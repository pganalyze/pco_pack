use super::*;

pub struct OptionReader<T: PcoSerde>
where
    T::Reader: Clone + Default,
{
    /// Indices where `Some` values are present.
    pub presence_indices: Arc<[u32]>,
    /// Total number of rows (Some + None) in this column.
    pub total_rows: usize,
    /// Reader for only the present values.
    pub inner_reader: T::Reader,
    /// Current position in the full row space.
    pub current_idx: usize,
    /// Current position in `presence_indices` / inner data.
    pub presence_idx: usize,
}

impl<T: PcoSerde> Default for OptionReader<T>
where
    T::Reader: Clone + Default,
{
    fn default() -> Self {
        Self {
            presence_indices: Arc::new([]),
            total_rows: 0,
            inner_reader: T::Reader::default(),
            current_idx: 0,
            presence_idx: 0,
        }
    }
}

impl<T: PcoSerde> Clone for OptionReader<T>
where
    T::Reader: Clone + Default,
{
    fn clone(&self) -> Self {
        Self {
            presence_indices: self.presence_indices.clone(),
            total_rows: self.total_rows,
            inner_reader: self.inner_reader.clone(),
            current_idx: self.current_idx,
            presence_idx: self.presence_idx,
        }
    }
}

impl<T: PcoSerde> PcoSerde for Option<T>
where
    T::Reader: Clone + Default,
{
    type Writer = ();
    type Reader = OptionReader<T>;

    fn write(data: Vec<Option<T>>, float_round: u32, time_round: chrono::Duration) -> Result<Vec<u8>> {
        let total_rows = data.len();
        let mut presence_indices = Vec::with_capacity(data.len());
        let mut inner_values = Vec::with_capacity(data.len());
        for (i, v) in data.into_iter().enumerate() {
            if let Some(value) = v {
                presence_indices.push(i as u32);
                inner_values.push(value);
            }
        }
        let config = pco_util::config();
        let presence_bytes = pco::standalone::simple_compress::<u32>(&presence_indices, &config)?;
        let mut out = Vec::new();
        out.extend_from_slice(&(total_rows as u64).to_le_bytes());
        out.extend_from_slice(&presence_bytes);
        let inner_bytes = T::write(inner_values, float_round, time_round)?;
        out.extend_from_slice(&inner_bytes);
        Ok(out)
    }

    fn read(src: &mut Cursor<&[u8]>, float_round: u32, time_round: chrono::Duration) -> Result<Self::Reader> {
        let mut buf = [0u8; 8];
        src.read_exact(&mut buf)?;
        let total_rows = u64::from_le_bytes(buf) as usize;
        let presence_indices: Vec<u32> = pco_util::decompress(src)?;
        let inner_reader =
            if presence_indices.is_empty() { T::Reader::default() } else { T::read(src, float_round, time_round)? };
        Ok(OptionReader {
            presence_indices: presence_indices.into(),
            total_rows,
            inner_reader,
            current_idx: 0,
            presence_idx: 0,
        })
    }

    fn validate_bounds(reader: &mut Self::Reader) -> Result<Option<usize>> {
        Ok(Some(reader.total_rows))
    }

    fn get(reader: &mut Self::Reader, index: usize) -> Result<Option<Self>> {
        if index >= reader.total_rows {
            return Ok(None);
        }
        // First, advance past any presence entries that are strictly less than this index.
        // This positions presence_idx at the first entry that could match `index`.
        while let Some(&presence) = reader.presence_indices.get(reader.presence_idx) {
            if presence < index as u32 {
                reader.presence_idx += 1;
            } else {
                break;
            }
        }
        // Now check if the current presence entry matches this index.
        if let Some(&presence) = reader.presence_indices.get(reader.presence_idx) {
            if presence == index as u32 {
                let inner_value = T::get(&mut reader.inner_reader, reader.presence_idx)?;
                reader.presence_idx += 1;
                reader.current_idx = index + 1;
                return Ok(Some(inner_value));
            }
        }
        // This row is None.
        reader.current_idx = index + 1;
        Ok(Some(None))
    }
}

impl<T> PcoFilter for Option<T>
where
    T: PcoFilter,
    T::Reader: Clone + Default,
{
    fn filter_bulk(reader: &mut Self::Reader, field: usize, filter: &Filter, matches: &mut FilterMask) -> Result<()> {
        let total_rows = reader.total_rows;
        let inner_count = reader.presence_indices.len();
        // Build a mask over the compressed inner data (only Some values)
        let mut inner_matches = FilterMask::ones(inner_count);
        T::filter_bulk(&mut reader.inner_reader, field, filter, &mut inner_matches)?;
        // Map inner results back to original row positions
        let mut matches_vec = vec![false; total_rows];
        for (pi, &orig_idx) in reader.presence_indices.iter().enumerate() {
            if inner_matches.as_bitslice()[pi as usize] {
                matches_vec[orig_idx as usize] = true;
            }
        }
        matches.and_with(&FilterMask::from_bool_slice(&matches_vec));
        Ok(())
    }

    fn filter_match(value: &Self, filter: &Filter) -> bool {
        match value {
            Some(inner) => T::filter_match(inner, filter),
            None => false,
        }
    }

    fn filter_nested(
        reader: &mut Self::Reader, path: &[usize], filter: &Filter, matches: &mut FilterMask,
    ) -> Result<()> {
        let total_rows = reader.total_rows;
        let inner_count = reader.presence_indices.len();
        let mut inner_matches = FilterMask::ones(inner_count);
        T::filter_nested(&mut reader.inner_reader, path, filter, &mut inner_matches)?;
        // Map inner results back to original row positions
        let mut matches_vec = vec![false; total_rows];
        for (pi, &orig_idx) in reader.presence_indices.iter().enumerate() {
            if inner_matches.as_bitslice()[pi as usize] {
                matches_vec[orig_idx as usize] = true;
            }
        }
        matches.and_with(&FilterMask::from_bool_slice(&matches_vec));
        Ok(())
    }

    fn resolve_filter(path: &str, json: &serde_json::Value) -> Result<ResolvedFilter> {
        T::resolve_filter(path, json)
    }
}
