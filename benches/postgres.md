# Postgres


Real Postgres monitoring datasets sourced from pganalyze.

### backend_snapshots (13328012 rows)

#### PcoPack scenarios

| Metric        | full     | time_round | Timeline | Timeline 10s |
|---------------|----------|------------|----------|--------------|
| Size          | 46.8 MB  | 39.1 MB    | 18.1 MB  | 17.6 MB      |
| Serialize     | 2.9 s    | 2.9 s      | 1.4 s    | 1.1 s        |
| Deserialize   | 1.2 s    | 1.2 s      | 456.3 ms | 390.2 ms     |
| Filter 0.49%  | 63.0 ms  | 64.1 ms    | 154.3 ms | 84.6 ms      |
| Filter 29.25% | 749.7 ms | 750.5 ms   | 439.8 ms | 379.9 ms     |
| Filter 77.25% | 1.1 s    | 1.1 s      | 364.2 ms | 304.8 ms     |
| Filter <0.01% | 10.8 ms  | 10.6 ms    | 10.4 ms  | 10.2 ms      |
| Reduce 0.49%  | 11.8 ms  | 12.1 ms    | 87.1 ms  | 24.9 ms      |
| Reduce 29.25% | 118.4 ms | 118.8 ms   | 59.3 ms  | 58.8 ms      |

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
| Serialize     | 1.3 s    | 3.0 s    | 3.0 s      | 4.0 s    | 3.8 s       |
| Deserialize   | 642.4 ms | 5.6 s    | 5.5 s      | 1.9 s    | 1.9 s       |
| Filter 0.55%  | 535.6 ms | 1.6 s    | 1.6 s      | 516.8 ms | 501.7 ms    |
| Filter 12.48% | 535.8 ms | 4.2 s    | 4.2 s      | 755.4 ms | 735.7 ms    |
| Filter 12.97% | 17.9 ms  | 130.2 ms | 126.5 ms   | 42.6 ms  | 42.4 ms     |
| Text miss     | 4.2 ms   | 3.8 ms   | 3.6 ms     | 2.8 ms   | 2.6 ms      |
| Reduce 0.55%  | 2.8 ms   | 2.7 ms   | 2.9 ms     | 2.3 ms   | 2.3 ms      |
| Reduce 12.48% | 2.5 ms   | 2.4 ms   | 2.4 ms     | 2.5 ms   | 1.9 ms      |

- PcoPack outperforms competitors even when storing the JSON as text. When storing JSON, PcoPack reduces compressed size but much more time is spent serializing.
- JSON lossy uses `float_round = 2, time_round = 1s` to cut ~7%
- The arena (AKA struct of arrays) replaces the JSON tree with a flat arena of typed fields. Arena is ~30% smaller than JSON, and lossy arena trims ~4% more. Arena deserialization is ~4x faster than JSON, and filters are 3-6x faster
- The whole capture fits one chunk, so chunk min/max bounds exclude nothing and selective filters decode every column (Filter 0.55% costs ~540 ms text / ~1.7 s JSON); dynamic chunk size by data volume is a possible future escape hatch

#### Other crates

| Metric        | Vortex   | columnar | serde_columnar | msgpack  |
|---------------|----------|----------|----------------|----------|
| Size          | 122.5 MB | 127.1 MB | 123.3 MB       | 110.3 MB |
| Serialize     | 4.9 s    | 1.5 s    | 1.5 s          | 2.0 s    |
| Deserialize   | 677.1 ms | 522.4 ms | 701.9 ms       | 4.7 s    |
| Filter 0.55%  | 16.9 ms  | 520.8 ms | 705.4 ms       | 5.9 s    |
| Filter 12.48% | 525.2 ms | 521.6 ms | 703.1 ms       | 5.9 s    |
| Filter 12.97% | 446.2 ms | 521.8 ms | 701.8 ms       | 5.8 s    |
| Text miss     | 0.2 ms   | 522.6 ms | 705.7 ms       | 5.9 s    |
| Reduce 0.55%  | 0.1 ms   | 521.6 ms | 703.0 ms       | 5.8 s    |
| Reduce 12.48% | 1.3 ms   | 522.1 ms | 706.0 ms       | 5.8 s    |

- Vortex, columnar, and serde_columnar store the JSON as text; the same as the PcoPack text scenario.

### query_stats_1min (29970299 rows)

| Metric        | PcoPack  | PcoPack time_round | Vortex   | columnar | serde_columnar | msgpack  |
|---------------|----------|--------------------|----------|----------|----------------|----------|
| Size          | 117.1 MB | 117.2 MB           | 129.8 MB | 191.7 MB | 179.4 MB       | 236.2 MB |
| Serialize     | 4.4 s    | 4.3 s              | 6.0 s    | 4.6 s    | 2.9 s          | 5.2 s    |
| Deserialize   | 1.3 s    | 1.1 s              | 2.1 s    | 1.4 s    | 2.4 s          | 4.4 s    |
| Filter 0.31%  | 115.8 ms | 113.3 ms           | 35.6 ms  | 1.4 s    | 2.4 s          | 4.5 s    |
| Filter 18.19% | 704.6 ms | 707.3 ms           | 770.5 ms | 1.4 s    | 2.4 s          | 4.5 s    |
| Filter 0.05%  | 7.7 ms   | 7.4 ms             | 7.4 ms   | 1.4 s    | 2.4 s          | 4.5 s    |
| Reduce 0.31%  | 33.4 ms  | 33.8 ms            | 9.2 ms   | 1.4 s    | 2.4 s          | 4.5 s    |
| Reduce 18.19% | 232.0 ms | 235.4 ms           | 203.5 ms | 1.5 s    | 2.4 s          | 4.7 s    |

### query_stats_1hour (1041799 rows)

| Metric        | PcoPack  | PcoPack time_round | Vortex   | columnar | serde_columnar | msgpack  |
|---------------|----------|--------------------|----------|----------|----------------|----------|
| Size          | 5.9 MB   | 5.9 MB             | 7.1 MB   | 10.0 MB  | 9.1 MB         | 11.6 MB  |
| Serialize     | 169.8 ms | 166.5 ms           | 251.3 ms | 152.9 ms | 117.3 ms       | 201.3 ms |
| Deserialize   | 42.8 ms  | 42.1 ms            | 76.0 ms  | 51.1 ms  | 90.0 ms        | 165.2 ms |
| Filter 0.63%  | 24.6 ms  | 24.1 ms            | 1.6 ms   | 50.6 ms  | 89.5 ms        | 166.3 ms |
| Filter 58.45% | 38.6 ms  | 37.6 ms            | 56.4 ms  | 50.2 ms  | 89.5 ms        | 166.5 ms |
| Filter 0.03%  | 0.4 ms   | 0.4 ms             | 4.9 ms   | 50.4 ms  | 89.0 ms        | 166.5 ms |
| Reduce 0.63%  | 7.2 ms   | 7.1 ms             | 0.4 ms   | 51.6 ms  | 89.4 ms        | 165.5 ms |
| Reduce 58.45% | 13.3 ms  | 13.0 ms            | 13.8 ms  | 59.2 ms  | 94.1 ms        | 168.0 ms |

- PcoPack is the smallest at both granularities
- `time_round` is less effective because data is captured once a minute versus backend_snapshots which are captured once every 10 seconds
