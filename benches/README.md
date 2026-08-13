# Benchmarks

Note: all serialization times and sizes include compression (either Pcodec or zstd).

## Collections

| Type                            | PcoPack | msgpack | Time ratio | PcoPack | msgpack | Size ratio |
|---------------------------------|---------|---------|------------|---------|---------|------------|
| `Vec<i32> (avg 4)`              | 1.6 ms  | 2.5 ms  | 1.56x      | 52 KB   | 533 KB  | 10.1x      |
| `Vec<String> (avg 4)`           | 13.4 ms | 8.1 ms  | 0.60x      | 139 KB  | 138 KB  | 1.0x       |
| `Option<i32> (50% null)`        | 0.3 ms  | 0.4 ms  | 1.33x      | 6 KB    | 105 KB  | 15.6x      |
| `i32 (50% zero as sentinel)`    | 0.3 ms  | 0.4 ms  | 1.33x      | 19 KB   | 105 KB  | 5.5x       |
| `ByteBuf (avg 128B)`            | 7.2 ms  | 4.1 ms  | 0.57x      | 40 KB   | 39 KB   | 1.0x       |
| `serde_json::Value`             | 17.4 ms | 7.9 ms  | 0.45x      | 254 KB  | 241 KB  | 0.9x       |
| `BTreeMap<(String, i32), i32>`  | 2.1 ms  | 1.7 ms  | 0.81x      | 16 KB   | 119 KB  | 7.5x       |
| `BTreeMap<(SmolStr, i32), i32>` | 1.3 ms  | 5.0 ms  | 3.85x      | 16 KB   | 119 KB  | 7.5x       |
| `HashMap<(String, i32), i32>`   | 2.5 ms  | 2.1 ms  | 0.84x      | 16 KB   | 159 KB  | 10.0x      |
| `HashMap<(SmolStr, i32), i32>`  | 1.5 ms  | 5.9 ms  | 3.93x      | 16 KB   | 159 KB  | 10.0x      |

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
| `i32 (full)`             | 0.2 ms  | 0.9 ms  | 4.50x      | 204 KB  | 448 KB  | 2.2x       |
| `i64 (full)`             | 0.3 ms  | 1.0 ms  | 3.33x      | 389 KB  | 849 KB  | 2.2x       |
| `u32 (full)`             | 0.2 ms  | 0.7 ms  | 3.50x      | 372 KB  | 437 KB  | 1.2x       |
| `f32 (normal)`           | 0.3 ms  | 0.6 ms  | 2.00x      | 26 KB   | 373 KB  | 13.9x      |
| `f64 (normal)`           | 0.3 ms  | 1.0 ms  | 3.33x      | 34 KB   | 765 KB  | 22.0x      |
| `i64 (80% zero, sparse)` | 0.3 ms  | 0.3 ms  | 1.00x      | 65 KB   | 63 KB   | 1.0x       |

PcoPack is faster and smaller across most types, though msgpack's bit packing and zstd's compression of zero-byte sequences can outperform PcoPack with small integer ranges.

## Others

| Type               | PcoPack | msgpack | Time ratio | PcoPack | msgpack | Size ratio |
|--------------------|---------|---------|------------|---------|---------|------------|
| `bool (50% true)`  | 0.6 ms  | 0.3 ms  | 0.50x      | 24 B    | 24 B    | 1.0x       |
| `String`           | 5.8 ms  | 3.2 ms  | 0.55x      | 104 KB  | 94 KB   | 0.9x       |
| `SmolStr`          | 2.9 ms  | 2.4 ms  | 0.83x      | 104 KB  | 94 KB   | 0.9x       |
| `Enum (simple)`    | 0.2 ms  | 1.5 ms  | 7.50x      | 19 KB   | 27 KB   | 1.4x       |
| `Enum (complex)`   | 1.4 ms  | 2.3 ms  | 1.64x      | 33 KB   | 129 KB  | 3.8x       |
| `chrono::DateTime` | 0.7 ms  | 7.8 ms  | 11.14x     | 22 KB   | 109 KB  | 4.8x       |
| `uuid::Uuid`       | 1.6 ms  | 1.8 ms  | 1.12x      | 367 KB  | 367 KB  | 1.0x       |

PcoPack significantly outperforms when storing enums and timestamps because of the efficient internal layout and Pcodec compression of numbers.

`bool`, `String`, and `SmolStr` are all internally compressed with msgpack, so PcoPack has the same compressed size but slower serialization in order to support lazy deserialization and fallback serialization formats.

## Structs

| Metric                          | PcoPack  | columnar | serde_columnar | msgpack  |
|---------------------------------|----------|----------|----------------|----------|
| Serialize                       | 103.7 ms | 69.0 ms  | 99.7 ms        | 102.9 ms |
| Deserialize                     | 75.4 ms  | 23.5 ms  | 83.0 ms        | 112.1 ms |
| Size                            | 1.4 MB   | 7.4 MB   | 5.4 MB         | 8.2 MB   |
| Filter account_id (20% of rows) | 18.1 ms  | 28.1 ms  | 76.1 ms        | 115.4 ms |
| Filter color + score (1 row)    | 6.1 ms   | 28.3 ms  | 76.9 ms        | 109.3 ms |

- `PcoPack` compressed size is 3-5x smaller than all others. Filtering is 4-28x faster via lazy decompression and `index`/`timestamp` indexing
- `columnar` has the fastest roundtrip serialization, but the compressed size is 3x larger than PcoPack
- `serde_columnar` has better compressed size than `columnar` because of per-field encoding (which users must manually set)
- msgpack has the worst compressed size and roundtrip time when including filtering. This uses a traditional row-based layout instead of a columnar layout, highlighting why a columnar layout is beneficial

## Timeline vs DateTime
| Uniqueness | Timeline | DateTime | Time ratio | Timeline | DateTime | Size ratio |
|------------|----------|----------|------------|----------|----------|------------|
| 10%        | 1.1 ms   | 3.8 ms   | 3.5x       | 938 B    | 50 KB    | 55.1x      |
| 20%        | 1.5 ms   | 3.8 ms   | 2.5x       | 1 KB     | 63 KB    | 42.7x      |
| 50%        | 3.9 ms   | 3.7 ms   | 0.9x       | 3 KB     | 81 KB    | 21.1x      |
| 80%        | 6.9 ms   | 3.4 ms   | 0.5x       | 6 KB     | 47 KB    | 7.8x       |
| 90%        | 7.4 ms   | 3.1 ms   | 0.4x       | 6 KB     | 27 KB    | 4.1x       |

`Timeline` is a non-contiguous time range type that allows PcoPack to store a single row when all other fields in the struct are identical, significantly reducing time and size as long as your data has >50% duplicates. Smaller sizes can even be seen at 90% uniqueness, though the extra serialization time may not be worth it.

## float_round

| Metric      | PcoPack (no round) | PcoPack (round=2) | msgpack |
|-------------|--------------------|-------------------|---------|
| Serialize   | 83.1 ms            | 88.8 ms           | 50.9 ms |
| Deserialize | 50.5 ms            | 50.4 ms           | 53.3 ms |
| Size        | 2.5 MB             | 983 KB            | 7.6 MB  |

`#[pco_pack(float_round = N)]` significantly reduces compressed size and slightly worsens serialization time.

## time_round

| Metric      | PcoPack (no round) | PcoPack (round=60s) | msgpack |
|-------------|--------------------|---------------------|---------|
| Serialize   | 60.6 ms            | 55.2 ms             | 79.1 ms |
| Deserialize | 21.7 ms            | 20.9 ms             | 84.8 ms |
| Size        | 1009 KB            | 689 KB              | 1.7 MB  |

`#[pco_pack(float_round = Duration::seconds(60))]` moderately reduces compressed size and slightly improves serialization time.

## chunk_size

### SmallStruct (32 bytes/row)

| Chunk size    | Serialize | Deserialize | Size  | Chunks | Memory per chunk |
|---------------|-----------|-------------|-------|--------|------------------|
| 2^13 = 8192   | 13.3 ms   | 4.5 ms      | 78 KB | 32     | 256 KB           |
| 2^14 = 16384  | 10.4 ms   | 4.5 ms      | 73 KB | 16     | 512 KB           |
| 2^15 = 32768  | 9.1 ms    | 4.4 ms      | 71 KB | 8      | 1.0 MB           |
| 2^16 = 65536  | 8.4 ms    | 4.3 ms      | 69 KB | 4      | 2.0 MB           |
| 2^17 = 131072 | 8.0 ms    | 4.3 ms      | 69 KB | 2      | 4.0 MB           |

### MediumStruct (80 bytes/row)

| Chunk size    | Serialize | Deserialize | Size   | Chunks | Memory per chunk |
|---------------|-----------|-------------|--------|--------|------------------|
| 2^13 = 8192   | 40.6 ms   | 25.1 ms     | 342 KB | 32     | 800 KB           |
| 2^14 = 16384  | 39.5 ms   | 24.5 ms     | 419 KB | 16     | 1.6 MB           |
| 2^15 = 32768  | 37.0 ms   | 24.4 ms     | 387 KB | 8      | 3.1 MB           |
| 2^16 = 65536  | 35.3 ms   | 23.9 ms     | 371 KB | 4      | 6.2 MB           |
| 2^17 = 131072 | 34.9 ms   | 23.8 ms     | 364 KB | 2      | 12.5 MB          |

### LargeStruct (176 bytes/row)

| Chunk size    | Serialize | Deserialize | Size   | Chunks | Memory per chunk |
|---------------|-----------|-------------|--------|--------|------------------|
| 2^13 = 8192   | 100.3 ms  | 52.8 ms     | 655 KB | 32     | 1.8 MB           |
| 2^14 = 16384  | 91.7 ms   | 51.7 ms     | 624 KB | 16     | 3.6 MB           |
| 2^15 = 32768  | 85.9 ms   | 51.4 ms     | 610 KB | 8      | 7.1 MB           |
| 2^16 = 65536  | 83.1 ms   | 52.2 ms     | 603 KB | 4      | 14.2 MB          |
| 2^17 = 131072 | 81.3 ms   | 52.2 ms     | 602 KB | 2      | 28.5 MB          |

## Filters

| Filter                                            | Time (ms) | Rows   |
|---------------------------------------------------|-----------|--------|
| no filter                                         | 41.5 ms   | 100000 |
| empty filter                                      | 41.3 ms   | 100000 |
| i64 exact (id == 50_000)                          | 6.5 ms    | 1000   |
| i64 range (50_000..=50_999)                       | 6.4 ms    | 1000   |
| i64 inclusion (100 values, 1 match)               | 7.0 ms    | 1000   |
| i32 exact (int32_val == 50)                       | 6.4 ms    | 1000   |
| i32 range (50..=50.99)                            | 6.4 ms    | 1000   |
| i32 inclusion (100 values, 1 match)               | 7.0 ms    | 1000   |
| i8 exact (int8_val == 50)                         | 6.4 ms    | 1000   |
| i8 range (50..=50.99)                             | 6.4 ms    | 1000   |
| i8 inclusion (100 values, 1 match)                | 7.1 ms    | 1000   |
| u8 exact (u8_val == 50)                           | 6.4 ms    | 1000   |
| u8 range (50..=50.99)                             | 6.5 ms    | 1000   |
| u8 inclusion (100 values, 1 match)                | 7.0 ms    | 1000   |
| f64 exact (float64_val == 50.0)                   | 6.5 ms    | 1000   |
| f64 range (50.0..=50.99)                          | 6.5 ms    | 1000   |
| f64 inclusion (100 values, 1 match)               | 9.5 ms    | 1000   |
| f32 exact (float32_val == 50.0)                   | 6.4 ms    | 1000   |
| f32 range (50.0..=50.99)                          | 6.5 ms    | 1000   |
| f32 inclusion (100 values, 1 match)               | 8.9 ms    | 1000   |
| f16 exact (f16_val == 50.0)                       | 6.6 ms    | 1000   |
| f16 range (50.0..=50.99)                          | 6.5 ms    | 1000   |
| f16 inclusion (100 values, 1 match)               | 8.8 ms    | 1000   |
| string exact                                      | 8.3 ms    | 1000   |
| string inclusion (10 values)                      | 9.1 ms    | 10000  |
| bool exact (bool_val == true)                     | 21.3 ms   | 1000   |
| bool exact (bool_val == false)                    | 40.8 ms   | 99000  |
| enum exact (status == V50)                        | 6.3 ms    | 1000   |
| enum inclusion (status in 0..9)                   | 7.1 ms    | 10000  |
| option exact (option_val == 50)                   | 6.4 ms    | 1000   |
| option range (50..=50.99)                         | 6.4 ms    | 1000   |
| option inclusion (100 values, 1 match)            | 7.0 ms    | 1000   |
| vec contains (vec_val has 5000)                   | 6.5 ms    | 1000   |
| vec contains inclusion (any of 10 values)         | 7.2 ms    | 10000  |
| bytes exact (hex match)                           | 8.4 ms    | 1000   |
| map exact (has key 'key_50')                      | 14.9 ms   | 1000   |
| map inclusion (any of key_0..key_9)               | 15.9 ms   | 10000  |
| json exact (tag_50 string)                        | 8.6 ms    | 1000   |
| uuid exact (nil)                                  | 4.1 ms    | 1000   |
| uuid inclusion (100 values, 1 match)              | 6.5 ms    | 1000   |
| nested (nested.inner_id == 50_000)                | 6.3 ms    | 1000   |
| nested (nested.inner_id range 50_000..=50_000.99) | 6.3 ms    | 1000   |
| nested (nested.inner_name exact)                  | 8.3 ms    | 1000   |
| nested (nested.inner_name inclusion, 10 values)   | 9.1 ms    | 10000  |
| partial fields (id + string_val)                  | 0.8 ms    | 1000   |
| multi-field (id + int32_val)                      | 6.4 ms    | 1000   |
