use crate as pco_pack;
use crate::PcoPack;
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(chunk_size = 3)]
struct ChunkBoundaryRow {
    id: i64,
    value: i32,
    label: String,
}

#[test]
fn filter_i32_across_chunk_boundary() {
    let data = vec![
        ChunkBoundaryRow { id: 1, value: 10, label: "a".into() }, // chunk 0
        ChunkBoundaryRow { id: 2, value: 20, label: "b".into() }, // chunk 0
        ChunkBoundaryRow { id: 3, value: 10, label: "c".into() }, // chunk 0 (boundary)
        ChunkBoundaryRow { id: 4, value: 30, label: "d".into() }, // chunk 1
        ChunkBoundaryRow { id: 5, value: 10, label: "e".into() }, // chunk 1 (match spans boundary)
        ChunkBoundaryRow { id: 6, value: 40, label: "f".into() }, // chunk 1
    ];

    let bytes = ChunkBoundaryRow::serialize(data).unwrap();
    let result = ChunkBoundaryRow::filter_bytes(&bytes, serde_json::json!({"value": 10}), &[]).unwrap();

    assert_eq!(result.len(), 3);
    assert_eq!(result[0].id, 1);
    assert_eq!(result[1].id, 3);
    assert_eq!(result[2].id, 5);
}

#[test]
fn filter_string_across_chunk_boundary() {
    let data = vec![
        ChunkBoundaryRow { id: 1, value: 1, label: "keep".into() },
        ChunkBoundaryRow { id: 2, value: 2, label: "drop".into() },
        ChunkBoundaryRow { id: 3, value: 3, label: "keep".into() },
        ChunkBoundaryRow { id: 4, value: 4, label: "drop".into() },
        ChunkBoundaryRow { id: 5, value: 5, label: "keep".into() },
        ChunkBoundaryRow { id: 6, value: 6, label: "drop".into() },
    ];

    let bytes = ChunkBoundaryRow::serialize(data).unwrap();
    let result = ChunkBoundaryRow::filter_bytes(&bytes, serde_json::json!({"label": "keep"}), &[]).unwrap();

    assert_eq!(result.len(), 3);
    assert_eq!(result[0].id, 1);
    assert_eq!(result[1].id, 3);
    assert_eq!(result[2].id, 5);
}

#[test]
fn filter_i64_across_chunk_boundary() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    #[pco_pack(chunk_size = 3)]
    struct RowI64 {
        id: i64,
        count: i64,
    }

    let data = vec![
        RowI64 { id: 1, count: 100 },
        RowI64 { id: 2, count: 200 },
        RowI64 { id: 3, count: 100 },
        RowI64 { id: 4, count: 300 },
        RowI64 { id: 5, count: 100 },
    ];

    let bytes = RowI64::serialize(data).unwrap();
    let result = RowI64::filter_bytes(&bytes, serde_json::json!({"count": 100}), &[]).unwrap();

    assert_eq!(result.len(), 3);
    assert_eq!(result[0].id, 1);
    assert_eq!(result[1].id, 3);
    assert_eq!(result[2].id, 5);
}

#[test]
fn filter_f64_across_chunk_boundary() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    #[pco_pack(chunk_size = 3)]
    struct RowF64 {
        id: i64,
        score: f64,
    }

    let data = vec![
        RowF64 { id: 1, score: 0.5 },
        RowF64 { id: 2, score: 0.7 },
        RowF64 { id: 3, score: 0.5 },
        RowF64 { id: 4, score: 0.9 },
        RowF64 { id: 5, score: 0.5 },
    ];

    let bytes = RowF64::serialize(data).unwrap();
    let result = RowF64::filter_bytes(&bytes, serde_json::json!({"score": 0.5}), &[]).unwrap();

    assert_eq!(result.len(), 3);
    assert_eq!(result[0].id, 1);
    assert_eq!(result[1].id, 3);
    assert_eq!(result[2].id, 5);
}

#[test]
fn filter_bool_across_chunk_boundary() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    #[pco_pack(chunk_size = 3)]
    struct RowBool {
        id: i64,
        active: bool,
    }

    let data = vec![
        RowBool { id: 1, active: true },
        RowBool { id: 2, active: false },
        RowBool { id: 3, active: true },
        RowBool { id: 4, active: false },
        RowBool { id: 5, active: true },
    ];

    let bytes = RowBool::serialize(data).unwrap();
    let result = RowBool::filter_bytes(&bytes, serde_json::json!({"active": true}), &[]).unwrap();

    assert_eq!(result.len(), 3);
    assert_eq!(result[0].id, 1);
    assert_eq!(result[1].id, 3);
    assert_eq!(result[2].id, 5);
}

#[test]
fn range_filter_across_chunk_boundary() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    #[pco_pack(chunk_size = 3)]
    struct RowRange {
        id: i64,
        value: i64,
    }

    let data = vec![
        RowRange { id: 1, value: 10 },
        RowRange { id: 2, value: 50 },
        RowRange { id: 3, value: 90 },
        RowRange { id: 4, value: 130 },
        RowRange { id: 5, value: 170 },
    ];

    let bytes = RowRange::serialize(data).unwrap();
    let result = RowRange::filter_bytes(&bytes, serde_json::json!({"value": {"start": 40, "end": 140}}), &[]).unwrap();

    assert_eq!(result.len(), 3);
    assert_eq!(result[0].id, 2);
    assert_eq!(result[1].id, 3);
    assert_eq!(result[2].id, 4);
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(index = [group_id], chunk_size = 2)]
struct IndexedRow {
    group_id: i64,
    id: i64,
    value: i32,
    label: String,
}

#[test]
fn filter_across_index_groups() {
    let data = vec![
        // group 0 (chunk 1)
        IndexedRow { group_id: 0, id: 1, value: 42, label: "a".into() },
        IndexedRow { group_id: 0, id: 2, value: 99, label: "b".into() },
        // group 1 (chunk 2)
        IndexedRow { group_id: 1, id: 3, value: 42, label: "c".into() },
        IndexedRow { group_id: 1, id: 4, value: 77, label: "d".into() },
        // group 2 (chunk 3)
        IndexedRow { group_id: 2, id: 5, value: 42, label: "e".into() },
    ];

    let bytes = IndexedRow::serialize(data).unwrap();
    let mut result = IndexedRow::filter_bytes(&bytes, serde_json::json!({"value": 42}), &[]).unwrap();
    result.sort_by_key(|r| r.id);

    assert_eq!(result.len(), 3);
    assert_eq!(result[0].group_id, 0);
    assert_eq!(result[1].group_id, 1);
    assert_eq!(result[2].group_id, 2);
}

#[test]
fn filter_string_across_index_groups() {
    let data = vec![
        IndexedRow { group_id: 0, id: 1, value: 42, label: "target".into() },
        IndexedRow { group_id: 0, id: 2, value: 99, label: "other".into() },
        IndexedRow { group_id: 1, id: 3, value: 42, label: "target".into() },
        IndexedRow { group_id: 1, id: 4, value: 77, label: "other".into() },
    ];

    let bytes = IndexedRow::serialize(data).unwrap();
    let mut result =
        IndexedRow::filter_bytes(&bytes, serde_json::json!({"value": 42, "label": "target"}), &[]).unwrap();
    result.sort_by_key(|r| r.id);

    assert_eq!(result.len(), 2);
    assert_eq!(result[0].group_id, 0);
    assert_eq!(result[1].group_id, 1);
}

#[test]
fn range_filter_across_index_groups() {
    let data = vec![
        IndexedRow { group_id: 0, id: 1, value: 10, label: "a".into() },
        IndexedRow { group_id: 0, id: 2, value: 50, label: "b".into() },
        IndexedRow { group_id: 1, id: 3, value: 60, label: "c".into() },
        IndexedRow { group_id: 1, id: 4, value: 90, label: "d".into() },
    ];

    let bytes = IndexedRow::serialize(data).unwrap();
    let mut result =
        IndexedRow::filter_bytes(&bytes, serde_json::json!({"value": {"start": 40, "end": 70}}), &[]).unwrap();
    result.sort_by_key(|r| r.id);

    assert_eq!(result.len(), 2);
    assert_eq!(result[0].group_id, 0);
    assert_eq!(result[1].group_id, 1);
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(chunk_size = 2)]
struct RowWithVec {
    id: i64,
    values: Vec<i32>,
}

#[test]
fn filter_preserves_vec_contents_across_chunks() {
    let data = vec![
        RowWithVec { id: 1, values: vec![10, 20] },     // chunk 0
        RowWithVec { id: 2, values: vec![] },           // chunk 0 (empty)
        RowWithVec { id: 3, values: vec![30, 40, 50] }, // chunk 1 (longer)
        RowWithVec { id: 4, values: vec![60] },         // chunk 1 (shorter)
    ];

    let bytes = RowWithVec::serialize(data).unwrap();
    let result = RowWithVec::filter_bytes(&bytes, serde_json::json!({"id": 3}), &[]).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].values, vec![30, 40, 50]);
}

#[test]
fn filter_multiple_rows_preserves_vec_contents() {
    let data = vec![
        RowWithVec { id: 1, values: vec![1] },
        RowWithVec { id: 2, values: vec![2, 3] },
        RowWithVec { id: 3, values: vec![4, 5, 6] },
        RowWithVec { id: 4, values: vec![] },
    ];

    let bytes = RowWithVec::serialize(data).unwrap();
    let result = RowWithVec::filter_bytes(&bytes, serde_json::json!({"id": [1, 3]}), &[]).unwrap();
    assert_eq!(result.len(), 2);
    let ids: Vec<i64> = result.iter().map(|r| r.id).collect();
    assert!(ids.contains(&1));
    assert!(ids.contains(&3));

    for row in &result {
        match row.id {
            1 => assert_eq!(row.values, vec![1]),
            3 => assert_eq!(row.values, vec![4, 5, 6]),
            _ => panic!("unexpected id"),
        }
    }
}

#[test]
fn filter_preserves_vec_across_index_groups() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    #[pco_pack(index = [group_id], chunk_size = 2)]
    struct IndexedRowWithVec {
        group_id: i64,
        id: i64,
        values: Vec<i32>,
    }

    let data = vec![
        IndexedRowWithVec { group_id: 0, id: 1, values: vec![10] },
        IndexedRowWithVec { group_id: 0, id: 2, values: vec![20, 30] },
        IndexedRowWithVec { group_id: 1, id: 3, values: vec![] },
        IndexedRowWithVec { group_id: 1, id: 4, values: vec![40, 50, 60] },
    ];

    let bytes = IndexedRowWithVec::serialize(data).unwrap();
    let mut result = IndexedRowWithVec::filter_bytes(&bytes, serde_json::json!({"id": [2, 4]}), &[]).unwrap();
    result.sort_by_key(|r| r.id);

    assert_eq!(result.len(), 2);
    for row in &result {
        match row.id {
            2 => {
                assert_eq!(row.group_id, 0);
                assert_eq!(row.values, vec![20, 30]);
            }
            4 => {
                assert_eq!(row.group_id, 1);
                assert_eq!(row.values, vec![40, 50, 60]);
            }
            _ => panic!("unexpected id"),
        }
    }
}

#[test]
fn filter_vec_contains_across_chunks() {
    let data = vec![
        RowWithVec { id: 1, values: vec![10, 20] },
        RowWithVec { id: 2, values: vec![30, 40] },
        RowWithVec { id: 3, values: vec![10, 50] },
        RowWithVec { id: 4, values: vec![60] },
    ];

    let bytes = RowWithVec::serialize(data).unwrap();
    let result = RowWithVec::filter_bytes(&bytes, serde_json::json!({"values": 10}), &[]).unwrap();

    assert_eq!(result.len(), 2);
    assert_eq!(result[0].id, 1);
    assert_eq!(result[1].id, 3);
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(chunk_size = 2)]
struct RowWithBTreeMap {
    id: i64,
    tags: BTreeMap<String, i32>,
}

#[test]
fn filter_preserves_btreemap_contents_across_chunks() {
    let mut m1 = BTreeMap::new();
    m1.insert("a".into(), 1);
    m1.insert("b".into(), 2);

    let mut m3 = BTreeMap::new();
    m3.insert("x".into(), 10);
    m3.insert("y".into(), 20);
    m3.insert("z".into(), 30);

    let data = vec![
        RowWithBTreeMap { id: 1, tags: m1 },              // chunk 0
        RowWithBTreeMap { id: 2, tags: BTreeMap::new() }, // chunk 0 (empty)
        RowWithBTreeMap { id: 3, tags: m3 },              // chunk 1 (larger map)
    ];

    let bytes = RowWithBTreeMap::serialize(data).unwrap();
    let result = RowWithBTreeMap::filter_bytes(&bytes, serde_json::json!({"id": 3}), &[]).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].tags.get("x"), Some(&10));
    assert_eq!(result[0].tags.get("y"), Some(&20));
    assert_eq!(result[0].tags.get("z"), Some(&30));
}

#[test]
fn filter_multiple_rows_preserves_btreemap_contents() {
    let mut m1 = BTreeMap::new();
    m1.insert("key".into(), 1);

    let mut m3 = BTreeMap::new();
    m3.insert("alpha".into(), 100);
    m3.insert("beta".into(), 200);

    let data = vec![
        RowWithBTreeMap { id: 1, tags: m1 },
        RowWithBTreeMap { id: 2, tags: BTreeMap::new() },
        RowWithBTreeMap { id: 3, tags: m3 },
    ];

    let bytes = RowWithBTreeMap::serialize(data).unwrap();
    let result = RowWithBTreeMap::filter_bytes(&bytes, serde_json::json!({"id": [1, 3]}), &[]).unwrap();

    assert_eq!(result.len(), 2);
    for row in &result {
        match row.id {
            1 => {
                assert_eq!(row.tags.len(), 1);
                assert_eq!(row.tags.get("key"), Some(&1));
            }
            3 => {
                assert_eq!(row.tags.len(), 2);
                assert_eq!(row.tags.get("alpha"), Some(&100));
                assert_eq!(row.tags.get("beta"), Some(&200));
            }
            _ => panic!("unexpected id"),
        }
    }
}

#[test]
fn filter_btreemap_contains_key_across_chunks() {
    let mut m1 = BTreeMap::new();
    m1.insert("shared".into(), 1);

    let mut m3 = BTreeMap::new();
    m3.insert("shared".into(), 2);
    m3.insert("other".into(), 3);

    let data = vec![
        RowWithBTreeMap { id: 1, tags: m1 },
        RowWithBTreeMap { id: 2, tags: BTreeMap::new() },
        RowWithBTreeMap { id: 3, tags: m3 },
    ];

    let bytes = RowWithBTreeMap::serialize(data).unwrap();
    let result = RowWithBTreeMap::filter_bytes(&bytes, serde_json::json!({"tags": "shared"}), &[]).unwrap();

    assert_eq!(result.len(), 2);
    assert_eq!(result[0].id, 1);
    assert_eq!(result[1].id, 3);
}

#[test]
fn filter_preserves_hashmap_across_index_groups() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    #[pco_pack(index = [group_id], chunk_size = 2)]
    struct IndexedRowWithHashMap {
        group_id: i64,
        id: i64,
        tags: HashMap<String, i32>,
    }

    let mut m1 = HashMap::new();
    m1.insert("a".into(), 1);

    let mut m4 = HashMap::new();
    m4.insert("x".into(), 10);
    m4.insert("y".into(), 20);

    let data = vec![
        IndexedRowWithHashMap { group_id: 0, id: 1, tags: m1 },
        IndexedRowWithHashMap { group_id: 0, id: 2, tags: HashMap::new() },
        IndexedRowWithHashMap { group_id: 1, id: 3, tags: HashMap::new() },
        IndexedRowWithHashMap { group_id: 1, id: 4, tags: m4 },
    ];

    let bytes = IndexedRowWithHashMap::serialize(data).unwrap();
    let mut result = IndexedRowWithHashMap::filter_bytes(&bytes, serde_json::json!({"id": [1, 4]}), &[]).unwrap();
    result.sort_by_key(|r| r.id);

    assert_eq!(result.len(), 2);
    for row in &result {
        match row.id {
            1 => {
                assert_eq!(row.group_id, 0);
                assert_eq!(row.tags.get("a"), Some(&1));
            }
            4 => {
                assert_eq!(row.group_id, 1);
                assert_eq!(row.tags.get("x"), Some(&10));
                assert_eq!(row.tags.get("y"), Some(&20));
            }
            _ => panic!("unexpected id"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(chunk_size = 2)]
struct RowWithOption {
    id: i64,
    value: Option<i32>,
}

#[test]
fn filter_preserves_option_across_chunks() {
    let data = vec![
        RowWithOption { id: 1, value: Some(42) },
        RowWithOption { id: 2, value: None },
        RowWithOption { id: 3, value: Some(99) },
        RowWithOption { id: 4, value: None },
    ];

    let bytes = RowWithOption::serialize(data).unwrap();
    let result = RowWithOption::filter_bytes(&bytes, serde_json::json!({"id": [1, 3]}), &[]).unwrap();

    assert_eq!(result.len(), 2);
    for row in &result {
        match row.id {
            1 => assert_eq!(row.value, Some(42)),
            3 => assert_eq!(row.value, Some(99)),
            _ => panic!("unexpected id"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(chunk_size = 2)]
struct OuterRow {
    id: i64,
    inner: InnerStruct,
}

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct InnerStruct {
    x: i32,
    y: String,
}

#[test]
fn filter_nested_field_across_chunks() {
    let data = vec![
        OuterRow { id: 1, inner: InnerStruct { x: 10, y: "a".into() } },
        OuterRow { id: 2, inner: InnerStruct { x: 20, y: "b".into() } },
        OuterRow { id: 3, inner: InnerStruct { x: 10, y: "c".into() } },
        OuterRow { id: 4, inner: InnerStruct { x: 30, y: "d".into() } },
    ];

    let bytes = OuterRow::serialize(data).unwrap();
    let result = OuterRow::filter_bytes(&bytes, serde_json::json!({"inner.x": 10}), &[]).unwrap();

    assert_eq!(result.len(), 2);
    assert_eq!(result[0].id, 1);
    assert_eq!(result[1].id, 3);
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(chunk_size = 2)]
struct RowWithVecString {
    id: i64,
    names: Vec<String>,
}

#[test]
fn filter_preserves_vec_string_across_chunks() {
    let data = vec![
        RowWithVecString { id: 1, names: vec!["alice".into(), "bob".into()] },
        RowWithVecString { id: 2, names: vec![] },
        RowWithVecString { id: 3, names: vec!["charlie".into(), "dave".into(), "eve".into()] },
    ];

    let bytes = RowWithVecString::serialize(data).unwrap();
    let result = RowWithVecString::filter_bytes(&bytes, serde_json::json!({"id": 3}), &[]).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].names, vec!["charlie", "dave", "eve"]);
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(chunk_size = 2)]
struct RowWithNestedVec {
    id: i64,
    matrix: Vec<Vec<i32>>,
}

#[test]
fn filter_preserves_nested_vec_across_chunks() {
    let data = vec![
        RowWithNestedVec { id: 1, matrix: vec![vec![1, 2], vec![3]] },
        RowWithNestedVec { id: 2, matrix: vec![] },
        RowWithNestedVec { id: 3, matrix: vec![vec![4, 5, 6], vec![7, 8], vec![9]] },
    ];

    let bytes = RowWithNestedVec::serialize(data).unwrap();
    let result = RowWithNestedVec::filter_bytes(&bytes, serde_json::json!({"id": 3}), &[]).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].matrix, vec![vec![4, 5, 6], vec![7, 8], vec![9]]);
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(chunk_size = 2)]
struct RowWithComplexMap {
    id: i64,
    groups: BTreeMap<String, Vec<i32>>,
}

#[test]
fn filter_preserves_complex_map_across_chunks() {
    let mut m1 = BTreeMap::new();
    m1.insert("a".into(), vec![1, 2]);

    let mut m3 = BTreeMap::new();
    m3.insert("x".into(), vec![10, 20, 30]);
    m3.insert("y".into(), vec![]);
    m3.insert("z".into(), vec![40]);

    let data = vec![
        RowWithComplexMap { id: 1, groups: m1 },
        RowWithComplexMap { id: 2, groups: BTreeMap::new() },
        RowWithComplexMap { id: 3, groups: m3 },
    ];

    let bytes = RowWithComplexMap::serialize(data).unwrap();
    let result = RowWithComplexMap::filter_bytes(&bytes, serde_json::json!({"id": 3}), &[]).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].groups.get("x"), Some(&vec![10, 20, 30]));
    assert_eq!(result[0].groups.get("y"), Some(&Vec::<i32>::new()));
    assert_eq!(result[0].groups.get("z"), Some(&vec![40]));
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(index = [group_id], chunk_size = 2)]
struct MultiFieldRow {
    group_id: i64,
    id: i64,
    value: i32,
    label: String,
}

#[test]
fn multi_field_filter_across_index_groups() {
    let data = vec![
        MultiFieldRow { group_id: 0, id: 1, value: 42, label: "keep".into() },
        MultiFieldRow { group_id: 0, id: 2, value: 99, label: "keep".into() },
        MultiFieldRow { group_id: 1, id: 3, value: 42, label: "drop".into() },
        MultiFieldRow { group_id: 1, id: 4, value: 42, label: "keep".into() },
    ];

    let bytes = MultiFieldRow::serialize(data).unwrap();
    let query = serde_json::json!({"value": 42, "label": "keep"});
    let mut result = MultiFieldRow::filter_bytes(&bytes, query, &[]).unwrap();
    result.sort_by_key(|r| r.id);

    assert_eq!(result.len(), 2);
    for row in &result {
        match row.id {
            1 => assert_eq!(row.group_id, 0),
            4 => assert_eq!(row.group_id, 1),
            _ => panic!("unexpected id"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(chunk_size = 2)]
struct ManyChunksRow {
    id: i64,
    value: i32,
}

#[test]
fn filter_across_many_chunks() {
    let data: Vec<ManyChunksRow> =
        (0..10).map(|i| ManyChunksRow { id: i, value: if i % 3 == 0 { 99 } else { 0 } }).collect();

    let bytes = ManyChunksRow::serialize(data).unwrap();
    let result = ManyChunksRow::filter_bytes(&bytes, serde_json::json!({"value": 99}), &[]).unwrap();
    assert_eq!(result.len(), 4);
    let ids: Vec<i64> = result.iter().map(|r| r.id).collect();
    assert_eq!(ids, vec![0, 3, 6, 9]);
}

#[test]
fn filter_across_many_chunks_and_groups() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    #[pco_pack(index = [group_id], chunk_size = 2)]
    struct ManyChunksIndexedRow {
        group_id: i64,
        id: i64,
        value: i32,
    }

    let data: Vec<ManyChunksIndexedRow> = (0..15)
        .map(|i| ManyChunksIndexedRow { group_id: i / 5, id: i, value: if i % 2 == 0 { 42 } else { 0 } })
        .collect();

    let bytes = ManyChunksIndexedRow::serialize(data).unwrap();
    let mut result = ManyChunksIndexedRow::filter_bytes(&bytes, serde_json::json!({"value": 42}), &[]).unwrap();
    result.sort_by_key(|r| r.id);

    assert_eq!(result.len(), 8);
    let ids: Vec<i64> = result.iter().map(|r| r.id).collect();
    assert_eq!(ids, vec![0, 2, 4, 6, 8, 10, 12, 14]);
}
