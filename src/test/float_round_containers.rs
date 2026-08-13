use crate as pco_pack;
use crate::PcoPack;
use std::collections::{BTreeMap, HashMap};

#[test]
fn float_round_on_hashmap() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    #[pco_pack(float_round = 2)]
    struct MetricRecord {
        id: i64,
        metrics: HashMap<String, f64>,
    }

    let data = vec![
        MetricRecord {
            id: 1,
            metrics: {
                let mut m = HashMap::new();
                m.insert("temperature".into(), 23.456);
                m.insert("humidity".into(), 45.678);
                m
            },
        },
        MetricRecord {
            id: 2,
            metrics: {
                let mut m = HashMap::new();
                m.insert("temperature".into(), 24.123);
                m.insert("pressure".into(), 1013.256);
                m
            },
        },
    ];

    let bytes = MetricRecord::serialize(data.clone()).unwrap();
    let result = MetricRecord::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result.len(), 2);

    let r1 = &result.iter().find(|r| r.id == 1).unwrap();
    assert_eq!(r1.metrics["temperature"], 23.46);
    assert_eq!(r1.metrics["humidity"], 45.68);

    let r2 = &result.iter().find(|r| r.id == 2).unwrap();
    assert_eq!(r2.metrics["temperature"], 24.12);
    assert_eq!(r2.metrics["pressure"], 1013.26);
}

#[test]
fn float_round_on_hashmap_f32() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    #[pco_pack(float_round = 3)]
    struct F32Metrics {
        id: i64,
        metrics: HashMap<String, f32>,
    }

    let data = vec![F32Metrics {
        id: 1,
        metrics: {
            let mut m = HashMap::new();
            m.insert("val".into(), 1.234567f32);
            m
        },
    }];

    let bytes = F32Metrics::serialize(data.clone()).unwrap();
    let result = F32Metrics::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].metrics["val"], 1.235);
}

#[test]
fn float_round_on_btreemap() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    #[pco_pack(float_round = 1)]
    struct BTMapRecord {
        id: i64,
        metrics: BTreeMap<String, f64>,
    }

    let data = vec![BTMapRecord {
        id: 1,
        metrics: {
            let mut m = BTreeMap::new();
            m.insert("alpha".into(), 3.456);
            m.insert("beta".into(), 7.854);
            m
        },
    }];

    let bytes = BTMapRecord::serialize(data.clone()).unwrap();
    let result = BTMapRecord::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].metrics["alpha"], 3.5);
    assert_eq!(result[0].metrics["beta"], 7.9);
}

#[test]
fn float_round_on_hashmap_with_index() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    #[pco_pack(index = [device_id], float_round = 2)]
    struct GroupedMetric {
        device_id: i64,
        metrics: HashMap<String, f64>,
    }

    let data = vec![
        GroupedMetric {
            device_id: 1,
            metrics: {
                let mut m = HashMap::new();
                m.insert("temp".into(), 21.456);
                m
            },
        },
        GroupedMetric {
            device_id: 2,
            metrics: {
                let mut m = HashMap::new();
                m.insert("temp".into(), 22.789);
                m
            },
        },
        GroupedMetric {
            device_id: 1,
            metrics: {
                let mut m = HashMap::new();
                m.insert("temp".into(), 21.544);
                m
            },
        },
    ];

    let bytes = GroupedMetric::serialize(data.clone()).unwrap();
    let result = GroupedMetric::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result.len(), 3);

    let dev1: Vec<_> = result.iter().filter(|r| r.device_id == 1).collect();
    assert_eq!(dev1.len(), 2);
    assert_eq!(dev1[0].metrics["temp"], 21.46);
    assert_eq!(dev1[1].metrics["temp"], 21.54);

    let dev2: Vec<_> = result.iter().filter(|r| r.device_id == 2).collect();
    assert_eq!(dev2.len(), 1);
    assert_eq!(dev2[0].metrics["temp"], 22.79);
}

#[test]
fn float_round_on_hashmap_with_timestamp() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    #[pco_pack(timestamp = ts, float_round = 3)]
    struct TimestampedMetric {
        ts: i64,
        metrics: HashMap<String, f64>,
    }

    let data = vec![
        TimestampedMetric {
            ts: 100,
            metrics: {
                let mut m = HashMap::new();
                m.insert("val".into(), 1.234567);
                m
            },
        },
        TimestampedMetric {
            ts: 200,
            metrics: {
                let mut m = HashMap::new();
                m.insert("val".into(), 2.345678);
                m
            },
        },
        TimestampedMetric {
            ts: 300,
            metrics: {
                let mut m = HashMap::new();
                m.insert("val".into(), 3.456789);
                m
            },
        },
    ];

    let bytes = TimestampedMetric::serialize(data.clone()).unwrap();
    let result = TimestampedMetric::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result.len(), 3);
    assert_eq!(result[0].metrics["val"], 1.235);
    assert_eq!(result[1].metrics["val"], 2.346);
    assert_eq!(result[2].metrics["val"], 3.457);
}

#[test]
fn float_round_on_hashmap_empty_map() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    #[pco_pack(float_round = 2)]
    struct WithEmptyMap {
        id: i64,
        metrics: HashMap<String, f64>,
    }

    let data = vec![
        WithEmptyMap { id: 1, metrics: HashMap::new() },
        WithEmptyMap {
            id: 2,
            metrics: {
                let mut m = HashMap::new();
                m.insert("val".into(), 1.234);
                m
            },
        },
    ];

    let bytes = WithEmptyMap::serialize(data.clone()).unwrap();
    let result = WithEmptyMap::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result.len(), 2);
    assert!(result[0].metrics.is_empty());
    assert_eq!(result[0].id, 1);
    assert_eq!(result[1].metrics["val"], 1.23);
}

#[test]
fn float_round_on_option_f64() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    #[pco_pack(float_round = 2)]
    struct OptionRecord {
        id: i64,
        value: Option<f64>,
    }

    let data = vec![
        OptionRecord { id: 1, value: Some(1.234) },
        OptionRecord { id: 2, value: None },
        OptionRecord { id: 3, value: Some(5.678) },
    ];

    let bytes = OptionRecord::serialize(data.clone()).unwrap();
    let result = OptionRecord::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result.len(), 3);
    assert_eq!(result[0].id, 1);
    assert!(result[0].value.is_some());
    assert_eq!(result[0].value.unwrap(), 1.23);
    assert_eq!(result[1].id, 2);
    assert!(result[1].value.is_none());
    assert_eq!(result[2].id, 3);
    assert!(result[2].value.is_some());
    assert_eq!(result[2].value.unwrap(), 5.68);
}

#[test]
fn float_round_on_option_f32() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    #[pco_pack(float_round = 3)]
    struct OptionF32Record {
        id: i64,
        value: Option<f32>,
    }

    let data = vec![OptionF32Record { id: 1, value: Some(1.234567f32) }, OptionF32Record { id: 2, value: None }];

    let bytes = OptionF32Record::serialize(data.clone()).unwrap();
    let result = OptionF32Record::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result.len(), 2);
    assert_eq!(result[0].id, 1);
    assert!(result[0].value.is_some());
    assert_eq!(result[0].value.unwrap(), 1.235);
    assert_eq!(result[1].id, 2);
    assert!(result[1].value.is_none());
}

#[test]
fn float_round_on_option_with_index() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    #[pco_pack(index = [category], float_round = 2)]
    struct GroupedOption {
        category: i64,
        value: Option<f64>,
    }

    let data = vec![
        GroupedOption { category: 1, value: Some(1.234) },
        GroupedOption { category: 2, value: Some(2.345) },
        GroupedOption { category: 1, value: None },
        GroupedOption { category: 2, value: Some(2.678) },
    ];

    let bytes = GroupedOption::serialize(data.clone()).unwrap();
    let result = GroupedOption::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result.len(), 4);
    let cat1: Vec<_> = result.iter().filter(|r| r.category == 1).collect();
    assert_eq!(cat1.len(), 2);
    assert!(cat1[0].value.is_some());
    assert_eq!(cat1[0].value.unwrap(), 1.23);
    assert!(cat1[1].value.is_none());
    let cat2: Vec<_> = result.iter().filter(|r| r.category == 2).collect();
    assert_eq!(cat2.len(), 2);
    assert_eq!(cat2[0].value.unwrap(), 2.35);
    assert_eq!(cat2[1].value.unwrap(), 2.68);
}

#[test]
fn float_round_on_option_with_timestamp() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    #[pco_pack(timestamp = ts, float_round = 2)]
    struct TimestampedOption {
        ts: i64,
        value: Option<f64>,
    }

    let data = vec![
        TimestampedOption { ts: 100, value: Some(1.234) },
        TimestampedOption { ts: 200, value: None },
        TimestampedOption { ts: 300, value: Some(3.456) },
    ];

    let bytes = TimestampedOption::serialize(data.clone()).unwrap();
    let result = TimestampedOption::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result.len(), 3);
    assert_eq!(result[0].ts, 100);
    assert_eq!(result[0].value.unwrap(), 1.23);
    assert!(result[1].value.is_none());
    assert_eq!(result[2].ts, 300);
    assert_eq!(result[2].value.unwrap(), 3.46);
}

#[test]
fn float_round_on_vec_f64() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    #[pco_pack(float_round = 2)]
    struct VecRecord {
        id: i64,
        values: Vec<f64>,
    }

    let data = vec![
        VecRecord { id: 1, values: vec![1.234, 5.678, 9.012] },
        VecRecord { id: 2, values: vec![] },
        VecRecord { id: 3, values: vec![3.456] },
    ];

    let bytes = VecRecord::serialize(data.clone()).unwrap();
    let result = VecRecord::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result.len(), 3);
    assert_eq!(result[0].id, 1);
    assert_eq!(result[0].values.len(), 3);
    assert_eq!(result[0].values[0], 1.23);
    assert_eq!(result[0].values[1], 5.68);
    assert_eq!(result[0].values[2], 9.01);
    assert_eq!(result[1].id, 2);
    assert!(result[1].values.is_empty());
    assert_eq!(result[2].id, 3);
    assert_eq!(result[2].values.len(), 1);
    assert_eq!(result[2].values[0], 3.46);
}

#[test]
fn float_round_on_vec_f32() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    #[pco_pack(float_round = 3)]
    struct VecF32Record {
        id: i64,
        values: Vec<f32>,
    }

    let data = vec![VecF32Record { id: 1, values: vec![1.234567f32, 2.345678f32] }];

    let bytes = VecF32Record::serialize(data.clone()).unwrap();
    let result = VecF32Record::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].values.len(), 2);
    assert_eq!(result[0].values[0], 1.235);
    assert_eq!(result[0].values[1], 2.346);
}

#[test]
fn float_round_on_vec_with_index() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    #[pco_pack(index = [category], float_round = 2)]
    struct GroupedVec {
        category: i64,
        values: Vec<f64>,
    }

    let data = vec![
        GroupedVec { category: 1, values: vec![1.234, 5.678] },
        GroupedVec { category: 2, values: vec![2.345] },
        GroupedVec { category: 1, values: vec![9.012] },
    ];

    let bytes = GroupedVec::serialize(data.clone()).unwrap();
    let result = GroupedVec::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result.len(), 3);
    let cat1: Vec<_> = result.iter().filter(|r| r.category == 1).collect();
    assert_eq!(cat1.len(), 2);
    assert_eq!(cat1[0].values[0], 1.23);
    assert_eq!(cat1[0].values[1], 5.68);
    assert_eq!(cat1[1].values[0], 9.01);
    let cat2: Vec<_> = result.iter().filter(|r| r.category == 2).collect();
    assert_eq!(cat2.len(), 1);
    assert_eq!(cat2[0].values[0], 2.35);
}

#[test]
fn float_round_on_vec_with_timestamp() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    #[pco_pack(timestamp = ts, float_round = 2)]
    struct TimestampedVec {
        ts: i64,
        values: Vec<f64>,
    }

    let data =
        vec![TimestampedVec { ts: 100, values: vec![1.234, 5.678] }, TimestampedVec { ts: 200, values: vec![9.012] }];

    let bytes = TimestampedVec::serialize(data.clone()).unwrap();
    let result = TimestampedVec::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result.len(), 2);
    assert_eq!(result[0].ts, 100);
    assert_eq!(result[0].values[0], 1.23);
    assert_eq!(result[0].values[1], 5.68);
    assert_eq!(result[1].ts, 200);
    assert_eq!(result[1].values[0], 9.01);
}

#[test]
fn float_round_on_option_vec_mixed() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    #[pco_pack(float_round = 2)]
    struct MixedContainerRecord {
        id: i64,
        opt_value: Option<f64>,
        vec_values: Vec<f64>,
    }

    let data = vec![
        MixedContainerRecord { id: 1, opt_value: Some(1.234), vec_values: vec![5.678, 9.012] },
        MixedContainerRecord { id: 2, opt_value: None, vec_values: vec![] },
    ];

    let bytes = MixedContainerRecord::serialize(data.clone()).unwrap();
    let result = MixedContainerRecord::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result.len(), 2);
    assert_eq!(result[0].opt_value.unwrap(), 1.23);
    assert_eq!(result[0].vec_values[0], 5.68);
    assert_eq!(result[0].vec_values[1], 9.01);
    assert!(result[1].opt_value.is_none());
    assert!(result[1].vec_values.is_empty());
}

#[test]
fn float_round_on_option_map_mixed() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    #[pco_pack(float_round = 2)]
    struct OptionMapMixed {
        id: i64,
        opt_value: Option<f64>,
        metrics: HashMap<String, f64>,
    }

    let data = vec![
        OptionMapMixed {
            id: 1,
            opt_value: Some(1.234),
            metrics: {
                let mut m = HashMap::new();
                m.insert("val".into(), 5.678);
                m
            },
        },
        OptionMapMixed { id: 2, opt_value: None, metrics: HashMap::new() },
    ];

    let bytes = OptionMapMixed::serialize(data.clone()).unwrap();
    let result = OptionMapMixed::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result.len(), 2);
    assert_eq!(result[0].opt_value.unwrap(), 1.23);
    assert_eq!(result[0].metrics["val"], 5.68);
    assert!(result[1].opt_value.is_none());
    assert!(result[1].metrics.is_empty());
}

#[test]
fn float_round_on_vec_map_mixed() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    #[pco_pack(float_round = 2)]
    struct VecMapMixed {
        id: i64,
        values: Vec<f64>,
        metrics: HashMap<String, f64>,
    }

    let data = vec![VecMapMixed {
        id: 1,
        values: vec![1.234, 5.678],
        metrics: {
            let mut m = HashMap::new();
            m.insert("val".into(), 9.012);
            m
        },
    }];

    let bytes = VecMapMixed::serialize(data.clone()).unwrap();
    let result = VecMapMixed::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].values[0], 1.23);
    assert_eq!(result[0].values[1], 5.68);
    assert_eq!(result[0].metrics["val"], 9.01);
}
