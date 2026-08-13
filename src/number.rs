use super::*;

#[derive(Clone, Default)]
pub struct NumberWriter<T> {
    pub values: Vec<T>,
}

#[derive(Clone, Debug)]
pub struct NumberReader<T> {
    pub values: Arc<[T]>,
    pub index: usize,
}

impl<T> Default for NumberReader<T> {
    fn default() -> Self {
        Self { values: Arc::new([]), index: 0 }
    }
}

macro_rules! impl_pco_number {
    ($t:ty, $filter_variant:ident) => {
        impl PcoSerde for $t {
            type Writer = NumberWriter<$t>;
            type Reader = NumberReader<$t>;

            fn write(data: Vec<$t>, _float_round: u32, _time_round: chrono::Duration) -> anyhow::Result<Vec<u8>> {
                let config = pco_util::config();
                Ok(pco::standalone::simple_compress(&data, &config)?)
            }

            fn read(
                src: &mut Cursor<&[u8]>, _float_round: u32, _time_round: chrono::Duration,
            ) -> anyhow::Result<Self::Reader> {
                let buf = *src.get_ref();
                let pos = src.position() as usize;
                if pos >= buf.len() {
                    return Ok(NumberReader { values: Vec::new().into(), index: 0 });
                }
                if buf[pos] == float_round::FLOAT_ROUND_MAGIC && buf.len() > pos + 1 {
                    let decimals = buf[pos + 1] as u32;
                    src.set_position((pos + 2) as u64);
                    let i64_vals = pco_util::decompress::<i64>(src)?;
                    if !i64_vals.is_empty() {
                        let f64_vals: Vec<f64> =
                            i64_vals.into_iter().map(|v| float_round::unround_float(v, decimals)).collect();
                        let coerced: Vec<$t> =
                            f64_vals.into_iter().map(|v| <$t as CoercibleNumber>::coerce_from_float(v)).collect();
                        return Ok(NumberReader { values: coerced.into(), index: 0 });
                    }
                }
                let vals = pco_util::decompress_as::<$t>(src)?;
                if vals.is_empty() {
                    return Ok(NumberReader { values: Vec::new().into(), index: 0 });
                }
                Ok(NumberReader { values: vals.into(), index: 0 })
            }

            fn validate_bounds(reader: &mut Self::Reader) -> anyhow::Result<Option<usize>> {
                Ok(Some(reader.values.len()))
            }

            fn get(reader: &mut Self::Reader, index: usize) -> anyhow::Result<Option<Self>> {
                Ok(reader.values.get(index).copied())
            }
        }

        impl PcoFilter for $t {
            fn filter_bulk(
                reader: &mut Self::Reader, _field: usize, filter: &Filter, matches: &mut FilterMask,
            ) -> Result<()> {
                match filter {
                    Filter::I64(_) | Filter::F64(_) => {
                        let tv = <$t as CoercibleNumber>::coerce_from(filter);
                        matches.build_with_and(&reader.values, |val| *val == tv)
                    }
                    Filter::Range(range) => matches.build_with_and(&reader.values, move |val| {
                        <$t as RangeCoercibleNumberExt>::matches_range(val, range)
                    }),
                    Filter::FloatRange(range) => {
                        let start = *range.start() as i64;
                        let end = *range.end() as i64;
                        matches.build_with_and(&reader.values, move |val| {
                            <$t as RangeCoercibleNumberExt>::matches_range(val, &(start..=end))
                        })
                    }
                    Filter::InclusionI64(values) => {
                        let vals = values.as_slice();
                        matches.build_with_and(&reader.values, move |val| {
                            vals.contains(&{ <$t as RangeCoercibleNumber>::to_i64(val) })
                        })
                    }
                    Filter::InclusionF64(values) => {
                        let vals = values.as_slice();
                        matches.build_with_and(&reader.values, move |val| {
                            let fval = <$t as RangeCoercibleNumber>::to_f64_val(val);
                            vals.iter().any(|v| (fval - v).abs() < f64::EPSILON)
                        })
                    }
                    _ => unreachable!("unsupported number filter: {filter:?}"),
                }
                Ok(())
            }

            fn filter_nested(
                _reader: &mut Self::Reader, _path: &[usize], _filter: &Filter, _matches: &mut FilterMask,
            ) -> Result<()> {
                unreachable!("filter_nested not supported for {}", stringify!($t));
            }

            fn filter_match(value: &Self, filter: &Filter) -> bool {
                match filter {
                    Filter::I64(_) => *value == <$t as CoercibleNumber>::coerce_from(filter),
                    Filter::Range(range) => <$t as RangeCoercibleNumberExt>::matches_range(value, range),
                    Filter::FloatRange(range) => {
                        let start = *range.start() as i64;
                        let end = *range.end() as i64;
                        <$t as RangeCoercibleNumberExt>::matches_range(value, &(start..=end))
                    }
                    Filter::InclusionI64(values) => {
                        let val: i64 = <$t as RangeCoercibleNumber>::to_i64(value);
                        values.contains(&val)
                    }
                    Filter::InclusionF64(values) => {
                        let fval = <$t as RangeCoercibleNumber>::to_f64_val(value);
                        values.iter().any(|v| (fval - v).abs() < f64::EPSILON)
                    }
                    _ => *value == <$t as CoercibleNumber>::coerce_from(filter),
                }
            }

            fn resolve_filter(path: &str, json: &serde_json::Value) -> ::anyhow::Result<ResolvedFilter> {
                if !path.is_empty() {
                    return Err(::anyhow::anyhow!("Field '{}' is a numeric leaf and cannot contain nested path", path));
                }
                if let serde_json::Value::Object(obj) = json {
                    if let (Some(start_val), Some(end_val)) = (obj.get("start"), obj.get("end")) {
                        let start = start_val
                            .as_i64()
                            .or_else(|| start_val.as_f64().map(|f| f as i64))
                            .context("Range start must be a number")?;
                        let end = end_val
                            .as_i64()
                            .or_else(|| end_val.as_f64().map(|f| f as i64))
                            .context("Range end must be a number")?;
                        return Ok(ResolvedFilter { path: vec![0], filter: Filter::Range(start..=end) });
                    }
                }
                if let serde_json::Value::Array(arr) = json {
                    if !arr.is_empty() {
                        let ints: Vec<i64> = arr.iter().filter_map(|v| v.as_i64()).collect();
                        if ints.len() == arr.len() {
                            return Ok(ResolvedFilter { path: vec![0], filter: Filter::InclusionI64(ints) });
                        }
                        let floats: Vec<f64> = arr.iter().filter_map(|v| v.as_f64()).collect();
                        if floats.len() == arr.len() {
                            return Ok(ResolvedFilter { path: vec![0], filter: Filter::InclusionF64(floats) });
                        }
                    }
                }
                let value = json.as_i64().or_else(|| json.as_f64().map(|f| f as i64));
                let value = value.with_context(|| format!("Expected numeric value for field '{}'", path))?;
                Ok(ResolvedFilter { path: vec![0], filter: Filter::$filter_variant(value) })
            }
        }
    };
}

impl_pco_number!(u8, I64);
impl_pco_number!(u16, I64);
impl_pco_number!(u32, I64);
impl_pco_number!(u64, I64);
impl_pco_number!(i8, I64);
impl_pco_number!(i16, I64);
impl_pco_number!(i32, I64);
impl_pco_number!(i64, I64);

pub trait CoercibleNumber: Sized {
    fn coerce_from(filter: &Filter) -> Self;
    fn coerce_from_int(val: i64) -> Self;
    fn coerce_from_u64(val: u64) -> Self {
        Self::coerce_from_int(val as i64)
    }
    fn coerce_from_float(val: f64) -> Self;
}

macro_rules! impl_coercible_int {
    ($t:ty, $min_i64:expr, $max_i64:expr) => {
        impl CoercibleNumber for $t {
            #[inline]
            fn coerce_from(filter: &Filter) -> Self {
                if let Filter::I64(val) = filter {
                    if *val < $min_i64 {
                        Self::MIN
                    } else if *val > $max_i64 {
                        Self::MAX
                    } else {
                        *val as Self
                    }
                } else {
                    unreachable!("unsupported filter variant {:?} for integer type", filter)
                }
            }

            #[inline]
            fn coerce_from_int(val: i64) -> Self {
                if val < $min_i64 {
                    Self::MIN
                } else if val > $max_i64 {
                    Self::MAX
                } else {
                    val as Self
                }
            }

            #[inline]
            fn coerce_from_u64(val: u64) -> Self {
                let max_u64 = $max_i64 as u64;
                if val > max_u64 { Self::MAX } else { Self::coerce_from_int(val as i64) }
            }

            #[inline]
            fn coerce_from_float(val: f64) -> Self {
                if val.is_infinite() && val > 0.0 {
                    return Self::MAX;
                }
                if val.is_infinite() && val < 0.0 {
                    return Self::MIN;
                }
                let ival = val as i64;
                if ival < $min_i64 {
                    Self::MIN
                } else if ival > $max_i64 {
                    Self::MAX
                } else {
                    ival as Self
                }
            }
        }
    };
}
impl_coercible_int!(u8, 0, u8::MAX as i64);
impl_coercible_int!(u16, 0, u16::MAX as i64);
impl_coercible_int!(u32, 0, u32::MAX as i64);
impl_coercible_int!(i8, i8::MIN as i64, i8::MAX as i64);
impl_coercible_int!(i16, i16::MIN as i64, i16::MAX as i64);
impl_coercible_int!(i32, i32::MIN as i64, i32::MAX as i64);
impl_coercible_int!(i64, i64::MIN, i64::MAX);

// Custom impl for u64: must handle values > i64::MAX without wrapping through i64
impl CoercibleNumber for u64 {
    #[inline]
    fn coerce_from(filter: &Filter) -> Self {
        if let Filter::I64(val) = filter {
            if *val < 0 { 0 } else { *val as u64 }
        } else {
            unreachable!("unsupported filter variant {:?} for u64", filter)
        }
    }

    #[inline]
    fn coerce_from_int(val: i64) -> Self {
        if val < 0 { 0 } else { val as u64 }
    }

    #[inline]
    fn coerce_from_u64(val: u64) -> Self {
        val
    }

    #[inline]
    fn coerce_from_float(val: f64) -> Self {
        if val.is_infinite() && val > 0.0 {
            return Self::MAX;
        }
        if val.is_infinite() && val < 0.0 {
            return 0;
        }
        val as u64
    }
}

pub trait RangeCoercibleNumber: Sized {
    fn range_bounds() -> (i64, i64);
    fn to_i64(&self) -> i64;
    fn to_f64_val(&self) -> f64;
}

macro_rules! impl_range_coercible_int {
    ($t:ty, $min:expr, $max:expr) => {
        impl RangeCoercibleNumber for $t {
            #[inline]
            fn range_bounds() -> (i64, i64) {
                ($min, $max)
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
impl_range_coercible_int!(u8, i64::MIN, u8::MAX as i64);
impl_range_coercible_int!(u16, i64::MIN, u16::MAX as i64);
impl_range_coercible_int!(u32, i64::MIN, u32::MAX as i64);
impl_range_coercible_int!(u64, i64::MIN, i64::MAX);
impl_range_coercible_int!(i8, i8::MIN as i64, i8::MAX as i64);
impl_range_coercible_int!(i16, i16::MIN as i64, i16::MAX as i64);
impl_range_coercible_int!(i32, i32::MIN as i64, i32::MAX as i64);
impl_range_coercible_int!(i64, i64::MIN, i64::MAX);

pub trait RangeCoercibleNumberExt: RangeCoercibleNumber {
    fn matches_range(&self, range: &RangeInclusive<i64>) -> bool {
        let val = self.to_i64();
        *range.start() <= val && val <= *range.end()
    }
}
impl<T: RangeCoercibleNumber> RangeCoercibleNumberExt for T {}
