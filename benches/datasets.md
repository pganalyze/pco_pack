# Datasets


Real datasets from public sources.

### air_quality (823743 rows)

NYC AirData single-sensor stream: wide i16 measurements, a monotonic i64 counter, ns `DateTime` stored as us.

| Metric       | PcoPack  | PcoPack (lossy) | Vortex   | columnar | serde_columnar | msgpack  |
|--------------|----------|-----------------|----------|----------|----------------|----------|
| Size         | 4.1 MB   | 2.5 MB          | 4.1 MB   | 14.0 MB  | 5.4 MB         | 15.9 MB  |
| Serialize    | 108.1 ms | 105.4 ms        | 208.6 ms | 98.4 ms  | 120.2 ms       | 281.7 ms |
| Deserialize  | 50.8 ms  | 51.5 ms         | 68.7 ms  | 33.7 ms  | 55.9 ms        | 197.3 ms |
| Filter 0.18% | 6.7 ms   | 6.4 ms          | 0.5 ms   | 32.1 ms  | 54.9 ms        | 197.5 ms |
| Filter 4.49% | 8.0 ms   | 7.8 ms          | 3.5 ms   | 32.2 ms  | 55.0 ms        | 197.9 ms |
| Reduce 0.18% | 1.9 ms   | 1.8 ms          | 0.1 ms   | 33.8 ms  | 54.5 ms        | 197.2 ms |
| Reduce 4.49% | 2.1 ms   | 1.8 ms          | 0.6 ms   | 34.1 ms  | 54.7 ms        | 202.1 ms |

- Full-fidelity size is tied between PcoPack and Vortex
- Lossy time rounding reduces size by 40%

### taxi (19144903 rows)

NYC taxi trips (also seen in the upstream pcodec benchmarks). All four timestamps are integer seconds and `float_round = 3` is lossless.

| Metric         | PcoPack  | PcoPack (lossy) | Vortex   | columnar | serde_columnar | msgpack  |
|----------------|----------|-----------------|----------|----------|----------------|----------|
| Size           | 285.1 MB | 275.7 MB        | 411.5 MB | 572.5 MB | 523.4 MB       | 661.5 MB |
| Serialize      | 8.6 s    | 8.2 s           | 20.4 s   | 7.5 s    | 8.5 s          | 10.9 s   |
| Deserialize    | 3.6 s    | 3.5 s           | 9.5 s    | 2.5 s    | 4.8 s          | 9.6 s    |
| Filter 0.2%    | 95.3 ms  | 92.9 ms         | 27.1 ms  | 2.5 s    | 4.9 s          | 10.0 s   |
| Filter 4.05%   | 211.6 ms | 207.8 ms        | 405.2 ms | 2.5 s    | 4.9 s          | 10.0 s   |
| Text eq 73.12% | 3.4 s    | 3.3 s           | 7.5 s    | 2.5 s    | 5.0 s          | 10.1 s   |
| Text eq 73.1%  | 3.5 s    | 3.4 s           | 7.5 s    | 2.5 s    | 5.0 s          | 10.1 s   |
| Reduce 4.05%   | 62.8 ms  | 53.4 ms         | 83.6 ms  | 2.5 s    | 4.9 s          | 10.0 s   |
| Reduce 73.12%  | 574.7 ms | 481.0 ms        | 254.5 ms | 2.6 s    | 5.0 s          | 9.7 s    |

- PcoPack is the smallest by a wide margin
- For filtering, hour window favors Vortex (finer row-group pruning) and day window favors PcoPack
- pcodec reference (numeric columns only): pco L8 333.6 MB, parquet+zstd1 456.5 MB

### OpenStreetMap

Geofabrik Austria PBF. Filters are padded boxes on the centroid: cross-border geometry puts a measurable fraction of nodes outside the nominal bbox. Ways and relations have no natural scalar filter.

#### Node (85929052 rows)

| Metric       | PcoPack  | PcoPack (lossy) | Vortex   | columnar | serde_columnar | msgpack |
|--------------|----------|-----------------|----------|----------|----------------|---------|
| Size         | 370.6 MB | 351.4 MB        | 386.8 MB | 1.2 GB   | 1.1 GB         | 1.3 GB  |
| Serialize    | 5.6 s    | 4.8 s           | 20.7 s   | 6.5 s    | 6.3 s          | 7.3 s   |
| Deserialize  | 3.8 s    | 3.6 s           | 4.6 s    | 2.2 s    | 3.0 s          | 6.0 s   |
| Filter 0.18% | 848.3 ms | 769.9 ms        | 525.2 ms | 2.1 s    | 3.1 s          | 6.4 s   |
| Filter 4.66% | 1.2 s    | 1.2 s           | 1.6 s    | 2.2 s    | 3.1 s          | 6.4 s   |
| Reduce 0.18% | 506.6 ms | 425.6 ms        | 417.0 ms | 2.2 s    | 3.1 s          | 6.4 s   |
| Reduce 4.66% | 556.5 ms | 458.1 ms        | 761.6 ms | 2.3 s    | 3.1 s          | 6.2 s   |

PcoPack (lossy) uses `float_round = 7`, cutting node size ~5% by dropping sub-centimeter coordinate precision.

#### Way (9172339 rows)

| Metric      | PcoPack  | Vortex   | columnar | serde_columnar | msgpack  |
|-------------|----------|----------|----------|----------------|----------|
| Size        | 194.1 MB | 206.3 MB | 384.8 MB | 448.2 MB       | 478.4 MB |
| Serialize   | 5.6 s    | 12.6 s   | 4.0 s    | 3.4 s          | 4.0 s    |
| Deserialize | 4.2 s    | 4.2 s    | 1.5 s    | 3.2 s          | 3.5 s    |
| Reduce 100% | 630.4 ms | 1.1 s    | 1.5 s    | 3.1 s          | 3.9 s    |

#### Relation (175564 rows)

| Metric      | PcoPack  | Vortex   | columnar | serde_columnar | msgpack  |
|-------------|----------|----------|----------|----------------|----------|
| Size        | 17.8 MB  | 18.0 MB  | 19.5 MB  | 16.6 MB        | 19.1 MB  |
| Serialize   | 247.8 ms | 676.3 ms | 179.8 ms | 203.5 ms       | 188.1 ms |
| Deserialize | 198.5 ms | 289.6 ms | 55.0 ms  | 175.6 ms       | 236.7 ms |
| Reduce 100% | 77.3 ms  | 197.9 ms | 55.2 ms  | 173.2 ms       | 253.7 ms |

### openalex (632276 works)

OpenAlex `works` snapshot: deeply nested JSON flattened to typed structs.

| Metric         | PcoPack  | PcoPack (lossy) | Vortex   | columnar | serde_columnar | msgpack  |
|----------------|----------|-----------------|----------|----------|----------------|----------|
| Size           | 322.1 MB | 316.0 MB        | 334.5 MB | 450.0 MB | 394.5 MB       | 434.6 MB |
| Serialize      | 8.2 s    | 8.5 s           | 12.2 s   | 4.7 s    | 5.7 s          | 4.1 s    |
| Deserialize    | 5.9 s    | 5.9 s           | 5.6 s    | 1.5 s    | 5.2 s          | 4.0 s    |
| Filter <0.01%  | 1.4 s    | 1.4 s           | 122.4 ms | 1.5 s    | 5.1 s          | 4.6 s    |
| Filter 2.96%   | 2.3 s    | 2.3 s           | 1.0 s    | 1.5 s    | 5.1 s          | 4.6 s    |
| Text eq 85.13% | 5.4 s    | 5.4 s           | 4.9 s    | 1.6 s    | 5.2 s          | 4.6 s    |
| Text eq <0.01% | 457.7 ms | 457.7 ms        | 69.3 ms  | 1.5 s    | 5.2 s          | 4.6 s    |
| Reduce 85.13%  | 27.3 ms  | 21.2 ms         | 11.0 ms  | 1.5 s    | 5.2 s          | 4.6 s    |
| Reduce 2.96%   | 9.0 ms   | 8.8 ms          | 4.1 ms   | 1.5 s    | 5.1 s          | 4.6 s    |

- PcoPack (lossy) `float_round = 4` saves 2% because `fwci` and scores carry ~15-digit model noise
- Vortex filters fastest (row-group pruning + predicate pushdown)
- At the production 262,144-row chunk, chunk min/max bounds rarely exclude a sparse-match chunk, so selective PcoPack filters decode nearly every blob (Filter <0.01% costs seconds, not tens of ms); sizing chunks to the capture is a possible future escape hatch
- Vortex compact output is not byte-deterministic run to run: its size varied 0.3% between the two passes
