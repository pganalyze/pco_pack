use crate as pco_pack;
use crate::PcoPack;
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct BTreeMapI32U64 {
    field: BTreeMap<i32, u64>,
}
#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct BTreeMapU32I32 {
    field: BTreeMap<u32, i32>,
}
#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct HashMapI32U64 {
    field: HashMap<i32, u64>,
}
#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct HashMapU32I32 {
    field: HashMap<u32, i32>,
}

#[test]
fn btremap_i32_u64_roundtrip() {
    let data: Vec<BTreeMapI32U64> = vec![
        BTreeMapI32U64 {
            field: {
                let mut m = BTreeMap::new();
                m.insert(1, 100);
                m.insert(2, 200);
                m
            },
        },
        BTreeMapI32U64 {
            field: {
                let mut m = BTreeMap::new();
                m.insert(10, 1000);
                m
            },
        },
        BTreeMapI32U64 { field: BTreeMap::new() },
        BTreeMapI32U64 {
            field: {
                let mut m = BTreeMap::new();
                m.insert(-1, 999);
                m.insert(0, 0);
                m.insert(1, 1);
                m
            },
        },
    ];
    let bytes = BTreeMapI32U64::serialize(data.clone()).unwrap();
    let result = BTreeMapI32U64::deserialize(&bytes).unwrap();
    assert_eq!(data, result);
}

#[test]
fn btremap_u32_i32_roundtrip() {
    let data: Vec<BTreeMapU32I32> = vec![
        BTreeMapU32I32 {
            field: {
                let mut m = BTreeMap::new();
                m.insert(1, -1);
                m.insert(2, 42);
                m
            },
        },
        BTreeMapU32I32 { field: BTreeMap::new() },
        BTreeMapU32I32 {
            field: {
                let mut m = BTreeMap::new();
                m.insert(0, i32::MAX);
                m
            },
        },
    ];
    let bytes = BTreeMapU32I32::serialize(data.clone()).unwrap();
    let result = BTreeMapU32I32::deserialize(&bytes).unwrap();
    assert_eq!(data, result);
}

#[test]
fn hashmap_i32_u64_roundtrip() {
    let data: Vec<HashMapI32U64> = vec![
        HashMapI32U64 {
            field: {
                let mut m = HashMap::new();
                m.insert(1, 100);
                m.insert(2, 200);
                m
            },
        },
        HashMapI32U64 {
            field: {
                let mut m = HashMap::new();
                m.insert(10, 1000);
                m
            },
        },
        HashMapI32U64 { field: HashMap::new() },
    ];
    let bytes = HashMapI32U64::serialize(data.clone()).unwrap();
    let result = HashMapI32U64::deserialize(&bytes).unwrap();
    assert_eq!(data.len(), result.len());
    for (orig, de) in data.iter().zip(result.iter()) {
        assert_eq!(orig.field.len(), de.field.len());
        for (k, v) in &orig.field {
            assert_eq!(de.field.get(k), Some(v));
        }
    }
}

#[test]
fn hashmap_u32_i32_roundtrip() {
    let data: Vec<HashMapU32I32> = vec![
        HashMapU32I32 {
            field: {
                let mut m = HashMap::new();
                m.insert(1, -1);
                m.insert(2, 42);
                m
            },
        },
        HashMapU32I32 { field: HashMap::new() },
    ];
    let bytes = HashMapU32I32::serialize(data.clone()).unwrap();
    let result = HashMapU32I32::deserialize(&bytes).unwrap();
    assert_eq!(data.len(), result.len());
    for (orig, de) in data.iter().zip(result.iter()) {
        assert_eq!(orig.field.len(), de.field.len());
        for (k, v) in &orig.field {
            assert_eq!(de.field.get(k), Some(v));
        }
    }
}

#[test]
fn empty_btremap_column() {
    let data: Vec<BTreeMapI32U64> = vec![];
    let bytes = BTreeMapI32U64::serialize(data).unwrap();
    let result = BTreeMapI32U64::deserialize(&bytes).unwrap();
    assert!(result.is_empty());
}

#[test]
fn empty_hashmap_column() {
    let data: Vec<HashMapI32U64> = vec![];
    let bytes = HashMapI32U64::serialize(data).unwrap();
    let result = HashMapI32U64::deserialize(&bytes).unwrap();
    assert!(result.is_empty());
}

#[test]
fn btreemap_to_hashmap_roundtrip() {
    let original: Vec<BTreeMapI32U64> = vec![
        BTreeMapI32U64 {
            field: {
                let mut m = BTreeMap::new();
                m.insert(1, 100);
                m.insert(2, 200);
                m
            },
        },
        BTreeMapI32U64 {
            field: {
                let mut m = BTreeMap::new();
                m.insert(10, 1000);
                m
            },
        },
        BTreeMapI32U64 { field: BTreeMap::new() },
    ];
    let compressed_bt = BTreeMapI32U64::write(original.clone()).unwrap();
    let buf = BTreeMapI32U64::to_bytes(&compressed_bt).unwrap();

    let result = HashMapI32U64::deserialize(&buf).unwrap();
    assert_eq!(result.len(), original.len());
    for (orig, de) in original.iter().zip(result.iter()) {
        assert_eq!(orig.field.len(), de.field.len());
        for (k, v) in &orig.field {
            assert_eq!(de.field.get(k), Some(v));
        }
    }
}

#[test]
fn hashmap_to_btreemap_roundtrip() {
    let original: Vec<HashMapI32U64> = vec![
        HashMapI32U64 {
            field: {
                let mut m = HashMap::new();
                m.insert(1, 100);
                m.insert(2, 200);
                m
            },
        },
        HashMapI32U64 { field: HashMap::new() },
        HashMapI32U64 {
            field: {
                let mut m = HashMap::new();
                m.insert(10, 1000);
                m
            },
        },
    ];
    let compressed_hm = HashMapI32U64::write(original.clone()).unwrap();
    let buf = HashMapI32U64::to_bytes(&compressed_hm).unwrap();

    let result = BTreeMapI32U64::deserialize(&buf).unwrap();
    assert_eq!(result.len(), original.len());
    for (orig, de) in original.iter().zip(result.iter()) {
        assert_eq!(orig.field.len(), de.field.len());
        for (k, v) in &orig.field {
            assert_eq!(de.field.get(k), Some(v));
        }
    }
}

#[test]
fn hashmap_filter_key_match() {
    let data: Vec<HashMapI32U64> = vec![
        HashMapI32U64 {
            field: {
                let mut m = HashMap::new();
                m.insert(1, 100);
                m.insert(2, 200);
                m
            },
        },
        HashMapI32U64 {
            field: {
                let mut m = HashMap::new();
                m.insert(10, 1000);
                m
            },
        },
        HashMapI32U64 { field: HashMap::new() },
        HashMapI32U64 {
            field: {
                let mut m = HashMap::new();
                m.insert(1, 50);
                m.insert(3, 300);
                m
            },
        },
    ];
    let bytes = HashMapI32U64::serialize(data).unwrap();
    let result = HashMapI32U64::filter_bytes(&bytes, serde_json::json!({"field": 1}), &[]).unwrap();
    assert_eq!(result.len(), 2);
    for row in &result {
        assert!(row.field.contains_key(&1));
    }
}

#[test]
fn hashmap_filter_key_no_match() {
    let data: Vec<HashMapI32U64> = vec![
        HashMapI32U64 {
            field: {
                let mut m = HashMap::new();
                m.insert(1, 100);
                m
            },
        },
        HashMapI32U64 {
            field: {
                let mut m = HashMap::new();
                m.insert(2, 200);
                m
            },
        },
    ];
    let bytes = HashMapI32U64::serialize(data).unwrap();
    let result = HashMapI32U64::filter_bytes(&bytes, serde_json::json!({"field": 99}), &[]).unwrap();
    assert!(result.is_empty());
}

#[test]
fn hashmap_filter_key_empty_map() {
    let data: Vec<HashMapI32U64> =
        vec![HashMapI32U64 { field: HashMap::new() }, HashMapI32U64 { field: HashMap::new() }];
    let bytes = HashMapI32U64::serialize(data).unwrap();
    let result = HashMapI32U64::filter_bytes(&bytes, serde_json::json!({"field": 1}), &[]).unwrap();
    assert!(result.is_empty());
}

#[test]
fn hashmap_filter_matches_btreemap_filter() {
    let bt_data: Vec<BTreeMapI32U64> = vec![
        BTreeMapI32U64 {
            field: {
                let mut m = BTreeMap::new();
                m.insert(1, 100);
                m.insert(2, 200);
                m
            },
        },
        BTreeMapI32U64 {
            field: {
                let mut m = BTreeMap::new();
                m.insert(10, 1000);
                m
            },
        },
        BTreeMapI32U64 { field: BTreeMap::new() },
        BTreeMapI32U64 {
            field: {
                let mut m = BTreeMap::new();
                m.insert(1, 50);
                m.insert(3, 300);
                m
            },
        },
    ];

    let hm_data: Vec<HashMapI32U64> =
        bt_data.iter().map(|r| HashMapI32U64 { field: r.field.iter().map(|(k, v)| (*k, *v)).collect() }).collect();

    let bt_bytes = BTreeMapI32U64::serialize(bt_data).unwrap();
    let hm_bytes = HashMapI32U64::serialize(hm_data).unwrap();

    let filter_json = serde_json::json!({"field": 1});

    let bt_result = BTreeMapI32U64::filter_bytes(&bt_bytes, filter_json.clone(), &[]).unwrap();
    let hm_result = HashMapI32U64::filter_bytes(&hm_bytes, filter_json, &[]).unwrap();

    assert_eq!(bt_result.len(), hm_result.len());
    for (bt_row, hm_row) in bt_result.iter().zip(hm_result.iter()) {
        for (k, v) in &bt_row.field {
            assert_eq!(hm_row.field.get(k), Some(v));
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct BTreeMapStringU64 {
    field: BTreeMap<String, u64>,
}

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct HashMapStringU64 {
    field: HashMap<String, u64>,
}

#[test]
fn btreemap_string_key_filter_match() {
    let data = vec![
        BTreeMapStringU64 {
            field: {
                let mut m = BTreeMap::new();
                m.insert("alpha".into(), 1);
                m.insert("beta".into(), 2);
                m
            },
        },
        BTreeMapStringU64 {
            field: {
                let mut m = BTreeMap::new();
                m.insert("gamma".into(), 3);
                m
            },
        },
        BTreeMapStringU64 {
            field: {
                let mut m = BTreeMap::new();
                m.insert("alpha".into(), 4);
                m.insert("delta".into(), 5);
                m
            },
        },
        BTreeMapStringU64 { field: BTreeMap::new() },
    ];

    let bytes = BTreeMapStringU64::serialize(data).unwrap();
    let result = BTreeMapStringU64::filter_bytes(&bytes, serde_json::json!({"field": "alpha"}), &[]).unwrap();
    assert_eq!(result.len(), 2);
    for row in &result {
        assert!(row.field.contains_key("alpha"));
    }
}

#[test]
fn btreemap_string_key_filter_no_match() {
    let data = vec![
        BTreeMapStringU64 {
            field: {
                let mut m = BTreeMap::new();
                m.insert("alpha".into(), 1);
                m
            },
        },
        BTreeMapStringU64 {
            field: {
                let mut m = BTreeMap::new();
                m.insert("beta".into(), 2);
                m
            },
        },
    ];

    let bytes = BTreeMapStringU64::serialize(data).unwrap();
    let result = BTreeMapStringU64::filter_bytes(&bytes, serde_json::json!({"field": "missing"}), &[]).unwrap();
    assert!(result.is_empty());
}

#[test]
fn hashmap_string_key_filter_match() {
    let data = vec![
        HashMapStringU64 {
            field: {
                let mut m = HashMap::new();
                m.insert("alpha".into(), 1);
                m.insert("beta".into(), 2);
                m
            },
        },
        HashMapStringU64 {
            field: {
                let mut m = HashMap::new();
                m.insert("gamma".into(), 3);
                m
            },
        },
        HashMapStringU64 {
            field: {
                let mut m = HashMap::new();
                m.insert("alpha".into(), 4);
                m.insert("delta".into(), 5);
                m
            },
        },
        HashMapStringU64 { field: HashMap::new() },
    ];

    let bytes = HashMapStringU64::serialize(data).unwrap();
    let result = HashMapStringU64::filter_bytes(&bytes, serde_json::json!({"field": "alpha"}), &[]).unwrap();
    assert_eq!(result.len(), 2);
    for row in &result {
        assert!(row.field.contains_key("alpha"));
    }
}

#[test]
fn hashmap_string_key_filter_no_match() {
    let data = vec![
        HashMapStringU64 {
            field: {
                let mut m = HashMap::new();
                m.insert("alpha".into(), 1);
                m
            },
        },
        HashMapStringU64 {
            field: {
                let mut m = HashMap::new();
                m.insert("beta".into(), 2);
                m
            },
        },
    ];

    let bytes = HashMapStringU64::serialize(data).unwrap();
    let result = HashMapStringU64::filter_bytes(&bytes, serde_json::json!({"field": "missing"}), &[]).unwrap();
    assert!(result.is_empty());
}
