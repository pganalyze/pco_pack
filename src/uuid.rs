use super::*;
use ::uuid::Uuid;

#[derive(Clone, Default)]
pub struct UuidReader {
    pub values: Arc<[Uuid]>,
}

impl PcoSerde for Uuid {
    type Writer = ();
    type Reader = UuidReader;

    fn write(data: Vec<Self>, _float_round: u32, _time_round: chrono::Duration) -> anyhow::Result<Vec<u8>> {
        let serialized = rmp_serde::to_vec(&data).map_err(|e| anyhow::anyhow!("failed to serialize uuids: {}", e))?;
        zstd::encode_all(serialized.as_slice(), 3).map_err(|e| anyhow::anyhow!("failed to compress uuids: {}", e))
    }

    fn read(
        src: &mut std::io::Cursor<&[u8]>, _float_round: u32, _time_round: chrono::Duration,
    ) -> anyhow::Result<Self::Reader> {
        let buf = *src.get_ref();
        let pos = src.position() as usize;
        if pos >= buf.len() || buf.is_empty() {
            return Ok(UuidReader { values: Arc::new([]) });
        }
        let compressed = &buf[pos..];
        let serialized = zstd::decode_all(std::io::Cursor::new(compressed))
            .map_err(|e| anyhow::anyhow!("failed to decompress uuids: {}", e))?;
        let uuids: Vec<Uuid> =
            rmp_serde::from_slice(&serialized).map_err(|e| anyhow::anyhow!("failed to deserialize uuids: {}", e))?;
        Ok(UuidReader { values: uuids.into() })
    }

    fn validate_bounds(reader: &mut Self::Reader) -> anyhow::Result<Option<usize>> {
        Ok(Some(reader.values.len()))
    }

    fn get(reader: &mut Self::Reader, index: usize) -> anyhow::Result<Option<Self>> {
        Ok(reader.values.get(index).cloned())
    }
}

impl PcoFilter for Uuid {
    fn resolve_filter(path: &str, json: &serde_json::Value) -> anyhow::Result<ResolvedFilter> {
        if !path.is_empty() {
            return Err(anyhow::anyhow!("Field '{}' is a UUID leaf and cannot contain nested path", path));
        }
        match json {
            serde_json::Value::String(s) => {
                let parsed = Uuid::parse_str(s).map_err(|e| anyhow::anyhow!("Invalid UUID in filter: {}", e))?;
                Ok(ResolvedFilter { path: vec![0], filter: Filter::Uuid(parsed) })
            }
            serde_json::Value::Array(arr) => {
                let uuids: Vec<Uuid> = arr
                    .iter()
                    .map(|v| {
                        v.as_str()
                            .ok_or_else(|| anyhow::anyhow!("UUID filter array must contain only strings"))
                            .and_then(|s| {
                                Uuid::parse_str(s).map_err(|e| anyhow::anyhow!("Invalid UUID in filter: {}", e))
                            })
                    })
                    .collect::<anyhow::Result<_>>()?;
                Ok(ResolvedFilter { path: vec![0], filter: Filter::InclusionUuid(uuids) })
            }
            _ => Err(anyhow::anyhow!("Expected string or array for UUID filter, got {}", json)),
        }
    }

    fn filter_bulk(reader: &mut Self::Reader, _field: usize, filter: &Filter, matches: &mut FilterMask) -> Result<()> {
        if reader.values.is_empty() {
            matches.handle_empty_column::<Uuid, _>(|val| Self::filter_match(val, filter));
            return Ok(());
        }
        matches.build_with_and(&reader.values, |v| Self::filter_match(v, filter));
        Ok(())
    }

    fn filter_nested(
        _reader: &mut Self::Reader, _path: &[usize], _filter: &Filter, _matches: &mut FilterMask,
    ) -> Result<()> {
        unreachable!("filter_nested not supported for uuid::Uuid");
    }

    fn filter_match(value: &Self, filter: &Filter) -> bool {
        match filter {
            Filter::Uuid(target) => value == target,
            Filter::InclusionUuid(targets) => targets.contains(value),
            _ => false,
        }
    }
}
