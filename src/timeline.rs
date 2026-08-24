use super::*;

/// Represents a series of non-overlapping, sorted time ranges.
///
/// Ranges are `(start, end)` pairs (inclusive, microseconds since epoch).
/// The const generic `RESOLUTION` controls how timestamps are bucketed.
///
/// - `RESOLUTION = 0` (default): exact mode. Store ranges as-is, merging overlaps.
/// - `RESOLUTION > 0`: bucket mode. Each added range is floored to the nearest bucket
///   boundary, and recorded as the full bucket window `[bucket_start, bucket_start + RESOLUTION)`.
///   Adjacent or overlapping buckets are merged. Original timestamps within a bucket are discarded.
///
/// Bucket mode is useful for de-noising timestamps or aggregating events into time windows.
///
/// # Example (exact mode)
/// ```
/// use pco_pack::Timeline;
///
/// let mut tl = Timeline::<0>::new();
/// tl.add(1000, 5000);
/// tl.add(4000, 8000); // overlaps -> merges
/// assert_eq!(tl.ranges(), &[(1000i64, 8000i64)]);
/// ```
///
/// # Example (bucket mode, 10-second resolution)
/// ```
/// use pco_pack::Timeline;
///
/// let mut tl = Timeline::<10_000_000>::new(); // 10 seconds (in microseconds)
/// tl.add(3_000_000, 5_000_000);   // 3-5 seconds -> bucket [0, 10M)
/// tl.add(7_000_000, 9_000_000);   // 7-9 seconds -> same bucket -> no new range
/// tl.add(15_000_000, 18_000_000); // 15-18 seconds -> bucket [10M, 20M), adjacent -> merges
/// assert_eq!(tl.ranges(), &[(0i64, 20_000_000i64)]);
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Timeline<const RESOLUTION: i64 = 0> {
    ranges: Vec<(i64, i64)>,
}

impl<const RESOLUTION: i64> Timeline<RESOLUTION> {
    pub fn new() -> Self {
        Self { ranges: Vec::new() }
    }

    /// Create a Timeline from a single range, applying the type's resolution.
    pub fn from_range(start: i64, end: i64) -> Self {
        let mut tl = Self::new();
        tl.add(start, end);
        tl
    }

    pub fn ranges(&self) -> &[(i64, i64)] {
        &self.ranges
    }

    pub fn start(&self) -> Option<i64> {
        self.ranges.first().map(|r| r.0)
    }

    pub fn end(&self) -> Option<i64> {
        self.ranges.last().map(|r| r.1)
    }

    pub fn span(&self) -> i64 {
        match self.ranges.first() {
            Some(first_start) => match self.ranges.last() {
                Some(last_end) => last_end.1.saturating_sub(first_start.0),
                None => 0,
            },
            None => 0,
        }
    }

    pub fn len(&self) -> usize {
        self.ranges.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }

    pub fn contains(&self, ts: i64) -> bool {
        self.ranges.iter().any(|&(s, e)| s <= ts && ts <= e)
    }

    pub fn overlaps(&self, start: i64, end: i64) -> bool {
        self.ranges.iter().any(|&(s, e)| s <= end && start <= e)
    }

    /// Add a time range, applying the type's resolution.
    ///
    /// In bucket mode (`RESOLUTION > 0`), the start is floored to the bucket boundary
    /// and the range becomes `[bucket_start, bucket_start + RESOLUTION)`.
    /// Adjacent and overlapping buckets are merged automatically.
    ///
    /// If `start > end`, the call is a no-op.
    pub fn add(&mut self, start: i64, end: i64) {
        if start > end {
            return;
        }

        let (bucket_start, bucket_end) = if RESOLUTION > 0 {
            let b = Self::bucket_start(start, RESOLUTION);
            (b, b + RESOLUTION)
        } else {
            (start, end)
        };

        self.merge_range(bucket_start, bucket_end);
    }

    /// Extend this Timeline with ranges that are already bucket-aligned.
    ///
    /// This skips the bucketing step and is used internally when merging
    /// already-processed Timeline ranges (e.g., during serialization grouping).
    /// For user code, prefer `add()` which applies resolution automatically.
    pub fn extend_ranges(&mut self, ranges: &[(i64, i64)]) {
        for &(s, e) in ranges {
            if s <= e {
                self.merge_range(s, e);
            }
        }
    }

    /// Floor a timestamp to the nearest bucket boundary.
    /// Uses floor division so negative timestamps are handled correctly.
    fn bucket_start(ts: i64, resolution: i64) -> i64 {
        if ts >= 0 {
            (ts / resolution) * resolution
        } else {
            // Floor division for negative numbers
            ((ts - resolution + 1) / resolution) * resolution
        }
    }

    /// Merge a range into the existing set, keeping ranges sorted and non-overlapping.
    fn merge_range(&mut self, mut start: i64, mut end: i64) {
        // Find first range that could overlap or be adjacent.
        // A range (s, e) overlaps/adjacent with (start, end) when s <= end + 1 && start <= e + 1.
        let idx = self.ranges.binary_search_by(|&(_, r_end)| r_end.cmp(&(start - 1)));
        let i = idx.unwrap_or_else(|e| e);

        // Merge all overlapping/adjacent ranges.
        let mut j = i;
        while j < self.ranges.len() && self.ranges[j].0 <= end + 1 {
            start = start.min(self.ranges[j].0);
            end = end.max(self.ranges[j].1);
            j += 1;
        }

        self.ranges.drain(i..j);
        self.ranges.insert(i, (start, end));
    }

    pub fn merge(&mut self, input: &Self) {
        if self.ranges.is_empty() {
            self.ranges.extend_from_slice(&input.ranges);
        } else {
            for &(start, end) in &input.ranges {
                self.add(start, end);
            }
        }
    }
}

impl<const RESOLUTION: i64> PcoSerde for Timeline<RESOLUTION> {
    type Writer = vec_number::VecNumberWriter<(i64, i64)>;
    type Reader = vec_number::VecNumberReader<(i64, i64)>;

    fn write(data: Vec<Self>, float_round: u32, time_round: chrono::Duration) -> anyhow::Result<Vec<u8>> {
        let inner: Vec<Vec<(i64, i64)>> = data.into_iter().map(|tl| tl.ranges).collect();
        Vec::<(i64, i64)>::write(inner, float_round, time_round)
    }

    fn read(src: &mut Cursor<&[u8]>, float_round: u32, time_round: chrono::Duration) -> anyhow::Result<Self::Reader> {
        Vec::<(i64, i64)>::read(src, float_round, time_round)
    }

    fn validate_bounds(reader: &mut Self::Reader) -> Result<Option<usize>> {
        Ok(Some(reader.lengths.len()))
    }

    fn get(reader: &mut Self::Reader, index: usize) -> Result<Option<Self>> {
        let target_len = reader.lengths.get(index).copied().context("length missing")? as usize;
        let mut offset = reader.cached_offset;
        for i in reader.length_idx..index {
            offset += reader.lengths[i] as usize;
        }
        reader.cached_offset = offset + target_len;
        reader.length_idx = index + 1;
        let mut ranges = Vec::with_capacity(target_len);
        for i in 0..target_len {
            let (s, e) = <(i64, i64)>::get(&mut reader.inner_reader, offset + i)?.context("inner missing")?;
            ranges.push((s, e));
        }
        Ok(Some(Timeline { ranges }))
    }
}

impl<const RESOLUTION: i64> PcoFilter for Timeline<RESOLUTION> {
    fn filter_bulk(reader: &mut Self::Reader, _field: usize, filter: &Filter, matches: &mut FilterMask) -> Result<()> {
        let num_rows = reader.lengths.len();
        let mut elem_de = reader.inner_reader.clone();
        let mut offset: usize = 0;
        let mut matches_vec: Vec<bool> = Vec::with_capacity(num_rows);
        for (_, &len) in reader.lengths.iter().enumerate() {
            let row_len = len as usize;
            let mut matched = false;
            for i in 0..row_len {
                if let Ok(Some((s, e))) = <(i64, i64)>::get(&mut elem_de, offset + i) {
                    matched = match filter {
                        Filter::Range(range) => s <= *range.end() && *range.start() <= e,
                        Filter::FloatRange(range) => {
                            let start = *range.start() as i64;
                            let end = *range.end() as i64;
                            s <= end && start <= e
                        }
                        Filter::I64(target) => s <= *target && *target <= e,
                        Filter::InclusionI64(values) => values.iter().any(|v| s <= *v && *v <= e),
                        _ => false,
                    };
                    if matched {
                        break;
                    }
                }
            }
            matches_vec.push(matched);
            offset += row_len;
        }
        matches.and_with(&FilterMask::from_bool_slice(&matches_vec));
        Ok(())
    }

    fn filter_match(value: &Self, filter: &Filter) -> bool {
        match filter {
            Filter::Range(range) => value.ranges.iter().any(|&(s, e)| s <= *range.end() && *range.start() <= e),
            Filter::FloatRange(range) => {
                let start = *range.start() as i64;
                let end = *range.end() as i64;
                value.ranges.iter().any(|&(s, e)| s <= end && start <= e)
            }
            Filter::I64(target) => value.ranges.iter().any(|&(s, e)| s <= *target && *target <= e),
            Filter::InclusionI64(values) => {
                value.ranges.iter().any(|&(s, e)| values.iter().any(|v| s <= *v && *v <= e))
            }
            _ => false,
        }
    }

    fn filter_nested(
        _reader: &mut Self::Reader, _path: &[usize], _filter: &Filter, _matches: &mut FilterMask,
    ) -> Result<()> {
        unreachable!("filter_nested not supported for Timeline")
    }

    fn resolve_filter(path: &str, json: &serde_json::Value) -> Result<ResolvedFilter> {
        if !path.is_empty() {
            return Err(anyhow::anyhow!("Field '{}' is a Timeline leaf and cannot contain nested path", path));
        }
        if let serde_json::Value::Object(obj) = json {
            if let (Some(start_val), Some(end_val)) = (obj.get("start"), obj.get("end")) {
                let start = start_val.as_i64().context("Range start must be an integer")?;
                let end = end_val.as_i64().context("Range end must be an integer")?;
                return Ok(ResolvedFilter { path: vec![0], filter: Filter::Range(start..=end) });
            }
        }
        if let serde_json::Value::Array(arr) = json {
            if !arr.is_empty() {
                let ints: Vec<i64> = arr.iter().filter_map(|v| v.as_i64()).collect();
                if ints.len() == arr.len() {
                    return Ok(ResolvedFilter {
                        path: vec![0],
                        filter: Filter::InclusionI64(ints.into_iter().collect()),
                    });
                }
            }
        }
        let ts = json.as_i64().context("Expected integer microseconds")?;
        Ok(ResolvedFilter { path: vec![0], filter: Filter::I64(ts) })
    }
}
