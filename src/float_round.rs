//! Float rounding: encode `f64` values as compressed `i64` by rounding to
//! configurable decimal places, improving compression ratios.
//!
//! Apply `#[pco_pack(float_round = N)]` on a struct. All float fields (including
//! nested `Vec<T>`, tuples, `HashMap`/`BTreeMap` values, `Option<T>`,
//! and `serde_json::Value`) are rounded to N decimal places during serialization.
//! Floats handle rounding transparently via their own `PcoSerde` impls internally.

/// Maximum float_round decimal precision. Enforced at compile time by the derive macro
/// (see `pco_pack_derive/src/lib.rs`). Values above this would risk overflow when scaling
/// floats for i64 storage.
pub const MAX_FLOAT_ROUND_PRECISION: u32 = 8;

/// Magic byte prefix indicating float_round format: next byte is decimals, followed by compressed i64 column data.
pub(crate) const FLOAT_ROUND_MAGIC: u8 = 0xFE;

/// Divide a scaled `i64` by `10^decimals` to restore the original float.
#[inline]
pub fn unround_float(raw: i64, decimals: u32) -> f64 {
    let scale = 10i64.pow(decimals) as f64;
    (raw as f64) / scale
}
