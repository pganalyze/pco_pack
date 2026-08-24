use super::*;

include!("lazy_msgpack.rs");

impl_lazy_msgpack!(StringReader; (); String;);

impl FallbackReader for StringReader {
    fn read_fallback(raw: &[u8]) -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        if raw.is_empty() {
            return Ok(Self::default());
        }
        // Legacy format: raw zstd-compressed stream of msgpack items.
        let decompressed = zstd::decode_all(std::io::Cursor::new(raw))
            .map_err(|e| anyhow::anyhow!("fallback decompression failed: {}", e))?;
        let mut cursor = std::io::Cursor::new(&decompressed[..]);
        let mut values = Vec::new();
        while cursor.position() < decompressed.len() as u64 {
            match rmp_serde::from_read::<_, String>(&mut cursor) {
                Ok(v) => values.push(v),
                Err(_) => break,
            }
        }
        if values.is_empty() {
            return Ok(Self::default());
        }
        let total_rows = values.len();
        Ok(StringReader {
            msgpack_bytes: Default::default(),
            total_rows,
            format_version: self::FORMAT_VERSION_CHUNKED,
            data_pos: 0,
            cached_chunk: None,
            cached_row_start: 0,
            predecoded: Some(values.into()),
        })
    }
}

impl PcoFilter for String {
    fn resolve_filter(path: &str, json: &serde_json::Value) -> anyhow::Result<ResolvedFilter> {
        if !path.is_empty() {
            return Err(anyhow::anyhow!("Field '{}' is a string leaf and cannot contain nested path", path));
        }
        match json {
            serde_json::Value::String(s) => Ok(ResolvedFilter { path: vec![0], filter: Filter::String(s.clone()) }),
            serde_json::Value::Array(arr) => {
                let strings: Vec<String> = arr.iter().filter_map(|v| v.as_str().map(String::from)).collect();
                if strings.len() != arr.len() {
                    return Err(anyhow::anyhow!("String filter array must contain only strings"));
                }
                Ok(ResolvedFilter { path: vec![0], filter: Filter::InclusionString(strings) })
            }
            _ => Err(anyhow::anyhow!("Expected string or array for string filter, got {}", json)),
        }
    }

    fn filter_bulk(reader: &mut Self::Reader, _field: usize, filter: &Filter, matches: &mut FilterMask) -> Result<()> {
        // Legacy fallback data is fully decoded; apply the predicate directly.
        if let Some(ref values) = reader.predecoded {
            matches.build_into(0, values.as_ref(), |val| Self::filter_match(val, filter));
            return Ok(());
        }
        if reader.total_rows == 0 || reader.msgpack_bytes.is_empty() {
            matches.handle_empty_column::<String, _>(|val| Self::filter_match(val, filter));
            return Ok(());
        }
        // Skip format_version byte, iterate chunks sequentially.
        let bytes = &reader.msgpack_bytes[1..];
        let mut pos: usize = 0;
        let mut row_idx: usize = 0;
        while row_idx < reader.total_rows && pos < bytes.len() {
            // Skip chunks whose rows are already eliminated.
            let chunk_end = (row_idx + self::CHUNK_SIZE).min(reader.total_rows);
            if !matches.any_set_in_range(row_idx..chunk_end) {
                if StringReader::skip_chunk(bytes, &mut pos).is_none() {
                    break;
                }
                row_idx = chunk_end;
                continue;
            }
            if let Some(chunk) = StringReader::deserialize_chunk(bytes, &mut pos) {
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
        unreachable!("filter_nested not supported for String");
    }

    fn filter_match(value: &Self, filter: &Filter) -> bool {
        match filter {
            Filter::String(target) => value == target,
            Filter::InclusionString(targets) => targets.contains(value),
            _ => false,
        }
    }
}

impl_lazy_msgpack!(SmolStrReader; (); smol_str::SmolStr;);

impl FallbackReader for SmolStrReader {
    fn read_fallback(raw: &[u8]) -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        if raw.is_empty() {
            return Ok(Self::default());
        }
        // Legacy format: raw zstd-compressed stream of msgpack items.
        let decompressed = zstd::decode_all(std::io::Cursor::new(raw))
            .map_err(|e| anyhow::anyhow!("fallback decompression failed: {}", e))?;
        let mut cursor = std::io::Cursor::new(&decompressed[..]);
        let mut values: Vec<smol_str::SmolStr> = Vec::new();
        while cursor.position() < decompressed.len() as u64 {
            match rmp_serde::from_read::<_, smol_str::SmolStr>(&mut cursor) {
                Ok(v) => values.push(v),
                Err(_) => break,
            }
        }
        if values.is_empty() {
            return Ok(Self::default());
        }
        let total_rows = values.len();
        Ok(SmolStrReader {
            msgpack_bytes: Default::default(),
            total_rows,
            format_version: self::FORMAT_VERSION_CHUNKED,
            data_pos: 0,
            cached_chunk: None,
            cached_row_start: 0,
            predecoded: Some(values.into()),
        })
    }
}

impl PcoFilter for smol_str::SmolStr {
    fn resolve_filter(path: &str, json: &serde_json::Value) -> anyhow::Result<ResolvedFilter> {
        if !path.is_empty() {
            return Err(anyhow::anyhow!("Field '{}' is a string leaf and cannot contain nested path", path));
        }
        match json {
            serde_json::Value::String(s) => Ok(ResolvedFilter { path: vec![0], filter: Filter::String(s.clone()) }),
            serde_json::Value::Array(arr) => {
                let strings: Vec<String> = arr.iter().filter_map(|v| v.as_str().map(String::from)).collect();
                if strings.len() != arr.len() {
                    return Err(anyhow::anyhow!("String filter array must contain only strings"));
                }
                Ok(ResolvedFilter { path: vec![0], filter: Filter::InclusionString(strings) })
            }
            _ => Err(anyhow::anyhow!("Expected string or array for string filter, got {}", json)),
        }
    }

    fn filter_bulk(reader: &mut Self::Reader, _field: usize, filter: &Filter, matches: &mut FilterMask) -> Result<()> {
        // Legacy fallback data is fully decoded; apply the predicate directly.
        if let Some(ref values) = reader.predecoded {
            matches.build_into(0, values.as_ref(), |val| Self::filter_match(val, filter));
            return Ok(());
        }
        if reader.total_rows == 0 || reader.msgpack_bytes.is_empty() {
            matches.handle_empty_column::<smol_str::SmolStr, _>(|val| Self::filter_match(val, filter));
            return Ok(());
        }
        let bytes = &reader.msgpack_bytes[1..];
        let mut pos: usize = 0;
        let mut row_idx: usize = 0;
        while row_idx < reader.total_rows && pos < bytes.len() {
            // Skip chunks whose rows are already eliminated.
            let chunk_end = (row_idx + self::CHUNK_SIZE).min(reader.total_rows);
            if !matches.any_set_in_range(row_idx..chunk_end) {
                if SmolStrReader::skip_chunk(bytes, &mut pos).is_none() {
                    break;
                }
                row_idx = chunk_end;
                continue;
            }
            if let Some(chunk) = SmolStrReader::deserialize_chunk(bytes, &mut pos) {
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
        unreachable!("filter_nested not supported for smol_str::SmolStr");
    }

    fn filter_match(value: &Self, filter: &Filter) -> bool {
        match filter {
            Filter::String(target) => value.as_ref() == target,
            Filter::InclusionString(targets) => targets.iter().any(|t| t == value.as_ref()),
            _ => false,
        }
    }
}
