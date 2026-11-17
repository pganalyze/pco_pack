# Datasets


Real datasets from public sources.

### air_quality (823743 rows)

NYC AirData single-sensor stream: wide i16 measurements, a monotonic i64 counter, ns `DateTime` stored as us.

| Metric       | PcoPack  | PcoPack (lossy) | Vortex   | columnar | serde_columnar | msgpack  |
|--------------|----------|-----------------|----------|----------|----------------|----------|
| Size         | 4.1 MB   | 2.5 MB          | 4.1 MB   | 14.0 MB  | 5.4 MB         | 15.9 MB  |
| Serialize    | 107.7 ms | 104.4 ms        | 208.6 ms | 98.4 ms  | 120.2 ms       | 281.7 ms |
| Deserialize  | 51.3 ms  | 51.9 ms         | 68.7 ms  | 33.7 ms  | 55.9 ms        | 197.3 ms |
| Filter 0.18% | 5.8 ms   | 5.8 ms          | 0.5 ms   | 32.1 ms  | 54.9 ms        | 197.5 ms |
| Filter 4.49% | 7.2 ms   | 7.2 ms          | 3.5 ms   | 32.2 ms  | 55.0 ms        | 197.9 ms |
| Reduce 0.18% | 1.1 ms   | 1.1 ms          | 0.1 ms   | 33.8 ms  | 54.5 ms        | 197.2 ms |
| Reduce 4.49% | 1.4 ms   | 1.1 ms          | 0.6 ms   | 34.1 ms  | 54.7 ms        | 202.1 ms |

- Full-fidelity size is tied between PcoPack and Vortex
- Lossy time rounding reduces size by 40%
- Reduce 0.18% is flat under the bounds prefix: the window already prunes disjoint chunks through `start_at`/`end_at`, so the surviving fold work is column decompression no prefix can skip

### taxi (19144903 rows)

NYC taxi trips (also seen in the upstream pcodec benchmarks). All four timestamps are integer seconds and `float_round = 3` is lossless.

| Metric         | PcoPack  | PcoPack (lossy) | Vortex   | columnar | serde_columnar | msgpack  |
|----------------|----------|-----------------|----------|----------|----------------|----------|
| Size           | 285.1 MB | 275.7 MB        | 411.5 MB | 572.5 MB | 523.4 MB       | 661.5 MB |
| Serialize      | 8.5 s    | 7.8 s           | 20.4 s   | 7.5 s    | 8.5 s          | 10.9 s   |
| Deserialize    | 3.7 s    | 3.5 s           | 9.5 s    | 2.5 s    | 4.8 s          | 9.6 s    |
| Filter 0.2%    | 35.3 ms  | 32.8 ms         | 27.1 ms  | 2.5 s    | 4.9 s          | 10.0 s   |
| Filter 4.05%   | 155.9 ms | 149.0 ms        | 405.2 ms | 2.5 s    | 4.9 s          | 10.0 s   |
| Text eq 73.12% | 3.4 s    | 3.3 s           | 7.5 s    | 2.5 s    | 5.0 s          | 10.1 s   |
| Text eq 73.1%  | 3.4 s    | 3.3 s           | 7.5 s    | 2.5 s    | 5.0 s          | 10.1 s   |
| Reduce 4.05%   | 15.8 ms  | 10.9 ms         | 83.6 ms  | 2.5 s    | 4.9 s          | 10.0 s   |
| Reduce 73.12%  | 536.6 ms | 453.4 ms        | 254.5 ms | 2.6 s    | 5.0 s          | 9.7 s    |

- PcoPack is the smallest by a wide margin
- For filtering, hour window favors Vortex (finer row-group pruning) and day window favors PcoPack
- pcodec reference (numeric columns only): pco L8 333.6 MB, parquet+zstd1 456.5 MB

### OpenStreetMap

Geofabrik Austria PBF. Filters are padded boxes on the centroid: cross-border geometry puts a measurable fraction of nodes outside the nominal bbox. Ways and relations have no natural scalar filter.

#### Node (85929052 rows)

| Metric       | PcoPack  | PcoPack (lossy) | Vortex   | columnar | serde_columnar | msgpack |
|--------------|----------|-----------------|----------|----------|----------------|---------|
| Size         | 370.7 MB | 351.5 MB        | 386.8 MB | 1.2 GB   | 1.1 GB         | 1.3 GB  |
| Serialize    | 5.4 s    | 4.5 s           | 20.7 s   | 6.5 s    | 6.3 s          | 7.3 s   |
| Deserialize  | 3.5 s    | 3.3 s           | 4.6 s    | 2.2 s    | 3.0 s          | 6.0 s   |
| Filter 0.18% | 736.8 ms | 656.1 ms        | 525.2 ms | 2.1 s    | 3.1 s          | 6.4 s   |
| Filter 4.66% | 1.2 s    | 1.1 s           | 1.6 s    | 2.2 s    | 3.1 s          | 6.4 s   |
| Reduce 0.18% | 479.6 ms | 392.6 ms        | 417.0 ms | 2.2 s    | 3.1 s          | 6.4 s   |
| Reduce 4.66% | 528.3 ms | 425.4 ms        | 761.6 ms | 2.3 s    | 3.1 s          | 6.2 s   |

PcoPack (lossy) uses `float_round = 7`, cutting node size ~5% by dropping sub-centimeter coordinate precision.

#### Way (9172339 rows)

| Metric      | PcoPack  | Vortex   | columnar | serde_columnar | msgpack  |
|-------------|----------|----------|----------|----------------|----------|
| Size        | 194.2 MB | 206.3 MB | 384.8 MB | 448.2 MB       | 478.4 MB |
| Serialize   | 5.2 s    | 12.6 s   | 4.0 s    | 3.4 s          | 4.0 s    |
| Deserialize | 4.0 s    | 4.2 s    | 1.5 s    | 3.2 s          | 3.5 s    |
| Reduce 100% | 602.5 ms | 1.1 s    | 1.5 s    | 3.1 s          | 3.9 s    |

#### Relation (175564 rows)

| Metric      | PcoPack  | Vortex   | columnar | serde_columnar | msgpack  |
|-------------|----------|----------|----------|----------------|----------|
| Size        | 17.8 MB  | 18.0 MB  | 19.5 MB  | 16.6 MB        | 19.1 MB  |
| Serialize   | 244.0 ms | 676.3 ms | 179.8 ms | 203.5 ms       | 188.1 ms |
| Deserialize | 201.7 ms | 289.6 ms | 55.0 ms  | 175.6 ms       | 236.7 ms |
| Reduce 100% | 83.5 ms  | 197.9 ms | 55.2 ms  | 173.2 ms       | 253.7 ms |

### openalex (632276 works)

OpenAlex `works` snapshot: deeply nested JSON flattened to typed structs.

| Metric         | PcoPack  | PcoPack (lossy) | Vortex   | columnar | serde_columnar | msgpack  |
|----------------|----------|-----------------|----------|----------|----------------|----------|
| Size           | 322.1 MB | 316.0 MB        | 334.5 MB | 450.0 MB | 394.5 MB       | 434.6 MB |
| Serialize      | 7.9 s    | 8.1 s           | 12.2 s   | 4.7 s    | 5.7 s          | 4.1 s    |
| Deserialize    | 5.6 s    | 5.7 s           | 5.6 s    | 1.5 s    | 5.2 s          | 4.0 s    |
| Filter <0.01%  | 1.3 s    | 1.3 s           | 122.4 ms | 1.5 s    | 5.1 s          | 4.6 s    |
| Filter 2.96%   | 2.2 s    | 2.2 s           | 1.0 s    | 1.5 s    | 5.1 s          | 4.6 s    |
| Text eq 85.13% | 5.1 s    | 5.2 s           | 4.9 s    | 1.6 s    | 5.2 s          | 4.6 s    |
| Text eq <0.01% | 437.1 ms | 439.4 ms        | 69.3 ms  | 1.5 s    | 5.2 s          | 4.6 s    |
| Reduce 85.13%  | 26.3 ms  | 19.5 ms         | 11.0 ms  | 1.5 s    | 5.2 s          | 4.6 s    |
| Reduce 2.96%   | 8.6 ms   | 8.4 ms          | 4.1 ms   | 1.5 s    | 5.1 s          | 4.6 s    |

- PcoPack (lossy) `float_round = 4` saves 2% because `fwci` and scores carry ~15-digit model noise
- Vortex filters fastest (row-group pruning + predicate pushdown)
- At the production 262,144-row chunk, chunk min/max bounds rarely exclude a sparse-match chunk, so selective PcoPack filters decode nearly every blob (Filter <0.01% costs seconds, not tens of ms); sizing chunks to the capture is a possible future escape hatch
- Vortex compact output is not byte-deterministic run to run: its size varied 0.3% between the two passes
