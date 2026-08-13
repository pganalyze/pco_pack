use crate as pco_pack;
use crate::PcoPack;
use serde::{Deserialize, Serialize};

#[derive(PcoPack, Serialize, Deserialize, Debug, PartialEq, Clone, Default)]
struct Child {
    id: i32,
    name: String,
}

#[derive(PcoPack, Serialize, Deserialize, Debug, PartialEq, Clone)]
struct Parent {
    label: String,
    children: Vec<Child>,
}

#[derive(PcoPack, Serialize, Deserialize, Debug, PartialEq, Clone)]
struct Grandparent {
    name: String,
    parents: Vec<Parent>,
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
struct VecChild {
    values: Vec<Child>,
}
#[derive(Debug, Clone, PartialEq, PcoPack)]
struct VecParent {
    values: Vec<Parent>,
}
#[derive(Debug, Clone, PartialEq, PcoPack)]
struct VecGrandparent {
    values: Vec<Grandparent>,
}
#[derive(Debug, Clone, PartialEq, PcoPack)]
struct VecOptionChild {
    values: Vec<Option<Child>>,
}
#[derive(Debug, Clone, PartialEq, PcoPack)]
struct VecString {
    values: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, PcoPack)]
struct VecOptionString {
    values: Vec<Option<String>>,
}

#[test]
fn vec_custom_struct_roundtrip() {
    let data: Vec<Vec<Child>> = vec![
        vec![Child { id: 1, name: "alice".to_string() }, Child { id: 2, name: "bob".to_string() }],
        vec![],
        vec![Child { id: 3, name: "charlie".to_string() }],
    ];
    let wrapped: Vec<VecChild> = data.into_iter().map(|v| VecChild { values: v }).collect();
    let bytes = VecChild::serialize(wrapped.clone()).unwrap();
    let result: Vec<VecChild> = VecChild::deserialize(&bytes).unwrap();
    assert_eq!(wrapped, result);
}

#[test]
fn vec_custom_struct_nested() {
    let data: Vec<Vec<Parent>> = vec![
        vec![
            Parent {
                label: "family-a".to_string(),
                children: vec![Child { id: 1, name: "alice".to_string() }, Child { id: 2, name: "bob".to_string() }],
            },
            Parent { label: "family-b".to_string(), children: vec![Child { id: 3, name: "charlie".to_string() }] },
        ],
        vec![],
        vec![Parent { label: "family-c".to_string(), children: vec![] }],
    ];
    let wrapped: Vec<VecParent> = data.into_iter().map(|v| VecParent { values: v }).collect();
    let bytes = VecParent::serialize(wrapped.clone()).unwrap();
    let result: Vec<VecParent> = VecParent::deserialize(&bytes).unwrap();
    assert_eq!(wrapped, result);
}

#[test]
fn vec_custom_struct_deeply_nested() {
    let data: Vec<Vec<Grandparent>> = vec![
        vec![Grandparent {
            name: "root".to_string(),
            parents: vec![
                Parent { label: "p1".to_string(), children: vec![Child { id: 1, name: "c1".to_string() }] },
                Parent {
                    label: "p2".to_string(),
                    children: vec![Child { id: 2, name: "c2".to_string() }, Child { id: 3, name: "c3".to_string() }],
                },
            ],
        }],
        vec![],
        vec![Grandparent { name: "empty".to_string(), parents: vec![] }],
    ];
    let wrapped: Vec<VecGrandparent> = data.into_iter().map(|v| VecGrandparent { values: v }).collect();
    let bytes = VecGrandparent::serialize(wrapped.clone()).unwrap();
    let result: Vec<VecGrandparent> = VecGrandparent::deserialize(&bytes).unwrap();
    assert_eq!(wrapped, result);
}

#[test]
fn filtered_reader_vec_child_by_id() {
    let data: Vec<Vec<Child>> = vec![
        vec![Child { id: 1, name: "alice".to_string() }, Child { id: 2, name: "bob".to_string() }],
        vec![Child { id: 3, name: "charlie".to_string() }],
        vec![Child { id: 4, name: "dave".to_string() }],
    ];
    let wrapped: Vec<VecChild> = data.into_iter().map(|v| VecChild { values: v }).collect();
    let bytes = VecChild::serialize(wrapped.clone()).unwrap();

    let query = serde_json::json!({"values.id": 3});
    let result: Vec<VecChild> = VecChild::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result, vec![VecChild { values: vec![Child { id: 3, name: "charlie".to_string() }] }]);
}

#[test]
fn filtered_reader_vec_child_by_name() {
    let data: Vec<Vec<Child>> = vec![
        vec![Child { id: 1, name: "alice".to_string() }, Child { id: 2, name: "bob".to_string() }],
        vec![Child { id: 3, name: "charlie".to_string() }],
        vec![Child { id: 4, name: "alice".to_string() }],
    ];
    let wrapped: Vec<VecChild> = data.into_iter().map(|v| VecChild { values: v }).collect();
    let bytes = VecChild::serialize(wrapped.clone()).unwrap();

    let query = serde_json::json!({"values.name": "alice"});
    let result: Vec<VecChild> = VecChild::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 2);
}

#[test]
fn vec_string_roundtrip() {
    let data: Vec<Vec<String>> = vec![
        vec!["hello".to_string(), "world".to_string()],
        vec![],
        vec!["single".to_string()],
        vec!["a".to_string(), "b".to_string(), "c".to_string(), "d".to_string()],
    ];
    let wrapped: Vec<VecString> = data.into_iter().map(|v| VecString { values: v }).collect();
    let bytes = VecString::serialize(wrapped.clone()).unwrap();
    let result: Vec<VecString> = VecString::deserialize(&bytes).unwrap();
    assert_eq!(wrapped, result);
}

#[test]
fn vec_string_empty_column() {
    let wrapped: Vec<VecString> = vec![];
    let bytes = VecString::serialize(wrapped.clone()).unwrap();
    let result = VecString::deserialize(&bytes).unwrap();
    assert!(result.is_empty());
}

#[test]
fn filtered_reader_vec_string_no_match() {
    let data: Vec<Vec<String>> =
        vec![vec!["apple".to_string()], vec!["banana".to_string()], vec!["cherry".to_string()]];
    let wrapped: Vec<VecString> = data.into_iter().map(|v| VecString { values: v }).collect();
    let bytes = VecString::serialize(wrapped.clone()).unwrap();

    let query = serde_json::json!({"values": "nonexistent"});
    let result: Vec<VecString> = VecString::filter_bytes(&bytes, query, &[]).unwrap();
    assert!(result.is_empty());
}

#[test]
fn filtered_reader_vec_string() {
    let data: Vec<Vec<String>> = vec![
        vec!["apple".to_string(), "banana".to_string()],
        vec!["cherry".to_string()],
        vec!["date".to_string(), "elderberry".to_string()],
    ];
    let wrapped: Vec<VecString> = data.into_iter().map(|v| VecString { values: v }).collect();
    let bytes = VecString::serialize(wrapped.clone()).unwrap();

    let query = serde_json::json!({"values": "cherry"});
    let result: Vec<VecString> = VecString::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert!(result[0].values.contains(&"cherry".to_string()));
}

#[test]
fn filtered_reader_vec_option_string() {
    let data: Vec<Vec<Option<String>>> = vec![
        vec![Some("apple".to_string()), None, Some("banana".to_string())],
        vec![None, None],
        vec![Some("cherry".to_string())],
    ];
    let wrapped: Vec<VecOptionString> = data.into_iter().map(|v| VecOptionString { values: v }).collect();
    let bytes = VecOptionString::serialize(wrapped.clone()).unwrap();

    let query = serde_json::json!({"values": "cherry"});
    let result: Vec<VecOptionString> = VecOptionString::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 1);
}

#[test]
fn filtered_reader_vec_child() {
    let data: Vec<Vec<Child>> = vec![
        vec![Child { id: 1, name: "alice".to_string() }, Child { id: 2, name: "bob".to_string() }],
        vec![Child { id: 3, name: "charlie".to_string() }],
        vec![Child { id: 4, name: "dave".to_string() }, Child { id: 5, name: "eve".to_string() }],
    ];
    let wrapped: Vec<VecChild> = data.into_iter().map(|v| VecChild { values: v }).collect();
    let bytes = VecChild::serialize(wrapped.clone()).unwrap();

    let query = serde_json::json!({"values.id": 3});
    let result: Vec<VecChild> = VecChild::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result, vec![VecChild { values: vec![Child { id: 3, name: "charlie".to_string() }] }]);
}

#[test]
fn vec_child_validate_bounds() {
    let data: Vec<Vec<Child>> = vec![
        vec![Child { id: 1, name: "a".to_string() }],
        vec![Child { id: 2, name: "b".to_string() }, Child { id: 3, name: "c".to_string() }],
        vec![],
    ];
    let wrapped: Vec<VecChild> = data.into_iter().map(|v| VecChild { values: v }).collect();
    let bytes = VecChild::serialize(wrapped.clone()).unwrap();
    let result = VecChild::deserialize(&bytes).unwrap();
    assert_eq!(result.len(), 3);
}

#[test]
fn vec_parent_roundtrip() {
    let data: Vec<Vec<Parent>> = vec![
        vec![
            Parent { label: "family-a".to_string(), children: vec![Child { id: 1, name: "alice".to_string() }] },
            Parent {
                label: "family-b".to_string(),
                children: vec![Child { id: 2, name: "bob".to_string() }, Child { id: 3, name: "charlie".to_string() }],
            },
        ],
        vec![],
        vec![Parent { label: "family-c".to_string(), children: vec![] }],
    ];
    let wrapped: Vec<VecParent> = data.into_iter().map(|v| VecParent { values: v }).collect();
    let bytes = VecParent::serialize(wrapped.clone()).unwrap();
    let result: Vec<VecParent> = VecParent::deserialize(&bytes).unwrap();
    assert_eq!(wrapped, result);
}

#[test]
fn vec_child_empty_column() {
    let wrapped: Vec<VecChild> = vec![];
    let bytes = VecChild::serialize(wrapped.clone()).unwrap();
    let result = VecChild::deserialize(&bytes).unwrap();
    assert!(result.is_empty());
}

#[test]
fn filtered_reader_vec_child_no_match() {
    let data: Vec<Vec<Child>> = vec![
        vec![Child { id: 1, name: "alice".to_string() }],
        vec![Child { id: 2, name: "bob".to_string() }],
        vec![Child { id: 3, name: "charlie".to_string() }],
    ];
    let wrapped: Vec<VecChild> = data.into_iter().map(|v| VecChild { values: v }).collect();
    let bytes = VecChild::serialize(wrapped.clone()).unwrap();

    let query = serde_json::json!({"values.id": 99});
    let result: Vec<VecChild> = VecChild::filter_bytes(&bytes, query, &[]).unwrap();
    assert!(result.is_empty());
}

#[test]
fn filtered_reader_vec_child_multi_field() {
    let data: Vec<Vec<Child>> = vec![
        vec![Child { id: 1, name: "alice".to_string() }, Child { id: 2, name: "bob".to_string() }],
        vec![Child { id: 3, name: "charlie".to_string() }],
        vec![Child { id: 2, name: "bob".to_string() }, Child { id: 4, name: "dave".to_string() }],
    ];
    let wrapped: Vec<VecChild> = data.into_iter().map(|v| VecChild { values: v }).collect();
    let bytes = VecChild::serialize(wrapped.clone()).unwrap();

    let query = serde_json::json!({"values.id": 2, "values.name": "bob"});
    let result: Vec<VecChild> = VecChild::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(
        result,
        vec![
            VecChild {
                values: vec![Child { id: 1, name: "alice".to_string() }, Child { id: 2, name: "bob".to_string() }]
            },
            VecChild {
                values: vec![Child { id: 2, name: "bob".to_string() }, Child { id: 4, name: "dave".to_string() }]
            },
        ]
    );
}

#[test]
fn filtered_reader_vec_child_multi_field_no_match() {
    let data: Vec<Vec<Child>> =
        vec![vec![Child { id: 1, name: "alice".to_string() }], vec![Child { id: 2, name: "bob".to_string() }]];
    let wrapped: Vec<VecChild> = data.into_iter().map(|v| VecChild { values: v }).collect();
    let bytes = VecChild::serialize(wrapped.clone()).unwrap();

    let query = serde_json::json!({"values.id": 1, "values.name": "bob"});
    let result: Vec<VecChild> = VecChild::filter_bytes(&bytes, query, &[]).unwrap();
    assert!(result.is_empty());
}

#[test]
fn vec_option_child_roundtrip() {
    let data: Vec<Vec<Option<Child>>> = vec![
        vec![Some(Child { id: 1, name: "alice".to_string() }), None, Some(Child { id: 2, name: "bob".to_string() })],
        vec![],
        vec![None, None],
        vec![Some(Child { id: 3, name: "charlie".to_string() })],
    ];
    let wrapped: Vec<VecOptionChild> = data.into_iter().map(|v| VecOptionChild { values: v }).collect();
    let bytes = VecOptionChild::serialize(wrapped.clone()).unwrap();
    let result: Vec<VecOptionChild> = VecOptionChild::deserialize(&bytes).unwrap();
    assert_eq!(wrapped, result);
}

#[test]
fn filtered_reader_vec_option_child() {
    let data: Vec<Vec<Option<Child>>> = vec![
        vec![Some(Child { id: 1, name: "alice".to_string() }), None],
        vec![None, None],
        vec![Some(Child { id: 2, name: "bob".to_string() })],
    ];
    let wrapped: Vec<VecOptionChild> = data.into_iter().map(|v| VecOptionChild { values: v }).collect();
    let bytes = VecOptionChild::serialize(wrapped.clone()).unwrap();

    let query = serde_json::json!({"values.id": 2});
    let result: Vec<VecOptionChild> = VecOptionChild::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result, vec![VecOptionChild { values: vec![Some(Child { id: 2, name: "bob".to_string() })] }]);
}
