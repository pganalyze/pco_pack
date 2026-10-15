# Benchmarks

Sizes and times always include compression (pcodec or zstd), measured on an M5 Max MBP at max fan speed. Per-run variance is high; [contributions are welcome](https://github.com/pganalyze/pco_pack/issues/3) to improve the benchmark suite.

- [Summary](#summary): where each codec wins and loses
- [Types](#types): measures synthetic per-field compression
- [Structs](#structs): measures a synthetic multi-field struct
- [Timeline](#timeline): measures structural deduplication using `pco_pack::Timeline`
- [chunk_size](#chunk_size): measures how `chunk_size` influences size, speed, and memory use
- [Filters](#filters): measures per-type filter timing
- [Datasets](#datasets): measures real external datasets
- [Postgres](#postgres): measures real Postgres monitoring datasets

## Summary

Every codec bets on a different kind of redundancy. PcoPack bets that redundancy lives in the schema: numbers trend, timestamps drift, keys repeat, and most float precision is noise. That pays off: every real dataset measured compresses smallest with PcoPack by default, with our optional features offering even better performance.

PcoPack is less effective in some scenarios:
- LZ compression absorbs byte-level repetition for free, so msgpack+zstd wins on clustered zeros, dwells, and repeating enum discriminants
- Vortex filtering is often faster because its per-field min/max bounds can skip entire chunks without decompression
- columnar offers the fastest decode by handing the user references instead of building structs

## Types

Each row simulates a single data distribution: entropy, type width, delta, skew, sparsity, placement, alphabet, dwell, distinct count, recurrence, chunk costs.

Where each codec wins:
- Frequency and delta redundancy (counters, skew, pools): pco/PcoPack up to ~70% under msgpack
- Sequential redundancy (dwells, clustered zeros, grouped placement): msgpack+zstd ties or wins
- Placement decides: the same multiset stores tens of times smaller grouped than shuffled
- Uniform full-width is the incompressible floor: every codec roughly ties

### Numbers

| Type                                        | pco     | PcoPack | msgpack | Vortex  | pco    | PcoPack | msgpack | Vortex |
|---------------------------------------------|---------|---------|---------|---------|--------|---------|---------|--------|
| `u8 (uniform, full width)`                  | 256 KB  | 256 KB  | 302 KB  | 258 KB  | 1.2 ms | 1.6 ms  | 1.6 ms  | 2.5 ms |
| `i16 (uniform, full width)`                 | 512 KB  | 512 KB  | 634 KB  | 514 KB  | 1.6 ms | 1.7 ms  | 1.6 ms  | 3.5 ms |
| `i32 (uniform, full width)`                 | 1024 KB | 1024 KB | 1175 KB | 1026 KB | 1.7 ms | 1.8 ms  | 2.0 ms  | 5.5 ms |
| `i64 (uniform, full width)`                 | 2.0 MB  | 2.0 MB  | 2.2 MB  | 2.0 MB  | 2.1 ms | 2.2 ms  | 2.9 ms  | 6.1 ms |
| `i16 (uniform 0..1000)`                     | 320 KB  | 320 KB  | 419 KB  | 322 KB  | 1.5 ms | 1.6 ms  | 3.3 ms  | 3.0 ms |
| `i32 (uniform 0..1000)`                     | 320 KB  | 320 KB  | 419 KB  | 322 KB  | 1.6 ms | 1.6 ms  | 3.2 ms  | 3.2 ms |
| `i64 (counter, monotonic)`                  | 181 KB  | 181 KB  | 346 KB  | 185 KB  | 2.3 ms | 2.4 ms  | 3.2 ms  | 7.4 ms |
| `i16 (counter, cyclic ~20% range)`          | 183 KB  | 183 KB  | 431 KB  | 186 KB  | 2.0 ms | 2.1 ms  | 3.0 ms  | 5.3 ms |
| `u8 (counter, cyclic 8-bit)`                | 181 KB  | 181 KB  | 271 KB  | 258 KB  | 2.0 ms | 2.0 ms  | 1.6 ms  | 2.5 ms |
| `i64 (right-skew, sparse variant)`          | 138 KB  | 138 KB  | 155 KB  | 141 KB  | 2.0 ms | 2.0 ms  | 1.6 ms  | 5.7 ms |
| `i64 (right-skew)`                          | 140 KB  | 140 KB  | 158 KB  | 143 KB  | 2.1 ms | 2.1 ms  | 1.6 ms  | 5.8 ms |
| `i64 (93% zeros, scattered)`                | 25 KB   | 25 KB   | 39 KB   | 28 KB   | 1.5 ms | 1.5 ms  | 1.2 ms  | 5.6 ms |
| `i64 (93% zeros, clustered windows)`        | 26 KB   | 26 KB   | 15 KB   | 17 KB   | 1.5 ms | 1.5 ms  | 0.9 ms  | 4.3 ms |
| `u8 (tiny alphabet i.i.d., d=4)`            | 35 KB   | 35 KB   | 55 KB   | 66 KB   | 0.9 ms | 0.9 ms  | 0.7 ms  | 3.3 ms |
| `u8 (dwell sweep, d=3, run~16)`             | 14 KB   | 14 KB   | 12 KB   | 14 KB   | 1.2 ms | 1.2 ms  | 0.5 ms  | 3.2 ms |
| `u8 (dwell sweep, d=3, run~256)`            | 1 KB    | 1 KB    | 1 KB    | 4 KB    | 1.2 ms | 1.2 ms  | 0.4 ms  | 3.3 ms |
| `u8 (dwell sweep, d=3, run~4096)`           | 235 B   | 235 B   | 274 B   | 3 KB    | 1.2 ms | 1.2 ms  | 0.3 ms  | 3.0 ms |
| `i64 (d-sweep, d=4)`                        | 64 KB   | 64 KB   | 220 KB  | 67 KB   | 1.4 ms | 1.5 ms  | 3.5 ms  | 5.4 ms |
| `i64 (d-sweep, d=35)`                       | 164 KB  | 164 KB  | 375 KB  | 167 KB  | 2.3 ms | 2.4 ms  | 4.1 ms  | 5.2 ms |
| `i64 (d-sweep, d=86)`                       | 206 KB  | 206 KB  | 422 KB  | 209 KB  | 2.6 ms | 2.6 ms  | 4.2 ms  | 5.6 ms |
| `i64 (d-sweep, d=1024)`                     | 375 KB  | 375 KB  | 544 KB  | 330 KB  | 8.1 ms | 8.3 ms  | 5.0 ms  | 4.4 ms |
| `i64 (d-sweep, d=8191)`                     | 559 KB  | 559 KB  | 681 KB  | 2.0 MB  | 8.4 ms | 8.5 ms  | 6.6 ms  | 4.1 ms |
| `i64 (d-sweep, d=8193)`                     | 558 KB  | 558 KB  | 681 KB  | 2.0 MB  | 8.5 ms | 8.5 ms  | 6.9 ms  | 4.2 ms |
| `i64 (d-sweep, d=32768)`                    | 1194 KB | 1194 KB | 1195 KB | 2.0 MB  | 8.3 ms | 8.4 ms  | 7.6 ms  | 4.7 ms |
| `i64 (placement pair, d=574 grouped)`       | 5 KB    | 5 KB    | 6 KB    | 8 KB    | 2.2 ms | 2.2 ms  | 0.9 ms  | 3.6 ms |
| `i64 (placement pair, d=574 shuffled)`      | 336 KB  | 336 KB  | 503 KB  | 278 KB  | 8.1 ms | 8.2 ms  | 4.5 ms  | 6.7 ms |
| `i64 (recurrence pair, d=4096 local-block)` | 174 KB  | 174 KB  | 315 KB  | 154 KB  | 7.6 ms | 7.6 ms  | 4.2 ms  | 6.4 ms |
| `i64 (recurrence pair, d=4096 full-cycle)`  | 40 KB   | 40 KB   | 35 KB   | 35 KB   | 6.6 ms | 6.6 ms  | 1.2 ms  | 5.2 ms |
| `i64 (d=64 pool, N=2k tail chunk)`          | 2 KB    | 2 KB    | 3 KB    | 4 KB    | 0.1 ms | 0.1 ms  | 0.0 ms  | 0.3 ms |
| `i64 (d=64 pool, full writer chunk)`        | 192 KB  | 192 KB  | 407 KB  | 195 KB  | 2.4 ms | 2.5 ms  | 4.2 ms  | 3.9 ms |

### Booleans

| Type                               | pco   | PcoPack | msgpack | Vortex | pco    | PcoPack | msgpack | Vortex |
|------------------------------------|-------|---------|---------|--------|--------|---------|---------|--------|
| `bool (i.i.d. 50% true)`           | 12 KB | 18 KB   | 18 KB   | 14 KB  | 0.2 ms | 0.3 ms  | 0.3 ms  | 1.3 ms |
| `bool (i.i.d. 90% true)`           | 5 KB  | 12 KB   | 12 KB   | 14 KB  | 0.3 ms | 0.2 ms  | 0.2 ms  | 0.9 ms |
| `bool (clustered runs)`            | 1 KB  | 1 KB    | 1 KB    | 14 KB  | 0.4 ms | 0.1 ms  | 0.2 ms  | 0.9 ms |
| `bool (alternating, period 2)`     | 12 KB | 24 B    | 24 B    | 14 KB  | 0.1 ms | 0.1 ms  | 0.1 ms  | 0.9 ms |
| `bool (i.i.d. 5% true)`            | 3 KB  | 7 KB    | 7 KB    | 14 KB  | 0.3 ms | 0.2 ms  | 0.2 ms  | 0.9 ms |
| `bool (clustered, ~20% true runs)` | 3 KB  | 6 KB    | 6 KB    | 14 KB  | 0.4 ms | 0.2 ms  | 0.2 ms  | 0.9 ms |

### Option<i32> vs sentinel

| Type                                 | PcoPack | msgpack | Vortex | PcoPack | msgpack | Vortex |
|--------------------------------------|---------|---------|--------|---------|---------|--------|
| `Option<i32> (80% null, clustered)`  | 10 KB   | 12 KB   | 26 KB  | 0.3 ms  | 0.2 ms  | 1.3 ms |
| `i32 (80% zero sentinel, clustered)` | 20 KB   | 12 KB   | 14 KB  | 0.6 ms  | 0.4 ms  | 1.7 ms |
| `Option<i32> (80% null, scattered)`  | 24 KB   | 33 KB   | 29 KB  | 0.5 ms  | 0.4 ms  | 1.9 ms |
| `i32 (80% zero sentinel, scattered)` | 24 KB   | 33 KB   | 26 KB  | 0.6 ms  | 0.5 ms  | 1.8 ms |
| `Option<i32> (90% null, clustered)`  | 5 KB    | 6 KB    | 20 KB  | 0.2 ms  | 0.2 ms  | 1.2 ms |
| `i32 (90% zero sentinel, clustered)` | 11 KB   | 6 KB    | 8 KB   | 0.6 ms  | 0.3 ms  | 1.5 ms |
| `Option<i32> (90% null, scattered)`  | 13 KB   | 20 KB   | 22 KB  | 0.3 ms  | 0.3 ms  | 1.5 ms |
| `i32 (90% zero sentinel, scattered)` | 13 KB   | 20 KB   | 16 KB  | 0.5 ms  | 0.4 ms  | 2.0 ms |

### Enums vs enum-as-integer

| Type                                                 | pco   | PcoPack | msgpack | Vortex | pco    | PcoPack | msgpack | Vortex |
|------------------------------------------------------|-------|---------|---------|--------|--------|---------|---------|--------|
| `Enum (simple, skewed 68/27/5)`                      | -     | 13 KB   | 55 KB   | 16 KB  | -      | 0.6 ms  | 0.8 ms  | 5.0 ms |
| `u8 (skewed 68/27/5, plain integer)`                 | 13 KB | 13 KB   | 21 KB   | 26 KB  | 0.3 ms | 0.3 ms  | 0.3 ms  | 1.2 ms |
| `Enum (complex, repeating pattern + payloads)`       | -     | 96 KB   | 180 KB  | -      | -      | 2.3 ms  | 2.0 ms  | -      |
| `u8 (repeating-pattern discriminant, plain integer)` | 24 KB | 24 KB   | 17 KB   | 26 KB  | 0.2 ms | 0.2 ms  | 0.3 ms  | 1.2 ms |

Note: Vortex does not currently support complex enums.

### Strings & bytes

| Type                                          | PcoPack | msgpack | Vortex  | PcoPack | msgpack | Vortex  |
|-----------------------------------------------|---------|---------|---------|---------|---------|---------|
| `String`                                      | 291 KB  | 288 KB  | 328 KB  | 4.3 ms  | 2.9 ms  | 13.4 ms |
| `SmolStr`                                     | 291 KB  | 288 KB  | 328 KB  | 3.1 ms  | 3.1 ms  | 14.0 ms |
| `Uuid (v4 random)`                            | 1692 KB | 1691 KB | 2001 KB | 1.8 ms  | 1.7 ms  | 31.5 ms |
| `ByteBuf (avg 128B)`                          | 12.4 MB | 12.4 MB | 12.3 MB | 13.3 ms | 11.3 ms | 35.3 ms |
| `Vec<String> (avg 4)`                         | 139 KB  | 138 KB  | 183 KB  | 8.5 ms  | 4.4 ms  | 18.3 ms |
| `serde_json::Value`                           | 635 KB  | 632 KB  | -       | 10.0 ms | 4.4 ms  | -       |
| `serde_json::Value (5 stable keys, 50k rows)` | 301 KB  | 299 KB  | 124 KB  | 9.6 ms  | 4.4 ms  | 10.2 ms |

Vortex supports stable-keyed JSON using struct promotion, but does not support heterogeneous JSON.

### Maps

| Type                                                             | PcoPack | msgpack | Vortex | PcoPack  | msgpack | Vortex  |
|------------------------------------------------------------------|---------|---------|--------|----------|---------|---------|
| `BTreeMap<String, i32>`                                          | 125 KB  | 123 KB  | 138 KB | 1.9 ms   | 1.3 ms  | 5.8 ms  |
| `HashMap<String, i32>`                                           | 125 KB  | 130 KB  | 142 KB | 2.5 ms   | 1.3 ms  | 5.8 ms  |
| `BTreeMap<SmolStr, i32>`                                         | 125 KB  | 123 KB  | 138 KB | 1.6 ms   | 3.7 ms  | 5.8 ms  |
| `HashMap<SmolStr, i32>`                                          | 125 KB  | 130 KB  | 142 KB | 2.4 ms   | 4.2 ms  | 5.8 ms  |
| `HashMap<SmolStr, i32> (8 stable keys)`                          | 43 KB   | 148 KB  | 51 KB  | 3.3 ms   | 1.9 ms  | 4.6 ms  |
| `HashMap<SmolStr, Option<i32>> (8 stable keys, ~60% null)`       | 26 KB   | 127 KB  | 36 KB  | 3.3 ms   | 1.8 ms  | 4.6 ms  |
| `HashMap<SmolStr, i32> (8 stable keys, walk)`                    | 115 KB  | 3.3 MB  | 114 KB | 112.9 ms | 51.7 ms | 61.5 ms |
| `HashMap<SmolStr, Option<i32>> (8 stable keys, ~60% null, walk)` | 852 KB  | 3.4 MB  | 352 KB | 95.9 ms  | 52.0 ms | 78.5 ms |

First four rows use native `vortex.map`, the rest struct promotion. Struct promotion requires string keys from a stable vocabulary.

- Tuple keys cannot promote, and promoting an open vocabulary silently drops entries the sampled schema missed, so random-key maps stay native. Native pays ~10% over PcoPack because keys live in the payload, not the schema.
- Stable keys beat PcoPack once rows move: dense walk ties, sparse walk lands ~60% smaller. At 10k static rows PcoPack's flat streams still edge ahead; zstd absorbs the repeated keys.
- Native maps interleave all keys into one stream, losing the per-key runs and deltas that make promotion win.

### Floats (float_round)

| Type                          | pco    | PcoPack | msgpack | Vortex | pco    | PcoPack | msgpack | Vortex |
|-------------------------------|--------|---------|---------|--------|--------|---------|---------|--------|
| `f64 (normal, no round)`      | 650 KB | 650 KB  | 792 KB  | 653 KB | 1.7 ms | 1.7 ms  | 0.8 ms  | 4.6 ms |
| `f64 (normal, float_round=1)` | 650 KB | 65 KB   | 792 KB  | 653 KB | 1.7 ms | 0.7 ms  | 0.8 ms  | 4.6 ms |
| `f64 (normal, float_round=2)` | 650 KB | 106 KB  | 792 KB  | 653 KB | 1.7 ms | 1.0 ms  | 0.8 ms  | 4.6 ms |
| `f32 (normal, no round)`      | 296 KB | 296 KB  | 391 KB  | 299 KB | 1.6 ms | 1.5 ms  | 0.6 ms  | 4.7 ms |
| `f32 (normal, float_round=1)` | 296 KB | 65 KB   | 391 KB  | 299 KB | 1.6 ms | 0.7 ms  | 0.6 ms  | 4.7 ms |
| `f32 (normal, float_round=2)` | 296 KB | 106 KB  | 391 KB  | 299 KB | 1.6 ms | 1.0 ms  | 0.6 ms  | 4.7 ms |

`float_round` discards unwanted float precision, intentionally causing data loss to significantly reduce compressed size.

### Timestamps (time_round)

| Type                                  | pco    | PcoPack | msgpack | Vortex | pco    | PcoPack | msgpack | Vortex  |
|---------------------------------------|--------|---------|---------|--------|--------|---------|---------|---------|
| `DateTime (60s step, no round)`       | 313 KB | 313 KB  | 825 KB  | 836 KB | 1.2 ms | 1.4 ms  | 14.9 ms | 34.6 ms |
| `DateTime (60s step, time_round=1s)`  | 313 KB | 69 KB   | 825 KB  | 836 KB | 1.2 ms | 1.3 ms  | 14.9 ms | 34.6 ms |
| `DateTime (60s step, time_round=60s)` | 313 KB | 14 KB   | 825 KB  | 836 KB | 1.2 ms | 1.0 ms  | 14.9 ms | 34.6 ms |

`time_round` cuts PcoPack's column ~78% at 1s, ~95% at 60s. pco stores i64 microseconds (1:1). Vortex has no time type: `DateTime` is an RFC3339 string.

## Structs

500k rows of a realistic struct: low-cardinality `account_id` index key, random names, log-normal scores, a clustered boolean flag.

- PcoPack is ~1% smaller than Vortex and ~37% faster round trip
- columnar deserializes fastest but has the second-worst compressed size
- PcoPack's `index` filter beats Vortex by ~2.8x. Vortex's single-row scan beats PcoPack by ~3.1x

| Metric        | PcoPack | Vortex   | columnar | serde_columnar | msgpack |
|---------------|---------|----------|----------|----------------|---------|
| Size          | 6.2 MB  | 6.3 MB   | 8.1 MB   | 7.1 MB         | 9.9 MB  |
| Serialize     | 77.9 ms | 129.5 ms | 61.0 ms  | 66.2 ms        | 87.8 ms |
| Deserialize   | 38.4 ms | 54.3 ms  | 21.3 ms  | 41.6 ms        | 80.0 ms |
| Filter 20%    | 7.8 ms  | 21.5 ms  | 22.7 ms  | 41.8 ms        | 80.6 ms |
| Filter <0.01% | 3.4 ms  | 1.1 ms   | 23.8 ms  | 41.8 ms        | 80.7 ms |

## Timeline

100 sensors report every second for 1,000 seconds. Rows in a second share a timestamp with sub-second jitter. Uniqueness is the chance a reading differs from the previous second.

- `Timeline` merges duplicate observations into ranges. Gains grow with stability, but costs serialization time at high uniqueness
- `time_round = 1s` saves ~28 KB everywhere, diluted to 1.05x of total at high uniqueness

### Compressed size

| Uniqueness | Timeline | DateTime | time_round=1s | Size ratio (TL/DT) | Size ratio (TR/DT) |
|------------|----------|----------|---------------|--------------------|--------------------|
| 10%        | 67 KB    | 126 KB   | 97 KB         | 1.9x               | 1.29x              |
| 20%        | 130 KB   | 189 KB   | 161 KB        | 1.4x               | 1.18x              |
| 50%        | 319 KB   | 485 KB   | 457 KB        | 1.5x               | 1.06x              |
| 80%        | 484 KB   | 600 KB   | 571 KB        | 1.2x               | 1.05x              |
| 90%        | 532 KB   | 600 KB   | 571 KB        | 1.1x               | 1.05x              |

### Serialization time

| Uniqueness | Timeline | DateTime | time_round=1s | Time ratio (TL/DT) | Time ratio (TR/DT) |
|------------|----------|----------|---------------|--------------------|--------------------|
| 10%        | 2.0 ms   | 7.4 ms   | 7.0 ms        | 3.7x               | 1.06x              |
| 20%        | 2.3 ms   | 7.5 ms   | 7.2 ms        | 3.3x               | 1.04x              |
| 50%        | 5.1 ms   | 6.4 ms   | 6.2 ms        | 1.3x               | 1.03x              |
| 80%        | 8.0 ms   | 4.5 ms   | 4.3 ms        | 0.6x               | 1.05x              |
| 90%        | 9.1 ms   | 4.5 ms   | 4.2 ms        | 0.5x               | 1.07x              |

## chunk_size

`chunk_size` caps columnar-to-row conversion memory. Bigger chunks compress better and serialize faster for more RAM; the 2^18 default is the sweet spot. `Write peak`/`Read peak` are per-phase peaks.

### SmallStruct (32 bytes/row)

| Chunk size    | Size   | Serialize | Deserialize | Chunks | Write peak | Read peak |
|---------------|--------|-----------|-------------|--------|------------|-----------|
| 2^13 = 8192   | 157 KB | 25.8 ms   | 8.3 ms      | 64     | 6.0 MB     | 24.4 MB   |
| 2^14 = 16384  | 147 KB | 20.6 ms   | 8.2 ms      | 32     | 6.0 MB     | 24.6 MB   |
| 2^15 = 32768  | 142 KB | 18.0 ms   | 7.8 ms      | 16     | 6.5 MB     | 25.1 MB   |
| 2^16 = 65536  | 139 KB | 16.5 ms   | 7.6 ms      | 8      | 8.8 MB     | 26.1 MB   |
| 2^17 = 131072 | 138 KB | 15.9 ms   | 7.6 ms      | 4      | 13.4 MB    | 28.1 MB   |
| 2^18 = 262144 | 138 KB | 16.3 ms   | 7.6 ms      | 2      | 22.6 MB    | 32.1 MB   |
| 2^19 = 524288 | 137 KB | 16.2 ms   | 7.6 ms      | 1      | 27.1 MB    | 32.1 MB   |

### MediumStruct (80 bytes/row)

| Chunk size    | Size   | Serialize | Deserialize | Chunks | Write peak | Read peak |
|---------------|--------|-----------|-------------|--------|------------|-----------|
| 2^13 = 8192   | 683 KB | 73.1 ms   | 40.5 ms     | 64     | 6.0 MB     | 65.9 MB   |
| 2^14 = 16384  | 836 KB | 70.4 ms   | 40.3 ms     | 32     | 7.3 MB     | 66.8 MB   |
| 2^15 = 32768  | 773 KB | 65.4 ms   | 39.6 ms     | 16     | 9.5 MB     | 68.3 MB   |
| 2^16 = 65536  | 742 KB | 63.6 ms   | 39.3 ms     | 8      | 14.2 MB    | 71.3 MB   |
| 2^17 = 131072 | 726 KB | 62.0 ms   | 39.4 ms     | 4      | 23.6 MB    | 77.4 MB   |
| 2^18 = 262144 | 718 KB | 61.7 ms   | 38.4 ms     | 2      | 42.4 MB    | 89.6 MB   |
| 2^19 = 524288 | 714 KB | 61.0 ms   | 38.3 ms     | 1      | 79.9 MB    | 89.6 MB   |

### LargeStruct (176 bytes/row)

| Chunk size    | Size    | Serialize | Deserialize | Chunks | Write peak | Read peak |
|---------------|---------|-----------|-------------|--------|------------|-----------|
| 2^13 = 8192   | 1309 KB | 184.5 ms  | 85.6 ms     | 64     | 7.6 MB     | 146.7 MB  |
| 2^14 = 16384  | 1246 KB | 167.0 ms  | 85.7 ms     | 32     | 9.6 MB     | 148.4 MB  |
| 2^15 = 32768  | 1216 KB | 159.6 ms  | 84.9 ms     | 16     | 13.9 MB    | 151.8 MB  |
| 2^16 = 65536  | 1202 KB | 155.8 ms  | 82.9 ms     | 8      | 22.5 MB    | 158.7 MB  |
| 2^17 = 131072 | 1197 KB | 152.6 ms  | 82.5 ms     | 4      | 39.8 MB    | 172.6 MB  |
| 2^18 = 262144 | 1194 KB | 155.4 ms  | 82.5 ms     | 2      | 74.3 MB    | 200.5 MB  |
| 2^19 = 524288 | 1193 KB | 150.1 ms  | 82.9 ms     | 1      | 143.3 MB   | 200.5 MB  |

## Filters

PcoPack skips deserializing non-matching rows: a 1%-selective filter pays ~20% of the full deserialize cost for most types.

| Filter                                               | Time (ms) | Rows   |
|------------------------------------------------------|-----------|--------|
| no filter                                            | 32.4 ms   | 100000 |
| empty filter                                         | 32.2 ms   | 100000 |
| i64 exact (id == 50_000)                             | 6.6 ms    | 1000   |
| i64 range (50_000..=50_999)                          | 6.1 ms    | 1000   |
| i64 inclusion (100 values, 1 match)                  | 6.2 ms    | 1000   |
| i32 exact (int32_val == 50)                          | 6.1 ms    | 1000   |
| i32 range (50..=50.99)                               | 6.2 ms    | 1000   |
| i32 inclusion (100 values, 1 match)                  | 6.3 ms    | 1000   |
| i8 exact (int8_val == 50)                            | 6.2 ms    | 1000   |
| i8 range (50..=50.99)                                | 6.2 ms    | 1000   |
| i8 inclusion (100 values, 1 match)                   | 6.6 ms    | 1000   |
| u8 exact (u8_val == 50)                              | 6.6 ms    | 1000   |
| u8 range (50..=50.99)                                | 6.6 ms    | 1000   |
| u8 inclusion (100 values, 1 match)                   | 6.8 ms    | 1000   |
| f64 exact (float64_val == 50.0)                      | 6.6 ms    | 1000   |
| f64 range (50.0..=50.99)                             | 6.5 ms    | 1000   |
| f64 inclusion (100 values, 1 match)                  | 8.9 ms    | 1000   |
| f32 exact (float32_val == 50.0)                      | 6.1 ms    | 1000   |
| f32 range (50.0..=50.99)                             | 6.1 ms    | 1000   |
| f32 inclusion (100 values, 1 match)                  | 8.8 ms    | 1000   |
| f16 exact (f16_val == 50.0)                          | 6.1 ms    | 1000   |
| f16 range (50.0..=50.99)                             | 6.2 ms    | 1000   |
| f16 inclusion (100 values, 1 match)                  | 8.8 ms    | 1000   |
| string exact                                         | 7.8 ms    | 1000   |
| string inclusion (10 values)                         | 7.9 ms    | 10000  |
| bool exact (bool_val == true)                        | 13.9 ms   | 1000   |
| bool exact (bool_val == false)                       | 31.8 ms   | 99000  |
| enum exact (status == V50)                           | 6.1 ms    | 1000   |
| enum inclusion (status in 0..9)                      | 6.1 ms    | 10000  |
| option exact (option_val == 50)                      | 6.2 ms    | 1000   |
| option range (50..=50.99)                            | 6.1 ms    | 1000   |
| option inclusion (100 values, 1 match)               | 6.3 ms    | 1000   |
| vec contains (vec_val has 5000)                      | 6.2 ms    | 1000   |
| vec contains inclusion (any of 10 values)            | 6.2 ms    | 10000  |
| bytes exact (hex match)                              | 7.8 ms    | 1000   |
| map exact (has key 'key_50')                         | 12.4 ms   | 1000   |
| map inclusion (any of key_0..key_9)                  | 13.7 ms   | 10000  |
| json exact (tag_50 string)                           | 9.0 ms    | 1000   |
| uuid exact (bucket 50)                               | 7.0 ms    | 1000   |
| uuid inclusion (100 values, 1 match)                 | 7.1 ms    | 1000   |
| nested (nested.inner_id == 50_000)                   | 6.9 ms    | 1000   |
| nested (nested.inner_id range 50_000..=50_000.99)    | 6.9 ms    | 1000   |
| nested (nested.inner_name exact)                     | 8.4 ms    | 1000   |
| nested (nested.inner_name inclusion, 10 values)      | 8.4 ms    | 10000  |
| partial fields (id + string_val)                     | 0.7 ms    | 1000   |
| multi-field (id + int32_val)                         | 6.3 ms    | 1000   |
| multi-field (bytes_val + u8_val)                     | 7.0 ms    | 1000   |
| multi-field (string_val + u8_val)                    | 7.1 ms    | 1000   |
| multi-field (json_val + u8_val)                      | 7.0 ms    | 1000   |
| multi-field zero-match (id + int32_val + string_val) | 0.4 ms    | 0      |

## Datasets

Real datasets from public sources.

### air_quality (823743 rows)

NYC AirData single-sensor stream: wide i16 measurements, a monotonic i64 counter, ns `DateTime` stored as us.

| Metric       | PcoPack  | PcoPack (lossy) | Vortex   | columnar | serde_columnar | msgpack  |
|--------------|----------|-----------------|----------|----------|----------------|----------|
| Size         | 4.1 MB   | 2.5 MB          | 4.1 MB   | 14.0 MB  | 5.4 MB         | 15.9 MB  |
| Serialize    | 112.7 ms | 107.7 ms        | 209.7 ms | 107.5 ms | 133.0 ms       | 304.6 ms |
| Deserialize  | 55.0 ms  | 53.8 ms         | 68.4 ms  | 35.8 ms  | 60.4 ms        | 217.1 ms |
| Filter 0.18% | 6.9 ms   | 6.7 ms          | 1.1 ms   | 33.9 ms  | 59.7 ms        | 205.9 ms |
| Filter 4.49% | 8.5 ms   | 8.0 ms          | 3.3 ms   | 34.0 ms  | 58.6 ms        | 206.7 ms |

- Full-fidelity size is tied between PcoPack and Vortex
- Lossy time rounding reduces size by 40%

### taxi (19144903 rows)

NYC taxi trips (also seen in the upstream pcodec benchmarks). All four timestamps are integer seconds and `float_round = 3` is lossless.

| Metric       | PcoPack  | PcoPack (lossy) | Vortex   | columnar | serde_columnar | msgpack  |
|--------------|----------|-----------------|----------|----------|----------------|----------|
| Size         | 285.5 MB | 275.7 MB        | 415.1 MB | 572.5 MB | 523.4 MB       | 661.5 MB |
| Serialize    | 9.2 s    | 8.4 s           | 20.7 s   | 7.7 s    | 10.1 s         | 11.9 s   |
| Deserialize  | 3.6 s    | 3.4 s           | 9.0 s    | 2.6 s    | 5.2 s          | 9.6 s    |
| Filter 0.2%  | 109.3 ms | 92.7 ms         | 28.1 ms  | 2.5 s    | 5.1 s          | 9.6 s    |
| Filter 4.05% | 218.4 ms | 206.9 ms        | 395.4 ms | 2.5 s    | 5.1 s          | 9.7 s    |

- PcoPack is the smallest by a wide margin
- For filtering, hour window favors Vortex (finer row-group pruning) and day window favors PcoPack
- pcodec reference (numeric columns only): pco L8 333.6 MB, parquet+zstd1 456.5 MB

### OpenStreetMap

Geofabrik Austria PBF. Filters are padded boxes on the centroid: cross-border geometry puts a measurable fraction of nodes outside the nominal bbox. Ways and relations have no natural scalar filter.

#### Node (85929052 rows)

| Metric       | PcoPack  | PcoPack (lossy) | Vortex   | columnar | serde_columnar | msgpack |
|--------------|----------|-----------------|----------|----------|----------------|---------|
| Size         | 847.6 MB | 351.4 MB        | 405.5 MB | 1.2 GB   | 1.1 GB         | 1.3 GB  |
| Serialize    | 6.5 s    | 6.4 s           | 40.8 s   | 7.9 s    | 7.5 s          | 7.8 s   |
| Deserialize  | 2.8 s    | 2.8 s           | 4.0 s    | 2.2 s    | 3.5 s          | 5.9 s   |
| Filter 0.18% | 707.9 ms | 749.7 ms        | 507.0 ms | 2.1 s    | 3.4 s          | 5.7 s   |
| Filter 4.66% | 1.1 s    | 1.2 s           | 2.0 s    | 2.1 s    | 3.4 s          | 5.7 s   |

- Vortex automatically converts limited-precision floats into integers, which is why it outperforms PcoPack by default
- PcoPack (lossy) uses `float_round = 7`, cutting node size ~59% and only drops sub-centimeter coordinate precision

#### Way (9172339 rows)

| Metric      | PcoPack  | Vortex   | columnar | serde_columnar | msgpack  |
|-------------|----------|----------|----------|----------------|----------|
| Size        | 195.2 MB | 208.1 MB | 384.8 MB | 448.2 MB       | 478.4 MB |
| Serialize   | 6.5 s    | 13.1 s   | 6.1 s    | 5.0 s          | 4.2 s    |
| Deserialize | 3.8 s    | 4.0 s    | 1.6 s    | 3.2 s          | 3.7 s    |

#### Relation (175564 rows)

| Metric      | PcoPack  | Vortex   | columnar | serde_columnar | msgpack  |
|-------------|----------|----------|----------|----------------|----------|
| Size        | 17.8 MB  | 18.4 MB  | 19.5 MB  | 16.6 MB        | 19.1 MB  |
| Serialize   | 328.2 ms | 653.0 ms | 269.7 ms | 293.1 ms       | 195.9 ms |
| Deserialize | 212.4 ms | 245.4 ms | 62.3 ms  | 189.6 ms       | 236.5 ms |

### openalex (632276 works)

OpenAlex `works` snapshot: deeply nested JSON flattened to typed structs.

| Metric        | PcoPack  | PcoPack (lossy) | Vortex   | columnar | serde_columnar | msgpack  |
|---------------|----------|-----------------|----------|----------|----------------|----------|
| Size          | 322.4 MB | 316.0 MB        | 334.4 MB | 450.0 MB | 394.5 MB       | 434.6 MB |
| Serialize     | 9.3 s    | 9.4 s           | 11.8 s   | 7.2 s    | 8.7 s          | 4.1 s    |
| Deserialize   | 5.3 s    | 5.2 s           | 4.8 s    | 1.5 s    | 5.3 s          | 4.2 s    |
| Filter <0.01% | 1.4 s    | 1.4 s           | 118.4 ms | 1.5 s    | 4.9 s          | 4.2 s    |
| Filter 2.96%  | 2.3 s    | 2.3 s           | 964.4 ms | 1.5 s    | 5.3 s          | 4.4 s    |

- PcoPack (lossy) `float_round = 4` saves 2% because `fwci` and scores carry ~15-digit model noise
- Vortex filters fastest (row-group pruning + predicate pushdown)

## Postgres

Real Postgres monitoring datasets sourced from pganalyze.

### backend_snapshots (13328012 rows)

| Metric        | PcoPack  | PcoPack time_round | PcoPack Timeline | Vortex   | columnar | serde_columnar | msgpack |
|---------------|----------|--------------------|------------------|----------|----------|----------------|---------|
| Size          | 47.1 MB  | 39.2 MB            | 18.0 MB          | 83.8 MB  | 68.8 MB  | 69.3 MB        | 80.2 MB |
| Serialize     | 3.1 s    | 3.3 s              | 2.1 s            | 3.7 s    | 1.1 s    | 2.8 s          | 4.6 s   |
| Deserialize   | 1.3 s    | 1.3 s              | 484.9 ms         | 1.5 s    | 561.7 ms | 1.9 s          | 4.8 s   |
| Filter 0.49%  | 65.9 ms  | 67.4 ms            | 171.6 ms         | 12.5 ms  | 538.6 ms | 2.1 s          | 4.7 s   |
| Filter 29.25% | 736.9 ms | 752.4 ms           | 438.2 ms         | 754.8 ms | 537.6 ms | 1.8 s          | 4.6 s   |
| Filter 77.25% | 1.1 s    | 1.1 s              | 372.1 ms         | 1.3 s    | 549.1 ms | 2.0 s          | 4.8 s   |

- Full-fidelity PcoPack beats Vortex by ~44%, columnar ~31%, msgpack ~41%. `time_round = 1s` cuts ~17% more
- Most backends are idle, allowing PcoPack `Timeline` to cut row count by ~71%, serialized size ~62%, and serialize time ~32%

### query_explains (237651 rows)

This data model is dominated by large Postgres EXPLAIN plans represented as JSON.

#### PcoPack scenarios

| Metric        | text     | JSON     | JSON lossy | arena    | arena lossy |
|---------------|----------|----------|------------|----------|-------------|
| Size          | 91.7 MB  | 82.0 MB  | 76.0 MB    | 57.7 MB  | 55.6 MB     |
| Serialize     | 1.7 s    | 4.6 s    | 4.2 s      | 4.4 s    | 4.1 s       |
| Deserialize   | 974.4 ms | 7.5 s    | 7.5 s      | 1.9 s    | 1.9 s       |
| Filter 0.55%  | 672.9 ms | 1.8 s    | 1.7 s      | 567.0 ms | 517.9 ms    |
| Filter 12.48% | 658.2 ms | 4.5 s    | 4.6 s      | 793.1 ms | 770.5 ms    |
| Filter 12.97% | 25.7 ms  | 154.3 ms | 151.4 ms   | 49.0 ms  | 45.6 ms     |

- PcoPack even when storing the explain JSON as text outperforms the other crates. When storing JSON, PcoPack reduces compressed size but much more time is spent serializing.
- JSON lossy uses `float_round = 2, time_round = 1s` to cut ~7% off the JSON path. Float rounding removes sub-millisecond precision.
- The arena (AKA struct of array) scenarios replace the JSON tree with a flattened typed arena of native columns. Arena is ~30% smaller than JSON, and lossy arena shaves ~3% more. Arena deserialization is ~4x faster than JSON, and filters are ~3x faster.

#### Other crates

| Metric        | Vortex   | columnar | serde_columnar | msgpack  |
|---------------|----------|----------|----------------|----------|
| Size          | 122.5 MB | 127.1 MB | 123.3 MB       | 110.3 MB |
| Serialize     | 5.2 s    | 1.8 s    | 4.8 s          | 2.3 s    |
| Deserialize   | 729.4 ms | 636.2 ms | 935.2 ms       | 5.5 s    |
| Filter 0.55%  | 19.4 ms  | 626.0 ms | 857.5 ms       | 5.4 s    |
| Filter 12.48% | 553.3 ms | 623.9 ms | 850.3 ms       | 4.9 s    |
| Filter 12.97% | 569.0 ms | 628.0 ms | 857.8 ms       | 4.8 s    |

- Vortex, columnar, and serde_columnar store the JSON as text; the same as the PcoPack text scenario.

### query_stats_1min (29970299 rows)

| Metric        | PcoPack  | PcoPack time_round | Vortex   | columnar | serde_columnar | msgpack  |
|---------------|----------|--------------------|----------|----------|----------------|----------|
| Size          | 116.5 MB | 116.4 MB           | 129.9 MB | 191.7 MB | 179.4 MB       | 236.2 MB |
| Serialize     | 4.4 s    | 4.4 s              | 6.5 s    | 4.6 s    | 3.6 s          | 6.0 s    |
| Deserialize   | 1.8 s    | 1.8 s              | 2.4 s    | 1.5 s    | 3.1 s          | 4.8 s    |
| Filter 0.31%  | 142.7 ms | 150.8 ms           | 75.9 ms  | 1.4 s    | 2.8 s          | 4.8 s    |
| Filter 18.19% | 793.9 ms | 808.4 ms           | 1.1 s    | 1.4 s    | 2.8 s          | 4.8 s    |
| Filter 0.05%  | 33.4 ms  | 33.3 ms            | 17.7 ms  | 1.4 s    | 3.0 s          | 4.7 s    |

### query_stats_1hour (1041799 rows)

| Metric        | PcoPack  | PcoPack time_round | Vortex   | columnar | serde_columnar | msgpack  |
|---------------|----------|--------------------|----------|----------|----------------|----------|
| Size          | 5.9 MB   | 5.9 MB             | 7.1 MB   | 10.0 MB  | 9.1 MB         | 11.6 MB  |
| Serialize     | 171.1 ms | 170.6 ms           | 253.3 ms | 168.7 ms | 210.2 ms       | 218.1 ms |
| Deserialize   | 71.3 ms  | 68.0 ms            | 96.1 ms  | 55.5 ms  | 146.0 ms       | 182.0 ms |
| Filter 0.63%  | 25.3 ms  | 24.4 ms            | 3.1 ms   | 55.6 ms  | 128.6 ms       | 178.5 ms |
| Filter 58.45% | 49.9 ms  | 50.6 ms            | 77.4 ms  | 55.3 ms  | 112.9 ms       | 178.7 ms |
| Filter 0.03%  | 1.7 ms   | 1.5 ms             | 14.5 ms  | 55.7 ms  | 105.3 ms       | 183.9 ms |

- PcoPack is the smallest at both granularities
- `time_round` is less effective because data is captured once a minute versus backend_snapshots which are captured once every 10 seconds
