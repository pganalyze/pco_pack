use crate as pco_pack;
use crate::PcoPack;
use smol_str::SmolStr;

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct SmolStrRecord {
    val: SmolStr,
}

#[test]
fn smol_str_filter_exact() {
    let data: Vec<SmolStrRecord> = vec![
        SmolStrRecord { val: SmolStr::new("apple") },
        SmolStrRecord { val: SmolStr::new("banana") },
        SmolStrRecord { val: SmolStr::new("apple") },
        SmolStrRecord { val: SmolStr::new("cherry") },
    ];
    let bytes = SmolStrRecord::serialize(data.clone()).unwrap();

    let query = serde_json::json!({"val": "apple"});
    let result = SmolStrRecord::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].val, data[0].val);
    assert_eq!(result[1].val, data[2].val);
}

#[test]
fn smol_str_filter_inclusion() {
    let data: Vec<SmolStrRecord> = vec![
        SmolStrRecord { val: SmolStr::new("apple") },
        SmolStrRecord { val: SmolStr::new("banana") },
        SmolStrRecord { val: SmolStr::new("cherry") },
        SmolStrRecord { val: SmolStr::new("date") },
    ];
    let bytes = SmolStrRecord::serialize(data.clone()).unwrap();

    let query = serde_json::json!({"val": ["banana", "date"]});
    let result = SmolStrRecord::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].val, data[1].val);
    assert_eq!(result[1].val, data[3].val);
}

#[test]
fn smol_str_filter_no_match() {
    let data: Vec<SmolStrRecord> =
        vec![SmolStrRecord { val: SmolStr::new("apple") }, SmolStrRecord { val: SmolStr::new("banana") }];
    let bytes = SmolStrRecord::serialize(data.clone()).unwrap();

    let query = serde_json::json!({"val": "grape"});
    let result = SmolStrRecord::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 0);
}
