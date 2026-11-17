//! Write-time bounds prefix: soundness and skip behavior.
use crate as pco_pack;
use crate::bounds::{BOUNDS_MAGIC, BoundsKind};
use crate::{Filter, FilterMask, LazyReader, PcoFilter, PcoPack, PcoSerde};
use std::io::Cursor;

fn write_col<T: PcoSerde>(vals: Vec<T>) -> Vec<u8> {
    T::write(vals, 0, chrono::Duration::zero()).unwrap()
}

/// Run one filter through the lazy reader and return surviving row count.
/// Every blob handed here holds 16 rows.
fn filter_ones<T>(blob: Vec<u8>, filter: &Filter) -> usize
where
    T: PcoSerde + PcoFilter + Default,
    T::Reader: Clone + Default,
{
    let mut reader = LazyReader::<T>::new(blob, 0, chrono::Duration::zero());
    let mut mask = FilterMask::ones(16);
    reader.filter_in_group(filter, &mut mask).unwrap();
    mask.count_ones()
}

#[test]
fn metadata_skips_disjoint_range_on_zero_column() {
    let blob = write_col(vec![0i64; 1000]);
    assert!(i64::metadata_excludes(&blob, &Filter::Range(10..=i64::MAX)));
    assert!(i64::metadata_excludes(&blob, &Filter::Range(-5000..=-1)));
    // Overlapping ranges must never skip.
    assert!(!i64::metadata_excludes(&blob, &Filter::Range(0..=i64::MAX)));
    assert!(!i64::metadata_excludes(&blob, &Filter::I64(0)));
}

#[test]
fn metadata_skips_eq_outside_bounds() {
    let blob = write_col(vec![500i64; 100]);
    assert!(i64::metadata_excludes(&blob, &Filter::I64(501)));
    assert!(i64::metadata_excludes(&blob, &Filter::I64(0)));
    assert!(!i64::metadata_excludes(&blob, &Filter::I64(500)));
}

#[test]
fn metadata_skips_out_of_bounds_inclusion() {
    let blob = write_col(vec![7i64; 64]);
    assert!(i64::metadata_excludes(&blob, &Filter::InclusionI64([1, 2, 3].into_iter().collect())));
    assert!(!i64::metadata_excludes(&blob, &Filter::InclusionI64([6, 7, 8].into_iter().collect())));
}

#[test]
fn metadata_skips_delta_modes_via_bounds() {
    // Monotonic runs pick delta encoding; pco header metadata hides the
    // original bounds in latents, but the stored bounds bound them exactly.
    let blob = write_col((0..100_000i64).map(|i| 1_700_000_000_000_000 + i).collect::<Vec<_>>());
    assert!(i64::metadata_excludes(&blob, &Filter::Range(0..=1)));
    assert!(i64::metadata_excludes(&blob, &Filter::I64(0)));
    // Same for bucket-run columns even when the target is disjoint.
    let buckets: Vec<i64> = (0..100_000).map(|i| (i / 1000) * 1000).collect();
    let blob = write_col(buckets);
    assert!(i64::metadata_excludes(&blob, &Filter::I64(-1)));
    // Targets inside the bounds must still decompress.
    assert!(!i64::metadata_excludes(&blob, &Filter::I64(500)));
}

#[test]
fn metadata_skips_multi_chunk_blobs_via_bounds() {
    // Beyond DEFAULT_MAX_PAGE_N pco splits the stream; the stored bounds bound
    // every chunk collectively, so skips stay sound multi-chunk.
    let blob = write_col(vec![0i64; 300_000]);
    assert!(i64::metadata_excludes(&blob, &Filter::Range(10..=i64::MAX)));
    assert!(!i64::metadata_excludes(&blob, &Filter::Range(-10..=10)));
}

#[test]
fn metadata_guards_empty_and_malformed() {
    assert!(!i64::metadata_excludes(b"", &Filter::Range(0..=1)));
    assert!(!i64::metadata_excludes(b"garbage bytes", &Filter::Range(0..=1)));
    // A prefix truncated below its full length is not a prefix.
    let mut short = write_col(vec![0i64; 8]);
    short.truncate(19);
    assert!(!i64::metadata_excludes(&short, &Filter::I64(999)));
    // Zero-row blob: trivially nothing to skip on.
    assert!(!i64::metadata_excludes(&write_col(Vec::<i64>::new()), &Filter::Range(0..=1)));
}

#[test]
fn inexact_bounds_never_skip() {
    // exact = 0 voids the skip answer even when min/max sit far from target.
    let mut out = vec![BOUNDS_MAGIC, BoundsKind::I64 as u8, 0];
    out.extend_from_slice(&1i64.to_le_bytes());
    out.extend_from_slice(&2i64.to_le_bytes());
    out.push(0);
    assert!(!i64::metadata_excludes(&out, &Filter::I64(999)));
}

#[test]
fn metadata_type_mismatch_never_skips() {
    let blob = write_col(vec![3u32; 50]);
    assert!(!i64::metadata_excludes(&blob, &Filter::Range(1000..=2000)));
}

#[test]
fn metadata_signed_small_type_bounds() {
    let blob = write_col(vec![-5i8; 200]);
    assert!(i8::metadata_excludes(&blob, &Filter::Range(100..=127)));
    assert!(i8::metadata_excludes(&blob, &Filter::I64(100)));
    assert!(!i8::metadata_excludes(&blob, &Filter::I64(-5)));
    assert!(!i8::metadata_excludes(&blob, &Filter::Range(-5..=127)));
}

#[test]
fn metadata_u64_straddles_i64_max() {
    let vals: Vec<u64> = (0..1000).map(|i| (i64::MAX as u64) + i).collect();
    let blob = write_col(vals.clone());
    // Saturating i64-space bounds must never fabricate a skip of large rows.
    assert!(!u64::metadata_excludes(&blob, &Filter::Range(i64::MAX..=i64::MAX)));
    assert!(!u64::metadata_excludes(&blob, &Filter::I64(0)));
    // An equality target that does match still returns its row through the
    // lazy reader: the inexact prefix cannot clear the mask.
    let mut reader = LazyReader::<u64>::new(blob, 0, chrono::Duration::zero());
    let mut mask = FilterMask::ones(vals.len());
    reader.filter_in_group(&Filter::I64(i64::MAX), &mut mask).unwrap();
    assert_eq!(mask.count_ones(), 1);
}

#[test]
fn metadata_float_variants_on_int_columns() {
    let blob = write_col(vec![0i64; 100]);
    // Rows are integral; |row - 100.0| can never be within EPS.
    assert!(i64::metadata_excludes(&blob, &Filter::InclusionF64(vec![100.0, 200.0])));
    assert!(!i64::metadata_excludes(&blob, &Filter::InclusionF64(vec![0.0, 200.0])));
    // Truncated bounds mirror filter_bulk row semantics exactly:
    // FloatRange(0.5..=9.5) truncates to (0..=9) and still hits a zero row.
    assert!(!i64::metadata_excludes(&blob, &Filter::FloatRange(0.5..=9.5)));
    let blob = write_col(vec![10i64; 100]);
    assert!(i64::metadata_excludes(&blob, &Filter::FloatRange(0.5..=9.5)));
    assert!(!i64::metadata_excludes(&blob, &Filter::FloatRange(0.5..=10.5)));
}

#[test]
fn every_int_width_skips_and_hits() {
    macro_rules! assert_int_width {
        ($t:ty) => {{
            let blob = write_col(vec![10 as $t; 16]);
            // Disjoint equality, range, and inclusion all skip to empty.
            assert_eq!(filter_ones::<$t>(blob.clone(), &Filter::I64(11)), 0);
            assert_eq!(filter_ones::<$t>(blob.clone(), &Filter::Range(20..=30)), 0);
            assert_eq!(filter_ones::<$t>(blob.clone(), &Filter::InclusionI64([1, 2].into_iter().collect())), 0);
            // The same shapes that cover the stored value return every row.
            assert_eq!(filter_ones::<$t>(blob.clone(), &Filter::I64(10)), 16);
            assert_eq!(filter_ones::<$t>(blob.clone(), &Filter::Range(5..=15)), 16);
            assert_eq!(filter_ones::<$t>(blob, &Filter::InclusionI64([9, 10].into_iter().collect())), 16);
        }};
    }
    assert_int_width!(i8);
    assert_int_width!(i16);
    assert_int_width!(i32);
    assert_int_width!(i64);
    assert_int_width!(u8);
    assert_int_width!(u16);
    assert_int_width!(u32);
    assert_int_width!(u64);
}

#[test]
fn every_float_type_skips_and_hits() {
    macro_rules! assert_float_type {
        ($t:ty, $v:expr, $target:expr) => {{
            let blob = write_col(vec![$v; 16]);
            assert_eq!(filter_ones::<$t>(blob.clone(), &Filter::F64(9.0)), 0);
            assert_eq!(filter_ones::<$t>(blob.clone(), &Filter::FloatRange(5.0..=6.0)), 0);
            assert_eq!(filter_ones::<$t>(blob.clone(), &Filter::InclusionF64(vec![1.0, 2.0])), 0);
            assert_eq!(filter_ones::<$t>(blob.clone(), &Filter::F64($target)), 16);
            assert_eq!(filter_ones::<$t>(blob, &Filter::FloatRange(-1.0..=4.0)), 16);
        }};
    }
    assert_float_type!(half::f16, half::f16::from_f32(2.5), 2.5f64);
    assert_float_type!(f32, 2.5f32, 2.5f64);
    assert_float_type!(f64, 2.5f64, 2.5f64);
}

#[test]
fn cross_width_float_read_never_skips() {
    // An f64-prefixed blob whose bounds hold the exact f64 0.1, consulted
    // through the f32 reader with the f32-rounded target. The f32 reader
    // coerces the stored value down to f32, where the target matches exactly,
    // yet the tighter f64-domain bounds would exclude that target by ~1.5e-9,
    // far over f64::EPSILON. Only the BoundsKind gate prevents the skip.
    let blob = write_col(vec![0.1f64; 8]);
    let target = f64::from(0.1f32);
    // The f64 reader may skip: raw f64 equality is exact and 0.1 != f32 0.1.
    assert!(f64::metadata_excludes(&blob, &Filter::F64(target)));
    // The f32 reader must never see these bounds.
    assert!(!f32::metadata_excludes(&blob, &Filter::F64(target)));
    // And reading the blob through f32 really does find the rows.
    let mut reader = LazyReader::<f32>::new(blob, 0, chrono::Duration::zero());
    let mut mask = FilterMask::ones(8);
    reader.filter_in_group(&Filter::F64(target), &mut mask).unwrap();
    assert_eq!(mask.count_ones(), 8);
}

/// Hand-assemble a full float bounds prefix with caller-chosen bytes.
fn float_prefix(kind: BoundsKind, exact: u8, min: f64, max: f64, decimals: u8) -> Vec<u8> {
    let mut out = vec![BOUNDS_MAGIC, kind as u8, exact];
    out.extend_from_slice(&min.to_le_bytes());
    out.extend_from_slice(&max.to_le_bytes());
    out.push(decimals);
    out
}

#[test]
fn corrupt_bounds_decimals_never_skip() {
    // Bounds in [1.0, 2.0] would exclude a 9.0 target, but an out-of-range
    // decimals byte is corruption and voids the whole prefix.
    let blob = float_prefix(BoundsKind::F64, 1, 1.0, 2.0, 99);
    assert!(!f64::metadata_excludes(&blob, &Filter::F64(9.0)));
    assert!(!f64::metadata_excludes(&blob, &Filter::Range(9..=10)));
}

#[test]
fn corrupted_prefix_byte_rejected_but_read_still_works() {
    // Mutating the blob's bounds decimals byte (offset 19) makes the bounds
    // untrusted while leaving the body intact.
    let mut blob = f64::write(vec![1.5f64, 2.5], 2, chrono::Duration::zero()).unwrap();
    assert_eq!(blob[19], 2);
    blob[19] = 200;
    assert!(!f64::metadata_excludes(&blob, &Filter::F64(500.0)));
    let mut cur = Cursor::new(&blob[..]);
    let mut reader = f64::read(&mut cur, 0, chrono::Duration::zero()).unwrap();
    assert_eq!(f64::reader_len(&mut reader).unwrap(), 2);
    assert_eq!(f64::get(&mut reader, 1).unwrap(), Some(2.5));
}

#[test]
fn corrupt_float_round_body_is_rejected_on_read() {
    let blob: Vec<u8> = vec![crate::float_round::FLOAT_ROUND_MAGIC, 99];
    let mut cur = Cursor::new(&blob[..]);
    assert!(f64::read(&mut cur, 0, chrono::Duration::zero()).is_err());
    let mut cur = Cursor::new(&blob[..]);
    assert!(i64::read(&mut cur, 0, chrono::Duration::zero()).is_err());
}

#[test]
fn metadata_empty_column_never_skips() {
    let blob = write_col(Vec::<i64>::new());
    // The inverted sentinel must not preempt the read path's empty-column
    // rule, which decides row survival against T::default().
    assert!(!i64::metadata_excludes(&blob, &Filter::I64(999)));
    assert!(!i64::metadata_excludes(&blob, &Filter::Range(5..=9)));
    assert!(!i64::metadata_excludes(&blob, &Filter::InclusionI64([7, 8].into_iter().collect())));
    // Rows survive exactly when the default matches, as before bounds existed.
    assert_eq!(filter_ones::<i64>(blob.clone(), &Filter::I64(0)), 16);
    assert_eq!(filter_ones::<i64>(blob.clone(), &Filter::Range(0..=1)), 16);
    assert_eq!(filter_ones::<i64>(blob, &Filter::I64(5)), 0);
}

#[test]
fn all_nan_column_defers_to_read_path() {
    // NaN never matches any predicate, so the fold leaves empty bounds. The
    // skip answers false there (empty bounds cannot distinguish an all-NaN
    // column from an empty one), and the read path clears the rows itself.
    let blob = write_col(vec![f64::NAN; 16]);
    assert!(!f64::metadata_excludes(&blob, &Filter::F64(1.0)));
    assert!(!f64::metadata_excludes(&blob, &Filter::InclusionF64(vec![1.0, 2.0])));
    assert_eq!(filter_ones::<f64>(blob, &Filter::F64(1.0)), 0);
}

#[test]
fn lazy_reader_skips_disjoint_filter() {
    let blob = write_col(vec![0i64; 10_000]);
    let mut reader = LazyReader::<i64>::new(blob, 0, chrono::Duration::zero());
    let mut mask = FilterMask::ones(10_000);
    reader.filter_in_group(&Filter::Range(10..=i64::MAX), &mut mask).unwrap();
    assert_eq!(mask.count_ones(), 0);

    // An overlapping filter still produces correct per-row results.
    let mut reader = LazyReader::<i64>::new(write_col(vec![1i64, 2, 3, 4]), 0, chrono::Duration::zero());
    let mut mask = FilterMask::ones(4);
    reader.filter_in_group(&Filter::Range(2..=3), &mut mask).unwrap();
    assert_eq!(mask.count_ones(), 2);
    assert_eq!(mask.as_bitslice().iter().map(|b| *b).collect::<Vec<bool>>(), vec![false, true, true, false]);
}

#[test]
fn e2e_filter_matches_predicate_across_chunks() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    #[pco_pack(timestamp = ts, index = [id])]
    struct Rec {
        id: i64,
        ts: i64,
        calls: i64,
    }
    let mut rows: Vec<Rec> = Vec::new();
    // id=1 group: calls bounds [0,35], fully below the thresholds below.
    for i in 0..262_144i64 {
        rows.push(Rec { id: 1, ts: 1000 + i, calls: (i % 8) * 5 });
    }
    // id=2 group: calls in [500,599]; exact targets pick some rows.
    let mut seed = 0x243f_6a88_85a3_08d3u64;
    for i in 0..1000i64 {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let calls = 500 + ((seed as i64 >> 11).abs() % 100);
        rows.push(Rec { id: 2, ts: 9000 + i, calls });
    }
    let bytes = Rec::serialize(rows.clone()).unwrap();

    // Range above the id=1 bounds: that group's column is ruled out from the
    // prefix alone; rows come only from id=2's bounds.
    let got = Rec::filter_bytes(&bytes, serde_json::json!({"calls": {"start": 500, "end": i64::MAX}}), &[]).unwrap();
    let truth: Vec<&Rec> = rows.iter().filter(|r| r.calls >= 500).collect();
    assert_eq!(got.len(), truth.len());
    for (g, t) in got.iter().zip(truth.iter()) {
        assert_eq!(*g, **t);
    }

    // Exact equality where some rows match.
    let query = serde_json::json!({"calls": {"start": 550, "end": 550}});
    let got = Rec::filter_bytes(&bytes, query, &[]).unwrap();
    let truth: Vec<&Rec> = rows.iter().filter(|r| r.calls == 550).collect();
    assert_eq!(got.len(), truth.len());

    // Disjoint equality: prefix skips both chunks without decompression.
    let query = serde_json::json!({"calls": {"start": 400, "end": 400}});
    let got = Rec::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(got.len(), 0);

    // Inclusion straddling both bounds.
    let got = Rec::filter_bytes(&bytes, serde_json::json!({"calls": [0, 550]}), &[]).unwrap();
    let truth: Vec<&Rec> = rows.iter().filter(|r| r.calls == 0 || r.calls == 550).collect();
    assert_eq!(got.len(), truth.len());
}

#[test]
fn empty_inclusion_still_skips() {
    // Resolution builds an empty inclusion to mean "matches nothing" (an
    // unparseable datetime array, for example), and the bounds answer must
    // agree: no stored value can be in the empty set.
    let blob = write_col(vec![7i64; 32]);
    assert!(i64::metadata_excludes(&blob, &Filter::InclusionI64(Default::default())));
    assert!(i64::metadata_excludes(&blob, &Filter::InclusionF64(vec![])));
    assert_eq!(filter_ones::<i64>(blob, &Filter::InclusionI64(Default::default())), 0);
}

#[test]
fn negative_float_range_on_unsigned_column() {
    // filter_bulk truncates -2.5..=-0.5 to -2..=0, which matches a zero row;
    // the bounds mirror the truncation and must not skip.
    let blob = write_col(vec![0u8; 16]);
    assert!(!u8::metadata_excludes(&blob, &Filter::FloatRange(-2.5..=-0.5)));
    assert_eq!(filter_ones::<u8>(blob, &Filter::FloatRange(-2.5..=-0.5)), 16);
    let blob = write_col(vec![3u8; 16]);
    assert!(u8::metadata_excludes(&blob, &Filter::FloatRange(-2.5..=-0.5)));
}

#[test]
fn option_column_skips_and_hits() {
    // Even rows hold Some(10..=17), odd rows hold None.
    let vals: Vec<Option<i64>> = (0..16).map(|i| if i % 2 == 0 { Some(10 + (i / 2) as i64) } else { None }).collect();
    let blob = write_col(vals);
    assert!(Option::<i64>::metadata_excludes(&blob, &Filter::I64(999)));
    assert!(!Option::<i64>::metadata_excludes(&blob, &Filter::I64(10)));
    assert_eq!(filter_ones::<Option<i64>>(blob.clone(), &Filter::Range(20..=30)), 0);
    // The same filter through the read path keeps exactly the matching Some
    // rows; None rows never match.
    assert_eq!(filter_ones::<Option<i64>>(blob, &Filter::Range(10..=12)), 3);
}

#[test]
fn all_none_option_column_defers() {
    // The hoisted header carries the empty-fold sentinel, so nothing may
    // skip; the read path clears every row itself (None never matches).
    let blob = write_col(vec![None::<i64>; 16]);
    assert!(!Option::<i64>::metadata_excludes(&blob, &Filter::I64(999)));
    assert_eq!(filter_ones::<Option<i64>>(blob, &Filter::I64(0)), 0);
}

#[test]
fn vec_column_skips_and_hits() {
    let vals: Vec<Vec<i64>> = (0..16).map(|i| vec![i as i64, i as i64 + 100]).collect();
    let blob = write_col(vals);
    assert!(Vec::<i64>::metadata_excludes(&blob, &Filter::Range(200..=300)));
    assert!(!Vec::<i64>::metadata_excludes(&blob, &Filter::Range(50..=60)));
    assert_eq!(filter_ones::<Vec<i64>>(blob.clone(), &Filter::Range(200..=300)), 0);
    assert_eq!(filter_ones::<Vec<i64>>(blob, &Filter::Range(100..=102)), 3);
}

#[test]
fn all_empty_vec_column_defers() {
    let blob = write_col(vec![Vec::<i64>::new(); 16]);
    assert!(!Vec::<i64>::metadata_excludes(&blob, &Filter::I64(999)));
    assert_eq!(filter_ones::<Vec<i64>>(blob, &Filter::I64(1)), 0);
}

#[test]
fn nested_option_vec_skips() {
    let vals: Vec<Option<Vec<i64>>> = (0..16).map(|i| if i % 4 == 0 { None } else { Some(vec![i as i64]) }).collect();
    let blob = write_col(vals);
    assert!(Option::<Vec<i64>>::metadata_excludes(&blob, &Filter::Range(100..=200)));
    assert!(!Option::<Vec<i64>>::metadata_excludes(&blob, &Filter::Range(2..=3)));
    assert_eq!(filter_ones::<Option<Vec<i64>>>(blob, &Filter::Range(2..=3)), 2);
}

#[test]
fn option_wrappers_never_skip_without_inner_bounds() {
    // A text inner gets the synthetic header (unknown kind, inexact): the
    // delegation cannot answer, but the read path must still work.
    let vals: Vec<Option<String>> = (0..16).map(|i| if i % 3 == 0 { None } else { Some(format!("v{i}")) }).collect();
    let blob = write_col(vals);
    assert!(!Option::<String>::metadata_excludes(&blob, &Filter::String("zzz".into())));
    assert_eq!(filter_ones::<Option<String>>(blob, &Filter::String("v1".into())), 1);
}

#[test]
fn rounded_float_option_never_skips() {
    // Body framing (float_round magic) is displaced by the presence stream,
    // so rounded wrappers stay on the read path.
    let vals: Vec<Option<f32>> = (0..16).map(|i| Some(i as f32)).collect();
    let blob = Option::<f32>::write(vals, 2, chrono::Duration::zero()).unwrap();
    assert!(!Option::<f32>::metadata_excludes(&blob, &Filter::F64(500.0)));
    assert_eq!(filter_ones::<Option<f32>>(blob, &Filter::F64(500.0)), 0);
}

#[test]
fn option_datetime_skips_via_i64_domain() {
    let base = 1_700_000_000_000_000i64;
    let vals: Vec<Option<chrono::DateTime<chrono::Utc>>> =
        (0..16).map(|i| chrono::DateTime::from_timestamp_micros(base + i * 1_000_000)).collect();
    let blob = write_col(vals);
    assert!(Option::<chrono::DateTime<chrono::Utc>>::metadata_excludes(&blob, &Filter::I64(0)));
    assert!(!Option::<chrono::DateTime<chrono::Utc>>::metadata_excludes(&blob, &Filter::I64(base)));
    assert_eq!(filter_ones::<Option<chrono::DateTime<chrono::Utc>>>(blob, &Filter::I64(base)), 1);
}
