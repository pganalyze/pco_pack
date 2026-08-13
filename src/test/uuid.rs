use crate as pco_pack;
use crate::PcoPack;

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct UuidRecord {
    id: i64,
    thing_id: uuid::Uuid,
}

fn roundtrip(data: Vec<UuidRecord>) -> Vec<UuidRecord> {
    let bytes = UuidRecord::serialize(data).unwrap();
    UuidRecord::deserialize(&bytes).unwrap()
}

#[test]
fn uuid_roundtrip() {
    let uuid = uuid::Uuid::parse_str("67e55044-10b1-426f-9247-bb680e5fe0c8").unwrap();
    let data = vec![UuidRecord { id: 1, thing_id: uuid }];
    assert_eq!(roundtrip(data.clone()), data);
}

#[test]
fn uuid_nil_roundtrip() {
    let uuid = uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000000").unwrap();
    let data = vec![UuidRecord { id: 1, thing_id: uuid }];
    assert_eq!(roundtrip(data.clone()), data);
}

#[test]
fn uuid_max_roundtrip() {
    let uuid = uuid::Uuid::parse_str("ffffffff-ffff-ffff-ffff-ffffffffffff").unwrap();
    let data = vec![UuidRecord { id: 1, thing_id: uuid }];
    assert_eq!(roundtrip(data.clone()), data);
}

#[test]
fn uuid_filter_string_match() {
    let target = uuid::Uuid::parse_str("67e55044-10b1-426f-9247-bb680e5fe0c8").unwrap();
    let data = vec![
        UuidRecord { id: 1, thing_id: uuid::Uuid::nil() },
        UuidRecord { id: 2, thing_id: target },
        UuidRecord { id: 3, thing_id: uuid::Uuid::max() },
    ];
    let bytes = UuidRecord::serialize(data).unwrap();
    let query = serde_json::json!({"thing_id": "67e55044-10b1-426f-9247-bb680e5fe0c8"});
    let result = UuidRecord::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].thing_id, target);
}

#[test]
fn uuid_filter_string_mismatch() {
    let data = vec![
        UuidRecord { id: 1, thing_id: uuid::Uuid::parse_str("67e55044-10b1-426f-9247-bb680e5fe0c8").unwrap() },
        UuidRecord { id: 2, thing_id: uuid::Uuid::max() },
    ];
    let bytes = UuidRecord::serialize(data).unwrap();
    let query = serde_json::json!({"thing_id": "00000000-0000-0000-0000-000000000000"});
    let result = UuidRecord::filter_bytes(&bytes, query, &[]).unwrap();
    assert!(result.is_empty());
}

#[test]
fn uuid_filter_inclusion_match() {
    let target = uuid::Uuid::parse_str("67e55044-10b1-426f-9247-bb680e5fe0c8").unwrap();
    let data = vec![
        UuidRecord { id: 1, thing_id: uuid::Uuid::nil() },
        UuidRecord { id: 2, thing_id: target },
        UuidRecord { id: 3, thing_id: uuid::Uuid::max() },
    ];
    let bytes = UuidRecord::serialize(data).unwrap();
    let query = serde_json::json!({
        "thing_id": ["00000000-0000-0000-0000-000000000000", "67e55044-10b1-426f-9247-bb680e5fe0c8"]
    });
    let result = UuidRecord::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 2);
}

#[test]
fn uuid_filter_inclusion_not_found() {
    let data = vec![
        UuidRecord { id: 1, thing_id: uuid::Uuid::parse_str("67e55044-10b1-426f-9247-bb680e5fe0c8").unwrap() },
        UuidRecord { id: 2, thing_id: uuid::Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap() },
    ];
    let bytes = UuidRecord::serialize(data).unwrap();
    let query = serde_json::json!({
        "thing_id": ["00000000-0000-0000-0000-000000000000", "ffffffff-ffff-ffff-ffff-ffffffffffff"]
    });
    let result = UuidRecord::filter_bytes(&bytes, query, &[]).unwrap();
    assert!(result.is_empty());
}

#[test]
fn uuid_filter_invalid_uuid() {
    let data = vec![UuidRecord { id: 1, thing_id: uuid::Uuid::nil() }];
    let bytes = UuidRecord::serialize(data).unwrap();
    let query = serde_json::json!({"thing_id": "not-a-uuid"});
    let result = UuidRecord::filter_bytes(&bytes, query, &[]);
    assert!(result.is_err());
}

#[test]
fn uuid_filter_nested_field_error() {
    let data = vec![UuidRecord { id: 1, thing_id: uuid::Uuid::nil() }];
    let bytes = UuidRecord::serialize(data).unwrap();
    let query = serde_json::json!({"thing_id.nested": "67e55044-10b1-426f-9247-bb680e5fe0c8"});
    let result = UuidRecord::filter_bytes(&bytes, query, &[]);
    assert!(result.is_err());
}
