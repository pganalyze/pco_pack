use super::*;

include!("lazy_msgpack.rs");

impl_lazy_msgpack!(ByteBufReader; (); serde_bytes::ByteBuf; false);

impl FallbackReader for ByteBufReader {
    fn read_fallback(raw: &[u8]) -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        if raw.is_empty() {
            return Ok(Self::default());
        }
        let value = rmp_serde::from_slice::<serde_bytes::ByteBuf>(raw)
            .map_err(|e| anyhow::anyhow!("fallback deserialization failed: {e}"))?;
        let mut buf = Vec::with_capacity(2 + raw.len());
        buf.push(self::FORMAT_VERSION_CHUNKED);
        rmp_serde::encode::write(&mut buf, std::slice::from_ref(&value) as &[serde_bytes::ByteBuf])?;
        Ok(ByteBufReader {
            msgpack_bytes: buf.into(),
            total_rows: 1,
            format_version: self::FORMAT_VERSION_CHUNKED,
            data_pos: 0,
            cached_chunk: Some(vec![value]),
            cached_row_start: 0,
            predecoded: None,
        })
    }
}

impl PcoFilter for serde_bytes::ByteBuf {
    fn resolve_filter(path: &str, json: &serde_json::Value) -> anyhow::Result<ResolvedFilter> {
        if !path.is_empty() {
            return Err(anyhow::anyhow!("Field '{}' is a bytes leaf and cannot contain nested path", path));
        }
        match json {
            serde_json::Value::String(s) => {
                let decoded = hex::decode(s).map_err(|e| anyhow::anyhow!("Invalid hex in filter: {}", e))?;
                Ok(ResolvedFilter { path: vec![0], filter: Filter::Bytes(decoded) })
            }
            serde_json::Value::Array(_) => Err(anyhow::anyhow!(
                "Inclusion queries are not supported for bytes columns; use exact equality instead"
            )),
            _ => Err(anyhow::anyhow!("Expected string for bytes filter, got {}", json_type_name(json))),
        }
    }

    fn filter_bulk(reader: &mut Self::Reader, _field: usize, filter: &Filter, matches: &mut FilterMask) -> Result<()> {
        // Bytes filters never yield scan targets, so this stays on the
        // deserialize-and-predicate loop; only the predecoded fallback branch
        // is new, and ByteBuf fallbacks never populate it.
        reader.filter_bulk(filter, matches, Self::filter_match)
    }

    fn filter_nested(
        _reader: &mut Self::Reader, _path: &[usize], _filter: &Filter, _matches: &mut FilterMask,
    ) -> Result<()> {
        unreachable!("filter_nested not supported for serde_bytes::ByteBuf");
    }

    fn filter_match(value: &Self, filter: &Filter) -> bool {
        match filter {
            Filter::Bytes(expected) => value.as_ref() == expected.as_slice(),
            _ => false,
        }
    }
}
