//! Temporary diagnostic: attribute the PcoPack-vs-Vortex size gap in benches/struct.rs per column.
//! Writes each field of Record individually through BOTH systems with identical data,
//! then prints per-column sizes so we can see which columns drive the difference.

use arrow_schema::FieldRef;
use pco_pack::PcoPack;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_arrow::schema::{SchemaLike, TracingOptions};
use serde_bytes::ByteBuf;
use vortex::array::stream::ArrayStreamExt as _;
use vortex::arrow::ArrowSessionExt as _;
use vortex::compressor::BtrBlocksCompressorBuilder;
use vortex::file::{OpenOptionsSessionExt, WriteOptionsSessionExt, WriteStrategyBuilder};
use vortex::session::VortexSession;

// ---- Same Record/Color/data() as benches/struct.rs (verbatim copies) ----

#[derive(Clone, PartialEq, PcoPack, serde::Serialize, serde::Deserialize)]
#[pco_pack(index = [account_id])]
struct Record {
    account_id: i64,
    name: String,
    score: f64,
    active: bool,
    tag: Option<i32>,
    tags: Vec<i32>,
    color: Color,
    payload: ByteBuf,
}

#[derive(Clone, Copy, PartialEq, Default, PcoPack)]
#[repr(u8)]
enum Color {
    #[default]
    Red = 0,
    Green = 1,
    Blue = 2,
    Yellow = 3,
}

impl Serialize for Color {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u8(*self as u8)
    }
}
impl<'de> Deserialize<'de> for Color {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        match u8::deserialize(deserializer)? {
            0 => Ok(Color::Red),
            1 => Ok(Color::Green),
            2 => Ok(Color::Blue),
            3 => Ok(Color::Yellow),
            _ => Err(serde::de::Error::custom("Invalid variant integer")),
        }
    }
}

fn noise_i32(i: usize) -> i32 {
    let h = (i as u32).wrapping_mul(0x01000193).wrapping_add(i as u32);
    (h ^ (h >> 16)) as i32 & 0xF
}

fn data(n: usize) -> Vec<Record> {
    (0..n)
        .map(|i| {
            let n = noise_i32(i);
            Record {
                account_id: i as i64 % 5,
                name: format!("name_{}_{}", i % 5000, n & 0xFF),
                score: (i as f64) / 100.0 + (n as f64) / 100.0,
                active: (i + n as usize) % 3 != 0,
                tag: if i % 2 == 0 { Some(((i % 100) as i32) + n) } else { None },
                tags: (0..((i % 5) + 1)).map(|j| (i * 100 + j) as i32 + n).collect(),
                color: match (i + n as usize) % 4 {
                    0 => Color::Red,
                    1 => Color::Green,
                    2 => Color::Blue,
                    _ => Color::Yellow,
                },
                payload: ByteBuf::from(vec![n as u8; 64]),
            }
        })
        .collect()
}

// ---- Per-field PcoPack wrappers (no index attributes -> pure column sizes) ----

#[derive(Clone, PcoPack, serde::Serialize, serde::Deserialize)]
struct F_account_id {
    account_id: i64,
}
#[derive(Clone, PcoPack, serde::Serialize, serde::Deserialize)]
struct F_name {
    name: String,
}
#[derive(Clone, PcoPack, serde::Serialize, serde::Deserialize)]
struct F_score {
    score: f64,
}
#[derive(Clone, PcoPack, serde::Serialize, serde::Deserialize)]
struct F_active {
    active: bool,
}
#[derive(Clone, PcoPack, serde::Serialize, serde::Deserialize)]
struct F_tag {
    tag: Option<i32>,
}
#[derive(Clone, PcoPack, serde::Serialize, serde::Deserialize)]
struct F_tags {
    tags: Vec<i32>,
}
#[derive(Clone, PcoPack, serde::Serialize, serde::Deserialize)]
struct F_color {
    color: Color,
}
#[derive(Clone, PcoPack, serde::Serialize, serde::Deserialize)]
struct F_payload {
    payload: ByteBuf,
}

// From impls so we can build per-field rows from Record.
impl From<&Record> for F_account_id {
    fn from(r: &Record) -> Self {
        Self { account_id: r.account_id }
    }
}
impl From<&Record> for F_name {
    fn from(r: &Record) -> Self {
        Self { name: r.name.clone() }
    }
}
impl From<&Record> for F_score {
    fn from(r: &Record) -> Self {
        Self { score: r.score }
    }
}
impl From<&Record> for F_active {
    fn from(r: &Record) -> Self {
        Self { active: r.active }
    }
}
impl From<&Record> for F_tag {
    fn from(r: &Record) -> Self {
        Self { tag: r.tag }
    }
}
impl From<&Record> for F_tags {
    fn from(r: &Record) -> Self {
        Self { tags: r.tags.clone() }
    }
}
impl From<&Record> for F_color {
    fn from(r: &Record) -> Self {
        Self { color: r.color }
    }
}
impl From<&Record> for F_payload {
    fn from(r: &Record) -> Self {
        Self { payload: ByteBuf::from(r.payload.as_slice().to_vec()) }
    }
}

// ---- Per-field vortex writer (same path as benches/struct.rs write_vortex_file) ----

async fn make_session() -> VortexSession {
    use vortex::VortexSessionDefault as _;
    // Must be constructed inside the Tokio context so vortex-io finds its runtime.
    VortexSession::default()
}

async fn vortex_write<T>(session: &VortexSession, rows: &[T]) -> Vec<u8>
where
    T: Serialize,
    for<'de> T: serde::Deserialize<'de>,
{
    let fields = <Vec<FieldRef>>::from_type::<T>(TracingOptions::default()).unwrap();
    let batch = serde_arrow::to_record_batch(&fields, &rows).unwrap();
    let schema_in = batch.schema();
    let array = session.arrow().from_arrow_record_batch(batch, &schema_in).unwrap();

    // Same compact schemes as the struct bench: PCO for numerics, zstd for strings/binary.
    let mut bytes: Vec<u8> = vec![];
    session
        .write_options()
        .with_strategy(
            WriteStrategyBuilder::default()
                .with_btrblocks_builder(BtrBlocksCompressorBuilder::default().with_compact())
                .build(),
        )
        .write(&mut bytes, array.to_array_stream())
        .await
        .unwrap();
    bytes
}

fn fmt_kb(b: usize) -> String {
    format!("{:.2} KB", b as f64 / 1024.0)
}

#[test]
fn attribute_sizes() {
    let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
    let data = data(500_000);
    let session = rt.block_on(make_session());

    // Full-shape sanity check vs the struct bench numbers (PcoPack 1.4 MB, Vortex 1.6 MB).
    let pco_full = Record::to_bytes(&Record::write(data.clone()).unwrap()).unwrap().len();
    let vtx_full = rt.block_on(async { vortex_write::<Record>(&session, &data).await }).len();
    println!("\n## Full shape: PcoPack {} vs Vortex {}\n", fmt_kb(pco_full), fmt_kb(vtx_full));

    println!("| {:<20} | {:>14} | {:>14} | {:>8} |", "column", "PcoPack", "Vortex", "ratio");
    let mut pco_total = 0usize;
    let mut vtx_total = 0usize;

    macro_rules! measure_col {
        ($t:ty, $label:expr) => {{
            let rows: Vec<$t> = data.iter().map(|r| <$t>::from(r)).collect();
            let pco_bytes = <$t as PcoPack>::to_bytes(&<$t as PcoPack>::write(rows.clone()).unwrap()).unwrap().len();
            let vtx_bytes = rt.block_on(async { vortex_write::<$t>(&session, &rows).await }).len();
            println!(
                "| {:<20} | {:>14} | {:>14} | {:>8.2}x |",
                $label,
                fmt_kb(pco_bytes),
                fmt_kb(vtx_bytes),
                vtx_bytes as f64 / pco_bytes.max(1) as f64
            );
            (pco_bytes, vtx_bytes)
        }};
    }

    let (a, b) = measure_col!(F_account_id, "account_id i64");
    pco_total += a;
    vtx_total += b;
    let (a, b) = measure_col!(F_name, "name String");
    pco_total += a;
    vtx_total += b;
    let (a, b) = measure_col!(F_score, "score f64");
    pco_total += a;
    vtx_total += b;
    let (a, b) = measure_col!(F_active, "active bool");
    pco_total += a;
    vtx_total += b;
    let (a, b) = measure_col!(F_tag, "tag Option<i32>");
    pco_total += a;
    vtx_total += b;
    let (a, b) = measure_col!(F_tags, "tags Vec<i32>");
    pco_total += a;
    vtx_total += b;
    let (a, b) = measure_col!(F_color, "color u8-enum");
    pco_total += a;
    vtx_total += b;
    let (a, b) = measure_col!(F_payload, "payload [u8;64]");
    pco_total += a;
    vtx_total += b;

    println!(
        "\nPer-column sums: PcoPack {} vs Vortex {} (separate files each pay container overhead)",
        fmt_kb(pco_total),
        fmt_kb(vtx_total)
    );
}
