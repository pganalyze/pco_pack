use crate as pco_pack;
use crate::PcoPack;

#[test]
fn coerce_i32_to_f64() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct EventI32 {
        id: i64,
        count: i32,
    }

    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct EventF64 {
        id: i64,
        count: f64,
    }

    let data = vec![EventI32 { id: 1, count: 42 }, EventI32 { id: 2, count: -7 }, EventI32 { id: 3, count: i32::MAX }];
    let bytes = EventI32::serialize(data).unwrap();

    let result = EventF64::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].count, 42.0);
    assert_eq!(result[1].count, -7.0);
    assert_eq!(result[2].count, i32::MAX as f64);
}

#[test]
fn coerce_u64_to_f64() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct MetricU64 {
        id: i64,
        value: u64,
    }

    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct MetricF64 {
        id: i64,
        value: f64,
    }

    let data = vec![
        MetricU64 { id: 1, value: 9007199254740992u64 }, // 2^53
        MetricU64 { id: 2, value: u64::MAX },
    ];
    let bytes = MetricU64::serialize(data).unwrap();

    let result = MetricF64::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].value, 9_007_199_254_740_992_f64);
}

#[test]
fn coerce_i64_to_i32() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct EventI64 {
        id: i64,
        value: i64,
    }

    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct EventI32 {
        id: i64,
        value: i32,
    }

    let data =
        vec![EventI64 { id: 1, value: 42 }, EventI64 { id: 2, value: -7 }, EventI64 { id: 3, value: i32::MAX as i64 }];
    let bytes = EventI64::serialize(data).unwrap();

    let result = EventI32::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].value, 42);
    assert_eq!(result[1].value, -7);
    assert_eq!(result[2].value, i32::MAX);
}

#[test]
fn coerce_f64_to_i32_no_float_round() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct MetricF64 {
        id: i64,
        value: f64,
    }

    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct MetricI32 {
        id: i64,
        value: i32,
    }

    let data =
        vec![MetricF64 { id: 1, value: 3.9 }, MetricF64 { id: 2, value: -2.7 }, MetricF64 { id: 3, value: 100.0 }];
    let bytes = MetricF64::serialize(data).unwrap();

    let result = MetricI32::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].value, 3);
    assert_eq!(result[1].value, -2);
    assert_eq!(result[2].value, 100);
}

#[test]
fn coerce_i32_to_u64() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct CountI32 {
        id: i64,
        value: i32,
    }

    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct CountU64 {
        id: i64,
        value: u64,
    }

    let data = vec![CountI32 { id: 1, value: 42 }, CountI32 { id: 2, value: 0 }];
    let bytes = CountI32::serialize(data).unwrap();

    let result = CountU64::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].value, 42u64);
    assert_eq!(result[1].value, 0u64);
}

#[test]
fn coerce_u64_to_i64_clamps_max() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct MetricU64 {
        id: i64,
        value: u64,
    }

    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct MetricI64 {
        id: i64,
        value: i64,
    }

    let data = vec![
        MetricU64 { id: 1, value: u64::MAX },                 // overflows i64
        MetricU64 { id: 2, value: (i64::MAX as u64) + 1000 }, // also overflows
        MetricU64 { id: 3, value: i64::MAX as u64 },          // exactly at boundary
    ];
    let bytes = MetricU64::serialize(data).unwrap();

    let result = MetricI64::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].value, i64::MAX);
    assert_eq!(result[1].value, i64::MAX);
    assert_eq!(result[2].value, i64::MAX);
}

#[test]
fn coerce_i64_to_u64_clamps_min() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct MetricI64 {
        id: i64,
        value: i64,
    }

    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct MetricU64 {
        id: i64,
        value: u64,
    }

    let data = vec![
        MetricI64 { id: 1, value: i64::MIN }, // underflows u64
        MetricI64 { id: 2, value: -1000 },    // negative
        MetricI64 { id: 3, value: 0 },        // boundary
        MetricI64 { id: 4, value: 500 },      // in range
    ];
    let bytes = MetricI64::serialize(data).unwrap();

    let result = MetricU64::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 4);
    assert_eq!(result[0].value, 0u64);
    assert_eq!(result[1].value, 0u64);
    assert_eq!(result[2].value, 0u64);
    assert_eq!(result[3].value, 500u64);
}

#[test]
fn coerce_u64_to_f64_no_wrap() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct MetricU64 {
        id: i64,
        value: u64,
    }

    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct MetricF64 {
        id: i64,
        value: f64,
    }

    let data = vec![
        MetricU64 { id: 1, value: u64::MAX },
        MetricU64 { id: 2, value: (i64::MAX as u64) + 1 },
        MetricU64 { id: 3, value: 1_000_000_000_000_000_000u64 },
    ];
    let bytes = MetricU64::serialize(data).unwrap();

    let result = MetricF64::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 3);
    assert!(result[0].value > 0.0, "u64::MAX should not wrap to negative when read as f64");
    assert!(result[1].value > 0.0);
    assert!(result[2].value > 0.0);
}

#[test]
fn coerce_u64_to_f32_no_wrap() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct MetricU64 {
        id: i64,
        value: u64,
    }

    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct MetricF32 {
        id: i64,
        value: f32,
    }

    let data = vec![MetricU64 { id: 1, value: u64::MAX }];
    let bytes = MetricU64::serialize(data).unwrap();

    let result = MetricF32::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert!(result[0].value > 0.0, "u64::MAX should not wrap to negative when read as f32");
}

#[test]
fn coerce_u64_to_f16_no_wrap() {
    use half::f16;

    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct MetricU64 {
        id: i64,
        value: u64,
    }

    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct MetricF16 {
        id: i64,
        value: f16,
    }

    let data = vec![MetricU64 { id: 1, value: u64::MAX }];
    let bytes = MetricU64::serialize(data).unwrap();

    let result = MetricF16::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert!(result[0].value.is_sign_positive(), "u64::MAX should not wrap to negative when read as f16");
}

#[test]
fn coerce_i64_to_f16_succeeds() {
    use half::f16;

    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct MetricI64 {
        id: i64,
        value: i64,
    }

    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct MetricF16 {
        id: i64,
        value: f16,
    }

    let data = vec![MetricI64 { id: 1, value: 42 }, MetricI64 { id: 2, value: -100 }];
    let bytes = MetricI64::serialize(data).unwrap();

    let result = MetricF16::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].value.to_f64(), 42.0);
    assert_eq!(result[1].value.to_f64(), -100.0);
}

#[test]
fn coerce_u64_to_u32_clamps_max_not_min() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct MetricU64 {
        id: i64,
        value: u64,
    }

    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct MetricU32 {
        id: i64,
        value: u32,
    }

    let data = vec![
        MetricU64 { id: 1, value: u64::MAX },
        MetricU64 { id: 2, value: (u32::MAX as u64) + 1000 },
        MetricU64 { id: 3, value: u32::MAX as u64 },
    ];
    let bytes = MetricU64::serialize(data).unwrap();

    let result = MetricU32::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].value, u32::MAX);
    assert_eq!(result[1].value, u32::MAX);
    assert_eq!(result[2].value, u32::MAX);
}

#[test]
fn coerce_u64_to_u16_clamps_max_not_min() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct MetricU64 {
        id: i64,
        value: u64,
    }

    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct MetricU16 {
        id: i64,
        value: u16,
    }

    let data = vec![MetricU64 { id: 1, value: u64::MAX }];
    let bytes = MetricU64::serialize(data).unwrap();

    let result = MetricU16::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].value, u16::MAX);
}

#[test]
fn coerce_u64_to_u8_clamps_max_not_min() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct MetricU64 {
        id: i64,
        value: u64,
    }

    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct MetricU8 {
        id: i64,
        value: u8,
    }

    let data = vec![MetricU64 { id: 1, value: u64::MAX }];
    let bytes = MetricU64::serialize(data).unwrap();

    let result = MetricU8::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].value, u8::MAX);
}

#[test]
fn coerce_u32_to_i32_clamps_max() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct MetricU32 {
        id: i64,
        value: u32,
    }

    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct MetricI32 {
        id: i64,
        value: i32,
    }

    let data = vec![
        MetricU32 { id: 1, value: u32::MAX },                // overflows i32
        MetricU32 { id: 2, value: (i32::MAX as u32) + 100 }, // also overflows
        MetricU32 { id: 3, value: i32::MAX as u32 },         // exactly at boundary
    ];
    let bytes = MetricU32::serialize(data).unwrap();

    let result = MetricI32::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].value, i32::MAX);
    assert_eq!(result[1].value, i32::MAX);
    assert_eq!(result[2].value, i32::MAX);
}

#[test]
fn coerce_i32_to_u32_clamps_min() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct MetricI32 {
        id: i64,
        value: i32,
    }

    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct MetricU32 {
        id: i64,
        value: u32,
    }

    let data = vec![
        MetricI32 { id: 1, value: i32::MIN }, // underflows u32
        MetricI32 { id: 2, value: -1 },       // negative
        MetricI32 { id: 3, value: 0 },        // boundary
        MetricI32 { id: 4, value: 500 },      // in range
    ];
    let bytes = MetricI32::serialize(data).unwrap();

    let result = MetricU32::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 4);
    assert_eq!(result[0].value, 0u32);
    assert_eq!(result[1].value, 0u32);
    assert_eq!(result[2].value, 0u32);
    assert_eq!(result[3].value, 500u32);
}

#[test]
fn coerce_i64_to_i8_clamps_both_ends() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct MetricI64 {
        id: i64,
        value: i64,
    }

    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct MetricI8 {
        id: i64,
        value: i8,
    }

    let data = vec![
        MetricI64 { id: 1, value: i64::MIN }, // underflows
        MetricI64 { id: 2, value: -500 },     // below i8 range
        MetricI64 { id: 3, value: -128 },     // exactly at min
        MetricI64 { id: 4, value: 42 },       // in range
        MetricI64 { id: 5, value: 127 },      // exactly at max
        MetricI64 { id: 6, value: i64::MAX }, // overflows
    ];
    let bytes = MetricI64::serialize(data).unwrap();

    let result = MetricI8::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 6);
    assert_eq!(result[0].value, i8::MIN);
    assert_eq!(result[1].value, i8::MIN);
    assert_eq!(result[2].value, -128);
    assert_eq!(result[3].value, 42);
    assert_eq!(result[4].value, 127);
    assert_eq!(result[5].value, i8::MAX);
}

#[test]
fn coerce_f64_to_i32_clamps_extremes() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct MetricF64 {
        id: i64,
        value: f64,
    }

    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct MetricI32 {
        id: i64,
        value: i32,
    }

    let data = vec![
        MetricF64 { id: 1, value: f64::INFINITY },            // +inf -> clamp to max
        MetricF64 { id: 2, value: f64::NEG_INFINITY },        // -inf -> clamp to min
        MetricF64 { id: 3, value: (i32::MAX as f64) * 10.0 }, // way above max
        MetricF64 { id: 4, value: (i32::MIN as f64) * 10.0 }, // way below min
        MetricF64 { id: 5, value: 42.9 },                     // normal truncation
    ];
    let bytes = MetricF64::serialize(data).unwrap();

    let result = MetricI32::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 5);
    assert_eq!(result[0].value, i32::MAX);
    assert_eq!(result[1].value, i32::MIN);
    assert_eq!(result[2].value, i32::MAX);
    assert_eq!(result[3].value, i32::MIN);
    assert_eq!(result[4].value, 42);
}

#[test]
fn add_new_field_single() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct UserV1 {
        id: i64,
        name: String,
    }

    #[derive(Debug, Clone, PartialEq, PcoPack, Default)]
    struct UserV2 {
        id: i64,
        name: String,
        active: bool,
    }

    let users_v1 = vec![UserV1 { id: 1, name: "Alice".to_string() }, UserV1 { id: 2, name: "Bob".to_string() }];
    let bytes = UserV1::serialize(users_v1.clone()).unwrap();

    let result = UserV2::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].id, users_v1[0].id);
    assert_eq!(result[0].name, "Alice");
    assert_eq!(result[0].active, false);
    assert_eq!(result[1].id, users_v1[1].id);
    assert_eq!(result[1].name, "Bob");
    assert_eq!(result[1].active, false);
}

#[test]
fn add_new_field_at_start() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct SensorReadingV1 {
        device_id: i64,
        temperature: f64,
        timestamp_ms: i64,
    }

    #[derive(Debug, Clone, PartialEq, PcoPack, Default)]
    struct SensorReadingV2 {
        version: u32,
        device_id: i64,
        temperature: f64,
        timestamp_ms: i64,
    }

    let readings_v1 = vec![
        SensorReadingV1 { device_id: 42, temperature: 23.5, timestamp_ms: 1_700_000_000_000 },
        SensorReadingV1 { device_id: 99, temperature: -1.2, timestamp_ms: 1_700_000_000_100 },
    ];
    let bytes = SensorReadingV1::serialize(readings_v1.clone()).unwrap();

    let result = SensorReadingV2::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].version, 0);
    assert_eq!(result[0].device_id, readings_v1[0].device_id);
    assert!((result[0].temperature - readings_v1[0].temperature).abs() < 1e-10);
    assert_eq!(result[0].timestamp_ms, readings_v1[0].timestamp_ms);

    assert_eq!(result[1].version, 0);
    assert_eq!(result[1].device_id, readings_v1[1].device_id);
    assert!((result[1].temperature - readings_v1[1].temperature).abs() < 1e-10);
}

#[test]
fn add_new_field_in_middle() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct LogV1 {
        id: i64,
        level: String,
        message: String,
    }

    #[derive(Debug, Clone, PartialEq, Default, PcoPack)]
    struct LogV2 {
        id: i64,
        level: String,
        timestamp_ms: i64,
        message: String,
    }

    let logs_v1 = vec![LogV1 { id: 1, level: "INFO".to_string(), message: "started".to_string() }];
    let bytes = LogV1::serialize(logs_v1.clone()).unwrap();

    let result = LogV2::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].id, logs_v1[0].id);
    assert_eq!(result[0].level, "INFO");
    assert_eq!(result[0].timestamp_ms, 0);
    assert_eq!(result[0].message, "started");
}

#[test]
fn add_new_fields_multiple() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct ProductV1 {
        id: i64,
        name: String,
        price_cents: i64,
    }

    #[derive(Debug, Clone, PartialEq, Default, PcoPack)]
    struct ProductV2 {
        id: i64,
        name: String,
        description: String,
        price_cents: i64,
        in_stock: bool,
        category_id: Option<i64>,
    }

    let products_v1 = vec![ProductV1 { id: 100, name: "Widget".to_string(), price_cents: 2999 }];
    let bytes = ProductV1::serialize(products_v1.clone()).unwrap();

    // Read with v2. New fields get Defaults.
    let result = ProductV2::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].id, products_v1[0].id);
    assert_eq!(result[0].name, "Widget");
    assert_eq!(result[0].description, "");
    assert_eq!(result[0].price_cents, 2999);
    assert_eq!(result[0].in_stock, false);
    assert_eq!(result[0].category_id, None);
}

#[test]
fn add_new_field_different_position() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct EventV1 {
        id: i64,
        payload: String,
    }

    #[derive(Debug, Clone, PartialEq, Default, PcoPack)]
    struct EventV2 {
        id: i64,
        tags: Vec<String>,
        payload: String,
    }

    let events_v1 = vec![EventV1 { id: 7, payload: "hello".to_string() }];
    let bytes = EventV1::serialize(events_v1.clone()).unwrap();

    let result = EventV2::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].id, events_v1[0].id);
    assert!(result[0].tags.is_empty());
    assert_eq!(result[0].payload, "hello");
}

#[test]
fn add_new_bool_field_filter_on_old_data() {
    /// v1: original schema without active
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct UserV1 {
        id: i64,
        name: String,
    }

    #[derive(Debug, Clone, PartialEq, PcoPack, Default)]
    struct UserV2 {
        id: i64,
        name: String,
        active: bool,
    }

    let users_v1 = vec![UserV1 { id: 1, name: "Alice".to_string() }, UserV1 { id: 2, name: "Bob".to_string() }];
    let bytes = UserV1::serialize(users_v1.clone()).unwrap();

    let result = UserV2::filter_bytes(&bytes, serde_json::json!({"active": true}), &[]).unwrap();
    assert_eq!(result.len(), 0);

    let result = UserV2::filter_bytes(&bytes, serde_json::json!({"active": false}), &[]).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].id, users_v1[0].id);
    assert_eq!(result[0].name, "Alice");
    assert_eq!(result[0].active, false);
    assert_eq!(result[1].id, users_v1[1].id);
    assert_eq!(result[1].name, "Bob");
    assert_eq!(result[1].active, false);
}

#[test]
fn add_new_string_field_filter_on_old_data() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct EventV1 {
        id: i64,
        message: String,
    }

    #[derive(Debug, Clone, PartialEq, PcoPack, Default)]
    struct EventV2 {
        id: i64,
        message: String,
        source: String,
    }

    let events_v1 =
        vec![EventV1 { id: 1, message: "hello".to_string() }, EventV1 { id: 2, message: "world".to_string() }];
    let bytes = EventV1::serialize(events_v1.clone()).unwrap();

    let result = EventV2::filter_bytes(&bytes, serde_json::json!({"source": "api"}), &[]).unwrap();
    assert_eq!(result.len(), 0);

    let result = EventV2::filter_bytes(&bytes, serde_json::json!({"source": ""}), &[]).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].id, events_v1[0].id);
    assert_eq!(result[0].message, "hello");
    assert_eq!(result[0].source, "");
}

#[test]
fn add_new_i64_field_filter_on_old_data() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct ItemV1 {
        id: i64,
        name: String,
    }

    #[derive(Debug, Clone, PartialEq, Default, PcoPack)]
    struct ItemV2 {
        id: i64,
        name: String,
        quantity: i64,
    }

    let items_v1 = vec![ItemV1 { id: 1, name: "A".to_string() }];
    let bytes = ItemV1::serialize(items_v1.clone()).unwrap();

    assert_eq!(ItemV2::filter_bytes(&bytes, serde_json::json!({"quantity": 5}), &[]).unwrap().len(), 0);

    let result = ItemV2::filter_bytes(&bytes, serde_json::json!({"quantity": 0}), &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].quantity, 0);
}

#[test]
fn add_new_f64_field_filter_on_old_data() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct ItemV1 {
        id: i64,
        name: String,
    }

    #[derive(Debug, Clone, PartialEq, Default, PcoPack)]
    struct ItemV2 {
        id: i64,
        name: String,
        price: f64,
    }

    let items_v1 = vec![ItemV1 { id: 1, name: "A".to_string() }];
    let bytes = ItemV1::serialize(items_v1.clone()).unwrap();

    assert_eq!(ItemV2::filter_bytes(&bytes, serde_json::json!({"price": 9.99}), &[]).unwrap().len(), 0);

    let result = ItemV2::filter_bytes(&bytes, serde_json::json!({"price": 0.0}), &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].price, 0.0);
}

#[test]
fn add_new_uuid_field_filter_on_old_data() {
    use uuid::Uuid;

    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct EventV1 {
        id: i64,
        message: String,
    }

    #[derive(Debug, Clone, PartialEq, Default, PcoPack)]
    struct EventV2 {
        id: i64,
        message: String,
        user_id: Uuid,
    }

    let events_v1 = vec![EventV1 { id: 1, message: "hello".to_string() }];
    let bytes = EventV1::serialize(events_v1.clone()).unwrap();

    let some_uuid = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
    let nil_uuid = Uuid::nil(); // Default for Uuid

    assert_eq!(
        EventV2::filter_bytes(&bytes, serde_json::json!({"user_id": some_uuid.to_string()}), &[]).unwrap().len(),
        0
    );

    let result = EventV2::filter_bytes(&bytes, serde_json::json!({"user_id": nil_uuid.to_string()}), &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].user_id, nil_uuid);
}

#[test]
fn add_new_option_field_filter_on_old_data() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct TaskV1 {
        id: i64,
        title: String,
    }

    #[derive(Debug, Clone, PartialEq, Default, PcoPack)]
    struct TaskV2 {
        id: i64,
        title: String,
        parent_id: Option<i64>,
    }

    let tasks_v1 = vec![TaskV1 { id: 1, title: "root".to_string() }];
    let bytes = TaskV1::serialize(tasks_v1.clone()).unwrap();

    let result = TaskV2::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].parent_id, None);
}

#[test]
fn add_new_vec_field_filter_on_old_data() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct PostV1 {
        id: i64,
        title: String,
    }

    #[derive(Debug, Clone, PartialEq, Default, PcoPack)]
    struct PostV2 {
        id: i64,
        title: String,
        tags: Vec<String>,
    }

    let posts_v1 = vec![PostV1 { id: 1, title: "Hello".to_string() }];
    let bytes = PostV1::serialize(posts_v1.clone()).unwrap();

    let result = PostV2::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert!(result[0].tags.is_empty());
}

#[test]
fn add_new_bytebuf_field_filter_on_old_data() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct RecordV1 {
        id: i64,
        name: String,
    }

    #[derive(Debug, Clone, PartialEq, Default, PcoPack)]
    struct RecordV2 {
        id: i64,
        name: String,
        payload: serde_bytes::ByteBuf,
    }

    let records_v1 = vec![RecordV1 { id: 1, name: "test".to_string() }];
    let bytes = RecordV1::serialize(records_v1.clone()).unwrap();

    let result = RecordV2::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert!(result[0].payload.is_empty());
}

#[test]
fn add_new_json_value_field_filter_on_old_data() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct ConfigV1 {
        id: i64,
        name: String,
    }

    #[derive(Debug, Clone, PartialEq, Default, PcoPack)]
    struct ConfigV2 {
        id: i64,
        name: String,
        metadata: serde_json::Value,
    }

    let configs_v1 = vec![ConfigV1 { id: 1, name: "app".to_string() }];
    let bytes = ConfigV1::serialize(configs_v1.clone()).unwrap();

    let result = ConfigV2::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].metadata, serde_json::Value::Null);
}

#[test]
fn remove_field_single() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct ContactV1 {
        id: i64,
        name: String,
        email: String,
    }

    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct ContactV2 {
        id: i64,
        name: String,
    }

    let contacts_v1 = vec![ContactV1 { id: 1, name: "Alice".to_string(), email: "alice@example.com".to_string() }];
    let bytes = ContactV1::serialize(contacts_v1.clone()).unwrap();

    let result = ContactV2::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].id, contacts_v1[0].id);
    assert_eq!(result[0].name, "Alice");
}

#[test]
fn remove_field_at_start() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct BatchV1 {
        batch_id: String,
        count: i64,
        status: String,
    }

    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct BatchV2 {
        count: i64,
        status: String,
    }

    let batches_v1 = vec![BatchV1 { batch_id: "batch-abc".to_string(), count: 500, status: "done".to_string() }];
    let bytes = BatchV1::serialize(batches_v1.clone()).unwrap();

    let result = BatchV2::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].count, batches_v1[0].count);
    assert_eq!(result[0].status, "done");
}

#[test]
fn remove_field_in_middle() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct MeasurementV1 {
        id: i64,
        sensor_id: String,
        value: f64,
        unit: String,
        timestamp_ms: i64,
    }

    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct MeasurementV2 {
        id: i64,
        value: f64,
        unit: String,
        timestamp_ms: i64,
    }

    let measurements_v1 = vec![MeasurementV1 {
        id: 1,
        sensor_id: "TEMP-01".to_string(),
        value: 23.5,
        unit: "C".to_string(),
        timestamp_ms: 1_700_000_000_000,
    }];
    let bytes = MeasurementV1::serialize(measurements_v1.clone()).unwrap();

    let result = MeasurementV2::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].id, measurements_v1[0].id);
    assert!((result[0].value - measurements_v1[0].value).abs() < 1e-10);
    assert_eq!(result[0].unit, "C");
    assert_eq!(result[0].timestamp_ms, measurements_v1[0].timestamp_ms);
}

#[test]
fn remove_fields_multiple() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct AuditLogV1 {
        id: i64,
        actor_id: i64,
        action: String,
        target_type: String,
        target_id: i64,
        metadata: String,
        ip_address: String,
        timestamp_ms: i64,
    }

    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct AuditLogV2 {
        id: i64,
        actor_id: i64,
        action: String,
        timestamp_ms: i64,
    }

    let logs_v1 = vec![AuditLogV1 {
        id: 42,
        actor_id: 5,
        action: "update".to_string(),
        target_type: "user".to_string(),
        target_id: 99,
        metadata: "{}".to_string(),
        ip_address: "10.0.0.1".to_string(),
        timestamp_ms: 1_700_000_000_000,
    }];
    let bytes = AuditLogV1::serialize(logs_v1.clone()).unwrap();

    let result = AuditLogV2::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].id, logs_v1[0].id);
    assert_eq!(result[0].actor_id, logs_v1[0].actor_id);
    assert_eq!(result[0].action, "update");
    assert_eq!(result[0].timestamp_ms, logs_v1[0].timestamp_ms);
}

#[test]
fn reorder_swap_adjacent() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct PointV1 {
        x: f64,
        y: f64,
        z: f64,
    }

    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct PointV2 {
        y: f64,
        x: f64,
        z: f64,
    }

    let points_v1 = vec![PointV1 { x: 1.0, y: 2.0, z: 3.0 }];
    let bytes = PointV1::serialize(points_v1.clone()).unwrap();

    let result = PointV2::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert!((result[0].x - points_v1[0].x).abs() < 1e-10);
    assert!((result[0].y - points_v1[0].y).abs() < 1e-10);
    assert!((result[0].z - points_v1[0].z).abs() < 1e-10);
}

#[test]
fn reorder_field_end_to_start() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct AlertV1 {
        id: i64,
        message: String,
        severity: u8,
        source: String,
    }

    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct AlertV2 {
        severity: u8,
        id: i64,
        message: String,
        source: String,
    }

    let alerts_v1 =
        vec![AlertV1 { id: 100, message: "CPU high".to_string(), severity: 3, source: "node-1".to_string() }];
    let bytes = AlertV1::serialize(alerts_v1.clone()).unwrap();

    let result = AlertV2::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].severity, alerts_v1[0].severity);
    assert_eq!(result[0].id, alerts_v1[0].id);
    assert_eq!(result[0].message, "CPU high");
    assert_eq!(result[0].source, "node-1");
}

#[test]
fn reorder_complex_permutation() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct TaskV1 {
        id: i64,
        title: String,
        assignee_id: i64,
        priority: u8,
        due_date_ms: i64,
        status: String,
    }

    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct TaskV2 {
        status: String,
        assignee_id: i64,
        id: i64,
        priority: u8,
        title: String,
        due_date_ms: i64,
    }

    let tasks_v1 = vec![TaskV1 {
        id: 7,
        title: "Fix bug".to_string(),
        assignee_id: 3,
        priority: 5,
        due_date_ms: 1_700_000_000_000,
        status: "in_progress".to_string(),
    }];
    let bytes = TaskV1::serialize(tasks_v1.clone()).unwrap();

    let result = TaskV2::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].status, "in_progress");
    assert_eq!(result[0].assignee_id, tasks_v1[0].assignee_id);
    assert_eq!(result[0].id, tasks_v1[0].id);
    assert_eq!(result[0].priority, tasks_v1[0].priority);
    assert_eq!(result[0].title, "Fix bug");
    assert_eq!(result[0].due_date_ms, tasks_v1[0].due_date_ms);
}

#[test]
fn reorder_and_add_field() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct SensorDataV1 {
        id: i64,
        value: f64,
        unit: String,
    }

    #[derive(Debug, Clone, PartialEq, Default, PcoPack)]
    struct SensorDataV2 {
        unit: String,
        timestamp_ms: i64,
        value: f64,
        id: i64,
    }

    let data_v1 = vec![SensorDataV1 { id: 1, value: 98.6, unit: "F".to_string() }];
    let bytes = SensorDataV1::serialize(data_v1.clone()).unwrap();

    let result = SensorDataV2::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].unit, "F");
    assert_eq!(result[0].timestamp_ms, 0);
    assert!((result[0].value - data_v1[0].value).abs() < 1e-10);
    assert_eq!(result[0].id, data_v1[0].id);
}

#[test]
fn reorder_and_remove_field() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct TransactionV1 {
        id: i64,
        amount_cents: i64,
        currency: String,
        description: String,
        created_at_ms: i64,
    }

    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct TransactionV2 {
        amount_cents: i64,
        created_at_ms: i64,
        id: i64,
        currency: String,
    }

    let txns_v1 = vec![TransactionV1 {
        id: 500,
        amount_cents: 2999,
        currency: "USD".to_string(),
        description: "Coffee".to_string(),
        created_at_ms: 1_700_000_000_000,
    }];
    let bytes = TransactionV1::serialize(txns_v1.clone()).unwrap();

    let result = TransactionV2::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].amount_cents, txns_v1[0].amount_cents);
    assert_eq!(result[0].created_at_ms, txns_v1[0].created_at_ms);
    assert_eq!(result[0].id, txns_v1[0].id);
    assert_eq!(result[0].currency, "USD");
}
