# Datasets


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
