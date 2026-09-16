# Benchmarks

All serialization times and sizes include compression (either Pcodec or zstd). These numbers are captured on an M5 Max MBP with fans set to max. There is a lot of run-to-run variance; [contributions are welcome](https://github.com/pganalyze/pco_pack/issues/3) to help smooth out the data.

## Collections

- `Vec<i32>` is 2x faster and 10x smaller because each element gets Pcodec compression
- `Option<i32>` is 2x faster and 16x smaller by tracking `Some` as indexes, then unwrapping the inner values so Pcodec compression can be used
- `ByteBuf` and `serde_json::Value` are serialized with msgpack and compressed with zstd in both formats, though PcoPack's wrapper adds some overhead
- `BTreeMap`/`HashMap` with String keys are 8-10x smaller because keys and values are stored as separately compressed columns
  - Note: SmolStr keys are faster than String keys because their small stack allocation is faster to serialize and hash

| Type                            | PcoPack | msgpack | Time ratio | PcoPack | msgpack | Size ratio |
|---------------------------------|---------|---------|------------|---------|---------|------------|
| `Vec<i32> (avg 4)`              | 1.2 ms  | 2.3 ms  | 1.92x      | 52 KB   | 533 KB  | 10.1x      |
| `Vec<String> (avg 4)`           | 12.1 ms | 7.4 ms  | 0.61x      | 139 KB  | 138 KB  | 1.0x       |
| `Option<i32> (50% null)`        | 0.2 ms  | 0.4 ms  | 2.00x      | 6 KB    | 105 KB  | 15.6x      |
| `i32 (50% zero as sentinel)`    | 0.3 ms  | 0.4 ms  | 1.33x      | 19 KB   | 105 KB  | 5.5x       |
| `ByteBuf (avg 128B)`            | 6.9 ms  | 3.6 ms  | 0.52x      | 40 KB   | 39 KB   | 1.0x       |
| `serde_json::Value`             | 16.1 ms | 7.4 ms  | 0.46x      | 254 KB  | 241 KB  | 0.9x       |
| `BTreeMap<(String, i32), i32>`  | 2.0 ms  | 1.6 ms  | 0.80x      | 16 KB   | 119 KB  | 7.5x       |
| `BTreeMap<(SmolStr, i32), i32>` | 1.2 ms  | 4.7 ms  | 3.92x      | 16 KB   | 119 KB  | 7.5x       |
| `HashMap<(String, i32), i32>`   | 2.3 ms  | 1.9 ms  | 0.83x      | 16 KB   | 159 KB  | 10.0x      |
| `HashMap<(SmolStr, i32), i32>`  | 1.4 ms  | 5.7 ms  | 4.07x      | 16 KB   | 159 KB  | 10.0x      |

## Numbers

PcoPack is faster and smaller across most types, though msgpack's bit packing and zstd's compression of zero-byte sequences can outperform PcoPack with small integer ranges.

| Type                     | PcoPack | msgpack | Time ratio | PcoPack | msgpack | Size ratio |
|--------------------------|---------|---------|------------|---------|---------|------------|
| `i8 (0-999)`             | 0.3 ms  | 0.3 ms  | 1.00x      | 20 KB   | 5 KB    | 0.3x       |
| `i16 (0-999)`            | 0.3 ms  | 0.4 ms  | 1.33x      | 20 KB   | 11 KB   | 0.5x       |
| `u8 (0-255)`             | 0.1 ms  | 0.3 ms  | 3.00x      | 23 B    | 394 B   | 17.1x      |
| `u16 (0-999)`            | 0.2 ms  | 0.3 ms  | 1.50x      | 177 B   | 1 KB    | 10.3x      |
| `i32 (full)`             | 0.2 ms  | 0.8 ms  | 4.00x      | 204 KB  | 448 KB  | 2.2x       |
| `i64 (full)`             | 0.3 ms  | 1.0 ms  | 3.33x      | 389 KB  | 849 KB  | 2.2x       |
| `u32 (full)`             | 0.2 ms  | 0.6 ms  | 3.00x      | 372 KB  | 437 KB  | 1.2x       |
| `f32 (normal)`           | 0.3 ms  | 0.6 ms  | 2.00x      | 26 KB   | 373 KB  | 13.9x      |
| `f64 (normal)`           | 0.3 ms  | 0.9 ms  | 3.00x      | 34 KB   | 765 KB  | 22.0x      |
| `i64 (80% zero, sparse)` | 0.3 ms  | 0.3 ms  | 1.00x      | 65 KB   | 63 KB   | 1.0x       |

## Others

PcoPack significantly outperforms when storing enums and timestamps because of the efficient internal layout and Pcodec compression of numbers.

`bool`, `String`, and `SmolStr` are all internally compressed with msgpack, so PcoPack has the same compressed size but slower serialization in order to support lazy deserialization and fallback serialization formats.

| Type               | PcoPack | msgpack | Time ratio | PcoPack | msgpack | Size ratio |
|--------------------|---------|---------|------------|---------|---------|------------|
| `bool (50% true)`  | 0.3 ms  | 0.2 ms  | 0.67x      | 24 B    | 24 B    | 1.0x       |
| `String`           | 5.3 ms  | 3.0 ms  | 0.57x      | 104 KB  | 94 KB   | 0.9x       |
| `SmolStr`          | 2.6 ms  | 2.1 ms  | 0.81x      | 104 KB  | 94 KB   | 0.9x       |
| `Enum (simple)`    | 0.2 ms  | 1.3 ms  | 6.50x      | 19 KB   | 27 KB   | 1.4x       |
| `Enum (complex)`   | 1.3 ms  | 2.1 ms  | 1.62x      | 33 KB   | 129 KB  | 3.8x       |
| `chrono::DateTime` | 0.6 ms  | 7.3 ms  | 12.17x     | 22 KB   | 109 KB  | 4.8x       |
| `uuid::Uuid`       | 1.6 ms  | 1.6 ms  | 1.00x      | 367 KB  | 367 KB  | 1.0x       |

## Structs

### PcoPack vs columnar, serde_columnar, msgpack

- PcoPack compressed size is 3-5x smaller. Filtering is 4-28x faster via lazy decompression and `index`/`timestamp` indexing
- columnar has the fastest roundtrip serialization, but the compressed size is 3x larger than PcoPack
- serde_columnar has better compressed size than columnar because of per-field encoding (which users must manually set)
- msgpack has the worst compressed size and roundtrip time when including filtering. This uses a traditional row-based layout instead of a columnar layout, highlighting why a columnar layout is beneficial

### PcoPack vs Vortex

PcoPack achieves 15% faster roundtrip serialization and 17% smaller compressed size. Vortex acheives a significantly faster 1-row filter while PcoPack outperforms with the account_id filter using its built-in indexed serialization format.

To summarize: PcoPack and Vortex have similar performance. Choose Vortex if ecosystem support is important and you want to customize the compression modes. Chose PcoPack if you want great performance with minimal code.

| Metric                          | PcoPack  | columnar | serde_columnar | msgpack  | Vortex   |
|---------------------------------|----------|----------|----------------|----------|----------|
| Serialize                       | 113.3 ms | 68.0 ms  | 95.6 ms        | 97.3 ms  | 141.5 ms |
| Deserialize                     | 74.0 ms  | 23.3 ms  | 69.3 ms        | 108.4 ms | 77.6 ms  |
| Size                            | 1383 KB  | 7.4 MB   | 5.4 MB         | 8.2 MB   | 1673 KB  |
| Filter account_id (20% of rows) | 19.5 ms  | 24.4 ms  | 70.9 ms        | 106.1 ms | 32.6 ms  |
| Filter color + score (1 row)    | 9.4 ms   | 24.3 ms  | 71.5 ms        | 103.9 ms | 0.8 ms   |

## Timeline

Scenario: 100 sensors report every second for 1,000 seconds. All rows in a given second share one timestamp with sub-second jitter. Uniqueness is the probability that a sensor's readings differ from its previous second.

Findings:
- `Timeline` merges duplicate observations into time ranges, achieving smaller sizes depending on uniqueness. The benefit grows as readings stay stable longer (more duplicates to merge). However, it requires changing your struct field type and adds serialization overhead at high uniqueness, where the extra cost may not be worth it
- `time_round = Duration::seconds(1)` consistently saves ~28 KB on the timestamp column across all scenarios. At low uniqueness this is a large fraction of total size (1.29x), but at high uniqueness the non-timestamp fields dominate and dilute the ratio to 1.05x

### Serialization time

| Uniqueness | Timeline | DateTime | time_round=1s | Time ratio (TL/DT) | Time ratio (TR/DT) |
|------------|----------|----------|---------------|--------------------|--------------------|
| 10%        | 1.4 ms   | 6.9 ms   | 6.5 ms        | 4.9x               | 1.06x              |
| 20%        | 2.3 ms   | 7.1 ms   | 6.8 ms        | 3.1x               | 1.04x              |
| 50%        | 5.0 ms   | 6.1 ms   | 5.8 ms        | 1.2x               | 1.05x              |
| 80%        | 8.0 ms   | 4.2 ms   | 3.9 ms        | 0.5x               | 1.08x              |
| 90%        | 9.0 ms   | 4.2 ms   | 3.9 ms        | 0.5x               | 1.08x              

### Compressed size

| Uniqueness | Timeline | DateTime | time_round=1s | Size ratio (TL/DT) | Size ratio (TR/DT) |
|------------|----------|----------|---------------|--------------------|--------------------|
| 10%        | 67 KB    | 126 KB   | 97 KB         | 1.9x               | 1.29x              |
| 20%        | 130 KB   | 189 KB   | 161 KB        | 1.4x               | 1.18x              |
| 50%        | 319 KB   | 485 KB   | 457 KB        | 1.5x               | 1.06x              |
| 80%        | 484 KB   | 600 KB   | 571 KB        | 1.2x               | 1.05x              |
| 90%        | 532 KB   | 600 KB   | 571 KB        | 1.1x               | 1.05x              |

## float_round

`#[pco_pack(float_round = N)]` significantly reduces compressed size and slightly worsens serialization time.

| Metric      | PcoPack (no round) | PcoPack (round=2) | msgpack |
|-------------|--------------------|-------------------|---------|
| Serialize   | 77.3 ms            | 84.1 ms           | 47.2 ms |
| Deserialize | 49.1 ms            | 47.2 ms           | 49.5 ms |
| Size        | 2.5 MB             | 983 KB            | 7.6 MB  |

## time_round

`#[pco_pack(float_round = Duration::seconds(60))]` moderately reduces compressed size and slightly improves serialization time.

| Metric      | PcoPack (no round) | PcoPack (round=60s) | msgpack |
|-------------|--------------------|---------------------|---------|
| Serialize   | 55.8 ms            | 51.0 ms             | 70.6 ms |
| Deserialize | 20.1 ms            | 19.5 ms             | 76.9 ms |
| Size        | 1009 KB            | 689 KB              | 1.7 MB  |

## chunk_size

`chunk_size` helps avoid out of memory crashes when converting between a columnar and row-based layout. Larger chunks naturally provide better compression and faster serialization, though require more memory. The crate's default 2^18 chunk size provides optimal performance while using a reasonable amount of memory. For very large structs or when running on memory-constrainted systems, you may want to use a smaller chunk size.

`Write peak` is the peak memory used while serializing, and `Read peak` is the peak while deserializing. Filtering to a small subset of the rows is significantly faster and uses less memory, as seen in the Filters benchmark.

### SmallStruct (32 bytes/row)

| Chunk size    | Serialize | Deserialize | Size   | Chunks | Write peak | Read peak |
|---------------|-----------|-------------|--------|--------|------------|-----------|
| 2^13 = 8192   | 25.9 ms   | 8.3 ms      | 157 KB | 64     | 6.0 MB     | 24.4 MB   |
| 2^14 = 16384  | 20.7 ms   | 8.2 ms      | 147 KB | 32     | 6.0 MB     | 24.6 MB   |
| 2^15 = 32768  | 18.2 ms   | 7.9 ms      | 142 KB | 16     | 6.5 MB     | 25.1 MB   |
| 2^16 = 65536  | 16.6 ms   | 7.7 ms      | 139 KB | 8      | 8.8 MB     | 26.1 MB   |
| 2^17 = 131072 | 15.7 ms   | 7.7 ms      | 138 KB | 4      | 13.4 MB    | 28.1 MB   |
| 2^18 = 262144 | 16.4 ms   | 7.7 ms      | 138 KB | 2      | 22.6 MB    | 32.1 MB   |
| 2^19 = 524288 | 16.4 ms   | 7.7 ms      | 137 KB | 1      | 27.1 MB    | 32.1 MB   |

### MediumStruct (96 bytes/row)

| Chunk size    | Serialize | Deserialize | Size   | Chunks | Write peak | Read peak |
|---------------|-----------|-------------|--------|--------|------------|-----------|
| 2^13 = 8192   | 84.3 ms   | 49.2 ms     | 683 KB | 64     | 6.0 MB     | 65.9 MB   |
| 2^14 = 16384  | 80.1 ms   | 50.3 ms     | 836 KB | 32     | 7.3 MB     | 66.8 MB   |
| 2^15 = 32768  | 80.1 ms   | 49.7 ms     | 773 KB | 16     | 9.5 MB     | 68.3 MB   |
| 2^16 = 65536  | 73.1 ms   | 48.7 ms     | 742 KB | 8      | 14.2 MB    | 71.3 MB   |
| 2^17 = 131072 | 73.5 ms   | 48.1 ms     | 726 KB | 4      | 23.6 MB    | 77.4 MB   |
| 2^18 = 262144 | 71.8 ms   | 47.0 ms     | 718 KB | 2      | 42.4 MB    | 89.6 MB   |
| 2^19 = 524288 | 71.9 ms   | 49.8 ms     | 714 KB | 1      | 79.9 MB    | 89.6 MB   |

### LargeStruct (176 bytes/row)

| Chunk size    | Serialize | Deserialize | Size    | Chunks | Write peak | Read peak |
|---------------|-----------|-------------|---------|--------|------------|-----------|
| 2^13 = 8192   | 204.1 ms  | 102.7 ms    | 1309 KB | 64     | 7.6 MB     | 146.7 MB  |
| 2^14 = 16384  | 185.3 ms  | 101.7 ms    | 1246 KB | 32     | 9.6 MB     | 148.4 MB  |
| 2^15 = 32768  | 174.8 ms  | 100.6 ms    | 1216 KB | 16     | 13.9 MB    | 151.8 MB  |
| 2^16 = 65536  | 171.0 ms  | 101.1 ms    | 1202 KB | 8      | 22.5 MB    | 158.7 MB  |
| 2^17 = 131072 | 167.4 ms  | 98.8 ms     | 1197 KB | 4      | 39.8 MB    | 172.6 MB  |
| 2^18 = 262144 | 168.4 ms  | 98.3 ms     | 1194 KB | 2      | 74.3 MB    | 200.5 MB  |
| 2^19 = 524288 | 167.6 ms  | 99.8 ms     | 1193 KB | 1      | 143.3 MB   | 200.5 MB  |

## Filters

PcoPack is designed to skip expensive deserialization work for rows that don't match the provided filter. For most data types, a filter matching 1% of rows pays just 15% of the runtime cost that otherwise would've been needed to deserialize all rows.

| Filter                                               | Time (ms) | Rows   |
|------------------------------------------------------|-----------|--------|
| no filter                                            | 39.3 ms   | 100000 |
| empty filter                                         | 39.6 ms   | 100000 |
| i64 exact (id == 50_000)                             | 6.1 ms    | 1000   |
| i64 range (50_000..=50_999)                          | 6.1 ms    | 1000   |
| i64 inclusion (100 values, 1 match)                  | 6.2 ms    | 1000   |
| i32 exact (int32_val == 50)                          | 6.1 ms    | 1000   |
| i32 range (50..=50.99)                               | 6.1 ms    | 1000   |
| i32 inclusion (100 values, 1 match)                  | 6.2 ms    | 1000   |
| i8 exact (int8_val == 50)                            | 6.0 ms    | 1000   |
| i8 range (50..=50.99)                                | 6.0 ms    | 1000   |
| i8 inclusion (100 values, 1 match)                   | 6.1 ms    | 1000   |
| u8 exact (u8_val == 50)                              | 6.0 ms    | 1000   |
| u8 range (50..=50.99)                                | 6.0 ms    | 1000   |
| u8 inclusion (100 values, 1 match)                   | 6.2 ms    | 1000   |
| f64 exact (float64_val == 50.0)                      | 6.1 ms    | 1000   |
| f64 range (50.0..=50.99)                             | 6.1 ms    | 1000   |
| f64 inclusion (100 values, 1 match)                  | 8.4 ms    | 1000   |
| f32 exact (float32_val == 50.0)                      | 6.0 ms    | 1000   |
| f32 range (50.0..=50.99)                             | 6.0 ms    | 1000   |
| f32 inclusion (100 values, 1 match)                  | 8.8 ms    | 1000   |
| f16 exact (f16_val == 50.0)                          | 6.1 ms    | 1000   |
| f16 range (50.0..=50.99)                             | 6.1 ms    | 1000   |
| f16 inclusion (100 values, 1 match)                  | 8.8 ms    | 1000   |
| string exact                                         | 7.9 ms    | 1000   |
| string inclusion (10 values)                         | 8.5 ms    | 10000  |
| bool exact (bool_val == true)                        | 19.9 ms   | 1000   |
| bool exact (bool_val == false)                       | 39.0 ms   | 99000  |
| enum exact (status == V50)                           | 6.1 ms    | 1000   |
| enum inclusion (status in 0..9)                      | 6.8 ms    | 10000  |
| option exact (option_val == 50)                      | 6.1 ms    | 1000   |
| option range (50..=50.99)                            | 6.1 ms    | 1000   |
| option inclusion (100 values, 1 match)               | 6.2 ms    | 1000   |
| vec contains (vec_val has 5000)                      | 6.2 ms    | 1000   |
| vec contains inclusion (any of 10 values)            | 6.8 ms    | 10000  |
| bytes exact (hex match)                              | 7.8 ms    | 1000   |
| map exact (has key 'key_50')                         | 14.4 ms   | 1000   |
| map inclusion (any of key_0..key_9)                  | 15.2 ms   | 10000  |
| json exact (tag_50 string)                           | 8.4 ms    | 1000   |
| uuid exact (bucket 50)                               | 6.1 ms    | 1000   |
| uuid inclusion (100 values, 1 match)                 | 6.2 ms    | 1000   |
| nested (nested.inner_id == 50_000)                   | 6.2 ms    | 1000   |
| nested (nested.inner_id range 50_000..=50_000.99)    | 6.1 ms    | 1000   |
| nested (nested.inner_name exact)                     | 8.0 ms    | 1000   |
| nested (nested.inner_name inclusion, 10 values)      | 8.6 ms    | 10000  |
| partial fields (id + string_val)                     | 0.7 ms    | 1000   |
| multi-field (id + int32_val)                         | 6.1 ms    | 1000   |
| multi-field (bytes_val + u8_val)                     | 6.7 ms    | 1000   |
| multi-field (string_val + u8_val)                    | 7.0 ms    | 1000   |
| multi-field (json_val + u8_val)                      | 7.0 ms    | 1000   |
| multi-field zero-match (id + int32_val + string_val) | 0.3 ms    | 0      |
