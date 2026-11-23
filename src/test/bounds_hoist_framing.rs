//! Wrapper hoisting must consult the inner writer, not the inner bytes.
//!
//! Option and Vec hoist the inner column's 20-byte bounds header to the blob
//! front. The hoist must fire only when the inner writer emitted a header:
//! headerless inner bodies (lazy msgpack, struct frames) start with a row or
//! record count whose low byte can equal BOUNDS_MAGIC (0xFD) for exactly one
//! row count in 256, and sniffing that byte slices 20 bytes off the body.
use crate as pco_pack;
use crate::{PcoPack, PcoSerde};

#[derive(Clone, Debug, PartialEq, Default, PcoPack)]
struct OptTextRow {
    id: i64,
    tag: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Default, PcoPack)]
struct OptJsonRow {
    id: i64,
    doc: Option<serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq, Default, PcoPack)]
struct VecTextRow {
    id: i64,
    tags: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Default, PcoPack)]
struct FrameInner {
    id: i64,
    label: String,
}

#[derive(Clone, Debug, PartialEq, Default, PcoPack)]
struct OptFrameRow {
    id: i64,
    inner: Option<FrameInner>,
}

fn roundtrip<T>(rows: Vec<T>)
where
    T: PcoPack + PartialEq + std::fmt::Debug,
{
    let chunks = <T as PcoPack>::write(rows.clone()).unwrap();
    let bytes = T::to_bytes(&chunks).unwrap();
    let got = T::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(got, rows);
    let back = <T as PcoPack>::read(T::from_bytes(&bytes).unwrap()).unwrap();
    assert_eq!(back, rows);
}

#[test]
fn option_string_with_fd_low_byte_roundtrips() {
    // 253 present values: the inner lazy body starts with 0xFD (253 LE).
    let rows: Vec<OptTextRow> =
        (0..254).map(|i| OptTextRow { id: i, tag: if i < 253 { Some(format!("t{i}")) } else { None } }).collect();
    roundtrip(rows);
}

#[test]
fn option_string_with_fd_low_byte_wraps_next_page() {
    // 509 present: same low byte, past the 256 wrap.
    let rows: Vec<OptTextRow> =
        (0..510).map(|i| OptTextRow { id: i, tag: if i < 509 { Some(format!("t{i}")) } else { None } }).collect();
    roundtrip(rows);
}

#[test]
fn option_json_with_fd_low_byte_roundtrips() {
    let rows: Vec<OptJsonRow> = (0..254)
        .map(|i| OptJsonRow { id: i, doc: if i < 253 { Some(serde_json::json!({"k": i})) } else { None } })
        .collect();
    roundtrip(rows);
}

#[test]
fn vec_string_fd_low_byte_roundtrips() {
    // Total elements (not rows) ride the length prefix the wrapper sniffs.
    let mut rows = Vec::new();
    for r in 0..127 {
        rows.push(VecTextRow { id: r, tags: (0..2).map(|e| format!("r{r}e{e}")).collect() });
    }
    rows.push(VecTextRow { id: 127, tags: vec!["x".to_string()] });
    assert_eq!(rows.iter().map(|r| r.tags.len()).sum::<usize>(), 255);
    // Element count 253: the last one-element row keeps the total at 253.
    let mut rows2 = rows.clone();
    rows2.pop();
    rows2.push(VecTextRow { id: 126, tags: vec![] });
    assert_eq!(rows2.iter().map(|r| r.tags.len()).sum::<usize>(), 254);
    for row_set in [rows.clone(), rows2] {
        roundtrip(row_set);
    }
    // 253 elements: 126 pairs plus a one-element row.
    let mut rows3 = Vec::new();
    for r in 0..126 {
        rows3.push(VecTextRow { id: r, tags: (0..2).map(|e| format!("r{r}e{e}")).collect() });
    }
    rows3.push(VecTextRow { id: 126, tags: vec!["x".to_string()] });
    assert_eq!(rows3.iter().map(|r| r.tags.len()).sum::<usize>(), 253);
    roundtrip(rows3);
}

#[test]
fn option_struct_frame_roundtrips() {
    // Struct frames start with the record length prefix: any low byte is
    // legal data, so a frame with 253 present rows must survive.
    let rows: Vec<OptFrameRow> = (0..254)
        .map(|i| OptFrameRow {
            id: i,
            inner: if i < 253 { Some(FrameInner { id: i, label: format!("l{i}") }) } else { None },
        })
        .collect();
    roundtrip(rows.clone());
    // The frame body genuinely starts with 0xFD; the column still filters.
    let got = {
        let bytes = OptFrameRow::to_bytes(&<OptFrameRow as PcoPack>::write(rows).unwrap()).unwrap();
        OptFrameRow::filter_bytes(&bytes, serde_json::json!({"inner.label": "l100"}), &[]).unwrap()
    };
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].id, 100);
}

#[test]
fn numeric_option_still_hoists() {
    // The hoist stays real for inner writers that emit a header: bounds skip
    // on Option<i64> must keep excluding disjoint chunks without a decode.
    #[derive(Clone, Debug, PartialEq, Default, PcoPack)]
    struct OptNumRow {
        id: i64,
        n: Option<i64>,
    }
    let rows: Vec<OptNumRow> = (0..64).map(|i| OptNumRow { id: i, n: Some(i + 1000) }).collect();
    let mut chunks = <OptNumRow as PcoPack>::write(rows.clone()).unwrap();
    // Clip each column's tail: a bounds-excluded predicate must answer from
    // the hoisted header alone, never reaching the wounded pco stream.
    for c in &mut chunks {
        let mut v = c.n.to_vec();
        v.truncate(v.len() - 12);
        c.n = pco_pack::serde_bytes::ByteBuf::from(v);
    }
    let got = OptNumRow::filter(&chunks, serde_json::json!({"n": 1}), &[]).unwrap();
    assert!(got.is_empty());
    roundtrip(rows);
}

// Const-enforced classification: a wrong writer classification fails the
// build, not just a test run.
const _: () = {
    assert!(<i64 as PcoSerde>::HAS_BOUNDS_HEADER);
    assert!(<f64 as PcoSerde>::HAS_BOUNDS_HEADER);
    assert!(<Option<i64> as PcoSerde>::HAS_BOUNDS_HEADER);
    assert!(<Vec<i64> as PcoSerde>::HAS_BOUNDS_HEADER);
    assert!(!<String as PcoSerde>::HAS_BOUNDS_HEADER);
    assert!(!<serde_json::Value as PcoSerde>::HAS_BOUNDS_HEADER);
    assert!(<Option<serde_json::Value> as PcoSerde>::HAS_BOUNDS_HEADER);
    assert!(<Vec<String> as PcoSerde>::HAS_BOUNDS_HEADER);
    assert!(<(i64, String) as PcoSerde>::HAS_BOUNDS_HEADER);
    assert!(!<(String, i64) as PcoSerde>::HAS_BOUNDS_HEADER);
    assert!(!<FrameInner as PcoSerde>::HAS_BOUNDS_HEADER);
};
