# Benchmarks

All serialization times and sizes include compression (either Pcodec or zstd). These numbers are captured on an M5 Max MBP with fans set to max. There is a lot of run-to-run variance; [contributions are welcome](https://github.com/pganalyze/pco_pack/issues/3) to help smooth out the data.

## Types

PcoPack vs standalone Pcodec vs msgpack+zstd vs Vortex, per field type and distribution. All rows run at PcoPack's default 262k-row chunk size. Pcodec uses zero-config defaults plus 8-bit types. msgpack+zstd uses the same chunking. Vortex is one compact btrblocks file per column, with "-" where the pipeline can't express the type: complex enums, `serde_json::Value`, maps.

Every Numbers row is a synthetic shape that isolates exactly one axis: entropy floor, type-agnosticism, delta, skew, sparsity and placement, alphabet and dwell length, distinct count, multiset placement, recurrence distance, chunk fixed costs. Real-column bytes live in the [Postgres](#postgres) section.

The `pco` column appears only where Pcodec compresses without transformation (plain numbers, bool -> u8, enum discriminants). PcoPack's structural decompositions (`Option` -> present+values, `Vec` -> lengths+values, dynamic JSON) have no standalone-Pcodec counterpart.

Where each codec wins:
- Frequency/delta redundancy (counters, skew, i.i.d. pools): pco/PcoPack up to ~70% smaller than msgpack
- Sequential redundancy (dwell runs, clustered zeros, grouped placement): msgpack+zstd ties or beats pco; long dwells collapse every codec to hundreds of bytes
- Placement decides: one multiset, grouped vs shuffled, 5-6 KB vs 336-503 KB
- Incompressible (uniform full-width): pco/PcoPack/Vortex tie within ~1%; msgpack pays its framing tax

### Numbers

| Type                                           | pco    | PcoPack | msgpack | Vortex  | pco     | PcoPack | msgpack | Vortex  |
|------------------------------------------------|--------|---------|---------|---------|---------|---------|---------|---------|
| `u8 (uniform, full width)`                     | 1.2 ms | 1.8 ms  | 1.6 ms  | 2.5 ms  | 256 KB  | 256 KB  | 302 KB  | 258 KB  |
| `i16 (uniform, full width)`                    | 1.6 ms | 1.7 ms  | 1.6 ms  | 3.6 ms  | 512 KB  | 512 KB  | 634 KB  | 514 KB  |
| `i32 (uniform, full width)`                    | 1.7 ms | 1.7 ms  | 2.0 ms  | 5.2 ms  | 1024 KB | 1024 KB | 1175 KB | 1026 KB |
| `i64 (uniform, full width)`                    | 2.0 ms | 2.1 ms  | 3.0 ms  | 5.6 ms  | 2.0 MB  | 2.0 MB  | 2.2 MB  | 2.0 MB  |
| `i16 (uniform 0..1000)`                        | 1.5 ms | 1.5 ms  | 3.3 ms  | 3.1 ms  | 320 KB  | 320 KB  | 419 KB  | 322 KB  |
| `i32 (uniform 0..1000)`                        | 1.5 ms | 1.6 ms  | 3.3 ms  | 3.2 ms  | 320 KB  | 320 KB  | 419 KB  | 322 KB  |
| `i64 (counter, monotonic)`                     | 2.2 ms | 2.2 ms  | 3.3 ms  | 7.2 ms  | 181 KB  | 181 KB  | 346 KB  | 185 KB  |
| `i16 (counter, cyclic ~20% range)`             | 2.0 ms | 2.0 ms  | 3.0 ms  | 5.4 ms  | 183 KB  | 183 KB  | 431 KB  | 185 KB  |
| `u8 (counter, cyclic 8-bit)`                   | 1.9 ms | 1.9 ms  | 1.6 ms  | 2.5 ms  | 181 KB  | 181 KB  | 271 KB  | 258 KB  |
| `i64 (right-skew, sparse variant)`             | 1.9 ms | 1.9 ms  | 1.6 ms  | 5.2 ms  | 138 KB  | 138 KB  | 155 KB  | 141 KB  |
| `i64 (right-skew)`                             | 1.9 ms | 2.0 ms  | 1.6 ms  | 5.2 ms  | 140 KB  | 140 KB  | 158 KB  | 143 KB  |
| `i64 (93% zeros, scattered)`                   | 1.4 ms | 1.4 ms  | 1.1 ms  | 5.3 ms  | 25 KB   | 25 KB   | 39 KB   | 28 KB   |
| `i64 (93% zeros, clustered windows)`           | 1.4 ms | 1.4 ms  | 0.9 ms  | 4.4 ms  | 26 KB   | 26 KB   | 15 KB   | 17 KB   |
| `u8 (tiny alphabet i.i.d., d=4)`               | 0.9 ms | 0.9 ms  | 0.7 ms  | 3.2 ms  | 35 KB   | 35 KB   | 55 KB   | 66 KB   |
| `u8 (dwell sweep, d=3, run~16)`                | 1.1 ms | 1.1 ms  | 0.5 ms  | 3.3 ms  | 14 KB   | 14 KB   | 12 KB   | 14 KB   |
| `u8 (dwell sweep, d=3, run~256)`               | 1.2 ms | 1.2 ms  | 0.3 ms  | 3.5 ms  | 1 KB    | 1 KB    | 1 KB    | 4 KB    |
| `u8 (dwell sweep, d=3, run~4096)`              | 1.2 ms | 1.2 ms  | 0.3 ms  | 3.2 ms  | 235 B   | 235 B   | 274 B   | 3 KB    |
| `i64 (d-sweep, d=4)`                           | 1.4 ms | 1.4 ms  | 3.8 ms  | 5.3 ms  | 64 KB   | 64 KB   | 220 KB  | 67 KB   |
| `i64 (d-sweep, d=35)`                          | 2.2 ms | 2.3 ms  | 4.4 ms  | 5.1 ms  | 164 KB  | 164 KB  | 375 KB  | 167 KB  |
| `i64 (d-sweep, d=86)`                          | 2.5 ms | 2.5 ms  | 4.4 ms  | 5.3 ms  | 206 KB  | 206 KB  | 422 KB  | 209 KB  |
| `i64 (d-sweep, d=1024)`                        | 7.5 ms | 7.5 ms  | 5.4 ms  | 4.1 ms  | 375 KB  | 375 KB  | 544 KB  | 330 KB  |
| `i64 (d-sweep, d=8191)`                        | 7.8 ms | 7.9 ms  | 7.0 ms  | 3.5 ms  | 559 KB  | 559 KB  | 681 KB  | 2.0 MB  |
| `i64 (d-sweep, d=8193)`                        | 7.8 ms | 7.9 ms  | 7.0 ms  | 3.4 ms  | 558 KB  | 558 KB  | 681 KB  | 2.0 MB  |
| `i64 (d-sweep, d=32768)`                       | 7.8 ms | 7.8 ms  | 8.0 ms  | 4.0 ms  | 1194 KB | 1194 KB | 1195 KB | 2.0 MB  |
| `i64 (placement pair, d=574 grouped)`          | 2.0 ms | 2.0 ms  | 1.1 ms  | 3.4 ms  | 5 KB    | 5 KB    | 6 KB    | 8 KB    |
| `i64 (placement pair, d=574 shuffled)`         | 7.5 ms | 7.4 ms  | 4.6 ms  | 6.3 ms  | 336 KB  | 336 KB  | 503 KB  | 278 KB  |
| `i64 (recurrence pair, d=4096 local-block)`    | 6.9 ms | 7.0 ms  | 4.4 ms  | 5.9 ms  | 174 KB  | 174 KB  | 315 KB  | 154 KB  |
| `i64 (recurrence pair, d=4096 full-cycle)`     | 7.3 ms | 7.4 ms  | 1.5 ms  | 5.0 ms  | 40 KB   | 40 KB   | 35 KB   | 35 KB   |
| `i64 (d=64 pool, N=2k tail chunk)`             | 0.1 ms | 0.1 ms  | 0.0 ms  | 0.4 ms  | 2 KB    | 2 KB    | 3 KB    | 4 KB    |
| `i64 (d=64 pool, full writer chunk)`           | 2.3 ms | 2.3 ms  | 4.5 ms  | 3.6 ms  | 192 KB  | 192 KB  | 407 KB  | 195 KB  |

### Booleans

| Type                               | pco    | PcoPack | msgpack | Vortex | pco   | PcoPack | msgpack | Vortex |
|------------------------------------|--------|---------|---------|--------|-------|---------|---------|--------|
| `bool (i.i.d. 50% true)`           | 0.1 ms | 0.3 ms  | 0.3 ms  | 1.2 ms | 12 KB | 18 KB   | 18 KB   | 14 KB  |
| `bool (i.i.d. 90% true)`           | 0.3 ms | 0.3 ms  | 0.2 ms  | 0.9 ms | 5 KB  | 12 KB   | 12 KB   | 14 KB  |
| `bool (clustered runs)`            | 0.4 ms | 0.2 ms  | 0.1 ms  | 0.9 ms | 1 KB  | 1 KB    | 1 KB    | 14 KB  |
| `bool (alternating, period 2)`     | 0.1 ms | 0.1 ms  | 0.1 ms  | 0.9 ms | 12 KB | 24 B    | 24 B    | 14 KB  |
| `bool (i.i.d. 5% true)`            | 0.3 ms | 0.2 ms  | 0.2 ms  | 0.9 ms | 3 KB  | 7 KB    | 7 KB    | 14 KB  |
| `bool (clustered, ~20% true runs)` | 0.4 ms | 0.2 ms  | 0.2 ms  | 0.9 ms | 3 KB  | 6 KB    | 6 KB    | 14 KB  |

### Option<i32> vs sentinel

| Type                                 | PcoPack | msgpack | Vortex | PcoPack | msgpack | Vortex |
|--------------------------------------|---------|---------|--------|---------|---------|--------|
| `Option<i32> (80% null, clustered)`  | 0.3 ms  | 0.2 ms  | 1.4 ms | 10 KB   | 12 KB   | 26 KB  |
| `i32 (80% zero sentinel, clustered)` | 0.6 ms  | 0.3 ms  | 1.8 ms | 20 KB   | 12 KB   | 14 KB  |
| `Option<i32> (80% null, scattered)`  | 0.5 ms  | 0.4 ms  | 2.1 ms | 24 KB   | 33 KB   | 29 KB  |
| `i32 (80% zero sentinel, scattered)` | 0.6 ms  | 0.4 ms  | 1.8 ms | 24 KB   | 33 KB   | 26 KB  |
| `Option<i32> (90% null, clustered)`  | 0.2 ms  | 0.2 ms  | 1.3 ms | 5 KB    | 6 KB    | 20 KB  |
| `i32 (90% zero sentinel, clustered)` | 0.6 ms  | 0.3 ms  | 1.6 ms | 11 KB   | 6 KB    | 8 KB   |
| `Option<i32> (90% null, scattered)`  | 0.3 ms  | 0.3 ms  | 1.7 ms | 13 KB   | 20 KB   | 22 KB  |
| `i32 (90% zero sentinel, scattered)` | 0.5 ms  | 0.4 ms  | 2.1 ms | 13 KB   | 20 KB   | 16 KB  |

### Enums vs enum-as-integer

| Type                                                 | pco    | PcoPack | msgpack | Vortex | pco   | PcoPack | msgpack | Vortex |
|------------------------------------------------------|--------|---------|---------|--------|-------|---------|---------|--------|
| `Enum (simple, skewed 68/27/5)`                      | -      | 0.6 ms  | 0.8 ms  | 4.6 ms | -     | 13 KB   | 55 KB   | 16 KB  |
| `u8 (skewed 68/27/5, plain integer)`                 | 0.3 ms | 0.3 ms  | 0.3 ms  | 1.3 ms | 13 KB | 13 KB   | 21 KB   | 26 KB  |
| `Enum (complex, repeating pattern + payloads)`       | -      | 2.5 ms  | 2.0 ms  | -      | -     | 96 KB   | 180 KB  | -      |
| `u8 (repeating-pattern discriminant, plain integer)` | 0.2 ms | 0.2 ms  | 0.3 ms  | 1.2 ms | 24 KB | 24 KB   | 17 KB   | 26 KB  |

### Strings & bytes

| Type                  | PcoPack | msgpack | Vortex  | PcoPack | msgpack | Vortex  |
|-----------------------|---------|---------|---------|---------|---------|---------|
| `String`              | 5.4 ms  | 2.8 ms  | 10.4 ms | 291 KB  | 288 KB  | 328 KB  |
| `SmolStr`             | 3.3 ms  | 3.0 ms  | 10.8 ms | 291 KB  | 288 KB  | 328 KB  |
| `Uuid (v4 random)`    | 1.8 ms  | 1.4 ms  | 23.2 ms | 1692 KB | 1692 KB | 2001 KB |
| `ByteBuf (avg 128B)`  | 13.6 ms | 10.6 ms | 19.2 ms | 12.4 MB | 12.4 MB | 12.6 MB |
| `Vec<String> (avg 4)` | 10.5 ms | 4.3 ms  | 14.5 ms | 139 KB  | 138 KB  | 183 KB  |
| `serde_json::Value`   | 13.2 ms | 4.4 ms  | -       | 635 KB  | 632 KB  | -       |

### Maps

| Type                     | PcoPack | msgpack | Vortex | PcoPack | msgpack | Vortex |
|--------------------------|---------|---------|--------|---------|---------|--------|
| `BTreeMap<String, i32>`  | 2.2 ms  | 1.3 ms  | -      | 125 KB  | 123 KB  | -      |
| `BTreeMap<SmolStr, i32>` | 1.7 ms  | 3.7 ms  | -      | 125 KB  | 123 KB  | -      |
| `HashMap<String, i32>`   | 2.8 ms  | 1.3 ms  | -      | 125 KB  | 130 KB  | -      |
| `HashMap<SmolStr, i32>`  | 2.4 ms  | 4.2 ms  | -      | 125 KB  | 130 KB  | -      |

### Floats (float_round)

| Type                          | pco    | PcoPack | msgpack | Vortex | pco    | PcoPack | msgpack | Vortex |
|-------------------------------|--------|---------|---------|--------|--------|---------|---------|--------|
| `f64 (normal, no round)`      | 1.7 ms | 1.7 ms  | 0.8 ms  | 4.9 ms | 650 KB | 650 KB  | 792 KB  | 653 KB |
| `f64 (normal, float_round=1)` | 1.7 ms | 0.7 ms  | 0.8 ms  | 4.9 ms | 650 KB | 65 KB   | 792 KB  | 653 KB |
| `f64 (normal, float_round=2)` | 1.7 ms | 1.0 ms  | 0.8 ms  | 4.9 ms | 650 KB | 106 KB  | 792 KB  | 653 KB |
| `f32 (normal, no round)`      | 1.6 ms | 1.6 ms  | 0.6 ms  | 4.7 ms | 296 KB | 296 KB  | 391 KB  | 299 KB |
| `f32 (normal, float_round=1)` | 1.6 ms | 0.7 ms  | 0.6 ms  | 4.7 ms | 296 KB | 65 KB   | 391 KB  | 299 KB |
| `f32 (normal, float_round=2)` | 1.6 ms | 1.0 ms  | 0.6 ms  | 4.7 ms | 296 KB | 106 KB  | 391 KB  | 299 KB |

`float_round = N` is a lossy knob (error <= 0.5 x 10^-N). Rounding is where the float win lives, not the codec: at N=1 the column is ~90% smaller than unrounded. Competitors have no quantization knob; their columns are constant across the table.

### Timestamps (time_round)

| Type                                  | pco    | PcoPack | msgpack | Vortex  | pco    | PcoPack | msgpack | Vortex |
|---------------------------------------|--------|---------|---------|---------|--------|---------|---------|--------|
| `DateTime (60s step, no round)`       | 1.2 ms | 1.4 ms  | 14.7 ms | 27.0 ms | 313 KB | 313 KB  | 825 KB  | 836 KB |
| `DateTime (60s step, time_round=1s)`  | 1.2 ms | 1.3 ms  | 14.7 ms | 27.0 ms | 313 KB | 69 KB   | 825 KB  | 836 KB |
| `DateTime (60s step, time_round=60s)` | 1.2 ms | 1.0 ms  | 14.7 ms | 27.0 ms | 313 KB | 14 KB   | 825 KB  | 836 KB |

The pco column is i64 microseconds, a 1:1 representation, so Pcodec is comparable. Vortex has no time type: `DateTime` is an RFC3339 string (~2.7x the exact-instant column). `time_round` is lossy: 1s rounding cuts PcoPack's column ~78%, 60s rounding ~95%.

## Structs

500k rows of a realistic metric struct: a low-cardinality `account_id` index key, random names, log-normal scores, and a clustered boolean flag. Vortex is one compact btrblocks file read back through Arrow. Standalone Pcodec is not compared: the struct holds fields Pcodec can't express (strings, `Vec`, `Option`, JSON); those rows live in [Types](#types). PcoPack stores `account_id` as per-chunk index metadata, not as a column.

- Size: PcoPack and Vortex are within 2% of each other; both 13-37% under columnar, serde_columnar, and msgpack
- Serialize: PcoPack ~33% faster than Vortex; deserialize: columnar is fastest, PcoPack ~20% faster than Vortex
- Filters: PcoPack's `account_id` index filter is ~2.5x faster than Vortex's; Vortex's 1-row scan predicate is ~1.8x faster than PcoPack's
- Filter methods differ: PcoPack materializes matching rows; columnar/serde_columnar/msgpack decompress everything and filter in Rust

| Metric        | PcoPack | Vortex   | columnar | serde_columnar | msgpack |
|---------------|---------|----------|----------|----------------|---------|
| Serialize     | 89.0 ms | 132.9 ms | 66.1 ms  | 72.6 ms        | 94.4 ms |
| Deserialize   | 47.1 ms | 59.2 ms  | 23.2 ms  | 43.8 ms        | 91.5 ms |
| Size          | 6.2 MB  | 6.3 MB   | 8.1 MB   | 7.1 MB         | 9.9 MB  |
| Filter 20%    | 9.6 ms  | 23.8 ms  | 24.6 ms  | 45.0 ms        | 88.8 ms |
| Filter <0.01% | 3.3 ms  | 1.8 ms   | 25.0 ms  | 45.9 ms        | 90.2 ms |

## Timeline

Scenario: 100 sensors report every second for 1,000 seconds. All rows in a given second share one timestamp with sub-second jitter. Uniqueness is the probability that a sensor's readings differ from its previous second.

Findings:
- `Timeline` merges duplicate observations into time ranges, achieving smaller sizes depending on uniqueness. The benefit grows as readings stay stable longer (more duplicates to merge). However, it requires changing your struct field type and adds serialization overhead at high uniqueness, where the extra cost may not be worth it
- `time_round = Duration::seconds(1)` consistently saves ~28 KB on the timestamp column across all scenarios. At low uniqueness this is a large fraction of total size (1.29x), but at high uniqueness the non-timestamp fields dominate and dilute the ratio to 1.05x

### Serialization time

| Uniqueness | Timeline | DateTime | time_round=1s | Time ratio (TL/DT) | Time ratio (TR/DT) |
|------------|----------|----------|---------------|--------------------|--------------------|
| 10%        | 2.0 ms   | 7.7 ms   | 7.3 ms        | 3.9x               | 1.05x              |
| 20%        | 2.4 ms   | 7.8 ms   | 7.4 ms        | 3.2x               | 1.05x              |
| 50%        | 5.4 ms   | 6.6 ms   | 6.3 ms        | 1.2x               | 1.05x              |
| 80%        | 8.8 ms   | 4.6 ms   | 4.2 ms        | 0.5x               | 1.10x              |
| 90%        | 9.7 ms   | 4.6 ms   | 4.2 ms        | 0.5x               | 1.10x              |              

### Compressed size

| Uniqueness | Timeline | DateTime | time_round=1s | Size ratio (TL/DT) | Size ratio (TR/DT) |
|------------|----------|----------|---------------|--------------------|--------------------|
| 10%        | 67 KB    | 126 KB   | 97 KB         | 1.9x               | 1.29x              |
| 20%        | 130 KB   | 189 KB   | 161 KB        | 1.4x               | 1.18x              |
| 50%        | 319 KB   | 485 KB   | 457 KB        | 1.5x               | 1.06x              |
| 80%        | 484 KB   | 600 KB   | 571 KB        | 1.2x               | 1.05x              |
| 90%        | 532 KB   | 600 KB   | 571 KB        | 1.1x               | 1.05x              |

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
