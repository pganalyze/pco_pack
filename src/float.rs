use super::*;

/// Internal reader for float columns that may have been serialized with or without rounding.
/// Used by f64/f32/half::f16 PcoSerde impls to transparently handle float_round encoding while keeping
/// FloatRoundStorage private. The filter logic uses the appropriate comparison strategy based on format.
#[derive(Clone, Debug)]
pub enum FloatReader<T> {
    Raw {
        values: Arc<[T]>,
    },
    /// Data stored as rounded i64s with given decimal precision (float_round > 0).
    Rounded {
        decimals: u8,
        values: Arc<[i64]>,
    },
}

impl<T> Default for FloatReader<T> {
    fn default() -> Self {
        Self::Raw { values: Arc::new([]) }
    }
}

impl PcoSerde for half::f16 {
    type Writer = NumberWriter<half::f16>;
    type Reader = FloatReader<half::f16>;

    fn write(data: Vec<half::f16>, float_round: u32, time_round: chrono::Duration) -> anyhow::Result<Vec<u8>> {
        if float_round > 0 {
            let scale = 10i64.pow(float_round) as f64;
            let rounded: Vec<i64> = data
                .into_iter()
                .map(|v| ((v.to_f64() * scale).round().clamp(i64::MIN as f64, i64::MAX as f64)) as i64)
                .collect();
            let mut out = Vec::new();
            out.push(float_round::FLOAT_ROUND_MAGIC);
            out.push(float_round as u8);
            out.extend_from_slice(&i64::write(rounded, 0, time_round)?);
            Ok(out)
        } else {
            // Store as native f16 via f64 compression
            let config = pco_util::config();
            Ok(pco::standalone::simple_compress(&data, &config)?)
        }
    }

    fn read(src: &mut Cursor<&[u8]>, _float_round: u32, _time_round: chrono::Duration) -> anyhow::Result<Self::Reader> {
        let buf = src.get_ref();
        let pos = src.position() as usize;
        if pos >= buf.len() {
            return Ok(FloatReader::Raw { values: Vec::new().into() });
        }
        // Detect format: if first byte is FLOAT_ROUND_MAGIC, it's float_round format.
        // Store as Rounded variant so filters can apply tolerance-based matching.
        if buf[pos] == float_round::FLOAT_ROUND_MAGIC && buf.len() > pos + 1 {
            let decimals = buf[pos + 1];
            src.set_position((pos + 2) as u64);
            let i64_vals = pco_util::decompress::<i64>(src)?;
            return Ok(FloatReader::Rounded { decimals, values: i64_vals.into() });
        }
        let vals = pco_util::decompress_as::<half::f16>(src)?;
        Ok(FloatReader::Raw { values: vals.into() })
    }

    fn validate_bounds(reader: &mut Self::Reader) -> anyhow::Result<Option<usize>> {
        match reader {
            FloatReader::Raw { values } => Ok(Some(values.len())),
            FloatReader::Rounded { values, .. } => Ok(Some(values.len())),
        }
    }

    fn get(reader: &mut Self::Reader, index: usize) -> anyhow::Result<Option<Self>> {
        match reader {
            FloatReader::Raw { values } => Ok(values.get(index).copied()),
            FloatReader::Rounded { decimals, values } => {
                Ok(values.get(index).map(|&v| half::f16::from_f64(float_round::unround_float(v, *decimals as u32))))
            }
        }
    }
}

impl PcoFilter for half::f16 {
    fn filter_bulk(reader: &mut Self::Reader, _field: usize, filter: &Filter, matches: &mut FilterMask) -> Result<()> {
        match reader {
            FloatReader::Raw { values } => match filter {
                Filter::F64(value) => {
                    let target = *value;
                    matches.build_with_and(values, move |&val| (val.to_f64() - target).abs() < f64::EPSILON)
                }
                Filter::Range(range) => {
                    let start = *range.start() as f64;
                    let end = *range.end() as f64;
                    matches.build_with_and(values, move |&val| {
                        let v = val.to_f64();
                        v >= start && v <= end
                    })
                }
                Filter::FloatRange(range) => matches.build_with_and(values, move |&val| {
                    let v = val.to_f64();
                    v >= *range.start() && v <= *range.end()
                }),
                Filter::InclusionF64(targets) => {
                    let vals = targets.as_slice();
                    matches.build_with_and(values, move |&val| {
                        vals.iter().any(|v| (val.to_f64() - *v).abs() < f64::EPSILON)
                    })
                }
                _ => unreachable!("unsupported f16 filter: {filter:?}"),
            },
            FloatReader::Rounded { decimals, values } => {
                let eps = 0.5_f64 / (10i64.pow(*decimals as u32) as f64);
                match filter {
                    Filter::F64(target) => {
                        matches.build_with_and(values, move |&val| {
                            let unrounded = float_round::unround_float(val, *decimals as u32);
                            (unrounded - target).abs() < eps
                        });
                    }
                    Filter::Range(range) => {
                        let start = *range.start() as f64;
                        let end = *range.end() as f64;
                        matches.build_with_and(values, move |&val| {
                            let unrounded = float_round::unround_float(val, *decimals as u32);
                            unrounded >= start && unrounded <= end
                        });
                    }
                    Filter::FloatRange(range) => {
                        matches.build_with_and(values, move |&val| {
                            let unrounded = float_round::unround_float(val, *decimals as u32);
                            unrounded >= *range.start() && unrounded <= *range.end()
                        });
                    }
                    Filter::InclusionF64(targets) => {
                        matches.build_with_and(values, move |&val| {
                            let unrounded = float_round::unround_float(val, *decimals as u32);
                            targets.iter().any(|t| (unrounded - t).abs() < eps)
                        });
                    }
                    _ => unreachable!("unsupported f16 filter: {filter:?}"),
                }
            }
        }
        Ok(())
    }

    fn filter_nested(
        _reader: &mut Self::Reader, _path: &[usize], _filter: &Filter, _matches: &mut FilterMask,
    ) -> Result<()> {
        unreachable!("filter_nested not supported for f16");
    }

    fn filter_match(value: &Self, filter: &Filter) -> bool {
        match filter {
            Filter::F64(target) => (value.to_f64() - target).abs() < f64::EPSILON,
            Filter::Range(range) => {
                let start = *range.start() as f64;
                let end = *range.end() as f64;
                let v = value.to_f64();
                v >= start && v <= end
            }
            Filter::FloatRange(range) => {
                let v = value.to_f64();
                v >= *range.start() && v <= *range.end()
            }
            Filter::InclusionF64(values) => {
                let fval = value.to_f64();
                values.iter().any(|v| (fval - v).abs() < f64::EPSILON)
            }
            _ => false,
        }
    }

    fn resolve_filter(path: &str, json: &serde_json::Value) -> ::anyhow::Result<ResolvedFilter> {
        if !path.is_empty() {
            return Err(::anyhow::anyhow!("Field '{}' is a numeric leaf and cannot contain nested path", path));
        }
        if let serde_json::Value::Object(obj) = json {
            if let (Some(start_val), Some(end_val)) = (obj.get("start"), obj.get("end")) {
                let start = start_val.as_f64().context("Range start must be a number")?;
                let end = end_val.as_f64().context("Range end must be a number")?;
                return Ok(ResolvedFilter { path: vec![0], filter: Filter::FloatRange(start..=end) });
            }
        }
        if let serde_json::Value::Array(arr) = json {
            if !arr.is_empty() {
                let floats: Vec<f64> = arr.iter().filter_map(|v| v.as_f64()).collect();
                if floats.len() == arr.len() {
                    return Ok(ResolvedFilter { path: vec![0], filter: Filter::InclusionF64(floats) });
                }
            }
        }
        let value = json_to_f64(json).with_context(|| format!("Expected numeric value for field '{}'", path))?;
        Ok(ResolvedFilter { path: vec![0], filter: Filter::F64(value) })
    }
}

impl PcoSerde for f32 {
    type Writer = NumberWriter<f32>;
    type Reader = FloatReader<f32>;

    fn write(data: Vec<f32>, float_round: u32, time_round: chrono::Duration) -> anyhow::Result<Vec<u8>> {
        if float_round > 0 {
            let scale = 10i64.pow(float_round) as f64;
            let rounded: Vec<i64> = data
                .into_iter()
                .map(|v| ((v as f64 * scale).round().clamp(i64::MIN as f64, i64::MAX as f64)) as i64)
                .collect();
            let mut out = Vec::new();
            out.push(float_round::FLOAT_ROUND_MAGIC);
            out.push(float_round as u8);
            out.extend_from_slice(&i64::write(rounded, 0, time_round)?);
            Ok(out)
        } else {
            let config = pco_util::config();
            Ok(pco::standalone::simple_compress(&data, &config)?)
        }
    }

    fn read(src: &mut Cursor<&[u8]>, _float_round: u32, _time_round: chrono::Duration) -> anyhow::Result<Self::Reader> {
        let buf = src.get_ref();
        let pos = src.position() as usize;
        if pos >= buf.len() {
            return Ok(FloatReader::Raw { values: Vec::new().into() });
        }
        if buf[pos] == float_round::FLOAT_ROUND_MAGIC && buf.len() > pos + 1 {
            let decimals = buf[pos + 1];
            src.set_position((pos + 2) as u64);
            let i64_vals = pco_util::decompress::<i64>(src)?;
            return Ok(FloatReader::Rounded { decimals, values: i64_vals.into() });
        }
        let vals = pco_util::decompress_as::<f32>(src)?;
        Ok(FloatReader::Raw { values: vals.into() })
    }

    fn validate_bounds(reader: &mut Self::Reader) -> anyhow::Result<Option<usize>> {
        match reader {
            FloatReader::Raw { values } => Ok(Some(values.len())),
            FloatReader::Rounded { values, .. } => Ok(Some(values.len())),
        }
    }

    fn get(reader: &mut Self::Reader, index: usize) -> anyhow::Result<Option<Self>> {
        match reader {
            FloatReader::Raw { values } => Ok(values.get(index).copied()),
            FloatReader::Rounded { decimals, values } => {
                Ok(values.get(index).map(|&v| float_round::unround_float(v, *decimals as u32) as f32))
            }
        }
    }
}

impl PcoFilter for f32 {
    fn filter_bulk(reader: &mut Self::Reader, _field: usize, filter: &Filter, matches: &mut FilterMask) -> Result<()> {
        match reader {
            FloatReader::Raw { values } => match filter {
                Filter::F64(value) => {
                    let target = *value;
                    matches.build_with_and(values, move |&val| (val as f64 - target).abs() < f64::EPSILON)
                }
                Filter::Range(range) => {
                    let start = *range.start() as f32;
                    let end = *range.end() as f32;
                    matches.build_with_and(values, move |&val| val >= start && val <= end)
                }
                Filter::FloatRange(range) => {
                    let start = *range.start() as f32;
                    let end = *range.end() as f32;
                    matches.build_with_and(values, move |&val| val >= start && val <= end)
                }
                Filter::InclusionF64(targets) => {
                    let vals = targets.as_slice();
                    matches
                        .build_with_and(values, move |&val| vals.iter().any(|v| (val as f64 - *v).abs() < f64::EPSILON))
                }
                _ => unreachable!("unsupported f32 filter: {filter:?}"),
            },
            FloatReader::Rounded { decimals, values } => {
                let eps = 0.5_f64 / (10i64.pow(*decimals as u32) as f64);
                match filter {
                    Filter::F64(target) => {
                        matches.build_with_and(values, move |&val| {
                            let unrounded = float_round::unround_float(val, *decimals as u32);
                            (unrounded - target).abs() < eps
                        });
                    }
                    Filter::Range(range) => {
                        let start = *range.start() as f64;
                        let end = *range.end() as f64;
                        matches.build_with_and(values, move |&val| {
                            let unrounded = float_round::unround_float(val, *decimals as u32);
                            unrounded >= start && unrounded <= end
                        });
                    }
                    Filter::FloatRange(range) => {
                        matches.build_with_and(values, move |&val| {
                            let unrounded = float_round::unround_float(val, *decimals as u32);
                            unrounded >= *range.start() && unrounded <= *range.end()
                        });
                    }
                    Filter::InclusionF64(targets) => {
                        matches.build_with_and(values, move |&val| {
                            let unrounded = float_round::unround_float(val, *decimals as u32);
                            targets.iter().any(|t| (unrounded - t).abs() < eps)
                        });
                    }
                    _ => unreachable!("unsupported f32 filter: {filter:?}"),
                }
            }
        }
        Ok(())
    }

    fn filter_nested(
        _reader: &mut Self::Reader, _path: &[usize], _filter: &Filter, _matches: &mut FilterMask,
    ) -> Result<()> {
        unreachable!("filter_nested not supported for f32");
    }

    fn filter_match(value: &Self, filter: &Filter) -> bool {
        match filter {
            Filter::F64(target) => (*value as f64 - target).abs() < f64::EPSILON,
            Filter::Range(range) => {
                let start = *range.start() as f32;
                let end = *range.end() as f32;
                *value >= start && *value <= end
            }
            Filter::FloatRange(range) => {
                let start = *range.start() as f32;
                let end = *range.end() as f32;
                *value >= start && *value <= end
            }
            Filter::InclusionF64(values) => {
                let fval = *value as f64;
                values.iter().any(|v| (fval - v).abs() < f64::EPSILON)
            }
            _ => false,
        }
    }

    fn resolve_filter(path: &str, json: &serde_json::Value) -> ::anyhow::Result<ResolvedFilter> {
        if !path.is_empty() {
            return Err(::anyhow::anyhow!("Field '{}' is a numeric leaf and cannot contain nested path", path));
        }
        if let serde_json::Value::Object(obj) = json {
            if let (Some(start_val), Some(end_val)) = (obj.get("start"), obj.get("end")) {
                let start = start_val.as_f64().context("Range start must be a number")?;
                let end = end_val.as_f64().context("Range end must be a number")?;
                return Ok(ResolvedFilter { path: vec![0], filter: Filter::FloatRange(start..=end) });
            }
        }
        if let serde_json::Value::Array(arr) = json {
            if !arr.is_empty() {
                let floats: Vec<f64> = arr.iter().filter_map(|v| v.as_f64()).collect();
                if floats.len() == arr.len() {
                    return Ok(ResolvedFilter { path: vec![0], filter: Filter::InclusionF64(floats) });
                }
            }
        }
        let value = json_to_f64(json).with_context(|| format!("Expected numeric value for field '{}'", path))?;
        Ok(ResolvedFilter { path: vec![0], filter: Filter::F64(value) })
    }
}

impl PcoSerde for f64 {
    type Writer = NumberWriter<f64>;
    type Reader = FloatReader<f64>;

    fn write(data: Vec<f64>, float_round: u32, time_round: chrono::Duration) -> anyhow::Result<Vec<u8>> {
        if float_round > 0 {
            let scale = 10i64.pow(float_round) as f64;
            let rounded: Vec<i64> =
                data.into_iter().map(|v| (v * scale).round().clamp(i64::MIN as f64, i64::MAX as f64) as i64).collect();
            let mut out = Vec::new();
            out.push(float_round::FLOAT_ROUND_MAGIC);
            out.push(float_round as u8);
            out.extend_from_slice(&i64::write(rounded, 0, time_round)?);
            Ok(out)
        } else {
            // Store as native f64
            let config = pco_util::config();
            Ok(pco::standalone::simple_compress(&data, &config)?)
        }
    }

    fn read(src: &mut Cursor<&[u8]>, _float_round: u32, _time_round: chrono::Duration) -> anyhow::Result<Self::Reader> {
        let buf = src.get_ref();
        let pos = src.position() as usize;
        if pos >= buf.len() {
            return Ok(FloatReader::Raw { values: Vec::new().into() });
        }
        if buf[pos] == float_round::FLOAT_ROUND_MAGIC && buf.len() > pos + 1 {
            let decimals = buf[pos + 1];
            src.set_position((pos + 2) as u64);
            let i64_vals = pco_util::decompress::<i64>(src)?;
            return Ok(FloatReader::Rounded { decimals, values: i64_vals.into() });
        }
        let vals = pco_util::decompress_as::<f64>(src)?;
        Ok(FloatReader::Raw { values: vals.into() })
    }

    fn validate_bounds(reader: &mut Self::Reader) -> anyhow::Result<Option<usize>> {
        match reader {
            FloatReader::Raw { values } => Ok(Some(values.len())),
            FloatReader::Rounded { values, .. } => Ok(Some(values.len())),
        }
    }

    fn get(reader: &mut Self::Reader, index: usize) -> anyhow::Result<Option<Self>> {
        match reader {
            FloatReader::Raw { values } => Ok(values.get(index).copied()),
            FloatReader::Rounded { decimals, values } => {
                Ok(values.get(index).map(|&v| float_round::unround_float(v, *decimals as u32)))
            }
        }
    }
}

impl PcoFilter for f64 {
    fn filter_bulk(reader: &mut Self::Reader, _field: usize, filter: &Filter, matches: &mut FilterMask) -> Result<()> {
        match reader {
            FloatReader::Raw { values } => match filter {
                Filter::F64(value) => {
                    let target = *value;
                    matches.build_with_and(values, move |&val| val == target)
                }
                Filter::Range(range) => {
                    let start = *range.start() as f64;
                    let end = *range.end() as f64;
                    matches.build_with_and(values, move |&val| val >= start && val <= end)
                }
                Filter::FloatRange(range) => {
                    let start = *range.start();
                    let end = *range.end();
                    matches.build_with_and(values, move |&val| val >= start && val <= end)
                }
                Filter::InclusionF64(targets) => {
                    let vals = targets.as_slice();
                    matches.build_with_and(values, move |&val| vals.iter().any(|v| (val - v).abs() < f64::EPSILON))
                }
                _ => unreachable!("unsupported f64 filter: {filter:?}"),
            },
            FloatReader::Rounded { decimals, values } => {
                let eps = 0.5_f64 / (10i64.pow(*decimals as u32) as f64);
                match filter {
                    Filter::F64(target) => {
                        matches.build_with_and(values, move |&val| {
                            let unrounded = float_round::unround_float(val, *decimals as u32);
                            (unrounded - target).abs() < eps
                        });
                    }
                    Filter::Range(range) => {
                        let start = *range.start() as f64;
                        let end = *range.end() as f64;
                        matches.build_with_and(values, move |&val| {
                            let unrounded = float_round::unround_float(val, *decimals as u32);
                            unrounded >= start && unrounded <= end
                        });
                    }
                    Filter::FloatRange(range) => {
                        matches.build_with_and(values, move |&val| {
                            let unrounded = float_round::unround_float(val, *decimals as u32);
                            unrounded >= *range.start() && unrounded <= *range.end()
                        });
                    }
                    Filter::InclusionF64(targets) => {
                        matches.build_with_and(values, move |&val| {
                            let unrounded = float_round::unround_float(val, *decimals as u32);
                            targets.iter().any(|t| (unrounded - t).abs() < eps)
                        });
                    }
                    _ => unreachable!("unsupported f64 filter: {filter:?}"),
                }
            }
        }
        Ok(())
    }

    fn filter_nested(
        _reader: &mut Self::Reader, _path: &[usize], _filter: &Filter, _matches: &mut FilterMask,
    ) -> Result<()> {
        unreachable!("filter_nested not supported for f64");
    }

    fn filter_match(value: &Self, filter: &Filter) -> bool {
        match filter {
            Filter::F64(target) => *value == *target,
            Filter::Range(range) => {
                let start = *range.start() as f64;
                let end = *range.end() as f64;
                *value >= start && *value <= end
            }
            Filter::FloatRange(range) => *value >= *range.start() && *value <= *range.end(),
            Filter::InclusionF64(values) => values.iter().any(|v| (*value - v).abs() < f64::EPSILON),
            _ => false,
        }
    }

    fn resolve_filter(path: &str, json: &serde_json::Value) -> ::anyhow::Result<ResolvedFilter> {
        if !path.is_empty() && !path.chars().all(|c| c.is_ascii_digit()) {
            return Err(::anyhow::anyhow!("Field '{}' is a numeric leaf and cannot contain nested path", path));
        }
        if let serde_json::Value::Object(obj) = json {
            if let (Some(start_val), Some(end_val)) = (obj.get("start"), obj.get("end")) {
                let start = start_val.as_f64().context("Range start must be a number")?;
                let end = end_val.as_f64().context("Range end must be a number")?;
                return Ok(ResolvedFilter { path: vec![0], filter: Filter::FloatRange(start..=end) });
            }
        }
        if let serde_json::Value::Array(arr) = json {
            if !arr.is_empty() {
                let floats: Vec<f64> = arr.iter().filter_map(|v| v.as_f64()).collect();
                if floats.len() == arr.len() {
                    return Ok(ResolvedFilter { path: vec![0], filter: Filter::InclusionF64(floats) });
                }
            }
        }
        let value = json_to_f64(json).with_context(|| format!("Expected numeric value for field '{}'", path))?;
        Ok(ResolvedFilter { path: vec![0], filter: Filter::F64(value) })
    }
}

impl CoercibleNumber for half::f16 {
    #[inline]
    fn coerce_from(filter: &Filter) -> Self {
        if let Filter::F64(val) = filter {
            half::f16::from_f64(*val)
        } else {
            unreachable!("unsupported filter variant {:?} for f16", filter)
        }
    }

    #[inline]
    fn coerce_from_int(val: i64) -> Self {
        half::f16::from_f64(val as f64)
    }

    #[inline]
    fn coerce_from_u64(val: u64) -> Self {
        half::f16::from_f64(val as f64)
    }

    #[inline]
    fn coerce_from_float(val: f64) -> Self {
        half::f16::from_f64(val)
    }
}

impl RangeCoercibleNumber for half::f16 {
    #[inline]
    fn range_bounds() -> (i64, i64) {
        (i64::MIN, i64::MAX)
    }
    #[inline]
    fn to_i64(&self) -> i64 {
        self.to_f64() as i64
    }
    #[inline]
    fn to_f64_val(&self) -> f64 {
        self.to_f64()
    }
}

macro_rules! impl_coercible_float {
    ($t:ty) => {
        impl CoercibleNumber for $t {
            #[inline]
            fn coerce_from(filter: &Filter) -> Self {
                if let Filter::F64(val) = filter {
                    *val as Self // Floating-point downcast
                } else {
                    unreachable!("unsupported filter variant {:?} for float type", filter)
                }
            }

            #[inline]
            fn coerce_from_int(val: i64) -> Self {
                val as Self
            }

            #[inline]
            fn coerce_from_u64(val: u64) -> Self {
                // Use f64 intermediate to avoid wrapping through i64
                val as f64 as Self
            }

            #[inline]
            fn coerce_from_float(val: f64) -> Self {
                val as Self
            }
        }

        impl RangeCoercibleNumber for $t {
            #[inline]
            fn range_bounds() -> (i64, i64) {
                (i64::MIN, i64::MAX)
            }
            #[inline]
            fn to_i64(&self) -> i64 {
                *self as i64
            }
            #[inline]
            fn to_f64_val(&self) -> f64 {
                *self as f64
            }
        }
    };
}
impl_coercible_float!(f32);
impl_coercible_float!(f64);

fn json_to_f64(v: &serde_json::Value) -> Option<f64> {
    v.as_f64()
}
