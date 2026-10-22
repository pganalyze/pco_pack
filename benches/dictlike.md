# Dictionary-like serialization and filter performance

## What this tests, and why

Real dictionary-like integer columns: a few to a few hundred distinct values
repeated across millions of rows. The columns come from openalex
(`work_type`, `oa_status`, and the high-cardinality control `cited_by_count`),
NYC taxi (pickup and dropoff location ids), and pganalyze backend snapshots
(`backend_type`, `backend_state`, `wait_event_type`). Each column is cut
into 262,144-row blobs, the production chunk envelope. Today production
writes plain pco with no metadata, so every filter decodes every blob.

Each table row adds one mechanism to the row above it:

- `bounds, pco`: a 28-byte prefix of exact min, max, and row count; a filter
  outside [min, max] skips the blob, inside it the blob still decodes.
- `bounds+dictionary, ...`: a sorted dictionary tail after the bounds; eq/IN
  skip even inside the bounds by scanning a few varints, for <= 0.2% size
  cost at distinct <= 256.
- Payload codecs compared on size and filter speed: pco over values or
  indexes, msgpack+zstd, zstd over raw u8/u16 indexes, run-length pairs,
  and separate dictionary streams.

An index is a row's position in the sorted dictionary: with [8, 17, 23],
the row holding 17 stores index 1, so `= 17` becomes a byte compare against
1. At distinct <= 256 every index fits in one byte; the u16 rows price the
gate above it.

Tables report total compressed size (MB), serialize time (ms), and query
time (ms) per column for four filters: eq most-common, eq rarest, IN of the
two rarest, and an absent value above max. `(n)` = blobs the guard skipped
without decoding. Every row asserts exact hit counts against a value
histogram.


## Summary

The winner is bounds prefix + dictionary tail + payload chosen per column
by a write-time duel between `bounds+dictionary, zstd u8 indexes` and
`bounds+dictionary, pco values`. Each rung of the ladder earns its bytes:

- Bounds are the cheapest metadata there is, 28 uncompressed bytes per blob,
  and they move every provably-out-of-range query to 0.0 ms and skip most
  blobs for rare values whose blob-local range excludes them (taxi
  pu_location_id eq rare: 72 of 74 blobs skipped). Bounds are blind inside
  their own range though: on cyclic enum columns every blob holds every
  value, so filtering a common value still decodes everything.
- The dictionary tail closes exactly that gap. Presence becomes provable
  per blob for <= 0.2% size cost (distinct <= 256), taking selective eq/IN
  to near-zero work: taxi IN 2 ~40 ms today vs ~2.5 ms, wait_event_type eq
  rare ~20 ms with no skip vs 0.4 ms (50 of 51 blobs skipped).
- With the guard paid for, the payload is free to be picked by size alone:
  `bounds+dictionary, zstd u8 indexes` where zstd beats pco because readers cycle
  values in repeating patterns zstd matches as sequences (work_type -9%,
  backend_type -50%, wait_event_type -21%); the one-byte-per-row payload
  also keeps eq scans 2-4x faster than upstream. `bounds+dictionary, pco values`
  where pco wins size (oa_status, backend_state, taxi location ids), which
  holds size at today's level under the same guard.

Measured dead ends:

- pco TryDict: every dictionary-like column here is positive-only, so its
unsigned dict mode does engage after a u64 cast, yet it measured no size
change at up to 3x serialize cost, and its value dictionary is only readable by
parsing pco chunk metadata: it duplicates what the bounds tail already
exposes through a plain byte scan.
- msgpack in the payload path: msgpack+zstd reaches the same size as raw
indexes but pays integer deserialization in every filter, 4-6x slower than
index scans.
- run-length indexes: real-data runs average ~4 rows, so (index, run) varint
pairs inflate payloads 180-390%; zstd already captures those runs through
cyclic matches.
- zstd over u16 indexes: measured above the 256 gate, where u8 indexes are
impossible. It loses on size (taxi location ids, d=261 and 262: 21.4 vs
18.0 and 21.2 vs 18.2 MB over `bounds+dictionary, pco values`) and never wins the
filter (eq common up to ~25% slower, eq rare and IN no better, identical
skip counts), while pco already packs these columns under one byte per
row. Below the gate u16 is not even offered: production picks the
narrower payload whenever it fits, so the wider one never contests. The
only column where u16 indexes filtered faster than pco is the 0.3 MB
cited_by_count control, too small a stage to trade size for. The gate at
distinct <= 256 is therefore measured, not assumed.


## Runtime heuristics

Writer, per chunk/column:

1. Census pass: exact distinct dictionary, min/max, row count, and the index
   stream (each row's position in the sorted dictionary). One scan; every
   part is needed anyway for the guard or the dictionary payload.
2. Emit the dictionary tail whenever the census proves it a small share of
   the blob: free at the gate's edge (taxi location ids tie at 18.0 to
   18.2 MB with and without the tail, and it buys IN skips over 69 of 74
   blobs), +3.9% at d=3267, where it stops paying. Do not gate the tail on
   the 256 boundary.
3. Gate the index payload on distinct <= 256. Above it bounds + dictionary + pco
   values, no duel: indexes stop fitting in one byte, and zstd over u16
   indexes measured +16% to +19% bytes over pco at the gate's edge (taxi,
   d=261 and 262) with no filter win.
4. Below the gate: duel pco(values) vs zstd(indexes). Pick
   zstd(indexes) only when it is >= ~5% smaller; measured winners clear
   the margin (-9%, -50%, -21%) with filters equal or faster, losers
   (+14%, +22%) stay on pco.
5. The duel's rival encode is cheap by construction: zstd runs over the
   dense index stream, one byte per row, no msgpack serialize or
   deserialize anywhere. The full pass is gated by a two-window probe: the
   head and tail 32KB of the index stream run through one streaming
   encoder (the tail compresses against the head as live history, so
   long-range periodicity is not mistaken for cold-start noise) and the
   worse window's rate projected across the blob; columns whose estimate
   cannot reach the margin never pay the full pass.

Reader:

- Bounds guard first (min/max, row count): provably-out-of-range queries
  answer at 0.0 ms.
- Exact-dictionary membership turns eq/IN into a varint scan of the blob's
  dictionary; an absent target skips the blob entirely. The scan costs are
  what bound the tail's size budget (a few percent of the blob).
- Surviving blobs decode per codec tag. zstd(indexes) blobs inflate to
  bytes; resolve the query's target indexes once with a binary search in
  the sorted dictionary, then byte-compare. A sorted dictionary makes even
  range predicates byte-range compares, so no row materialization is ever
  needed for numeric filters. Production inflates and materializes today;
  the byte-scan engine in the `bounds+dictionary, zstd u8 indexes` row is the
  fast path that framing the payload as raw index bytes makes possible.

Not needed: no cyclic predictor (at this chunk size the duel measures
cheaper than guessing), no pco TryDict, no msgpack payloads, no RLE.


## Framing key

| Approach                                | Framing                                                                                                                               |
|-----------------------------------------|---------------------------------------------------------------------------------------------------------------------------------------|
| upstream: pco, no bounds                | no metadata at all; production upstream today, every filter decodes every blob                                                        |
| bounds, pco                             | uncompressed bounds prefix with min, max, and row count; skips only queries provably outside [min, max]                               |
| bounds+dictionary, pco values           | bounds + sorted dictionary; pco payload of values                                                                                     |
| bounds+dictionary, msgpack+zstd values  | bounds + dictionary; payload is zstd(msgpack(values))                                                                                 |
| bounds+dictionary, pco indexes          | bounds + dictionary; payload is pco of indexes                                                                                        |
| bounds+dictionary, msgpack+zstd indexes | bounds + dictionary; payload is zstd(msgpack(indexes))                                                                                |
| bounds+dictionary, rle indexes          | bounds + dictionary; payload is run-length coded (index, run) varint pairs                                                            |
| bounds+dictionary, zstd u8 indexes      | bounds + dictionary; payload is zstd over one index byte per row; needs distinct count <= 256                                         |
| bounds+dictionary, zstd u16 indexes     | bounds + dictionary; payload is zstd over two little-endian index bytes per row; covers distinct counts past the 256 gate up to 65536 |
| bounds, pco dict + pco indexes          | bounds; length-prefixed pco(dict) then pco(indexes); the dictionary lives in its own dict stream the reader must decompress           |
| bounds, pco dict (TryDict)              | bounds; payload is the column cast to u64 encoded by pco in forced TryDict mode; the dictionary lives inside the pco chunk header     |
| bounds, mp+zstd dict + mp+zstd indexes  | bounds; length-prefixed zstd(msgpack(dict)) then zstd(msgpack(indexes))                                                               |
| bounds, msgpack+zstd values             | bounds only, no dictionary; payload is zstd(msgpack(values))                                                                          |


## openalex work_type - 632276 rows, 20 distinct, 3 blobs

| Approach                                | Size MB | ser ms | eq common | eq rare | IN 2    | absent  |
|-----------------------------------------|---------|--------|-----------|---------|---------|---------|
| upstream: pco, no bounds                | 0.2     | 8.3    | 1.2       | 1.1     | 1.3     | 1.1     |
| bounds, pco                             | 0.2     | 4.4    | 1.2       | 0.5 (2) | 1.4     | 0.0 (3) |
| bounds+dictionary, pco values           | 0.2     | 5.4    | 1.2       | 0.5 (2) | 1.1 (1) | 0.0 (3) |
| bounds+dictionary, msgpack+zstd values  | 0.1     | 4.7    | 2.0       | 0.8 (2) | 1.8 (1) | 0.0 (3) |
| bounds+dictionary, pco indexes          | 0.2     | 8.4    | 1.7       | 0.5 (2) | 1.0 (1) | 0.0 (3) |
| bounds+dictionary, msgpack+zstd indexes | 0.1     | 7.3    | 2.6       | 0.9 (2) | 1.9 (1) | 0.0 (3) |
| bounds+dictionary, rle indexes          | 0.7     | 6.8    | 0.5       | 0.2 (2) | 0.3 (1) | 0.0 (3) |
| bounds+dictionary, zstd u8 indexes      | 0.1     | 6.2    | 0.6       | 0.3 (2) | 0.6 (1) | 0.0 (3) |
| bounds, pco dict + pco indexes          | 0.2     | 8.4    | 1.8       | 0.5 (2) | 1.1 (1) | 0.0 (3) |
| bounds, pco dict (TryDict)              | 0.2     | 13.6   | 1.5       | 0.6 (2) | 1.3 (1) | 0.0 (3) |
| bounds, mp+zstd dict + mp+zstd indexes  | 0.1     | 7.3    | 2.6       | 0.9 (2) | 1.9 (1) | 0.0 (3) |
| bounds, msgpack+zstd values             | 0.1     | 3.4    | 2.0       | 0.8 (2) | 2.2     | 0.0 (3) |


## openalex oa_status - 632276 rows, 6 distinct, 3 blobs

| Approach                                | Size MB | ser ms | eq common | eq rare | IN 2 | absent  |
|-----------------------------------------|---------|--------|-----------|---------|------|---------|
| upstream: pco, no bounds                | 0.2     | 3.3    | 1.0       | 1.0     | 1.3  | 0.9     |
| bounds, pco                             | 0.2     | 3.8    | 0.9       | 0.9     | 1.3  | 0.0 (3) |
| bounds+dictionary, pco values           | 0.2     | 4.7    | 1.0       | 0.9     | 1.3  | 0.0 (3) |
| bounds+dictionary, msgpack+zstd values  | 0.2     | 4.6    | 2.0       | 1.9     | 2.3  | 0.0 (3) |
| bounds+dictionary, pco indexes          | 0.2     | 7.8    | 1.5       | 1.1     | 1.4  | 0.0 (3) |
| bounds+dictionary, msgpack+zstd indexes | 0.2     | 7.3    | 2.7       | 2.2     | 2.5  | 0.0 (3) |
| bounds+dictionary, rle indexes          | 0.7     | 6.9    | 0.7       | 0.4     | 0.6  | 0.0 (3) |
| bounds+dictionary, zstd u8 indexes      | 0.2     | 6.2    | 0.6       | 0.6     | 0.8  | 0.0 (3) |
| bounds, pco dict + pco indexes          | 0.2     | 7.8    | 1.6       | 1.2     | 1.4  | 0.0 (3) |
| bounds, pco dict (TryDict)              | 0.2     | 13.5   | 1.3       | 1.3     | 1.6  | 0.0 (3) |
| bounds, mp+zstd dict + mp+zstd indexes  | 0.2     | 7.4    | 2.7       | 2.2     | 2.5  | 0.0 (3) |
| bounds, msgpack+zstd values             | 0.2     | 3.6    | 2.0       | 1.9     | 2.2  | 0.0 (3) |


## openalex cited_by_count (ctrl) - 632276 rows, 3267 distinct, 3 blobs

| Approach                                | Size MB | ser ms | eq common | eq rare | IN 2    | absent  |
|-----------------------------------------|---------|--------|-----------|---------|---------|---------|
| upstream: pco, no bounds                | 0.3     | 12.3   | 1.2       | 1.1     | 1.3     | 1.1     |
| bounds, pco                             | 0.3     | 5.0    | 1.2       | 1.1     | 1.3     | 0.0 (3) |
| bounds+dictionary, pco values           | 0.3     | 9.6    | 1.2       | 0.5 (2) | 1.1 (1) | 0.0 (3) |
| bounds+dictionary, msgpack+zstd values  | 0.3     | 5.0    | 2.1       | 0.9 (2) | 1.8 (1) | 0.0 (3) |
| bounds+dictionary, pco indexes          | 0.3     | 9.3    | 1.5       | 0.5 (2) | 1.0 (1) | 0.0 (3) |
| bounds+dictionary, msgpack+zstd indexes | 0.3     | 7.9    | 2.5       | 1.0 (2) | 1.8 (1) | 0.0 (3) |
| bounds+dictionary, rle indexes          | 0.8     | 7.6    | 0.8       | 0.3 (2) | 0.4 (1) | 0.0 (3) |
| bounds+dictionary, zstd u16 indexes     | 0.4     | 8.0    | 1.0       | 0.4 (2) | 0.8 (1) | 0.0 (3) |
| bounds, pco dict + pco indexes          | 0.3     | 9.7    | 1.5       | 0.5 (2) | 1.1 (1) | 0.0 (3) |
| bounds, pco dict (TryDict)              | 0.3     | 14.3   | 1.5       | 0.6 (2) | 1.3 (1) | 0.0 (3) |
| bounds, mp+zstd dict + mp+zstd indexes  | 0.3     | 8.0    | 2.6       | 1.0 (2) | 1.9 (1) | 0.0 (3) |
| bounds, msgpack+zstd values             | 0.3     | 3.6    | 2.1       | 2.1     | 2.2     | 0.0 (3) |


## taxi pu_location_id - 19144903 rows, 262 distinct, 74 blobs

| Approach                                | Size MB | ser ms | eq common | eq rare  | IN 2      | absent   |
|-----------------------------------------|---------|--------|-----------|----------|-----------|----------|
| upstream: pco, no bounds                | 18.2    | 182.2  | 35.1      | 34.6     | 39.6      | 35.0     |
| bounds, pco                             | 18.2    | 197.4  | 34.7      | 1.0 (72) | 39.2      | 0.0 (74) |
| bounds+dictionary, pco values           | 18.2    | 261.5  | 35.0      | 1.0 (72) | 2.7 (69)  | 0.0 (74) |
| bounds+dictionary, msgpack+zstd values  | 21.9    | 257.7  | 143.1     | 3.9 (72) | 10.0 (69) | 0.0 (74) |
| bounds+dictionary, msgpack+zstd indexes | 21.4    | 334.0  | 143.4     | 3.9 (72) | 9.8 (69)  | 0.0 (74) |
| bounds+dictionary, rle indexes          | 51.1    | 288.4  | 37.8      | 1.0 (72) | 2.6 (69)  | 0.0 (74) |
| bounds+dictionary, zstd u16 indexes     | 21.2    | 320.5  | 41.3      | 1.1 (72) | 2.9 (69)  | 0.0 (74) |
| bounds, pco dict (TryDict)              | 18.0    | 439.5  | 43.0      | 1.4 (72) | 3.4 (69)  | 0.2 (74) |
| bounds, msgpack+zstd values             | 21.8    | 194.2  | 142.4     | 3.8 (72) | 144.9     | 0.0 (74) |


## taxi do_location_id - 19144903 rows, 261 distinct, 74 blobs

| Approach                                | Size MB | ser ms | eq common | eq rare   | IN 2      | absent   |
|-----------------------------------------|---------|--------|-----------|-----------|-----------|----------|
| upstream: pco, no bounds                | 18.0    | 181.9  | 34.9      | 34.8      | 39.4      | 34.7     |
| bounds, pco                             | 18.0    | 197.2  | 34.8      | 34.7      | 39.1      | 0.0 (74) |
| bounds+dictionary, pco values           | 18.0    | 260.4  | 34.9      | 4.4 (65)  | 18.0 (40) | 0.0 (74) |
| bounds+dictionary, msgpack+zstd values  | 22.1    | 263.3  | 147.9     | 18.8 (65) | 70.6 (40) | 0.0 (74) |
| bounds+dictionary, msgpack+zstd indexes | 21.7    | 341.3  | 153.1     | 18.7 (65) | 70.5 (40) | 0.0 (74) |
| bounds+dictionary, rle indexes          | 51.8    | 287.2  | 40.4      | 4.6 (65)  | 17.2 (40) | 0.0 (74) |
| bounds+dictionary, zstd u16 indexes     | 21.4    | 322.0  | 40.8      | 5.1 (65)  | 19.5 (40) | 0.0 (74) |
| bounds, pco dict (TryDict)              | 17.9    | 439.2  | 42.8      | 5.4 (65)  | 22.2 (40) | 0.2 (74) |
| bounds, msgpack+zstd values             | 22.0    | 200.3  | 149.7     | 151.0     | 154.9     | 0.0 (74) |


## backends backend_type - 13328012 rows, 11 distinct, 51 blobs

| Approach                                | Size MB | ser ms | eq common | eq rare | IN 2 | absent   |
|-----------------------------------------|---------|--------|-----------|---------|------|----------|
| upstream: pco, no bounds                | 4.4     | 76.7   | 19.7      | 19.7    | 23.3 | 19.6     |
| bounds, pco                             | 4.4     | 86.5   | 19.5      | 19.4    | 23.0 | 0.0 (51) |
| bounds+dictionary, pco values           | 4.4     | 109.6  | 19.3      | 19.6    | 23.6 | 0.0 (51) |
| bounds+dictionary, msgpack+zstd values  | 2.2     | 84.1   | 37.8      | 37.2    | 41.0 | 0.0 (51) |
| bounds+dictionary, pco indexes          | 4.4     | 172.4  | 31.0      | 21.7    | 25.2 | 0.0 (51) |
| bounds+dictionary, msgpack+zstd indexes | 2.2     | 144.7  | 51.2      | 41.9    | 45.4 | 0.0 (51) |
| bounds+dictionary, rle indexes          | 16.8    | 148.5  | 18.2      | 6.8     | 9.2  | 0.0 (51) |
| bounds+dictionary, zstd u8 indexes      | 2.2     | 122.1  | 9.3       | 9.2     | 14.0 | 0.0 (51) |
| bounds, pco dict (TryDict)              | 4.4     | 271.2  | 26.8      | 26.4    | 30.4 | 0.1 (51) |
| bounds, msgpack+zstd values             | 2.2     | 63.2   | 38.9      | 38.5    | 42.1 | 0.0 (51) |


## backends backend_state - 13328012 rows, 4 distinct, 51 blobs

| Approach                                | Size MB | ser ms | eq common | eq rare  | IN 2 | absent   |
|-----------------------------------------|---------|--------|-----------|----------|------|----------|
| upstream: pco, no bounds                | 1.9     | 56.4   | 19.5      | 19.8     | 24.8 | 19.6     |
| bounds, pco                             | 1.9     | 66.8   | 19.7      | 1.1 (48) | 24.4 | 0.0 (51) |
| bounds+dictionary, pco values           | 1.9     | 84.5   | 19.8      | 1.2 (48) | 24.2 | 0.0 (51) |
| bounds+dictionary, msgpack+zstd values  | 2.3     | 80.6   | 40.5      | 2.4 (48) | 45.1 | 0.0 (51) |
| bounds+dictionary, pco indexes          | 1.9     | 146.9  | 29.8      | 1.3 (48) | 24.8 | 0.0 (51) |
| bounds+dictionary, msgpack+zstd indexes | 2.3     | 140.2  | 52.4      | 2.5 (48) | 47.1 | 0.0 (51) |
| bounds+dictionary, rle indexes          | 6.8     | 138.7  | 4.6       | 0.2 (48) | 4.4  | 0.0 (51) |
| bounds+dictionary, zstd u8 indexes      | 2.3     | 116.8  | 12.3      | 0.7 (48) | 12.2 | 0.0 (51) |
| bounds, pco dict (TryDict)              | 1.9     | 255.7  | 26.6      | 1.6 (48) | 31.0 | 0.1 (51) |
| bounds, msgpack+zstd values             | 2.3     | 63.2   | 40.3      | 2.4 (48) | 45.4 | 0.0 (51) |


## backends wait_event_type - 13328012 rows, 10 distinct, 51 blobs

| Approach                                | Size MB | ser ms | eq common | eq rare  | IN 2 | absent   |
|-----------------------------------------|---------|--------|-----------|----------|------|----------|
| upstream: pco, no bounds                | 3.6     | 74.6   | 19.5      | 19.2     | 22.9 | 19.4     |
| bounds, pco                             | 3.6     | 84.2   | 19.7      | 19.5     | 22.9 | 0.0 (51) |
| bounds+dictionary, pco values           | 3.6     | 104.8  | 19.4      | 0.4 (50) | 22.9 | 0.0 (51) |
| bounds+dictionary, msgpack+zstd values  | 2.9     | 85.4   | 39.1      | 0.8 (50) | 41.8 | 0.0 (51) |
| bounds+dictionary, pco indexes          | 3.6     | 167.0  | 30.3      | 0.4 (50) | 21.5 | 0.0 (51) |
| bounds+dictionary, msgpack+zstd indexes | 2.9     | 147.0  | 52.5      | 0.8 (50) | 43.2 | 0.0 (51) |
| bounds+dictionary, rle indexes          | 13.2    | 147.5  | 14.3      | 0.1 (50) | 6.3  | 0.0 (51) |
| bounds+dictionary, zstd u8 indexes      | 2.9     | 123.9  | 10.7      | 0.2 (50) | 10.8 | 0.0 (51) |
| bounds, pco dict (TryDict)              | 3.6     | 275.5  | 26.6      | 0.7 (50) | 28.7 | 0.1 (51) |
| bounds, msgpack+zstd values             | 2.9     | 66.7   | 38.6      | 39.3     | 42.1 | 0.0 (51) |

