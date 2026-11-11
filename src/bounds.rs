//! Write-time value bounds that let filters skip a column without reading it.
//!
//! A bounds prefix on a column blob lets `metadata_excludes` rule the blob out
//! without parsing pco headers or decompressing anything. Prefix bytes cannot
//! collide with a legacy blob's first byte: pco standalone streams always
//! start with their magic (`pco!`, 0x70) and float_round blobs with
//! FLOAT_ROUND_MAGIC (0xFE).
//!
//! Layout: `[BOUNDS_MAGIC][kind][exact][min i64 LE][max i64 LE][decimals]`.
//! The prefix length is identical for every type because schema evolution
//! reads a blob written by one type's `write` through another type's `read`;
//! readers strip without knowing the writer. `kind` records the writer so
//! bounds are only ever consulted in the domain they were folded from.
//! `exact` is 1 when min/max bound every stored value in the i64 domain; 0
//! marks bounds that may not be trusted (for example a u64 value crossing
//! `i64::MAX`, where the i64 projection filters use stops being contiguous).
//! Floats store their decoded f64 min/max and the blob's float_round
//! `decimals` (0 when uncompressed); their `exact` is always 1 since the
//! decoded domain is contiguous.
//!
//! The skip decision trusts the prefix exactly as much as the read path trusts
//! `start_at`/`end_at` and the index fields: correctness lives in the chunk
//! metadata, and foreign or corrupt files are out of contract.

use crate::{CoercibleNumber, Filter, RangeCoercibleNumber};

pub(crate) const BOUNDS_MAGIC: u8 = 0xFD;
pub(crate) const BOUNDS_PREFIX_LEN: usize = 20;

/// The writer's value domain, echoed in the bounds prefix. Float kinds carry
/// the stored width because f32's `filter_bulk` casts range endpoints to f32
/// before comparing, and the bounds check must apply the same cast.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub(crate) enum BoundsKind {
    I8 = 0,
    I16 = 1,
    I32 = 2,
    I64 = 3,
    U8 = 4,
    U16 = 5,
    U32 = 6,
    U64 = 7,
    F64 = 8,
    F32 = 9,
    F16 = 10,
}

/// Advance `pos` past a bounds prefix when present.
#[inline]
pub(crate) fn strip_prefix(buf: &[u8], pos: usize) -> usize {
    if buf.get(pos) == Some(&BOUNDS_MAGIC) && buf.len() >= pos + BOUNDS_PREFIX_LEN {
        pos + BOUNDS_PREFIX_LEN
    } else {
        pos
    }
}

#[inline]
pub(crate) fn push_bounds(out: &mut Vec<u8>, kind: BoundsKind, exact: bool, min: i64, max: i64, decimals: u8) {
    out.push(BOUNDS_MAGIC);
    out.push(kind as u8);
    out.push(exact as u8);
    out.extend_from_slice(&min.to_le_bytes());
    out.extend_from_slice(&max.to_le_bytes());
    out.push(decimals);
}

/// Float bounds carry f64 bit patterns in the same 8-byte fields.
#[inline]
pub(crate) fn push_float_bounds(out: &mut Vec<u8>, kind: BoundsKind, min: f64, max: f64, decimals: u8) {
    out.push(BOUNDS_MAGIC);
    out.push(kind as u8);
    out.push(1);
    out.extend_from_slice(&min.to_le_bytes());
    out.extend_from_slice(&max.to_le_bytes());
    out.push(decimals);
}

/// Rule out `filter` against integer bounds written by `kind`. A missing
/// prefix, a kind mismatch, or an inexact fold all answer false (never skip).
/// Comparisons mirror `filter_bulk` exactly, including the saturation filter
/// targets pass through. The int domain never sees `Filter::F64`: resolution
/// routes float equality to float columns, whose bulk path is the guard there.
pub(crate) fn int_bounds_excludes<T: CoercibleNumber + RangeCoercibleNumber>(
    compressed: &[u8], filter: &Filter, kind: BoundsKind,
) -> bool {
    if compressed.len() < BOUNDS_PREFIX_LEN
        || compressed[0] != BOUNDS_MAGIC
        || BoundsKind::from(compressed[1]) != kind
        || compressed[2] != 1
    {
        return false;
    }
    let min = i64::from_le_bytes(compressed[3..11].try_into().unwrap());
    let max = i64::from_le_bytes(compressed[11..19].try_into().unwrap());
    match filter {
        Filter::I64(target) => {
            let tv = T::coerce_from_int(*target);
            let tv = RangeCoercibleNumber::to_i64(&tv);
            tv < min || tv > max
        }
        Filter::Range(range) => *range.end() < min || *range.start() > max,
        // Integer columns truncate float range endpoints before comparing.
        Filter::FloatRange(range) => {
            let start = *range.start() as i64;
            let end = *range.end() as i64;
            end < min || start > max
        }
        Filter::InclusionI64(values) => values.iter().all(|v| *v < min || *v > max),
        Filter::InclusionF64(values) => {
            let minf = min as f64;
            let maxf = max as f64;
            values.iter().all(|v| minf - *v >= f64::EPSILON || *v - maxf >= f64::EPSILON)
        }
        _ => false,
    }
}

/// Rule out `filter` against float bounds written by `kind`. A missing
/// prefix, a kind other than the reader's own, or corrupt decimals all answer
/// false (never skip), so a blob folded in another domain can never cause a
/// cross-width skip. Tolerances mirror `filter_bulk`: rounding tolerance for
/// rounded blobs, f64::EPSILON for uncompressed equality and inclusion. NaN
/// never matches any predicate, so an all-NaN blob (empty bounds) excludes
/// trivially.
pub(crate) fn float_bounds_excludes(compressed: &[u8], filter: &Filter, kind: BoundsKind) -> bool {
    let Some((exact, min, max, decimals)) = parse_float_bounds(compressed, kind) else {
        return false;
    };
    if !exact {
        return false;
    }
    // Uncompressed f32 columns compare Range/FloatRange endpoints after an
    // f32 cast (matching filter_bulk); other domains compare in f64.
    let f32_cast = kind == BoundsKind::F32 && decimals == 0;
    let eps = if decimals == 0 { f64::EPSILON } else { 0.5_f64 / (10i64.pow(decimals as u32) as f64) };
    match filter {
        Filter::F64(target) => excludes_point(min, max, *target, eps),
        Filter::InclusionF64(values) => values.iter().all(|v| excludes_point(min, max, *v, eps)),
        Filter::Range(range) => {
            let (start, end) = if f32_cast {
                (*range.start() as f32 as f64, *range.end() as f32 as f64)
            } else {
                (*range.start() as f64, *range.end() as f64)
            };
            max < start || min > end
        }
        Filter::FloatRange(range) => {
            let (start, end) = if f32_cast {
                (*range.start() as f32 as f64, *range.end() as f32 as f64)
            } else {
                (*range.start(), *range.end())
            };
            max < start || min > end
        }
        _ => false,
    }
}

#[inline]
fn parse_float_bounds(buf: &[u8], kind: BoundsKind) -> Option<(bool, f64, f64, u8)> {
    if buf.len() < BOUNDS_PREFIX_LEN || buf[0] != BOUNDS_MAGIC || BoundsKind::from(buf[1]) != kind {
        return None;
    }
    // The writer's `float_round` is compile-time capped, so an out-of-range
    // decimals byte is corruption: the tolerance derived from decimals becomes
    // meaningless, so answer nothing.
    let decimals = buf[19];
    if decimals as u32 > crate::float_round::MAX_FLOAT_ROUND_PRECISION {
        return None;
    }
    let exact = buf[2] == 1;
    let min = f64::from_le_bytes(buf[3..11].try_into().unwrap());
    let max = f64::from_le_bytes(buf[11..19].try_into().unwrap());
    Some((exact, min, max, decimals))
}

/// A point predicate (equality, or a set of points sharing one tolerance)
/// matches some value in `[min, max]` iff the bounds's distance to the point
/// is under tolerance. Distances are computed by subtraction: `min - target`
/// and `target - max` are exact when the operands are close, whereas adding
/// a tiny tolerance to a large-magnitude target can round back to the
/// original and fabricate a skip. An empty bounds excludes every point.
#[inline]
fn excludes_point(min: f64, max: f64, target: f64, eps: f64) -> bool {
    min - target >= eps || target - max >= eps
}

/// Fold a min/max over values that round-trip to themselves in the filter
/// domain (integers). `None` for an empty column.
pub(crate) fn fold_int_bounds<T: Ord + Copy>(data: &[T]) -> Option<(T, T)> {
    data.iter().fold(None, |acc, &v| match acc {
        Some((min, max)) => Some((min.min(v), max.max(v))),
        None => Some((v, v)),
    })
}

/// Fold a min/max over the f64 view of float values. NaN values never match
/// any predicate, so they are skipped; a column of only NaN yields `None`,
/// empty bounds.
pub(crate) fn fold_float_bounds(data: impl IntoIterator<Item = f64>) -> Option<(f64, f64)> {
    data.into_iter().fold(None, |acc, v| {
        if v.is_nan() {
            return acc;
        }
        match acc {
            Some((min, max)) => Some((min.min(v), max.max(v))),
            None => Some((v, v)),
        }
    })
}

/// Round `data` to `decimals` places for i64 storage, matching the
/// float_round write path (clamp to the i64 domain).
pub(crate) fn round_to_i64(data: impl IntoIterator<Item = f64>, decimals: u32) -> Vec<i64> {
    let scale = 10i64.pow(decimals) as f64;
    data.into_iter().map(|v| (v * scale).round().clamp(i64::MIN as f64, i64::MAX as f64) as i64).collect()
}

impl From<u8> for BoundsKind {
    #[inline]
    fn from(b: u8) -> Self {
        match b {
            0 => BoundsKind::I8,
            1 => BoundsKind::I16,
            2 => BoundsKind::I32,
            3 => BoundsKind::I64,
            4 => BoundsKind::U8,
            5 => BoundsKind::U16,
            6 => BoundsKind::U32,
            7 => BoundsKind::U64,
            8 => BoundsKind::F64,
            9 => BoundsKind::F32,
            _ => BoundsKind::F16,
        }
    }
}
