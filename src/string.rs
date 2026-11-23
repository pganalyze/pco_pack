use super::*;

include!("lazy_msgpack.rs");

// A String clone is a fresh heap allocation, so stage references.
impl_lazy_msgpack!(StringReader; (); String; false);

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
                Ok(ResolvedFilter { path: vec![0], filter: Filter::InclusionString(strings.into_iter().collect()) })
            }
            _ => Err(anyhow::anyhow!("Expected string or array for string filter, got {}", json_type_name(json))),
        }
    }

    fn filter_bulk(reader: &mut Self::Reader, _field: usize, filter: &Filter, matches: &mut FilterMask) -> Result<()> {
        reader.filter_bulk(filter, matches, Self::filter_match)
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

// SmolStr clones are memcpy or a short heap copy; owned staging
// compresses measurably faster for it (taxi bench), so it stays cheap.
impl_lazy_msgpack!(SmolStrReader; (); smol_str::SmolStr; true);

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
                Ok(ResolvedFilter { path: vec![0], filter: Filter::InclusionString(strings.into_iter().collect()) })
            }
            _ => Err(anyhow::anyhow!("Expected string or array for string filter, got {}", json_type_name(json))),
        }
    }

    fn filter_bulk(reader: &mut Self::Reader, _field: usize, filter: &Filter, matches: &mut FilterMask) -> Result<()> {
        reader.filter_bulk(filter, matches, Self::filter_match)
    }

    fn filter_nested(
        _reader: &mut Self::Reader, _path: &[usize], _filter: &Filter, _matches: &mut FilterMask,
    ) -> Result<()> {
        unreachable!("filter_nested not supported for smol_str::SmolStr");
    }

    fn filter_match(value: &Self, filter: &Filter) -> bool {
        match filter {
            Filter::String(target) => value.as_str() == target.as_str(),
            Filter::InclusionString(targets) => targets.contains(value.as_str()),
            _ => false,
        }
    }
}
