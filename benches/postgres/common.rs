//! Shared postgres plumbing: capture readers and codec-table printers; no
//! `bench()` of its own. The codec runners come from the datasets bench
//! (`crate::ds_common`) and ride along here so source modules reach every
//! shared piece through one `super::common` path.

use crate::{format_bytes, format_ms};
use pco_pack::{rmp_serde, zstd};
use std::io::BufRead as _;
use std::path::{Path, PathBuf};

pub use crate::ds_common::{
    CodecMetrics, ScStore, bench_columnar, bench_msgpack, bench_pco_pack, bench_serde_columnar, bench_vortex_ds,
    filter_label, round_f32, ts_rounded,
};

// ---- Captures: neutral msgpack+zstd inputs ----

/// The zstd frame magic; a capture whose first bytes are anything else is
/// an un-fetched Git LFS pointer.
const ZSTD_MAGIC: [u8; 4] = [0x28, 0xB5, 0x2F, 0xFD];

/// Public captures live in `benches/data/` (Git LFS); override with
/// `$PCO_PACK_BENCH_DATA` to point at a local re-capture.
pub fn data_dir() -> PathBuf {
    match std::env::var("PCO_PACK_BENCH_DATA") {
        Ok(p) if !p.is_empty() => PathBuf::from(p),
        _ => Path::new(env!("CARGO_MANIFEST_DIR")).join("benches/data"),
    }
}

/// The explains capture lives in the private `pco_pack_bench_data` repo,
/// cloned beside this checkout: explain plans can contain sensitive
/// information, so the blob never rides in this repo's history.
pub fn private_data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../pco_pack_bench_data")
}

/// Each capture is one msgpack array of rows behind a single zstd frame
/// (`tmp/pg_capture`'s dump format; `query_stats` trimmed to its last 14
/// days). Stream-decoded to avoid the multi-GB uncompressed intermediate. A
/// capture that is absent or still an un-fetched Git LFS pointer prints
/// `warning` and yields `None`, so a missing input skips its section
/// instead of failing the whole run.
pub fn read_blob<T: serde::de::DeserializeOwned>(
    name: &str, dir: &Path, warning: &str,
) -> anyhow::Result<Option<Vec<T>>> {
    let path = dir.join(name);
    let Ok(file) = std::fs::File::open(&path) else {
        eprintln!("warning: {warning}");
        return Ok(None);
    };
    let mut buffered = std::io::BufReader::new(file);
    if buffered.fill_buf()?.get(..4) != Some(&ZSTD_MAGIC) {
        eprintln!("warning: {warning}");
        return Ok(None);
    }
    eprintln!("decoding {name}");
    let decoder = zstd::stream::read::Decoder::new(buffered)?;
    Ok(Some(rmp_serde::decode::from_read(decoder)?))
}

/// The hint every public capture shares when it is absent or still an
/// un-fetched Git LFS pointer.
pub fn lfs_warning(name: &str) -> String {
    format!("{name} missing: run `git lfs pull`")
}

// ---- Table printers ----

#[derive(Debug)]
pub struct CodecRow {
    pub metric: String,
    pub pco_pack: String,
    pub pco_time_round: String,
    pub vortex: String,
    pub columnar: String,
    pub serde_columnar: String,
    pub msgpack: String,
}

impl markdown_tables::MarkdownTableRow for CodecRow {
    fn column_names() -> Vec<&'static str> {
        vec!["Metric", "PcoPack", "PcoPack time_round", "Vortex", "columnar", "serde_columnar", "msgpack"]
    }
    fn column_values(&self) -> Vec<String> {
        vec![
            self.metric.clone(),
            self.pco_pack.clone(),
            self.pco_time_round.clone(),
            self.vortex.clone(),
            self.columnar.clone(),
            self.serde_columnar.clone(),
            self.msgpack.clone(),
        ]
    }
}

/// query_explains: the five PcoPack scenarios in one table. string keeps the
/// JSON payload as a plain string (the way Vortex stores it); JSON keeps the
/// `serde_json::Value` tree; arena replaces `output_json` with the flattened
/// plan-tree arena of `postgres/arena.rs`; the lossy variants add
/// `float_round = 2` (plus `time_round = 1 s` on the rows).
#[derive(Debug)]
pub struct CodecRowPcoPack {
    pub metric: String,
    pub text: String,
    pub json: String,
    pub json_lossy: String,
    pub arena: String,
    pub arena_lossy: String,
}

impl markdown_tables::MarkdownTableRow for CodecRowPcoPack {
    fn column_names() -> Vec<&'static str> {
        vec!["Metric", "text", "JSON", "JSON lossy", "arena", "arena lossy"]
    }
    fn column_values(&self) -> Vec<String> {
        vec![
            self.metric.clone(),
            self.text.clone(),
            self.json.clone(),
            self.json_lossy.clone(),
            self.arena.clone(),
            self.arena_lossy.clone(),
        ]
    }
}

/// query_explains: the non-PcoPack crates in their own table.
#[derive(Debug)]
pub struct CodecRowOther {
    pub metric: String,
    pub vortex: String,
    pub columnar: String,
    pub serde_columnar: String,
    pub msgpack: String,
}

impl markdown_tables::MarkdownTableRow for CodecRowOther {
    fn column_names() -> Vec<&'static str> {
        vec!["Metric", "Vortex", "columnar", "serde_columnar", "msgpack"]
    }
    fn column_values(&self) -> Vec<String> {
        vec![
            self.metric.clone(),
            self.vortex.clone(),
            self.columnar.clone(),
            self.serde_columnar.clone(),
            self.msgpack.clone(),
        ]
    }
}

/// backend_snapshots: the codec table plus the `PcoPack Timeline` dedup column.
#[derive(Debug)]
pub struct CodecRowTimeline {
    pub metric: String,
    pub pco_pack: String,
    pub pco_time_round: String,
    pub pco_timeline: String,
    pub vortex: String,
    pub columnar: String,
    pub serde_columnar: String,
    pub msgpack: String,
}

impl markdown_tables::MarkdownTableRow for CodecRowTimeline {
    fn column_names() -> Vec<&'static str> {
        vec![
            "Metric",
            "PcoPack",
            "PcoPack time_round",
            "PcoPack Timeline",
            "Vortex",
            "columnar",
            "serde_columnar",
            "msgpack",
        ]
    }
    fn column_values(&self) -> Vec<String> {
        vec![
            self.metric.clone(),
            self.pco_pack.clone(),
            self.pco_time_round.clone(),
            self.pco_timeline.clone(),
            self.vortex.clone(),
            self.columnar.clone(),
            self.serde_columnar.clone(),
            self.msgpack.clone(),
        ]
    }
}

pub fn cell(m: &CodecMetrics, kind: &str, filter_idx: Option<usize>) -> String {
    let value = match kind {
        "Serialize" => m.serial_ms.map(format_ms),
        "Deserialize" => m.deserial_ms.map(format_ms),
        "Size" => m.size.map(format_bytes),
        _ => match filter_idx {
            Some(i) => m.filter_ms.get(i).copied().flatten().map(format_ms),
            None => None,
        },
    };
    value.unwrap_or_else(|| "-".to_string())
}
