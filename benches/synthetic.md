# Synthetic benchmarks

Generated data: per-type compression, struct and enum round trips, timeline deduplication, chunk size effects, and filter timing. Run one scenario alone with `cargo bench --bench synthetic -- types`.

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
| `u8 (uniform, full width)`                  | 256 KB  | 256 KB  | 302 KB  | 258 KB  | 1.1 ms | 1.2 ms  | 1.5 ms  | 2.4 ms |
| `i16 (uniform, full width)`                 | 512 KB  | 512 KB  | 634 KB  | 514 KB  | 1.6 ms | 1.6 ms  | 1.5 ms  | 3.3 ms |
| `i32 (uniform, full width)`                 | 1024 KB | 1024 KB | 1175 KB | 1026 KB | 1.6 ms | 1.7 ms  | 1.8 ms  | 5.2 ms |
| `i64 (uniform, full width)`                 | 2.0 MB  | 2.0 MB  | 2.2 MB  | 2.0 MB  | 1.9 ms | 2.0 ms  | 2.7 ms  | 5.5 ms |
| `i16 (uniform 0..1000)`                     | 320 KB  | 320 KB  | 419 KB  | 322 KB  | 1.4 ms | 1.4 ms  | 3.0 ms  | 2.9 ms |
| `i32 (uniform 0..1000)`                     | 320 KB  | 320 KB  | 419 KB  | 322 KB  | 1.5 ms | 1.5 ms  | 3.1 ms  | 2.9 ms |
| `i64 (counter, monotonic)`                  | 181 KB  | 181 KB  | 346 KB  | 185 KB  | 2.0 ms | 2.1 ms  | 3.0 ms  | 6.6 ms |
| `i16 (counter, cyclic ~20% range)`          | 183 KB  | 183 KB  | 431 KB  | 186 KB  | 2.0 ms | 1.9 ms  | 2.8 ms  | 5.0 ms |
| `u8 (counter, cyclic 8-bit)`                | 181 KB  | 181 KB  | 271 KB  | 258 KB  | 1.8 ms | 1.8 ms  | 1.5 ms  | 2.3 ms |
| `i64 (right-skew, sparse variant)`          | 138 KB  | 138 KB  | 155 KB  | 141 KB  | 1.9 ms | 1.9 ms  | 1.3 ms  | 5.1 ms |
| `i64 (right-skew)`                          | 140 KB  | 140 KB  | 158 KB  | 143 KB  | 1.8 ms | 1.9 ms  | 1.3 ms  | 5.1 ms |
| `i64 (93% zeros, scattered)`                | 25 KB   | 25 KB   | 39 KB   | 28 KB   | 1.3 ms | 1.4 ms  | 0.9 ms  | 4.8 ms |
| `i64 (93% zeros, clustered windows)`        | 26 KB   | 26 KB   | 15 KB   | 17 KB   | 1.3 ms | 1.3 ms  | 0.7 ms  | 3.7 ms |
| `u8 (tiny alphabet i.i.d., d=4)`            | 35 KB   | 35 KB   | 55 KB   | 66 KB   | 0.8 ms | 0.8 ms  | 0.6 ms  | 2.9 ms |
| `u8 (dwell sweep, d=3, run~16)`             | 14 KB   | 14 KB   | 12 KB   | 14 KB   | 1.0 ms | 1.0 ms  | 0.4 ms  | 2.8 ms |
| `u8 (dwell sweep, d=3, run~256)`            | 1 KB    | 1 KB    | 1 KB    | 4 KB    | 1.1 ms | 1.1 ms  | 0.3 ms  | 2.9 ms |
| `u8 (dwell sweep, d=3, run~4096)`           | 235 B   | 235 B   | 274 B   | 3 KB    | 1.1 ms | 1.1 ms  | 0.3 ms  | 2.7 ms |
| `i64 (d-sweep, d=4)`                        | 64 KB   | 64 KB   | 220 KB  | 67 KB   | 1.3 ms | 1.3 ms  | 3.5 ms  | 4.7 ms |
| `i64 (d-sweep, d=35)`                       | 164 KB  | 164 KB  | 375 KB  | 167 KB  | 2.1 ms | 2.1 ms  | 4.1 ms  | 4.7 ms |
| `i64 (d-sweep, d=86)`                       | 206 KB  | 206 KB  | 422 KB  | 209 KB  | 2.4 ms | 2.4 ms  | 4.1 ms  | 5.0 ms |
| `i64 (d-sweep, d=1024)`                     | 375 KB  | 375 KB  | 544 KB  | 330 KB  | 7.7 ms | 7.7 ms  | 4.8 ms  | 3.7 ms |
| `i64 (d-sweep, d=8191)`                     | 559 KB  | 559 KB  | 681 KB  | 2.0 MB  | 8.2 ms | 8.3 ms  | 6.2 ms  | 3.3 ms |
| `i64 (d-sweep, d=8193)`                     | 558 KB  | 558 KB  | 681 KB  | 2.0 MB  | 8.1 ms | 8.2 ms  | 6.3 ms  | 3.3 ms |
| `i64 (d-sweep, d=32768)`                    | 1194 KB | 1194 KB | 1195 KB | 2.0 MB  | 7.9 ms | 8.1 ms  | 7.3 ms  | 3.8 ms |
| `i64 (placement pair, d=574 grouped)`       | 5 KB    | 5 KB    | 6 KB    | 8 KB    | 1.9 ms | 1.9 ms  | 0.9 ms  | 3.0 ms |
| `i64 (placement pair, d=574 shuffled)`      | 336 KB  | 336 KB  | 503 KB  | 278 KB  | 7.0 ms | 7.0 ms  | 4.4 ms  | 5.8 ms |
| `i64 (recurrence pair, d=4096 local-block)` | 174 KB  | 174 KB  | 315 KB  | 154 KB  | 6.1 ms | 6.1 ms  | 4.0 ms  | 5.5 ms |
| `i64 (recurrence pair, d=4096 full-cycle)`  | 40 KB   | 40 KB   | 35 KB   | 35 KB   | 5.7 ms | 5.7 ms  | 1.3 ms  | 4.6 ms |
| `i64 (d=64 pool, N=2k tail chunk)`          | 2 KB    | 2 KB    | 3 KB    | 4 KB    | 0.0 ms | 0.0 ms  | 0.0 ms  | 0.3 ms |
| `i64 (d=64 pool, full writer chunk)`        | 192 KB  | 192 KB  | 407 KB  | 195 KB  | 2.2 ms | 2.2 ms  | 4.1 ms  | 3.3 ms |

### Booleans

| Type                               | pco   | PcoPack | msgpack | Vortex | pco    | PcoPack | msgpack | Vortex |
|------------------------------------|-------|---------|---------|--------|--------|---------|---------|--------|
| `bool (i.i.d. 50% true)`           | 12 KB | 18 KB   | 18 KB   | 14 KB  | 0.1 ms | 0.2 ms  | 0.2 ms  | 1.1 ms |
| `bool (i.i.d. 90% true)`           | 5 KB  | 12 KB   | 12 KB   | 14 KB  | 0.3 ms | 0.2 ms  | 0.2 ms  | 0.9 ms |
| `bool (clustered runs)`            | 1 KB  | 1 KB    | 1 KB    | 14 KB  | 0.4 ms | 0.1 ms  | 0.1 ms  | 0.9 ms |
| `bool (alternating, period 2)`     | 12 KB | 24 B    | 24 B    | 14 KB  | 0.1 ms | 0.1 ms  | 0.1 ms  | 0.9 ms |
| `bool (i.i.d. 5% true)`            | 3 KB  | 7 KB    | 7 KB    | 14 KB  | 0.3 ms | 0.2 ms  | 0.2 ms  | 0.9 ms |
| `bool (clustered, ~20% true runs)` | 3 KB  | 6 KB    | 6 KB    | 14 KB  | 0.4 ms | 0.2 ms  | 0.2 ms  | 0.9 ms |

### Option<i32> vs sentinel

| Type                                 | PcoPack | msgpack | Vortex | PcoPack | msgpack | Vortex |
|--------------------------------------|---------|---------|--------|---------|---------|--------|
| `Option<i32> (80% null, clustered)`  | 10 KB   | 12 KB   | 26 KB  | 0.3 ms  | 0.2 ms  | 1.2 ms |
| `i32 (80% zero sentinel, clustered)` | 20 KB   | 12 KB   | 14 KB  | 0.5 ms  | 0.3 ms  | 1.6 ms |
| `Option<i32> (80% null, scattered)`  | 24 KB   | 33 KB   | 29 KB  | 0.4 ms  | 0.4 ms  | 1.7 ms |
| `i32 (80% zero sentinel, scattered)` | 24 KB   | 33 KB   | 26 KB  | 0.6 ms  | 0.4 ms  | 1.7 ms |
| `Option<i32> (90% null, clustered)`  | 5 KB    | 6 KB    | 20 KB  | 0.2 ms  | 0.2 ms  | 1.1 ms |
| `i32 (90% zero sentinel, clustered)` | 11 KB   | 6 KB    | 8 KB   | 0.5 ms  | 0.3 ms  | 1.4 ms |
| `Option<i32> (90% null, scattered)`  | 13 KB   | 20 KB   | 22 KB  | 0.3 ms  | 0.3 ms  | 1.4 ms |
| `i32 (90% zero sentinel, scattered)` | 13 KB   | 20 KB   | 16 KB  | 0.5 ms  | 0.3 ms  | 1.9 ms |

### Enums vs enum-as-integer

| Type                                                 | pco   | PcoPack | msgpack | Vortex | pco    | PcoPack | msgpack | Vortex |
|------------------------------------------------------|-------|---------|---------|--------|--------|---------|---------|--------|
| `Enum (simple, skewed 68/27/5)`                      | -     | 13 KB   | 55 KB   | 16 KB  | -      | 0.6 ms  | 0.8 ms  | 5.1 ms |
| `u8 (skewed 68/27/5, plain integer)`                 | 13 KB | 13 KB   | 21 KB   | 26 KB  | 0.3 ms | 0.3 ms  | 0.2 ms  | 1.1 ms |
| `Enum (complex, repeating pattern + payloads)`       | -     | 96 KB   | 180 KB  | -      | -      | 1.8 ms  | 1.8 ms  | -      |
| `u8 (repeating-pattern discriminant, plain integer)` | 24 KB | 24 KB   | 17 KB   | 26 KB  | 0.2 ms | 0.2 ms  | 0.2 ms  | 1.0 ms |

Note: Vortex does not currently support complex enums.

### Strings & bytes

| Type                                          | PcoPack | msgpack | Vortex  | PcoPack | msgpack | Vortex  |
|-----------------------------------------------|---------|---------|---------|---------|---------|---------|
| `String`                                      | 291 KB  | 288 KB  | 328 KB  | 3.9 ms  | 2.6 ms  | 11.8 ms |
| `SmolStr`                                     | 291 KB  | 288 KB  | 328 KB  | 2.8 ms  | 2.6 ms  | 11.8 ms |
| `Uuid (v4 random)`                            | 1692 KB | 1692 KB | 2001 KB | 1.5 ms  | 1.3 ms  | 27.5 ms |
| `ByteBuf (avg 128B)`                          | 12.4 MB | 12.4 MB | 12.3 MB | 10.4 ms | 8.5 ms  | 28.4 ms |
| `Vec<String> (avg 4)`                         | 139 KB  | 138 KB  | 179 KB  | 7.5 ms  | 4.0 ms  | 12.3 ms |
| `serde_json::Value`                           | 635 KB  | 632 KB  | -       | 8.6 ms  | 4.1 ms  | -       |
| `serde_json::Value (5 stable keys, 50k rows)` | 301 KB  | 299 KB  | 124 KB  | 9.8 ms  | 4.1 ms  | 9.8 ms  |

Vortex supports stable-keyed JSON using struct promotion, but does not support heterogeneous JSON.

### Maps

| Type                                                             | PcoPack | msgpack | Vortex | PcoPack  | msgpack | Vortex  |
|------------------------------------------------------------------|---------|---------|--------|----------|---------|---------|
| `BTreeMap<String, i32>`                                          | 125 KB  | 123 KB  | 137 KB | 1.7 ms   | 1.1 ms  | 3.8 ms  |
| `HashMap<String, i32>`                                           | 125 KB  | 130 KB  | 141 KB | 2.4 ms   | 1.1 ms  | 3.7 ms  |
| `BTreeMap<SmolStr, i32>`                                         | 125 KB  | 123 KB  | 137 KB | 1.5 ms   | 3.5 ms  | 3.7 ms  |
| `HashMap<SmolStr, i32>`                                          | 125 KB  | 130 KB  | 141 KB | 2.2 ms   | 3.9 ms  | 3.8 ms  |
| `HashMap<SmolStr, i32> (8 stable keys)`                          | 43 KB   | 148 KB  | 51 KB  | 2.9 ms   | 1.8 ms  | 4.2 ms  |
| `HashMap<SmolStr, Option<i32>> (8 stable keys, ~60% null)`       | 26 KB   | 127 KB  | 36 KB  | 3.0 ms   | 1.7 ms  | 4.2 ms  |
| `HashMap<SmolStr, i32> (8 stable keys, walk)`                    | 115 KB  | 3.3 MB  | 114 KB | 104.8 ms | 49.6 ms | 61.9 ms |
| `HashMap<SmolStr, Option<i32>> (8 stable keys, ~60% null, walk)` | 852 KB  | 3.4 MB  | 352 KB | 90.3 ms  | 49.6 ms | 78.8 ms |

First four rows use native `vortex.map`, the rest struct promotion. Struct promotion requires string keys from a stable vocabulary.

- Tuple keys cannot promote, and promoting an open vocabulary silently drops entries the sampled schema missed, so random-key maps stay native. Native pays ~10% over PcoPack because keys live in the payload, not the schema.
- Stable keys beat PcoPack once rows move: dense walk ties, sparse walk lands ~60% smaller. At 10k static rows PcoPack's flat streams still edge ahead; zstd absorbs the repeated keys.
- Native maps interleave all keys into one stream, losing the per-key runs and deltas that make promotion win.

### Floats (float_round)

| Type                          | pco    | PcoPack | msgpack | Vortex | pco    | PcoPack | msgpack | Vortex |
|-------------------------------|--------|---------|---------|--------|--------|---------|---------|--------|
| `f64 (normal, no round)`      | 650 KB | 650 KB  | 792 KB  | 653 KB | 1.5 ms | 1.6 ms  | 0.7 ms  | 4.2 ms |
| `f64 (normal, float_round=1)` | 650 KB | 65 KB   | 792 KB  | 653 KB | 1.5 ms | 0.7 ms  | 0.7 ms  | 4.2 ms |
| `f64 (normal, float_round=2)` | 650 KB | 106 KB  | 792 KB  | 653 KB | 1.5 ms | 0.9 ms  | 0.7 ms  | 4.2 ms |
| `f32 (normal, no round)`      | 296 KB | 296 KB  | 391 KB  | 299 KB | 1.5 ms | 1.4 ms  | 0.4 ms  | 4.5 ms |
| `f32 (normal, float_round=1)` | 296 KB | 65 KB   | 391 KB  | 299 KB | 1.5 ms | 0.7 ms  | 0.4 ms  | 4.5 ms |
| `f32 (normal, float_round=2)` | 296 KB | 106 KB  | 391 KB  | 299 KB | 1.5 ms | 0.9 ms  | 0.4 ms  | 4.5 ms |

`float_round` discards unwanted float precision, intentionally causing data loss to significantly reduce compressed size.

### Timestamps (time_round)

| Type                                  | pco    | PcoPack | msgpack | Vortex | pco    | PcoPack | msgpack | Vortex  |
|---------------------------------------|--------|---------|---------|--------|--------|---------|---------|---------|
| `DateTime (60s step, no round)`       | 313 KB | 313 KB  | 825 KB  | 836 KB | 1.1 ms | 1.3 ms  | 12.8 ms | 32.6 ms |
| `DateTime (60s step, time_round=1s)`  | 313 KB | 69 KB   | 825 KB  | 836 KB | 1.1 ms | 1.2 ms  | 12.8 ms | 32.6 ms |
| `DateTime (60s step, time_round=60s)` | 313 KB | 14 KB   | 825 KB  | 836 KB | 1.1 ms | 0.9 ms  | 12.8 ms | 32.6 ms |

`time_round` cuts PcoPack's column ~78% at 1s, ~95% at 60s. pco stores i64 microseconds (1:1). Vortex has no time type: `DateTime` is an RFC3339 string.

## Structs

500k rows of a realistic struct: low-cardinality `account_id` index key, random names, log-normal scores, a clustered boolean flag.

- PcoPack is ~5% smaller than Vortex and ~35% faster round trip
- columnar deserializes fastest but has the second-worst compressed size
- Vortex outperforms PcoPack at both filter and reduce scenarios

| Metric           | PcoPack | Vortex   | columnar | serde_columnar | msgpack |
|------------------|---------|----------|----------|----------------|---------|
| Size             | 6.2 MB  | 6.5 MB   | 8.1 MB   | 7.1 MB         | 9.9 MB  |
| Serialize        | 78.1 ms | 123.3 ms | 57.4 ms  | 61.8 ms        | 83.2 ms |
| Deserialize      | 37.6 ms | 50.7 ms  | 19.2 ms  | 36.9 ms        | 68.0 ms |
| Filter 20%       | 7.9 ms  | 1.4 ms   | 20.5 ms  | 37.8 ms        | 73.4 ms |
| Filter <0.01%    | 3.2 ms  | 0.7 ms   | 21.2 ms  | 38.1 ms        | 75.5 ms |
| Text eq (1 rows) | 15.7 ms | 5.5 ms   | 21.1 ms  | 37.9 ms        | 76.5 ms |
| Text miss        | 14.1 ms | 0.0 ms   | 20.4 ms  | 37.6 ms        | 74.6 ms |
| Text IN (2 rows) | 16.8 ms | 11.4 ms  | 22.0 ms  | 38.8 ms        | 75.1 ms |
| Reduce 70.04%    | 12.6 ms | 5.1 ms   | 23.7 ms  | 40.9 ms        | 78.6 ms |
| Reduce <0.01%    | 1.6 ms  | 0.7 ms   | 21.4 ms  | 39.0 ms        | 77.9 ms |

## Enums

500k rows of a top-level enum: a unit `Idle` variant, a ~10% `Tick` variant carrying sensor/value plus a never-read json blob, and a ~60% `Note` variant carrying author/text.

| Metric                  | PcoPack | Vortex  | columnar | serde_columnar | msgpack |
|-------------------------|---------|---------|----------|----------------|---------|
| Size                    | 1554 KB | 1743 KB | 2.2 MB   | 1579 KB        | 2.1 MB  |
| Serialize               | 30.0 ms | 97.5 ms | 34.7 ms  | 41.0 ms        | 23.1 ms |
| Deserialize             | 34.1 ms | 29.2 ms | 21.6 ms  | 25.5 ms        | 30.0 ms |
| Filter Tick 10%         | 8.4 ms  | 0.1 ms  | 22.3 ms  | 26.3 ms        | 30.2 ms |
| Filter Note 60%         | 25.0 ms | 0.1 ms  | 22.4 ms  | 26.1 ms        | 30.3 ms |
| Filter Tick or Note 70% | 33.8 ms | 0.1 ms  | 22.4 ms  | 26.2 ms        | 30.4 ms |
| Reduce Tick 10%         | 33.6 ms | 1.7 ms  | 22.1 ms  | 26.6 ms        | 30.2 ms |
| Reduce Note 60%         | 35.8 ms | 17.4 ms | 26.5 ms  | 31.1 ms        | 32.4 ms |

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
| 10%        | 1.3 ms   | 6.8 ms   | 6.5 ms        | 5.2x               | 1.05x              |
| 20%        | 2.2 ms   | 7.0 ms   | 6.6 ms        | 3.2x               | 1.06x              |
| 50%        | 5.1 ms   | 6.3 ms   | 5.9 ms        | 1.2x               | 1.07x              |
| 80%        | 7.9 ms   | 4.3 ms   | 4.1 ms        | 0.5x               | 1.05x              |
| 90%        | 9.2 ms   | 4.4 ms   | 4.1 ms        | 0.5x               | 1.07x              |

## chunk_size

`chunk_size` caps columnar-to-row conversion memory. Bigger chunks compress better and serialize faster for more RAM; the 2^18 default is the sweet spot. `Write peak`/`Read peak` are per-phase peaks.

### SmallStruct (32 bytes/row)

| Chunk size    | Size   | Serialize | Deserialize | Chunks | Write peak | Read peak |
|---------------|--------|-----------|-------------|--------|------------|-----------|
| 2^13 = 8192   | 157 KB | 22.9 ms   | 8.1 ms      | 64     | 6.0 MB     | 24.4 MB   |
| 2^14 = 16384  | 147 KB | 20.2 ms   | 7.9 ms      | 32     | 6.0 MB     | 24.6 MB   |
| 2^15 = 32768  | 142 KB | 17.6 ms   | 7.5 ms      | 16     | 6.5 MB     | 25.1 MB   |
| 2^16 = 65536  | 139 KB | 16.2 ms   | 7.3 ms      | 8      | 8.8 MB     | 26.1 MB   |
| 2^17 = 131072 | 138 KB | 15.5 ms   | 7.2 ms      | 4      | 13.4 MB    | 28.1 MB   |
| 2^18 = 262144 | 138 KB | 15.9 ms   | 7.3 ms      | 2      | 22.6 MB    | 32.1 MB   |
| 2^19 = 524288 | 137 KB | 15.9 ms   | 7.3 ms      | 1      | 27.1 MB    | 32.1 MB   |

### MediumStruct (80 bytes/row)

| Chunk size    | Size   | Serialize | Deserialize | Chunks | Write peak | Read peak |
|---------------|--------|-----------|-------------|--------|------------|-----------|
| 2^13 = 8192   | 683 KB | 64.6 ms   | 39.5 ms     | 64     | 6.0 MB     | 65.9 MB   |
| 2^14 = 16384  | 665 KB | 59.8 ms   | 39.0 ms     | 32     | 6.7 MB     | 66.6 MB   |
| 2^15 = 32768  | 773 KB | 58.6 ms   | 38.7 ms     | 16     | 8.8 MB     | 68.3 MB   |
| 2^16 = 65536  | 570 KB | 53.7 ms   | 38.0 ms     | 8      | 12.6 MB    | 71.1 MB   |
| 2^17 = 131072 | 555 KB | 52.5 ms   | 37.8 ms     | 4      | 20.5 MB    | 77.2 MB   |
| 2^18 = 262144 | 661 KB | 55.1 ms   | 37.6 ms     | 2      | 36.4 MB    | 89.5 MB   |
| 2^19 = 524288 | 714 KB | 55.9 ms   | 38.2 ms     | 1      | 68.0 MB    | 89.6 MB   |

### LargeStruct (176 bytes/row)

| Chunk size    | Size    | Serialize | Deserialize | Chunks | Write peak | Read peak |
|---------------|---------|-----------|-------------|--------|------------|-----------|
| 2^13 = 8192   | 1309 KB | 167.0 ms  | 85.5 ms     | 64     | 7.2 MB     | 146.7 MB  |
| 2^14 = 16384  | 1246 KB | 152.2 ms  | 83.2 ms     | 32     | 8.9 MB     | 148.4 MB  |
| 2^15 = 32768  | 1216 KB | 146.5 ms  | 82.4 ms     | 16     | 12.5 MB    | 151.8 MB  |
| 2^16 = 65536  | 1202 KB | 145.0 ms  | 83.0 ms     | 8      | 19.6 MB    | 158.7 MB  |
| 2^17 = 131072 | 1197 KB | 148.0 ms  | 82.8 ms     | 4      | 34.0 MB    | 172.6 MB  |
| 2^18 = 262144 | 1194 KB | 146.0 ms  | 81.5 ms     | 2      | 62.7 MB    | 200.5 MB  |
| 2^19 = 524288 | 1193 KB | 143.9 ms  | 81.1 ms     | 1      | 120.2 MB   | 200.5 MB  |

## Filters

PcoPack skips deserializing non-matching rows: a 1%-selective filter pays ~20% of the full deserialize cost for most types.

| Filter                                               | Time (ms) | Rows   |
|------------------------------------------------------|-----------|--------|
| no filter                                            | 33.4 ms   | 100000 |
| empty filter                                         | 32.9 ms   | 100000 |
| i64 exact (id == 50_000)                             | 6.0 ms    | 1000   |
| i64 range (50_000..=50_999)                          | 6.0 ms    | 1000   |
| i64 inclusion (100 values, 1 match)                  | 6.1 ms    | 1000   |
| i32 exact (int32_val == 50)                          | 6.0 ms    | 1000   |
| i32 range (50..=50.99)                               | 6.0 ms    | 1000   |
| i32 inclusion (100 values, 1 match)                  | 6.1 ms    | 1000   |
| i8 exact (int8_val == 50)                            | 6.0 ms    | 1000   |
| i8 range (50..=50.99)                                | 6.0 ms    | 1000   |
| i8 inclusion (100 values, 1 match)                   | 6.1 ms    | 1000   |
| u8 exact (u8_val == 50)                              | 6.0 ms    | 1000   |
| u8 range (50..=50.99)                                | 6.0 ms    | 1000   |
| u8 inclusion (100 values, 1 match)                   | 6.1 ms    | 1000   |
| f64 exact (float64_val == 50.0)                      | 5.9 ms    | 1000   |
| f64 range (50.0..=50.99)                             | 5.9 ms    | 1000   |
| f64 inclusion (100 values, 1 match)                  | 8.4 ms    | 1000   |
| f32 exact (float32_val == 50.0)                      | 6.0 ms    | 1000   |
| f32 range (50.0..=50.99)                             | 6.0 ms    | 1000   |
| f32 inclusion (100 values, 1 match)                  | 8.6 ms    | 1000   |
| f16 exact (f16_val == 50.0)                          | 6.0 ms    | 1000   |
| f16 range (50.0..=50.99)                             | 6.0 ms    | 1000   |
| f16 inclusion (100 values, 1 match)                  | 8.7 ms    | 1000   |
| string exact                                         | 7.7 ms    | 1000   |
| string inclusion (10 values)                         | 8.0 ms    | 10000  |
| bool exact (bool_val == true)                        | 14.0 ms   | 1000   |
| bool exact (bool_val == false)                       | 32.7 ms   | 99000  |
| enum exact (status == V50)                           | 6.0 ms    | 1000   |
| enum inclusion (status in 0..9)                      | 6.1 ms    | 10000  |
| option exact (option_val == 50)                      | 6.0 ms    | 1000   |
| option range (50..=50.99)                            | 6.0 ms    | 1000   |
| option inclusion (100 values, 1 match)               | 6.1 ms    | 1000   |
| vec contains (vec_val has 5000)                      | 6.1 ms    | 1000   |
| vec contains inclusion (any of 10 values)            | 6.3 ms    | 10000  |
| bytes exact (hex match)                              | 7.6 ms    | 1000   |
| map exact (has key 'key_50')                         | 11.7 ms   | 1000   |
| map inclusion (any of key_0..key_9)                  | 12.1 ms   | 10000  |
| json exact (tag_50 string)                           | 8.1 ms    | 1000   |
| uuid exact (bucket 50)                               | 5.9 ms    | 1000   |
| uuid inclusion (100 values, 1 match)                 | 6.1 ms    | 1000   |
| nested (nested.inner_id == 50_000)                   | 6.0 ms    | 1000   |
| nested (nested.inner_id range 50_000..=50_000.99)    | 5.9 ms    | 1000   |
| nested (nested.inner_name exact)                     | 7.8 ms    | 1000   |
| nested (nested.inner_name inclusion, 10 values)      | 7.9 ms    | 10000  |
| partial fields (id + string_val)                     | 0.7 ms    | 1000   |
| multi-field (id + int32_val)                         | 6.0 ms    | 1000   |
| multi-field (bytes_val + u8_val)                     | 6.5 ms    | 1000   |
| multi-field (string_val + u8_val)                    | 6.7 ms    | 1000   |
| multi-field (json_val + u8_val)                      | 6.7 ms    | 1000   |
| multi-field zero-match (id + int32_val + string_val) | 0.4 ms    | 0      |
