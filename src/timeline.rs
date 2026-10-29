use super::*;

/// Bucket width for [`Timeline`], specified in whatever unit suits the call site.
///
/// - `Resolution::exact()` or the integer `0`: timestamps are stored as-is.
/// - `u32`/`i32`: whole seconds, the natural unit for collection cadences (10 for
///   a 10-second heartbeat).
/// - `chrono::Duration`: finer than whole seconds, truncated to microseconds.
///
/// There is deliberately no `From<i64>`: i64 is the timestamp unit, and accepting it
/// here would let the old `add(ts, ts)` shape keep compiling after migration while
/// silently bucketing at absurd widths.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Resolution(i64);

impl Resolution {
    /// Exact mode: every stored timestamp is preserved as given.
    pub const fn exact() -> Self {
        Resolution(0)
    }

    /// Whole-second bucket width. Panics on negative seconds.
    pub const fn seconds(s: i64) -> Self {
        assert!(s >= 0, "Resolution must be non-negative");
        Resolution(s * 1_000_000)
    }
}

impl From<u32> for Resolution {
    fn from(seconds: u32) -> Self {
        Resolution(seconds as i64 * 1_000_000)
    }
}

impl From<i32> for Resolution {
    fn from(seconds: i32) -> Self {
        assert!(seconds >= 0, "Resolution must be non-negative");
        Resolution(seconds as i64 * 1_000_000)
    }
}

impl From<chrono::Duration> for Resolution {
    fn from(d: chrono::Duration) -> Self {
        let us = d.num_microseconds().expect("Resolution overflows microseconds");
        assert!(us >= 0, "Resolution must be non-negative");
        Resolution(us)
    }
}

/// Represents a series of non-overlapping, sorted, inclusive `(start, end)` ranges in
/// microseconds since the epoch.
///
/// Unit convention: timestamps passed in and ranges stored are microseconds since the epoch.
/// The resolution argument is specified in a friendlier unit via [`Resolution`]: a bare
/// integer means whole seconds, a `chrono::Duration` allows sub-second widths, and zero
/// (in any spelling) means exact mode.
///
/// - Exact mode: ranges are stored as-is, merging overlaps.
/// - Bucket mode: each added timestamp or range is floored to bucket boundaries and recorded
///   as inclusive windows `[bucket_start, bucket_start + resolution - 1]` covering every
///   overlapped bucket. Adjacent or overlapping buckets are merged.
///
/// Incoming ranges merged via [`merge`](Self::merge) are never re-bucketed; they keep the
/// coverage their source recorded. Bucket mode is useful for de-noising timestamps or
/// aggregating events into time windows.
///
/// # Example (exact mode)
/// ```
/// use pco_pack::{Resolution, Timeline};
///
/// let mut tl = Timeline::default();
/// tl.add_range(Resolution::exact(), 1000, 5000);
/// tl.add_range(Resolution::exact(), 4000, 8000); // overlaps -> merges
/// assert_eq!(tl.ranges(), &[(1000i64, 8000i64)]);
/// ```
///
/// # Example (bucket mode, 10-second resolution)
/// ```
/// use pco_pack::Timeline;
///
/// let mut tl = Timeline::default();
/// tl.add(10, 3_000_000); // 3s -> bucket [0, 10M)
/// tl.add(10, 7_000_000); // same bucket -> no new range
/// tl.add(10, 15_000_000); // bucket [10M, 20M), adjacent -> merges
/// assert_eq!(tl.ranges(), &[(0i64, 19_999_999)]);
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Timeline {
    ranges: Vec<(i64, i64)>,
}

impl Timeline {
    /// Create a Timeline with a single observation at `ts_us`, bucketed to `resolution`.
    pub fn new(resolution: impl Into<Resolution>, ts_us: i64) -> Self {
        let mut tl = Self::default();
        tl.add(resolution, ts_us);
        tl
    }

    /// Create a Timeline with a single observation from a DateTime, bucketed to `resolution`.
    pub fn new_time(resolution: impl Into<Resolution>, dt: chrono::DateTime<chrono::Utc>) -> Self {
        Self::new(resolution, dt.timestamp_micros())
    }

    /// Create a Timeline from a single exact-mode range, stored as-is.
    pub fn from_range(start_us: i64, end_us: i64) -> Self {
        let mut tl = Self::default();
        tl.add_range(Resolution::exact(), start_us, end_us);
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
        match (self.ranges.first(), self.ranges.last()) {
            (Some(first), Some(last)) => last.1.saturating_sub(first.0).saturating_add(1),
            _ => 0,
        }
    }

    pub fn len(&self) -> usize {
        self.ranges.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }

    pub fn contains(&self, ts_us: i64) -> bool {
        self.ranges.iter().any(|&(s, e)| s <= ts_us && ts_us <= e)
    }

    pub fn overlaps(&self, start_us: i64, end_us: i64) -> bool {
        self.ranges.iter().any(|&(s, e)| s <= end_us && start_us <= e)
    }

    /// Add a single observation at `ts_us`, bucketed to `resolution`. Exact resolutions
    /// store the instant as-is; bucket resolutions record the inclusive window
    /// `[bucket_start, bucket_start + resolution - 1]`.
    pub fn add(&mut self, resolution: impl Into<Resolution>, ts_us: i64) {
        let Resolution(res_us) = resolution.into();
        if res_us == 0 {
            self.merge_range(ts_us, ts_us);
        } else {
            let b = Self::bucket_start(ts_us, res_us);
            self.merge_range(b, b + res_us - 1);
        }
    }

    /// Add a single observation from a DateTime, bucketed to `resolution`.
    pub fn add_datetime(&mut self, resolution: impl Into<Resolution>, dt: chrono::DateTime<chrono::Utc>) {
        self.add(resolution, dt.timestamp_micros());
    }

    /// Add an observation window `[start_us, end_us]`. Exact resolutions store the range
    /// as-is; bucket resolutions fill every bucket the window overlaps, storing the union
    /// inclusive. `start > end` is a no-op.
    pub fn add_range(&mut self, resolution: impl Into<Resolution>, start_us: i64, end_us: i64) {
        if start_us > end_us {
            return;
        }
        let Resolution(res_us) = resolution.into();
        if res_us == 0 {
            self.merge_range(start_us, end_us);
        } else {
            let s = Self::bucket_start(start_us, res_us);
            let e = Self::bucket_start(end_us, res_us);
            self.merge_range(s, e + res_us - 1);
        }
    }

    /// Merge another Timeline's ranges into this one.
    ///
    /// Incoming ranges are already bucket-aligned by the timeline that recorded them, so
    /// they are merged as-is and never re-bucketed.
    pub fn merge(&mut self, input: &Self) {
        if self.ranges.is_empty() {
            self.ranges.extend_from_slice(&input.ranges);
        } else {
            for &(start, end) in &input.ranges {
                if start <= end {
                    self.merge_range(start, end);
                }
            }
        }
    }

    /// Floor a timestamp to the nearest bucket boundary.
    /// Uses floor division so negative timestamps are handled correctly.
    fn bucket_start(ts_us: i64, resolution_us: i64) -> i64 {
        if ts_us >= 0 {
            (ts_us / resolution_us) * resolution_us
        } else {
            // Floor division for negative numbers
            ((ts_us - resolution_us + 1) / resolution_us) * resolution_us
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
}

impl PcoSerde for Timeline {
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

impl PcoFilter for Timeline {
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
                let start = start_val
                    .as_i64()
                    .with_context(|| format!("Range start must be an integer, got {}", json_type_name(start_val)))?;
                let end = end_val
                    .as_i64()
                    .with_context(|| format!("Range end must be an integer, got {}", json_type_name(end_val)))?;
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
        let ts =
            json.as_i64().with_context(|| format!("Expected integer microseconds, got {}", json_type_name(json)))?;
        Ok(ResolvedFilter { path: vec![0], filter: Filter::I64(ts) })
    }
}
