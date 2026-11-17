# Postgres


Real Postgres monitoring datasets sourced from pganalyze.

### backend_snapshots (13328012 rows)

#### PcoPack scenarios

| Metric        | full     | time_round | Timeline | Timeline 10s |
|---------------|----------|------------|----------|--------------|
| Size          | 46.8 MB  | 39.1 MB    | 18.1 MB  | 17.6 MB      |
| Serialize     | 2.7 s    | 2.8 s      | 1.3 s    | 1.1 s        |
| Deserialize   | 1.1 s    | 1.2 s      | 436.6 ms | 374.1 ms     |
| Filter 0.49%  | 60.1 ms  | 61.0 ms    | 147.4 ms | 81.4 ms      |
| Filter 29.25% | 693.6 ms | 694.7 ms   | 405.4 ms | 352.9 ms     |
| Filter 77.25% | 994.8 ms | 1.0 s      | 332.2 ms | 272.2 ms     |
| Filter <0.01% | 10.5 ms  | 9.9 ms     | 10.1 ms  | 9.7 ms       |
| Reduce 0.49%  | 11.4 ms  | 11.3 ms    | 83.5 ms  | 24.1 ms      |
| Reduce 29.25% | 115.2 ms | 115.0 ms   | 56.4 ms  | 57.5 ms      |

- `time_round = 1s` drops the tick jitter and cuts ~17% off full fidelity
- `Timeline` cuts row count by ~71%, serialized size ~62%, and serialize time ~50%
- 10 second `Timeline` buckets trim only ~2% size, but significantly improve runtime

#### Other crates

| Metric        | Vortex   | columnar | serde_columnar | msgpack |
|---------------|----------|----------|----------------|---------|
| Size          | 84.2 MB  | 68.8 MB  | 69.3 MB        | 80.2 MB |
| Serialize     | 3.5 s    | 932.6 ms | 2.2 s          | 3.8 s   |
| Deserialize   | 1.4 s    | 499.6 ms | 1.5 s          | 4.3 s   |
| Filter 0.49%  | 10.4 ms  | 473.6 ms | 1.6 s          | 4.4 s   |
| Filter 29.25% | 591.3 ms | 474.0 ms | 1.6 s          | 4.5 s   |
| Filter 77.25% | 1.2 s    | 475.3 ms | 1.6 s          | 4.5 s   |
| Filter <0.01% | 1.4 ms   | 479.0 ms | 1.5 s          | 4.3 s   |
| Reduce 0.49%  | 1.7 ms   | 487.3 ms | 1.6 s          | 4.5 s   |
| Reduce 29.25% | 79.2 ms  | 524.3 ms | 1.6 s          | 4.8 s   |

- PcoPack compresses ~40% smaller than competitors, with lossy/Timeline scenarios doing even better
- Vortex outperforms during some filter and reduce operations
- columnar deserializes fastest among the comparison crates

### query_explains (237651 rows)

This data model is dominated by large Postgres EXPLAIN plans represented as JSON.

#### PcoPack scenarios

| Metric        | text     | JSON     | JSON lossy | arena    | arena lossy |
|---------------|----------|----------|------------|----------|-------------|
| Size          | 91.7 MB  | 82.0 MB  | 76.0 MB    | 57.7 MB  | 55.5 MB     |
| Serialize     | 1.2 s    | 3.0 s    | 3.0 s      | 3.8 s    | 3.7 s       |
| Deserialize   | 683.5 ms | 7.6 s    | 7.5 s      | 1.8 s    | 1.8 s       |
| Filter 0.55%  | 515.6 ms | 1.6 s    | 1.6 s      | 502.1 ms | 487.8 ms    |
| Filter 12.48% | 527.8 ms | 4.4 s    | 4.4 s      | 735.0 ms | 724.4 ms    |
| Filter 12.97% | 20.2 ms  | 172.0 ms | 171.1 ms   | 42.1 ms  | 42.0 ms     |
| Text miss     | 5.4 ms   | 5.0 ms   | 4.7 ms     | 4.0 ms   | 4.0 ms      |
| Reduce 0.55%  | 2.7 ms   | 2.5 ms   | 2.5 ms     | 2.2 ms   | 2.3 ms      |
| Reduce 12.48% | 2.4 ms   | 2.4 ms   | 2.1 ms     | 2.0 ms   | 1.8 ms      |

- PcoPack outperforms competitors even when storing the JSON as text. When storing JSON, PcoPack reduces compressed size but much more time is spent serializing.
- JSON lossy uses `float_round = 2, time_round = 1s` to cut ~7%
- The arena (AKA struct of arrays) replaces the JSON tree with a flat arena of typed fields. Arena is ~30% smaller than JSON, and lossy arena trims ~4% more. Arena deserialization is ~4x faster than JSON, and filters are 3-6x faster
- The whole capture fits one chunk, so chunk min/max bounds exclude nothing and selective filters decode every column (Filter 0.55% costs ~540 ms text / ~1.7 s JSON); dynamic chunk size by data volume is a possible future escape hatch

#### Other crates

| Metric        | Vortex   | columnar | serde_columnar | msgpack  |
|---------------|----------|----------|----------------|----------|
| Size          | 122.5 MB | 127.1 MB | 123.3 MB       | 110.3 MB |
| Serialize     | 5.0 s    | 1.6 s    | 1.6 s          | 2.1 s    |
| Deserialize   | 695.8 ms | 542.1 ms | 745.2 ms       | 5.0 s    |
| Filter 0.55%  | 17.1 ms  | 539.7 ms | 763.5 ms       | 6.2 s    |
| Filter 12.48% | 533.0 ms | 538.3 ms | 762.0 ms       | 6.2 s    |
| Filter 12.97% | 458.3 ms | 537.4 ms | 750.8 ms       | 6.2 s    |
| Text miss     | 0.2 ms   | 537.9 ms | 748.9 ms       | 6.2 s    |
| Reduce 0.55%  | 0.2 ms   | 543.7 ms | 749.6 ms       | 6.2 s    |
| Reduce 12.48% | 1.4 ms   | 540.4 ms | 811.5 ms       | 6.1 s    |

- Vortex, columnar, and serde_columnar store the JSON as text; the same as the PcoPack text scenario.

### query_stats_1min (29970299 rows)

| Metric        | PcoPack  | PcoPack time_round | Vortex   | columnar | serde_columnar | msgpack  |
|---------------|----------|--------------------|----------|----------|----------------|----------|
| Size          | 117.1 MB | 117.2 MB           | 129.8 MB | 191.7 MB | 179.4 MB       | 236.2 MB |
| Serialize     | 4.2 s    | 4.1 s              | 6.0 s    | 4.6 s    | 2.9 s          | 5.2 s    |
| Deserialize   | 1.7 s    | 1.5 s              | 2.1 s    | 1.4 s    | 2.4 s          | 4.4 s    |
| Filter 0.31%  | 118.7 ms | 116.5 ms           | 35.6 ms  | 1.4 s    | 2.4 s          | 4.5 s    |
| Filter 18.19% | 718.2 ms | 712.3 ms           | 770.5 ms | 1.4 s    | 2.4 s          | 4.5 s    |
| Filter 0.05%  | 8.0 ms   | 7.9 ms             | 7.4 ms   | 1.4 s    | 2.4 s          | 4.5 s    |
| Reduce 0.31%  | 33.2 ms  | 33.1 ms            | 9.2 ms   | 1.4 s    | 2.4 s          | 4.5 s    |
| Reduce 18.19% | 220.6 ms | 221.9 ms           | 203.5 ms | 1.5 s    | 2.4 s          | 4.7 s    |

### query_stats_1hour (1041799 rows)

| Metric        | PcoPack  | PcoPack time_round | Vortex   | columnar | serde_columnar | msgpack  |
|---------------|----------|--------------------|----------|----------|----------------|----------|
| Size          | 5.9 MB   | 5.9 MB             | 7.1 MB   | 10.0 MB  | 9.1 MB         | 11.6 MB  |
| Serialize     | 163.3 ms | 161.2 ms           | 251.3 ms | 152.9 ms | 117.3 ms       | 201.3 ms |
| Deserialize   | 59.8 ms  | 59.4 ms            | 76.0 ms  | 51.1 ms  | 90.0 ms        | 165.2 ms |
| Filter 0.63%  | 24.2 ms  | 23.6 ms            | 1.6 ms   | 50.6 ms  | 89.5 ms        | 166.3 ms |
| Filter 58.45% | 45.9 ms  | 45.2 ms            | 56.4 ms  | 50.2 ms  | 89.5 ms        | 166.5 ms |
| Filter 0.03%  | 0.4 ms   | 0.4 ms             | 4.9 ms   | 50.4 ms  | 89.0 ms        | 166.5 ms |
| Reduce 0.63%  | 6.9 ms   | 6.7 ms             | 0.4 ms   | 51.6 ms  | 89.4 ms        | 165.5 ms |
| Reduce 58.45% | 12.4 ms  | 12.4 ms            | 13.8 ms  | 59.2 ms  | 94.1 ms        | 168.0 ms |

- PcoPack is the smallest at both granularities
- `time_round` is less effective because data is captured once a minute versus backend_snapshots which are captured once every 10 seconds
