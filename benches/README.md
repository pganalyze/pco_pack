# Benchmarks

## Details

- [Synthetic](synthetic.md): various benchmarks on synthetic data
- [Datasets](datasets.md): NYC air quality, NYC taxi, OpenStreetMap Austria, OpenAlex
- [Postgres](postgres.md): Postgres monitoring captures sourced from pganalyze

## Summary

Every codec bets on a different kind of redundancy. PcoPack bets that redundancy lives in the schema: numbers trend, timestamps drift, keys repeat, and most float precision is noise. That pays off: every real dataset compresses smallest with PcoPack, with our optional features offering even better performance.

PcoPack is less effective in some scenarios:
- LZ compression absorbs byte-level repetition for free, so (columnar) msgpack+zstd wins on clustered zeros, dwells, and repeating enum discriminants
- Vortex filtering is often faster because its per-field metadata can skip entire chunks without decompression
- columnar offers the fastest decode by handing the user references instead of building structs

## Usage

- `cargo bench` takes ~70 minutes, peaking near 50 GB of memory
- `cargo bench -- --crates pco_pack` runs benchmarks for a single crate
- `cargo bench --bench datasets -- taxi` runs a single scenario inside a larger benchmark
