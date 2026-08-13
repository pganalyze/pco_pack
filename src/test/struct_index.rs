use crate as pco_pack;
use crate::PcoPack;

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(index = [sensor_id])]
struct SensorReading {
    sensor_id: i64,
    temperature: f64,
    label: String,
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
struct SimpleRecord {
    id: i64,
    value: i32,
}

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
enum Status {
    #[default]
    Active = 0,
    Inactive = 1,
    Pending = 2,
}

#[test]
fn test_per_chunk_serialization_with_index() {
    let data = vec![
        SensorReading { sensor_id: 1, temperature: 20.0, label: "a".into() },
        SensorReading { sensor_id: 2, temperature: 25.0, label: "b".into() },
        SensorReading { sensor_id: 1, temperature: 21.0, label: "c".into() },
        SensorReading { sensor_id: 3, temperature: 30.0, label: "d".into() },
        SensorReading { sensor_id: 2, temperature: 26.0, label: "e".into() },
    ];

    let chunks = SensorReading::write(data.clone()).unwrap();

    assert_eq!(chunks.len(), 3);

    for chunk in &chunks {
        assert!(!SensorReading::to_bytes(&[chunk.clone()]).unwrap().is_empty());
    }

    let bytes = SensorReading::serialize(data.clone()).unwrap();
    let result = SensorReading::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 5);
}

#[test]
fn test_per_chunk_serialization_no_index() {
    let data = vec![
        SimpleRecord { id: 1, value: 100 },
        SimpleRecord { id: 2, value: 200 },
        SimpleRecord { id: 3, value: 300 },
    ];

    let chunks = SimpleRecord::write(data.clone()).unwrap();

    assert_eq!(chunks.len(), 1);

    let bytes = SimpleRecord::serialize(data.clone()).unwrap();
    let result = SimpleRecord::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 3);
}

#[test]
fn test_per_chunk_serialization_enum() {
    let data = vec![Status::Active, Status::Inactive, Status::Pending, Status::Active];

    let chunks = Status::write(data.clone()).unwrap();
    assert_eq!(chunks.len(), 1);

    let bytes = Status::serialize(data.clone()).unwrap();
    let result = Status::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 4);
}

#[test]
fn test_per_chunk_serialization_empty() {
    let data: Vec<SensorReading> = Vec::new();
    let chunks = SensorReading::write(data).unwrap();
    assert_eq!(chunks.len(), 0);
}

#[test]
fn test_per_chunk_equivalent_to_regular() {
    let data = vec![
        SensorReading { sensor_id: 1, temperature: 20.0, label: "a".into() },
        SensorReading { sensor_id: 2, temperature: 25.0, label: "b".into() },
        SensorReading { sensor_id: 1, temperature: 21.0, label: "c".into() },
    ];

    let bytes = SensorReading::serialize(data.clone()).unwrap();
    let rows = SensorReading::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(rows.len(), 3);
}

#[test]
fn test_chunk_boundary_creates_two_chunks() {
    let n = SimpleRecord::CHUNK_SIZE + 1;
    let data: Vec<SimpleRecord> = (0..n).map(|i| SimpleRecord { id: i as i64, value: i as i32 }).collect();

    let chunks = SimpleRecord::write(data.clone()).unwrap();
    assert_eq!(chunks.len(), 2);

    let bytes = SimpleRecord::serialize(data.clone()).unwrap();
    let result = SimpleRecord::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), n);
}

#[test]
fn test_chunk_boundary_exact_size() {
    let n = SimpleRecord::CHUNK_SIZE;
    let data: Vec<SimpleRecord> = (0..n).map(|i| SimpleRecord { id: i as i64, value: i as i32 }).collect();

    let chunks = SimpleRecord::write(data.clone()).unwrap();
    assert_eq!(chunks.len(), 1);

    let bytes = SimpleRecord::serialize(data.clone()).unwrap();
    let result = SimpleRecord::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), n);
}

#[test]
fn test_chunk_boundary_multiple_groups() {
    let n = SimpleRecord::CHUNK_SIZE + 1;
    let data: Vec<SensorReading> = (0..n)
        .map(|i| SensorReading { sensor_id: 1, temperature: i as f64, label: format!("a_{}", i) })
        .chain((0..n).map(|i| SensorReading { sensor_id: 2, temperature: i as f64, label: format!("b_{}", i) }))
        .collect();

    let total_rows = 2 * n;
    let chunks = SensorReading::write(data.clone()).unwrap();
    assert_eq!(chunks.len(), 4);

    let bytes = SensorReading::serialize(data.clone()).unwrap();
    let result = SensorReading::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), total_rows);
}
