# Postgres


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
