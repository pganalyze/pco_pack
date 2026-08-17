pub struct Task {
    id: i64,
    value: f64,
    label: String,
    state: State,
}
const _: () = {
    #[allow(unused)]
    use pco_pack::anyhow::Context;
    use pco_pack::serde::Serialize;
    /// Intermediate compressed form for [Task] with metadata fields
    /// (index, timestamp bounds) uncompressed and payload columns
    /// stored as compressed ByteBuf. One instance per logical group.
    #[derive(pco_pack::serde::Serialize, pco_pack::serde::Deserialize, Default, Clone)]
    pub struct Chunk {
        #[serde(default)]
        pub id: pco_pack::serde_bytes::ByteBuf,
        #[serde(default)]
        pub value: pco_pack::serde_bytes::ByteBuf,
        #[serde(default)]
        pub label: pco_pack::serde_bytes::ByteBuf,
        #[serde(default)]
        pub state: pco_pack::serde_bytes::ByteBuf,
    }
    #[derive(Clone, Default)]
    pub struct Reader {
        pub id: pco_pack::LazyReader<i64>,
        pub value: pco_pack::LazyReader<f64>,
        pub label: pco_pack::LazyReader<String>,
        pub state: pco_pack::LazyReader<State>,
    }
    pub struct Writer {
        pub id: Vec<i64>,
        pub value: Vec<f64>,
        pub label: Vec<String>,
        pub state: Vec<State>,
    }
    impl Default for Writer {
        fn default() -> Self {
            Self {
                id: Default::default(),
                value: Default::default(),
                label: Default::default(),
                state: Default::default(),
            }
        }
    }
    /// Typed filter struct for [`#name`].
    #[derive(Clone, Default, pco_pack::serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Filter {
        pub id: Option<pco_pack::I64Filter>,
        pub value: Option<pco_pack::F64Filter>,
        pub label: Option<pco_pack::StringFilter>,
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
            if let Some(ref v) = value.value {
                map.insert("value".to_string(), pco_pack::serde_json::to_value(v)?);
            }
            if let Some(ref v) = value.label {
                map.insert("label".to_string(), pco_pack::serde_json::to_value(v)?);
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
    impl pco_pack::PcoSerde for Task {
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
                let mut col_value: Vec<f64> = Vec::new();
                let mut col_label: Vec<String> = Vec::new();
                let mut col_state: Vec<State> = Vec::new();
                for &idx in indices.iter() {
                    let rec = &data[idx];
                    col_id.push(rec.id.clone());
                    col_value.push(rec.value.clone());
                    col_label.push(rec.label.clone());
                    col_state.push(rec.state.clone());
                }
                let cb_id = <i64 as pco_pack::PcoSerde>::write(
                    col_id,
                    0u32 as u32,
                    Default::default(),
                )?;
                let cb_value = <f64 as pco_pack::PcoSerde>::write(
                    col_value,
                    0u32 as u32,
                    Default::default(),
                )?;
                let cb_label = <String as pco_pack::PcoSerde>::write(
                    col_label,
                    0u32 as u32,
                    Default::default(),
                )?;
                let cb_state = <State as pco_pack::PcoSerde>::write(
                    col_state,
                    0u32 as u32,
                    Default::default(),
                )?;
                records
                    .push(Chunk {
                        id: cb_id.into(),
                        value: cb_value.into(),
                        label: cb_label.into(),
                        state: cb_state.into(),
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
                id: pco_pack::LazyReader::new(rec.id.to_vec(), 0u32, Default::default()),
                value: pco_pack::LazyReader::new(
                    rec.value.to_vec(),
                    0u32,
                    Default::default(),
                ),
                label: pco_pack::LazyReader::new(
                    rec.label.to_vec(),
                    0u32,
                    Default::default(),
                ),
                state: pco_pack::LazyReader::new(
                    rec.state.to_vec(),
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
    impl pco_pack::PcoFilter for Task {
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
                return reader.value.filter_in_group(filter, matches);
            }
            if field == 2usize {
                return reader.label.filter_in_group(filter, matches);
            }
            if field == 3usize {
                return reader.state.filter_in_group(filter, matches);
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
                    return reader.value.filter_nested(&path[1..], filter, matches);
                }
                2usize => {
                    return reader.label.filter_nested(&path[1..], filter, matches);
                }
                3usize => {
                    return reader.state.filter_nested(&path[1..], filter, matches);
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
                "value" => {
                    let sub_path = remainder.unwrap_or("");
                    let mut filter = <f64 as pco_pack::PcoFilter>::resolve_filter(
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
                "label" => {
                    let sub_path = remainder.unwrap_or("");
                    let mut filter = <String as pco_pack::PcoFilter>::resolve_filter(
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
                "state" => {
                    let sub_path = remainder.unwrap_or("");
                    let mut filter = <State as pco_pack::PcoFilter>::resolve_filter(
                        sub_path,
                        json,
                    )?;
                    if sub_path.is_empty() {
                        filter.path[0] = 3usize;
                    } else {
                        filter.path.insert(0, 3usize);
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
    impl pco_pack::PcoPack for Task {
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
                for group_indices in indices.chunks(Task::CHUNK_SIZE) {
                    let mut col_id: Vec<i64> = Vec::new();
                    let mut col_value: Vec<f64> = Vec::new();
                    let mut col_label: Vec<String> = Vec::new();
                    let mut col_state: Vec<State> = Vec::new();
                    for &idx in group_indices {
                        let rec = &data[idx];
                        col_id.push(rec.id.clone());
                        col_value.push(rec.value.clone());
                        col_label.push(rec.label.clone());
                        col_state.push(rec.state.clone());
                    }
                    let cb_id = <i64 as pco_pack::PcoSerde>::write(
                        col_id,
                        0u32 as u32,
                        Default::default(),
                    )?;
                    let cb_value = <f64 as pco_pack::PcoSerde>::write(
                        col_value,
                        0u32 as u32,
                        Default::default(),
                    )?;
                    let cb_label = <String as pco_pack::PcoSerde>::write(
                        col_label,
                        0u32 as u32,
                        Default::default(),
                    )?;
                    let cb_state = <State as pco_pack::PcoSerde>::write(
                        col_state,
                        0u32 as u32,
                        Default::default(),
                    )?;
                    let rec = Chunk {
                        id: cb_id.into(),
                        value: cb_value.into(),
                        label: cb_label.into(),
                        state: cb_state.into(),
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
            let known: &[&str] = &["id", "value", "label", "state"];
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
            match g.value.row_count() {
                Ok(n) if n > 0 => count = n,
                Err(e) => return Err(e),
                _ => {}
            }
        }
        if count == 0 {
            match g.label.row_count() {
                Ok(n) if n > 0 => count = n,
                Err(e) => return Err(e),
                _ => {}
            }
        }
        if count == 0 {
            match g.state.row_count() {
                Ok(n) if n > 0 => count = n,
                Err(e) => return Err(e),
                _ => {}
            }
        }
        Ok(count)
    }
    #[inline]
    fn expand_row(g: &mut Reader, row: usize) -> pco_pack::anyhow::Result<Task> {
        Ok(Task {
            id: g.id.pop_inner(row)?,
            value: g.value.pop_inner(row)?,
            label: g.label.pop_inner(row)?,
            state: g.state.pop_inner(row)?,
        })
    }
    impl Chunk {
        fn compute_row_count(
            reader: &mut Reader,
            fields: &[&str],
        ) -> pco_pack::anyhow::Result<usize> {
            let mut count = 0usize;
            if count == 0 && fields.contains(&"state") {
                let r = reader.state.row_count();
                match r {
                    Ok(n) if n > 0 => count = n,
                    Err(e) => return Err(e),
                    _ => {}
                }
            }
            if count == 0 && fields.contains(&"label") {
                let r = reader.label.row_count();
                match r {
                    Ok(n) if n > 0 => count = n,
                    Err(e) => return Err(e),
                    _ => {}
                }
            }
            if count == 0 && fields.contains(&"value") {
                let r = reader.value.row_count();
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
            let has_loaded_payload = fields.contains(&"id") || fields.contains(&"value")
                || fields.contains(&"label") || fields.contains(&"state") || false;
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
        pub fn expand(self) -> pco_pack::anyhow::Result<Vec<Task>> {
            let mut reader = Reader {
                id: <pco_pack::LazyReader<
                    i64,
                >>::new(self.id.to_vec(), 0u32, Default::default()),
                value: <pco_pack::LazyReader<
                    f64,
                >>::new(self.value.to_vec(), 0u32, Default::default()),
                label: <pco_pack::LazyReader<
                    String,
                >>::new(self.label.to_vec(), 0u32, Default::default()),
                state: <pco_pack::LazyReader<
                    State,
                >>::new(self.state.to_vec(), 0u32, Default::default()),
            };
            let loaded_fields: &[&str] = &["id", "value", "label", "state"];
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
        ) -> pco_pack::anyhow::Result<Vec<Task>> {
            let fields = <Task as pco_pack::PcoPack>::resolve_fields(query, fields)?;
            let execution_plan = <Task as pco_pack::PcoFilter>::resolve_query(query)?;
            let mut reader = Reader {
                id: <pco_pack::LazyReader<
                    i64,
                >>::new(
                    if fields.is_empty() || fields.contains(&"id") {
                        self.id.to_vec()
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
                label: <pco_pack::LazyReader<
                    String,
                >>::new(
                    if fields.is_empty() || fields.contains(&"label") {
                        self.label.to_vec()
                    } else {
                        Vec::new()
                    },
                    0u32,
                    Default::default(),
                ),
                state: <pco_pack::LazyReader<
                    State,
                >>::new(
                    if fields.is_empty() || fields.contains(&"state") {
                        self.state.to_vec()
                    } else {
                        Vec::new()
                    },
                    0u32,
                    Default::default(),
                ),
            };
            let row_count = Self::compute_row_count(&mut reader, &fields)?;
            let mut matches = pco_pack::FilterMask::ones(row_count);
            let mut plan: Vec<usize> = (0..execution_plan.len()).collect();
            let sizes = [
                self.id.len(),
                self.value.len(),
                self.label.len(),
                self.state.len(),
            ];
            plan.sort_by_key(|&i| sizes[execution_plan[i].path[0]]);
            for i in plan {
                <Task as pco_pack::PcoFilter>::filter_step(
                    &mut reader,
                    &execution_plan[i].path,
                    &execution_plan[i].filter,
                    &mut matches,
                )?;
                if !matches.any_set_in_range(0..row_count) {
                    return Ok(Vec::new());
                }
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
#[derive(Clone, Default)]
pub enum State {
    #[default]
    Idle,
    Running(i32),
    Paused,
}
const _: () = {
    #[allow(unused)]
    use pco_pack::anyhow::Context;
    use pco_pack::serde::Serialize;
    #[doc = concat!("Writer container for accumulating ", stringify!(State), " rows.")]
    pub struct Writer {
        row_variant: pco_pack::NumberWriter<i64>,
        variant_1: Vec<i32>,
    }
    impl Default for Writer {
        fn default() -> Self {
            Self {
                row_variant: pco_pack::NumberWriter::default(),
                variant_1: Vec::new(),
            }
        }
    }
    #[doc = concat!("Reader state for ", stringify!(State), " data.")]
    #[derive(Clone, Default)]
    pub struct Reader {
        row_variant: pco_pack::NumberReader<i64>,
        variant_1: pco_pack::LazyReader<i32>,
        variant_1_index: usize,
    }
    impl pco_pack::PcoSerde for State {
        type Writer = Writer;
        type Reader = Reader;
        fn write(
            data: Vec<Self>,
            float_round: u32,
            time_round: pco_pack::chrono::Duration,
        ) -> pco_pack::anyhow::Result<Vec<u8>> {
            let mut writer = Writer::default();
            for item in data {
                match item {
                    Self::Idle => {
                        writer.row_variant.values.push(0i64 as i64);
                    }
                    Self::Running(inner) => {
                        writer.row_variant.values.push(1i64 as i64);
                        writer.variant_1.push(inner);
                    }
                    Self::Paused => {
                        writer.row_variant.values.push(2i64 as i64);
                    }
                }
            }
            let mut out = Vec::new();
            out.extend_from_slice(
                &<i64 as pco_pack::PcoSerde>::write(
                    writer.row_variant.values,
                    float_round,
                    time_round,
                )?,
            );
            let non_unit_count: usize = 1usize;
            out.extend_from_slice(&(non_unit_count as u64).to_le_bytes());
            if !writer.variant_1.is_empty() {
                let compressed = <i32 as pco_pack::PcoSerde>::write(
                    writer.variant_1,
                    float_round,
                    time_round,
                )?;
                out.extend_from_slice(&(compressed.len() as u64).to_le_bytes());
                out.extend_from_slice(&compressed);
            } else {
                out.extend_from_slice(&(0u64).to_le_bytes());
            }
            Ok(out)
        }
        fn read(
            src: &mut std::io::Cursor<&[u8]>,
            _float_round: u32,
            _time_round: pco_pack::chrono::Duration,
        ) -> pco_pack::anyhow::Result<Self::Reader> {
            let row_variant = <i64 as pco_pack::PcoSerde>::read(
                src,
                0,
                Default::default(),
            )?;
            let mut len_buf = [0u8; 8];
            <std::io::Cursor<&[u8]> as std::io::Read>::read_exact(src, &mut len_buf)?;
            let _num_variants = u64::from_le_bytes(len_buf) as usize;
            let mut _block_count = _num_variants;
            let variant_1 = {
                let mut block_buf = [0u8; 8];
                <std::io::Cursor<
                    &[u8],
                > as std::io::Read>::read_exact(src, &mut block_buf)?;
                let block_len = u64::from_le_bytes(block_buf) as usize;
                if block_len > 0 {
                    let current_pos = src.position() as usize;
                    let compressed = src
                        .get_ref()[current_pos..current_pos + block_len]
                        .to_vec();
                    src.set_position((current_pos + block_len) as u64);
                    pco_pack::LazyReader::new(
                        compressed,
                        _float_round,
                        _time_round.clone(),
                    )
                } else {
                    pco_pack::LazyReader::new(
                        Vec::new(),
                        _float_round,
                        _time_round.clone(),
                    )
                }
            };
            _block_count -= 1;
            Ok(Reader {
                row_variant,
                variant_1,
                variant_1_index: 0,
            })
        }
        fn validate_bounds(
            reader: &mut Self::Reader,
        ) -> pco_pack::anyhow::Result<Option<usize>> {
            Ok(Some(reader.row_variant.values.len()))
        }
        fn get(
            reader: &mut Self::Reader,
            index: usize,
        ) -> pco_pack::anyhow::Result<Option<Self>> {
            let discriminant = <i64 as pco_pack::PcoSerde>::get(
                &mut reader.row_variant,
                index,
            )?;
            match discriminant {
                Some(0i64) => Ok(Some(Self::Idle)),
                Some(1i64) => {
                    let idx = reader.variant_1_index;
                    reader.variant_1_index += 1;
                    return Ok(Some(Self::Running(reader.variant_1.pop_inner(idx)?)));
                }
                Some(2i64) => Ok(Some(Self::Paused)),
                None | Some(_) => Ok(Some(Self::default())),
            }
        }
    }
    impl pco_pack::PcoFilter for State {
        fn filter_bulk(
            reader: &mut Self::Reader,
            _field: usize,
            filter: &pco_pack::Filter,
            matches: &mut pco_pack::FilterMask,
        ) -> pco_pack::anyhow::Result<()> {
            <i64 as pco_pack::PcoFilter>::filter_bulk(
                &mut reader.row_variant,
                0,
                filter,
                matches,
            )?;
            Ok(())
        }
        fn filter_match(value: &Self, filter: &pco_pack::Filter) -> bool {
            match value {
                Self::Idle => {
                    match filter {
                        pco_pack::Filter::I64(target) => *target == 0i64,
                        _ => false,
                    }
                }
                Self::Running(_inner) => {
                    match filter {
                        pco_pack::Filter::I64(target) => *target == 1i64,
                        pco_pack::Filter::InclusionI64(targets) => {
                            targets.contains(&1i64)
                        }
                        _ => false,
                    }
                }
                Self::Paused => {
                    match filter {
                        pco_pack::Filter::I64(target) => *target == 2i64,
                        _ => false,
                    }
                }
            }
        }
        fn filter_nested(
            _reader: &mut Self::Reader,
            _path: &[usize],
            _filter: &pco_pack::Filter,
            _matches: &mut pco_pack::FilterMask,
        ) -> pco_pack::anyhow::Result<()> {
            unreachable!("filter_nested not supported for '{}'", stringify!(State));
        }
        fn resolve_filter(
            path: &str,
            json: &pco_pack::serde_json::Value,
        ) -> pco_pack::anyhow::Result<pco_pack::ResolvedFilter> {
            if path.is_empty() {
                if let pco_pack::serde_json::Value::Object(obj) = json {
                    if let (Some(start_val), Some(end_val)) = (
                        obj.get("start"),
                        obj.get("end"),
                    ) {
                        let start = start_val
                            .as_i64()
                            .context("Range start must be an integer")?;
                        let end = end_val
                            .as_i64()
                            .context("Range end must be an integer")?;
                        return Ok(pco_pack::ResolvedFilter {
                            path: vec![0],
                            filter: pco_pack::Filter::Range(start..=end),
                        });
                    }
                }
                if let pco_pack::serde_json::Value::Array(arr) = json {
                    if !arr.is_empty() {
                        let ints: Vec<i64> = arr
                            .iter()
                            .filter_map(|v| v.as_i64())
                            .collect();
                        if ints.len() == arr.len() {
                            return Ok(pco_pack::ResolvedFilter {
                                path: vec![0],
                                filter: pco_pack::Filter::InclusionI64(ints),
                            });
                        }
                    }
                }
                let discriminant: Option<i64> = json
                    .as_i64()
                    .or_else(|| json.as_f64().map(|f| f as i64));
                let discriminant = discriminant
                    .context("Expected i64 discriminant value for enum filter")?;
                return Ok(pco_pack::ResolvedFilter {
                    path: vec![0],
                    filter: pco_pack::Filter::I64(discriminant),
                });
            }
            Err(
                pco_pack::anyhow::anyhow!(
                    "Enum '{}' has no nested fields; cannot resolve path '{}'",
                    stringify!(State), path
                ),
            )
        }
    }
    impl pco_pack::PcoPack for State {
        type Reader = <Self as pco_pack::PcoSerde>::Reader;
        type Chunk = ::std::vec::Vec<u8>;
        type Filter = serde_json::Value;
        fn write(data: Vec<Self>) -> pco_pack::anyhow::Result<Vec<Self::Chunk>> {
            Ok(
                vec![
                    < Self as pco_pack::PcoSerde > ::write(data, 0u32,
                    Default::default()) ?
                ],
            )
        }
        fn read(chunks: Vec<Self::Chunk>) -> pco_pack::anyhow::Result<Vec<Self>> {
            let mut result = Vec::new();
            for chunk in chunks {
                let mut cursor = std::io::Cursor::new(chunk.as_slice());
                let mut reader = <Self as pco_pack::PcoSerde>::read(
                    &mut cursor,
                    0u32,
                    Default::default(),
                )?;
                let row_count = <Self as pco_pack::PcoSerde>::validate_bounds(
                        &mut reader,
                    )
                    .context("Invalid enum payload")?
                    .ok_or_else(|| pco_pack::anyhow::anyhow!("No rows in enum data"))?;
                for row_idx in 0..row_count {
                    result
                        .push(
                            <Self as pco_pack::PcoSerde>::get(&mut reader, row_idx)
                                .context("Unexpected end of data")?
                                .ok_or_else(|| {
                                    pco_pack::anyhow::anyhow!(
                                        "Missing enum variant at index {}", row_idx
                                    )
                                })?,
                        );
                }
            }
            Ok(result)
        }
        fn filter_value(
            chunks: &[Self::Chunk],
            query: &pco_pack::serde_json::Value,
            fields: &[&str],
        ) -> pco_pack::anyhow::Result<Vec<Self>> {
            if !fields.is_empty() {
                return Err(
                    pco_pack::anyhow::anyhow!(
                        "Enum '{}' does not support field matches", stringify!(State)
                    ),
                );
            }
            let all_bytes: Vec<u8> = chunks
                .iter()
                .flat_map(|b| b.iter().copied())
                .collect();
            if all_bytes.is_empty() {
                return Ok(Vec::new());
            }
            let mut cursor = std::io::Cursor::new(all_bytes.as_slice());
            let mut reader = <Self as pco_pack::PcoSerde>::read(
                &mut cursor,
                0u32,
                Default::default(),
            )?;
            let row_count: usize = <Self as pco_pack::PcoSerde>::validate_bounds(
                    &mut reader,
                )
                .context("Invalid enum payload")?
                .ok_or_else(|| pco_pack::anyhow::anyhow!("No rows in enum data"))?;
            let execution_plan = <Self as pco_pack::PcoFilter>::resolve_query(query)?;
            let mut matches = pco_pack::FilterMask::ones(row_count);
            for step in &execution_plan {
                <Self as pco_pack::PcoFilter>::filter_step(
                    &mut reader,
                    &step.path,
                    &step.filter,
                    &mut matches,
                )?;
                if !matches.any_set_in_range(0..row_count) {
                    return Ok(Vec::new());
                }
            }
            let mut result = Vec::with_capacity(matches.count_ones());
            {
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
                        result
                            .push(
                                <Self as pco_pack::PcoSerde>::get(
                                        &mut reader,
                                        actual_row_index,
                                    )
                                    .context("Unexpected end of data")?
                                    .ok_or_else(|| {
                                        pco_pack::anyhow::anyhow!(
                                            "Missing enum variant at index {}", actual_row_index
                                        )
                                    })?,
                            );
                        current_chunk &= current_chunk - 1;
                    }
                    base_row_index += 64;
                }
            }
            Ok(result)
        }
        fn to_bytes(chunks: &[Self::Chunk]) -> pco_pack::anyhow::Result<Vec<u8>> {
            let mut out = Vec::new();
            chunks
                .serialize(
                    &mut pco_pack::rmp_serde::Serializer::new(&mut out).with_struct_map(),
                )?;
            Ok(out)
        }
        fn from_bytes(bytes: &[u8]) -> pco_pack::anyhow::Result<Vec<Self::Chunk>> {
            Ok(pco_pack::rmp_serde::from_slice(bytes)?)
        }
        fn resolve_fields(
            _query: &pco_pack::serde_json::Value,
            _fields: &[&str],
        ) -> pco_pack::anyhow::Result<Vec<&'static str>> {
            Ok(Vec::new())
        }
    }
};
