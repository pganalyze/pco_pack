# Benchmarks

Sizes and times include compression, measured on an M5 Max. Every codec serializes and deserializes the whole capture in one call, matching how its API is used in production. Times are the minimum of repeated runs, taken from two back-to-back full-suite runs that agree within 0.3% at the median.

`cargo bench` runs the suite one binary at a time, in about 70 minutes, peaking near 50 GB of memory; let it run alone. The comparison crates change rarely, so `cargo bench -- --crates pco_pack` skips most of the work. [Contributions are welcome](https://github.com/pganalyze/pco_pack/issues/3).

The full results live in three files:

- [Synthetic](synthetic.md): per-type compression, struct round trips, timeline deduplication, chunk size effects, and filter timing on generated data
- [Datasets](datasets.md): real external datasets (NYC air quality, NYC taxi, OpenStreetMap Austria, OpenAlex)
- [Postgres](postgres.md): real Postgres monitoring captures sourced from pganalyze

Each file ends its scenario tables with bullets on what the numbers show. Run a single scenario alone with a positional arg, e.g. `cargo bench --bench datasets -- taxi`.

## Summary

Every codec bets on a different kind of redundancy. PcoPack bets that redundancy lives in the schema: numbers trend, timestamps drift, keys repeat, and most float precision is noise. That pays off: every real dataset measured compresses smallest with PcoPack by default, with our optional features offering even better performance.

PcoPack is less effective in some scenarios:
- LZ compression absorbs byte-level repetition for free, so msgpack+zstd wins on clustered zeros, dwells, and repeating enum discriminants
- Vortex filtering is often faster because its per-field metadata can skip entire chunks without decompression
- columnar offers the fastest decode by handing the user references instead of building structs
