pub struct Outer {
    name: String,
    inner: Inner,
}
const _: () = {
    #[allow(unused)]
    use pco_pack::anyhow::Context;
    use pco_pack::serde::Serialize;
    /// Intermediate compressed form for [Outer] with metadata fields
    /// (index, timestamp bounds) uncompressed and payload columns
    /// stored as compressed ByteBuf. One instance per logical group.
    #[derive(pco_pack::serde::Serialize, pco_pack::serde::Deserialize, Default, Clone)]
    pub struct Chunk {
        #[serde(default)]
        pub name: pco_pack::serde_bytes::ByteBuf,
        #[serde(default)]
        pub inner: pco_pack::serde_bytes::ByteBuf,
    }
    #[derive(Clone, Default)]
    pub struct Reader {
        pub name: pco_pack::LazyReader<String>,
        pub inner: pco_pack::LazyReader<Inner>,
    }
    pub struct Writer {
        pub name: Vec<String>,
        pub inner: Vec<Inner>,
    }
    impl Default for Writer {
        fn default() -> Self {
            Self {
                name: Default::default(),
                inner: Default::default(),
            }
        }
    }
    /// Typed filter struct for [`#name`].
    #[derive(Clone, Default, pco_pack::serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Filter {
        pub name: Option<pco_pack::StringFilter>,
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
            if let Some(ref v) = value.name {
                map.insert("name".to_string(), pco_pack::serde_json::to_value(v)?);
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
    impl pco_pack::PcoSerde for Outer {
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
                let mut col_name: Vec<String> = Vec::new();
                let mut col_inner: Vec<Inner> = Vec::new();
                for &idx in indices.iter() {
                    let rec = &data[idx];
                    col_name.push(rec.name.clone());
                    col_inner.push(rec.inner.clone());
                }
                let cb_name = <String as pco_pack::PcoSerde>::write(
                    col_name,
                    0u32 as u32,
                    Default::default(),
                )?;
                let cb_inner = <Inner as pco_pack::PcoSerde>::write(
                    col_inner,
                    0u32 as u32,
                    Default::default(),
                )?;
                records
                    .push(Chunk {
                        name: cb_name.into(),
                        inner: cb_inner.into(),
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
                name: pco_pack::LazyReader::new(
                    rec.name.to_vec(),
                    0u32,
                    Default::default(),
                ),
                inner: pco_pack::LazyReader::new(
                    rec.inner.to_vec(),
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
    impl pco_pack::PcoFilter for Outer {
        fn filter_bulk(
            reader: &mut Self::Reader,
            field: usize,
            filter: &pco_pack::Filter,
            matches: &mut pco_pack::FilterMask,
        ) -> pco_pack::anyhow::Result<()> {
            if field == 0usize {
                return reader.name.filter_in_group(filter, matches);
            }
            if field == 1usize {
                return reader.inner.filter_in_group(filter, matches);
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
                    return reader.name.filter_nested(&path[1..], filter, matches);
                }
                1usize => {
                    return reader.inner.filter_nested(&path[1..], filter, matches);
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
                    let mut filter = <String as pco_pack::PcoFilter>::resolve_filter(
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
                "name" => {
                    let sub_path = remainder.unwrap_or("");
                    let mut filter = <String as pco_pack::PcoFilter>::resolve_filter(
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
                "inner" => {
                    let sub_path = remainder.unwrap_or("");
                    let mut filter = <Inner as pco_pack::PcoFilter>::resolve_filter(
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
    impl pco_pack::PcoPack for Outer {
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
                for group_indices in indices.chunks(Outer::CHUNK_SIZE) {
                    let mut col_name: Vec<String> = Vec::new();
                    let mut col_inner: Vec<Inner> = Vec::new();
                    for &idx in group_indices {
                        let rec = &data[idx];
                        col_name.push(rec.name.clone());
                        col_inner.push(rec.inner.clone());
                    }
                    let cb_name = <String as pco_pack::PcoSerde>::write(
                        col_name,
                        0u32 as u32,
                        Default::default(),
                    )?;
                    let cb_inner = <Inner as pco_pack::PcoSerde>::write(
                        col_inner,
                        0u32 as u32,
                        Default::default(),
                    )?;
                    let rec = Chunk {
                        name: cb_name.into(),
                        inner: cb_inner.into(),
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
            let known: &[&str] = &["name", "inner"];
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
            match g.name.row_count() {
                Ok(n) if n > 0 => count = n,
                Err(e) => return Err(e),
                _ => {}
            }
        }
        if count == 0 {
            match g.inner.row_count() {
                Ok(n) if n > 0 => count = n,
                Err(e) => return Err(e),
                _ => {}
            }
        }
        Ok(count)
    }
    #[inline]
    fn expand_row(g: &mut Reader, row: usize) -> pco_pack::anyhow::Result<Outer> {
        Ok(Outer {
            name: g.name.pop_inner(row)?,
            inner: g.inner.pop_inner(row)?,
        })
    }
    impl Chunk {
        fn compute_row_count(
            reader: &mut Reader,
            fields: &[&str],
        ) -> pco_pack::anyhow::Result<usize> {
            let mut count = 0usize;
            if count == 0 && fields.contains(&"inner") {
                let r = reader.inner.row_count();
                match r {
                    Ok(n) if n > 0 => count = n,
                    Err(e) => return Err(e),
                    _ => {}
                }
            }
            if count == 0 && fields.contains(&"name") {
                let r = reader.name.row_count();
                match r {
                    Ok(n) if n > 0 => count = n,
                    Err(e) => return Err(e),
                    _ => {}
                }
            }
            let count = count;
            let has_loaded_payload = fields.contains(&"name")
                || fields.contains(&"inner") || false;
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
        pub fn expand(self) -> pco_pack::anyhow::Result<Vec<Outer>> {
            let mut reader = Reader {
                name: <pco_pack::LazyReader<
                    String,
                >>::new(self.name.to_vec(), 0u32, Default::default()),
                inner: <pco_pack::LazyReader<
                    Inner,
                >>::new(self.inner.to_vec(), 0u32, Default::default()),
            };
            let loaded_fields: &[&str] = &["name", "inner"];
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
        ) -> pco_pack::anyhow::Result<Vec<Outer>> {
            let fields = <Outer as pco_pack::PcoPack>::resolve_fields(query, fields)?;
            let execution_plan = <Outer as pco_pack::PcoFilter>::resolve_query(query)?;
            let mut reader = Reader {
                name: <pco_pack::LazyReader<
                    String,
                >>::new(
                    if fields.is_empty() || fields.contains(&"name") {
                        self.name.to_vec()
                    } else {
                        Vec::new()
                    },
                    0u32,
                    Default::default(),
                ),
                inner: <pco_pack::LazyReader<
                    Inner,
                >>::new(
                    if fields.is_empty() || fields.contains(&"inner") {
                        self.inner.to_vec()
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
            let sizes = [self.name.len(), self.inner.len()];
            plan.sort_by_key(|&i| sizes[execution_plan[i].path[0]]);
            for i in plan {
                <Outer as pco_pack::PcoFilter>::filter_step(
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
pub struct Inner {
    x: i32,
    label: String,
}
const _: () = {
    #[allow(unused)]
    use pco_pack::anyhow::Context;
    use pco_pack::serde::Serialize;
    /// Intermediate compressed form for [Inner] with metadata fields
    /// (index, timestamp bounds) uncompressed and payload columns
    /// stored as compressed ByteBuf. One instance per logical group.
    #[derive(pco_pack::serde::Serialize, pco_pack::serde::Deserialize, Default, Clone)]
    pub struct Chunk {
        #[serde(default)]
        pub x: pco_pack::serde_bytes::ByteBuf,
        #[serde(default)]
        pub label: pco_pack::serde_bytes::ByteBuf,
    }
    #[derive(Clone, Default)]
    pub struct Reader {
        pub x: pco_pack::LazyReader<i32>,
        pub label: pco_pack::LazyReader<String>,
    }
    pub struct Writer {
        pub x: Vec<i32>,
        pub label: Vec<String>,
    }
    impl Default for Writer {
        fn default() -> Self {
            Self {
                x: Default::default(),
                label: Default::default(),
            }
        }
    }
    /// Typed filter struct for [`#name`].
    #[derive(Clone, Default, pco_pack::serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Filter {
        pub x: Option<pco_pack::I64Filter>,
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
            if let Some(ref v) = value.x {
                map.insert("x".to_string(), pco_pack::serde_json::to_value(v)?);
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
    impl pco_pack::PcoSerde for Inner {
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
                let mut col_x: Vec<i32> = Vec::new();
                let mut col_label: Vec<String> = Vec::new();
                for &idx in indices.iter() {
                    let rec = &data[idx];
                    col_x.push(rec.x.clone());
                    col_label.push(rec.label.clone());
                }
                let cb_x = <i32 as pco_pack::PcoSerde>::write(
                    col_x,
                    0u32 as u32,
                    Default::default(),
                )?;
                let cb_label = <String as pco_pack::PcoSerde>::write(
                    col_label,
                    0u32 as u32,
                    Default::default(),
                )?;
                records
                    .push(Chunk {
                        x: cb_x.into(),
                        label: cb_label.into(),
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
                x: pco_pack::LazyReader::new(rec.x.to_vec(), 0u32, Default::default()),
                label: pco_pack::LazyReader::new(
                    rec.label.to_vec(),
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
    impl pco_pack::PcoFilter for Inner {
        fn filter_bulk(
            reader: &mut Self::Reader,
            field: usize,
            filter: &pco_pack::Filter,
            matches: &mut pco_pack::FilterMask,
        ) -> pco_pack::anyhow::Result<()> {
            if field == 0usize {
                return reader.x.filter_in_group(filter, matches);
            }
            if field == 1usize {
                return reader.label.filter_in_group(filter, matches);
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
                    return reader.x.filter_nested(&path[1..], filter, matches);
                }
                1usize => {
                    return reader.label.filter_nested(&path[1..], filter, matches);
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
                    let mut filter = <i32 as pco_pack::PcoFilter>::resolve_filter(
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
                "x" => {
                    let sub_path = remainder.unwrap_or("");
                    let mut filter = <i32 as pco_pack::PcoFilter>::resolve_filter(
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
                "label" => {
                    let sub_path = remainder.unwrap_or("");
                    let mut filter = <String as pco_pack::PcoFilter>::resolve_filter(
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
    impl pco_pack::PcoPack for Inner {
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
                for group_indices in indices.chunks(Inner::CHUNK_SIZE) {
                    let mut col_x: Vec<i32> = Vec::new();
                    let mut col_label: Vec<String> = Vec::new();
                    for &idx in group_indices {
                        let rec = &data[idx];
                        col_x.push(rec.x.clone());
                        col_label.push(rec.label.clone());
                    }
                    let cb_x = <i32 as pco_pack::PcoSerde>::write(
                        col_x,
                        0u32 as u32,
                        Default::default(),
                    )?;
                    let cb_label = <String as pco_pack::PcoSerde>::write(
                        col_label,
                        0u32 as u32,
                        Default::default(),
                    )?;
                    let rec = Chunk {
                        x: cb_x.into(),
                        label: cb_label.into(),
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
            let known: &[&str] = &["x", "label"];
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
            match g.x.row_count() {
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
        Ok(count)
    }
    #[inline]
    fn expand_row(g: &mut Reader, row: usize) -> pco_pack::anyhow::Result<Inner> {
        Ok(Inner {
            x: g.x.pop_inner(row)?,
            label: g.label.pop_inner(row)?,
        })
    }
    impl Chunk {
        fn compute_row_count(
            reader: &mut Reader,
            fields: &[&str],
        ) -> pco_pack::anyhow::Result<usize> {
            let mut count = 0usize;
            if count == 0 && fields.contains(&"label") {
                let r = reader.label.row_count();
                match r {
                    Ok(n) if n > 0 => count = n,
                    Err(e) => return Err(e),
                    _ => {}
                }
            }
            if count == 0 && fields.contains(&"x") {
                let r = reader.x.row_count();
                match r {
                    Ok(n) if n > 0 => count = n,
                    Err(e) => return Err(e),
                    _ => {}
                }
            }
            let count = count;
            let has_loaded_payload = fields.contains(&"x") || fields.contains(&"label")
                || false;
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
        pub fn expand(self) -> pco_pack::anyhow::Result<Vec<Inner>> {
            let mut reader = Reader {
                x: <pco_pack::LazyReader<
                    i32,
                >>::new(self.x.to_vec(), 0u32, Default::default()),
                label: <pco_pack::LazyReader<
                    String,
                >>::new(self.label.to_vec(), 0u32, Default::default()),
            };
            let loaded_fields: &[&str] = &["x", "label"];
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
        ) -> pco_pack::anyhow::Result<Vec<Inner>> {
            let fields = <Inner as pco_pack::PcoPack>::resolve_fields(query, fields)?;
            let execution_plan = <Inner as pco_pack::PcoFilter>::resolve_query(query)?;
            let mut reader = Reader {
                x: <pco_pack::LazyReader<
                    i32,
                >>::new(
                    if fields.is_empty() || fields.contains(&"x") {
                        self.x.to_vec()
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
            };
            let row_count = Self::compute_row_count(&mut reader, &fields)?;
            let mut matches = pco_pack::FilterMask::ones(row_count);
            let mut plan: Vec<usize> = (0..execution_plan.len()).collect();
            let sizes = [self.x.len(), self.label.len()];
            plan.sort_by_key(|&i| sizes[execution_plan[i].path[0]]);
            for i in plan {
                <Inner as pco_pack::PcoFilter>::filter_step(
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
