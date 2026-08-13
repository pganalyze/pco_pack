pub struct TimeSeries {
    database_id: i64,
    collected_at: i64,
    value: f64,
}
const _: () = {
    #[allow(unused)]
    use pco_pack::anyhow::Context;
    use pco_pack::serde::Serialize;
    /// Intermediate compressed form for [TimeSeries] with metadata fields
    /// (index, timestamp bounds) uncompressed and payload columns
    /// stored as compressed ByteBuf. One instance per logical group.
    #[derive(pco_pack::serde::Serialize, pco_pack::serde::Deserialize, Default, Clone)]
    pub struct Chunk {
        #[serde(with = "pco_pack::chrono::serde::ts_microseconds")]
        pub start_at: pco_pack::chrono::DateTime<pco_pack::chrono::Utc>,
        #[serde(with = "pco_pack::chrono::serde::ts_microseconds")]
        pub end_at: pco_pack::chrono::DateTime<pco_pack::chrono::Utc>,
        #[serde(default)]
        pub database_id: serde_bytes::ByteBuf,
        #[serde(default)]
        pub value: serde_bytes::ByteBuf,
        #[serde(default)]
        pub collected_at: serde_bytes::ByteBuf,
    }
    #[derive(Clone, Default)]
    pub struct Reader {
        pub database_id: pco_pack::LazyReader<i64>,
        pub collected_at: pco_pack::LazyReader<i64>,
        pub value: pco_pack::LazyReader<f64>,
        pub start_at: pco_pack::chrono::DateTime<pco_pack::chrono::Utc>,
        pub end_at: pco_pack::chrono::DateTime<pco_pack::chrono::Utc>,
    }
    pub struct Writer {
        pub database_id: Vec<i64>,
        pub collected_at: Vec<i64>,
        pub value: Vec<f64>,
    }
    impl Default for Writer {
        fn default() -> Self {
            Self {
                database_id: Default::default(),
                collected_at: Default::default(),
                value: Default::default(),
            }
        }
    }
    /// Typed filter struct for [`#name`].
    #[derive(Clone, Default, pco_pack::serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Filter {
        pub database_id: Option<pco_pack::I64Filter>,
        pub collected_at: Option<pco_pack::DateTimeFilter>,
        pub value: Option<pco_pack::F64Filter>,
        #[serde(flatten)]
        others: pco_pack::serde_json::Map<String, pco_pack::serde_json::Value>,
    }
    impl Filter {
        /// Create a new filter with the specified index and timestamp constraints.
        /// Additional fields can be set via `Index<&str>` access on the returned instance.
        pub fn new(collected_at: impl Into<pco_pack::DateTimeFilter>) -> Self {
            Self {
                collected_at: Some(collected_at.into()),
                ..Default::default()
            }
        }
        /// Get a filter value for an arbitrary field (e.g., nested fields).
        pub fn get(&self, field: &str) -> Option<&pco_pack::serde_json::Value> {
            self.others.get(field)
        }
        /// Set a filter value for an arbitrary field.
        pub fn set(&mut self, field: &str, value: pco_pack::serde_json::Value) {
            self.others.insert(field.to_string(), value);
        }
        /// Returns the start and end timestamps from this filter. Requires the timestamp field to be set as a range.
        pub fn range_bounds(
            &self,
        ) -> pco_pack::anyhow::Result<
            (
                pco_pack::chrono::DateTime<pco_pack::chrono::Utc>,
                pco_pack::chrono::DateTime<pco_pack::chrono::Utc>,
            ),
        > {
            let filter = self
                .collected_at
                .as_ref()
                .ok_or_else(|| pco_pack::anyhow::anyhow!("Timestamp missing"))?;
            filter.range_bounds()
        }
        /// Returns the duration of this filter's time range. Requires the timestamp field to be set as a range.
        pub fn range_duration(
            &self,
        ) -> pco_pack::anyhow::Result<pco_pack::chrono::Duration> {
            let (start, end) = self.range_bounds()?;
            Ok(end - start)
        }
        /// Shifts the filter's time range by the given duration. Requires the timestamp field to be set as a range.
        pub fn range_shift(
            &mut self,
            shift: pco_pack::chrono::Duration,
        ) -> pco_pack::anyhow::Result<()> {
            let (start, end) = self.range_bounds()?;
            self.collected_at = Some(pco_pack::DateTimeFilter::Range {
                start: start + shift,
                end: end + shift,
            });
            Ok(())
        }
    }
    impl std::ops::Index<&str> for Filter {
        type Output = pco_pack::serde_json::Value;
        fn index(&self, field: &str) -> &Self::Output {
            self.others.get(field).unwrap_or(&pco_pack::serde_json::Value::Null)
        }
    }
    impl std::ops::IndexMut<&str> for Filter {
        fn index_mut(&mut self, field: &str) -> &mut Self::Output {
            self.others
                .entry(field.to_string())
                .or_insert(pco_pack::serde_json::Value::Null)
        }
    }
    impl TryFrom<Filter> for pco_pack::serde_json::Value {
        type Error = pco_pack::anyhow::Error;
        fn try_from(value: Filter) -> Result<Self, Self::Error> {
            let mut map = pco_pack::serde_json::Map::new();
            if let Some(ref v) = value.database_id {
                map.insert(
                    "database_id".to_string(),
                    pco_pack::serde_json::to_value(v)?,
                );
            }
            if let Some(ref v) = value.collected_at {
                map.insert(
                    "collected_at".to_string(),
                    pco_pack::serde_json::to_value(v)?,
                );
            }
            if let Some(ref v) = value.value {
                map.insert("value".to_string(), pco_pack::serde_json::to_value(v)?);
            }
            for (k, v) in &value.others {
                map.insert(k.clone(), v.clone());
            }
            Ok(pco_pack::serde_json::Value::Object(map))
        }
    }
    impl TryFrom<pco_pack::serde_json::Value> for Filter {
        type Error = pco_pack::anyhow::Error;
        fn try_from(value: pco_pack::serde_json::Value) -> Result<Self, Self::Error> {
            Ok(pco_pack::serde_json::from_value(value)?)
        }
    }
    impl pco_pack::PcoSerde for TimeSeries {
        type Writer = Writer;
        type Reader = Reader;
        fn write(
            data: Vec<Self>,
            _float_round: u32,
            _time_round: pco_pack::chrono::Duration,
        ) -> pco_pack::anyhow::Result<Vec<u8>> {
            let mut groups: pco_pack::ahash::HashMap<(), Vec<usize>> = Default::default();
            for (i, _) in data.iter().enumerate() {
                groups.entry(()).or_default().push(i);
            }
            let mut records: Vec<Chunk> = Vec::with_capacity(groups.len());
            for (_key, mut indices) in groups {
                indices
                    .sort_by(|&a, &b| {
                        timestamp_to_i64(&data[a].collected_at)
                            .cmp(&timestamp_to_i64(&data[b].collected_at))
                    });
                let mut col_collected_at: Vec<i64> = Vec::new();
                let mut col_database_id: Vec<i64> = Vec::new();
                let mut col_value: Vec<f64> = Vec::new();
                for &idx in indices.iter() {
                    let rec = &data[idx];
                    col_collected_at.push(rec.collected_at.clone());
                    col_database_id.push(rec.database_id.clone());
                    col_value.push(rec.value.clone());
                }
                let cb_collected_at = <i64 as pco_pack::PcoSerde>::write(
                    col_collected_at,
                    0u32 as u32,
                    Default::default(),
                )?;
                let cb_database_id = <i64 as pco_pack::PcoSerde>::write(
                    col_database_id,
                    0u32 as u32,
                    Default::default(),
                )?;
                let cb_value = <f64 as pco_pack::PcoSerde>::write(
                    col_value,
                    0u32 as u32,
                    Default::default(),
                )?;
                let (g_start, g_end) = if !indices.is_empty() {
                    let first = timestamp_to_i64(&data[indices[0]].collected_at);
                    let last = timestamp_to_i64(
                        &data[indices[indices.len() - 1]].collected_at,
                    );
                    (first, last)
                } else {
                    (0i64, 0i64)
                };
                records
                    .push(Chunk {
                        start_at: pco_pack::chrono::DateTime::<
                            pco_pack::chrono::Utc,
                        >::from_timestamp_micros(g_start)
                            .expect("valid timestamp"),
                        end_at: pco_pack::chrono::DateTime::<
                            pco_pack::chrono::Utc,
                        >::from_timestamp_micros(g_end)
                            .expect("valid timestamp"),
                        collected_at: cb_collected_at.into(),
                        database_id: cb_database_id.into(),
                        value: cb_value.into(),
                    });
            }
            let mut out = Vec::new();
            for rec in records {
                let mut msgpack = Vec::new();
                rec.serialize(
                    &mut pco_pack::rmp_serde::Serializer::new(&mut msgpack)
                        .with_struct_map(),
                )?;
                out.extend_from_slice(&(msgpack.len() as u64).to_le_bytes());
                out.extend_from_slice(&msgpack);
            }
            Ok(out)
        }
        fn read(
            src: &mut std::io::Cursor<&[u8]>,
            _float_round: u32,
            _time_round: pco_pack::chrono::Duration,
        ) -> pco_pack::anyhow::Result<Self::Reader> {
            let buf: &[u8] = src.get_ref();
            let pos = src.position() as usize;
            if pos + 8 > buf.len() {
                return Err(
                    pco_pack::anyhow::anyhow!("invalid data: expected length prefix"),
                );
            }
            let bl = {
                let mut bl_buf = [0u8; 8];
                for i in 0..8 {
                    bl_buf[i] = buf[pos + i];
                }
                u64::from_le_bytes(bl_buf) as usize
            };
            src.set_position(pos as u64 + 8);
            if bl == 0 {
                return Err(pco_pack::anyhow::anyhow!("invalid data: zero length"));
            }
            let data_start = src.position() as usize;
            if data_start + bl > buf.len() {
                return Err(pco_pack::anyhow::anyhow!("invalid data: truncated chunk"));
            }
            let mp = buf[data_start..data_start + bl].to_vec();
            src.set_position((data_start + bl) as u64);
            let rec: Chunk = match pco_pack::rmp_serde::from_slice(&mp) {
                Ok(r) => r,
                Err(e) => {
                    return Err(
                        pco_pack::anyhow::anyhow!("failed to deserialize chunk: {}", e),
                    );
                }
            };
            Ok(Reader {
                start_at: rec.start_at,
                end_at: rec.end_at,
                collected_at: pco_pack::LazyReader::new(
                    rec.collected_at.to_vec(),
                    0u32,
                    Default::default(),
                ),
                database_id: pco_pack::LazyReader::new(
                    rec.database_id.to_vec(),
                    0u32,
                    Default::default(),
                ),
                value: pco_pack::LazyReader::new(
                    rec.value.to_vec(),
                    0u32,
                    Default::default(),
                ),
            })
        }
        fn validate_bounds(
            reader: &mut Self::Reader,
        ) -> pco_pack::anyhow::Result<Option<usize>> {
            row_count(reader).map(Some)
        }
        fn get(
            reader: &mut Self::Reader,
            index: usize,
        ) -> pco_pack::anyhow::Result<Option<Self>> {
            let row_count = row_count(reader)?;
            if index < row_count {
                expand_row(reader, index).map(Some)
            } else {
                Ok(None)
            }
        }
    }
    impl pco_pack::PcoFilter for TimeSeries {
        fn filter_bulk(
            reader: &mut Self::Reader,
            field: usize,
            filter: &pco_pack::Filter,
            matches: &mut pco_pack::FilterMask,
        ) -> pco_pack::anyhow::Result<()> {
            if field == 0usize {
                return reader.database_id.filter_in_group(filter, matches);
            }
            if field == 1usize {
                match filter {
                    pco_pack::Filter::Range(range) => {
                        let (start, end) = (*range.start(), *range.end());
                        let chunk_start = reader.start_at.timestamp_micros();
                        let chunk_end = reader.end_at.timestamp_micros();
                        if chunk_start > end || chunk_end < start {
                            return Ok(matches.fill(false));
                        } else if chunk_start >= start && chunk_end <= end {
                            return Ok(());
                        } else {
                            return reader.collected_at.filter_in_group(filter, matches);
                        }
                    }
                    _ => {
                        return reader.collected_at.filter_in_group(filter, matches);
                    }
                }
            }
            if field == 2usize {
                return reader.value.filter_in_group(filter, matches);
            }
            unreachable!("filter_bulk called for unknown field index {}", field);
        }
        fn filter_nested(
            reader: &mut Self::Reader,
            path: &[usize],
            filter: &pco_pack::Filter,
            matches: &mut pco_pack::FilterMask,
        ) -> pco_pack::anyhow::Result<()> {
            match path[0] {
                0usize => {
                    return reader.database_id.filter_nested(&path[1..], filter, matches);
                }
                1usize => {
                    return reader
                        .collected_at
                        .filter_nested(&path[1..], filter, matches);
                }
                2usize => {
                    return reader.value.filter_nested(&path[1..], filter, matches);
                }
                _ => unreachable!("filter_nested called with invalid path {:?}", path),
            }
        }
        fn filter_match(_value: &Self, _filter: &pco_pack::Filter) -> bool {
            false
        }
        fn resolve_filter(
            path: &str,
            json: &pco_pack::serde_json::Value,
        ) -> pco_pack::anyhow::Result<pco_pack::ResolvedFilter> {
            let (root, remainder) = match path.split_once('.') {
                Some((head, tail)) => (head, Some(tail)),
                None => (path, None),
            };
            match root {
                "" => {
                    if remainder.is_some() {
                        return Err(
                            pco_pack::anyhow::anyhow!(
                                "Empty field name cannot have a nested path"
                            ),
                        );
                    }
                    let sub_path = remainder.unwrap_or("");
                    let mut filter = <i64 as pco_pack::PcoFilter>::resolve_filter(
                        sub_path,
                        json,
                    )?;
                    if sub_path.is_empty() {
                        filter.path[0] = 0;
                    } else {
                        filter.path.insert(0, 0);
                    }
                    Ok(filter)
                }
                "database_id" => {
                    let sub_path = remainder.unwrap_or("");
                    let mut filter = <i64 as pco_pack::PcoFilter>::resolve_filter(
                        sub_path,
                        json,
                    )?;
                    if sub_path.is_empty() {
                        filter.path[0] = 0usize;
                    } else {
                        filter.path.insert(0, 0usize);
                    }
                    Ok(filter)
                }
                "collected_at" => {
                    let sub_path = remainder.unwrap_or("");
                    let mut filter = <i64 as pco_pack::PcoFilter>::resolve_filter(
                        sub_path,
                        json,
                    )?;
                    if sub_path.is_empty() {
                        filter.path[0] = 1usize;
                    } else {
                        filter.path.insert(0, 1usize);
                    }
                    Ok(filter)
                }
                "value" => {
                    let sub_path = remainder.unwrap_or("");
                    let mut filter = <f64 as pco_pack::PcoFilter>::resolve_filter(
                        sub_path,
                        json,
                    )?;
                    if sub_path.is_empty() {
                        filter.path[0] = 2usize;
                    } else {
                        filter.path.insert(0, 2usize);
                    }
                    Ok(filter)
                }
                _ => {
                    Err(
                        pco_pack::anyhow::anyhow!(
                            "Field path segment '{}' does not exist in schema definition",
                            root
                        ),
                    )
                }
            }
        }
    }
    impl pco_pack::PcoPack for TimeSeries {
        type Reader = Reader;
        type Chunk = Chunk;
        type Filter = Filter;
        fn write(data: Vec<Self>) -> pco_pack::anyhow::Result<Vec<Self::Chunk>> {
            let mut groups: pco_pack::ahash::HashMap<(), Vec<usize>> = Default::default();
            for (i, _) in data.iter().enumerate() {
                groups.entry(()).or_default().push(i);
            }
            let mut result: Vec<Chunk> = Vec::new();
            for (_key, mut indices) in groups {
                indices
                    .sort_by(|&a, &b| {
                        timestamp_to_i64(&data[a].collected_at)
                            .cmp(&timestamp_to_i64(&data[b].collected_at))
                    });
                for group_indices in indices.chunks(TimeSeries::CHUNK_SIZE) {
                    let mut col_collected_at: Vec<i64> = Vec::new();
                    let mut col_database_id: Vec<i64> = Vec::new();
                    let mut col_value: Vec<f64> = Vec::new();
                    for &idx in group_indices {
                        let rec = &data[idx];
                        col_collected_at.push(rec.collected_at.clone());
                        col_database_id.push(rec.database_id.clone());
                        col_value.push(rec.value.clone());
                    }
                    let cb_collected_at = <i64 as pco_pack::PcoSerde>::write(
                        col_collected_at,
                        0u32 as u32,
                        Default::default(),
                    )?;
                    let cb_database_id = <i64 as pco_pack::PcoSerde>::write(
                        col_database_id,
                        0u32 as u32,
                        Default::default(),
                    )?;
                    let cb_value = <f64 as pco_pack::PcoSerde>::write(
                        col_value,
                        0u32 as u32,
                        Default::default(),
                    )?;
                    let (g_start, g_end) = if !group_indices.is_empty() {
                        let first = timestamp_to_i64(
                            &data[group_indices[0]].collected_at,
                        );
                        let last = timestamp_to_i64(
                            &data[group_indices[group_indices.len() - 1]].collected_at,
                        );
                        (first, last)
                    } else {
                        (0i64, 0i64)
                    };
                    let rec = Chunk {
                        start_at: pco_pack::chrono::DateTime::<
                            pco_pack::chrono::Utc,
                        >::from_timestamp_micros(g_start)
                            .expect("valid timestamp"),
                        end_at: pco_pack::chrono::DateTime::<
                            pco_pack::chrono::Utc,
                        >::from_timestamp_micros(g_end)
                            .expect("valid timestamp"),
                        collected_at: cb_collected_at.into(),
                        database_id: cb_database_id.into(),
                        value: cb_value.into(),
                    };
                    result.push(rec);
                }
            }
            Ok(result)
        }
        fn read(chunks: Vec<Self::Chunk>) -> pco_pack::anyhow::Result<Vec<Self>> {
            let mut result = Vec::new();
            for c in chunks {
                result.extend(c.expand()?);
            }
            Ok(result)
        }
        fn to_bytes(chunks: &[Self::Chunk]) -> pco_pack::anyhow::Result<Vec<u8>> {
            let mut out = Vec::new();
            let mut ser = pco_pack::rmp_serde::Serializer::new(&mut out)
                .with_struct_map();
            pco_pack::serde::Serialize::serialize(chunks, &mut ser)?;
            Ok(out)
        }
        fn from_bytes(bytes: &[u8]) -> pco_pack::anyhow::Result<Vec<Self::Chunk>> {
            Ok(pco_pack::rmp_serde::from_slice(bytes)?)
        }
        fn filter_value(
            chunks: &[Self::Chunk],
            query: &pco_pack::serde_json::Value,
            fields: &[&str],
        ) -> pco_pack::anyhow::Result<Vec<Self>> {
            let mut result = Vec::new();
            for c in chunks {
                result.extend(c.filter(query, fields)?);
            }
            Ok(result)
        }
        fn resolve_fields(
            query: &pco_pack::serde_json::Value,
            fields: &[&str],
        ) -> pco_pack::anyhow::Result<Vec<&'static str>> {
            let known: &[&str] = &[
                "database_id",
                "collected_at",
                "value",
                "start_at",
                "end_at",
            ];
            let query_fields: Vec<String> = match query {
                pco_pack::serde_json::Value::Object(map) => {
                    map.keys()
                        .map(|k| {
                            k
                                .split_once('.')
                                .map(|(top, _)| top.to_string())
                                .unwrap_or(k.clone())
                        })
                        .collect()
                }
                _ => Vec::new(),
            };
            for field in &query_fields {
                if !known.contains(&field.as_str()) {
                    return Err(pco_pack::anyhow::anyhow!("Unknown field: {}", field));
                }
            }
            for &field in fields {
                if field.contains('.') {
                    let top = field.split_once('.').map(|(t, _)| t).unwrap_or(field);
                    return Err(
                        pco_pack::anyhow::anyhow!(
                            "Nested field path '{field}' is not supported; use '{top}' instead when specifying which fields to load",
                        ),
                    );
                }
                if !known.contains(&field) {
                    return Err(pco_pack::anyhow::anyhow!("Unknown field: {}", field));
                }
            }
            let mut all_fields = Vec::new();
            if fields.is_empty() {
                all_fields.extend(known);
            } else {
                for &k in known {
                    if fields.contains(&k)
                        || query_fields.iter().any(|q| q.as_str() == k)
                    {
                        all_fields.push(k);
                    }
                }
            }
            let ts_field = "collected_at";
            let timestamp_requested = fields.iter().any(|&f| f == ts_field)
                || query_fields.iter().any(|q| q.as_str() == ts_field);
            if timestamp_requested || fields.is_empty() {
                all_fields.push("start_at");
                all_fields.push("end_at");
            }
            Ok(all_fields)
        }
    }
    #[inline]
    fn timestamp_to_i64(v: &i64) -> i64 {
        *v
    }
    #[inline]
    fn row_count(g: &mut Reader) -> pco_pack::anyhow::Result<usize> {
        g.collected_at.row_count()
    }
    #[inline]
    fn expand_row(g: &mut Reader, row: usize) -> pco_pack::anyhow::Result<TimeSeries> {
        Ok(TimeSeries {
            database_id: g.database_id.pop_inner(row)?,
            collected_at: g.collected_at.pop_inner(row)?,
            value: g.value.pop_inner(row)?,
        })
    }
    impl Chunk {
        fn compute_row_count(
            reader: &mut Reader,
            fields: &[&str],
        ) -> pco_pack::anyhow::Result<usize> {
            let mut count = 0usize;
            if count == 0 && fields.contains(&"value") {
                let r = reader.value.row_count();
                match r {
                    Ok(n) if n > 0 => count = n,
                    Err(e) => return Err(e),
                    _ => {}
                }
            }
            if count == 0 && fields.contains(&"database_id") {
                let r = reader.database_id.row_count();
                match r {
                    Ok(n) if n > 0 => count = n,
                    Err(e) => return Err(e),
                    _ => {}
                }
            }
            if count == 0 && fields.contains(&"collected_at") {
                let r = reader.collected_at.row_count();
                match r {
                    Ok(n) if n > 0 => count = n,
                    Err(e) => return Err(e),
                    _ => {}
                }
            }
            let count = count;
            let has_loaded_payload = fields.contains(&"collected_at")
                || fields.contains(&"database_id") || fields.contains(&"value") || false;
            if count == 0 && !has_loaded_payload && false {
                return Ok(1);
            }
            if count == 0 {
                return Err(
                    pco_pack::anyhow::anyhow!(
                        "Chunk has no row data and no index fields"
                    ),
                );
            }
            Ok(count)
        }
        /// Decompresses all payload columns and reconstructs the original rows.
        pub fn expand(self) -> pco_pack::anyhow::Result<Vec<TimeSeries>> {
            let mut reader = Reader {
                start_at: self.start_at,
                end_at: self.end_at,
                collected_at: <pco_pack::LazyReader<
                    i64,
                >>::new(self.collected_at.to_vec(), 0u32, Default::default()),
                database_id: <pco_pack::LazyReader<
                    i64,
                >>::new(self.database_id.to_vec(), 0u32, Default::default()),
                value: <pco_pack::LazyReader<
                    f64,
                >>::new(self.value.to_vec(), 0u32, Default::default()),
            };
            let loaded_fields: &[&str] = &["collected_at", "database_id", "value"];
            let row_count = Self::compute_row_count(&mut reader, loaded_fields)?;
            let mut results = Vec::with_capacity(row_count);
            for row_idx in 0..row_count {
                results.push(expand_row(&mut reader, row_idx)?);
            }
            Ok(results)
        }
        /// Filters rows from compressed form using a JSON query.
        pub fn filter(
            &self,
            query: &pco_pack::serde_json::Value,
            fields: &[&str],
        ) -> pco_pack::anyhow::Result<Vec<TimeSeries>> {
            let fields = <TimeSeries as pco_pack::PcoPack>::resolve_fields(
                query,
                fields,
            )?;
            let execution_plan = <TimeSeries as pco_pack::PcoFilter>::resolve_query(
                query,
            )?;
            let mut reader = Reader {
                start_at: self.start_at,
                end_at: self.end_at,
                collected_at: <pco_pack::LazyReader<
                    i64,
                >>::new(
                    if fields.is_empty() || fields.contains(&"collected_at") {
                        self.collected_at.to_vec()
                    } else {
                        Vec::new()
                    },
                    0u32,
                    Default::default(),
                ),
                database_id: <pco_pack::LazyReader<
                    i64,
                >>::new(
                    if fields.is_empty() || fields.contains(&"database_id") {
                        self.database_id.to_vec()
                    } else {
                        Vec::new()
                    },
                    0u32,
                    Default::default(),
                ),
                value: <pco_pack::LazyReader<
                    f64,
                >>::new(
                    if fields.is_empty() || fields.contains(&"value") {
                        self.value.to_vec()
                    } else {
                        Vec::new()
                    },
                    0u32,
                    Default::default(),
                ),
            };
            let row_count = Self::compute_row_count(&mut reader, &fields)?;
            let mut matches = pco_pack::FilterMask::ones(row_count);
            for step in &execution_plan {
                <TimeSeries as pco_pack::PcoFilter>::filter_step(
                    &mut reader,
                    &step.path,
                    &step.filter,
                    &mut matches,
                )?;
            }
            let mut results = Vec::with_capacity(matches.count_ones());
            let raw_chunks = matches.as_raw_slice();
            let mut base_row_index: usize = 0;
            let total_chunks = (row_count + 63) / 64;
            for (chunk_idx, &chunk) in raw_chunks.iter().enumerate() {
                if chunk_idx >= total_chunks {
                    break;
                }
                let remaining_bits = row_count.saturating_sub(base_row_index);
                let masked_chunk = if remaining_bits < 64 {
                    chunk & ((1u64 << remaining_bits) - 1)
                } else {
                    chunk
                };
                let mut current_chunk = masked_chunk;
                while current_chunk != 0 {
                    let skip = current_chunk.trailing_zeros() as usize;
                    let actual_row_index = base_row_index + skip;
                    results.push(expand_row(&mut reader, actual_row_index)?);
                    current_chunk &= current_chunk - 1;
                }
                base_row_index += 64;
            }
            Ok(results)
        }
    }
};
