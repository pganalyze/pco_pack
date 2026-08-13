use crate as pco_pack;
use crate::PcoPack;

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(float_round = 2)]
struct TupleFloatRounded {
    id: i64,
    coords: (f64, i32, f64),
}

#[test]
fn float_round_on_tuple() {
    let data = vec![
        TupleFloatRounded { id: 1, coords: (3.14159, 100, 2.71828) },
        TupleFloatRounded { id: 2, coords: (1.41421, 200, 0.57721) },
        TupleFloatRounded { id: 3, coords: (9.99999, 300, 1.00001) },
    ];

    let bytes = TupleFloatRounded::serialize(data.clone()).unwrap();
    let result = TupleFloatRounded::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result.len(), 3);
    assert_eq!(result[0].coords.0, 3.14);
    assert_eq!(result[0].coords.1, 100);
    assert_eq!(result[0].coords.2, 2.72);
    assert_eq!(result[1].coords.0, 1.41);
    assert_eq!(result[1].coords.1, 200);
    assert_eq!(result[1].coords.2, 0.58);
    assert_eq!(result[2].coords.0, 10.0);
    assert_eq!(result[2].coords.1, 300);
    assert_eq!(result[2].coords.2, 1.0);
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(float_round = 3)]
struct TupleFloatRoundedF32 {
    id: i64,
    data: (f32, f32),
}

#[test]
fn float_round_on_tuple_f32() {
    let data = vec![
        TupleFloatRoundedF32 { id: 1, data: (1.234567, 9.876543) },
        TupleFloatRoundedF32 { id: 2, data: (0.0001, 0.9999) },
    ];

    let bytes = TupleFloatRoundedF32::serialize(data.clone()).unwrap();
    let result = TupleFloatRoundedF32::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result.len(), 2);
    assert_eq!(result[0].data.0, 1.235);
    assert_eq!(result[0].data.1, 9.877);
    assert_eq!(result[1].data.0, 0.0);
    assert_eq!(result[1].data.1, 1.0);
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(float_round = 2)]
struct TupleMixedWithGroupBy {
    group: i64,
    values: (f64, f64),
}

#[test]
fn float_round_on_tuple_with_index() {
    let data = vec![
        TupleMixedWithGroupBy { group: 1, values: (3.14159, 2.71828) },
        TupleMixedWithGroupBy { group: 1, values: (1.41421, 1.73205) },
        TupleMixedWithGroupBy { group: 2, values: (9.99999, 0.00001) },
    ];
    let bytes = TupleMixedWithGroupBy::serialize(data.clone()).unwrap();
    let result = TupleMixedWithGroupBy::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result.len(), 3);
    assert_eq!(result[0].values.0, 3.14);
    assert_eq!(result[0].values.1, 2.72);
    assert_eq!(result[1].values.0, 1.41);
    assert_eq!(result[1].values.1, 1.73);
    assert_eq!(result[2].values.0, 10.0);
    assert_eq!(result[2].values.1, 0.0);
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(float_round = 2)]
struct TupleAllFloats {
    id: i64,
    point: (f64, f64),
}

#[test]
fn float_round_on_tuple_all_floats() {
    let data = vec![TupleAllFloats { id: 1, point: (1.555, 2.445) }, TupleAllFloats { id: 2, point: (0.005, 9.995) }];
    let bytes = TupleAllFloats::serialize(data.clone()).unwrap();
    let result = TupleAllFloats::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result.len(), 2);
    assert_eq!(result[0].point.0, 1.56);
    assert_eq!(result[0].point.1, 2.44);
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(float_round = 2)]
struct TupleWithTimestamp {
    #[pco_pack(timestamp)]
    ts: chrono::DateTime<chrono::Utc>,
    measurement: (f64, i32),
}

#[test]
fn float_round_on_tuple_with_timestamp() {
    let data = vec![
        TupleWithTimestamp {
            ts: chrono::DateTime::<chrono::Utc>::from_timestamp(1000, 0).unwrap(),
            measurement: (3.14159, 42),
        },
        TupleWithTimestamp {
            ts: chrono::DateTime::<chrono::Utc>::from_timestamp(2000, 0).unwrap(),
            measurement: (2.71828, 99),
        },
    ];
    let bytes = TupleWithTimestamp::serialize(data.clone()).unwrap();
    let result = TupleWithTimestamp::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result.len(), 2);
    assert_eq!(result[0].measurement.0, 3.14);
    assert_eq!(result[0].measurement.1, 42);
    assert_eq!(result[1].measurement.0, 2.72);
    assert_eq!(result[1].measurement.1, 99);
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(float_round = 2)]
struct MapTupleValues {
    id: i64,
    metrics: std::collections::HashMap<String, (f64, i32, f64)>,
}

#[test]
fn float_round_on_map_tuple_values() {
    let data = vec![
        MapTupleValues {
            id: 1,
            metrics: {
                let mut m = std::collections::HashMap::new();
                m.insert("temp".into(), (3.14159, 100, 2.71828));
                m.insert("pressure".into(), (1013.25, 200, 101.325));
                m
            },
        },
        MapTupleValues { id: 2, metrics: std::collections::HashMap::new() },
        MapTupleValues {
            id: 3,
            metrics: {
                let mut m = std::collections::HashMap::new();
                m.insert("humidity".into(), (0.0001, 50, 0.9999));
                m
            },
        },
    ];
    let bytes = MapTupleValues::serialize(data.clone()).unwrap();
    let result = MapTupleValues::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result.len(), 3);
    assert_eq!(result[0].metrics["temp"].0, 3.14);
    assert_eq!(result[0].metrics["temp"].1, 100);
    assert_eq!(result[0].metrics["temp"].2, 2.72);
    assert_eq!(result[0].metrics["pressure"].0, 1013.25);
    assert_eq!(result[0].metrics["pressure"].1, 200);
    assert_eq!(result[0].metrics["pressure"].2, 101.33);
    assert!(result[1].metrics.is_empty());
    assert_eq!(result[2].metrics["humidity"].0, 0.0);
    assert_eq!(result[2].metrics["humidity"].1, 50);
    assert_eq!(result[2].metrics["humidity"].2, 1.0);
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(float_round = 2)]
struct MapTupleAllFloats {
    id: i64,
    points: std::collections::HashMap<String, (f64, f64)>,
}

#[test]
fn float_round_on_map_tuple_all_floats() {
    let data = vec![MapTupleAllFloats {
        id: 1,
        points: {
            let mut m = std::collections::HashMap::new();
            m.insert("origin".into(), (0.0, 0.0));
            m.insert("point_a".into(), (1.555, 2.445));
            m
        },
    }];
    let bytes = MapTupleAllFloats::serialize(data.clone()).unwrap();
    let result = MapTupleAllFloats::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].points["origin"].0, 0.0);
    assert_eq!(result[0].points["origin"].1, 0.0);
    assert_eq!(result[0].points["point_a"].0, 1.56);
    assert_eq!(result[0].points["point_a"].1, 2.44);
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(float_round = 2)]
struct OptionMapTupleValues {
    id: i64,
    metrics: Option<std::collections::HashMap<String, (f64, i32)>>,
}

#[test]
fn float_round_on_option_map_tuple_values() {
    let data = vec![
        OptionMapTupleValues {
            id: 1,
            metrics: Some({
                let mut m = std::collections::HashMap::new();
                m.insert("data".into(), (3.14159, 42));
                m
            }),
        },
        OptionMapTupleValues { id: 2, metrics: None },
    ];
    let bytes = OptionMapTupleValues::serialize(data.clone()).unwrap();
    let result = OptionMapTupleValues::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result.len(), 2);
    assert_eq!(result[0].metrics.as_ref().unwrap()["data"].0, 3.14);
    assert_eq!(result[0].metrics.as_ref().unwrap()["data"].1, 42);
    assert!(result[1].metrics.is_none());
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(float_round = 2)]
struct VecMapTupleValues {
    id: i64,
    batches: Vec<std::collections::HashMap<String, (f64, i32)>>,
}

#[test]
fn float_round_on_vec_map_tuple_values() {
    let data = vec![VecMapTupleValues {
        id: 1,
        batches: vec![
            {
                let mut m = std::collections::HashMap::new();
                m.insert("a".into(), (1.234, 10));
                m
            },
            {
                let mut m = std::collections::HashMap::new();
                m.insert("b".into(), (5.678, 20));
                m
            },
        ],
    }];
    let bytes = VecMapTupleValues::serialize(data.clone()).unwrap();
    let result = VecMapTupleValues::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].batches[0]["a"].0, 1.23);
    assert_eq!(result[0].batches[0]["a"].1, 10);
    assert_eq!(result[0].batches[1]["b"].0, 5.68);
    assert_eq!(result[0].batches[1]["b"].1, 20);
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(float_round = 2, index = [device_id])]
struct GroupByMapTupleValues {
    device_id: i64,
    metrics: std::collections::HashMap<String, (f64, i32)>,
}

#[test]
fn float_round_on_map_tuple_with_index() {
    let data = vec![
        GroupByMapTupleValues {
            device_id: 1,
            metrics: {
                let mut m = std::collections::HashMap::new();
                m.insert("temp".into(), (3.14159, 100));
                m
            },
        },
        GroupByMapTupleValues {
            device_id: 1,
            metrics: {
                let mut m = std::collections::HashMap::new();
                m.insert("temp".into(), (2.71828, 200));
                m
            },
        },
        GroupByMapTupleValues {
            device_id: 2,
            metrics: {
                let mut m = std::collections::HashMap::new();
                m.insert("temp".into(), (1.41421, 300));
                m
            },
        },
    ];
    let bytes = GroupByMapTupleValues::serialize(data.clone()).unwrap();
    let mut result = GroupByMapTupleValues::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    result.sort_by_key(|r| r.device_id);
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].metrics["temp"], (3.14, 100));
    assert_eq!(result[1].metrics["temp"], (2.72, 200));
    assert_eq!(result[2].metrics["temp"], (1.41, 300));
}
