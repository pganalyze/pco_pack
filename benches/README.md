# Benchmarks

Sizes and times include compression, measured on an M5 Max. Every codec serializes and deserializes the whole capture in one call, matching how its API is used in production. Times are the minimum of repeated runs, taken from two back-to-back full-suite runs that agree within 0.3% at the median.

`cargo bench` runs the suite one binary at a time, in about 70 minutes, peaking near 50 GB of memory; let it run alone. The comparison crates change rarely, so `cargo bench -- --crates pco_pack` skips most of the work. [Contributions are welcome](https://github.com/pganalyze/pco_pack/issues/3).

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
- Vortex filtering is often faster because its per-field metadata can skip entire chunks without decompression
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
| `u8 (uniform, full width)`                  | 256 KB  | 256 KB  | 302 KB  | 258 KB  | 1.1 ms | 1.2 ms  | 1.5 ms  | 2.3 ms |
| `i16 (uniform, full width)`                 | 512 KB  | 512 KB  | 634 KB  | 514 KB  | 1.5 ms | 1.5 ms  | 1.5 ms  | 3.1 ms |
| `i32 (uniform, full width)`                 | 1024 KB | 1024 KB | 1175 KB | 1026 KB | 1.6 ms | 1.6 ms  | 1.8 ms  | 4.9 ms |
| `i64 (uniform, full width)`                 | 2.0 MB  | 2.0 MB  | 2.2 MB  | 2.0 MB  | 1.9 ms | 1.9 ms  | 2.7 ms  | 5.2 ms |
| `i16 (uniform 0..1000)`                     | 320 KB  | 320 KB  | 419 KB  | 322 KB  | 1.4 ms | 1.4 ms  | 3.0 ms  | 2.7 ms |
| `i32 (uniform 0..1000)`                     | 320 KB  | 320 KB  | 419 KB  | 322 KB  | 1.5 ms | 1.5 ms  | 3.1 ms  | 2.8 ms |
| `i64 (counter, monotonic)`                  | 181 KB  | 181 KB  | 346 KB  | 185 KB  | 2.0 ms | 2.1 ms  | 3.0 ms  | 6.5 ms |
| `i16 (counter, cyclic ~20% range)`          | 183 KB  | 183 KB  | 431 KB  | 186 KB  | 1.8 ms | 1.8 ms  | 2.8 ms  | 4.8 ms |
| `u8 (counter, cyclic 8-bit)`                | 181 KB  | 181 KB  | 271 KB  | 258 KB  | 1.8 ms | 1.8 ms  | 1.5 ms  | 2.3 ms |
| `i64 (right-skew, sparse variant)`          | 138 KB  | 138 KB  | 155 KB  | 141 KB  | 1.7 ms | 1.7 ms  | 1.3 ms  | 4.8 ms |
| `i64 (right-skew)`                          | 140 KB  | 140 KB  | 158 KB  | 143 KB  | 1.8 ms | 1.8 ms  | 1.3 ms  | 4.9 ms |
| `i64 (93% zeros, scattered)`                | 25 KB   | 25 KB   | 39 KB   | 28 KB   | 1.3 ms | 1.3 ms  | 0.9 ms  | 4.7 ms |
| `i64 (93% zeros, clustered windows)`        | 26 KB   | 26 KB   | 15 KB   | 17 KB   | 1.3 ms | 1.3 ms  | 0.7 ms  | 3.6 ms |
| `u8 (tiny alphabet i.i.d., d=4)`            | 35 KB   | 35 KB   | 55 KB   | 66 KB   | 0.8 ms | 0.8 ms  | 0.6 ms  | 2.9 ms |
| `u8 (dwell sweep, d=3, run~16)`             | 14 KB   | 14 KB   | 12 KB   | 14 KB   | 1.0 ms | 1.0 ms  | 0.4 ms  | 2.8 ms |
| `u8 (dwell sweep, d=3, run~256)`            | 1 KB    | 1 KB    | 1 KB    | 4 KB    | 1.1 ms | 1.1 ms  | 0.3 ms  | 3.0 ms |
| `u8 (dwell sweep, d=3, run~4096)`           | 235 B   | 235 B   | 274 B   | 3 KB    | 1.1 ms | 1.1 ms  | 0.3 ms  | 2.7 ms |
| `i64 (d-sweep, d=4)`                        | 64 KB   | 64 KB   | 220 KB  | 67 KB   | 1.2 ms | 1.3 ms  | 3.5 ms  | 4.6 ms |
| `i64 (d-sweep, d=35)`                       | 164 KB  | 164 KB  | 375 KB  | 167 KB  | 2.0 ms | 2.0 ms  | 4.1 ms  | 4.6 ms |
| `i64 (d-sweep, d=86)`                       | 206 KB  | 206 KB  | 422 KB  | 209 KB  | 2.3 ms | 2.2 ms  | 4.1 ms  | 4.8 ms |
| `i64 (d-sweep, d=1024)`                     | 375 KB  | 375 KB  | 544 KB  | 330 KB  | 7.5 ms | 7.6 ms  | 4.8 ms  | 3.7 ms |
| `i64 (d-sweep, d=8191)`                     | 559 KB  | 559 KB  | 681 KB  | 2.0 MB  | 7.4 ms | 7.5 ms  | 6.2 ms  | 3.3 ms |
| `i64 (d-sweep, d=8193)`                     | 558 KB  | 558 KB  | 681 KB  | 2.0 MB  | 7.4 ms | 7.5 ms  | 6.3 ms  | 3.3 ms |
| `i64 (d-sweep, d=32768)`                    | 1194 KB | 1194 KB | 1195 KB | 2.0 MB  | 7.4 ms | 7.4 ms  | 7.3 ms  | 3.7 ms |
| `i64 (placement pair, d=574 grouped)`       | 5 KB    | 5 KB    | 6 KB    | 8 KB    | 1.9 ms | 1.9 ms  | 0.9 ms  | 2.9 ms |
| `i64 (placement pair, d=574 shuffled)`      | 336 KB  | 336 KB  | 503 KB  | 278 KB  | 7.0 ms | 7.0 ms  | 4.4 ms  | 5.7 ms |
| `i64 (recurrence pair, d=4096 local-block)` | 174 KB  | 174 KB  | 315 KB  | 154 KB  | 6.5 ms | 6.5 ms  | 4.0 ms  | 5.3 ms |
| `i64 (recurrence pair, d=4096 full-cycle)`  | 40 KB   | 40 KB   | 35 KB   | 35 KB   | 6.0 ms | 6.1 ms  | 1.3 ms  | 4.5 ms |
| `i64 (d=64 pool, N=2k tail chunk)`          | 2 KB    | 2 KB    | 3 KB    | 4 KB    | 0.1 ms | 0.1 ms  | 0.0 ms  | 0.3 ms |
| `i64 (d=64 pool, full writer chunk)`        | 192 KB  | 192 KB  | 407 KB  | 195 KB  | 2.1 ms | 2.1 ms  | 4.1 ms  | 3.2 ms |

### Booleans

| Type                               | pco   | PcoPack | msgpack | Vortex | pco    | PcoPack | msgpack | Vortex |
|------------------------------------|-------|---------|---------|--------|--------|---------|---------|--------|
| `bool (i.i.d. 50% true)`           | 12 KB | 18 KB   | 18 KB   | 14 KB  | 0.1 ms | 0.2 ms  | 0.2 ms  | 1.0 ms |
| `bool (i.i.d. 90% true)`           | 5 KB  | 12 KB   | 12 KB   | 14 KB  | 0.3 ms | 0.2 ms  | 0.2 ms  | 0.8 ms |
| `bool (clustered runs)`            | 1 KB  | 1 KB    | 1 KB    | 14 KB  | 0.4 ms | 0.1 ms  | 0.1 ms  | 0.8 ms |
| `bool (alternating, period 2)`     | 12 KB | 24 B    | 24 B    | 14 KB  | 0.1 ms | 0.1 ms  | 0.1 ms  | 0.8 ms |
| `bool (i.i.d. 5% true)`            | 3 KB  | 7 KB    | 7 KB    | 14 KB  | 0.3 ms | 0.2 ms  | 0.2 ms  | 0.8 ms |
| `bool (clustered, ~20% true runs)` | 3 KB  | 6 KB    | 6 KB    | 14 KB  | 0.4 ms | 0.2 ms  | 0.2 ms  | 0.8 ms |

### Option<i32> vs sentinel

| Type                                 | PcoPack | msgpack | Vortex | PcoPack | msgpack | Vortex |
|--------------------------------------|---------|---------|--------|---------|---------|--------|
| `Option<i32> (80% null, clustered)`  | 10 KB   | 12 KB   | 26 KB  | 0.3 ms  | 0.2 ms  | 1.2 ms |
| `i32 (80% zero sentinel, clustered)` | 20 KB   | 12 KB   | 14 KB  | 0.5 ms  | 0.3 ms  | 1.6 ms |
| `Option<i32> (80% null, scattered)`  | 24 KB   | 33 KB   | 29 KB  | 0.4 ms  | 0.4 ms  | 1.7 ms |
| `i32 (80% zero sentinel, scattered)` | 24 KB   | 33 KB   | 26 KB  | 0.5 ms  | 0.4 ms  | 1.6 ms |
| `Option<i32> (90% null, clustered)`  | 5 KB    | 6 KB    | 20 KB  | 0.2 ms  | 0.2 ms  | 1.1 ms |
| `i32 (90% zero sentinel, clustered)` | 11 KB   | 6 KB    | 8 KB   | 0.5 ms  | 0.3 ms  | 1.4 ms |
| `Option<i32> (90% null, scattered)`  | 13 KB   | 20 KB   | 22 KB  | 0.3 ms  | 0.3 ms  | 1.4 ms |
| `i32 (90% zero sentinel, scattered)` | 13 KB   | 20 KB   | 16 KB  | 0.5 ms  | 0.3 ms  | 1.8 ms |

### Enums vs enum-as-integer

| Type                                                 | pco   | PcoPack | msgpack | Vortex | pco    | PcoPack | msgpack | Vortex |
|------------------------------------------------------|-------|---------|---------|--------|--------|---------|---------|--------|
| `Enum (simple, skewed 68/27/5)`                      | -     | 13 KB   | 55 KB   | 16 KB  | -      | 0.5 ms  | 0.8 ms  | 4.3 ms |
| `u8 (skewed 68/27/5, plain integer)`                 | 13 KB | 13 KB   | 21 KB   | 26 KB  | 0.3 ms | 0.3 ms  | 0.2 ms  | 1.1 ms |
| `Enum (complex, repeating pattern + payloads)`       | -     | 96 KB   | 180 KB  | -      | -      | 2.1 ms  | 1.8 ms  | -      |
| `u8 (repeating-pattern discriminant, plain integer)` | 24 KB | 24 KB   | 17 KB   | 26 KB  | 0.2 ms | 0.2 ms  | 0.2 ms  | 1.0 ms |

Note: Vortex does not currently support complex enums.

### Strings & bytes

| Type                                          | PcoPack | msgpack | Vortex  | PcoPack | msgpack | Vortex  |
|-----------------------------------------------|---------|---------|---------|---------|---------|---------|
| `String`                                      | 291 KB  | 288 KB  | 328 KB  | 3.9 ms  | 2.6 ms  | 11.7 ms |
| `SmolStr`                                     | 291 KB  | 288 KB  | 328 KB  | 2.7 ms  | 2.6 ms  | 11.6 ms |
| `Uuid (v4 random)`                            | 1692 KB | 1692 KB | 2001 KB | 1.5 ms  | 1.3 ms  | 27.3 ms |
| `ByteBuf (avg 128B)`                          | 12.4 MB | 12.4 MB | 12.3 MB | 10.4 ms | 8.5 ms  | 29.2 ms |
| `Vec<String> (avg 4)`                         | 139 KB  | 138 KB  | 183 KB  | 7.4 ms  | 4.0 ms  | 16.4 ms |
| `serde_json::Value`                           | 635 KB  | 632 KB  | -       | 8.3 ms  | 4.1 ms  | -       |
| `serde_json::Value (5 stable keys, 50k rows)` | 301 KB  | 299 KB  | 124 KB  | 9.5 ms  | 4.1 ms  | 9.6 ms  |

Vortex supports stable-keyed JSON using struct promotion, but does not support heterogeneous JSON.

### Maps

| Type                                                             | PcoPack | msgpack | Vortex | PcoPack  | msgpack | Vortex  |
|------------------------------------------------------------------|---------|---------|--------|----------|---------|---------|
| `BTreeMap<String, i32>`                                          | 125 KB  | 123 KB  | 138 KB | 1.8 ms   | 1.1 ms  | 5.4 ms  |
| `HashMap<String, i32>`                                           | 125 KB  | 130 KB  | 142 KB | 2.4 ms   | 1.1 ms  | 5.3 ms  |
| `BTreeMap<SmolStr, i32>`                                         | 125 KB  | 123 KB  | 138 KB | 1.5 ms   | 3.5 ms  | 5.4 ms  |
| `HashMap<SmolStr, i32>`                                          | 125 KB  | 130 KB  | 142 KB | 2.2 ms   | 3.9 ms  | 5.5 ms  |
| `HashMap<SmolStr, i32> (8 stable keys)`                          | 43 KB   | 148 KB  | 51 KB  | 3.1 ms   | 1.8 ms  | 4.2 ms  |
| `HashMap<SmolStr, Option<i32>> (8 stable keys, ~60% null)`       | 26 KB   | 127 KB  | 36 KB  | 3.1 ms   | 1.7 ms  | 4.2 ms  |
| `HashMap<SmolStr, i32> (8 stable keys, walk)`                    | 115 KB  | 3.3 MB  | 114 KB | 108.4 ms | 49.6 ms | 58.7 ms |
| `HashMap<SmolStr, Option<i32>> (8 stable keys, ~60% null, walk)` | 852 KB  | 3.4 MB  | 352 KB | 93.5 ms  | 49.6 ms | 75.4 ms |

First four rows use native `vortex.map`, the rest struct promotion. Struct promotion requires string keys from a stable vocabulary.

- Tuple keys cannot promote, and promoting an open vocabulary silently drops entries the sampled schema missed, so random-key maps stay native. Native pays ~10% over PcoPack because keys live in the payload, not the schema.
- Stable keys beat PcoPack once rows move: dense walk ties, sparse walk lands ~60% smaller. At 10k static rows PcoPack's flat streams still edge ahead; zstd absorbs the repeated keys.
- Native maps interleave all keys into one stream, losing the per-key runs and deltas that make promotion win.

### Floats (float_round)

| Type                          | pco    | PcoPack | msgpack | Vortex | pco    | PcoPack | msgpack | Vortex |
|-------------------------------|--------|---------|---------|--------|--------|---------|---------|--------|
| `f64 (normal, no round)`      | 650 KB | 650 KB  | 792 KB  | 653 KB | 1.6 ms | 1.6 ms  | 0.7 ms  | 4.2 ms |
| `f64 (normal, float_round=1)` | 650 KB | 65 KB   | 792 KB  | 653 KB | 1.6 ms | 0.7 ms  | 0.7 ms  | 4.2 ms |
| `f64 (normal, float_round=2)` | 650 KB | 106 KB  | 792 KB  | 653 KB | 1.6 ms | 0.9 ms  | 0.7 ms  | 4.2 ms |
| `f32 (normal, no round)`      | 296 KB | 296 KB  | 391 KB  | 299 KB | 1.4 ms | 1.5 ms  | 0.4 ms  | 4.4 ms |
| `f32 (normal, float_round=1)` | 296 KB | 65 KB   | 391 KB  | 299 KB | 1.4 ms | 0.7 ms  | 0.4 ms  | 4.4 ms |
| `f32 (normal, float_round=2)` | 296 KB | 106 KB  | 391 KB  | 299 KB | 1.4 ms | 0.9 ms  | 0.4 ms  | 4.4 ms |

`float_round` discards unwanted float precision, intentionally causing data loss to significantly reduce compressed size.

### Timestamps (time_round)

| Type                                  | pco    | PcoPack | msgpack | Vortex | pco    | PcoPack | msgpack | Vortex  |
|---------------------------------------|--------|---------|---------|--------|--------|---------|---------|---------|
| `DateTime (60s step, no round)`       | 313 KB | 313 KB  | 825 KB  | 836 KB | 1.1 ms | 1.3 ms  | 12.8 ms | 32.3 ms |
| `DateTime (60s step, time_round=1s)`  | 313 KB | 69 KB   | 825 KB  | 836 KB | 1.1 ms | 1.2 ms  | 12.8 ms | 32.3 ms |
| `DateTime (60s step, time_round=60s)` | 313 KB | 14 KB   | 825 KB  | 836 KB | 1.1 ms | 0.9 ms  | 12.8 ms | 32.3 ms |

`time_round` cuts PcoPack's column ~78% at 1s, ~95% at 60s. pco stores i64 microseconds (1:1). Vortex has no time type: `DateTime` is an RFC3339 string.

## Structs

500k rows of a realistic struct: low-cardinality `account_id` index key, random names, log-normal scores, a clustered boolean flag.

- PcoPack is ~2% smaller than Vortex and ~35% faster round trip
- columnar deserializes fastest but has the second-worst compressed size
- Vortex outperforms PcoPack at both filter and reduce scenarios

| Metric           | PcoPack | Vortex   | columnar | serde_columnar | msgpack |
|------------------|---------|----------|----------|----------------|---------|
| Size             | 6.2 MB  | 6.3 MB   | 8.1 MB   | 7.1 MB         | 9.9 MB  |
| Serialize        | 78.0 ms | 125.5 ms | 57.4 ms  | 61.8 ms        | 83.2 ms |
| Deserialize      | 37.6 ms | 48.9 ms  | 19.6 ms  | 36.9 ms        | 68.0 ms |
| Filter 20%       | 7.9 ms  | 1.4 ms   | 20.9 ms  | 37.8 ms        | 73.4 ms |
| Filter <0.01%    | 3.1 ms  | 0.7 ms   | 21.6 ms  | 38.1 ms        | 75.5 ms |
| Text eq (1 rows) | 15.9 ms | 5.6 ms   | 21.7 ms  | 37.9 ms        | 76.5 ms |
| Text miss        | 14.4 ms | 0.0 ms   | 21.0 ms  | 37.6 ms        | 74.6 ms |
| Text IN (2 rows) | 16.9 ms | 11.2 ms  | 22.7 ms  | 38.8 ms        | 75.1 ms |
| Reduce 70.04%    | 12.9 ms | 5.2 ms   | 24.3 ms  | 40.9 ms        | 78.6 ms |

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
| 10%        | 1.3 ms   | 7.1 ms   | 6.7 ms        | 5.5x               | 1.06x              |
| 20%        | 2.3 ms   | 7.2 ms   | 6.9 ms        | 3.1x               | 1.04x              |
| 50%        | 5.1 ms   | 6.3 ms   | 5.9 ms        | 1.2x               | 1.07x              |
| 80%        | 7.9 ms   | 4.3 ms   | 4.0 ms        | 0.5x               | 1.07x              |
| 90%        | 8.9 ms   | 4.3 ms   | 4.0 ms        | 0.5x               | 1.07x              |

## chunk_size

`chunk_size` caps columnar-to-row conversion memory. Bigger chunks compress better and serialize faster for more RAM; the 2^18 default is the sweet spot. `Write peak`/`Read peak` are per-phase peaks.

### SmallStruct (32 bytes/row)

| Chunk size    | Size   | Serialize | Deserialize | Chunks | Write peak | Read peak |
|---------------|--------|-----------|-------------|--------|------------|-----------|
| 2^13 = 8192   | 157 KB | 25.6 ms   | 8.1 ms      | 64     | 6.0 MB     | 24.4 MB   |
| 2^14 = 16384  | 147 KB | 20.5 ms   | 8.0 ms      | 32     | 6.0 MB     | 24.6 MB   |
| 2^15 = 32768  | 142 KB | 17.9 ms   | 7.7 ms      | 16     | 6.5 MB     | 25.1 MB   |
| 2^16 = 65536  | 139 KB | 16.2 ms   | 7.6 ms      | 8      | 8.8 MB     | 26.1 MB   |
| 2^17 = 131072 | 138 KB | 15.6 ms   | 7.6 ms      | 4      | 13.4 MB    | 28.1 MB   |
| 2^18 = 262144 | 138 KB | 16.0 ms   | 7.4 ms      | 2      | 22.6 MB    | 32.1 MB   |
| 2^19 = 524288 | 137 KB | 15.9 ms   | 7.4 ms      | 1      | 27.1 MB    | 32.1 MB   |

### MediumStruct (80 bytes/row)

| Chunk size    | Size   | Serialize | Deserialize | Chunks | Write peak | Read peak |
|---------------|--------|-----------|-------------|--------|------------|-----------|
| 2^13 = 8192   | 683 KB | 72.6 ms   | 40.4 ms     | 64     | 6.0 MB     | 65.9 MB   |
| 2^14 = 16384  | 836 KB | 70.1 ms   | 40.0 ms     | 32     | 7.3 MB     | 66.8 MB   |
| 2^15 = 32768  | 773 KB | 65.7 ms   | 39.2 ms     | 16     | 9.5 MB     | 68.3 MB   |
| 2^16 = 65536  | 742 KB | 62.6 ms   | 38.6 ms     | 8      | 14.2 MB    | 71.3 MB   |
| 2^17 = 131072 | 726 KB | 61.5 ms   | 38.2 ms     | 4      | 23.6 MB    | 77.4 MB   |
| 2^18 = 262144 | 718 KB | 60.2 ms   | 38.0 ms     | 2      | 42.4 MB    | 89.6 MB   |
| 2^19 = 524288 | 714 KB | 60.5 ms   | 37.9 ms     | 1      | 79.9 MB    | 89.6 MB   |

### LargeStruct (176 bytes/row)

| Chunk size    | Size    | Serialize | Deserialize | Chunks | Write peak | Read peak |
|---------------|---------|-----------|-------------|--------|------------|-----------|
| 2^13 = 8192   | 1309 KB | 183.2 ms  | 84.8 ms     | 64     | 7.6 MB     | 146.7 MB  |
| 2^14 = 16384  | 1246 KB | 164.4 ms  | 84.1 ms     | 32     | 9.6 MB     | 148.4 MB  |
| 2^15 = 32768  | 1216 KB | 154.4 ms  | 83.2 ms     | 16     | 13.9 MB    | 151.8 MB  |
| 2^16 = 65536  | 1202 KB | 149.5 ms  | 83.5 ms     | 8      | 22.5 MB    | 158.7 MB  |
| 2^17 = 131072 | 1197 KB | 148.2 ms  | 82.7 ms     | 4      | 39.8 MB    | 172.6 MB  |
| 2^18 = 262144 | 1194 KB | 147.7 ms  | 81.9 ms     | 2      | 74.3 MB    | 200.5 MB  |
| 2^19 = 524288 | 1193 KB | 146.1 ms  | 80.9 ms     | 1      | 143.3 MB   | 200.5 MB  |

## Filters

PcoPack skips deserializing non-matching rows: a 1%-selective filter pays ~20% of the full deserialize cost for most types.

| Filter                                               | Time (ms) | Rows   |
|------------------------------------------------------|-----------|--------|
| no filter                                            | 32.5 ms   | 100000 |
| empty filter                                         | 33.0 ms   | 100000 |
| i64 exact (id == 50_000)                             | 6.1 ms    | 1000   |
| i64 range (50_000..=50_999)                          | 6.1 ms    | 1000   |
| i64 inclusion (100 values, 1 match)                  | 6.3 ms    | 1000   |
| i32 exact (int32_val == 50)                          | 6.0 ms    | 1000   |
| i32 range (50..=50.99)                               | 6.1 ms    | 1000   |
| i32 inclusion (100 values, 1 match)                  | 6.2 ms    | 1000   |
| i8 exact (int8_val == 50)                            | 6.1 ms    | 1000   |
| i8 range (50..=50.99)                                | 6.0 ms    | 1000   |
| i8 inclusion (100 values, 1 match)                   | 6.2 ms    | 1000   |
| u8 exact (u8_val == 50)                              | 6.1 ms    | 1000   |
| u8 range (50..=50.99)                                | 6.1 ms    | 1000   |
| u8 inclusion (100 values, 1 match)                   | 6.2 ms    | 1000   |
| f64 exact (float64_val == 50.0)                      | 6.1 ms    | 1000   |
| f64 range (50.0..=50.99)                             | 6.1 ms    | 1000   |
| f64 inclusion (100 values, 1 match)                  | 8.5 ms    | 1000   |
| f32 exact (float32_val == 50.0)                      | 6.1 ms    | 1000   |
| f32 range (50.0..=50.99)                             | 6.1 ms    | 1000   |
| f32 inclusion (100 values, 1 match)                  | 8.7 ms    | 1000   |
| f16 exact (f16_val == 50.0)                          | 6.2 ms    | 1000   |
| f16 range (50.0..=50.99)                             | 6.1 ms    | 1000   |
| f16 inclusion (100 values, 1 match)                  | 8.8 ms    | 1000   |
| string exact                                         | 7.9 ms    | 1000   |
| string inclusion (10 values)                         | 8.0 ms    | 10000  |
| bool exact (bool_val == true)                        | 13.7 ms   | 1000   |
| bool exact (bool_val == false)                       | 32.4 ms   | 99000  |
| enum exact (status == V50)                           | 6.0 ms    | 1000   |
| enum inclusion (status in 0..9)                      | 6.2 ms    | 10000  |
| option exact (option_val == 50)                      | 6.0 ms    | 1000   |
| option range (50..=50.99)                            | 6.0 ms    | 1000   |
| option inclusion (100 values, 1 match)               | 6.1 ms    | 1000   |
| vec contains (vec_val has 5000)                      | 6.1 ms    | 1000   |
| vec contains inclusion (any of 10 values)            | 6.3 ms    | 10000  |
| bytes exact (hex match)                              | 7.5 ms    | 1000   |
| map exact (has key 'key_50')                         | 11.9 ms   | 1000   |
| map inclusion (any of key_0..key_9)                  | 12.3 ms   | 10000  |
| json exact (tag_50 string)                           | 8.0 ms    | 1000   |
| uuid exact (bucket 50)                               | 5.9 ms    | 1000   |
| uuid inclusion (100 values, 1 match)                 | 6.0 ms    | 1000   |
| nested (nested.inner_id == 50_000)                   | 6.0 ms    | 1000   |
| nested (nested.inner_id range 50_000..=50_000.99)    | 6.1 ms    | 1000   |
| nested (nested.inner_name exact)                     | 7.9 ms    | 1000   |
| nested (nested.inner_name inclusion, 10 values)      | 8.0 ms    | 10000  |
| partial fields (id + string_val)                     | 0.7 ms    | 1000   |
| multi-field (id + int32_val)                         | 6.1 ms    | 1000   |
| multi-field (bytes_val + u8_val)                     | 6.5 ms    | 1000   |
| multi-field (string_val + u8_val)                    | 6.7 ms    | 1000   |
| multi-field (json_val + u8_val)                      | 6.8 ms    | 1000   |
| multi-field zero-match (id + int32_val + string_val) | 0.4 ms    | 0      |

## Datasets

Real datasets from public sources.

### air_quality (823743 rows)

NYC AirData single-sensor stream: wide i16 measurements, a monotonic i64 counter, ns `DateTime` stored as us.

| Metric       | PcoPack  | PcoPack (lossy) | Vortex   | columnar | serde_columnar | msgpack  |
|--------------|----------|-----------------|----------|----------|----------------|----------|
| Size         | 4.1 MB   | 2.5 MB          | 4.1 MB   | 14.0 MB  | 5.4 MB         | 15.9 MB  |
| Serialize    | 106.2 ms | 102.7 ms        | 199.9 ms | 96.3 ms  | 120.2 ms       | 281.7 ms |
| Deserialize  | 57.0 ms  | 57.0 ms         | 66.2 ms  | 33.0 ms  | 55.9 ms        | 197.3 ms |
| Filter 0.18% | 6.3 ms   | 6.4 ms          | 0.5 ms   | 31.4 ms  | 54.9 ms        | 197.5 ms |
| Filter 4.49% | 8.0 ms   | 8.0 ms          | 3.2 ms   | 31.5 ms  | 55.0 ms        | 197.9 ms |
| Reduce 0.18% | 1.8 ms   | 1.8 ms          | 0.1 ms   | 33.3 ms  | 54.5 ms        | 197.2 ms |

- Full-fidelity size is tied between PcoPack and Vortex
- Lossy time rounding reduces size by 40%

### taxi (19144903 rows)

NYC taxi trips (also seen in the upstream pcodec benchmarks). All four timestamps are integer seconds and `float_round = 3` is lossless.

| Metric         | PcoPack  | PcoPack (lossy) | Vortex   | columnar | serde_columnar | msgpack  |
|----------------|----------|-----------------|----------|----------|----------------|----------|
| Size           | 285.5 MB | 275.7 MB        | 415.1 MB | 572.5 MB | 523.4 MB       | 661.5 MB |
| Serialize      | 8.7 s    | 8.2 s           | 19.7 s   | 7.4 s    | 8.5 s          | 10.9 s   |
| Deserialize    | 3.8 s    | 3.7 s           | 9.4 s    | 2.6 s    | 4.8 s          | 9.6 s    |
| Filter 0.2%    | 92.8 ms  | 92.8 ms         | 29.3 ms  | 2.5 s    | 4.9 s          | 10.0 s   |
| Filter 4.05%   | 216.9 ms | 210.7 ms        | 394.1 ms | 2.5 s    | 4.9 s          | 10.0 s   |
| Text eq 73.12% | 3.6 s    | 3.4 s           | 7.2 s    | 2.6 s    | 5.0 s          | 10.1 s   |
| Text eq 73.1%  | 3.6 s    | 3.4 s           | 7.2 s    | 2.6 s    | 5.0 s          | 10.1 s   |
| Reduce 4.05%   | 118.6 ms | 102.5 ms        | 81.5 ms  | 2.5 s    | 4.9 s          | 10.0 s   |

- PcoPack is the smallest by a wide margin
- For filtering, hour window favors Vortex (finer row-group pruning) and day window favors PcoPack
- pcodec reference (numeric columns only): pco L8 333.6 MB, parquet+zstd1 456.5 MB

### OpenStreetMap

Geofabrik Austria PBF. Filters are padded boxes on the centroid: cross-border geometry puts a measurable fraction of nodes outside the nominal bbox. Ways and relations have no natural scalar filter.

#### Node (85929052 rows)

| Metric       | PcoPack  | PcoPack (lossy) | Vortex   | columnar | serde_columnar | msgpack |
|--------------|----------|-----------------|----------|----------|----------------|---------|
| Size         | 847.6 MB | 351.4 MB        | 405.5 MB | 1.2 GB   | 1.1 GB         | 1.3 GB  |
| Serialize    | 6.2 s    | 6.0 s           | 41.9 s   | 6.5 s    | 6.3 s          | 7.3 s   |
| Deserialize  | 3.6 s    | 3.5 s           | 4.8 s    | 2.2 s    | 3.0 s          | 6.0 s   |
| Filter 0.18% | 714.8 ms | 755.9 ms        | 513.1 ms | 2.2 s    | 3.1 s          | 6.4 s   |
| Filter 4.66% | 1.1 s    | 1.2 s           | 2.1 s    | 2.2 s    | 3.1 s          | 6.4 s   |
| Reduce 0.18% | 463.9 ms | 502.3 ms        | 379.6 ms | 2.2 s    | 3.1 s          | 6.4 s   |

- Vortex automatically converts limited-precision floats into integers, which is why it outperforms PcoPack by default
- PcoPack (lossy) uses `float_round = 7`, cutting node size ~59% and only drops sub-centimeter coordinate precision

#### Way (9172339 rows)

| Metric      | PcoPack  | Vortex   | columnar | serde_columnar | msgpack  |
|-------------|----------|----------|----------|----------------|----------|
| Size        | 195.2 MB | 208.1 MB | 384.8 MB | 448.2 MB       | 478.4 MB |
| Serialize   | 6.6 s    | 13.2 s   | 4.0 s    | 3.4 s          | 4.0 s    |
| Deserialize | 4.1 s    | 4.1 s    | 1.5 s    | 3.2 s          | 3.5 s    |

#### Relation (175564 rows)

| Metric      | PcoPack  | Vortex   | columnar | serde_columnar | msgpack  |
|-------------|----------|----------|----------|----------------|----------|
| Size        | 17.8 MB  | 18.4 MB  | 19.5 MB  | 16.6 MB        | 19.1 MB  |
| Serialize   | 289.9 ms | 652.3 ms | 179.2 ms | 203.5 ms       | 188.1 ms |
| Deserialize | 197.8 ms | 274.8 ms | 56.6 ms  | 175.6 ms       | 236.7 ms |

### openalex (632276 works)

OpenAlex `works` snapshot: deeply nested JSON flattened to typed structs.

| Metric         | PcoPack  | PcoPack (lossy) | Vortex   | columnar | serde_columnar | msgpack  |
|----------------|----------|-----------------|----------|----------|----------------|----------|
| Size           | 322.4 MB | 316.0 MB        | 334.7 MB | 450.0 MB | 394.5 MB       | 434.6 MB |
| Serialize      | 9.1 s    | 9.4 s           | 12.2 s   | 4.8 s    | 5.7 s          | 4.1 s    |
| Deserialize    | 5.8 s    | 5.8 s           | 5.5 s    | 1.6 s    | 5.2 s          | 4.0 s    |
| Filter <0.01%  | 1.4 s    | 1.4 s           | 121.5 ms | 1.6 s    | 5.1 s          | 4.6 s    |
| Filter 2.96%   | 2.3 s    | 2.2 s           | 994.1 ms | 1.6 s    | 5.1 s          | 4.6 s    |
| Text eq 85.13% | 5.2 s    | 5.3 s           | 4.7 s    | 1.6 s    | 5.2 s          | 4.6 s    |
| Text eq <0.01% | 462.2 ms | 462.8 ms        | 69.2 ms  | 1.6 s    | 5.2 s          | 4.6 s    |
| Reduce 85.13%  | 88.0 ms  | 75.5 ms         | 11.3 ms  | 1.6 s    | 5.2 s          | 4.6 s    |

- PcoPack (lossy) `float_round = 4` saves 2% because `fwci` and scores carry ~15-digit model noise
- Vortex filters fastest (row-group pruning + predicate pushdown)
- At the production 262,144-row chunk, chunk min/max bounds rarely exclude a sparse-match chunk, so selective PcoPack filters decode nearly every blob (Filter <0.01% costs seconds, not tens of ms); sizing chunks to the capture is a possible future escape hatch
- Vortex compact output is not byte-deterministic run to run: its size varied 0.3% between the two passes

## Postgres

Real Postgres monitoring datasets sourced from pganalyze.

### backend_snapshots (13328012 rows)

#### PcoPack scenarios

| Metric        | full     | time_round | Timeline | Timeline 10s |
|---------------|----------|------------|----------|--------------|
| Size          | 47.1 MB  | 39.2 MB    | 18.0 MB  | 17.6 MB      |
| Serialize     | 2.9 s    | 2.9 s      | 1.6 s    | 1.1 s        |
| Deserialize   | 1.2 s    | 1.2 s      | 473.7 ms | 401.6 ms     |
| Filter 0.49%  | 64.1 ms  | 65.1 ms    | 156.2 ms | 83.8 ms      |
| Filter 29.25% | 707.4 ms | 730.0 ms   | 432.4 ms | 379.0 ms     |
| Filter 77.25% | 1.0 s    | 1.1 s      | 356.7 ms | 289.5 ms     |
| Reduce 0.49%  | 47.7 ms  | 47.9 ms    | 104.9 ms | 30.6 ms      |

- `time_round = 1s` drops the tick jitter and cuts ~17% off full fidelity
- `Timeline` cuts row count by ~71%, serialized size ~62%, and serialize time ~45%
- 10 second `Timeline` buckets trim only ~2% size, but significantly improve runtime

#### Other crates

| Metric        | Vortex   | columnar | serde_columnar | msgpack |
|---------------|----------|----------|----------------|---------|
| Size          | 83.8 MB  | 68.8 MB  | 69.3 MB        | 80.2 MB |
| Serialize     | 3.4 s    | 922.0 ms | 2.2 s          | 3.8 s   |
| Deserialize   | 1.5 s    | 511.8 ms | 1.5 s          | 4.3 s   |
| Filter 0.49%  | 11.5 ms  | 486.0 ms | 1.6 s          | 4.4 s   |
| Filter 29.25% | 716.6 ms | 487.1 ms | 1.6 s          | 4.5 s   |
| Filter 77.25% | 1.3 s    | 486.2 ms | 1.6 s          | 4.5 s   |
| Reduce 0.49%  | 2.1 ms   | 497.9 ms | 1.6 s          | 4.5 s   |

- PcoPack compresses ~40% smaller than competitors, with lossy/Timeline scenarios doing even better
- Vortex outperforms during some filter and reduce operations
- columnar deserializes fastest among the comparison crates

### query_explains (237651 rows)

This data model is dominated by large Postgres EXPLAIN plans represented as JSON.

#### PcoPack scenarios

| Metric        | text     | JSON     | JSON lossy | arena    | arena lossy |
|---------------|----------|----------|------------|----------|-------------|
| Size          | 91.7 MB  | 82.0 MB  | 76.0 MB    | 57.7 MB  | 55.6 MB     |
| Serialize     | 1.3 s    | 3.5 s    | 4.1 s      | 4.5 s    | 4.4 s       |
| Deserialize   | 749.8 ms | 8.5 s    | 8.3 s      | 1.9 s    | 1.9 s       |
| Filter 0.55%  | 567.0 ms | 1.7 s    | 1.7 s      | 522.7 ms | 514.9 ms    |
| Filter 12.48% | 572.8 ms | 4.6 s    | 4.6 s      | 763.8 ms | 757.2 ms    |
| Filter 12.97% | 23.9 ms  | 181.8 ms | 179.6 ms   | 44.7 ms  | 44.5 ms     |
| Text miss     | 7.3 ms   | 6.6 ms   | 6.5 ms     | 7.7 ms   | 7.8 ms      |
| Reduce 0.55%  | 2.9 ms   | 2.7 ms   | 2.8 ms     | 2.4 ms   | 2.5 ms      |

- PcoPack outperforms competitors even when storing the JSON as text. When storing JSON, PcoPack reduces compressed size but much more time is spent serializing.
- JSON lossy uses `float_round = 2, time_round = 1s` to cut ~7%
- The arena (AKA struct of arrays) replaces the JSON tree with a flat arena of typed fields. Arena is ~30% smaller than JSON, and lossy arena trims ~4% more. Arena deserialization is ~4x faster than JSON, and filters are 3-6x faster
- The whole capture fits one chunk, so chunk min/max bounds exclude nothing and selective filters decode every column (Filter 0.55% costs ~570 ms text / ~1.7 s JSON); dynamic chunk size by data volume is a possible future escape hatch

#### Other crates

| Metric        | Vortex   | columnar | serde_columnar | msgpack  |
|---------------|----------|----------|----------------|----------|
| Size          | 122.5 MB | 127.1 MB | 123.3 MB       | 110.3 MB |
| Serialize     | 5.0 s    | 1.5 s    | 1.6 s          | 2.1 s    |
| Deserialize   | 698.4 ms | 565.3 ms | 745.2 ms       | 5.0 s    |
| Filter 0.55%  | 17.4 ms  | 562.8 ms | 763.5 ms       | 6.2 s    |
| Filter 12.48% | 536.6 ms | 563.1 ms | 762.0 ms       | 6.2 s    |
| Filter 12.97% | 462.4 ms | 563.0 ms | 750.8 ms       | 6.2 s    |
| Text miss     | 0.2 ms   | 563.0 ms | 748.9 ms       | 6.2 s    |
| Reduce 0.55%  | 0.2 ms   | 567.0 ms | 749.6 ms       | 6.2 s    |

- Vortex, columnar, and serde_columnar store the JSON as text; the same as the PcoPack text scenario.

### query_stats_1min (29970299 rows)

| Metric        | PcoPack  | PcoPack time_round | Vortex   | columnar | serde_columnar | msgpack  |
|---------------|----------|--------------------|----------|----------|----------------|----------|
| Size          | 116.5 MB | 116.4 MB           | 129.9 MB | 191.7 MB | 179.4 MB       | 236.2 MB |
| Serialize     | 4.5 s    | 4.5 s              | 6.3 s    | 4.7 s    | 2.9 s          | 5.2 s    |
| Deserialize   | 1.9 s    | 1.8 s              | 2.3 s    | 1.5 s    | 2.4 s          | 4.4 s    |
| Filter 0.31%  | 153.2 ms | 139.7 ms           | 72.0 ms  | 1.5 s    | 2.4 s          | 4.5 s    |
| Filter 18.19% | 784.9 ms | 751.6 ms           | 1.0 s    | 1.5 s    | 2.4 s          | 4.5 s    |
| Filter 0.05%  | 34.4 ms  | 32.6 ms            | 16.3 ms  | 1.5 s    | 2.4 s          | 4.5 s    |
| Reduce 0.31%  | 71.3 ms  | 69.6 ms            | 15.2 ms  | 1.5 s    | 2.4 s          | 4.5 s    |

### query_stats_1hour (1041799 rows)

| Metric        | PcoPack  | PcoPack time_round | Vortex   | columnar | serde_columnar | msgpack  |
|---------------|----------|--------------------|----------|----------|----------------|----------|
| Size          | 5.9 MB   | 5.9 MB             | 7.1 MB   | 10.0 MB  | 9.1 MB         | 11.6 MB  |
| Serialize     | 169.8 ms | 169.4 ms           | 253.6 ms | 151.7 ms | 117.3 ms       | 201.3 ms |
| Deserialize   | 63.6 ms  | 65.2 ms            | 93.0 ms  | 52.8 ms  | 90.0 ms        | 165.2 ms |
| Filter 0.63%  | 25.4 ms  | 24.1 ms            | 2.8 ms   | 52.1 ms  | 89.5 ms        | 166.3 ms |
| Filter 58.45% | 47.5 ms  | 50.1 ms            | 72.7 ms  | 52.0 ms  | 89.5 ms        | 166.5 ms |
| Filter 0.03%  | 1.5 ms   | 1.4 ms             | 14.2 ms  | 51.6 ms  | 89.0 ms        | 166.5 ms |
| Reduce 0.63%  | 7.7 ms   | 7.4 ms             | 0.5 ms   | 52.7 ms  | 89.4 ms        | 165.5 ms |

- PcoPack is the smallest at both granularities
- `time_round` is less effective because data is captured once a minute versus backend_snapshots which are captured once every 10 seconds
