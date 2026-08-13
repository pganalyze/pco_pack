use super::*;

#[derive(Clone)]
pub struct BoolReader {
    pub values: Arc<[bool]>,
    pub current_idx: usize,
}

impl Default for BoolReader {
    fn default() -> Self {
        Self { values: Arc::new([]), current_idx: 0 }
    }
}

impl PcoSerde for bool {
    type Writer = ();
    type Reader = BoolReader;

    fn write(data: Vec<bool>, _float_round: u32, _time_round: chrono::Duration) -> anyhow::Result<Vec<u8>> {
        let mut raw_buffer = Vec::new();
        data.serialize(&mut rmp_serde::Serializer::new(&mut raw_buffer))?;
        Ok(zstd::encode_all(raw_buffer.as_slice(), 3)?)
    }

    fn read(src: &mut Cursor<&[u8]>, _float_round: u32, _time_round: chrono::Duration) -> anyhow::Result<Self::Reader> {
        let buf = src.get_ref();
        let pos = src.position() as usize;
        if pos >= buf.len() || buf[pos..].is_empty() {
            return Ok(BoolReader::default());
        }
        if let Ok(msgpack_bytes) = zstd::decode_all(&buf[pos..]) {
            if let Ok(values) = rmp_serde::from_slice::<Vec<bool>>(&msgpack_bytes) {
                src.set_position(buf.len() as u64);
                return Ok(BoolReader { values: values.into(), current_idx: 0 });
            }
        }
        // Legacy Pcodec-compressed bool format for backwards compatibility
        if let Ok(reader) = <BoolReader as FallbackReader>::read_fallback(&buf[pos..]) {
            src.set_position(buf.len() as u64);
            return Ok(reader);
        }
        anyhow::bail!("Failed to deserialize bool column")
    }

    fn validate_bounds(reader: &mut Self::Reader) -> anyhow::Result<Option<usize>> {
        Ok(Some(reader.values.len()))
    }

    #[inline(always)]
    fn get(reader: &mut Self::Reader, index: usize) -> anyhow::Result<Option<Self>> {
        Ok(reader.values.get(index).copied())
    }
}

impl FallbackReader for BoolReader {
    fn read_fallback(raw: &[u8]) -> anyhow::Result<Self> {
        let values = pco::standalone::simple_decompress::<u16>(raw)?;
        let values: Arc<[bool]> = values.into_iter().map(|b| b != 0).collect();
        Ok(BoolReader { values, current_idx: 0 })
    }
}

impl PcoFilter for bool {
    fn filter_bulk(reader: &mut Self::Reader, _field: usize, filter: &Filter, matches: &mut FilterMask) -> Result<()> {
        match filter {
            Filter::Bool(v) => {
                matches.build_with_and(&reader.values, |&val| val == *v);
            }
            Filter::InclusionBool(values) => {
                matches.build_with_and(&reader.values, |&val| values.contains(&val));
            }
            _ => unreachable!("bool does not support filter: {filter:?}"),
        }
        Ok(())
    }

    fn filter_nested(
        _reader: &mut Self::Reader, _path: &[usize], _filter: &Filter, matches: &mut FilterMask,
    ) -> Result<()> {
        matches.fill(false);
        Ok(())
    }

    fn filter_match(value: &Self, filter: &Filter) -> bool {
        filter_match_impl(value, filter)
    }

    fn resolve_filter(path: &str, json: &serde_json::Value) -> ::anyhow::Result<ResolvedFilter> {
        if !path.is_empty() {
            anyhow::bail!("Field '{}' is a bool leaf and cannot contain nested path", path);
        }
        let filter = match json {
            serde_json::Value::Array(arr) => {
                let mut bools = Vec::with_capacity(arr.len());
                for v in arr {
                    match v {
                        serde_json::Value::Bool(b) => bools.push(*b),
                        _ => anyhow::bail!("Expected boolean value for field '{}'", path),
                    }
                }
                Filter::InclusionBool(bools)
            }
            serde_json::Value::Bool(b) => Filter::Bool(*b),
            _ => anyhow::bail!("Expected boolean value for field '{}'", path),
        };
        Ok(ResolvedFilter { path: vec![0], filter })
    }
}

fn filter_match_impl(value: &bool, filter: &Filter) -> bool {
    match filter {
        Filter::Bool(v) => *value == *v,
        Filter::InclusionBool(values) => values.contains(value),
        _ => unreachable!("unsupported bool filter: {filter:?}"),
    }
}
