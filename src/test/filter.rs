use crate as pco_pack;
use crate::{PcoFilter, PcoPack};
use std::collections::{BTreeMap, HashMap};

#[test]
fn resolve_fields_no_duplicates_on_repeated_calls() {
    let query = serde_json::json!({"id": 1, "name": "alice"});
    let fields = &["name", "score"];
    let result = SimpleRecord::resolve_fields(&query, fields).unwrap();
    assert_eq!(result, ["id", "name", "score"]);
    let result2 = SimpleRecord::resolve_fields(&query, fields).unwrap();
    assert_eq!(result, result2);
}

#[test]
fn resolve_fields_no_duplicates_when_query_and_fields_overlap_completely() {
    let query = serde_json::json!({"id": 1, "name": "alice"});
    let fields = &["id", "name"];
    let result = SimpleRecord::resolve_fields(&query, fields).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result, ["id", "name"]);
}

#[test]
fn resolve_fields_no_duplicates_with_nested_query_paths() {
    let query = serde_json::json!({"meta.name": "alice", "meta.score": 95.0});
    let fields = &["meta"];
    let result = NestedRecord::resolve_fields(&query, fields).unwrap();
    assert_eq!(result, ["meta"]);
}

#[test]
fn resolve_fields_empty_inputs() {
    let result = SimpleRecord::resolve_fields(&serde_json::json!({}), &[]).unwrap();
    assert_eq!(result, ["id", "name", "score"]);
    let query = serde_json::json!({"id": 1});
    let result = SimpleRecord::resolve_fields(&query, &[]).unwrap();
    assert_eq!(result, ["id", "name", "score"]);
}

#[test]
fn resolve_fields_rejects_unknown_query_field() {
    let query = serde_json::json!({"unknown": 42});
    let result = SimpleRecord::resolve_fields(&query, &[]);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Unknown field: unknown"));
}

#[test]
fn resolve_fields_rejects_nested_path_in_fields() {
    let query = serde_json::json!({});
    let result = NestedRecord::resolve_fields(&query, &["meta.name"]);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("Nested field path 'meta.name' is not supported; use 'meta' instead"));
}

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct SimpleRecord {
    id: i64,
    name: String,
    score: f32,
}

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct NestedRecord {
    id: i64,
    name: String,
    meta: SimpleRecord,
}

#[test]
fn schema_string_nested_rejected() {
    let result = String::resolve_filter("name.extra", &serde_json::json!("alice"));
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("string leaf"));
}

#[test]
fn schema_numeric_nested_rejected() {
    let result = i64::resolve_filter("id.extra", &serde_json::json!(42));
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("numeric leaf"));
}

fn make_simple_data() -> Vec<SimpleRecord> {
    vec![
        SimpleRecord { id: 1, name: "alice".into(), score: 95.5 },
        SimpleRecord { id: 2, name: "bob".into(), score: 80.0 },
        SimpleRecord { id: 3, name: "charlie".into(), score: 95.5 },
        SimpleRecord { id: 4, name: "diana".into(), score: 70.0 },
    ]
}

#[test]
fn filter_simple_i64() {
    let data = make_simple_data();
    let bytes = SimpleRecord::serialize(data.clone()).unwrap();
    let query = serde_json::json!({"id": 2});
    let result = SimpleRecord::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].id, 2);
    assert_eq!(result[0].name, "bob");
}

#[test]
fn filter_simple_string() {
    let data = make_simple_data();
    let bytes = SimpleRecord::serialize(data.clone()).unwrap();
    let query = serde_json::json!({"name": "alice"});
    let result = SimpleRecord::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].name, "alice");
}

#[test]
fn filter_simple_f32() {
    let data = make_simple_data();
    let bytes = SimpleRecord::serialize(data.clone()).unwrap();
    let query = serde_json::json!({"score": 95.5});
    let result = SimpleRecord::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].name, "alice");
    assert_eq!(result[1].name, "charlie");
}

#[test]
fn filter_simple_multiple() {
    let data = make_simple_data();
    let bytes = SimpleRecord::serialize(data.clone()).unwrap();
    let query = serde_json::json!({"name": "alice", "score": 95.5});
    let result = SimpleRecord::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].id, 1);
}

#[test]
fn filter_simple_no_match() {
    let data = make_simple_data();
    let bytes = SimpleRecord::serialize(data.clone()).unwrap();
    let query = serde_json::json!({"id": 999});
    let result = SimpleRecord::filter_bytes(&bytes, query, &[]).unwrap();
    assert!(result.is_empty());
}

#[test]
fn filter_simple_unknown_field() {
    let data = make_simple_data();
    let bytes = SimpleRecord::serialize(data.clone()).unwrap();
    let query = serde_json::json!({"nonexistent": 42});
    let result = SimpleRecord::filter_bytes(&bytes, query, &[]);
    match result {
        Err(err) => assert!(err.to_string().contains("nonexistent"), "error message: {}", err),
        Ok(_) => panic!("expected error for nonexistent field"),
    }
}

fn make_nested_data() -> Vec<NestedRecord> {
    vec![
        NestedRecord {
            id: 1,
            name: "person1".into(),
            meta: SimpleRecord { id: 100, name: "alice".into(), score: 95.5 },
        },
        NestedRecord { id: 2, name: "person2".into(), meta: SimpleRecord { id: 200, name: "bob".into(), score: 80.0 } },
        NestedRecord {
            id: 3,
            name: "person3".into(),
            meta: SimpleRecord { id: 300, name: "alice".into(), score: 70.0 },
        },
    ]
}

#[test]
fn filter_nested_top_level() {
    let data = make_nested_data();
    let bytes = NestedRecord::serialize(data.clone()).unwrap();
    let query = serde_json::json!({"id": 2});
    let result = NestedRecord::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].name, "person2");
}

#[test]
fn filter_nested_nested_field_works() {
    let data = make_nested_data();
    let bytes = NestedRecord::serialize(data.clone()).unwrap();
    let query = serde_json::json!({"meta.name": "alice"});
    let result = NestedRecord::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 2);
    assert!(result.iter().all(|r| r.meta.name == "alice"));
}

#[test]
fn filter_nested_deep_field_works() {
    let data = make_nested_data();
    let bytes = NestedRecord::serialize(data.clone()).unwrap();
    let query = serde_json::json!({"meta.score": 95.5});
    let result = NestedRecord::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].id, 1);
}

#[test]
fn schema_btremap_key_filter_nested_rejected() {
    let result = BTreeMap::<String, i32>::resolve_filter("x.extra", &serde_json::json!("target_key"));
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("key matching"));
}

#[test]
fn schema_hashmap_key_filter_nested_rejected() {
    let result = HashMap::<String, i32>::resolve_filter("x.extra", &serde_json::json!("target_key"));
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("key matching"));
}

#[test]
fn filter_tuple_field() {
    #[derive(Debug, Clone, PartialEq, Default, PcoPack)]
    struct TupleRecord {
        id: i64,
        pair: (i64, String),
    }

    let data = vec![
        TupleRecord { id: 1, pair: (42, "alice".into()) },
        TupleRecord { id: 2, pair: (99, "bob".into()) },
        TupleRecord { id: 3, pair: (42, "charlie".into()) },
    ];
    let bytes = TupleRecord::serialize(data).unwrap();

    let result = TupleRecord::filter_bytes(&bytes, serde_json::json!({"pair.0": 42}), &[]).unwrap();
    assert_eq!(result.len(), 2);
    assert!(result.iter().all(|r| r.pair.0 == 42));

    let result = TupleRecord::filter_bytes(&bytes, serde_json::json!({"pair.1": "bob"}), &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].pair.1, "bob");
}

#[test]
fn filter_empty_query_returns_all() {
    let data = make_simple_data();
    let bytes = SimpleRecord::serialize(data.clone()).unwrap();

    let query = serde_json::json!({});
    let result = SimpleRecord::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 4);
}

#[test]
fn resolve_query_simple() {
    let query = serde_json::json!({"id": 42, "name": "alice", "score": 95.5});
    let plans = SimpleRecord::resolve_query(&query).unwrap();
    assert_eq!(plans.len(), 3);
}

#[test]
fn resolve_query_invalid_field() {
    let query = serde_json::json!({"nonexistent": 42});
    let result = SimpleRecord::resolve_query(&query);
    assert!(result.is_err());
}

#[test]
fn filter_inclusion_i64() {
    let data = make_simple_data();
    let bytes = SimpleRecord::serialize(data.clone()).unwrap();
    let query = serde_json::json!({"id": [1, 3]});
    let result = SimpleRecord::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].id, 1);
    assert_eq!(result[1].id, 3);
}

#[test]
fn filter_inclusion_i64_no_match() {
    let data = make_simple_data();
    let bytes = SimpleRecord::serialize(data.clone()).unwrap();
    let query = serde_json::json!({"id": [99, 100]});
    let result = SimpleRecord::filter_bytes(&bytes, query, &[]).unwrap();
    assert!(result.is_empty());
}

#[test]
fn filter_inclusion_string() {
    let data = make_simple_data();
    let bytes = SimpleRecord::serialize(data.clone()).unwrap();
    let query = serde_json::json!({"name": ["alice", "charlie"]});
    let result = SimpleRecord::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 2);
}

#[test]
fn filter_inclusion_f64() {
    let data = make_simple_data();
    let bytes = SimpleRecord::serialize(data.clone()).unwrap();
    let query = serde_json::json!({"score": [95.5, 85.0]});
    let result = SimpleRecord::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 2);
}

#[test]
fn filter_range_i64() {
    let data = make_simple_data();
    let bytes = SimpleRecord::serialize(data.clone()).unwrap();
    let query = serde_json::json!({"id": {"start": 2, "end": 3}});
    let result = SimpleRecord::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].id, 2);
    assert_eq!(result[1].id, 3);
}

#[test]
fn filter_range_f64() {
    let data = make_simple_data();
    let bytes = SimpleRecord::serialize(data.clone()).unwrap();
    let query = serde_json::json!({"score": {"start": 80.0, "end": 95.5}});
    let result = SimpleRecord::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 3);
}

#[test]
fn filter_combined_inclusion_and_range() {
    let data = make_simple_data();
    let bytes = SimpleRecord::serialize(data.clone()).unwrap();
    let query = serde_json::json!({"id": [1, 2], "score": {"start": 80.0, "end": 100.0}});
    let result = SimpleRecord::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 2);
    assert!(result.iter().all(|r| r.id == 1 || r.id == 2));
}

#[test]
fn filter_nested_field_path_in_load_fields_errors() {
    let data = make_nested_data();
    let bytes = NestedRecord::serialize(data.clone()).unwrap();
    let result = NestedRecord::filter_bytes(&bytes, serde_json::json!({}), &["id", "meta.name"]);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("Nested field path 'meta.name' is not supported; use 'meta' instead"));
}

#[test]
fn filter_full_nested_field_in_load_fields_works() {
    let data = make_nested_data();
    let bytes = NestedRecord::serialize(data.clone()).unwrap();
    let result = NestedRecord::filter_bytes(&bytes, serde_json::json!({}), &["id", "meta"]).unwrap();
    assert_eq!(result.len(), 3);
}
