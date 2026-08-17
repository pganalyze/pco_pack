# Benchmarks

Note: all serialization times and sizes include compression (either Pcodec or zstd).

## Collections

| Type                            | PcoPack | msgpack | Time ratio | PcoPack | msgpack | Size ratio |
|---------------------------------|---------|---------|------------|---------|---------|------------|
| `Vec<i32> (avg 4)`              | 1.4 ms  | 2.4 ms  | 1.71x      | 52 KB   | 533 KB  | 10.1x      |
| `Vec<String> (avg 4)`           | 12.8 ms | 8.0 ms  | 0.62x      | 139 KB  | 138 KB  | 1.0x       |
| `Option<i32> (50% null)`        | 0.2 ms  | 0.4 ms  | 2.00x      | 6 KB    | 105 KB  | 15.6x      |
| `i32 (50% zero as sentinel)`    | 0.3 ms  | 0.4 ms  | 1.33x      | 19 KB   | 105 KB  | 5.5x       |
| `ByteBuf (avg 128B)`            | 7.0 ms  | 4.0 ms  | 0.57x      | 40 KB   | 39 KB   | 1.0x       |
| `serde_json::Value`             | 16.8 ms | 7.7 ms  | 0.46x      | 254 KB  | 241 KB  | 0.9x       |
| `BTreeMap<(String, i32), i32>`  | 2.0 ms  | 1.6 ms  | 0.80x      | 16 KB   | 119 KB  | 7.5x       |
| `BTreeMap<(SmolStr, i32), i32>` | 1.2 ms  | 4.9 ms  | 4.08x      | 16 KB   | 119 KB  | 7.5x       |
| `HashMap<(String, i32), i32>`   | 2.4 ms  | 2.0 ms  | 0.83x      | 16 KB   | 159 KB  | 9.9x       |
| `HashMap<(SmolStr, i32), i32>`  | 1.5 ms  | 6.0 ms  | 4.00x      | 16 KB   | 159 KB  | 10.0x      |

- `Vec<i32>` is 2x faster and 10x smaller because each element gets Pcodec compression
- `Option<i32>` is 2x faster and 16x smaller by tracking `Some` as indexes, then unwrapping the inner values so Pcodec compression can be used
- `ByteBuf` and `serde_json::Value` are serialized with msgpack and compressed with zstd in both formats, though PcoPack's wrapper adds some overhead
- `BTreeMap`/`HashMap` with String keys are 8-10x smaller because keys and values are stored as separately compressed columns
  - Note: SmolStr keys are faster than String keys because their small stack allocation is faster to serialize and hash

## Numbers

| Type                     | PcoPack | msgpack | Time ratio | PcoPack | msgpack | Size ratio |
|--------------------------|---------|---------|------------|---------|---------|------------|
| `i8 (0-999)`             | 0.4 ms  | 0.4 ms  | 1.00x      | 20 KB   | 5 KB    | 0.3x       |
| `i16 (0-999)`            | 0.3 ms  | 0.4 ms  | 1.33x      | 20 KB   | 11 KB   | 0.5x       |
| `u8 (0-255)`             | 0.1 ms  | 0.3 ms  | 3.00x      | 23 B    | 394 B   | 17.1x      |
| `u16 (0-999)`            | 0.2 ms  | 0.3 ms  | 1.50x      | 177 B   | 1 KB    | 10.3x      |
| `i32 (full)`             | 0.2 ms  | 0.7 ms  | 3.50x      | 204 KB  | 448 KB  | 2.2x       |
| `i64 (full)`             | 0.3 ms  | 1.0 ms  | 3.33x      | 389 KB  | 849 KB  | 2.2x       |
| `u32 (full)`             | 0.2 ms  | 0.6 ms  | 3.00x      | 372 KB  | 437 KB  | 1.2x       |
| `f32 (normal)`           | 0.3 ms  | 0.6 ms  | 2.00x      | 26 KB   | 373 KB  | 13.9x      |
| `f64 (normal)`           | 0.3 ms  | 0.9 ms  | 3.00x      | 34 KB   | 765 KB  | 22.0x      |
| `i64 (80% zero, sparse)` | 0.3 ms  | 0.3 ms  | 1.00x      | 65 KB   | 63 KB   | 1.0x       |

PcoPack is faster and smaller across most types, though msgpack's bit packing and zstd's compression of zero-byte sequences can outperform PcoPack with small integer ranges.

## Others

| Type               | PcoPack | msgpack | Time ratio | PcoPack | msgpack | Size ratio |
|--------------------|---------|---------|------------|---------|---------|------------|
| `bool (50% true)`  | 0.5 ms  | 0.3 ms  | 0.60x      | 24 B    | 24 B    | 1.0x       |
| `String`           | 5.6 ms  | 3.0 ms  | 0.54x      | 104 KB  | 94 KB   | 0.9x       |
| `SmolStr`          | 2.7 ms  | 2.2 ms  | 0.81x      | 104 KB  | 94 KB   | 0.9x       |
| `Enum (simple)`    | 0.2 ms  | 1.4 ms  | 7.00x      | 19 KB   | 27 KB   | 1.4x       |
| `Enum (complex)`   | 1.4 ms  | 2.2 ms  | 1.57x      | 33 KB   | 129 KB  | 3.8x       |
| `chrono::DateTime` | 0.6 ms  | 7.7 ms  | 12.83x     | 22 KB   | 109 KB  | 4.8x       |
| `uuid::Uuid`       | 1.6 ms  | 1.7 ms  | 1.06x      | 367 KB  | 367 KB  | 1.0x       |

PcoPack significantly outperforms when storing enums and timestamps because of the efficient internal layout and Pcodec compression of numbers.

`bool`, `String`, and `SmolStr` are all internally compressed with msgpack, so PcoPack has the same compressed size but slower serialization in order to support lazy deserialization and fallback serialization formats.

## Structs

| Metric                          | PcoPack  | columnar | serde_columnar | msgpack  |
|---------------------------------|----------|----------|----------------|----------|
| Serialize                       | 103.9 ms | 68.1 ms  | 97.9 ms        | 99.8 ms  |
| Deserialize                     | 75.4 ms  | 23.3 ms  | 76.1 ms        | 102.4 ms |
| Size                            | 1.4 MB   | 7.4 MB   | 5.4 MB         | 8.2 MB   |
| Filter account_id (20% of rows) | 17.5 ms  | 24.2 ms  | 71.3 ms        | 109.9 ms |
| Filter color + score (1 row)    | 6.0 ms   | 25.1 ms  | 71.1 ms        | 106.8 ms |

- `PcoPack` compressed size is 3-5x smaller than all others. Filtering is 4-28x faster via lazy decompression and `index`/`timestamp` indexing
- `columnar` has the fastest roundtrip serialization, but the compressed size is 3x larger than PcoPack
- `serde_columnar` has better compressed size than `columnar` because of per-field encoding (which users must manually set)
- msgpack has the worst compressed size and roundtrip time when including filtering. This uses a traditional row-based layout instead of a columnar layout, highlighting why a columnar layout is beneficial

## Timeline vs DateTime

| Uniqueness | Timeline | DateTime | Time ratio | Timeline | DateTime | Size ratio |
|------------|----------|----------|------------|----------|----------|------------|
| 10%        | 1.1 ms   | 3.7 ms   | 3.4x       | 938 B    | 50 KB    | 55.1x      |
| 20%        | 1.5 ms   | 3.9 ms   | 2.6x       | 1 KB     | 63 KB    | 42.7x      |
| 50%        | 3.9 ms   | 3.5 ms   | 0.9x       | 3 KB     | 81 KB    | 21.1x      |
| 80%        | 6.5 ms   | 3.3 ms   | 0.5x       | 6 KB     | 47 KB    | 7.8x       |
| 90%        | 7.4 ms   | 3.0 ms   | 0.4x       | 6 KB     | 27 KB    | 4.1x       |

`Timeline` is a non-contiguous time range type that allows PcoPack to store a single row when all other fields in the struct are identical, significantly reducing time and size as long as your data has >50% duplicates. Smaller sizes can even be seen at 90% uniqueness, though the extra serialization time may not be worth it.

## float_round

| Metric      | PcoPack (no round) | PcoPack (round=2) | msgpack |
|-------------|--------------------|-------------------|---------|
| Serialize   | 80.7 ms            | 86.6 ms           | 50.0 ms |
| Deserialize | 48.8 ms            | 49.1 ms           | 50.5 ms |
| Size        | 2.5 MB             | 983 KB            | 7.6 MB  |

`#[pco_pack(float_round = N)]` significantly reduces compressed size and slightly worsens serialization time.

## time_round

| Metric      | PcoPack (no round) | PcoPack (round=60s) | msgpack |
|-------------|--------------------|---------------------|---------|
| Serialize   | 58.7 ms            | 53.0 ms             | 76.8 ms |
| Deserialize | 21.3 ms            | 20.3 ms             | 82.2 ms |
| Size        | 1009 KB            | 689 KB              | 1.7 MB  |

`#[pco_pack(float_round = Duration::seconds(60))]` moderately reduces compressed size and slightly improves serialization time.

## chunk_size

### SmallStruct (32 bytes/row)

| Chunk size    | Serialize | Deserialize | Size  | Chunks | Memory per chunk |
|---------------|-----------|-------------|-------|--------|------------------|
| 2^13 = 8192   | 13.2 ms   | 4.5 ms      | 78 KB | 32     | 256 KB           |
| 2^14 = 16384  | 10.3 ms   | 4.4 ms      | 73 KB | 16     | 512 KB           |
| 2^15 = 32768  | 9.0 ms    | 4.4 ms      | 71 KB | 8      | 1.0 MB           |
| 2^16 = 65536  | 8.2 ms    | 4.2 ms      | 69 KB | 4      | 2.0 MB           |
| 2^17 = 131072 | 7.8 ms    | 4.2 ms      | 69 KB | 2      | 4.0 MB           |

### MediumStruct (80 bytes/row)

| Chunk size    | Serialize | Deserialize | Size   | Chunks | Memory per chunk |
|---------------|-----------|-------------|--------|--------|------------------|
| 2^13 = 8192   | 40.6 ms   | 24.5 ms     | 342 KB | 32     | 800 KB           |
| 2^14 = 16384  | 39.1 ms   | 24.3 ms     | 419 KB | 16     | 1.6 MB           |
| 2^15 = 32768  | 36.8 ms   | 23.8 ms     | 387 KB | 8      | 3.1 MB           |
| 2^16 = 65536  | 35.5 ms   | 23.7 ms     | 371 KB | 4      | 6.2 MB           |
| 2^17 = 131072 | 34.9 ms   | 23.5 ms     | 364 KB | 2      | 12.5 MB          |

### LargeStruct (176 bytes/row)

| Chunk size    | Serialize | Deserialize | Size   | Chunks | Memory per chunk |
|---------------|-----------|-------------|--------|--------|------------------|
| 2^13 = 8192   | 99.7 ms   | 52.1 ms     | 655 KB | 32     | 1.8 MB           |
| 2^14 = 16384  | 91.2 ms   | 49.9 ms     | 624 KB | 16     | 3.6 MB           |
| 2^15 = 32768  | 85.2 ms   | 50.3 ms     | 610 KB | 8      | 7.1 MB           |
| 2^16 = 65536  | 84.2 ms   | 49.8 ms     | 603 KB | 4      | 14.2 MB          |
| 2^17 = 131072 | 81.4 ms   | 50.7 ms     | 602 KB | 2      | 28.5 MB          |

## Filters

| Filter                                               | Time (ms) | Rows   |
|------------------------------------------------------|-----------|--------|
| no filter                                            | 40.1 ms   | 100000 |
| empty filter                                         | 40.7 ms   | 100000 |
| i64 exact (id == 50_000)                             | 6.2 ms    | 1000   |
| i64 range (50_000..=50_999)                          | 6.2 ms    | 1000   |
| i64 inclusion (100 values, 1 match)                  | 6.8 ms    | 1000   |
| i32 exact (int32_val == 50)                          | 6.3 ms    | 1000   |
| i32 range (50..=50.99)                               | 6.2 ms    | 1000   |
| i32 inclusion (100 values, 1 match)                  | 6.7 ms    | 1000   |
| i8 exact (int8_val == 50)                            | 6.1 ms    | 1000   |
| i8 range (50..=50.99)                                | 6.1 ms    | 1000   |
| i8 inclusion (100 values, 1 match)                   | 6.8 ms    | 1000   |
| u8 exact (u8_val == 50)                              | 6.2 ms    | 1000   |
| u8 range (50..=50.99)                                | 6.3 ms    | 1000   |
| u8 inclusion (100 values, 1 match)                   | 6.8 ms    | 1000   |
| f64 exact (float64_val == 50.0)                      | 6.2 ms    | 1000   |
| f64 range (50.0..=50.99)                             | 6.3 ms    | 1000   |
| f64 inclusion (100 values, 1 match)                  | 8.6 ms    | 1000   |
| f32 exact (float32_val == 50.0)                      | 6.4 ms    | 1000   |
| f32 range (50.0..=50.99)                             | 6.3 ms    | 1000   |
| f32 inclusion (100 values, 1 match)                  | 8.9 ms    | 1000   |
| f16 exact (f16_val == 50.0)                          | 6.3 ms    | 1000   |
| f16 range (50.0..=50.99)                             | 6.3 ms    | 1000   |
| f16 inclusion (100 values, 1 match)                  | 8.9 ms    | 1000   |
| string exact                                         | 8.4 ms    | 1000   |
| string inclusion (10 values)                         | 9.0 ms    | 10000  |
| bool exact (bool_val == true)                        | 21.1 ms   | 1000   |
| bool exact (bool_val == false)                       | 41.7 ms   | 99000  |
| enum exact (status == V50)                           | 6.3 ms    | 1000   |
| enum inclusion (status in 0..9)                      | 7.0 ms    | 10000  |
| option exact (option_val == 50)                      | 6.3 ms    | 1000   |
| option range (50..=50.99)                            | 6.3 ms    | 1000   |
| option inclusion (100 values, 1 match)               | 6.9 ms    | 1000   |
| vec contains (vec_val has 5000)                      | 6.4 ms    | 1000   |
| vec contains inclusion (any of 10 values)            | 7.1 ms    | 10000  |
| bytes exact (hex match)                              | 8.2 ms    | 1000   |
| map exact (has key 'key_50')                         | 14.8 ms   | 1000   |
| map inclusion (any of key_0..key_9)                  | 15.7 ms   | 10000  |
| json exact (tag_50 string)                           | 8.5 ms    | 1000   |
| uuid exact (nil)                                     | 3.9 ms    | 1000   |
| uuid inclusion (100 values, 1 match)                 | 6.6 ms    | 1000   |
| nested (nested.inner_id == 50_000)                   | 6.2 ms    | 1000   |
| nested (nested.inner_id range 50_000..=50_000.99)    | 6.1 ms    | 1000   |
| nested (nested.inner_name exact)                     | 8.1 ms    | 1000   |
| nested (nested.inner_name inclusion, 10 values)      | 9.1 ms    | 10000  |
| partial fields (id + string_val)                     | 0.8 ms    | 1000   |
| multi-field (id + int32_val)                         | 6.2 ms    | 1000   |
| multi-field (bytes_val + u8_val)                     | 6.9 ms    | 1000   |
| multi-field (string_val + u8_val)                    | 7.1 ms    | 1000   |
| multi-field (json_val + u8_val)                      | 6.9 ms    | 1000   |
| multi-field zero-match (id + int32_val + string_val) | 0.4 ms    | 0      |
