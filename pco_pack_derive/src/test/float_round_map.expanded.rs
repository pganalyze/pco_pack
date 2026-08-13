pub struct MetricRecord {
    id: i64,
    metrics: HashMap<String, f64>,
}
const _: () = {
    #[allow(unused)]
    use pco_pack::anyhow::Context;
    use pco_pack::serde::Serialize;
    /// Intermediate compressed form for [MetricRecord] with metadata fields
    /// (index, timestamp bounds) uncompressed and payload columns
    /// stored as compressed ByteBuf. One instance per logical group.
    #[derive(pco_pack::serde::Serialize, pco_pack::serde::Deserialize, Default, Clone)]
    pub struct Chunk {
        #[serde(default)]
        pub id: serde_bytes::ByteBuf,
        #[serde(default)]
        pub metrics: serde_bytes::ByteBuf,
    }
    #[derive(Clone, Default)]
    pub struct Reader {
        pub id: pco_pack::LazyReader<i64>,
        pub metrics: pco_pack::LazyReader<HashMap<String, f64>>,
    }
    pub struct Writer {
        pub id: Vec<i64>,
        pub metrics: Vec<HashMap<String, f64>>,
    }
    impl Default for Writer {
        fn default() -> Self {
            Self {
                id: Default::default(),
                metrics: Default::default(),
            }
        }
    }
    /// Typed filter struct for [`#name`].
    #[derive(Clone, Default, pco_pack::serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Filter {
        pub id: Option<pco_pack::I64Filter>,
        #[serde(flatten)]
        others: pco_pack::serde_json::Map<String, pco_pack::serde_json::Value>,
    }
    impl Filter {
        /// Create a new filter with the specified index and timestamp constraints.
        /// Additional fields can be set via `Index<&str>` access on the returned instance.
        pub fn new() -> Self {
            Self { ..Default::default() }
        }
        /// Get a filter value for an arbitrary field (e.g., nested fields).
        pub fn get(&self, field: &str) -> Option<&pco_pack::serde_json::Value> {
            self.others.get(field)
        }
        /// Set a filter value for an arbitrary field.
        pub fn set(&mut self, field: &str, value: pco_pack::serde_json::Value) {
            self.others.insert(field.to_string(), value);
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
            if let Some(ref v) = value.id {
                map.insert("id".to_string(), pco_pack::serde_json::to_value(v)?);
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
    impl pco_pack::PcoSerde for MetricRecord {
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
            for (_key, indices) in groups {
                let mut col_id: Vec<i64> = Vec::new();
                let mut col_metrics: Vec<HashMap<String, f64>> = Vec::new();
                for &idx in indices.iter() {
                    let rec = &data[idx];
                    col_id.push(rec.id.clone());
                    col_metrics.push(rec.metrics.clone());
                }
                let cb_id = <i64 as pco_pack::PcoSerde>::write(
                    col_id,
                    2u32 as u32,
                    Default::default(),
                )?;
                let cb_metrics = <HashMap<
                    String,
                    f64,
                > as pco_pack::PcoSerde>::write(
                    col_metrics,
                    2u32 as u32,
                    Default::default(),
                )?;
                records
                    .push(Chunk {
                        id: cb_id.into(),
                        metrics: cb_metrics.into(),
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
                id: pco_pack::LazyReader::new(rec.id.to_vec(), 2u32, Default::default()),
                metrics: pco_pack::LazyReader::new(
                    rec.metrics.to_vec(),
                    2u32,
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
    impl pco_pack::PcoFilter for MetricRecord {
        fn filter_bulk(
            reader: &mut Self::Reader,
            field: usize,
            filter: &pco_pack::Filter,
            matches: &mut pco_pack::FilterMask,
        ) -> pco_pack::anyhow::Result<()> {
            if field == 0usize {
                return reader.id.filter_in_group(filter, matches);
            }
            if field == 1usize {
                return reader.metrics.filter_in_group(filter, matches);
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
                    return reader.id.filter_nested(&path[1..], filter, matches);
                }
                1usize => {
                    return reader.metrics.filter_nested(&path[1..], filter, matches);
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
                "id" => {
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
                "metrics" => {
                    let sub_path = remainder.unwrap_or("");
                    let mut filter = <HashMap<
                        String,
                        f64,
                    > as pco_pack::PcoFilter>::resolve_filter(sub_path, json)?;
                    if sub_path.is_empty() {
                        filter.path[0] = 1usize;
                    } else {
                        filter.path.insert(0, 1usize);
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
    impl pco_pack::PcoPack for MetricRecord {
        type Reader = Reader;
        type Chunk = Chunk;
        type Filter = Filter;
        fn write(data: Vec<Self>) -> pco_pack::anyhow::Result<Vec<Self::Chunk>> {
            let mut groups: pco_pack::ahash::HashMap<(), Vec<usize>> = Default::default();
            for (i, _) in data.iter().enumerate() {
                groups.entry(()).or_default().push(i);
            }
            let mut result: Vec<Chunk> = Vec::new();
            for (_key, indices) in groups {
                for group_indices in indices.chunks(MetricRecord::CHUNK_SIZE) {
                    let mut col_id: Vec<i64> = Vec::new();
                    let mut col_metrics: Vec<HashMap<String, f64>> = Vec::new();
                    for &idx in group_indices {
                        let rec = &data[idx];
                        col_id.push(rec.id.clone());
                        col_metrics.push(rec.metrics.clone());
                    }
                    let cb_id = <i64 as pco_pack::PcoSerde>::write(
                        col_id,
                        2u32 as u32,
                        Default::default(),
                    )?;
                    let cb_metrics = <HashMap<
                        String,
                        f64,
                    > as pco_pack::PcoSerde>::write(
                        col_metrics,
                        2u32 as u32,
                        Default::default(),
                    )?;
                    let rec = Chunk {
                        id: cb_id.into(),
                        metrics: cb_metrics.into(),
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
            let known: &[&str] = &["id", "metrics"];
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
            Ok(all_fields)
        }
    }
    #[inline]
    fn row_count(g: &mut Reader) -> pco_pack::anyhow::Result<usize> {
        let mut count = 0usize;
        if count == 0 {
            match g.id.row_count() {
                Ok(n) if n > 0 => count = n,
                Err(e) => return Err(e),
                _ => {}
            }
        }
        if count == 0 {
            match g.metrics.row_count() {
                Ok(n) if n > 0 => count = n,
                Err(e) => return Err(e),
                _ => {}
            }
        }
        Ok(count)
    }
    #[inline]
    fn expand_row(g: &mut Reader, row: usize) -> pco_pack::anyhow::Result<MetricRecord> {
        Ok(MetricRecord {
            id: g.id.pop_inner(row)?,
            metrics: g.metrics.pop_inner(row)?,
        })
    }
    impl Chunk {
        fn compute_row_count(
            reader: &mut Reader,
            fields: &[&str],
        ) -> pco_pack::anyhow::Result<usize> {
            let mut count = 0usize;
            if count == 0 && fields.contains(&"metrics") {
                let r = reader.metrics.row_count();
                match r {
                    Ok(n) if n > 0 => count = n,
                    Err(e) => return Err(e),
                    _ => {}
                }
            }
            if count == 0 && fields.contains(&"id") {
                let r = reader.id.row_count();
                match r {
                    Ok(n) if n > 0 => count = n,
                    Err(e) => return Err(e),
                    _ => {}
                }
            }
            let count = count;
            let has_loaded_payload = fields.contains(&"id")
                || fields.contains(&"metrics") || false;
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
        pub fn expand(self) -> pco_pack::anyhow::Result<Vec<MetricRecord>> {
            let mut reader = Reader {
                id: <pco_pack::LazyReader<
                    i64,
                >>::new(self.id.to_vec(), 2u32, Default::default()),
                metrics: <pco_pack::LazyReader<
                    HashMap<String, f64>,
                >>::new(self.metrics.to_vec(), 2u32, Default::default()),
            };
            let loaded_fields: &[&str] = &["id", "metrics"];
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
        ) -> pco_pack::anyhow::Result<Vec<MetricRecord>> {
            let fields = <MetricRecord as pco_pack::PcoPack>::resolve_fields(
                query,
                fields,
            )?;
            let execution_plan = <MetricRecord as pco_pack::PcoFilter>::resolve_query(
                query,
            )?;
            let mut reader = Reader {
                id: <pco_pack::LazyReader<
                    i64,
                >>::new(
                    if fields.is_empty() || fields.contains(&"id") {
                        self.id.to_vec()
                    } else {
                        Vec::new()
                    },
                    2u32,
                    Default::default(),
                ),
                metrics: <pco_pack::LazyReader<
                    HashMap<String, f64>,
                >>::new(
                    if fields.is_empty() || fields.contains(&"metrics") {
                        self.metrics.to_vec()
                    } else {
                        Vec::new()
                    },
                    2u32,
                    Default::default(),
                ),
            };
            let row_count = Self::compute_row_count(&mut reader, &fields)?;
            let mut matches = pco_pack::FilterMask::ones(row_count);
            for step in &execution_plan {
                <MetricRecord as pco_pack::PcoFilter>::filter_step(
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
pub struct MapTupleRecord {
    id: i64,
    metrics: HashMap<String, (f64, i32)>,
}
const _: () = {
    #[allow(unused)]
    use pco_pack::anyhow::Context;
    use pco_pack::serde::Serialize;
    /// Intermediate compressed form for [MapTupleRecord] with metadata fields
    /// (index, timestamp bounds) uncompressed and payload columns
    /// stored as compressed ByteBuf. One instance per logical group.
    #[derive(pco_pack::serde::Serialize, pco_pack::serde::Deserialize, Default, Clone)]
    pub struct Chunk {
        #[serde(default)]
        pub id: serde_bytes::ByteBuf,
        #[serde(default)]
        pub metrics: serde_bytes::ByteBuf,
    }
    #[derive(Clone, Default)]
    pub struct Reader {
        pub id: pco_pack::LazyReader<i64>,
        pub metrics: pco_pack::LazyReader<HashMap<String, (f64, i32)>>,
    }
    pub struct Writer {
        pub id: Vec<i64>,
        pub metrics: Vec<HashMap<String, (f64, i32)>>,
    }
    impl Default for Writer {
        fn default() -> Self {
            Self {
                id: Default::default(),
                metrics: Default::default(),
            }
        }
    }
    /// Typed filter struct for [`#name`].
    #[derive(Clone, Default, pco_pack::serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Filter {
        pub id: Option<pco_pack::I64Filter>,
        #[serde(flatten)]
        others: pco_pack::serde_json::Map<String, pco_pack::serde_json::Value>,
    }
    impl Filter {
        /// Create a new filter with the specified index and timestamp constraints.
        /// Additional fields can be set via `Index<&str>` access on the returned instance.
        pub fn new() -> Self {
            Self { ..Default::default() }
        }
        /// Get a filter value for an arbitrary field (e.g., nested fields).
        pub fn get(&self, field: &str) -> Option<&pco_pack::serde_json::Value> {
            self.others.get(field)
        }
        /// Set a filter value for an arbitrary field.
        pub fn set(&mut self, field: &str, value: pco_pack::serde_json::Value) {
            self.others.insert(field.to_string(), value);
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
            if let Some(ref v) = value.id {
                map.insert("id".to_string(), pco_pack::serde_json::to_value(v)?);
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
    impl pco_pack::PcoSerde for MapTupleRecord {
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
            for (_key, indices) in groups {
                let mut col_id: Vec<i64> = Vec::new();
                let mut col_metrics: Vec<HashMap<String, (f64, i32)>> = Vec::new();
                for &idx in indices.iter() {
                    let rec = &data[idx];
                    col_id.push(rec.id.clone());
                    col_metrics.push(rec.metrics.clone());
                }
                let cb_id = <i64 as pco_pack::PcoSerde>::write(
                    col_id,
                    2u32 as u32,
                    Default::default(),
                )?;
                let cb_metrics = <HashMap<
                    String,
                    (f64, i32),
                > as pco_pack::PcoSerde>::write(
                    col_metrics,
                    2u32 as u32,
                    Default::default(),
                )?;
                records
                    .push(Chunk {
                        id: cb_id.into(),
                        metrics: cb_metrics.into(),
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
                id: pco_pack::LazyReader::new(rec.id.to_vec(), 2u32, Default::default()),
                metrics: pco_pack::LazyReader::new(
                    rec.metrics.to_vec(),
                    2u32,
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
    impl pco_pack::PcoFilter for MapTupleRecord {
        fn filter_bulk(
            reader: &mut Self::Reader,
            field: usize,
            filter: &pco_pack::Filter,
            matches: &mut pco_pack::FilterMask,
        ) -> pco_pack::anyhow::Result<()> {
            if field == 0usize {
                return reader.id.filter_in_group(filter, matches);
            }
            if field == 1usize {
                return reader.metrics.filter_in_group(filter, matches);
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
                    return reader.id.filter_nested(&path[1..], filter, matches);
                }
                1usize => {
                    return reader.metrics.filter_nested(&path[1..], filter, matches);
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
                "id" => {
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
                "metrics" => {
                    let sub_path = remainder.unwrap_or("");
                    let mut filter = <HashMap<
                        String,
                        (f64, i32),
                    > as pco_pack::PcoFilter>::resolve_filter(sub_path, json)?;
                    if sub_path.is_empty() {
                        filter.path[0] = 1usize;
                    } else {
                        filter.path.insert(0, 1usize);
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
    impl pco_pack::PcoPack for MapTupleRecord {
        type Reader = Reader;
        type Chunk = Chunk;
        type Filter = Filter;
        fn write(data: Vec<Self>) -> pco_pack::anyhow::Result<Vec<Self::Chunk>> {
            let mut groups: pco_pack::ahash::HashMap<(), Vec<usize>> = Default::default();
            for (i, _) in data.iter().enumerate() {
                groups.entry(()).or_default().push(i);
            }
            let mut result: Vec<Chunk> = Vec::new();
            for (_key, indices) in groups {
                for group_indices in indices.chunks(MapTupleRecord::CHUNK_SIZE) {
                    let mut col_id: Vec<i64> = Vec::new();
                    let mut col_metrics: Vec<HashMap<String, (f64, i32)>> = Vec::new();
                    for &idx in group_indices {
                        let rec = &data[idx];
                        col_id.push(rec.id.clone());
                        col_metrics.push(rec.metrics.clone());
                    }
                    let cb_id = <i64 as pco_pack::PcoSerde>::write(
                        col_id,
                        2u32 as u32,
                        Default::default(),
                    )?;
                    let cb_metrics = <HashMap<
                        String,
                        (f64, i32),
                    > as pco_pack::PcoSerde>::write(
                        col_metrics,
                        2u32 as u32,
                        Default::default(),
                    )?;
                    let rec = Chunk {
                        id: cb_id.into(),
                        metrics: cb_metrics.into(),
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
            let known: &[&str] = &["id", "metrics"];
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
            Ok(all_fields)
        }
    }
    #[inline]
    fn row_count(g: &mut Reader) -> pco_pack::anyhow::Result<usize> {
        let mut count = 0usize;
        if count == 0 {
            match g.id.row_count() {
                Ok(n) if n > 0 => count = n,
                Err(e) => return Err(e),
                _ => {}
            }
        }
        if count == 0 {
            match g.metrics.row_count() {
                Ok(n) if n > 0 => count = n,
                Err(e) => return Err(e),
                _ => {}
            }
        }
        Ok(count)
    }
    #[inline]
    fn expand_row(
        g: &mut Reader,
        row: usize,
    ) -> pco_pack::anyhow::Result<MapTupleRecord> {
        Ok(MapTupleRecord {
            id: g.id.pop_inner(row)?,
            metrics: g.metrics.pop_inner(row)?,
        })
    }
    impl Chunk {
        fn compute_row_count(
            reader: &mut Reader,
            fields: &[&str],
        ) -> pco_pack::anyhow::Result<usize> {
            let mut count = 0usize;
            if count == 0 && fields.contains(&"metrics") {
                let r = reader.metrics.row_count();
                match r {
                    Ok(n) if n > 0 => count = n,
                    Err(e) => return Err(e),
                    _ => {}
                }
            }
            if count == 0 && fields.contains(&"id") {
                let r = reader.id.row_count();
                match r {
                    Ok(n) if n > 0 => count = n,
                    Err(e) => return Err(e),
                    _ => {}
                }
            }
            let count = count;
            let has_loaded_payload = fields.contains(&"id")
                || fields.contains(&"metrics") || false;
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
        pub fn expand(self) -> pco_pack::anyhow::Result<Vec<MapTupleRecord>> {
            let mut reader = Reader {
                id: <pco_pack::LazyReader<
                    i64,
                >>::new(self.id.to_vec(), 2u32, Default::default()),
                metrics: <pco_pack::LazyReader<
                    HashMap<String, (f64, i32)>,
                >>::new(self.metrics.to_vec(), 2u32, Default::default()),
            };
            let loaded_fields: &[&str] = &["id", "metrics"];
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
        ) -> pco_pack::anyhow::Result<Vec<MapTupleRecord>> {
            let fields = <MapTupleRecord as pco_pack::PcoPack>::resolve_fields(
                query,
                fields,
            )?;
            let execution_plan = <MapTupleRecord as pco_pack::PcoFilter>::resolve_query(
                query,
            )?;
            let mut reader = Reader {
                id: <pco_pack::LazyReader<
                    i64,
                >>::new(
                    if fields.is_empty() || fields.contains(&"id") {
                        self.id.to_vec()
                    } else {
                        Vec::new()
                    },
                    2u32,
                    Default::default(),
                ),
                metrics: <pco_pack::LazyReader<
                    HashMap<String, (f64, i32)>,
                >>::new(
                    if fields.is_empty() || fields.contains(&"metrics") {
                        self.metrics.to_vec()
                    } else {
                        Vec::new()
                    },
                    2u32,
                    Default::default(),
                ),
            };
            let row_count = Self::compute_row_count(&mut reader, &fields)?;
            let mut matches = pco_pack::FilterMask::ones(row_count);
            for step in &execution_plan {
                <MapTupleRecord as pco_pack::PcoFilter>::filter_step(
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
pub struct OptionMapTupleRecord {
    id: i64,
    metrics: Option<HashMap<String, (f32, i32)>>,
}
const _: () = {
    #[allow(unused)]
    use pco_pack::anyhow::Context;
    use pco_pack::serde::Serialize;
    /// Intermediate compressed form for [OptionMapTupleRecord] with metadata fields
    /// (index, timestamp bounds) uncompressed and payload columns
    /// stored as compressed ByteBuf. One instance per logical group.
    #[derive(pco_pack::serde::Serialize, pco_pack::serde::Deserialize, Default, Clone)]
    pub struct Chunk {
        #[serde(default)]
        pub id: serde_bytes::ByteBuf,
        #[serde(default)]
        pub metrics: serde_bytes::ByteBuf,
    }
    #[derive(Clone, Default)]
    pub struct Reader {
        pub id: pco_pack::LazyReader<i64>,
        pub metrics: pco_pack::LazyReader<Option<HashMap<String, (f32, i32)>>>,
    }
    pub struct Writer {
        pub id: Vec<i64>,
        pub metrics: Vec<Option<HashMap<String, (f32, i32)>>>,
    }
    impl Default for Writer {
        fn default() -> Self {
            Self {
                id: Default::default(),
                metrics: Default::default(),
            }
        }
    }
    /// Typed filter struct for [`#name`].
    #[derive(Clone, Default, pco_pack::serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Filter {
        pub id: Option<pco_pack::I64Filter>,
        #[serde(flatten)]
        others: pco_pack::serde_json::Map<String, pco_pack::serde_json::Value>,
    }
    impl Filter {
        /// Create a new filter with the specified index and timestamp constraints.
        /// Additional fields can be set via `Index<&str>` access on the returned instance.
        pub fn new() -> Self {
            Self { ..Default::default() }
        }
        /// Get a filter value for an arbitrary field (e.g., nested fields).
        pub fn get(&self, field: &str) -> Option<&pco_pack::serde_json::Value> {
            self.others.get(field)
        }
        /// Set a filter value for an arbitrary field.
        pub fn set(&mut self, field: &str, value: pco_pack::serde_json::Value) {
            self.others.insert(field.to_string(), value);
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
            if let Some(ref v) = value.id {
                map.insert("id".to_string(), pco_pack::serde_json::to_value(v)?);
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
    impl pco_pack::PcoSerde for OptionMapTupleRecord {
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
            for (_key, indices) in groups {
                let mut col_id: Vec<i64> = Vec::new();
                let mut col_metrics: Vec<Option<HashMap<String, (f32, i32)>>> = Vec::new();
                for &idx in indices.iter() {
                    let rec = &data[idx];
                    col_id.push(rec.id.clone());
                    col_metrics.push(rec.metrics.clone());
                }
                let cb_id = <i64 as pco_pack::PcoSerde>::write(
                    col_id,
                    2u32 as u32,
                    Default::default(),
                )?;
                let cb_metrics = <Option<
                    HashMap<String, (f32, i32)>,
                > as pco_pack::PcoSerde>::write(
                    col_metrics,
                    2u32 as u32,
                    Default::default(),
                )?;
                records
                    .push(Chunk {
                        id: cb_id.into(),
                        metrics: cb_metrics.into(),
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
                id: pco_pack::LazyReader::new(rec.id.to_vec(), 2u32, Default::default()),
                metrics: pco_pack::LazyReader::new(
                    rec.metrics.to_vec(),
                    2u32,
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
    impl pco_pack::PcoFilter for OptionMapTupleRecord {
        fn filter_bulk(
            reader: &mut Self::Reader,
            field: usize,
            filter: &pco_pack::Filter,
            matches: &mut pco_pack::FilterMask,
        ) -> pco_pack::anyhow::Result<()> {
            if field == 0usize {
                return reader.id.filter_in_group(filter, matches);
            }
            if field == 1usize {
                return reader.metrics.filter_in_group(filter, matches);
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
                    return reader.id.filter_nested(&path[1..], filter, matches);
                }
                1usize => {
                    return reader.metrics.filter_nested(&path[1..], filter, matches);
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
                "id" => {
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
                "metrics" => {
                    let sub_path = remainder.unwrap_or("");
                    let mut filter = <Option<
                        HashMap<String, (f32, i32)>,
                    > as pco_pack::PcoFilter>::resolve_filter(sub_path, json)?;
                    if sub_path.is_empty() {
                        filter.path[0] = 1usize;
                    } else {
                        filter.path.insert(0, 1usize);
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
    impl pco_pack::PcoPack for OptionMapTupleRecord {
        type Reader = Reader;
        type Chunk = Chunk;
        type Filter = Filter;
        fn write(data: Vec<Self>) -> pco_pack::anyhow::Result<Vec<Self::Chunk>> {
            let mut groups: pco_pack::ahash::HashMap<(), Vec<usize>> = Default::default();
            for (i, _) in data.iter().enumerate() {
                groups.entry(()).or_default().push(i);
            }
            let mut result: Vec<Chunk> = Vec::new();
            for (_key, indices) in groups {
                for group_indices in indices.chunks(OptionMapTupleRecord::CHUNK_SIZE) {
                    let mut col_id: Vec<i64> = Vec::new();
                    let mut col_metrics: Vec<Option<HashMap<String, (f32, i32)>>> = Vec::new();
                    for &idx in group_indices {
                        let rec = &data[idx];
                        col_id.push(rec.id.clone());
                        col_metrics.push(rec.metrics.clone());
                    }
                    let cb_id = <i64 as pco_pack::PcoSerde>::write(
                        col_id,
                        2u32 as u32,
                        Default::default(),
                    )?;
                    let cb_metrics = <Option<
                        HashMap<String, (f32, i32)>,
                    > as pco_pack::PcoSerde>::write(
                        col_metrics,
                        2u32 as u32,
                        Default::default(),
                    )?;
                    let rec = Chunk {
                        id: cb_id.into(),
                        metrics: cb_metrics.into(),
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
            let known: &[&str] = &["id", "metrics"];
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
            Ok(all_fields)
        }
    }
    #[inline]
    fn row_count(g: &mut Reader) -> pco_pack::anyhow::Result<usize> {
        let mut count = 0usize;
        if count == 0 {
            match g.id.row_count() {
                Ok(n) if n > 0 => count = n,
                Err(e) => return Err(e),
                _ => {}
            }
        }
        if count == 0 {
            match g.metrics.row_count() {
                Ok(n) if n > 0 => count = n,
                Err(e) => return Err(e),
                _ => {}
            }
        }
        Ok(count)
    }
    #[inline]
    fn expand_row(
        g: &mut Reader,
        row: usize,
    ) -> pco_pack::anyhow::Result<OptionMapTupleRecord> {
        Ok(OptionMapTupleRecord {
            id: g.id.pop_inner(row)?,
            metrics: g.metrics.pop_inner(row)?,
        })
    }
    impl Chunk {
        fn compute_row_count(
            reader: &mut Reader,
            fields: &[&str],
        ) -> pco_pack::anyhow::Result<usize> {
            let mut count = 0usize;
            if count == 0 && fields.contains(&"metrics") {
                let r = reader.metrics.row_count();
                match r {
                    Ok(n) if n > 0 => count = n,
                    Err(e) => return Err(e),
                    _ => {}
                }
            }
            if count == 0 && fields.contains(&"id") {
                let r = reader.id.row_count();
                match r {
                    Ok(n) if n > 0 => count = n,
                    Err(e) => return Err(e),
                    _ => {}
                }
            }
            let count = count;
            let has_loaded_payload = fields.contains(&"id")
                || fields.contains(&"metrics") || false;
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
        pub fn expand(self) -> pco_pack::anyhow::Result<Vec<OptionMapTupleRecord>> {
            let mut reader = Reader {
                id: <pco_pack::LazyReader<
                    i64,
                >>::new(self.id.to_vec(), 2u32, Default::default()),
                metrics: <pco_pack::LazyReader<
                    Option<HashMap<String, (f32, i32)>>,
                >>::new(self.metrics.to_vec(), 2u32, Default::default()),
            };
            let loaded_fields: &[&str] = &["id", "metrics"];
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
        ) -> pco_pack::anyhow::Result<Vec<OptionMapTupleRecord>> {
            let fields = <OptionMapTupleRecord as pco_pack::PcoPack>::resolve_fields(
                query,
                fields,
            )?;
            let execution_plan = <OptionMapTupleRecord as pco_pack::PcoFilter>::resolve_query(
                query,
            )?;
            let mut reader = Reader {
                id: <pco_pack::LazyReader<
                    i64,
                >>::new(
                    if fields.is_empty() || fields.contains(&"id") {
                        self.id.to_vec()
                    } else {
                        Vec::new()
                    },
                    2u32,
                    Default::default(),
                ),
                metrics: <pco_pack::LazyReader<
                    Option<HashMap<String, (f32, i32)>>,
                >>::new(
                    if fields.is_empty() || fields.contains(&"metrics") {
                        self.metrics.to_vec()
                    } else {
                        Vec::new()
                    },
                    2u32,
                    Default::default(),
                ),
            };
            let row_count = Self::compute_row_count(&mut reader, &fields)?;
            let mut matches = pco_pack::FilterMask::ones(row_count);
            for step in &execution_plan {
                <OptionMapTupleRecord as pco_pack::PcoFilter>::filter_step(
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
pub struct VecMapTupleRecord {
    id: i64,
    batches: Vec<HashMap<String, (half::f16, i32)>>,
}
const _: () = {
    #[allow(unused)]
    use pco_pack::anyhow::Context;
    use pco_pack::serde::Serialize;
    /// Intermediate compressed form for [VecMapTupleRecord] with metadata fields
    /// (index, timestamp bounds) uncompressed and payload columns
    /// stored as compressed ByteBuf. One instance per logical group.
    #[derive(pco_pack::serde::Serialize, pco_pack::serde::Deserialize, Default, Clone)]
    pub struct Chunk {
        #[serde(default)]
        pub id: serde_bytes::ByteBuf,
        #[serde(default)]
        pub batches: serde_bytes::ByteBuf,
    }
    #[derive(Clone, Default)]
    pub struct Reader {
        pub id: pco_pack::LazyReader<i64>,
        pub batches: pco_pack::LazyReader<Vec<HashMap<String, (half::f16, i32)>>>,
    }
    pub struct Writer {
        pub id: Vec<i64>,
        pub batches: Vec<Vec<HashMap<String, (half::f16, i32)>>>,
    }
    impl Default for Writer {
        fn default() -> Self {
            Self {
                id: Default::default(),
                batches: Default::default(),
            }
        }
    }
    /// Typed filter struct for [`#name`].
    #[derive(Clone, Default, pco_pack::serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Filter {
        pub id: Option<pco_pack::I64Filter>,
        #[serde(flatten)]
        others: pco_pack::serde_json::Map<String, pco_pack::serde_json::Value>,
    }
    impl Filter {
        /// Create a new filter with the specified index and timestamp constraints.
        /// Additional fields can be set via `Index<&str>` access on the returned instance.
        pub fn new() -> Self {
            Self { ..Default::default() }
        }
        /// Get a filter value for an arbitrary field (e.g., nested fields).
        pub fn get(&self, field: &str) -> Option<&pco_pack::serde_json::Value> {
            self.others.get(field)
        }
        /// Set a filter value for an arbitrary field.
        pub fn set(&mut self, field: &str, value: pco_pack::serde_json::Value) {
            self.others.insert(field.to_string(), value);
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
            if let Some(ref v) = value.id {
                map.insert("id".to_string(), pco_pack::serde_json::to_value(v)?);
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
    impl pco_pack::PcoSerde for VecMapTupleRecord {
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
            for (_key, indices) in groups {
                let mut col_id: Vec<i64> = Vec::new();
                let mut col_batches: Vec<Vec<HashMap<String, (half::f16, i32)>>> = Vec::new();
                for &idx in indices.iter() {
                    let rec = &data[idx];
                    col_id.push(rec.id.clone());
                    col_batches.push(rec.batches.clone());
                }
                let cb_id = <i64 as pco_pack::PcoSerde>::write(
                    col_id,
                    2u32 as u32,
                    Default::default(),
                )?;
                let cb_batches = <Vec<
                    HashMap<String, (half::f16, i32)>,
                > as pco_pack::PcoSerde>::write(
                    col_batches,
                    2u32 as u32,
                    Default::default(),
                )?;
                records
                    .push(Chunk {
                        id: cb_id.into(),
                        batches: cb_batches.into(),
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
                id: pco_pack::LazyReader::new(rec.id.to_vec(), 2u32, Default::default()),
                batches: pco_pack::LazyReader::new(
                    rec.batches.to_vec(),
                    2u32,
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
    impl pco_pack::PcoFilter for VecMapTupleRecord {
        fn filter_bulk(
            reader: &mut Self::Reader,
            field: usize,
            filter: &pco_pack::Filter,
            matches: &mut pco_pack::FilterMask,
        ) -> pco_pack::anyhow::Result<()> {
            if field == 0usize {
                return reader.id.filter_in_group(filter, matches);
            }
            if field == 1usize {
                return reader.batches.filter_in_group(filter, matches);
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
                    return reader.id.filter_nested(&path[1..], filter, matches);
                }
                1usize => {
                    return reader.batches.filter_nested(&path[1..], filter, matches);
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
                "id" => {
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
                "batches" => {
                    let sub_path = remainder.unwrap_or("");
                    let mut filter = <Vec<
                        HashMap<String, (half::f16, i32)>,
                    > as pco_pack::PcoFilter>::resolve_filter(sub_path, json)?;
                    if sub_path.is_empty() {
                        filter.path[0] = 1usize;
                    } else {
                        filter.path.insert(0, 1usize);
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
    impl pco_pack::PcoPack for VecMapTupleRecord {
        type Reader = Reader;
        type Chunk = Chunk;
        type Filter = Filter;
        fn write(data: Vec<Self>) -> pco_pack::anyhow::Result<Vec<Self::Chunk>> {
            let mut groups: pco_pack::ahash::HashMap<(), Vec<usize>> = Default::default();
            for (i, _) in data.iter().enumerate() {
                groups.entry(()).or_default().push(i);
            }
            let mut result: Vec<Chunk> = Vec::new();
            for (_key, indices) in groups {
                for group_indices in indices.chunks(VecMapTupleRecord::CHUNK_SIZE) {
                    let mut col_id: Vec<i64> = Vec::new();
                    let mut col_batches: Vec<Vec<HashMap<String, (half::f16, i32)>>> = Vec::new();
                    for &idx in group_indices {
                        let rec = &data[idx];
                        col_id.push(rec.id.clone());
                        col_batches.push(rec.batches.clone());
                    }
                    let cb_id = <i64 as pco_pack::PcoSerde>::write(
                        col_id,
                        2u32 as u32,
                        Default::default(),
                    )?;
                    let cb_batches = <Vec<
                        HashMap<String, (half::f16, i32)>,
                    > as pco_pack::PcoSerde>::write(
                        col_batches,
                        2u32 as u32,
                        Default::default(),
                    )?;
                    let rec = Chunk {
                        id: cb_id.into(),
                        batches: cb_batches.into(),
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
            let known: &[&str] = &["id", "batches"];
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
            Ok(all_fields)
        }
    }
    #[inline]
    fn row_count(g: &mut Reader) -> pco_pack::anyhow::Result<usize> {
        let mut count = 0usize;
        if count == 0 {
            match g.id.row_count() {
                Ok(n) if n > 0 => count = n,
                Err(e) => return Err(e),
                _ => {}
            }
        }
        if count == 0 {
            match g.batches.row_count() {
                Ok(n) if n > 0 => count = n,
                Err(e) => return Err(e),
                _ => {}
            }
        }
        Ok(count)
    }
    #[inline]
    fn expand_row(
        g: &mut Reader,
        row: usize,
    ) -> pco_pack::anyhow::Result<VecMapTupleRecord> {
        Ok(VecMapTupleRecord {
            id: g.id.pop_inner(row)?,
            batches: g.batches.pop_inner(row)?,
        })
    }
    impl Chunk {
        fn compute_row_count(
            reader: &mut Reader,
            fields: &[&str],
        ) -> pco_pack::anyhow::Result<usize> {
            let mut count = 0usize;
            if count == 0 && fields.contains(&"batches") {
                let r = reader.batches.row_count();
                match r {
                    Ok(n) if n > 0 => count = n,
                    Err(e) => return Err(e),
                    _ => {}
                }
            }
            if count == 0 && fields.contains(&"id") {
                let r = reader.id.row_count();
                match r {
                    Ok(n) if n > 0 => count = n,
                    Err(e) => return Err(e),
                    _ => {}
                }
            }
            let count = count;
            let has_loaded_payload = fields.contains(&"id")
                || fields.contains(&"batches") || false;
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
        pub fn expand(self) -> pco_pack::anyhow::Result<Vec<VecMapTupleRecord>> {
            let mut reader = Reader {
                id: <pco_pack::LazyReader<
                    i64,
                >>::new(self.id.to_vec(), 2u32, Default::default()),
                batches: <pco_pack::LazyReader<
                    Vec<HashMap<String, (half::f16, i32)>>,
                >>::new(self.batches.to_vec(), 2u32, Default::default()),
            };
            let loaded_fields: &[&str] = &["id", "batches"];
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
        ) -> pco_pack::anyhow::Result<Vec<VecMapTupleRecord>> {
            let fields = <VecMapTupleRecord as pco_pack::PcoPack>::resolve_fields(
                query,
                fields,
            )?;
            let execution_plan = <VecMapTupleRecord as pco_pack::PcoFilter>::resolve_query(
                query,
            )?;
            let mut reader = Reader {
                id: <pco_pack::LazyReader<
                    i64,
                >>::new(
                    if fields.is_empty() || fields.contains(&"id") {
                        self.id.to_vec()
                    } else {
                        Vec::new()
                    },
                    2u32,
                    Default::default(),
                ),
                batches: <pco_pack::LazyReader<
                    Vec<HashMap<String, (half::f16, i32)>>,
                >>::new(
                    if fields.is_empty() || fields.contains(&"batches") {
                        self.batches.to_vec()
                    } else {
                        Vec::new()
                    },
                    2u32,
                    Default::default(),
                ),
            };
            let row_count = Self::compute_row_count(&mut reader, &fields)?;
            let mut matches = pco_pack::FilterMask::ones(row_count);
            for step in &execution_plan {
                <VecMapTupleRecord as pco_pack::PcoFilter>::filter_step(
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
