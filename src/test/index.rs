use crate as pco_pack;
use crate::PcoPack;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(index = [device_id])]
struct DeviceTelemetry {
    device_id: i64,
    temperature: i64,
    humidity: i64,
}

#[test]
fn index_roundtrip() {
    let data = vec![
        DeviceTelemetry { device_id: 1, temperature: 20, humidity: 50 },
        DeviceTelemetry { device_id: 2, temperature: 25, humidity: 60 },
        DeviceTelemetry { device_id: 1, temperature: 21, humidity: 51 },
        DeviceTelemetry { device_id: 2, temperature: 26, humidity: 61 },
        DeviceTelemetry { device_id: 1, temperature: 22, humidity: 52 },
    ];
    let bytes = DeviceTelemetry::serialize(data.clone()).unwrap();
    let result = DeviceTelemetry::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 5);
    let dev1_rows: Vec<_> = result.iter().filter(|r| r.device_id == 1).collect();
    assert_eq!(dev1_rows.len(), 3);
    let dev2_rows: Vec<_> = result.iter().filter(|r| r.device_id == 2).collect();
    assert_eq!(dev2_rows.len(), 2);
}

#[test]
fn index_single_group() {
    let data = vec![
        DeviceTelemetry { device_id: 1, temperature: 20, humidity: 50 },
        DeviceTelemetry { device_id: 1, temperature: 21, humidity: 51 },
        DeviceTelemetry { device_id: 1, temperature: 22, humidity: 52 },
    ];
    let bytes = DeviceTelemetry::serialize(data.clone()).unwrap();
    let result = DeviceTelemetry::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result.len(), 3);
    for i in 0..3 {
        assert_eq!(result[i].device_id, 1);
        assert_eq!(result[i].temperature, 20 + i as i64);
    }
}

#[test]
fn index_each_row_different_group() {
    let data = vec![
        DeviceTelemetry { device_id: 1, temperature: 20, humidity: 50 },
        DeviceTelemetry { device_id: 2, temperature: 25, humidity: 60 },
        DeviceTelemetry { device_id: 3, temperature: 30, humidity: 70 },
    ];
    let bytes = DeviceTelemetry::serialize(data.clone()).unwrap();
    let result = DeviceTelemetry::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 3);
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(timestamp = collected_at, index = [database_id])]
struct QueryStat {
    database_id: i64,
    collected_at: i64,
    fingerprint: i64,
    calls: i64,
    rows: i64,
}

#[test]
fn combined_index_range_roundtrip() {
    let data = vec![
        QueryStat { database_id: 1, collected_at: 300, fingerprint: 100, calls: 50, rows: 1000 },
        QueryStat { database_id: 2, collected_at: 100, fingerprint: 200, calls: 30, rows: 500 },
        QueryStat { database_id: 1, collected_at: 100, fingerprint: 101, calls: 55, rows: 1100 },
        QueryStat { database_id: 2, collected_at: 300, fingerprint: 201, calls: 35, rows: 600 },
        QueryStat { database_id: 1, collected_at: 200, fingerprint: 102, calls: 60, rows: 1200 },
    ];
    let bytes = QueryStat::serialize(data.clone()).unwrap();
    let result = QueryStat::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result.len(), 5);

    let db1: Vec<_> = result.iter().filter(|r| r.database_id == 1).collect();
    assert_eq!(db1.len(), 3);
    assert_eq!(db1[0].collected_at, 100);
    assert_eq!(db1[1].collected_at, 200);
    assert_eq!(db1[2].collected_at, 300);

    let db2: Vec<_> = result.iter().filter(|r| r.database_id == 2).collect();
    assert_eq!(db2.len(), 2);
    assert_eq!(db2[0].collected_at, 100);
    assert_eq!(db2[1].collected_at, 300);
}

#[test]
fn filter_on_index_field() {
    let data = vec![
        DeviceTelemetry { device_id: 1, temperature: 20, humidity: 50 },
        DeviceTelemetry { device_id: 2, temperature: 25, humidity: 60 },
        DeviceTelemetry { device_id: 1, temperature: 21, humidity: 51 },
        DeviceTelemetry { device_id: 3, temperature: 30, humidity: 70 },
    ];
    let bytes = DeviceTelemetry::serialize(data.clone()).unwrap();
    let result = DeviceTelemetry::filter_bytes(&bytes, serde_json::json!({"device_id": 1}), &[]).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].device_id, 1);
    assert_eq!(result[1].device_id, 1);
}

#[test]
fn combined_empty() {
    let bytes = QueryStat::serialize(Vec::new()).unwrap();
    let result = QueryStat::deserialize(&bytes).unwrap();
    assert!(result.is_empty());
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(index = [region])]
struct RegionMetric {
    region: String,
    value: i64,
}

#[test]
fn index_string_field_roundtrip() {
    let data = vec![
        RegionMetric { region: "us-east".into(), value: 10 },
        RegionMetric { region: "eu-west".into(), value: 20 },
        RegionMetric { region: "us-east".into(), value: 30 },
        RegionMetric { region: "ap-south".into(), value: 40 },
        RegionMetric { region: "eu-west".into(), value: 50 },
    ];
    let bytes = RegionMetric::serialize(data.clone()).unwrap();
    let result = RegionMetric::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result.len(), 5);
    let us_east: Vec<_> = result.iter().filter(|r| r.region == "us-east").collect();
    let eu_west: Vec<_> = result.iter().filter(|r| r.region == "eu-west").collect();
    let ap_south: Vec<_> = result.iter().filter(|r| r.region == "ap-south").collect();

    assert_eq!(us_east.len(), 2);
    assert_eq!(eu_west.len(), 2);
    assert_eq!(ap_south.len(), 1);
}

#[test]
fn index_string_field_filter() {
    let data = vec![
        RegionMetric { region: "us-east".into(), value: 10 },
        RegionMetric { region: "eu-west".into(), value: 20 },
        RegionMetric { region: "us-east".into(), value: 30 },
    ];
    let bytes = RegionMetric::serialize(data).unwrap();

    let result = RegionMetric::filter_bytes(&bytes, serde_json::json!({"region": "us-east"}), &[]).unwrap();
    assert_eq!(result.len(), 2);
    assert!(result.iter().all(|r| r.region == "us-east"));
}

#[test]
fn index_string_field_empty_value() {
    let data = vec![
        RegionMetric { region: "".into(), value: 1 },
        RegionMetric { region: "a".into(), value: 2 },
        RegionMetric { region: "".into(), value: 3 },
    ];
    let bytes = RegionMetric::serialize(data).unwrap();

    let result = RegionMetric::filter_bytes(&bytes, serde_json::json!({"region": ""}), &[]).unwrap();
    assert_eq!(result.len(), 2);
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(index = [device_id, region])]
struct MultiIndexMetric {
    device_id: i64,
    region: String,
    value: f64,
}

#[test]
fn index_multi_field_roundtrip() {
    let data = vec![
        MultiIndexMetric { device_id: 1, region: "us".into(), value: 10.0 },
        MultiIndexMetric { device_id: 2, region: "eu".into(), value: 20.0 },
        MultiIndexMetric { device_id: 1, region: "eu".into(), value: 30.0 },
        MultiIndexMetric { device_id: 1, region: "us".into(), value: 40.0 },
        MultiIndexMetric { device_id: 2, region: "us".into(), value: 50.0 },
    ];
    let bytes = MultiIndexMetric::serialize(data.clone()).unwrap();
    let result = MultiIndexMetric::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result.len(), 5);
}

#[test]
fn index_multi_field_filter_first() {
    let data = vec![
        MultiIndexMetric { device_id: 1, region: "us".into(), value: 10.0 },
        MultiIndexMetric { device_id: 2, region: "eu".into(), value: 20.0 },
        MultiIndexMetric { device_id: 1, region: "eu".into(), value: 30.0 },
    ];
    let bytes = MultiIndexMetric::serialize(data).unwrap();

    let result = MultiIndexMetric::filter_bytes(&bytes, serde_json::json!({"device_id": 1}), &[]).unwrap();
    assert_eq!(result.len(), 2);
    assert!(result.iter().all(|r| r.device_id == 1));
}

#[test]
fn index_multi_field_filter_second() {
    let data = vec![
        MultiIndexMetric { device_id: 1, region: "us".into(), value: 10.0 },
        MultiIndexMetric { device_id: 2, region: "eu".into(), value: 20.0 },
        MultiIndexMetric { device_id: 1, region: "eu".into(), value: 30.0 },
    ];
    let bytes = MultiIndexMetric::serialize(data).unwrap();

    let result = MultiIndexMetric::filter_bytes(&bytes, serde_json::json!({"region": "eu"}), &[]).unwrap();
    assert_eq!(result.len(), 2);
    assert!(result.iter().all(|r| r.region == "eu"));
}

#[test]
fn index_multi_field_filter_both() {
    let data = vec![
        MultiIndexMetric { device_id: 1, region: "us".into(), value: 10.0 },
        MultiIndexMetric { device_id: 2, region: "eu".into(), value: 20.0 },
        MultiIndexMetric { device_id: 1, region: "eu".into(), value: 30.0 },
        MultiIndexMetric { device_id: 1, region: "us".into(), value: 40.0 },
    ];
    let bytes = MultiIndexMetric::serialize(data).unwrap();

    let result =
        MultiIndexMetric::filter_bytes(&bytes, serde_json::json!({"device_id": 1, "region": "us"}), &[]).unwrap();
    assert_eq!(result.len(), 2);
    for r in &result {
        assert_eq!(r.device_id, 1);
        assert_eq!(r.region, "us");
    }
}

#[test]
fn index_multi_field_empty() {
    let bytes = MultiIndexMetric::serialize(Vec::new()).unwrap();
    let result = MultiIndexMetric::deserialize(&bytes).unwrap();
    assert!(result.is_empty());
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(index = [tenant_id])]
struct TenantMetric {
    tenant_id: Uuid,
    value: i64,
}

#[test]
fn index_uuid_field_roundtrip() {
    let t1 = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
    let t2 = Uuid::parse_str("6ba7b810-9dad-11d1-80b4-00c04fd430c8").unwrap();

    let data = vec![
        TenantMetric { tenant_id: t1, value: 10 },
        TenantMetric { tenant_id: t2, value: 20 },
        TenantMetric { tenant_id: t1, value: 30 },
        TenantMetric { tenant_id: t2, value: 40 },
    ];

    let bytes = TenantMetric::serialize(data.clone()).unwrap();
    let result = TenantMetric::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result.len(), 4);
}

#[test]
fn index_uuid_field_filter_single() {
    let t1 = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
    let t2 = Uuid::parse_str("6ba7b810-9dad-11d1-80b4-00c04fd430c8").unwrap();

    let data = vec![
        TenantMetric { tenant_id: t1, value: 10 },
        TenantMetric { tenant_id: t2, value: 20 },
        TenantMetric { tenant_id: t1, value: 30 },
    ];

    let bytes = TenantMetric::serialize(data).unwrap();

    // Filter on the UUID index field using a string (parsed to Uuid)
    let result = TenantMetric::filter_bytes(
        &bytes,
        serde_json::json!({"tenant_id": "550e8400-e29b-41d4-a716-446655440000"}),
        &[],
    )
    .unwrap();

    assert_eq!(result.len(), 2);
    assert!(result.iter().all(|r| r.tenant_id == t1));
}

#[test]
fn index_uuid_field_filter_inclusion() {
    let t1 = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
    let t2 = Uuid::parse_str("6ba7b810-9dad-11d1-80b4-00c04fd430c8").unwrap();
    let t3 = Uuid::parse_str("6ba7b810-9dad-11d1-80b4-00c04fd430c9").unwrap();

    let data = vec![
        TenantMetric { tenant_id: t1, value: 10 },
        TenantMetric { tenant_id: t2, value: 20 },
        TenantMetric { tenant_id: t3, value: 30 },
        TenantMetric { tenant_id: t1, value: 40 },
    ];

    let bytes = TenantMetric::serialize(data).unwrap();
    let result = TenantMetric::filter_bytes(
        &bytes,
        serde_json::json!({"tenant_id": ["550e8400-e29b-41d4-a716-446655440000", "6ba7b810-9dad-11d1-80b4-00c04fd430c8"]}),
        &[],
    )
    .unwrap();

    assert_eq!(result.len(), 3);
    for r in &result {
        assert!(r.tenant_id == t1 || r.tenant_id == t2);
    }
}

#[test]
fn index_uuid_field_filter_no_match() {
    let t1 = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();

    let data = vec![TenantMetric { tenant_id: t1, value: 10 }];
    let bytes = TenantMetric::serialize(data).unwrap();
    let result = TenantMetric::filter_bytes(
        &bytes,
        serde_json::json!({"tenant_id": "00000000-0000-0000-0000-000000000001"}),
        &[],
    )
    .unwrap();

    assert!(result.is_empty());
}

#[test]
fn index_uuid_field_empty() {
    let bytes = TenantMetric::serialize(Vec::new()).unwrap();
    let result = TenantMetric::deserialize(&bytes).unwrap();
    assert!(result.is_empty());
}
