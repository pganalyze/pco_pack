# Benchmarks

All serialization times and sizes include compression (either Pcodec or zstd). These numbers are captured on an M5 Max MBP with fans set to max. There is a lot of run-to-run variance; [contributions are welcome](https://github.com/pganalyze/pco_pack/issues/3) to help smooth out the data.

## Types

PcoPack vs standalone Pcodec vs msgpack+zstd vs Vortex, per field type and distribution. All rows run at PcoPack's default 262k-row chunk size. Pcodec uses zero-config defaults plus 8-bit types. msgpack+zstd uses the same chunking. Vortex is one compact btrblocks file per column, with "-" where the pipeline can't express the type: complex enums and heterogeneous-shape `serde_json::Value`. Map rows declare their Vortex layout: native `vortex.map` for open tuple-key vocabularies, struct-promoted columns for stable vocabularies.

Every Numbers row is a synthetic shape that isolates exactly one axis: entropy floor, type-agnosticism, delta, skew, sparsity and placement, alphabet and dwell length, distinct count, multiset placement, recurrence distance, chunk fixed costs. Real-column bytes live in the [Postgres](#postgres) section.

The `pco` column appears only where Pcodec compresses without transformation (plain numbers, bool -> u8, enum discriminants). PcoPack's structural decompositions (`Option` -> present+values, `Vec` -> lengths+values, dynamic JSON) have no standalone-Pcodec counterpart.

Where each codec wins:
- Frequency/delta redundancy (counters, skew, i.i.d. pools): pco/PcoPack up to ~70% smaller than msgpack
- Sequential redundancy (dwell runs, clustered zeros, grouped placement): msgpack+zstd ties or beats pco; long dwells collapse every codec to hundreds of bytes
- Placement decides: one multiset, grouped vs shuffled, 5-6 KB vs 336-503 KB
- Incompressible (uniform full-width): pco/PcoPack/Vortex tie within ~1%; msgpack pays its framing tax

### Numbers

| Type                                        | pco     | PcoPack | msgpack | Vortex  | pco    | PcoPack | msgpack | Vortex |
|---------------------------------------------|---------|---------|---------|---------|--------|---------|---------|--------|
| `u8 (uniform, full width)`                  | 256 KB  | 256 KB  | 302 KB  | 258 KB  | 1.2 ms | 1.8 ms  | 1.6 ms  | 2.5 ms |
| `i16 (uniform, full width)`                 | 512 KB  | 512 KB  | 634 KB  | 514 KB  | 1.6 ms | 1.7 ms  | 1.6 ms  | 3.5 ms |
| `i32 (uniform, full width)`                 | 1024 KB | 1024 KB | 1175 KB | 1026 KB | 1.7 ms | 1.8 ms  | 2.0 ms  | 5.3 ms |
| `i64 (uniform, full width)`                 | 2.0 MB  | 2.0 MB  | 2.2 MB  | 2.0 MB  | 2.0 ms | 2.1 ms  | 3.0 ms  | 5.7 ms |
| `i16 (uniform 0..1000)`                     | 320 KB  | 320 KB  | 419 KB  | 322 KB  | 1.5 ms | 1.5 ms  | 3.3 ms  | 3.1 ms |
| `i32 (uniform 0..1000)`                     | 320 KB  | 320 KB  | 419 KB  | 322 KB  | 1.5 ms | 1.6 ms  | 3.3 ms  | 3.2 ms |
| `i64 (counter, monotonic)`                  | 181 KB  | 181 KB  | 346 KB  | 185 KB  | 2.2 ms | 2.2 ms  | 3.2 ms  | 7.0 ms |
| `i16 (counter, cyclic ~20% range)`          | 183 KB  | 183 KB  | 431 KB  | 185 KB  | 2.0 ms | 2.0 ms  | 3.1 ms  | 5.2 ms |
| `u8 (counter, cyclic 8-bit)`                | 181 KB  | 181 KB  | 271 KB  | 258 KB  | 1.9 ms | 2.0 ms  | 1.6 ms  | 2.5 ms |
| `i64 (right-skew, sparse variant)`          | 138 KB  | 138 KB  | 155 KB  | 141 KB  | 1.9 ms | 1.9 ms  | 1.6 ms  | 5.1 ms |
| `i64 (right-skew)`                          | 140 KB  | 140 KB  | 158 KB  | 143 KB  | 1.9 ms | 2.0 ms  | 1.6 ms  | 5.6 ms |
| `i64 (93% zeros, scattered)`                | 25 KB   | 25 KB   | 39 KB   | 28 KB   | 1.4 ms | 1.5 ms  | 1.1 ms  | 5.3 ms |
| `i64 (93% zeros, clustered windows)`        | 26 KB   | 26 KB   | 15 KB   | 17 KB   | 1.4 ms | 1.4 ms  | 0.9 ms  | 4.4 ms |
| `u8 (tiny alphabet i.i.d., d=4)`            | 35 KB   | 35 KB   | 55 KB   | 66 KB   | 0.9 ms | 0.9 ms  | 0.7 ms  | 3.3 ms |
| `u8 (dwell sweep, d=3, run~16)`             | 14 KB   | 14 KB   | 12 KB   | 14 KB   | 1.1 ms | 1.1 ms  | 0.5 ms  | 3.4 ms |
| `u8 (dwell sweep, d=3, run~256)`            | 1 KB    | 1 KB    | 1 KB    | 4 KB    | 1.2 ms | 1.2 ms  | 0.3 ms  | 3.7 ms |
| `u8 (dwell sweep, d=3, run~4096)`           | 235 B   | 235 B   | 274 B   | 3 KB    | 1.2 ms | 1.2 ms  | 0.3 ms  | 3.3 ms |
| `i64 (d-sweep, d=4)`                        | 64 KB   | 64 KB   | 220 KB  | 67 KB   | 1.4 ms | 1.4 ms  | 3.7 ms  | 5.3 ms |
| `i64 (d-sweep, d=35)`                       | 164 KB  | 164 KB  | 375 KB  | 167 KB  | 2.2 ms | 2.2 ms  | 4.3 ms  | 5.0 ms |
| `i64 (d-sweep, d=86)`                       | 206 KB  | 206 KB  | 422 KB  | 209 KB  | 2.4 ms | 2.5 ms  | 4.4 ms  | 5.3 ms |
| `i64 (d-sweep, d=1024)`                     | 375 KB  | 375 KB  | 544 KB  | 330 KB  | 8.0 ms | 8.0 ms  | 5.1 ms  | 4.2 ms |
| `i64 (d-sweep, d=8191)`                     | 559 KB  | 559 KB  | 681 KB  | 2.0 MB  | 8.0 ms | 8.0 ms  | 6.7 ms  | 3.5 ms |
| `i64 (d-sweep, d=8193)`                     | 558 KB  | 558 KB  | 681 KB  | 2.0 MB  | 8.0 ms | 8.0 ms  | 6.6 ms  | 3.6 ms |
| `i64 (d-sweep, d=32768)`                    | 1194 KB | 1194 KB | 1195 KB | 2.0 MB  | 7.9 ms | 7.9 ms  | 7.7 ms  | 4.0 ms |
| `i64 (placement pair, d=574 grouped)`       | 5 KB    | 5 KB    | 6 KB    | 8 KB    | 2.0 ms | 2.0 ms  | 1.0 ms  | 3.4 ms |
| `i64 (placement pair, d=574 shuffled)`      | 336 KB  | 336 KB  | 503 KB  | 278 KB  | 7.6 ms | 7.6 ms  | 4.6 ms  | 6.2 ms |
| `i64 (recurrence pair, d=4096 local-block)` | 174 KB  | 174 KB  | 315 KB  | 154 KB  | 7.0 ms | 7.0 ms  | 4.2 ms  | 5.9 ms |
| `i64 (recurrence pair, d=4096 full-cycle)`  | 40 KB   | 40 KB   | 35 KB   | 35 KB   | 6.3 ms | 6.3 ms  | 1.5 ms  | 5.0 ms |
| `i64 (d=64 pool, N=2k tail chunk)`          | 2 KB    | 2 KB    | 3 KB    | 4 KB    | 0.1 ms | 0.1 ms  | 0.0 ms  | 0.4 ms |
| `i64 (d=64 pool, full writer chunk)`        | 192 KB  | 192 KB  | 407 KB  | 195 KB  | 2.4 ms | 2.4 ms  | 4.2 ms  | 3.7 ms |

### Booleans

| Type                               | pco   | PcoPack | msgpack | Vortex | pco    | PcoPack | msgpack | Vortex |
|------------------------------------|-------|---------|---------|--------|--------|---------|---------|--------|
| `bool (i.i.d. 50% true)`           | 12 KB | 18 KB   | 18 KB   | 14 KB  | 0.1 ms | 0.3 ms  | 0.3 ms  | 1.2 ms |
| `bool (i.i.d. 90% true)`           | 5 KB  | 12 KB   | 12 KB   | 14 KB  | 0.3 ms | 0.2 ms  | 0.2 ms  | 0.9 ms |
| `bool (clustered runs)`            | 1 KB  | 1 KB    | 1 KB    | 14 KB  | 0.4 ms | 0.1 ms  | 0.1 ms  | 0.9 ms |
| `bool (alternating, period 2)`     | 12 KB | 24 B    | 24 B    | 14 KB  | 0.1 ms | 0.1 ms  | 0.1 ms  | 0.9 ms |
| `bool (i.i.d. 5% true)`            | 3 KB  | 7 KB    | 7 KB    | 14 KB  | 0.3 ms | 0.2 ms  | 0.2 ms  | 0.9 ms |
| `bool (clustered, ~20% true runs)` | 3 KB  | 6 KB    | 6 KB    | 14 KB  | 0.4 ms | 0.2 ms  | 0.2 ms  | 0.9 ms |

### Option<i32> vs sentinel

| Type                                 | PcoPack | msgpack | Vortex | PcoPack | msgpack | Vortex |
|--------------------------------------|---------|---------|--------|---------|---------|--------|
| `Option<i32> (80% null, clustered)`  | 10 KB   | 12 KB   | 26 KB  | 0.3 ms  | 0.2 ms  | 1.4 ms |
| `i32 (80% zero sentinel, clustered)` | 20 KB   | 12 KB   | 14 KB  | 0.6 ms  | 0.3 ms  | 1.8 ms |
| `Option<i32> (80% null, scattered)`  | 24 KB   | 33 KB   | 29 KB  | 0.5 ms  | 0.3 ms  | 2.0 ms |
| `i32 (80% zero sentinel, scattered)` | 24 KB   | 33 KB   | 26 KB  | 0.6 ms  | 0.4 ms  | 1.8 ms |
| `Option<i32> (90% null, clustered)`  | 5 KB    | 6 KB    | 20 KB  | 0.2 ms  | 0.2 ms  | 1.2 ms |
| `i32 (90% zero sentinel, clustered)` | 11 KB   | 6 KB    | 8 KB   | 0.6 ms  | 0.3 ms  | 1.6 ms |
| `Option<i32> (90% null, scattered)`  | 13 KB   | 20 KB   | 22 KB  | 0.3 ms  | 0.2 ms  | 1.6 ms |
| `i32 (90% zero sentinel, scattered)` | 13 KB   | 20 KB   | 16 KB  | 0.5 ms  | 0.4 ms  | 2.1 ms |

### Enums vs enum-as-integer

| Type                                                 | pco   | PcoPack | msgpack | Vortex | pco    | PcoPack | msgpack | Vortex |
|------------------------------------------------------|-------|---------|---------|--------|--------|---------|---------|--------|
| `Enum (simple, skewed 68/27/5)`                      | -     | 13 KB   | 55 KB   | 16 KB  | -      | 0.6 ms  | 0.8 ms  | 4.6 ms |
| `u8 (skewed 68/27/5, plain integer)`                 | 13 KB | 13 KB   | 21 KB   | 26 KB  | 0.3 ms | 0.3 ms  | 0.3 ms  | 1.2 ms |
| `Enum (complex, repeating pattern + payloads)`       | -     | 96 KB   | 180 KB  | -      | -      | 2.4 ms  | 1.9 ms  | -      |
| `u8 (repeating-pattern discriminant, plain integer)` | 24 KB | 24 KB   | 17 KB   | 26 KB  | 0.2 ms | 0.2 ms  | 0.3 ms  | 1.2 ms |

Complex enums stay `-`: serde_arrow traces them to arrow dense unions, and
the Vortex arrow import does not implement unions (`vortex.union` joins no
core edition writable set).

### Strings & bytes

| Type                                          | PcoPack | msgpack | Vortex  | PcoPack | msgpack | Vortex  |
|-----------------------------------------------|---------|---------|---------|---------|---------|---------|
| `String`                                      | 291 KB  | 288 KB  | 328 KB  | 5.2 ms  | 2.7 ms  | 10.5 ms |
| `SmolStr`                                     | 291 KB  | 288 KB  | 328 KB  | 3.2 ms  | 2.8 ms  | 10.3 ms |
| `Uuid (v4 random)`                            | 1692 KB | 1692 KB | 2001 KB | 1.6 ms  | 1.4 ms  | 23.0 ms |
| `ByteBuf (avg 128B)`                          | 12.4 MB | 12.4 MB | 12.6 MB | 13.2 ms | 10.4 ms | 18.8 ms |
| `Vec<String> (avg 4)`                         | 139 KB  | 138 KB  | 183 KB  | 9.9 ms  | 4.1 ms  | 14.3 ms |
| `serde_json::Value`                           | 635 KB  | 632 KB  | -       | 13.6 ms | 4.3 ms  | -       |
| `serde_json::Value (5 stable keys, 50k rows)` | 301 KB  | 299 KB  | 124 KB  | 14.7 ms | 4.4 ms  | 10.1 ms |

Stable-shape JSON objects ride the struct-promoted layout (key names enter
the schema once, not per object). Heterogeneous-shape `serde_json::Value`
stays `-`: serde_arrow cannot merge mismatched samples into one schema.

### Maps

| Type                                                             | PcoPack | msgpack | Vortex | PcoPack  | msgpack | Vortex  |
|------------------------------------------------------------------|---------|---------|--------|----------|---------|---------|
| `BTreeMap<String, i32>`                                          | 125 KB  | 123 KB  | 138 KB | 2.2 ms   | 1.2 ms  | 5.9 ms  |
| `HashMap<String, i32>`                                           | 125 KB  | 130 KB  | 142 KB | 2.8 ms   | 1.3 ms  | 5.8 ms  |
| `BTreeMap<SmolStr, i32>`                                         | 125 KB  | 123 KB  | 138 KB | 1.7 ms   | 3.9 ms  | 5.8 ms  |
| `HashMap<SmolStr, i32>`                                          | 125 KB  | 130 KB  | 142 KB | 2.5 ms   | 4.2 ms  | 5.8 ms  |
| `HashMap<SmolStr, i32> (8 stable keys)`                          | 43 KB   | 148 KB  | 51 KB  | 3.5 ms   | 1.9 ms  | 4.7 ms  |
| `HashMap<SmolStr, Option<i32>> (8 stable keys, ~60% null)`       | 26 KB   | 127 KB  | 36 KB  | 3.5 ms   | 1.9 ms  | 4.5 ms  |
| `HashMap<SmolStr, i32> (8 stable keys, walk)`                    | 115 KB  | 3.3 MB  | 114 KB | 117.0 ms | 51.6 ms | 61.6 ms |
| `HashMap<SmolStr, Option<i32>> (8 stable keys, ~60% null, walk)` | 852 KB  | 3.4 MB  | 352 KB | 100.9 ms | 52.0 ms | 77.0 ms |

Each row stores maps in whichever Vortex layout wins on its distribution:
the first four rows use native `vortex.map`, the stable-key rows use
struct-promoted columns (one column per key, key names in the schema).

- The first four rows stress a ~5000-key random keyspace with tuple-composed
  keys, and use native `vortex.map`: struct promotion cannot express them.
  Tuple keys fail serde_arrow tracing ("cannot interpret key as string"),
  and with string keys the first-100-row schema silently drops every entry
  outside the sample (measured: 39872 entries in, 13963 out). Native costs
  ~10% over PcoPack here: keys stay in the entries column (varbin->zstd)
  instead of the schema.
- The stable-key rows declare struct promotion and trace into one Vortex
  column per key. At 10k rows PcoPack's flattened counts+keys+values streams
  still edge out the struct, because zstd absorbs the repeated keys. The
  native layout measures ~1.5-1.7x above the promoted cells on this data.
- Dense walk (chunk-sized, autocorrelated values along row order, a
  time-sorted capture shape): struct vs PcoPack is a dead heat (114 KB vs
  115 KB). PcoPack's zstd'd key column already neutralizes constant keys, and
  one interleaved value stream loses little when every key moves. On this row
  native would pay ~18x (2033 KB): one interleaved stream cannot delta-code
  eight key-specific walks.
- Sparse walk: per-key separation wins big. Vortex lands ~59% under PcoPack
  (352 KB vs 852 KB): a hot-key column keeps its run/delta structure intact
  across absent keys, while the flat layout pays a presence index per entry
  and a gap-jumpier value stream. Native on the same distribution (1866 KB)
  loses that separation but still beats msgpack+zstd (3.4 MB).

### Floats (float_round)

| Type                          | pco    | PcoPack | msgpack | Vortex | pco    | PcoPack | msgpack | Vortex |
|-------------------------------|--------|---------|---------|--------|--------|---------|---------|--------|
| `f64 (normal, no round)`      | 650 KB | 650 KB  | 792 KB  | 653 KB | 1.7 ms | 1.7 ms  | 0.8 ms  | 5.0 ms |
| `f64 (normal, float_round=1)` | 650 KB | 65 KB   | 792 KB  | 653 KB | 1.7 ms | 0.7 ms  | 0.8 ms  | 5.0 ms |
| `f64 (normal, float_round=2)` | 650 KB | 106 KB  | 792 KB  | 653 KB | 1.7 ms | 1.0 ms  | 0.8 ms  | 5.0 ms |
| `f32 (normal, no round)`      | 296 KB | 296 KB  | 391 KB  | 299 KB | 1.6 ms | 1.5 ms  | 0.6 ms  | 4.7 ms |
| `f32 (normal, float_round=1)` | 296 KB | 65 KB   | 391 KB  | 299 KB | 1.6 ms | 0.7 ms  | 0.6 ms  | 4.7 ms |
| `f32 (normal, float_round=2)` | 296 KB | 106 KB  | 391 KB  | 299 KB | 1.6 ms | 1.0 ms  | 0.6 ms  | 4.7 ms |

`float_round = N` is a lossy knob (error <= 0.5 x 10^-N). Rounding is where the float win lives, not the codec: at N=1 the column is ~90% smaller than unrounded. Competitors have no quantization knob; their columns are constant across the table.

### Timestamps (time_round)

| Type                                  | pco    | PcoPack | msgpack | Vortex | pco    | PcoPack | msgpack | Vortex  |
|---------------------------------------|--------|---------|---------|--------|--------|---------|---------|---------|
| `DateTime (60s step, no round)`       | 313 KB | 313 KB  | 825 KB  | 836 KB | 1.2 ms | 1.3 ms  | 15.4 ms | 26.7 ms |
| `DateTime (60s step, time_round=1s)`  | 313 KB | 69 KB   | 825 KB  | 836 KB | 1.2 ms | 1.3 ms  | 15.4 ms | 26.7 ms |
| `DateTime (60s step, time_round=60s)` | 313 KB | 14 KB   | 825 KB  | 836 KB | 1.2 ms | 1.0 ms  | 15.4 ms | 26.7 ms |

The pco column is i64 microseconds, a 1:1 representation, so Pcodec is comparable. Vortex has no time type: `DateTime` is an RFC3339 string (~2.7x the exact-instant column). `time_round` is lossy: 1s rounding cuts PcoPack's column ~78%, 60s rounding ~95%.

## Structs

500k rows of a realistic metric struct: a low-cardinality `account_id` index key, random names, log-normal scores, and a clustered boolean flag. Vortex is one compact btrblocks file read back through Arrow. Standalone Pcodec is not compared: the struct holds fields Pcodec can't express (strings, `Vec`, `Option`, JSON); those rows live in [Types](#types). PcoPack stores `account_id` as per-chunk index metadata, not as a column.

- Size: PcoPack and Vortex are within 2% of each other; both 13-37% under columnar, serde_columnar, and msgpack
- Serialize: PcoPack ~33% faster than Vortex; deserialize: columnar is fastest, PcoPack ~22% faster than Vortex
- Filters: PcoPack's `account_id` index filter is ~2.5x faster than Vortex's; Vortex's 1-row scan predicate is ~1.9x faster than PcoPack's
- Filter methods differ: PcoPack materializes matching rows; columnar/serde_columnar/msgpack decompress everything and filter in Rust

| Metric        | PcoPack | Vortex   | columnar | serde_columnar | msgpack |
|---------------|---------|----------|----------|----------------|---------|
| Size          | 6.2 MB  | 6.3 MB   | 8.1 MB   | 7.1 MB         | 9.9 MB  |
| Serialize     | 83.5 ms | 125.1 ms | 61.0 ms  | 66.3 ms        | 88.8 ms |
| Deserialize   | 43.6 ms | 56.0 ms  | 21.3 ms  | 40.0 ms        | 80.1 ms |
| Filter 20%    | 8.7 ms  | 22.4 ms  | 22.7 ms  | 40.9 ms        | 79.1 ms |
| Filter <0.01% | 3.2 ms  | 1.7 ms   | 23.6 ms  | 41.1 ms        | 80.8 ms |

## Timeline

Scenario: 100 sensors report every second for 1,000 seconds. All rows in a given second share one timestamp with sub-second jitter. Uniqueness is the probability that a sensor's readings differ from its previous second.

Findings:
- `Timeline` merges duplicate observations into time ranges, achieving smaller sizes depending on uniqueness. The benefit grows as readings stay stable longer (more duplicates to merge). However, it requires changing your struct field type and adds serialization overhead at high uniqueness, where the extra cost may not be worth it
- `time_round = Duration::seconds(1)` consistently saves ~28 KB on the timestamp column across all scenarios. At low uniqueness this is a large fraction of total size (1.29x), but at high uniqueness the non-timestamp fields dominate and dilute the ratio to 1.05x

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
| 10%        | 2.0 ms   | 7.4 ms   | 7.1 ms        | 3.7x               | 1.04x              |
| 20%        | 2.3 ms   | 7.6 ms   | 7.4 ms        | 3.3x               | 1.03x              |
| 50%        | 5.2 ms   | 6.6 ms   | 6.3 ms        | 1.3x               | 1.05x              |
| 80%        | 8.2 ms   | 4.6 ms   | 4.3 ms        | 0.6x               | 1.07x              |
| 90%        | 9.1 ms   | 4.6 ms   | 4.3 ms        | 0.5x               | 1.07x              |

## chunk_size

`chunk_size` helps avoid out of memory crashes when converting between a columnar and row-based layout. Larger chunks naturally provide better compression and faster serialization, though require more memory. The crate's default 2^18 chunk size provides optimal performance while using a reasonable amount of memory. For very large structs or when running on memory-constrainted systems, you may want to use a smaller chunk size.

`Write peak` is the peak memory used while serializing, and `Read peak` is the peak while deserializing. Filtering to a small subset of the rows is significantly faster and uses less memory, as seen in the Filters benchmark.

### SmallStruct (32 bytes/row)

| Chunk size    | Size   | Serialize | Deserialize | Chunks | Write peak | Read peak |
|---------------|--------|-----------|-------------|--------|------------|-----------|
| 2^13 = 8192   | 157 KB | 25.7 ms   | 8.3 ms      | 64     | 6.0 MB     | 24.4 MB   |
| 2^14 = 16384  | 147 KB | 20.7 ms   | 8.1 ms      | 32     | 6.0 MB     | 24.6 MB   |
| 2^15 = 32768  | 142 KB | 18.2 ms   | 7.9 ms      | 16     | 6.5 MB     | 25.1 MB   |
| 2^16 = 65536  | 139 KB | 16.6 ms   | 7.7 ms      | 8      | 8.8 MB     | 26.1 MB   |
| 2^17 = 131072 | 138 KB | 15.7 ms   | 7.7 ms      | 4      | 13.4 MB    | 28.1 MB   |
| 2^18 = 262144 | 138 KB | 16.4 ms   | 7.7 ms      | 2      | 22.6 MB    | 32.1 MB   |
| 2^19 = 524288 | 137 KB | 16.4 ms   | 7.7 ms      | 1      | 27.1 MB    | 32.1 MB   |

### MediumStruct (80 bytes/row)

| Chunk size    | Size   | Serialize | Deserialize | Chunks | Write peak | Read peak |
|---------------|--------|-----------|-------------|--------|------------|-----------|
| 2^13 = 8192   | 683 KB | 84.3 ms   | 49.2 ms     | 64     | 6.0 MB     | 65.9 MB   |
| 2^14 = 16384  | 836 KB | 80.1 ms   | 50.3 ms     | 32     | 7.3 MB     | 66.8 MB   |
| 2^15 = 32768  | 773 KB | 80.1 ms   | 49.7 ms     | 16     | 9.5 MB     | 68.3 MB   |
| 2^16 = 65536  | 742 KB | 73.1 ms   | 48.7 ms     | 8      | 14.2 MB    | 71.3 MB   |
| 2^17 = 131072 | 726 KB | 73.5 ms   | 48.1 ms     | 4      | 23.6 MB    | 77.4 MB   |
| 2^18 = 262144 | 718 KB | 71.8 ms   | 47.0 ms     | 2      | 42.4 MB    | 89.6 MB   |
| 2^19 = 524288 | 714 KB | 71.9 ms   | 49.8 ms     | 1      | 79.9 MB    | 89.6 MB   |

### LargeStruct (176 bytes/row)

| Chunk size    | Size    | Serialize | Deserialize | Chunks | Write peak | Read peak |
|---------------|---------|-----------|-------------|--------|------------|-----------|
| 2^13 = 8192   | 1309 KB | 204.1 ms  | 102.7 ms    | 64     | 7.6 MB     | 146.7 MB  |
| 2^14 = 16384  | 1246 KB | 185.3 ms  | 101.7 ms    | 32     | 9.6 MB     | 148.4 MB  |
| 2^15 = 32768  | 1216 KB | 174.8 ms  | 100.6 ms    | 16     | 13.9 MB    | 151.8 MB  |
| 2^16 = 65536  | 1202 KB | 171.0 ms  | 101.1 ms    | 8      | 22.5 MB    | 158.7 MB  |
| 2^17 = 131072 | 1197 KB | 167.4 ms  | 98.8 ms     | 4      | 39.8 MB    | 172.6 MB  |
| 2^18 = 262144 | 1194 KB | 168.4 ms  | 98.3 ms     | 2      | 74.3 MB    | 200.5 MB  |
| 2^19 = 524288 | 1193 KB | 167.6 ms  | 99.8 ms     | 1      | 143.3 MB   | 200.5 MB  |

## Filters

PcoPack is designed to skip expensive deserialization work for rows that don't match the provided filter. For most data types, a filter matching 1% of rows pays just 15% of the runtime cost that otherwise would've been needed to deserialize all rows.

| Filter                                               | Time (ms) | Rows   |
|------------------------------------------------------|-----------|--------|
| no filter                                            | 38.7 ms   | 100000 |
| empty filter                                         | 38.5 ms   | 100000 |
| i64 exact (id == 50_000)                             | 6.4 ms    | 1000   |
| i64 range (50_000..=50_999)                          | 6.4 ms    | 1000   |
| i64 inclusion (100 values, 1 match)                  | 6.5 ms    | 1000   |
| i32 exact (int32_val == 50)                          | 6.3 ms    | 1000   |
| i32 range (50..=50.99)                               | 6.4 ms    | 1000   |
| i32 inclusion (100 values, 1 match)                  | 6.4 ms    | 1000   |
| i8 exact (int8_val == 50)                            | 6.3 ms    | 1000   |
| i8 range (50..=50.99)                                | 6.3 ms    | 1000   |
| i8 inclusion (100 values, 1 match)                   | 6.4 ms    | 1000   |
| u8 exact (u8_val == 50)                              | 6.3 ms    | 1000   |
| u8 range (50..=50.99)                                | 6.3 ms    | 1000   |
| u8 inclusion (100 values, 1 match)                   | 6.5 ms    | 1000   |
| f64 exact (float64_val == 50.0)                      | 6.4 ms    | 1000   |
| f64 range (50.0..=50.99)                             | 6.3 ms    | 1000   |
| f64 inclusion (100 values, 1 match)                  | 8.8 ms    | 1000   |
| f32 exact (float32_val == 50.0)                      | 6.3 ms    | 1000   |
| f32 range (50.0..=50.99)                             | 6.2 ms    | 1000   |
| f32 inclusion (100 values, 1 match)                  | 9.0 ms    | 1000   |
| f16 exact (f16_val == 50.0)                          | 6.4 ms    | 1000   |
| f16 range (50.0..=50.99)                             | 6.4 ms    | 1000   |
| f16 inclusion (100 values, 1 match)                  | 9.1 ms    | 1000   |
| string exact                                         | 8.0 ms    | 1000   |
| string inclusion (10 values)                         | 8.4 ms    | 10000  |
| bool exact (bool_val == true)                        | 19.4 ms   | 1000   |
| bool exact (bool_val == false)                       | 38.5 ms   | 99000  |
| enum exact (status == V50)                           | 6.3 ms    | 1000   |
| enum inclusion (status in 0..9)                      | 6.8 ms    | 10000  |
| option exact (option_val == 50)                      | 6.4 ms    | 1000   |
| option range (50..=50.99)                            | 6.3 ms    | 1000   |
| option inclusion (100 values, 1 match)               | 6.5 ms    | 1000   |
| vec contains (vec_val has 5000)                      | 6.4 ms    | 1000   |
| vec contains inclusion (any of 10 values)            | 6.8 ms    | 10000  |
| bytes exact (hex match)                              | 7.9 ms    | 1000   |
| map exact (has key 'key_50')                         | 14.4 ms   | 1000   |
| map inclusion (any of key_0..key_9)                  | 15.0 ms   | 10000  |
| json exact (tag_50 string)                           | 8.5 ms    | 1000   |
| uuid exact (bucket 50)                               | 6.4 ms    | 1000   |
| uuid inclusion (100 values, 1 match)                 | 6.5 ms    | 1000   |
| nested (nested.inner_id == 50_000)                   | 6.4 ms    | 1000   |
| nested (nested.inner_id range 50_000..=50_000.99)    | 6.4 ms    | 1000   |
| nested (nested.inner_name exact)                     | 8.1 ms    | 1000   |
| nested (nested.inner_name inclusion, 10 values)      | 8.5 ms    | 10000  |
| partial fields (id + string_val)                     | 0.8 ms    | 1000   |
| multi-field (id + int32_val)                         | 6.3 ms    | 1000   |
| multi-field (bytes_val + u8_val)                     | 7.1 ms    | 1000   |
| multi-field (string_val + u8_val)                    | 7.3 ms    | 1000   |
| multi-field (json_val + u8_val)                      | 7.2 ms    | 1000   |
| multi-field zero-match (id + int32_val + string_val) | 0.4 ms    | 0      |

## Datasets

Four public datasets run end to end at full scale: serialize, deserialize, size, and filtered
reads. Each operation runs one pass, so sizes are exact and timings carry run-to-run variance.
Run `cargo bench --bench datasets`; the pinned payloads are cached in gitignored
`tmp/` on first run, offline afterwards; override the dir with `$PCO_PACK_BENCH_DATA`:

| File                                            | Source                                                                                        | Size          | Pin                                    |
|-------------------------------------------------|-----------------------------------------------------------------------------------------------|---------------|----------------------------------------|
| `devinrsmith-air-quality.20220714.zstd.parquet` | https://pcodec-public.s3.amazonaws.com/devinrsmith-air-quality.20220714.zstd.parquet          | 15,661,377 B  | size (multipart ETag)                  |
| `fhvhv_tripdata_2023-04.parquet`                | https://d37ci6vzurychx.cloudfront.net/trip-data/fhvhv_tripdata_2023-04.parquet                | 491,826,475 B | md5 `7233505d557e0c4f7a2e64ba4bfe5c2d` |
| `austria-260923.osm.pbf`                        | https://download.geofabrik.de/europe/austria-260923.osm.pbf (ODbL)                            | 810,746,083 B | md5 `481bb65e61d5d5d22bb8a065b22506e2` |
| `openalex_part_0000.gz`                         | https://openalex.s3.amazonaws.com/data/jsonl/works/updated_date=2026-09-23/part_0000.gz (CC0) | 772,597,338 B | size (multipart ETag)                  |
| `openalex_part_0001.gz`                         | https://openalex.s3.amazonaws.com/data/jsonl/works/updated_date=2026-09-23/part_0001.gz (CC0) | 771,641,892 B | size (multipart ETag)                  |

Each schema maps to its most expressive Rust representation (closed vocabularies -> enums,
sparse flags -> `Option<bool>`, small strings -> `SmolStr`), and every codec round-trips the
same values through mirrors where needed (columnar codecs: enum as u8, `DateTime` as i64,
maps as `Vec<(K, V)>`; Vortex: enum utf8 mirrors, maps as native `vortex.map`). Filter results are asserted against the
counts computed from each codec's own stored precision.

### air_quality (823743 rows)

NYC AirData single-sensor stream (2022-06-22 to 07-14): wide i16 measurements, a monotonic
i64 counter, ns-precision `DateTime` (stored as us, real truncation), and the only genuine
default-vs-lossy pair (`time_round = 1 s`). Runs at the default chunk size (4 chunks):
`chunk_size = 2048` measured ~2x faster window filters but ~10% worse size and ~4x slower
serialize.

| Metric       | PcoPack  | PcoPack (lossy) | Vortex   | columnar | serde_columnar | msgpack  |
|--------------|----------|-----------------|----------|----------|----------------|----------|
| Size         | 4.1 MB   | 2.5 MB          | 4.1 MB   | 14.0 MB  | 5.4 MB         | 15.9 MB  |
| Serialize    | 118.0 ms | 111.4 ms        | 221.4 ms | 106.5 ms | 133.2 ms       | 300.5 ms |
| Deserialize  | 52.7 ms  | 52.0 ms         | 67.5 ms  | 34.7 ms  | 57.8 ms        | 210.0 ms |
| Filter 0.18% | 6.6 ms   | 6.5 ms          | 2.2 ms   | 34.7 ms  | 58.4 ms        | 208.2 ms |
| Filter 4.49% | 8.1 ms   | 7.6 ms          | 4.1 ms   | 32.9 ms  | 57.7 ms        | 209.9 ms |

- Size ties Vortex (both full-fidelity), ~71% under columnar and ~74% under msgpack;
  `time_round = 1 s` cuts another 40%
- Deserialize ~4x faster than msgpack; window filters cost ~12-15% of the full deserialize:
  both windows sit inside chunk 0, so the rest of the file is never touched
- Vortex mirrors the timestamp as i64 ns (its RFC3339 string column would make the range
  filter unsound) and is the fastest filterer here (2.2 / 4.1 ms)

### taxi (19144903 rows)

NYC TLC FHV trips, April 2023, the file pcodec benchmarks as "taxi". All four timestamps
are integer seconds (asserted); `float_round = 3` is exactly lossless (asserted): miles
carry a third decimal on 24% of rows, amounts <= 2.

| Metric       | PcoPack  | PcoPack (lossy) | Vortex   | columnar | serde_columnar | msgpack  |
|--------------|----------|-----------------|----------|----------|----------------|----------|
| Size         | 285.5 MB | 275.7 MB        | 413.8 MB | 572.5 MB | 523.4 MB       | 661.5 MB |
| Serialize    | 9.0 s    | 8.7 s           | 20.2 s   | 7.8 s    | 10.0 s         | 12.3 s   |
| Deserialize  | 3.6 s    | 3.5 s           | 9.4 s    | 2.6 s    | 5.2 s          | 9.8 s    |
| Filter 0.2%  | 93.7 ms  | 94.4 ms         | 30.7 ms  | 2.5 s    | 5.5 s          | 9.8 s    |
| Filter 4.05% | 214.4 ms | 209.5 ms        | 443.1 ms | 2.5 s    | 6.1 s          | 9.9 s    |

- Smallest codec on the table by a wide margin: ~31% under Vortex, ~57% under msgpack
- Hour window favors Vortex (finer row-group pruning); day window favors PcoPack (~52%
  faster): chunk bounds vs row groups
- pcodec's published numbers for reference (M3 Max, numeric columns only): pco L8 333.6 MB,
  parquet+zstd1 456.5 MB

### osm (87800672 nodes / 9378713 ways / 180809 relations)

Geofabrik Austria PBF (ODbL). Node coords are nanodegree decimals (9 fractional digits);
`float_round = 7` drops only sub-centimeter noise. Filters use padded boxes centered on the
data centroid: cross-border geometry puts 0.013% of nodes outside the nominal country
bbox. Ways/relations have no natural scalar filter.

#### Node (87800672 rows)

| Metric       | PcoPack  | PcoPack (lossy) | Vortex   | columnar | serde_columnar | msgpack |
|--------------|----------|-----------------|----------|----------|----------------|---------|
| Size         | 867.1 MB | 359.7 MB        | 415.6 MB | 1.2 GB   | 1.1 GB         | 1.4 GB  |
| Serialize    | 10.9 s   | 10.2 s          | 43.1 s   | 8.6 s    | 9.2 s          | 8.6 s   |
| Deserialize  | 3.2 s    | 3.2 s           | 5.1 s    | 2.3 s    | 5.0 s          | 6.2 s   |
| Filter 0.17% | 740.7 ms | 801.6 ms        | 550.0 ms | 2.2 s    | 10.0 s         | 6.9 s   |
| Filter 4.65% | 1.2 s    | 1.3 s           | 2.2 s    | 2.2 s    | 4.1 s          | 6.0 s   |

- Vortex is ~52% smaller full-fidelity; the gap is entirely the raw f64 coordinate
columns, where PcoPack pays ~4.9 B/coord; tags favor PcoPack
- Tags are native `vortex.map` columns. The entries column keeps every key
  materialized (varbin->zstd), so the map type buys schema honesty, not size:
  a flattened `Vec<(String, String)>` layout of the same tags measures ~1%
  smaller and writes ~40% faster, with unchanged filtering. Promoting the open
  vocabulary (serde_arrow `map_as_struct=true` plus a sampled schema) silently
  drops ~65% of tag entries with no error, so native maps are the honest
  representation here
- `float_round = 7` cuts node size ~59% and lands ~13% below every full-fidelity codec
- The wide box runs ~37% faster than Vortex (chunk bounds skip); the narrow box trails it
  (~40% slower): Vortex's row groups prune spatial data better at fine granularity

#### Way (9378713 rows)

| Metric      | PcoPack  | Vortex   | columnar | serde_columnar | msgpack  |
|-------------|----------|----------|----------|----------------|----------|
| Size        | 199.7 MB | 212.1 MB | 395.1 MB | 459.9 MB       | 490.9 MB |
| Serialize   | 10.7 s   | 13.8 s   | 6.5 s    | 5.7 s          | 4.5 s    |
| Deserialize | 4.8 s    | 4.7 s    | 1.8 s    | 3.7 s          | 4.0 s    |

#### Relation (180809 rows)

| Metric      | PcoPack  | Vortex   | columnar | serde_columnar | msgpack  |
|-------------|----------|----------|----------|----------------|----------|
| Size        | 18.4 MB  | 19.3 MB  | 20.2 MB  | 17.3 MB        | 19.8 MB  |
| Serialize   | 459.6 ms | 687.0 ms | 292.6 ms | 362.6 ms       | 201.4 ms |
| Deserialize | 234.4 ms | 244.8 ms | 64.6 ms  | 197.2 ms       | 253.0 ms |

Ways: PcoPack ~6% under Vortex, ~49% under columnar. Vortex stores `Member.rtype` as a
utf8 string. `relation` members are too rare in its 100-row sampling to observe every
enum variant, and an unobserved variant leaves an unencodable Union.

### openalex (558403 works)

OpenAlex `works` snapshot (CC0): deeply nested JSON flattened to typed structs: enums,
`Option<bool>`, maps as `Vec<(K, V)>`, nested authorship/topic collections. `float_round = 4`
is a deliberate fidelity cut (not exact): `fwci` and scores carry ~15-digit model noise.

| Metric       | PcoPack  | PcoPack (lossy) | Vortex   | columnar | serde_columnar | msgpack  |
|--------------|----------|-----------------|----------|----------|----------------|----------|
| Size         | 411.5 MB | 404.2 MB        | 422.1 MB | 556.3 MB | 513.4 MB       | 566.7 MB |
| Serialize    | 12.5 s   | 12.1 s          | 11.8 s   | 8.3 s    | 9.2 s          | 5.1 s    |
| Deserialize  | 6.4 s    | 6.3 s           | 5.0 s    | 1.7 s    | 5.8 s          | 4.5 s    |
| Filter 0.02% | 2.1 s    | 2.1 s           | 471.5 ms | 1.7 s    | 5.6 s          | 4.8 s    |
| Filter 2.94% | 2.6 s    | 2.5 s           | 1.1 s    | 1.9 s    | 5.3 s          | 4.3 s    |

- ~2.5% under Vortex full-fidelity; ~26% under columnar
- Filter cost barely tracks selectivity here (~33-40% of the full deserialize either way):
  the bench asks for whole rows (`fields = &[]`), so every surviving chunk decompresses
  everything before matching. A narrow `fields` list would read only the filtered column
- Vortex is the fastest filterer (row-group pruning + predicate pushdown); columnar's full
  decode+scan costs ~1.7-1.9 s

Filter selectivity: every dataset carries one highly selective (<= 1% of rows) and one
looser (<= 30%) filter, labeled in the tables. PcoPack's advantage is largest on the flat,
time-sorted data (air_quality, taxi) and shrinks on the deeply nested openalex schema.

## Postgres

Real pganalyze monitoring payloads captured from a live pgaweb deployment, the only
non-public datasets here. The captures commit as neutral msgpack+zstd input blobs on
Git LFS (content-addressed, so no extra checksum pins), so the benchmark reproduces
without re-capturing: `cargo bench --bench postgres` decodes them (override the dir with
`$PCO_PACK_BENCH_DATA`) and re-encodes the same loaded rows through every codec, one
table per data source (`backend_snapshots`, its Timeline-dedup variant, and
`query_stats` per granularity), in the same format as [Datasets](#datasets).
`query_stats` is trimmed to the capture's last 14 days:
the full capture spans ~100 days for some database IDs (114.3M rows, 748 MB); keeping
the tail (31.0M rows, 165 MB, 2026-09-09 to 2026-09-23) keeps LFS hosting cheap and
full runs tractable. Regenerate with the capture crate in `tmp/pg_capture` against a
live deployment, then trim to the last 14 days before committing:

| File                            | Content                                                                                                                                                                                                           | Size          |
|---------------------------------|-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|---------------|
| `backend_snapshots.msgpack.zst` | every `backend_snapshots_7d` tick reconstructed with the pco backfill's exact keep rule (`application_name`/`client_addr` are empty because the raw table doesn't persist them; the enum stacks carry the signal) | 58,898,437 B  |
| `query_stats.msgpack.zst`       | last 14 days of the pco_store `query_stats` partition parent (1min + 1hour children; 31.0M rows)                                                                                                                  | 172,956,811 B |

Schemas mirror production verbatim (index + timestamp). PcoPack reports full-fidelity,
plus the `TimeRound` storage model (production's rounded writes: `time_round = 1 s`
and `float_round = 2` on the query_stats f64 aggregates) as the `PcoPack time_round`
column. backend_snapshots adds `PcoPack Timeline`: the same rows deduped so unchanged
state between ticks collapses into one row carrying a `Timeline<0>` of the
ticks it was observed at (see [Timeline](#timeline)). Indexed writes return rows
index-grouped, not in stored order, so PcoPack roundtrips verify row counts (the
convention for indexed structs, see [Structs](#structs)); filter results are asserted
against counts computed from each codec's own stored precision.

### backend_snapshots (13328012 rows)

| Metric        | PcoPack  | PcoPack time_round | PcoPack Timeline | Vortex   | columnar | serde_columnar | msgpack |
|---------------|----------|--------------------|------------------|----------|----------|----------------|---------|
| Size          | 47.1 MB  | 39.2 MB            | 18.0 MB          | 83.8 MB  | 68.8 MB  | 69.3 MB        | 80.2 MB |
| Serialize     | 3.3 s    | 3.3 s              | 1.8 s            | 3.7 s    | 1.1 s    | 2.7 s          | 4.4 s   |
| Deserialize   | 1.3 s    | 1.3 s              | 474.3 ms         | 1.7 s    | 552.0 ms | 1.9 s          | 4.8 s   |
| Filter 0.49%  | 66.0 ms  | 65.9 ms            | 166.5 ms         | 13.2 ms  | 529.5 ms | 2.1 s          | 4.8 s   |
| Filter 29.25% | 763.0 ms | 768.5 ms           | 442.7 ms         | 794.1 ms | 526.5 ms | 1.7 s          | 4.8 s   |
| Filter 77.25% | 1.1 s    | 1.1 s              | 365.1 ms         | 1.4 s    | 525.4 ms | 1.7 s          | 4.8 s   |

(3794455 of 13328012 ticks survive the Timeline fold)

- Full-fidelity ~44% under Vortex, ~31% under columnar, ~41% under msgpack;
  `time_round = 1 s` cuts another ~17% (`backend_start`/`query_start` carry the us)
- Vortex is the fastest selective filterer (13.2 ms); columnar's full decode+scan
  amortizes once windows widen (sub-second across every window), where PcoPack pays
  whole-row chunk reads
- The last-1h window is the production dashboard query: 65,194 of 13.3M rows in ~66 ms
- `PcoPack Timeline` re-encodes the same ticks deduped: 3,794,455 fold rows survive
  out of 13.3M (idle backends dominate), and PcoPack's range columns make the fold
  pay: 47.1 MB to 18.0 MB (-62%), ~1.8x faster serialize, ~2.7x faster deserialize.
  Active ticks barely fold (their payload changes every tick), so they dominate the
  deduped set and the window filter pays range-overlap checks across chunk boundaries
  (~167 ms vs ~66 ms), while the wide scans still speed up (~1.7x/~3x). Filter
  labels are per-tick fractions; the Timeline column answers the same queries over
  the fold rows (0.44%/83.34%/52.12%), asserted internally

### query_stats_1min (29970299 rows)

| Metric        | PcoPack  | PcoPack time_round | Vortex   | columnar | serde_columnar | msgpack  |
|---------------|----------|--------------------|----------|----------|----------------|----------|
| Size          | 116.5 MB | 116.4 MB           | 130.0 MB | 191.7 MB | 179.4 MB       | 236.2 MB |
| Serialize     | 4.5 s    | 4.4 s              | 6.6 s    | 4.3 s    | 4.5 s          | 6.2 s    |
| Deserialize   | 1.8 s    | 1.8 s              | 2.7 s    | 1.6 s    | 3.4 s          | 4.8 s    |
| Filter 0.31%  | 142.7 ms | 139.7 ms           | 87.9 ms  | 1.4 s    | 3.2 s          | 4.7 s    |
| Filter 18.19% | 792.8 ms | 772.6 ms           | 1.1 s    | 1.4 s    | 3.3 s          | 4.8 s    |
| Filter 0.05%  | 32.6 ms  | 32.7 ms            | 19.3 ms  | 1.5 s    | 3.8 s          | 4.7 s    |

### query_stats_1hour (1041799 rows)

| Metric        | PcoPack  | PcoPack time_round | Vortex   | columnar | serde_columnar | msgpack  |
|---------------|----------|--------------------|----------|----------|----------------|----------|
| Size          | 5.9 MB   | 5.9 MB             | 7.1 MB   | 10.0 MB  | 9.1 MB         | 11.6 MB  |
| Serialize     | 189.0 ms | 175.8 ms           | 272.0 ms | 168.6 ms | 137.4 ms       | 231.3 ms |
| Deserialize   | 74.3 ms  | 68.3 ms            | 96.4 ms  | 55.4 ms  | 178.1 ms       | 184.9 ms |
| Filter 0.63%  | 26.0 ms  | 24.4 ms            | 2.7 ms   | 55.9 ms  | 179.1 ms       | 178.6 ms |
| Filter 58.45% | 51.3 ms  | 49.7 ms            | 74.5 ms  | 59.4 ms  | 111.4 ms       | 179.3 ms |
| Filter 0.03%  | 1.5 ms   | 1.6 ms             | 15.4 ms  | 59.8 ms  | 109.2 ms       | 178.9 ms |

- PcoPack full-fidelity is ~10% under Vortex, ~39% under columnar, ~51% under
  msgpack at both granularities: fingerprints and the calls/rows counters keep
  every re-encode honest and cheap to delta-encode
- The `PcoPack time_round` column barely moves here (116.5 MB to 116.4 MB): ticks are
  already second-aligned and pco's float stage already squeezes the f64
  aggregates, so the rounding knobs earn their keep on backend_snapshots, not stats
- Vortex is again the sharpest selective filterer (87.9/19.3 ms at 1min, 2.7/15.4 ms
  at 1hour); PcoPack's index+timestamp layout tracks it within ~70%
  while every decode-and-scan codec pays full store decode on every window
