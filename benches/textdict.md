# Text-column framing and filter bake-off

Production text (`src/lazy_msgpack.rs`) packs msgpack chunks of 128 rows into one zstd frame per column blob, so every selective read inflates the whole column. This run encodes real text from the pganalyze captures whole-frame and framed (each frame its own zstd frame behind a fingerprint-stamp envelope), adding one mechanism at a time:

- `Eq`: the guard path. Metadata check, decode only matching units, exact compare.
- `gather`: positional survivor masks over the first 5% and last 1% of rows. `head inflate MB` and `tail inflate MB` count the decompressed bytes those gathers pay, which framing removes.
- `gated`: the proposed write-time rule. Frame fat or guard-fitting blobs, keep the rest whole-frame; each table names the per-blob pick.
- `framed 2048 + bloom`: a fingerprint bloom behind stamps that overflow the 64-value cap, letting near-unique columns skip frames too.

Framing keys:

| Approach            | Framing                                                                                                                                                                                                                               |
|---------------------|---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| current: 1 frame    | one zstd frame over msgpack chunks of 128 rows, production bytes; every read inflates the whole blob                                                                                                                                  |
| 1 frame + envelope  | same frame; envelope of (decompressed offset, fingerprint stamp) per 128-row chunk; eq skips chunk deserialize, inflate stays charged                                                                                                 |
| framed 1024         | chunk of 1024 rows = own zstd frame; envelope of (frame len, fingerprint stamp); guards skip frames without inflating                                                                                                                 |
| framed 2048         | chunk of 2048 rows = own zstd frame; envelope of (frame len, fingerprint stamp); guards skip frames without inflating                                                                                                                 |
| framed 2048 + bloom | framed 2048; frames whose stamp crossed the 64-value cap append a fingerprint bloom (10 bits per distinct value, k=7, about 1% false positives); near-unique columns skip frames without inflating for about 2 envelope bytes per row |
| framed 128          | chunk of 128 rows = own zstd frame; envelope of (frame len, fingerprint stamp); guards skip frames without inflating                                                                                                                  |
| gated               | write-time rule: frame when the blob averages 32+ bytes per row or 90% of its probe windows fit the fingerprint guard; otherwise whole frame plus envelope                                                                            |


## openalex language (632276 rows)

632276 rows, 126 distinct over 3 blobs; common `en` x538257; rare `ang` x1

| Approach            | Size MB | Envelope KB | ser ms  | full read | eq common       | eq rare    | head 5% gather | tail 1% gather | head inflate MB | tail inflate MB |
|---------------------|---------|-------------|---------|-----------|-----------------|------------|----------------|----------------|-----------------|-----------------|
| current: 1 frame    | 0.2     | 0.0         | 5.5 ms  | 12.8 ms   | 14.2 ms x538257 | 10.8 ms x1 | 5.7 ms x31613  | 5.5 ms x6322   | 2               | 2               |
| 1 frame + envelope  | 0.2     | 342.9       | 10.1 ms | 12.1 ms   | 14.4 ms x538257 | 1.1 ms x1  | 1.9 ms x31613  | 1.4 ms x6322   | 2               | 2               |
| framed 1024         | 0.2     | 101.3       | 12.1 ms | 13.3 ms   | 15.2 ms x538257 | 0.0 ms x1  | 0.9 ms x31613  | 0.4 ms x6322   | 0               | 0               |
| framed 2048         | 0.2     | 63.7        | 10.3 ms | 12.7 ms   | 14.7 ms x538257 | 0.0 ms x1  | 0.9 ms x31613  | 0.4 ms x6322   | 0               | 0               |
| framed 2048 + bloom | 0.2     | 64.3        | 10.2 ms | 12.8 ms   | 14.9 ms x538257 | 0.0 ms x1  | 0.9 ms x31613  | 0.4 ms x6322   | 0               | 0               |
| framed 128          | 0.4     | 342.9       | 33.7 ms | 16.3 ms   | 17.0 ms x538257 | 0.1 ms x1  | 1.0 ms x31613  | 0.4 ms x6322   | 0               | 0               |
| gated               | 0.2     | 63.7        | 10.1 ms | 13.1 ms   | 14.6 ms x538257 | 0.0 ms x1  | 0.8 ms x31613  | 0.4 ms x6322   | 0               | 0               |


Gated chose per blob: framed 2048, framed 2048, framed 2048.

## openalex source (632276 rows)

632276 rows, 24773 distinct over 3 blobs; common `NIFS` x148562; rare `14th Aerospace Sciences Meet...` x1

| Approach            | Size MB | Envelope KB | ser ms  | full read | eq common       | eq rare    | head 5% gather | tail 1% gather | head inflate MB | tail inflate MB |
|---------------------|---------|-------------|---------|-----------|-----------------|------------|----------------|----------------|-----------------|-----------------|
| current: 1 frame    | 2.3     | 0.0         | 28.5 ms | 29.3 ms   | 26.9 ms x148562 | 26.7 ms x1 | 18.3 ms x31613 | 17.8 ms x6322  | 16              | 16              |
| 1 frame + envelope  | 2.3     | 488.1       | 38.2 ms | 28.4 ms   | 25.5 ms x148562 | 20.7 ms x1 | 8.6 ms x31613  | 7.8 ms x6322   | 16              | 16              |
| framed 1024         | 4.6     | 37.9        | 38.0 ms | 32.2 ms   | 31.3 ms x148562 | 28.0 ms x1 | 2.2 ms x31613  | 0.6 ms x6322   | 1               | 0               |
| framed 2048         | 4.1     | 4.9         | 34.0 ms | 30.5 ms   | 30.6 ms x148562 | 29.2 ms x1 | 2.1 ms x31613  | 0.7 ms x6322   | 1               | 0               |
| framed 2048 + bloom | 4.1     | 435.2       | 45.6 ms | 30.5 ms   | 8.3 ms x148562  | 0.1 ms x1  | 2.1 ms x31613  | 0.6 ms x6322   | 1               | 0               |
| framed 128          | 6.9     | 488.1       | 87.8 ms | 42.5 ms   | 37.6 ms x148562 | 27.7 ms x1 | 2.7 ms x31613  | 0.7 ms x6322   | 1               | 0               |
| gated               | 2.3     | 488.1       | 39.1 ms | 28.7 ms   | 25.0 ms x148562 | 20.5 ms x1 | 8.7 ms x31613  | 7.7 ms x6322   | 16              | 16              |


Gated chose per blob: whole + envelope, whole + envelope, whole + envelope.

## openalex title (632276 rows)

632276 rows, 624273 distinct over 3 blobs; common `Occurrence Download` x931; rare `" 8H#"**4<I<` x1

| Approach            | Size MB | Envelope KB | ser ms   | full read | eq common    | eq rare    | head 5% gather | tail 1% gather | head inflate MB | tail inflate MB |
|---------------------|---------|-------------|----------|-----------|--------------|------------|----------------|----------------|-----------------|-----------------|
| current: 1 frame    | 21.1    | 0.0         | 182.1 ms | 74.2 ms   | 70.6 ms x931 | 70.3 ms x1 | 60.0 ms x31613 | 61.0 ms x6322  | 53              | 53              |
| 1 frame + envelope  | 21.1    | 24.7        | 190.4 ms | 73.6 ms   | 70.4 ms x931 | 69.9 ms x1 | 43.3 ms x31613 | 42.7 ms x6322  | 53              | 53              |
| framed 1024         | 24.3    | 3.1         | 142.4 ms | 71.9 ms   | 69.4 ms x931 | 69.5 ms x1 | 4.4 ms x31613  | 1.1 ms x6322   | 4               | 1               |
| framed 2048         | 23.2    | 1.5         | 145.6 ms | 71.9 ms   | 69.4 ms x931 | 70.7 ms x1 | 4.5 ms x31613  | 1.2 ms x6322   | 4               | 1               |
| framed 2048 + bloom | 23.2    | 1265.8      | 164.7 ms | 70.5 ms   | 18.8 ms x931 | 0.4 ms x1  | 4.5 ms x31613  | 1.2 ms x6322   | 4               | 1               |
| framed 128          | 28.1    | 24.7        | 181.5 ms | 86.6 ms   | 81.0 ms x931 | 79.1 ms x1 | 5.2 ms x31613  | 1.2 ms x6322   | 3               | 1               |
| gated               | 23.2    | 1.5         | 143.6 ms | 70.4 ms   | 69.2 ms x931 | 69.4 ms x1 | 4.5 ms x31613  | 1.2 ms x6322   | 4               | 1               |


Gated chose per blob: framed 2048, framed 2048, framed 2048.

## query_explains output_json as text (237651 rows, the text scenario's column)

237651 rows, 237284 distinct over 1 blobs; common `[{"Plan":{"Actual Loops":1,"...` x10; rare `[{"Plan":{"Actual Loops":1,"...` x1

| Approach            | Size MB | Envelope KB | ser ms | full read | eq common    | eq rare     | head 5% gather  | tail 1% gather | head inflate MB | tail inflate MB |
|---------------------|---------|-------------|--------|-----------|--------------|-------------|-----------------|----------------|-----------------|-----------------|
| current: 1 frame    | 123.7   | 0.0         | 1.5 s  | 867.5 ms  | 737.0 ms x10 | 732.0 ms x1 | 714.2 ms x11882 | 703.9 ms x2376 | 2052            | 2052            |
| 1 frame + envelope  | 123.7   | 9.3         | 1.7 s  | 748.7 ms  | 738.3 ms x10 | 713.4 ms x1 | 588.1 ms x11882 | 587.2 ms x2376 | 2052            | 2052            |
| framed 1024         | 126.2   | 1.2         | 1.7 s  | 780.9 ms  | 760.6 ms x10 | 760.9 ms x1 | 44.7 ms x11882  | 10.4 ms x2376  | 118             | 29              |
| framed 2048         | 124.9   | 0.6         | 1.5 s  | 782.4 ms  | 776.0 ms x10 | 776.1 ms x1 | 47.6 ms x11882  | 14.8 ms x2376  | 118             | 39              |
| framed 2048 + bloom | 124.9   | 476.1       | 1.8 s  | 781.9 ms  | 67.1 ms x10  | 7.2 ms x1   | 43.9 ms x11882  | 14.3 ms x2376  | 118             | 39              |
| framed 128          | 143.9   | 9.3         | 1.4 s  | 660.5 ms  | 620.9 ms x10 | 620.1 ms x1 | 36.6 ms x11882  | 6.8 ms x2376   | 115             | 22              |
| gated               | 124.9   | 0.6         | 1.5 s  | 787.9 ms  | 772.9 ms x10 | 771.0 ms x1 | 44.8 ms x11882  | 14.6 ms x2376  | 118             | 39              |


Gated chose per blob: framed 2048.
