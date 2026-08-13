use super::*;

impl PcoSerde for chrono::DateTime<chrono::Utc> {
    type Writer = NumberWriter<i64>;
    type Reader = NumberReader<i64>;

    fn write(data: Vec<Self>, _float_round: u32, time_round: chrono::Duration) -> anyhow::Result<Vec<u8>> {
        let granularity_us =
            if time_round.is_zero() { 0i64 } else { time_round.num_microseconds().unwrap_or(i64::MAX) };
        let micros: Vec<i64> = if granularity_us > 0 {
            data.into_iter().map(|dt| round_half_up(dt.timestamp_micros(), granularity_us) * granularity_us).collect()
        } else {
            data.into_iter().map(|dt| dt.timestamp_micros()).collect()
        };
        i64::write(micros, 0, time_round)
    }

    fn read(src: &mut Cursor<&[u8]>, float_round: u32, time_round: chrono::Duration) -> anyhow::Result<Self::Reader> {
        i64::read(src, float_round, time_round)
    }

    fn validate_bounds(reader: &mut Self::Reader) -> anyhow::Result<Option<usize>> {
        i64::validate_bounds(reader)
    }

    fn get(reader: &mut Self::Reader, index: usize) -> anyhow::Result<Option<Self>> {
        let micros = i64::get(reader, index)?;
        Ok(micros.map(|m| Self::from_timestamp_micros(m).unwrap_or(Self::UNIX_EPOCH)))
    }
}

// Matches the behavior of chrono::DurationRound.
fn round_half_up(v: i64, g: i64) -> i64 {
    let q = v / g;
    let r = v % g;
    let half = g / 2;
    if r >= half || (r < 0 && (-r) > half) { q + 1 } else { q }
}

impl PcoFilter for chrono::DateTime<chrono::Utc> {
    fn filter_bulk(reader: &mut Self::Reader, _field: usize, filter: &Filter, matches: &mut FilterMask) -> Result<()> {
        match filter {
            Filter::I64(target_value) => {
                let target = *target_value;
                matches.build_with_and(&reader.values, move |&val| val == target)
            }
            Filter::Range(range) => {
                let start = *range.start();
                let end = *range.end();
                matches.build_with_and(&reader.values, move |&val| start <= val && val <= end)
            }
            Filter::FloatRange(range) => {
                let start = *range.start() as i64;
                let end = *range.end() as i64;
                matches.build_with_and(&reader.values, move |&val| start <= val && val <= end)
            }
            Filter::InclusionI64(values) => {
                let vals = values.as_slice();
                matches.build_with_and(&reader.values, move |&val| vals.contains(&val))
            }
            _ => unreachable!("unsupported chrono::DateTime filter: {filter:?}"),
        }
        Ok(())
    }

    fn filter_match(value: &Self, filter: &Filter) -> bool {
        let ts = value.timestamp_micros();
        match filter {
            Filter::I64(target) => ts == *target,
            Filter::Range(range) => *range.start() <= ts && ts <= *range.end(),
            Filter::FloatRange(range) => {
                let start = *range.start() as i64;
                let end = *range.end() as i64;
                start <= ts && ts <= end
            }
            Filter::InclusionI64(values) => values.contains(&ts),
            _ => false,
        }
    }

    fn filter_nested(
        _reader: &mut Self::Reader, _path: &[usize], _filter: &Filter, _matches: &mut FilterMask,
    ) -> Result<()> {
        unreachable!("filter_nested not supported for chrono::DateTime");
    }

    fn resolve_filter(path: &str, json: &serde_json::Value) -> ::anyhow::Result<ResolvedFilter> {
        if !path.is_empty() {
            return Err(::anyhow::anyhow!("Field '{}' is a timestamp leaf and cannot contain nested path", path));
        }

        if let serde_json::Value::Object(obj) = json {
            if let (Some(start_val), Some(end_val)) = (obj.get("start"), obj.get("end")) {
                let start = parse_datetime_value(start_val, &format!("{}.start", path))
                    .context("Range 'start' must be an integer or RFC 3339 datetime string")?;
                let end = parse_datetime_value(end_val, &format!("{}.end", path))
                    .context("Range 'end' must be an integer or RFC 3339 datetime string")?;
                return Ok(ResolvedFilter { path: vec![0], filter: Filter::Range(start..=end) });
            }
        }

        if let serde_json::Value::Array(arr) = json {
            if !arr.is_empty() {
                let mut values = Vec::with_capacity(arr.len());
                for (i, v) in arr.iter().enumerate() {
                    match parse_datetime_value(v, &format!("{}[{}]", path, i)) {
                        Ok(val) => values.push(val),
                        Err(_) => {
                            return Ok(ResolvedFilter { path: vec![0], filter: Filter::InclusionI64(Vec::new()) });
                        }
                    }
                }
                if !values.is_empty() {
                    return Ok(ResolvedFilter { path: vec![0], filter: Filter::InclusionI64(values) });
                }
            }
        }

        let ts = parse_datetime_value(json, path)?;
        Ok(ResolvedFilter { path: vec![0], filter: Filter::I64(ts) })
    }
}

/// Parse a JSON value into microseconds since epoch.
/// Supports:
/// - Integer (microseconds since epoch)
/// - String (RFC 3339 datetime format)
fn parse_datetime_value(json: &serde_json::Value, field_path: &str) -> Result<i64> {
    if let Some(val) = json.as_i64() {
        return Ok(val);
    }
    let Some(s) = json.as_str() else {
        anyhow::bail!("Expected integer (microseconds since epoch) or RFC 3339 string for field '{field_path}'")
    };
    let dt = chrono::DateTime::<chrono::FixedOffset>::parse_from_str(s, "%+")
        .with_context(|| format!("Failed to parse RFC 3339 string for field '{field_path}'"))?;
    Ok(dt.with_timezone(&chrono::Utc).timestamp_micros())
}

impl CoercibleNumber for chrono::DateTime<chrono::Utc> {
    fn coerce_from(filter: &Filter) -> Self {
        if let Filter::I64(val) = filter {
            Self::from_timestamp_micros(*val).unwrap_or_else(|| if *val < 0 { Self::MIN_UTC } else { Self::MAX_UTC })
        } else {
            unreachable!("unsupported filter variant {:?} for DateTime", filter)
        }
    }

    fn coerce_from_int(val: i64) -> Self {
        Self::from_timestamp_micros(val).unwrap_or_else(|| if val < 0 { Self::MIN_UTC } else { Self::MAX_UTC })
    }

    fn coerce_from_float(val: f64) -> Self {
        let micros = val as i64;
        Self::from_timestamp_micros(micros).unwrap_or_else(|| if micros < 0 { Self::MIN_UTC } else { Self::MAX_UTC })
    }
}

impl RangeCoercibleNumber for chrono::DateTime<chrono::Utc> {
    fn range_bounds() -> (i64, i64) {
        (i64::MIN, i64::MAX)
    }
    fn to_i64(&self) -> i64 {
        self.timestamp_micros()
    }
    fn to_f64_val(&self) -> f64 {
        self.timestamp_micros() as f64
    }
}
