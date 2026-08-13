# Contributing

## Running tests

When making changes to the pco_pack crate:

`cargo test --workspace`

When making changes to pco_pack_derive, and debugging compile errors in the proc macro:

`cargo test -p pco_pack_derive && cargo test --test codegen`

## Architecture overview

### Core traits

| Trait | Purpose |
|-------|--------|
| `PcoSerde<T>` | Columnar write/read for a single type. Defines `Writer` and `Reader` associated types.
| `PcoFilter<T>` | Filtering on columnar data. Resolves JSON queries to typed filters, evaluates them against readers.
| `PcoPack<T>` | High-level API: chunking, grouping by index fields, timestamp sorting, serialize/deserialize/filter.

`VecPackable<T>` extends this for `Vec<T>`, enabling nested columnar packing.

### Write path

```mermaid
flowchart TD
    A["Input: Vec<MyStruct>"] --> B["Group by index fields"]
    B --> C["Sort each group by timestamp field"]
    C --> D["Split into chunks (~32K rows)"]
    D --> E["Transpose: one column vector per field"]
    E --> F["Compress each column with type-specific codec"]
    F --> G["Serialize Chunk structs (msgpack)"]
    G --> H["Output: Vec<u8>"]
```

Filtering deserializes chunks, skips those that don't match index/timestamp bounds, decompresses filter fields first to build a `FilterMask` bitmask, then only decompresses requested fields for matching rows.

### float_round flow

Float rounding is applied before compression to reduce precision noise:

```mermaid
flowchart LR
    A["raw f64: 23.999999"] --> B["round to N decimals"]
    B --> C["rounded: 24.0"]
    C --> D["Pcodec compress"]
```

Set via `#[pco_pack(float_round = N)]`. Applied recursively to nested collections but must be explicitly set on nested structs/enums.

### Schema evolution flow

When a numeric field's type changes between write and read versions, coercion happens at read time:

```mermaid
flowchart LR
    A["Written as i32: 50000"] --> B{"Read as different type?"}
    B -- "i64" --> C["Widen: 50000"]
    B -- "u16" --> D["Clamp to u16::MAX: 65535"]
    B -- "f64" --> E["Convert: 50000.0"]
```

Implemented via the `CoercibleNumber` trait in `number.rs`. Values are clamped to the new type's bounds rather than failing.
