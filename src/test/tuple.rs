use crate as pco_pack;
use crate::PcoPack;

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct Tuple2 {
    field: (i32, u64),
}
#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct Tuple3 {
    field: (i64, f32, bool),
}
#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct Tuple4 {
    field: (u8, u16, u32, u64),
}

#[test]
fn tuple_2_roundtrip() {
    let data: Vec<Tuple2> = vec![
        Tuple2 { field: (1, 100) },
        Tuple2 { field: (2, 200) },
        Tuple2 { field: (0, 0) },
        Tuple2 { field: (-1, 999) },
    ];
    let bytes = Tuple2::serialize(data.clone()).unwrap();
    let result = Tuple2::deserialize(&bytes).unwrap();
    assert_eq!(data, result);
}

#[test]
fn tuple_3_roundtrip() {
    let data: Vec<Tuple3> = vec![
        Tuple3 { field: (1, 1.5, true) },
        Tuple3 { field: (2, -2.5, false) },
        Tuple3 { field: (0, 0.0, true) },
        Tuple3 { field: (-100, 999.99, false) },
    ];
    let bytes = Tuple3::serialize(data.clone()).unwrap();
    let result = Tuple3::deserialize(&bytes).unwrap();
    assert_eq!(data, result);
}

#[test]
fn tuple_4_roundtrip() {
    let data: Vec<Tuple4> = vec![
        Tuple4 { field: (1, 2, 3, 4) },
        Tuple4 { field: (255, 65535, u32::MAX, u64::MAX) },
        Tuple4 { field: (0, 0, 0, 0) },
    ];
    let bytes = Tuple4::serialize(data.clone()).unwrap();
    let result = Tuple4::deserialize(&bytes).unwrap();
    assert_eq!(data, result);
}

#[test]
fn tuple_add_field_at_end() {
    #[derive(Debug, Clone, PartialEq, Default, PcoPack)]
    struct PointV1 {
        coord: (i32, i32),
    }

    #[derive(Debug, Clone, PartialEq, Default, PcoPack)]
    struct PointV2 {
        coord: (i32, i32, i32),
    }

    let data_v1 = vec![PointV1 { coord: (10, 20) }, PointV1 { coord: (-5, 42) }];
    let bytes = PointV1::serialize(data_v1.clone()).unwrap();

    let result = PointV2::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].coord.0, data_v1[0].coord.0);
    assert_eq!(result[0].coord.1, data_v1[0].coord.1);
    assert_eq!(result[0].coord.2, 0);

    assert_eq!(result[1].coord.0, data_v1[1].coord.0);
    assert_eq!(result[1].coord.1, data_v1[1].coord.1);
    assert_eq!(result[1].coord.2, 0);
}

#[test]
fn tuple_remove_field_at_end() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct ReadingV1 {
        value: (i64, f64, String), // (id, temp, unit)
    }

    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct ReadingV2 {
        value: (i64, f64), // (id, temp)
    }

    let data_v1 = vec![ReadingV1 { value: (1, 23.5, "C".to_string()) }];
    let bytes = ReadingV1::serialize(data_v1.clone()).unwrap();

    let result = ReadingV2::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].value.0, data_v1[0].value.0);
    assert!((result[0].value.1 - data_v1[0].value.1).abs() < 1e-10);
}

#[test]
fn tuple_add_multiple_fields_at_end() {
    #[derive(Debug, Clone, PartialEq, Default, PcoPack)]
    struct VecV1 {
        data: (u8, u16), // version, length
    }

    /// v2 adds two more fields at the end.
    #[derive(Debug, Clone, PartialEq, Default, PcoPack)]
    struct VecV2 {
        data: (u8, u16, bool, String), // version, length, compressed, format_name
    }

    let data_v1 = vec![VecV1 { data: (2, 4096) }];
    let bytes = VecV1::serialize(data_v1.clone()).unwrap();

    let result = VecV2::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].data.0, data_v1[0].data.0); // version.
    assert_eq!(result[0].data.1, data_v1[0].data.1); // length.
    assert!(!result[0].data.2); // compressed defaults to false.
    assert_eq!(result[0].data.3, ""); // format_name defaults to empty string.
}

#[test]
fn tuple_remove_multiple_fields_at_end() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct EventV1 {
        detail: (i64, u32, f64, bool), // id, priority, score, important
    }

    /// v2 keeps only the first two fields.
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct EventV2 {
        detail: (i64, u32), // just id and priority
    }

    let data_v1 = vec![EventV1 { detail: (99, 5, 3.14, true) }];
    let bytes = EventV1::serialize(data_v1.clone()).unwrap();

    let result = EventV2::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].detail.0, data_v1[0].detail.0); // id preserved.
    assert_eq!(result[0].detail.1, data_v1[0].detail.1); // priority preserved.
}

#[test]
fn tuple_add_field_and_struct_field() {
    #[derive(Debug, Clone, PartialEq, Default, PcoPack)]
    struct SampleV1 {
        id: i64,
        coord: (i32, i32),
    }

    #[derive(Debug, Clone, PartialEq, Default, PcoPack)]
    struct SampleV2 {
        id: i64,
        coord: (i32, i32, u64), // added timestamp_ms
        status: String,
    }

    let data_v1 = vec![SampleV1 { id: 7, coord: (50, 100) }];
    let bytes = SampleV1::serialize(data_v1.clone()).unwrap();

    let result = SampleV2::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].id, data_v1[0].id);
    assert_eq!(result[0].coord.0, data_v1[0].coord.0);
    assert_eq!(result[0].coord.1, data_v1[0].coord.1);
    assert_eq!(result[0].coord.2, 0); // timestamp_ms defaults to 0.
    assert_eq!(result[0].status, ""); // status defaults to empty string.
}

#[test]
fn tuple_remove_non_tail_field_errors() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct RecV1 {
        t: (i32, u64, String), // positions: 0=i32, 1=u64, 2=String
    }

    /// Reader removes the middle field. This is UNSAFE.
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct RecV2Bad {
        t: (i32, String), // reader pos 0->writer col 0=i32 OK; reader pos 1 tries to read writer col 1=u64 as String
    }

    let data_v1 = vec![RecV1 { t: (42, 999u64, "hello".to_string()) }];
    let bytes = RecV1::serialize(data_v1.clone()).unwrap();

    // Schema mismatch: reader expects String at position 1, but writer stored u64 there.
    // Both the new format and fallback deserialization fail, so we get an error.
    let result = RecV2Bad::filter_bytes(&bytes, serde_json::json!({}), &[]);
    assert!(result.is_err());
    let err = result.unwrap_err();
    let full_err = format!("{:?}", err);
    assert!(full_err.contains("String"));
}
