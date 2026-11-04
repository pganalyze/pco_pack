#[derive(Clone)]
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
    #[serde(crate = "pco_pack::serde")]
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
    #[derive(Default)]
    pub struct Writer {
        pub id: Vec<i64>,
        pub value: Vec<f64>,
        pub label: Vec<String>,
        pub state: Vec<State>,
    }
    /// Typed view over one chunk's decompressed columns, with one accessor per field.
    /// Fields outside the loaded projection return `Default`, matching `filter` semantics.
    /// Rows are read in ascending order only, mirroring the forward-only `PcoSerde::get`.
    /// Chunk bounds (`start_at`/`end_at`) have no accessors; they reach consumers only
    /// through `hydrate` rows, never through `fold!` projections.
    pub struct Columns<'a> {
        reader: &'a mut Reader,
    }
    impl Columns<'_> {
        pub fn id(&mut self, row: usize) -> pco_pack::anyhow::Result<i64> {
            self.reader.id.pop_inner(row)
        }
        pub fn value(&mut self, row: usize) -> pco_pack::anyhow::Result<f64> {
            self.reader.value.pop_inner(row)
        }
        pub fn label(&mut self, row: usize) -> pco_pack::anyhow::Result<String> {
            self.reader.label.pop_inner(row)
        }
        pub fn state(&mut self, row: usize) -> pco_pack::anyhow::Result<State> {
            self.reader.state.pop_inner(row)
        }
    }
    /// Typed filter struct for [`#name`].
    #[derive(Clone, Default, Debug, pco_pack::serde::Deserialize)]
    #[serde(crate = "pco_pack::serde", deny_unknown_fields)]
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
        const CHEAP_TO_CLONE: bool = <i64 as pco_pack::PcoSerde>::CHEAP_TO_CLONE
            && <f64 as pco_pack::PcoSerde>::CHEAP_TO_CLONE
            && <String as pco_pack::PcoSerde>::CHEAP_TO_CLONE
            && <State as pco_pack::PcoSerde>::CHEAP_TO_CLONE;
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
                let mut colr_id: Vec<&i64> = Vec::new();
                let mut col_value: Vec<f64> = Vec::new();
                let mut colr_value: Vec<&f64> = Vec::new();
                let mut col_label: Vec<String> = Vec::new();
                let mut colr_label: Vec<&String> = Vec::new();
                let mut col_state: Vec<State> = Vec::new();
                let mut colr_state: Vec<&State> = Vec::new();
                for &idx in indices.iter() {
                    let rec = &data[idx];
                    if <i64 as pco_pack::PcoSerde>::CHEAP_TO_CLONE {
                        col_id.push(rec.id);
                    } else {
                        colr_id.push(&rec.id);
                    }
                    if <f64 as pco_pack::PcoSerde>::CHEAP_TO_CLONE {
                        col_value.push(rec.value);
                    } else {
                        colr_value.push(&rec.value);
                    }
                    if <String as pco_pack::PcoSerde>::CHEAP_TO_CLONE {
                        col_label.push(rec.label.clone());
                    } else {
                        colr_label.push(&rec.label);
                    }
                    if <State as pco_pack::PcoSerde>::CHEAP_TO_CLONE {
                        col_state.push(rec.state.clone());
                    } else {
                        colr_state.push(&rec.state);
                    }
                }
                let cb_id = if <i64 as pco_pack::PcoSerde>::CHEAP_TO_CLONE {
                    <i64 as pco_pack::PcoSerde>::write(col_id, 0u32, Default::default())?
                } else {
                    <i64 as pco_pack::PcoSerde>::write_borrowed(
                        &colr_id,
                        0u32,
                        Default::default(),
                    )?
                };
                let cb_value = if <f64 as pco_pack::PcoSerde>::CHEAP_TO_CLONE {
                    <f64 as pco_pack::PcoSerde>::write(
                        col_value,
                        0u32,
                        Default::default(),
                    )?
                } else {
                    <f64 as pco_pack::PcoSerde>::write_borrowed(
                        &colr_value,
                        0u32,
                        Default::default(),
                    )?
                };
                let cb_label = if <String as pco_pack::PcoSerde>::CHEAP_TO_CLONE {
                    <String as pco_pack::PcoSerde>::write(
                        col_label,
                        0u32,
                        Default::default(),
                    )?
                } else {
                    <String as pco_pack::PcoSerde>::write_borrowed(
                        &colr_label,
                        0u32,
                        Default::default(),
                    )?
                };
                let cb_state = if <State as pco_pack::PcoSerde>::CHEAP_TO_CLONE {
                    <State as pco_pack::PcoSerde>::write(
                        col_state,
                        0u32,
                        Default::default(),
                    )?
                } else {
                    <State as pco_pack::PcoSerde>::write_borrowed(
                        &colr_state,
                        0u32,
                        Default::default(),
                    )?
                };
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
        fn write_borrowed(
            data: &[&Self],
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
                let mut colr_id: Vec<&i64> = Vec::new();
                let mut col_value: Vec<f64> = Vec::new();
                let mut colr_value: Vec<&f64> = Vec::new();
                let mut col_label: Vec<String> = Vec::new();
                let mut colr_label: Vec<&String> = Vec::new();
                let mut col_state: Vec<State> = Vec::new();
                let mut colr_state: Vec<&State> = Vec::new();
                for &idx in indices.iter() {
                    let rec = &data[idx];
                    if <i64 as pco_pack::PcoSerde>::CHEAP_TO_CLONE {
                        col_id.push(rec.id);
                    } else {
                        colr_id.push(&rec.id);
                    }
                    if <f64 as pco_pack::PcoSerde>::CHEAP_TO_CLONE {
                        col_value.push(rec.value);
                    } else {
                        colr_value.push(&rec.value);
                    }
                    if <String as pco_pack::PcoSerde>::CHEAP_TO_CLONE {
                        col_label.push(rec.label.clone());
                    } else {
                        colr_label.push(&rec.label);
                    }
                    if <State as pco_pack::PcoSerde>::CHEAP_TO_CLONE {
                        col_state.push(rec.state.clone());
                    } else {
                        colr_state.push(&rec.state);
                    }
                }
                let cb_id = if <i64 as pco_pack::PcoSerde>::CHEAP_TO_CLONE {
                    <i64 as pco_pack::PcoSerde>::write(col_id, 0u32, Default::default())?
                } else {
                    <i64 as pco_pack::PcoSerde>::write_borrowed(
                        &colr_id,
                        0u32,
                        Default::default(),
                    )?
                };
                let cb_value = if <f64 as pco_pack::PcoSerde>::CHEAP_TO_CLONE {
                    <f64 as pco_pack::PcoSerde>::write(
                        col_value,
                        0u32,
                        Default::default(),
                    )?
                } else {
                    <f64 as pco_pack::PcoSerde>::write_borrowed(
                        &colr_value,
                        0u32,
                        Default::default(),
                    )?
                };
                let cb_label = if <String as pco_pack::PcoSerde>::CHEAP_TO_CLONE {
                    <String as pco_pack::PcoSerde>::write(
                        col_label,
                        0u32,
                        Default::default(),
                    )?
                } else {
                    <String as pco_pack::PcoSerde>::write_borrowed(
                        &colr_label,
                        0u32,
                        Default::default(),
                    )?
                };
                let cb_state = if <State as pco_pack::PcoSerde>::CHEAP_TO_CLONE {
                    <State as pco_pack::PcoSerde>::write(
                        col_state,
                        0u32,
                        Default::default(),
                    )?
                } else {
                    <State as pco_pack::PcoSerde>::write_borrowed(
                        &colr_state,
                        0u32,
                        Default::default(),
                    )?
                };
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
                bl_buf.copy_from_slice(&buf[pos..pos + 8]);
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
                id: pco_pack::LazyReader::new(
                    rec.id.into_vec(),
                    0u32,
                    Default::default(),
                ),
                value: pco_pack::LazyReader::new(
                    rec.value.into_vec(),
                    0u32,
                    Default::default(),
                ),
                label: pco_pack::LazyReader::new(
                    rec.label.into_vec(),
                    0u32,
                    Default::default(),
                ),
                state: pco_pack::LazyReader::new(
                    rec.state.into_vec(),
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
                0usize => reader.id.filter_nested(&path[1..], filter, matches),
                1usize => reader.value.filter_nested(&path[1..], filter, matches),
                2usize => reader.label.filter_nested(&path[1..], filter, matches),
                3usize => reader.state.filter_nested(&path[1..], filter, matches),
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
        type Columns<'a> = Columns<'a>;
        fn hydrate(
            columns: &mut Self::Columns<'_>,
            row: usize,
        ) -> pco_pack::anyhow::Result<Self> {
            expand_row(columns.reader, row)
        }
        fn columns(
            reader: &mut <Self as pco_pack::PcoSerde>::Reader,
        ) -> Self::Columns<'_> {
            Columns { reader }
        }
        fn fold_value<C>(
            chunks: &[Self::Chunk],
            query: &pco_pack::serde_json::Value,
            fields: &[&str],
            consumer: &mut C,
        ) -> pco_pack::anyhow::Result<()>
        where
            C: pco_pack::Consumer<Self>,
        {
            let mut base: Vec<&str> = fields.to_vec();
            base.extend_from_slice(<C as pco_pack::Consumer<Self>>::FIELDS);
            let fields = <Task as pco_pack::PcoPack>::resolve_fields(query, &base)?;
            let plan = <Task as pco_pack::PcoFilter>::resolve_query(query)?;
            Self::Chunk::validate_projection(&fields)?;
            let mut order: Vec<usize> = Vec::new();
            let mut matches = pco_pack::FilterMask::ones(0);
            for c in chunks {
                c.fold_chunk(&plan, &fields, &mut order, &mut matches, consumer)?;
            }
            Ok(())
        }
        fn write(data: Vec<Self>) -> pco_pack::anyhow::Result<Vec<Self::Chunk>> {
            let mut groups: pco_pack::ahash::HashMap<(), Vec<usize>> = Default::default();
            for (i, _) in data.iter().enumerate() {
                groups.entry(()).or_default().push(i);
            }
            let mut result: Vec<Chunk> = Vec::new();
            for (_key, indices) in groups {
                for group_indices in indices.chunks(Task::CHUNK_SIZE) {
                    let mut col_id: Vec<i64> = Vec::new();
                    let mut colr_id: Vec<&i64> = Vec::new();
                    let mut col_value: Vec<f64> = Vec::new();
                    let mut colr_value: Vec<&f64> = Vec::new();
                    let mut col_label: Vec<String> = Vec::new();
                    let mut colr_label: Vec<&String> = Vec::new();
                    let mut col_state: Vec<State> = Vec::new();
                    let mut colr_state: Vec<&State> = Vec::new();
                    for &idx in group_indices {
                        let rec = &data[idx];
                        if <i64 as pco_pack::PcoSerde>::CHEAP_TO_CLONE {
                            col_id.push(rec.id);
                        } else {
                            colr_id.push(&rec.id);
                        }
                        if <f64 as pco_pack::PcoSerde>::CHEAP_TO_CLONE {
                            col_value.push(rec.value);
                        } else {
                            colr_value.push(&rec.value);
                        }
                        if <String as pco_pack::PcoSerde>::CHEAP_TO_CLONE {
                            col_label.push(rec.label.clone());
                        } else {
                            colr_label.push(&rec.label);
                        }
                        if <State as pco_pack::PcoSerde>::CHEAP_TO_CLONE {
                            col_state.push(rec.state.clone());
                        } else {
                            colr_state.push(&rec.state);
                        }
                    }
                    let cb_id = if <i64 as pco_pack::PcoSerde>::CHEAP_TO_CLONE {
                        <i64 as pco_pack::PcoSerde>::write(
                            col_id,
                            0u32,
                            Default::default(),
                        )?
                    } else {
                        <i64 as pco_pack::PcoSerde>::write_borrowed(
                            &colr_id,
                            0u32,
                            Default::default(),
                        )?
                    };
                    let cb_value = if <f64 as pco_pack::PcoSerde>::CHEAP_TO_CLONE {
                        <f64 as pco_pack::PcoSerde>::write(
                            col_value,
                            0u32,
                            Default::default(),
                        )?
                    } else {
                        <f64 as pco_pack::PcoSerde>::write_borrowed(
                            &colr_value,
                            0u32,
                            Default::default(),
                        )?
                    };
                    let cb_label = if <String as pco_pack::PcoSerde>::CHEAP_TO_CLONE {
                        <String as pco_pack::PcoSerde>::write(
                            col_label,
                            0u32,
                            Default::default(),
                        )?
                    } else {
                        <String as pco_pack::PcoSerde>::write_borrowed(
                            &colr_label,
                            0u32,
                            Default::default(),
                        )?
                    };
                    let cb_state = if <State as pco_pack::PcoSerde>::CHEAP_TO_CLONE {
                        <State as pco_pack::PcoSerde>::write(
                            col_state,
                            0u32,
                            Default::default(),
                        )?
                    } else {
                        <State as pco_pack::PcoSerde>::write_borrowed(
                            &colr_state,
                            0u32,
                            Default::default(),
                        )?
                    };
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
                >>::new(self.id.into_vec(), 0u32, Default::default()),
                value: <pco_pack::LazyReader<
                    f64,
                >>::new(self.value.into_vec(), 0u32, Default::default()),
                label: <pco_pack::LazyReader<
                    String,
                >>::new(self.label.into_vec(), 0u32, Default::default()),
                state: <pco_pack::LazyReader<
                    State,
                >>::new(self.state.into_vec(), 0u32, Default::default()),
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
            matches
                .for_each_set_row(
                    row_count,
                    |actual_row_index| {
                        results.push(expand_row(&mut reader, actual_row_index)?);
                        Ok(())
                    },
                )?;
            Ok(results)
        }
        /// Validates a projection: an unloaded payload column reads back as
        /// `Default`, so an unresolved or misspelled field name must not silently
        /// become zeroed values.
        pub fn validate_projection(fields: &[&str]) -> pco_pack::anyhow::Result<()> {
            let known: &[&str] = &["id", "value", "label", "state"];
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
            Ok(())
        }
        /// Streams this chunk's matching rows into a consumer, mirroring `filter`
        /// but never materializing a row vector or a full row struct. `plan` and
        /// `fields` are the outputs of `PcoFilter::resolve_query` and
        /// `PcoPack::resolve_fields`; direct callers use this one-shot path, while
        /// `for_each` resolves once per stream and reuses scratch buffers via
        /// `fold_chunk`.
        pub fn fold_into<C>(
            &self,
            plan: &[pco_pack::ResolvedFilter],
            fields: &[&str],
            consumer: &mut C,
        ) -> pco_pack::anyhow::Result<()>
        where
            C: pco_pack::Consumer<Task>,
        {
            Self::validate_projection(fields)?;
            let mut order = Vec::new();
            let mut matches = pco_pack::FilterMask::ones(0);
            self.fold_chunk(plan, fields, &mut order, &mut matches, consumer)
        }
        /// Per-chunk streaming body. `order` and `matches` are stream-scope scratch
        /// reused across chunks: the filter order depends on this chunk's column sizes,
        /// so it is re-sorted here, but neither needs a fresh allocation.
        pub fn fold_chunk<C>(
            &self,
            plan: &[pco_pack::ResolvedFilter],
            fields: &[&str],
            order: &mut Vec<usize>,
            matches: &mut pco_pack::FilterMask,
            consumer: &mut C,
        ) -> pco_pack::anyhow::Result<()>
        where
            C: pco_pack::Consumer<Task>,
        {
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
            let row_count = Self::compute_row_count(&mut reader, fields)?;
            if plan.is_empty() {
                let mut columns = Columns { reader: &mut reader };
                for row_index in 0..row_count {
                    consumer.consume(&mut columns, row_index)?;
                }
                return Ok(());
            }
            matches.reset_ones(row_count);
            order.clear();
            order.extend(0..plan.len());
            let sizes = [
                self.id.len(),
                self.value.len(),
                self.label.len(),
                self.state.len(),
            ];
            order.sort_by_key(|&i| sizes[plan[i].path[0]]);
            for &i in order.iter() {
                <Task as pco_pack::PcoFilter>::filter_step(
                    &mut reader,
                    &plan[i].path,
                    &plan[i].filter,
                    matches,
                )?;
                if !matches.any_set_in_range(0..row_count) {
                    return Ok(());
                }
            }
            let mut columns = Columns { reader: &mut reader };
            matches
                .for_each_set_row(row_count, |row| consumer.consume(&mut columns, row))
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
    #[derive(Default)]
    pub struct Writer {
        row_variant: pco_pack::NumberWriter<i64>,
        variant_1: Vec<i32>,
    }
    #[doc = concat!("Reader state for ", stringify!(State), " data.")]
    #[derive(Clone, Default)]
    pub struct Reader {
        row_variant: pco_pack::LazyReader<i64>,
        variant_1: pco_pack::LazyReader<i32>,
        variant_1_index: usize,
        #[cfg(debug_assertions)]
        variant_1_last_row: Option<usize>,
    }
    /// Column view for enum consumers. `hydrate` reconstructs variants via
    /// `PcoSerde::get`; a `variant_*` view exposes one variant's payload block
    /// for pattern-`fold!` projections that skip the payload struct entirely.
    pub struct Columns<'a> {
        reader: &'a mut Reader,
    }
    impl Columns<'_> {
        /// Payload view for `Self::Running` rows: the variant's lazy block and the payload slot for the current row.
        #[allow(non_snake_case)]
        pub fn variant_Running(
            &mut self,
            row: usize,
        ) -> pco_pack::anyhow::Result<(&mut pco_pack::LazyReader<i32>, usize)> {
            let slot = self.reader.variant_1_index;
            self.reader.variant_1_index += 1;
            debug_assert!(
                slot <= row,
                "variant_{} payload slot {} outruns row {}: every call must follow an ascending walk over this variant's rows",
                "Running", slot, row,
            );
            #[cfg(debug_assertions)]
            {
                let last = self.reader.variant_1_last_row.replace(row);
                debug_assert!(
                    last.is_none_or(| last | row > last),
                    "variant_{} payload view called for row {} after row {}: one call per matching row, ascending",
                    "Running", row, last.unwrap_or_default(),
                );
            }
            Ok((&mut self.reader.variant_1, slot))
        }
    }
    impl pco_pack::PcoSerde for State {
        type Writer = Writer;
        type Reader = Reader;
        const CHEAP_TO_CLONE: bool = <i32 as pco_pack::PcoSerde>::CHEAP_TO_CLONE;
        fn write(
            data: Vec<Self>,
            float_round: u32,
            time_round: pco_pack::chrono::Duration,
        ) -> pco_pack::anyhow::Result<Vec<u8>> {
            let mut writer = Writer::default();
            for item in data {
                match item {
                    Self::Idle => {
                        writer.row_variant.values.push(0i64);
                    }
                    Self::Running(inner) => {
                        writer.row_variant.values.push(1i64);
                        writer.variant_1.push(inner);
                    }
                    Self::Paused => {
                        writer.row_variant.values.push(2i64);
                    }
                }
            }
            let mut out = Vec::new();
            let disc_block = <i64 as pco_pack::PcoSerde>::write(
                writer.row_variant.values,
                float_round,
                time_round,
            )?;
            out.extend_from_slice(&(disc_block.len() as u64).to_le_bytes());
            out.extend_from_slice(&disc_block);
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
            float_round: u32,
            time_round: pco_pack::chrono::Duration,
        ) -> pco_pack::anyhow::Result<Self::Reader> {
            let row_variant = {
                let block = pco_enum_block(src)?;
                pco_pack::LazyReader::new(block.to_vec(), float_round, time_round)
            };
            let mut len_buf = [0u8; 8];
            <std::io::Cursor<&[u8]> as std::io::Read>::read_exact(src, &mut len_buf)?;
            let variant_1 = {
                let block = pco_enum_block(src)?;
                pco_pack::LazyReader::new(block.to_vec(), float_round, time_round)
            };
            Ok(Reader {
                row_variant,
                variant_1,
                variant_1_index: 0,
                #[cfg(debug_assertions)]
                variant_1_last_row: None,
            })
        }
        fn validate_bounds(
            reader: &mut Self::Reader,
        ) -> pco_pack::anyhow::Result<Option<usize>> {
            Ok(Some(reader.row_variant.row_count()?))
        }
        fn get(
            reader: &mut Self::Reader,
            index: usize,
        ) -> pco_pack::anyhow::Result<Option<Self>> {
            let discriminant = <i64 as pco_pack::PcoSerde>::get(
                reader.row_variant.reader_mut()?,
                index,
            )?;
            match discriminant {
                Some(0i64) => Ok(Some(Self::Idle)),
                Some(1i64) => {
                    let idx = reader.variant_1_index;
                    reader.variant_1_index += 1;
                    Ok(Some(Self::Running(reader.variant_1.pop_inner(idx)?)))
                }
                Some(2i64) => Ok(Some(Self::Paused)),
                None => Ok(None),
                Some(other) => {
                    Err(
                        pco_pack::anyhow::anyhow!(
                            "Enum '{}' has no variant with discriminant {}",
                            stringify!(State), other
                        ),
                    )
                }
            }
        }
    }
    /// Slice the next length-prefixed block out of the blob, returning an error
    /// for truncated or format-mismatched blobs.
    fn pco_enum_block<'a>(
        src: &mut std::io::Cursor<&'a [u8]>,
    ) -> pco_pack::anyhow::Result<&'a [u8]> {
        let mut len_buf = [0u8; 8];
        <std::io::Cursor<&[u8]> as std::io::Read>::read_exact(src, &mut len_buf)?;
        let block_len = u64::from_le_bytes(len_buf) as usize;
        let start = <std::io::Cursor<&[u8]>>::position(src) as usize;
        let end = start
            .checked_add(block_len)
            .ok_or_else(|| {
                pco_pack::anyhow::anyhow!(
                    "enum block length {} overflows the buffer", block_len
                )
            })?;
        let buf = *src.get_ref();
        let block = <[u8]>::get(buf, start..end)
            .ok_or_else(|| {
                pco_pack::anyhow::anyhow!(
                    "enum block of {} bytes at offset {} runs past the {}-byte chunk",
                    block_len, start, < [u8] > ::len(buf)
                )
            })?;
        <std::io::Cursor<&[u8]>>::set_position(src, end as u64);
        Ok(block)
    }
    impl pco_pack::PcoFilter for State {
        fn filter_bulk(
            reader: &mut Self::Reader,
            _field: usize,
            filter: &pco_pack::Filter,
            matches: &mut pco_pack::FilterMask,
        ) -> pco_pack::anyhow::Result<()> {
            reader.row_variant.filter_in_group(filter, matches)
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
                if let Some((start_val, end_val)) = json
                    .get("start")
                    .zip(json.get("end"))
                {
                    let start = start_val
                        .as_i64()
                        .with_context(|| {
                            format!(
                                "Range start must be an integer, got {}",
                                pco_pack::json_type_name(start_val)
                            )
                        })?;
                    let end = end_val
                        .as_i64()
                        .with_context(|| {
                            format!(
                                "Range end must be an integer, got {}",
                                pco_pack::json_type_name(end_val)
                            )
                        })?;
                    return Ok(pco_pack::ResolvedFilter {
                        path: vec![0],
                        filter: pco_pack::Filter::Range(start..=end),
                    });
                }
                if let Some(arr) = json.as_array().filter(|a| !a.is_empty()) {
                    let ints: Vec<i64> = arr.iter().filter_map(|v| v.as_i64()).collect();
                    if ints.len() == arr.len() {
                        return Ok(pco_pack::ResolvedFilter {
                            path: vec![0],
                            filter: pco_pack::Filter::InclusionI64(
                                ints.into_iter().collect(),
                            ),
                        });
                    }
                }
                let discriminant: Option<i64> = json
                    .as_i64()
                    .or_else(|| json.as_f64().map(|f| f as i64));
                let discriminant = discriminant
                    .with_context(|| {
                        format!(
                            "Expected i64 discriminant value for enum filter, got {}",
                            pco_pack::json_type_name(json)
                        )
                    })?;
                return Ok(pco_pack::ResolvedFilter {
                    path: vec![0],
                    filter: pco_pack::Filter::I64(discriminant),
                });
            }
            if path.contains('.') {
                return Err(
                    pco_pack::anyhow::anyhow!(
                        "Enum '{}' has no nested fields; cannot resolve path '{}'",
                        stringify!(State), path
                    ),
                );
            }
            let variant_disc: Option<i64> = match path {
                "Idle" => Some(0i64),
                "Running" => Some(1i64),
                "Paused" => Some(2i64),
                _ => None,
            };
            let Some(discriminant) = variant_disc else {
                return Err(
                    pco_pack::anyhow::anyhow!(
                        "Enum '{}' has no variant '{}'; query keys are variant names or the empty discriminant key",
                        stringify!(State), path
                    ),
                );
            };
            match json.as_bool() {
                Some(true) => {
                    Ok(pco_pack::ResolvedFilter {
                        path: vec![0],
                        filter: pco_pack::Filter::I64(discriminant),
                    })
                }
                Some(false) => {
                    Err(
                        pco_pack::anyhow::anyhow!(
                            "Enum '{}' variant '{}' filter must be true; exclude a variant by listing the discriminants to keep under the empty key",
                            stringify!(State), path
                        ),
                    )
                }
                None => {
                    Err(
                        pco_pack::anyhow::anyhow!(
                            "Enum '{}' variant '{}' filter must be a boolean, got {}",
                            stringify!(State), path, pco_pack::json_type_name(json)
                        ),
                    )
                }
            }
        }
    }
    impl pco_pack::PcoPack for State {
        type Reader = <Self as pco_pack::PcoSerde>::Reader;
        type Chunk = pco_pack::serde_bytes::ByteBuf;
        type Filter = serde_json::Value;
        type Columns<'a> = Columns<'a>;
        fn hydrate(
            columns: &mut Self::Columns<'_>,
            row: usize,
        ) -> pco_pack::anyhow::Result<Self> {
            <Self as pco_pack::PcoSerde>::get(columns.reader, row)?
                .ok_or_else(|| {
                    pco_pack::anyhow::anyhow!("Missing enum variant at index {}", row)
                })
        }
        fn columns(
            reader: &mut <Self as pco_pack::PcoSerde>::Reader,
        ) -> Self::Columns<'_> {
            Columns { reader }
        }
        fn fold_value<C>(
            chunks: &[Self::Chunk],
            query: &pco_pack::serde_json::Value,
            fields: &[&str],
            consumer: &mut C,
        ) -> pco_pack::anyhow::Result<()>
        where
            C: pco_pack::Consumer<Self>,
        {
            if !fields.is_empty() {
                return Err(
                    pco_pack::anyhow::anyhow!(
                        "Enum '{}' does not support field matches", stringify!(State)
                    ),
                );
            }
            let mut declared: Vec<i64> = Vec::new();
            for &name in <C as pco_pack::Consumer<Self>>::FIELDS {
                let disc: Option<i64> = match name {
                    "Idle" => Some(0i64),
                    "Running" => Some(1i64),
                    "Paused" => Some(2i64),
                    _ => None,
                };
                let Some(disc) = disc else {
                    return Err(
                        pco_pack::anyhow::anyhow!(
                            "Enum '{}' has no variant '{}'", stringify!(State), name
                        ),
                    );
                };
                declared.push(disc);
            }
            let execution_plan = <Self as pco_pack::PcoFilter>::resolve_query(query)?;
            let stream_all = execution_plan.is_empty() && declared.is_empty();
            let mut pinned: Option<Vec<i64>> = None;
            let mut bounded = true;
            for step in &execution_plan {
                if !bounded {
                    break;
                }
                match &step.filter {
                    pco_pack::Filter::I64(target) if step.path.first() == Some(&0) => {
                        pinned = Some(
                            match pinned {
                                None => vec![* target],
                                Some(cur) => {
                                    cur.into_iter().filter(|d| d == target).collect()
                                }
                            },
                        );
                    }
                    pco_pack::Filter::InclusionI64(
                        targets,
                    ) if step.path.first() == Some(&0) => {
                        pinned = Some(
                            match pinned {
                                None => targets.iter().copied().collect(),
                                Some(cur) => {
                                    cur.into_iter().filter(|d| targets.contains(d)).collect()
                                }
                            },
                        );
                    }
                    _ => bounded = false,
                }
            }
            let pops = <C as pco_pack::Consumer<Self>>::POPS_PAYLOAD;
            let slot_stream = bounded && declared.len() == 1 && pops.len() == 1
                && pops[0]
                && match &pinned {
                    Some(pins) => pins.as_slice() == declared.as_slice(),
                    None => execution_plan.is_empty(),
                };
            let mut matches = pco_pack::FilterMask::ones(0);
            'chunks: for chunk in chunks {
                if chunk.is_empty() {
                    continue 'chunks;
                }
                let mut cursor = std::io::Cursor::new(chunk.as_slice());
                let mut reader = <Self as pco_pack::PcoSerde>::read(
                    &mut cursor,
                    0u32,
                    Default::default(),
                )?;
                if slot_stream {
                    let slot_count: Option<usize> = match Some(declared[0]) {
                        Some(1i64) => Some(reader.variant_1.row_count()?),
                        _ => None,
                    };
                    if let Some(slot_count) = slot_count {
                        let mut columns = Columns { reader: &mut reader };
                        for slot in 0..slot_count {
                            consumer.consume(&mut columns, slot)?;
                        }
                        #[cfg(debug_assertions)]
                        {
                            if declared[0] == 1i64 {
                                debug_assert_eq!(
                                    reader.variant_1_index, slot_count,
                                    "fold arm for variant {} advanced its payload slot cursor {} times for {} streamed slots",
                                    "Running", reader.variant_1_index, slot_count,
                                );
                            }
                        }
                        continue 'chunks;
                    }
                }
                #[cfg(debug_assertions)]
                let mut __pco_delivered = vec![0usize; declared.len()];
                let row_count: usize = <Self as pco_pack::PcoSerde>::validate_bounds(
                        &mut reader,
                    )
                    .context("Invalid enum payload")?
                    .ok_or_else(|| pco_pack::anyhow::anyhow!("No rows in enum data"))?;
                if stream_all {
                    let mut columns = Columns { reader: &mut reader };
                    for row_index in 0..row_count {
                        consumer.consume(&mut columns, row_index)?;
                    }
                    continue 'chunks;
                }
                matches.reset_ones(row_count);
                for step in &execution_plan {
                    <Self as pco_pack::PcoFilter>::filter_step(
                        &mut reader,
                        &step.path,
                        &step.filter,
                        &mut matches,
                    )?;
                    if !matches.any_set_in_range(0..row_count) {
                        continue 'chunks;
                    }
                }
                if !declared.is_empty() {
                    let redundant = pinned
                        .as_ref()
                        .is_some_and(|pins| {
                            bounded && pins.iter().all(|d| declared.contains(d))
                        });
                    if !redundant {
                        let disc_filter = if declared.len() == 1 {
                            pco_pack::Filter::I64(declared[0])
                        } else {
                            pco_pack::Filter::InclusionI64(
                                declared.iter().copied().collect(),
                            )
                        };
                        <Self as pco_pack::PcoFilter>::filter_bulk(
                            &mut reader,
                            0,
                            &disc_filter,
                            &mut matches,
                        )?;
                        if !matches.any_set_in_range(0..row_count) {
                            continue 'chunks;
                        }
                    }
                }
                if declared.len() > 1 {
                    matches
                        .for_each_set_row(
                            row_count,
                            |row_index| {
                                let disc = <i64 as pco_pack::PcoSerde>::get(
                                        reader.row_variant.reader_mut()?,
                                        row_index,
                                    )?
                                    .ok_or_else(|| {
                                        pco_pack::anyhow::anyhow!(
                                            "Missing discriminant at row {}", row_index
                                        )
                                    })?;
                                let Some(variant) = declared.iter().position(|&d| d == disc)
                                else {
                                    return Err(
                                        pco_pack::anyhow::anyhow!(
                                            "Row {} carries discriminant {} outside the declared variants",
                                            row_index, disc
                                        ),
                                    );
                                };
                                #[cfg(debug_assertions)]
                                {
                                    __pco_delivered[variant] += 1;
                                }
                                let mut columns = Columns { reader: &mut reader };
                                consumer.consume_variant(&mut columns, row_index, variant)
                            },
                        )?;
                } else {
                    let mut columns = Columns { reader: &mut reader };
                    matches
                        .for_each_set_row(
                            row_count,
                            |row_index| {
                                #[cfg(debug_assertions)]
                                if let Some(n) = __pco_delivered.first_mut() {
                                    *n += 1;
                                }
                                consumer.consume(&mut columns, row_index)
                            },
                        )?;
                }
                #[cfg(debug_assertions)]
                if !pops.is_empty() {
                    debug_assert!(
                        pops.len() == declared.len(),
                        "Consumer::POPS_PAYLOAD must align with FIELDS"
                    );
                    for __pco_arm in 0..pops.len().min(declared.len()) {
                        if !pops[__pco_arm] {
                            continue;
                        }
                        if declared[__pco_arm] == 1i64 {
                            debug_assert_eq!(
                                reader.variant_1_index, __pco_delivered[__pco_arm],
                                "fold arm {} for variant {} advanced its payload slot cursor {} times for {} delivered rows",
                                __pco_arm, "Running", reader.variant_1_index,
                                __pco_delivered[__pco_arm],
                            );
                        }
                    }
                }
            }
            Ok(())
        }
        fn write(data: Vec<Self>) -> pco_pack::anyhow::Result<Vec<Self::Chunk>> {
            let mut result = Vec::new();
            let mut rows = data.into_iter();
            while let Some(first) = rows.next() {
                let mut piece = Vec::with_capacity(Self::CHUNK_SIZE);
                piece.push(first);
                piece.extend(rows.by_ref().take(Self::CHUNK_SIZE - 1));
                let chunk = <Self as pco_pack::PcoSerde>::write(
                    piece,
                    0u32,
                    Default::default(),
                )?;
                result.push(chunk.into());
            }
            Ok(result)
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
                            <Self as pco_pack::PcoSerde>::get(&mut reader, row_idx)?
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
            let execution_plan = <Self as pco_pack::PcoFilter>::resolve_query(query)?;
            let mut result = Vec::new();
            'chunks: for chunk in chunks {
                if chunk.is_empty() {
                    continue 'chunks;
                }
                let mut cursor = std::io::Cursor::new(chunk.as_slice());
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
                let mut matches = pco_pack::FilterMask::ones(row_count);
                for step in &execution_plan {
                    <Self as pco_pack::PcoFilter>::filter_step(
                        &mut reader,
                        &step.path,
                        &step.filter,
                        &mut matches,
                    )?;
                    if !matches.any_set_in_range(0..row_count) {
                        continue 'chunks;
                    }
                }
                let mut chunk_hits = Vec::with_capacity(matches.count_ones());
                matches
                    .for_each_set_row(
                        row_count,
                        |actual_row_index| {
                            chunk_hits
                                .push(
                                    <Self as pco_pack::PcoSerde>::get(
                                            &mut reader,
                                            actual_row_index,
                                        )?
                                        .ok_or_else(|| {
                                            pco_pack::anyhow::anyhow!(
                                                "Missing enum variant at index {}", actual_row_index
                                            )
                                        })?,
                                );
                            Ok(())
                        },
                    )?;
                result.extend(chunk_hits);
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
    #[allow(non_upper_case_globals)]
    impl State {
        #[doc(hidden)]
        pub const __pco_variant_Idle: () = ();
        #[doc(hidden)]
        pub const __pco_variant_Running: () = ();
        #[doc(hidden)]
        pub const __pco_variant_Paused: () = ();
    }
};
