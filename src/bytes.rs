use super::*;

include!("lazy_msgpack.rs");

impl_lazy_msgpack!(ByteBufReader; (); serde_bytes::ByteBuf;);

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
        rmp_serde::encode::write(&mut buf, &vec![value.clone()] as &[serde_bytes::ByteBuf])?;
        Ok(ByteBufReader {
            msgpack_bytes: buf.into(),
            total_rows: 1,
            format_version: self::FORMAT_VERSION_CHUNKED,
            data_pos: 0,
            cached_chunk: Some(vec![value]),
            cached_row_start: 0,
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
            _ => Err(anyhow::anyhow!("Expected string for bytes filter, got {}", json)),
        }
    }

    fn filter_bulk(reader: &mut Self::Reader, _field: usize, filter: &Filter, matches: &mut FilterMask) -> Result<()> {
        if reader.total_rows == 0 || reader.msgpack_bytes.is_empty() {
            matches.handle_empty_column::<serde_bytes::ByteBuf, _>(|val| Self::filter_match(val, filter));
            return Ok(());
        }
        let bytes = &reader.msgpack_bytes[1..];
        let mut pos: usize = 0;
        let mut row_idx: usize = 0;
        while row_idx < reader.total_rows && pos < bytes.len() {
            // Skip chunks whose rows are already eliminated.
            let chunk_end = (row_idx + self::CHUNK_SIZE).min(reader.total_rows);
            if !matches.any_set_in_range(row_idx..chunk_end) {
                if ByteBufReader::skip_chunk(bytes, &mut pos).is_none() {
                    break;
                }
                row_idx = chunk_end;
                continue;
            }
            if let Some(chunk) = ByteBufReader::deserialize_chunk(bytes, &mut pos) {
                matches.build_into(row_idx, &chunk, |val| Self::filter_match(val, filter));
                row_idx += chunk.len();
            } else {
                break;
            }
        }
        Ok(())
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
