use super::*;
use std::io::Cursor;

/// Chunk size for batched serialization/deserialization.
pub(crate) const CHUNK_SIZE: usize = 128;
/// Format version indicating chunked msgpack arrays.
pub(crate) const FORMAT_VERSION_CHUNKED: u8 = 1;

fn round_json_value(value: &serde_json::Value, decimals: u32) -> serde_json::Value {
    if decimals == 0 {
        return value.clone();
    }
    let scale = 10i64.pow(decimals) as f64;
    match value {
        serde_json::Value::Number(n) => {
            if n.as_i64().is_some() && n.as_f64().unwrap_or(0.0).fract().abs() <= f64::EPSILON {
                return value.clone();
            }
            if let Some(f) = n.as_f64() {
                let rounded = ((f * scale).round()) / scale;
                serde_json::Value::Number(serde_json::Number::from_f64(rounded).unwrap_or(serde_json::Number::from(0)))
            } else {
                value.clone()
            }
        }
        serde_json::Value::Object(map) => {
            serde_json::Value::Object(map.iter().map(|(k, v)| (k.clone(), round_json_value(v, decimals))).collect())
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(|v| round_json_value(v, decimals)).collect())
        }
        other => other.clone(),
    }
}

pub(crate) fn matches_json_value(value: &serde_json::Value, filter: &super::Filter) -> bool {
    match filter {
        super::Filter::Json(target) => value == target,
        _ => false,
    }
}

/// Lazy-deserialization reader for serde_json::Value using chunked msgpack+zstd.
/// Same format as lazy_msgpack readers: `[format_version][chunk_0][chunk_1]...` where each chunk is a msgpack array of up to CHUNK_SIZE items.
#[derive(Clone, Default)]
pub struct JsonValueReader {
    pub msgpack_bytes: std::sync::Arc<[u8]>,
    pub total_rows: usize,
    pub format_version: u8,
    /// Current position in the decompressed data stream (after format_version byte).
    pub(crate) data_pos: usize,
    /// Cached deserialized chunk.
    cached_chunk: Option<Vec<serde_json::Value>>,
    /// Row index where `cached_chunk` starts.
    cached_row_start: usize,
}

impl JsonValueReader {
    #[inline]
    pub(crate) fn deserialize_chunk(bytes: &[u8], pos: &mut usize) -> Option<Vec<serde_json::Value>> {
        if *pos >= bytes.len() {
            return None;
        }
        let mut deserializer = rmp_serde::Deserializer::new(Cursor::new(&bytes[*pos..]));
        let chunk: Vec<serde_json::Value> = Vec::<serde_json::Value>::deserialize(&mut deserializer).ok()?;
        *pos += deserializer.into_inner().position() as usize;
        Some(chunk)
    }

    /// Deserialize one item from a flat (non-chunked) msgpack byte stream. Used for fallback reads.
    #[inline]
    pub(crate) fn deserialize_one(bytes: &[u8], pos: &mut usize) -> Option<serde_json::Value> {
        if *pos >= bytes.len() {
            return None;
        }
        let mut deserializer = rmp_serde::Deserializer::new(Cursor::new(&bytes[*pos..]));
        let val = serde_json::Value::deserialize(&mut deserializer).ok()?;
        *pos += deserializer.into_inner().position() as usize;
        Some(val)
    }

    /// Skip an entire msgpack array chunk without allocating. Used by get() to fast-forward.
    #[inline]
    pub(crate) fn skip_chunk(bytes: &[u8], pos: &mut usize) -> Option<()> {
        if *pos >= bytes.len() {
            return None;
        }
        let mut deserializer = rmp_serde::Deserializer::new(Cursor::new(&bytes[*pos..]));
        <Vec<serde::de::IgnoredAny>>::deserialize(&mut deserializer).ok()?;
        *pos += deserializer.into_inner().position() as usize;
        Some(())
    }
}

impl super::PcoSerde for serde_json::Value {
    type Writer = ();
    type Reader = JsonValueReader;

    fn write(data: Vec<Self>, float_round: u32, _time_round: chrono::Duration) -> anyhow::Result<Vec<u8>> {
        let rounded: Vec<serde_json::Value> =
            if float_round > 0 { data.into_iter().map(|v| round_json_value(&v, float_round)).collect() } else { data };
        let len = rounded.len();
        let mut raw_buffer = Vec::with_capacity(len * 64);
        raw_buffer.push(FORMAT_VERSION_CHUNKED);
        for chunk in rounded.chunks(CHUNK_SIZE) {
            rmp_serde::encode::write(&mut raw_buffer, &chunk.to_vec())?;
        }
        let compressed = zstd::encode_all(raw_buffer.as_slice(), 3)?;
        let mut out = Vec::with_capacity(16 + compressed.len());
        out.extend_from_slice(&(len as u64).to_le_bytes());
        out.extend_from_slice(&(compressed.len() as u64).to_le_bytes());
        out.extend_from_slice(&compressed);
        Ok(out)
    }

    fn read(
        src: &mut std::io::Cursor<&[u8]>, _float_round: u32, _time_round: chrono::Duration,
    ) -> anyhow::Result<Self::Reader> {
        let saved_pos = src.position();
        let buf = src.get_ref();
        if saved_pos as usize >= buf.len() || buf.is_empty() {
            return Ok(JsonValueReader::default());
        }
        let remaining = &buf[saved_pos as usize..];
        let buf_len = buf.len();
        // Try new format: [u64 LE total_rows][u64 LE compressed_len][zstd payload]
        if remaining.len() >= 16 {
            let total_rows = u64::from_le_bytes((&remaining[0..8]).try_into().unwrap()) as usize;
            let block_len = u64::from_le_bytes((&remaining[8..16]).try_into().unwrap()) as usize;
            let compressed_start = 16usize;
            if compressed_start + block_len <= remaining.len() {
                let compressed = &remaining[compressed_start..compressed_start + block_len];
                match zstd::decode_all(Cursor::new(compressed)) {
                    Ok(msgpack_bytes) => {
                        src.set_position(saved_pos + compressed_start as u64 + block_len as u64);
                        let fmt_ver = if msgpack_bytes.is_empty() { FORMAT_VERSION_CHUNKED } else { msgpack_bytes[0] };
                        return Ok(JsonValueReader {
                            msgpack_bytes: msgpack_bytes.into(),
                            total_rows,
                            format_version: fmt_ver,
                            data_pos: 0,
                            cached_chunk: None,
                            cached_row_start: 0,
                        });
                    }
                    Err(_) => {} // Decompress failed, try fallback
                }
            }
        }
        if let Ok(reader) = <Self::Reader as FallbackReader>::read_fallback(remaining) {
            src.set_position(buf_len as u64);
            return Ok(reader);
        }
        Err(anyhow::anyhow!(
            "failed to deserialize serde_json::Value column: new format and fallback format both failed"
        ))
    }

    fn validate_bounds(reader: &mut Self::Reader) -> anyhow::Result<Option<usize>> {
        Ok(Some(reader.total_rows))
    }

    /// Get an item by index. Scans forward-only through chunks; retains the current deserialized chunk in cache until index advances past that range.
    fn get(reader: &mut Self::Reader, index: usize) -> anyhow::Result<Option<Self>> {
        let bytes = &reader.msgpack_bytes;
        if index >= reader.total_rows || bytes.is_empty() {
            return Ok(None);
        }
        let fmt_ver = bytes[0];
        match fmt_ver {
            FORMAT_VERSION_CHUNKED => {
                let data_start = &bytes[1..];
                if let Some(ref cache) = reader.cached_chunk {
                    let end_row_excl = reader.cached_row_start + cache.len();
                    if index >= reader.cached_row_start && index < end_row_excl {
                        return Ok(cache.get(index - reader.cached_row_start).cloned());
                    }
                }
                let mut local_pos = reader.data_pos;
                let mut current_row_offset: usize = match &reader.cached_chunk {
                    Some(cache) => reader.cached_row_start + cache.len(),
                    None => 0usize,
                };
                loop {
                    if local_pos >= data_start.len() || current_row_offset >= reader.total_rows {
                        break; // Ran out of data.
                    }
                    let chunk_end_excl = std::cmp::min(current_row_offset + CHUNK_SIZE, reader.total_rows);
                    if index < chunk_end_excl {
                        match Self::Reader::deserialize_chunk(data_start, &mut local_pos) {
                            Some(chunk) => {
                                reader.cached_chunk = Some(chunk.clone());
                                reader.cached_row_start = current_row_offset;
                                reader.data_pos = local_pos;
                                return Ok(chunk.get(index - current_row_offset).cloned());
                            }
                            None => break, // Deserialization failed.
                        }
                    }
                    if Self::Reader::skip_chunk(data_start, &mut local_pos).is_none() {
                        break;
                    }
                    let skipped_count = std::cmp::min(CHUNK_SIZE, reader.total_rows - current_row_offset);
                    current_row_offset += skipped_count;
                }
                return Ok(None);
            }
            _ => {
                // Flat msgpack fallback
                if index == 0 && !bytes.is_empty() {
                    return Ok(Self::Reader::deserialize_one(bytes, &mut 0usize));
                }
                Ok(None)
            }
        }
    }
}

impl FallbackReader for JsonValueReader {
    fn read_fallback(raw: &[u8]) -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        if raw.is_empty() {
            return Ok(Self::default());
        }
        let value = rmp_serde::from_slice::<serde_json::Value>(raw)
            .map_err(|e| anyhow::anyhow!("fallback deserialization failed: {}", e))?;
        Ok(JsonValueReader {
            msgpack_bytes: raw.to_vec().into(),
            total_rows: 1,
            format_version: FORMAT_VERSION_CHUNKED,
            data_pos: 0,
            cached_chunk: Some(vec![value]),
            cached_row_start: 0,
        })
    }
}

impl super::PcoFilter for serde_json::Value {
    fn filter_match(value: &Self, filter: &super::Filter) -> bool {
        matches_json_value(value, filter)
    }

    fn filter_bulk(
        reader: &mut Self::Reader, _field: usize, filter: &super::Filter, matches: &mut FilterMask,
    ) -> ::anyhow::Result<()> {
        if reader.total_rows == 0 || reader.msgpack_bytes.is_empty() {
            matches.handle_empty_column::<serde_json::Value, _>(|val| matches_json_value(val, filter));
            return Ok(());
        }
        let bytes = &reader.msgpack_bytes[1..];
        let mut pos: usize = 0;
        let mut row_idx: usize = 0;
        while row_idx < reader.total_rows && pos < bytes.len() {
            if let Some(chunk) = JsonValueReader::deserialize_chunk(bytes, &mut pos) {
                matches.build_into(row_idx, &chunk, |val| matches_json_value(val, filter));
                row_idx += chunk.len();
            } else {
                break;
            }
        }
        Ok(())
    }

    fn filter_nested(
        _reader: &mut Self::Reader, _path: &[usize], _filter: &super::Filter, _matches: &mut FilterMask,
    ) -> ::anyhow::Result<()> {
        unreachable!("filter_nested not supported for serde_json::Value");
    }

    fn resolve_filter(path: &str, json: &serde_json::Value) -> ::anyhow::Result<ResolvedFilter> {
        if !path.is_empty() && path.contains('.') {
            return Err(::anyhow::anyhow!(
                "Field '{}' is a JSON Value and nested path filtering is not supported",
                path
            ));
        }
        match json {
            serde_json::Value::Array(_) => Err(::anyhow::anyhow!(
                "Inclusion queries are not supported for JSON Value columns; use exact equality instead"
            )),
            _ if json.is_null() => {
                Err(::anyhow::anyhow!("Cannot filter JSON Value column with null query value for field '{}'", path))
            }
            _ => Ok(ResolvedFilter { path: vec![0], filter: Filter::Json(json.clone()) }),
        }
    }
}
