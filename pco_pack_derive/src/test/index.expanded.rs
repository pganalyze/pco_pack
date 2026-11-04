#[derive(Clone)]
pub struct DeviceTelemetry {
    device_id: i64,
    temperature: i64,
    humidity: i64,
}
const _: () = {
    #[allow(unused)]
    use pco_pack::anyhow::Context;
    use pco_pack::serde::Serialize;
    /// Intermediate compressed form for [DeviceTelemetry] with metadata fields
    /// (index, timestamp bounds) uncompressed and payload columns
    /// stored as compressed ByteBuf. One instance per logical group.
    #[derive(pco_pack::serde::Serialize, pco_pack::serde::Deserialize, Default, Clone)]
    #[serde(crate = "pco_pack::serde")]
    pub struct Chunk {
        pub device_id: i64,
        #[serde(default)]
        pub temperature: pco_pack::serde_bytes::ByteBuf,
        #[serde(default)]
        pub humidity: pco_pack::serde_bytes::ByteBuf,
    }
    #[derive(Clone, Default)]
    pub struct Reader {
        pub device_id: i64,
        pub temperature: pco_pack::LazyReader<i64>,
        pub humidity: pco_pack::LazyReader<i64>,
    }
    #[derive(Default)]
    pub struct Writer {
        pub device_id: Vec<i64>,
        pub temperature: Vec<i64>,
        pub humidity: Vec<i64>,
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
        pub fn device_id(&self, _row: usize) -> pco_pack::anyhow::Result<i64> {
            Ok(self.reader.device_id)
        }
        pub fn temperature(&mut self, row: usize) -> pco_pack::anyhow::Result<i64> {
            self.reader.temperature.pop_inner(row)
        }
        pub fn humidity(&mut self, row: usize) -> pco_pack::anyhow::Result<i64> {
            self.reader.humidity.pop_inner(row)
        }
    }
    /// Typed filter struct for [`#name`].
    #[derive(Clone, Default, Debug, pco_pack::serde::Deserialize)]
    #[serde(crate = "pco_pack::serde", deny_unknown_fields)]
    pub struct Filter {
        pub device_id: Option<pco_pack::I64Filter>,
        pub temperature: Option<pco_pack::I64Filter>,
        pub humidity: Option<pco_pack::I64Filter>,
        #[serde(flatten)]
        others: pco_pack::serde_json::Map<String, pco_pack::serde_json::Value>,
    }
    impl Filter {
        /// Create a new filter with the specified index and timestamp constraints.
        /// Additional fields can be set via `Index<&str>` access on the returned instance.
        pub fn new(device_id: impl Into<pco_pack::I64Filter>) -> Self {
            Self {
                device_id: Some(device_id.into()),
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
            if let Some(ref v) = value.device_id {
                map.insert("device_id".to_string(), pco_pack::serde_json::to_value(v)?);
            }
            if let Some(ref v) = value.temperature {
                map.insert(
                    "temperature".to_string(),
                    pco_pack::serde_json::to_value(v)?,
                );
            }
            if let Some(ref v) = value.humidity {
                map.insert("humidity".to_string(), pco_pack::serde_json::to_value(v)?);
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
    impl pco_pack::PcoSerde for DeviceTelemetry {
        type Writer = Writer;
        type Reader = Reader;
        const CHEAP_TO_CLONE: bool = <i64 as pco_pack::PcoSerde>::CHEAP_TO_CLONE;
        fn write(
            data: Vec<Self>,
            _float_round: u32,
            _time_round: pco_pack::chrono::Duration,
        ) -> pco_pack::anyhow::Result<Vec<u8>> {
            let mut groups: pco_pack::ahash::HashMap<(i64,), Vec<usize>> = Default::default();
            for (i, rec) in data.iter().enumerate() {
                let key = (rec.device_id,);
                groups.entry(key).or_default().push(i);
            }
            let mut records: Vec<Chunk> = Vec::with_capacity(groups.len());
            for (key, indices) in groups {
                let mut col_temperature: Vec<i64> = Vec::new();
                let mut colr_temperature: Vec<&i64> = Vec::new();
                let mut col_humidity: Vec<i64> = Vec::new();
                let mut colr_humidity: Vec<&i64> = Vec::new();
                for &idx in indices.iter() {
                    let rec = &data[idx];
                    if <i64 as pco_pack::PcoSerde>::CHEAP_TO_CLONE {
                        col_temperature.push(rec.temperature);
                    } else {
                        colr_temperature.push(&rec.temperature);
                    }
                    if <i64 as pco_pack::PcoSerde>::CHEAP_TO_CLONE {
                        col_humidity.push(rec.humidity);
                    } else {
                        colr_humidity.push(&rec.humidity);
                    }
                }
                let cb_temperature = if <i64 as pco_pack::PcoSerde>::CHEAP_TO_CLONE {
                    <i64 as pco_pack::PcoSerde>::write(
                        col_temperature,
                        0u32,
                        Default::default(),
                    )?
                } else {
                    <i64 as pco_pack::PcoSerde>::write_borrowed(
                        &colr_temperature,
                        0u32,
                        Default::default(),
                    )?
                };
                let cb_humidity = if <i64 as pco_pack::PcoSerde>::CHEAP_TO_CLONE {
                    <i64 as pco_pack::PcoSerde>::write(
                        col_humidity,
                        0u32,
                        Default::default(),
                    )?
                } else {
                    <i64 as pco_pack::PcoSerde>::write_borrowed(
                        &colr_humidity,
                        0u32,
                        Default::default(),
                    )?
                };
                records
                    .push(Chunk {
                        device_id: key.0,
                        temperature: cb_temperature.into(),
                        humidity: cb_humidity.into(),
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
            let mut groups: pco_pack::ahash::HashMap<(i64,), Vec<usize>> = Default::default();
            for (i, rec) in data.iter().enumerate() {
                let key = (rec.device_id,);
                groups.entry(key).or_default().push(i);
            }
            let mut records: Vec<Chunk> = Vec::with_capacity(groups.len());
            for (key, indices) in groups {
                let mut col_temperature: Vec<i64> = Vec::new();
                let mut colr_temperature: Vec<&i64> = Vec::new();
                let mut col_humidity: Vec<i64> = Vec::new();
                let mut colr_humidity: Vec<&i64> = Vec::new();
                for &idx in indices.iter() {
                    let rec = &data[idx];
                    if <i64 as pco_pack::PcoSerde>::CHEAP_TO_CLONE {
                        col_temperature.push(rec.temperature);
                    } else {
                        colr_temperature.push(&rec.temperature);
                    }
                    if <i64 as pco_pack::PcoSerde>::CHEAP_TO_CLONE {
                        col_humidity.push(rec.humidity);
                    } else {
                        colr_humidity.push(&rec.humidity);
                    }
                }
                let cb_temperature = if <i64 as pco_pack::PcoSerde>::CHEAP_TO_CLONE {
                    <i64 as pco_pack::PcoSerde>::write(
                        col_temperature,
                        0u32,
                        Default::default(),
                    )?
                } else {
                    <i64 as pco_pack::PcoSerde>::write_borrowed(
                        &colr_temperature,
                        0u32,
                        Default::default(),
                    )?
                };
                let cb_humidity = if <i64 as pco_pack::PcoSerde>::CHEAP_TO_CLONE {
                    <i64 as pco_pack::PcoSerde>::write(
                        col_humidity,
                        0u32,
                        Default::default(),
                    )?
                } else {
                    <i64 as pco_pack::PcoSerde>::write_borrowed(
                        &colr_humidity,
                        0u32,
                        Default::default(),
                    )?
                };
                records
                    .push(Chunk {
                        device_id: key.0,
                        temperature: cb_temperature.into(),
                        humidity: cb_humidity.into(),
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
                device_id: rec.device_id,
                temperature: pco_pack::LazyReader::new(
                    rec.temperature.into_vec(),
                    0u32,
                    Default::default(),
                ),
                humidity: pco_pack::LazyReader::new(
                    rec.humidity.into_vec(),
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
    impl pco_pack::PcoFilter for DeviceTelemetry {
        fn filter_bulk(
            reader: &mut Self::Reader,
            field: usize,
            filter: &pco_pack::Filter,
            matches: &mut pco_pack::FilterMask,
        ) -> pco_pack::anyhow::Result<()> {
            if field == 0usize {
                match filter {
                    pco_pack::Filter::I64(v) => {
                        (reader.device_id != *v).then(|| matches.fill(false));
                    }
                    pco_pack::Filter::Range(range) => {
                        let val = reader.device_id;
                        (!(*range.start() <= val && val <= *range.end()))
                            .then(|| matches.fill(false));
                    }
                    pco_pack::Filter::InclusionI64(values) => {
                        (!values.contains(&(reader.device_id)))
                            .then(|| matches.fill(false));
                    }
                    _ => {
                        unreachable!(
                            "unexpected filter type {:?} for field {}", filter,
                            stringify!(device_id)
                        )
                    }
                }
                return Ok(());
            }
            if field == 1usize {
                return reader.temperature.filter_in_group(filter, matches);
            }
            if field == 2usize {
                return reader.humidity.filter_in_group(filter, matches);
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
                    if path.len() > 1 {
                        return Ok(());
                    }
                    match filter {
                        pco_pack::Filter::I64(v) => {
                            (reader.device_id != *v).then(|| matches.fill(false));
                        }
                        pco_pack::Filter::Range(range) => {
                            let val = reader.device_id;
                            (!(*range.start() <= val && val <= *range.end()))
                                .then(|| matches.fill(false));
                        }
                        pco_pack::Filter::InclusionI64(values) => {
                            (!values.contains(&(reader.device_id)))
                                .then(|| matches.fill(false));
                        }
                        _ => {
                            unreachable!(
                                "unexpected filter type {:?} for field {}", filter,
                                stringify!(device_id)
                            )
                        }
                    }
                    Ok(())
                }
                1usize => reader.temperature.filter_nested(&path[1..], filter, matches),
                2usize => reader.humidity.filter_nested(&path[1..], filter, matches),
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
                "device_id" => {
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
                "temperature" => {
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
                "humidity" => {
                    let sub_path = remainder.unwrap_or("");
                    let mut filter = <i64 as pco_pack::PcoFilter>::resolve_filter(
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
    impl pco_pack::PcoPack for DeviceTelemetry {
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
            let fields = <DeviceTelemetry as pco_pack::PcoPack>::resolve_fields(
                query,
                &base,
            )?;
            let plan = <DeviceTelemetry as pco_pack::PcoFilter>::resolve_query(query)?;
            Self::Chunk::validate_projection(&fields)?;
            let mut order: Vec<usize> = Vec::new();
            let mut matches = pco_pack::FilterMask::ones(0);
            for c in chunks {
                c.fold_chunk(&plan, &fields, &mut order, &mut matches, consumer)?;
            }
            Ok(())
        }
        fn write(data: Vec<Self>) -> pco_pack::anyhow::Result<Vec<Self::Chunk>> {
            let mut groups: pco_pack::ahash::HashMap<(i64,), Vec<usize>> = Default::default();
            for (i, rec) in data.iter().enumerate() {
                let key = (rec.device_id,);
                groups.entry(key).or_default().push(i);
            }
            let mut result: Vec<Chunk> = Vec::new();
            for (key, indices) in groups {
                for group_indices in indices.chunks(DeviceTelemetry::CHUNK_SIZE) {
                    let mut col_temperature: Vec<i64> = Vec::new();
                    let mut colr_temperature: Vec<&i64> = Vec::new();
                    let mut col_humidity: Vec<i64> = Vec::new();
                    let mut colr_humidity: Vec<&i64> = Vec::new();
                    for &idx in group_indices {
                        let rec = &data[idx];
                        if <i64 as pco_pack::PcoSerde>::CHEAP_TO_CLONE {
                            col_temperature.push(rec.temperature);
                        } else {
                            colr_temperature.push(&rec.temperature);
                        }
                        if <i64 as pco_pack::PcoSerde>::CHEAP_TO_CLONE {
                            col_humidity.push(rec.humidity);
                        } else {
                            colr_humidity.push(&rec.humidity);
                        }
                    }
                    let cb_temperature = if <i64 as pco_pack::PcoSerde>::CHEAP_TO_CLONE {
                        <i64 as pco_pack::PcoSerde>::write(
                            col_temperature,
                            0u32,
                            Default::default(),
                        )?
                    } else {
                        <i64 as pco_pack::PcoSerde>::write_borrowed(
                            &colr_temperature,
                            0u32,
                            Default::default(),
                        )?
                    };
                    let cb_humidity = if <i64 as pco_pack::PcoSerde>::CHEAP_TO_CLONE {
                        <i64 as pco_pack::PcoSerde>::write(
                            col_humidity,
                            0u32,
                            Default::default(),
                        )?
                    } else {
                        <i64 as pco_pack::PcoSerde>::write_borrowed(
                            &colr_humidity,
                            0u32,
                            Default::default(),
                        )?
                    };
                    let rec = Chunk {
                        device_id: key.0,
                        temperature: cb_temperature.into(),
                        humidity: cb_humidity.into(),
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
            let known: &[&str] = &["device_id", "temperature", "humidity"];
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
            match g.temperature.row_count() {
                Ok(n) if n > 0 => count = n,
                Err(e) => return Err(e),
                _ => {}
            }
        }
        if count == 0 {
            match g.humidity.row_count() {
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
    ) -> pco_pack::anyhow::Result<DeviceTelemetry> {
        Ok(DeviceTelemetry {
            device_id: g.device_id,
            temperature: g.temperature.pop_inner(row)?,
            humidity: g.humidity.pop_inner(row)?,
        })
    }
    impl Chunk {
        fn compute_row_count(
            reader: &mut Reader,
            fields: &[&str],
        ) -> pco_pack::anyhow::Result<usize> {
            let mut count = 0usize;
            if count == 0 && fields.contains(&"humidity") {
                let r = reader.humidity.row_count();
                match r {
                    Ok(n) if n > 0 => count = n,
                    Err(e) => return Err(e),
                    _ => {}
                }
            }
            if count == 0 && fields.contains(&"temperature") {
                let r = reader.temperature.row_count();
                match r {
                    Ok(n) if n > 0 => count = n,
                    Err(e) => return Err(e),
                    _ => {}
                }
            }
            let count = count;
            let has_loaded_payload = fields.contains(&"temperature")
                || fields.contains(&"humidity") || false;
            if count == 0 && !has_loaded_payload && true {
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
        pub fn expand(self) -> pco_pack::anyhow::Result<Vec<DeviceTelemetry>> {
            let mut reader = Reader {
                device_id: self.device_id,
                temperature: <pco_pack::LazyReader<
                    i64,
                >>::new(self.temperature.into_vec(), 0u32, Default::default()),
                humidity: <pco_pack::LazyReader<
                    i64,
                >>::new(self.humidity.into_vec(), 0u32, Default::default()),
            };
            let loaded_fields: &[&str] = &["temperature", "humidity"];
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
        ) -> pco_pack::anyhow::Result<Vec<DeviceTelemetry>> {
            let fields = <DeviceTelemetry as pco_pack::PcoPack>::resolve_fields(
                query,
                fields,
            )?;
            let execution_plan = <DeviceTelemetry as pco_pack::PcoFilter>::resolve_query(
                query,
            )?;
            let mut reader = Reader {
                device_id: self.device_id,
                temperature: <pco_pack::LazyReader<
                    i64,
                >>::new(
                    if fields.is_empty() || fields.contains(&"temperature") {
                        self.temperature.to_vec()
                    } else {
                        Vec::new()
                    },
                    0u32,
                    Default::default(),
                ),
                humidity: <pco_pack::LazyReader<
                    i64,
                >>::new(
                    if fields.is_empty() || fields.contains(&"humidity") {
                        self.humidity.to_vec()
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
            let sizes = [0, self.temperature.len(), self.humidity.len()];
            plan.sort_by_key(|&i| sizes[execution_plan[i].path[0]]);
            for i in plan {
                <DeviceTelemetry as pco_pack::PcoFilter>::filter_step(
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
            let known: &[&str] = &["device_id", "temperature", "humidity"];
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
            C: pco_pack::Consumer<DeviceTelemetry>,
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
            C: pco_pack::Consumer<DeviceTelemetry>,
        {
            let mut reader = Reader {
                device_id: self.device_id,
                temperature: <pco_pack::LazyReader<
                    i64,
                >>::new(
                    if fields.is_empty() || fields.contains(&"temperature") {
                        self.temperature.to_vec()
                    } else {
                        Vec::new()
                    },
                    0u32,
                    Default::default(),
                ),
                humidity: <pco_pack::LazyReader<
                    i64,
                >>::new(
                    if fields.is_empty() || fields.contains(&"humidity") {
                        self.humidity.to_vec()
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
            let sizes = [0, self.temperature.len(), self.humidity.len()];
            order.sort_by_key(|&i| sizes[plan[i].path[0]]);
            for &i in order.iter() {
                <DeviceTelemetry as pco_pack::PcoFilter>::filter_step(
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
pub fn fold_example() -> impl pco_pack::Consumer<DeviceTelemetry> {
    {
        #[doc(hidden)]
        struct PcoFold0<__PcoF>(__PcoF);
        impl<__PcoF, __PcoO> pco_pack::Consumer<DeviceTelemetry> for PcoFold0<__PcoF>
        where
            __PcoF: FnMut(i64, i64) -> __PcoO,
            __PcoO: pco_pack::Output,
        {
            const FIELDS: &'static [&'static str] = &["device_id", "temperature"];
            fn consume(
                &mut self,
                __pco_cols: &mut <DeviceTelemetry as pco_pack::PcoPack>::Columns<'_>,
                __pco_row: usize,
            ) -> pco_pack::anyhow::Result<()> {
                pco_pack::Output::lift(
                    (self
                        .0)(
                        __pco_cols.device_id(__pco_row)?,
                        __pco_cols.temperature(__pco_row)?,
                    ),
                )
            }
        }
        PcoFold0(|device_id: i64, temperature: i64| {
            let _ = (device_id, temperature);
        })
    }
}
