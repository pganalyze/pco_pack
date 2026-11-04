use crate as pco_pack;
use crate::{Consumer, PcoPack, fold};
use pco_pack::anyhow::Result;
use pco_pack::serde_json;

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
#[pco_pack(index = [device_id])]
struct Wide {
    device_id: i64,
    fingerprint: u64,
    calls: u32,
    name: String,
    payload: serde_json::Value,
}

fn sample_chunks() -> Vec<<Wide as PcoPack>::Chunk> {
    let data = vec![
        Wide {
            device_id: 1,
            fingerprint: 10,
            calls: 2,
            name: "a".into(),
            payload: serde_json::json!({"big": "never read"}),
        },
        Wide {
            device_id: 2,
            fingerprint: 20,
            calls: 3,
            name: "b".into(),
            payload: serde_json::json!({"big": "never read"}),
        },
        Wide {
            device_id: 1,
            fingerprint: 30,
            calls: 5,
            name: "c".into(),
            payload: serde_json::json!({"big": "never read"}),
        },
    ];
    Wide::write(data).unwrap()
}

fn consumer_fields<R: PcoPack, C: Consumer<R>>(_: &C) -> &'static [&'static str] {
    C::FIELDS
}

fn check_nonzero(ratio: u64) -> Result<()> {
    if ratio == 0 { Err(pco_pack::anyhow::anyhow!("zero ratio")) } else { Ok(()) }
}

#[test]
fn for_each_hydrated_closure() {
    let chunks = sample_chunks();
    let mut sum = 0u64;
    // The annotation is required: parameter types are not inferred through for_each's
    // generic `Consumer` bound.
    Wide::for_each(&chunks, serde_json::json!({}), &[], |row: Wide| sum += row.calls as u64).unwrap();
    assert_eq!(sum, 10);
}

#[test]
fn for_each_hydrated_closure_returns_result() {
    let chunks = sample_chunks();
    let mut seen = Vec::new();
    let result = Wide::for_each(&chunks, serde_json::json!({}), &[], |row: Wide| -> Result<()> {
        seen.push(row.fingerprint);
        if row.fingerprint == 20 { Err(pco_pack::anyhow::anyhow!("stop")) } else { Ok(()) }
    });
    assert!(result.unwrap_err().to_string().contains("stop"));
    // device_id grouping leaves chunk order unspecified: either the device-2 chunk
    // aborts first or the device-1 chunk ran fully. Nothing after the failing row runs.
    assert!(seen == [20] || seen == [10, 30, 20]);
}

#[test]
fn fold_projects_by_parameter_names() {
    let chunks = sample_chunks();
    let mut pairs = Vec::new();
    let consumer = fold!(Wide, |fingerprint: u64, calls: u32| {
        pairs.push((fingerprint, calls));
    });
    assert_eq!(consumer_fields(&consumer), ["fingerprint", "calls"]);
    Wide::for_each(&chunks, serde_json::json!({}), &[], consumer).unwrap();
    // Rows regroup by index when written, so compare as a multiset.
    pairs.sort();
    assert_eq!(pairs, [(10, 2), (20, 3), (30, 5)]);
}

#[test]
fn fold_query_and_filter_merge() {
    let chunks = sample_chunks();
    // Query field and projection field are different; both must be available.
    // Both surviving rows live in the device_id=1 chunk, so chunk order is fixed.
    let mut names = Vec::new();
    Wide::for_each(
        &chunks,
        serde_json::json!({"device_id": 1}),
        &[],
        fold!(Wide, |name: String, fingerprint: u64| names.push((fingerprint, name))),
    )
    .unwrap();
    assert_eq!(names, [(10, "a".to_string()), (30, "c".to_string())]);
}

#[test]
fn fold_closure_can_return_result_with_question_mark() {
    let chunks = sample_chunks();
    let mut total = 0u64;
    Wide::for_each(
        &chunks,
        serde_json::json!({}),
        &[],
        fold!(Wide, |fingerprint: u64, calls: u32| -> Result<()> {
            let ratio = fingerprint / u64::from(calls.max(1));
            check_nonzero(ratio)?;
            total += ratio;
            Ok(())
        }),
    )
    .unwrap();
    assert_eq!(total, 5 + 6 + 6);
}

#[test]
fn fold_result_can_fail_mid_stream() {
    let chunks = sample_chunks();
    let result = Wide::for_each(
        &chunks,
        serde_json::json!({}),
        &[],
        fold!(Wide, |fingerprint: u64, calls: u32| -> Result<()> {
            check_nonzero(fingerprint / u64::from(calls.max(1)))?;
            if fingerprint == 20 { Err(pco_pack::anyhow::anyhow!("nope")) } else { Ok(()) }
        }),
    );
    assert!(result.unwrap_err().to_string().contains("nope"));
}

#[test]
fn fold_pushes_values_into_vec() {
    let chunks = sample_chunks();
    let mut sums = Vec::new();
    Wide::for_each(
        &chunks,
        serde_json::json!({}),
        &[],
        fold!(Wide, |fingerprint: u64, calls: u32| sums.push(fingerprint + calls as u64)),
    )
    .unwrap();
    sums.sort();
    assert_eq!(sums, [12, 23, 35]);

    let bytes = Wide::to_bytes(&chunks).unwrap();
    let mut names = Vec::new();
    Wide::for_each_bytes(&bytes, serde_json::json!({}), &[], fold!(Wide, |name: String| names.push(name))).unwrap();
    names.sort();
    assert_eq!(names, ["a", "b", "c"]);
}

#[test]
fn fold_includes_index_fields() {
    let chunks = sample_chunks();
    let mut rows = Vec::new();
    Wide::for_each(
        &chunks,
        serde_json::json!({}),
        &[],
        fold!(Wide, |device_id: i64, fingerprint: u64| rows.push((device_id, fingerprint))),
    )
    .unwrap();
    rows.sort();
    assert_eq!(rows, [(1, 10), (1, 30), (2, 20)]);
}

#[test]
fn explicit_fields_merge_with_consumer_fields() {
    let chunks = sample_chunks();
    // The consumer declares fingerprint and calls; the caller adds name, which
    // the consumer still never pops.
    let mut pairs = Vec::new();
    Wide::for_each(
        &chunks,
        serde_json::json!({}),
        &["name"],
        fold!(Wide, |fingerprint: u64, calls: u32| pairs.push((fingerprint, calls))),
    )
    .unwrap();
    pairs.sort();
    assert_eq!(pairs, [(10, 2), (20, 3), (30, 5)]);
}

/// Shape of a persistence wrapper: generic over the row type and the consumer,
/// using the consumer's declared fields to narrow its own column selection.
fn persistence_scan<R: PcoPack, C: Consumer<R>>(
    chunks: &[<R as PcoPack>::Chunk], query: &serde_json::Value, fields: &[&str], consumer: C,
) -> Result<(Vec<&'static str>, ())> {
    let select = R::resolve_fields(query, pco_pack::projection_fields::<R, C>())?;
    let streamed = R::for_each(chunks, query.clone(), fields, consumer);
    Ok((select, streamed?))
}

#[test]
fn persistence_wrapper_passes_consumer_through() {
    let chunks = sample_chunks();
    let (select, ()) = persistence_scan(
        &chunks,
        &serde_json::json!({"device_id": 1}),
        &[],
        fold!(Wide, |fingerprint: u64, calls: u32| {
            let _ = fingerprint;
            let _: u32 = calls;
        }),
    )
    .unwrap();
    // Query fields merge into the SELECT list alongside the declared projection.
    assert_eq!(select, ["device_id", "fingerprint", "calls"]);

    let mut sum = 0u64;
    persistence_scan(
        &chunks,
        &serde_json::json!({}),
        &[],
        fold!(Wide, |fingerprint: u64, calls: u32| sum += fingerprint + calls as u64),
    )
    .unwrap();
    assert_eq!(sum, 12 + 23 + 35);
}

#[test]
fn repeated_fold_expansions_share_a_scope() {
    let chunks = sample_chunks();
    // Same struct, same fields, same scope: each expansion is its own type.
    let mut x = Vec::new();
    let mut y = Vec::new();
    let a = fold!(Wide, |fingerprint: u64, calls: u32| x.push((fingerprint, calls)));
    let b = fold!(Wide, |fingerprint: u64, calls: u32| y.push(fingerprint - calls as u64));
    Wide::for_each(&chunks, serde_json::json!({}), &[], a).unwrap();
    Wide::for_each(&chunks, serde_json::json!({}), &[], b).unwrap();
    x.sort();
    y.sort();
    assert_eq!(x, [(10, 2), (20, 3), (30, 5)]);
    assert_eq!(y, [8, 17, 25]);
}

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
enum JobState {
    #[default]
    Idle,
    Running(i64),
    Done(u32),
}

#[test]
fn enum_for_each_streams() {
    let data = vec![JobState::Idle, JobState::Running(7), JobState::Done(9), JobState::Running(1)];
    let chunks = JobState::write(data).unwrap();
    let mut seen = Vec::new();
    JobState::for_each(&chunks, serde_json::json!({}), &[], |v: JobState| seen.push(v)).unwrap();
    assert_eq!(seen, JobState::read(chunks.clone()).unwrap());

    let mut counted = Vec::new();
    JobState::for_each(&chunks, serde_json::json!({}), &[], |v: JobState| {
        counted.push(match v {
            JobState::Running(_) => 1,
            _ => 0,
        })
    })
    .unwrap();
    assert_eq!(counted, [0, 1, 0, 1]);
}

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct Reading {
    sensor: u32,
    value: f64,
    blob: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
#[pco_pack(chunk_size = 16)]
enum Stream {
    #[default]
    Idle,
    Tick(Reading),
    Note(String),
}

/// 130 rows so the mask walk spans several 64-bit words, interleaving variants.
fn stream_data() -> Vec<Stream> {
    (0..130)
        .map(|i| match i % 3 {
            0 => Stream::Tick(Reading { sensor: i % 7, value: i as f64, blob: serde_json::json!({"never": "read"}) }),
            1 => Stream::Idle,
            _ => Stream::Note(format!("n{i}")),
        })
        .collect()
}

#[test]
fn enum_fold_variant_pattern() {
    let data = stream_data();
    let chunks = Stream::write(data.clone()).unwrap();
    let mut seen: Vec<(u32, f64)> = Vec::new();
    Stream::for_each(
        &chunks,
        serde_json::json!({}),
        &[],
        fold!(Stream, |Stream::Tick(Reading { sensor: u32, value: f64 })| seen.push((sensor, value))),
    )
    .unwrap();
    let truth: Vec<(u32, f64)> = data
        .iter()
        .filter_map(|s| match s {
            Stream::Tick(r) => Some((r.sensor, r.value)),
            _ => None,
        })
        .collect();
    // Exact order across mask words proves payload slots stay aligned with rows.
    assert_eq!(seen, truth);
}

#[test]
fn enum_fold_variant_pattern_merges_query() {
    let data = stream_data();
    let chunks = Stream::write(data.clone()).unwrap();
    // Query keeps only Note rows; the declared Tick variant intersects to nothing.
    let mut seen = Vec::new();
    Stream::for_each(
        &chunks,
        serde_json::json!({"Note": true}),
        &[],
        fold!(Stream, |Stream::Tick(Reading { value: f64 })| seen.push(value)),
    )
    .unwrap();
    assert!(seen.is_empty());

    let mut seen = Vec::new();
    Stream::for_each(
        &chunks,
        serde_json::json!({"Tick": true}),
        &[],
        fold!(Stream, |Stream::Tick(Reading { value: f64 })| seen.push(value)),
    )
    .unwrap();
    let truth: Vec<f64> = data
        .iter()
        .filter_map(|s| match s {
            Stream::Tick(r) => Some(r.value),
            _ => None,
        })
        .collect();
    assert_eq!(seen, truth);
}

#[test]
fn enum_pattern_folds_share_a_scope() {
    let chunks = Stream::write(stream_data()).unwrap();
    // Same variant, same fields: identical impl headers, so each expansion needs its own type.
    let mut sensors = Vec::new();
    let mut total = 0u32;
    let a = fold!(Stream, |Stream::Tick(Reading { sensor: u32 })| sensors.push(sensor));
    let b = fold!(Stream, |Stream::Tick(Reading { sensor: u32 })| total += sensor);
    Stream::for_each(&chunks, serde_json::json!({}), &[], a).unwrap();
    Stream::for_each(&chunks, serde_json::json!({}), &[], b).unwrap();
    let sum: u32 = sensors.iter().sum();
    assert_eq!(total, sum);
    assert_eq!(sensors.len(), 44);
}

/// A unit variant has no payload block to stream, so the fold engine narrows the mask to
/// its discriminant and calls a zero-argument closure per matching row. Both the bare and
/// the empty-parens pattern forms reach the same expansion.
#[test]
fn enum_fold_unit_variant() {
    let data = stream_data();
    let chunks = Stream::write(data.clone()).unwrap();
    let idle_truth = data.iter().filter(|s| matches!(s, Stream::Idle)).count();

    let mut idle = 0usize;
    Stream::for_each(&chunks, serde_json::json!({}), &[], fold!(Stream, |Stream::Idle| idle += 1)).unwrap();
    assert_eq!(idle, idle_truth);

    let mut idle_parens = 0usize;
    Stream::for_each(&chunks, serde_json::json!({}), &[], fold!(Stream, |Stream::Idle()| idle_parens += 1)).unwrap();
    assert_eq!(idle_parens, idle_truth);

    // A bare single-segment path reaches the same expansion.
    let mut idle_bare = 0usize;
    Stream::for_each(&chunks, serde_json::json!({}), &[], fold!(Stream, |Idle| idle_bare += 1)).unwrap();
    assert_eq!(idle_bare, idle_truth);

    // A pattern unit fold still intersects with a query that keeps other variants.
    let mut counted = 0usize;
    Stream::for_each(&chunks, serde_json::json!({"Note": true}), &[], fold!(Stream, |Stream::Idle| counted += 1))
        .unwrap();
    assert_eq!(counted, 0);

    let mut counted = 0usize;
    Stream::for_each(&chunks, serde_json::json!({"Idle": true}), &[], fold!(Stream, |Stream::Idle| counted += 1))
        .unwrap();
    assert_eq!(counted, idle_truth);
}

/// Payload variants fold without binding fields too, which counts their rows without
/// decompressing the payload block.
#[test]
fn enum_fold_variant_membership_without_payload() {
    let data = stream_data();
    let chunks = Stream::write(data.clone()).unwrap();
    let note_truth = data.iter().filter(|s| matches!(s, Stream::Note(_))).count();
    let mut notes = 0usize;
    Stream::for_each(&chunks, serde_json::json!({}), &[], fold!(Stream, |Stream::Note| notes += 1)).unwrap();
    assert_eq!(notes, note_truth);
}

/// A bare variant name parses exactly like its qualified spelling.
#[test]
fn enum_fold_bare_pattern() {
    let chunks = Stream::write(stream_data()).unwrap();

    let mut qualified = Vec::new();
    Stream::for_each(
        &chunks,
        serde_json::json!({}),
        &[],
        fold!(Stream, |Stream::Tick(Reading { sensor: u32, value: f64 })| qualified.push((sensor, value))),
    )
    .unwrap();
    let mut bare = Vec::new();
    Stream::for_each(
        &chunks,
        serde_json::json!({}),
        &[],
        fold!(Stream, |Tick(Reading { sensor: u32, value: f64 })| bare.push((sensor, value))),
    )
    .unwrap();
    assert_eq!(bare, qualified);
    assert_eq!(bare.len(), 44);
}

/// The fast path (empty query, slot streaming) and the mask path (query whose
/// equality step pins Tick, making the declared narrowing redundant) must agree with
/// each other and with a hydrated filter of the same query.
#[test]
fn enum_fold_fast_path_matches_mask_path() {
    let chunks = Stream::write(stream_data()).unwrap();

    let mut fast = Vec::new();
    Stream::for_each(
        &chunks,
        serde_json::json!({}),
        &[],
        fold!(Stream, |Tick(Reading { sensor: u32, value: f64 })| fast.push((sensor, value))),
    )
    .unwrap();
    let mut masked = Vec::new();
    Stream::for_each(
        &chunks,
        serde_json::json!({"Tick": true}),
        &[],
        fold!(Stream, |Tick(Reading { sensor: u32, value: f64 })| masked.push((sensor, value))),
    )
    .unwrap();
    assert_eq!(fast, masked);

    let hydrated: Vec<(u32, f64)> = Stream::filter(&chunks, serde_json::json!({"Tick": true}), &[])
        .unwrap()
        .iter()
        .map(|s| match s {
            Stream::Tick(r) => (r.sensor, r.value),
            other => panic!("unexpected {other:?}"),
        })
        .collect();
    assert_eq!(masked, hydrated);
}

/// One pass over each chunk delivers every row to whichever arm owns its variant:
/// a payload arm pops from its block, unit and membership arms just count.
#[test]
fn enum_fold_multi_variant() {
    let data = stream_data();
    let chunks = Stream::write(data.clone()).unwrap();

    let mut ticks: Vec<(u32, f64)> = Vec::new();
    let mut idle = 0usize;
    let mut notes = 0usize;
    let consumer = fold!(
        Stream,
        |Tick(Reading { sensor: u32, value: f64 })| ticks.push((sensor, value)),
        |Idle| idle += 1,
        |Note()| notes += 1,
    );
    assert_eq!(consumer_fields(&consumer), ["Tick", "Idle", "Note"]);
    Stream::for_each(&chunks, serde_json::json!({}), &[], consumer).unwrap();

    let ticks_truth: Vec<(u32, f64)> = data
        .iter()
        .filter_map(|s| match s {
            Stream::Tick(r) => Some((r.sensor, r.value)),
            _ => None,
        })
        .collect();
    assert_eq!(ticks, ticks_truth);
    assert_eq!(idle, data.iter().filter(|s| matches!(s, Stream::Idle)).count());
    assert_eq!(notes, data.iter().filter(|s| matches!(s, Stream::Note(_))).count());
}

/// A multi-arm fold intersects with the query like a single arm does: rows outside
/// the kept variants never dispatch, and the kept arms still share one pass.
#[test]
fn enum_fold_multi_variant_with_query() {
    let chunks = Stream::write(stream_data()).unwrap();
    let mut ticks = 0usize;
    let mut idle = 0usize;
    Stream::for_each(
        &chunks,
        serde_json::json!({"Tick": true}),
        &[],
        fold!(
            Stream,
            |Tick(Reading { value: f64 })| {
                let _ = value;
                ticks += 1;
            },
            |Idle| idle += 1
        ),
    )
    .unwrap();
    assert_eq!(ticks, 44);
    assert_eq!(idle, 0);
}

/// A corrupted column must fail the stream, not read back zeroed values. All chunks
/// are visited regardless of grouping order, so the error is deterministic.
#[test]
fn for_each_propagates_decode_errors() {
    let mut chunks = sample_chunks();
    // Corrupt one chunk's compressed `fingerprint` column.
    for byte in chunks[1].fingerprint.iter_mut() {
        *byte = !*byte;
    }
    let err = Wide::for_each(
        &chunks,
        serde_json::json!({}),
        &[],
        fold!(Wide, |fingerprint: u64| {
            let _ = fingerprint;
        }),
    )
    .unwrap_err();
    assert!(!err.to_string().is_empty());
}

#[test]
fn for_each_empty_chunks() {
    let bytes = Wide::serialize(vec![]).unwrap();
    let mut out = Vec::new();
    Wide::for_each_bytes(&bytes, serde_json::json!({}), &[], fold!(Wide, |fingerprint: u64| out.push(fingerprint)))
        .unwrap();
    assert!(out.is_empty());
}

/// 130 rows over `chunk_size = 16`: every pattern-fold slot cursor restarts at the
/// next chunk, so the walk must stay aligned across the chunk boundary.
#[test]
fn enum_folds_and_filters_span_chunks() {
    let data = stream_data();
    let chunks = Stream::write(data.clone()).unwrap();
    assert_eq!(chunks.len(), data.len().div_ceil(16));

    let mut seen: Vec<(u32, f64)> = Vec::new();
    Stream::for_each(
        &chunks,
        serde_json::json!({}),
        &[],
        fold!(Stream, |Stream::Tick(Reading { sensor: u32, value: f64 })| seen.push((sensor, value))),
    )
    .unwrap();
    let truth: Vec<(u32, f64)> = data
        .iter()
        .filter_map(|s| match s {
            Stream::Tick(r) => Some((r.sensor, r.value)),
            _ => None,
        })
        .collect();
    assert_eq!(seen, truth);

    let mut hydrated = Vec::new();
    Stream::for_each(&chunks, serde_json::json!({}), &[], |s: Stream| hydrated.push(s)).unwrap();
    assert_eq!(hydrated, Stream::read(chunks.clone()).unwrap());

    let filtered = Stream::filter(&chunks, serde_json::json!({"Tick": true}), &[]).unwrap();
    let truth: Vec<Stream> = data.iter().filter(|s| matches!(s, Stream::Tick(_))).cloned().collect();
    assert_eq!(filtered, truth);
}

#[test]
fn enum_for_each_empty_data() {
    let chunks = Stream::write(vec![]).unwrap();
    let mut seen = Vec::new();
    Stream::for_each(
        &chunks,
        serde_json::json!({}),
        &[],
        fold!(Stream, |Stream::Tick(Reading { value: f64 })| seen.push(value)),
    )
    .unwrap();
    assert!(seen.is_empty());
}

#[test]
fn fold_into_rejects_unresolved_fields() {
    let chunks = sample_chunks();
    let plan = <Wide as pco_pack::PcoFilter>::resolve_query(&serde_json::json!({})).unwrap();
    let mut consumer = fold!(Wide, |fingerprint: u64| {
        let _: u64 = fingerprint;
    });
    let err = chunks[0].fold_into(&plan, &["nope"], &mut consumer).unwrap_err().to_string();
    assert!(err.contains("Unknown field"), "{err}");
    let err = chunks[0].fold_into(&plan, &["payload.big"], &mut consumer).unwrap_err().to_string();
    assert!(err.contains("Nested field path"), "{err}");

    // A resolved projection validates and streams the whole chunk (empty plan path).
    let fields = Wide::resolve_fields(&serde_json::json!({}), &["fingerprint"]).unwrap();
    let mut total = 0u64;
    let mut consumer = fold!(Wide, |fingerprint: u64| total += fingerprint);
    chunks[0].fold_into(&plan, &fields, &mut consumer).unwrap();
    assert!(total > 0);
}

#[test]
fn lazy_reader_pop_error_and_default_semantics() {
    // Corrupted bytes fail loudly instead of reading back as the default value.
    let mut damaged = pco_pack::LazyReader::<u64>::new(vec![0x7f; 24], 0, chrono::Duration::zero());
    assert!(damaged.pop_inner(0).is_err());
    // A column absent from the loaded projection has no bytes and reads back as Default.
    let mut absent = pco_pack::LazyReader::<u64>::new(vec![], 0, chrono::Duration::zero());
    assert_eq!(absent.pop_inner(0).unwrap(), 0);
}
