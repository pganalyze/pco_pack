# pco_pack

Zero-config columnar compression for Rust

## Features

- `#[derive(PcoPack)]` for ideal compression of arbitrary nested types
- Numbers are compressed with [Pcodec](https://github.com/pcodec/pcodec); other types use MessagePack + zstd
- Extremely fast [filtering](#filtering) and [partial reads](#partial-reads)
- [Advanced schema evolution](#schema-evolution)
- [Novel timeseries compression](#timeline)

## Benchmarks

Detailed benchmarks are available in [benches/README.md](benches/README.md). See also [rust_serialization_benchmark](https://github.com/djkoloski/rust_serialization_benchmark) for an ecosystem-wide comparison.

## Supported types

| Type | Compression |
|------|-------------|
| `u8`, `u16`, `u32`, `u64`, `i8`, `i16`, `i32`, `i64`, `half::f16`, `f32`, `f64` | Pcodec |
| `bool`, `String`, `smol_str::SmolStr` | msgpack + zstd |
| `Option<T>`, `Vec<T>` | Flattened, using `T`-specific compression |
| `BTreeMap`, `HashMap` | Columnar compression of keys and values |
| `serde_json::Value`, `serde_bytes::ByteBuf` | msgpack + zstd |
| `chrono::DateTime<Utc>`, `pco_pack::Timeline` | Pcodec (microseconds since epoch) |
| `uuid::Uuid` | msgpack + zstd |
| Structs, enums, tuples | Columnar compression of each field |

Recommendations:
- Use `SmolStr` for short strings (<22 bytes) to improve serialization time
- Use `BTreeMap` for faster serialization, or use `HashMap` if hash lookup performance is more important
- Use `Option` instead of sentinel values (e.g. zero meaning null) for better compression
- Use `serde_bytes::ByteBuf` for binary data; `Vec<u8>` is assumed to be an array of numbers which is compressed with Pcodec
- Map data to a struct instead of storing JSON. Only use `serde_json::Value` when the schema is truly dynamic
- For the best compression, use Vecs for correlated data points (e.g. RGB color channels, coordinate pairs). Use structs or tuples for unrelated data so they're compressed separately
- UUIDs are supported but not recommended. UUIDs are random 128-bit values that do not compress well with any codec. In the future we may support timestamp-prefixed UUIDs with Pcodec compression of the timestamp portion, but even then the value will be much larger than makes sense in most use cases

## Schema evolution

- Struct fields can be added, removed, and reordered, but they cannot be renamed
- New fields automatically use `Default` when reading old data; no need to wrap in an `Option`
- Enum variants can be removed, reorered, and renamed as long as each variant has a discriminant value
- Numeric fields can change to any other numeric field; PcoPack automatically clamps the values to the new type's bounds. This allows you to safely switch to a larger type when you need more range, or to a smaller type for faster serialization and smaller compressed size
- Map fields can seamlessly switch between `HashMap` and `BTreeMap` because their underlying storage is the same
- Tuples support adding/removing fields at the end of the tuple, though it's better to use a struct to avoid future compatibility issues. Removing a field and then later adding a new field would cause incorrect historic data to be read for the new field

## Limitations

- All fields must implement `Default` (this is why `SystemTime` isn't supported)
- Pcodec doesn't support `i128` or `u128`
- Struct variants are not supported in enums
- Recursive data structures are not supported; use a flat layout instead

## Macro settings

- `index = [fields]` groups rows by these fields and stores a single columnar payload per unique index. Intended to be used with an indexed storage layer.
- `timestamp = field` marks a field as a timestamp for timeseries data. Before compression the data is sorted by this field for ideal pco compression. The serialized format includes `start_at` and `end_at` per chunk, enabling efficient range filtering. The timestamp field (and all other timestamps) are stored as `i64` microseconds internally.
- `float_round = N` reduces precision of float fields by rounding to `N` decimal places, improving compression when you don't need full float precision.
- `time_round = chrono::Duration::seconds(N)` rounds timestamps to the nearest multiple of the given duration (e.g. 10 seconds), reducing microsecond-level noise for better compression
- `chunk_size = N` sets the chunk size (default 32,768) used for serialization. This is mostly intended for testing; the benchmarks suggest that there isn't much benefit to changing the chunk size based on the size of your struct

Note: `float_round` and `time_round` are applied to nested types (e.g. collections), but must be explicitly set on nested structs or enums because they do not inherit the parent's settings.

### Struct example

```rust
use pco_pack::PcoPack;
use chrono::{DateTime, Duration, Utc};

#[derive(PcoPack)]
#[pco_pack(index = [device_id], timestamp = collected_at, float_round = 2, time_round = Duration::minutes(1))]
struct DeviceMetric {
    // device_id, start_at, and end_at are stored once per chunk
    device_id: i64,
    // rounded to nearest minute
    collected_at: DateTime<Utc>,
    // rounded to 2 decimal places
    temperature: f64,
}
```

### Enum example

```rust
use pco_pack::PcoPack;

#[derive(PcoPack, Debug, PartialEq, Default)]
#[pco_pack(float_round = 2)]
enum Metric {
    #[default]
    Null,
    Temperature(f64),
    Count(i32),
}
use Metric::*;

let data = vec![Temperature(23.999999), Count(42), Temperature(19.459999), Null];
let rows = Metric::deserialize(&Metric::serialize(data).unwrap()).unwrap();
assert_eq!(rows, vec![Temperature(24.0), Count(42), Temperature(19.46), Null]);
```

## Usage

### Serialize to bytes

```rust
use pco_pack::PcoPack;

#[derive(PcoPack, Debug, PartialEq)]
struct Event { id: i64, name: String, score: f32 }

let data = vec![
    Event { id: 1, name: "a".into(), score: 42.0 },
    Event { id: 2, name: "b".into(), score: 99.5 },
];

let bytes = Event::serialize(data).unwrap();
let all_rows = Event::deserialize(&bytes).unwrap();
assert_eq!(all_rows.len(), 2);

let results = Event::filter_bytes(&bytes, serde_json::json!({"id": 1}), &[]).unwrap();
assert_eq!(results, vec![Event { id: 1, name: "a".into(), score: 42.0 }]);
```

### Write to indexed data store

PcoPack exposes an intermediate compressed form as `PcoPack::Chunk` that stores metadata fields (index values, timestamp bounds) uncompressed while payload columns are compressed. This format can be written to an indexed data store to optimize reads.

```rust
use pco_pack::PcoPack;
use chrono::{DateTime, Utc};

#[derive(Default, Debug, PartialEq, PcoPack)]
#[pco_pack(index = [device_id], timestamp = collected_at)]
struct Sensor {
    device_id: i64,
    collected_at: DateTime<Utc>,
    temperature: f64,
}

let collected_at = DateTime::parse_from_rfc3339("2026-01-15T12:00:00Z").unwrap().with_timezone(&Utc);
let data = vec![
    Sensor { device_id: 1, collected_at, temperature: 42.0 },
    Sensor { device_id: 2, collected_at, temperature: 99.5 },
];
let chunks = Sensor::write(data).unwrap();

// Each chunk exposes metadata
for chunk in &chunks {
    println!("device_id: {}, ts range: {}..{}", chunk.device_id, chunk.start_at, chunk.end_at);
    // Store chunk in indexed storage, using device_id and timestamp bounds as index keys
}

// Later when reading data back, the compressed form can be fully decompressed
let all_rows = Sensor::read(chunks.clone()).unwrap();
assert_eq!(all_rows.len(), 2);

// Or efficiently filtered
let results = Sensor::filter(&chunks, serde_json::json!({"device_id": 1}), &[]).unwrap();
assert_eq!(results, vec![Sensor { device_id: 1, collected_at, temperature: 42.0 }]);
```

## Filtering

PcoPack offers ideal filtering performance with these optimizations:

- Filter fields are decompressed first, generating a bitmask of matching rows, allowing us to skip deserializing expensive types like JSON for unmatched rows (or even the entire chunk)
- Fields marked with `index` are stored uncompressed, enabling fast pre-filtering before column decompression
- Fields marked with `timestamp` store min/max bounds per chunk, enabling range-based chunk skipping

In benchmarks, PcoPack filtering is 4-28x faster than row-based alternatives (msgpack) and 4-10x faster than other columnar formats (serde_columnar) for equivalent queries.

### Typed filters

PcoPack generates a typed filter struct that provides ergonomic construction while still supporting arbitrary field access for nested types.

The typed filter supports:
- `new()` constructor for index and timestamp fields
- Direct field accessors for simple data types (i64, f64, String, bool, Uuid, DateTime)
- `Index` / `IndexMut` for complex nested fields (structs, maps, etc., stored as JSON)

When a struct has a `timestamp` field, the filter struct also provides range helpers:
- `range_bounds()` returns `(start, end)` from a range timestamp filter
- `range_duration()` returns the duration of the time range
- `range_shift(duration)` shifts the entire time range forward or backward

```rust
use pco_pack::PcoPack;
use chrono::{DateTime, Duration, Utc};

#[derive(PcoPack)]
#[pco_pack(index = [device_id], timestamp = collected_at)]
struct Sensor {
    device_id: i64,
    collected_at: DateTime<Utc>,
    temperature: f64,
}
type Filter = <Sensor as PcoPack>::Filter;

let now = Utc::now();
let data = vec![
    Sensor { device_id: 1, collected_at: now - Duration::minutes(5), temperature: 23.0 },
    Sensor { device_id: 2, collected_at: now, temperature: 42.0 },
];

// Filter::new accepts index and timestamp fields
let mut filter = Filter::new(1, (now - Duration::minutes(10))..=now);
let bytes = Sensor::serialize(data).unwrap();
let results = Sensor::filter_bytes(&bytes, filter, &[]).unwrap();
assert_eq!(results.len(), 1);

// Filters can also start out empty and be composed field by field
let mut filter = Filter::default();
filter.device_id = Some([1, 2].into());          // inclusion
filter.temperature = Some((20.0..=30.0).into()); // range
filter.temperature = Some(25.0.into());          // exact match
filter["nested_struct.id"] = serde_json::json!({ "start": 1, "end": 10 });
```

### JSON filters

Filters can be expressed as JSON objects mapping field paths to filter values:

```rust
use pco_pack::PcoPack;

#[derive(PcoPack)]
struct MyStruct { id: i64, name: String, score: f32, status: String }

let data = vec![
    MyStruct { id: 1, name: "alice".into(), score: 95.0, status: "active".into() },
    MyStruct { id: 2, name: "bob".into(), score: 80.0, status: "pending".into() },
];
let bytes = MyStruct::serialize(data).unwrap();

let filter = serde_json::json!({
    "id": 1,                                // exact match
    "name": "alice",                        // exact match
    "score": {"start": 50.0, "end": 100.0}, // range filter (inclusive)
    "status": ["active", "pending"],        // inclusion filter (any match)
});
let results = MyStruct::filter_bytes(&bytes, filter, &[]).unwrap();
assert_eq!(results.len(), 1);
assert_eq!(results[0].name, "alice");
```

### Filter types

| Filter | Syntax | Supported Types |
|--------|--------|-----------------|
| Exact match | `"field": value` | All types |
| Range | `"field": {"start": min, "end": max}` | Numeric types, `DateTime<Utc>`, `Timeline` |
| Inclusion | `"field": [v1, v2, ...]` | All types |

### Filter paths

- Top-level fields use the field name directly: `"id"`, `"name"`, `"temperature"`
- Nested fields use dot notation: `"meta.severity"`, `"data.value"`, `"children.id"`
- Tuples use numeric indices: `"tuple.0"`, `"tuple.1"`

## Partial reads

The `fields` parameter controls which fields are decompressed after filtering. Fields referenced by the filter are automatically included:

```rust
use pco_pack::PcoPack;

#[derive(PcoPack)]
struct MyStruct { id: i64, name: String, score: f32 }

let data = vec![
    MyStruct { id: 1, name: "alice".into(), score: 95.0 },
];
let bytes = MyStruct::serialize(data).unwrap();
let filter = serde_json::json!({"id": 1});

// Decompress all fields (default behavior)
let results = MyStruct::filter_bytes(&bytes, filter.clone(), &[]).unwrap();
assert_eq!(results[0].name, "alice");
assert_eq!(results[0].score, 95.0);

// Only decompress id and name; score gets Default (0.0)
let results = MyStruct::filter_bytes(&bytes, filter, &["id", "name"]).unwrap();
assert_eq!(results[0].name, "alice");
assert_eq!(results[0].score, 0.0);
```

## Timeline

`Timeline<N>` is a non-contiguous time range type that allows PcoPack to store a single row when all other fields in the struct are identical, significantly reducing serialization time and compressed size when data has >50% duplicates.

`N` is the bucket size in microseconds. When `N > 0`, each timestamp is floored to its bucket boundary and recorded as the full bucket window `[bucket_start, bucket_start + N)`. Adjacent and overlapping buckets are merged. Original timestamps within a bucket are discarded.

This is useful for de-noising timestamps or aggregating events into time windows. For example, with `Timeline<10_000_000>` (10-second buckets):
- Events at 3s and 7s both map to bucket `[0, 10s)`
- An event at 15s maps to bucket `[10s, 20s)`, which merges with the first bucket since they're adjacent
- Result: a single range `[0, 20s)` representing continuous activity

Note: `time_round` on the struct does not affect Timeline fields. Each Timeline controls its own resolution via its type parameter.

```rust
use pco_pack::{PcoPack, Timeline};

#[derive(PcoPack)]
#[pco_pack(timestamp = seen_at)]
struct DeviceReading {
    seen_at: Timeline<{chrono::Duration::seconds(11).num_microseconds().unwrap()}>,
    // Or seen_at: Timeline<11_000_000>,
    sensor_id: u32,
    temperature: f32,
    status: u8,
}
```

## License

MIT
