//! Text-column framing bake-off: what per-chunk metadata and independent
//! zstd frames buy on real text columns, measured end to end.
//!
//! Production text (`src/lazy_msgpack.rs`) writes msgpack chunks of 128
//! rows inside ONE zstd frame per column blob with no skip guard at all,
//! so every read inflates the whole column. The approaches below add
//! one mechanism at a time, so every gain has a name:
//!
//! - `current`: production bytes exactly: one frame, 128-row chunk walks
//!   skip msgpack deserialization but inflate anyway.
//! - `1 frame + envelope`: same payload; a side envelope carries each
//!   128-row chunk's decompressed offset and fingerprint set. Eq guards
//!   skip chunk deserialization; inflate stays charged. This is the ceiling
//!   of envelope-only metadata.
//! - `framed 128 / framed 1024 / framed 2048`: msgpack chunks of N rows
//!   become zstd frames; the envelope carries the frame byte lengths and
//!   fingerprint stamps, offsets by prefix-sum. Guards skip frames without
//!   inflating them, so on high-cardinality columns (64-fingerprint cap
//!   exceeded, stamp never skips) the eq cost is where framed is charged to
//!   the frames that actually match. Bigger frames compress better (longer
//!   match distances, fewer frame headers) and halve envelope bytes per step,
//!   but every guard touch inflates that much more.
//! - `gated`: the write-time rule this bake-off argues for: frame a blob when
//!   it averages `FAT_AVG_BYTES` per row (the fat-text regime, where
//!   independent frames cost about 2% of size) or when at least 90% of its
//!   probe windows fit the fingerprint guard (the low-cardinality regime, where
//!   framing is free and stamps actually skip). Everything else keeps the
//!   whole-frame envelope, accepting the positional-inflate cost rather than
//!   paying the measured size doubling of framing lean mid-cardinality text.
//!   Each table names the rule's per-blob pick below it.
//! - `framed 2048 + bloom`: the same frames; a frame whose fingerprint stamp
//!   overflowed the 64-value cap appends a Bloom filter of the frame's value
//!   fingerprints to its envelope record (10 bits per distinct value, k=7
//!   hashes, about 1% false positives). Near-unique columns (`title`, plan
//!   JSON) never skip by stamp alone, so they pay a guard inflating every
//!   frame; the bloom skips them for about 2 envelope bytes per row on
//!   near-unique columns, charging only the false-positive inflates to eq.
//!
//! Columns are real text columns of the production schemas, chosen by
//! probing the captures (backend_snapshots `application_name`/`client_addr`
//! ride empty there): openalex `language` (~80 distinct 3-byte codes),
//! `source` (display names, thousands distinct), `title` (near-unique),
//! and private `query_explains` `output_json` as the text scenario stores
//! it, the fat free-form plans behind the postgres 670 ms filter.
//! Scenarios: full read, eq of the most-common and rarest values, and
//! contiguous positional masks (the first 5% and the last 1% of rows) as a
//! numeric range filter would produce them. `head inflate MB` and
//! `tail inflate MB` name the decompressed bytes those two gathers pay with
//! the counter reset per scenario, the resource framing removes. Fingerprints are the
//! production SipHash-2-4 zero-key fold copied from `src/text_meta.rs`
//! (crate-private there); exact string compare still confirms every hit.
//! The run's output is captured verbatim at `benches/textdict.md`; refresh
//! it after changing the encodings.

// Shared bench modules ride along whole; this harness uses part of each.
#![allow(dead_code)]

include!("bench_common.rs");

#[path = "datasets/common.rs"]
pub mod ds_common;

mod datasets {
    pub use super::*;
    pub mod common;
    pub mod openalex;
}

mod postgres {
    pub mod common;
}

use markdown_tables::MarkdownTableRow;
use pco_pack::rmp_serde;
use pco_pack::zstd;
use postgres::common::{private_data_dir, read_blob};
use serde::Deserialize;
use serde::de::IgnoredAny;
use std::cell::Cell;
use std::collections::HashMap;

/// Production `text_meta` fingerprint cap per guard unit.
const FP_CAP: usize = 64;
/// Msgpack chunk rows of the production and whole-frame approaches.
const MSGPACK_CHUNK: usize = 128;
/// Frame rows for the framed approaches and the gate's guard probe.
const FRAME_ROWS: usize = 2048;
/// Avg bytes per row that marks the fat-text regime, where framed blobs
/// measured about 2% larger than whole-frame blobs.
const FAT_AVG_BYTES: usize = 32;
/// Rows sampled by the write-time gate.
const PROBE_ROWS: usize = 8192;
/// Bloom bits per distinct value for `framed 2048 + bloom`: k=7 hashes at
/// 10 bits per value target a 1% false-positive rate.
const BLOOM_BITS: usize = 10;
const BLOOM_HASHES: u64 = 7;

// ---- production fingerprint (copy of src/text_meta.rs) ----

const C0: u64 = 0x736f6d6570736575;
const C1: u64 = 0x646f72616e646f6d;
const C2: u64 = 0x6c79676e657261;
const C3: u64 = 0x7465646279746573;

#[inline(always)]
fn sipround(v: &mut [u64; 4], m: u64) {
    v[0] = v[0].wrapping_add(v[1]).wrapping_add(m);
    v[1] = v[1].rotate_left(13);
    v[1] ^= v[0];
    v[0] = v[0].rotate_left(32);
    v[2] = v[2].wrapping_add(v[3]);
    v[3] = v[3].rotate_left(16);
    v[3] ^= v[2];
    v[0] = v[0].wrapping_add(v[3]);
    v[3] = v[3].rotate_left(21);
    v[3] ^= v[0];
    v[2] = v[2].wrapping_add(v[1]);
    v[1] = v[1].rotate_left(17);
    v[1] ^= v[2];
    v[2] = v[2].rotate_left(32);
}

fn fingerprint(bytes: &[u8]) -> u64 {
    let mut v = [C0, C1, C2, C3];
    let mut chunks = bytes.chunks_exact(8);
    for chunk in &mut chunks {
        sipround(&mut v, u64::from_le_bytes(chunk.try_into().unwrap()));
    }
    let rem = chunks.remainder();
    let mut tail = [0u8; 8];
    tail[..rem.len()].copy_from_slice(rem);
    tail[7] = (bytes.len() & 0xFF) as u8;
    sipround(&mut v, u64::from_le_bytes(tail));
    v[2] ^= 0xFF;
    sipround(&mut v, 0);
    sipround(&mut v, 0);
    v[0] ^ v[1] ^ v[2] ^ v[3]
}

/// Fold a guard unit's distinct values: `[count u8][fp u64 LE]*`; count 0
/// means the unit crossed the cap and its guard never skips.
fn fold_unit(unit: &[String]) -> Vec<u8> {
    let mut fps: Vec<u64> = Vec::new();
    let mut seen: ahash::HashSet<u64> = ahash::HashSet::default();
    for value in unit {
        let fp = fingerprint(value.as_bytes());
        if !seen.insert(fp) {
            continue;
        }
        if fps.len() == FP_CAP {
            fps.clear();
            break;
        }
        fps.push(fp);
    }
    let mut out = Vec::with_capacity(1 + 8 * fps.len());
    out.push(fps.len() as u8);
    for fp in fps {
        out.extend_from_slice(&fp.to_le_bytes());
    }
    out
}

/// Does this stamp prove the target absent? Count 0 (cap exceeded) never
/// skips.
fn excludes(stamp: &[u8], target: u64) -> bool {
    let count = stamp[0] as usize;
    count != 0 && !stamp[1..].chunks_exact(8).map(|c| u64::from_le_bytes(c.try_into().unwrap())).any(|f| f == target)
}

/// Fingerprint bloom for a guard unit whose stamp overflowed the cap:
/// `BLOOM_BITS` bits per distinct value with `BLOOM_HASHES` double-hashed
/// probes (about 1% false positives). A stamp that still fits guards exactly,
/// so those units carry no bloom.
fn build_bloom(unit: &[String]) -> Vec<u8> {
    let mut fps: Vec<u64> = Vec::new();
    let mut seen: ahash::HashSet<u64> = ahash::HashSet::default();
    for value in unit {
        let fp = fingerprint(value.as_bytes());
        if seen.insert(fp) {
            fps.push(fp);
        }
    }
    let bits = (fps.len() * BLOOM_BITS).next_power_of_two().max(1024);
    let mut bloom = vec![0u8; bits / 8];
    for &fp in &fps {
        let (h1, h2) = (fp, (fp >> 32) | 1);
        for k in 0..BLOOM_HASHES {
            let bit = h1.wrapping_add(k.wrapping_mul(h2)) & (bits as u64 - 1);
            bloom[bit as usize >> 3] |= 1 << (bit & 7);
        }
    }
    bloom
}

/// Could the target's fingerprint live in this bloom? A false `true` costs
/// one inflate plus the exact compare; empty (stamp-guarded) units always
/// answer `true`.
fn bloom_may(bloom: &[u8], target: u64) -> bool {
    if bloom.is_empty() {
        return true;
    }
    let bits = (bloom.len() * 8) as u64;
    let (h1, h2) = (target, (target >> 32) | 1);
    (0..BLOOM_HASHES).all(|k| {
        let bit = h1.wrapping_add(k.wrapping_mul(h2)) & (bits - 1);
        bloom[bit as usize >> 3] & (1 << (bit & 7)) != 0
    })
}

/// Parse the whole-frame envelope: `[offset u32 LE][stamp]` per chunk.
fn chunk_units(env: &[u8]) -> Vec<(usize, &[u8])> {
    let mut out = Vec::new();
    let mut rest = env;
    while !rest.is_empty() {
        let offset = u32::from_le_bytes(rest[..4].try_into().unwrap()) as usize;
        rest = &rest[4..];
        let end = 1 + 8 * rest[0] as usize;
        out.push((offset, &rest[..end]));
        rest = &rest[end..];
    }
    out
}

/// Parse the framed envelope: `[frame_len u32 LE][stamp]*`; `bloom` blobs
/// append `[bloom_len u16 LE][bloom]` after each stamp. The bloom tail is
/// only read by the bloom rung; every other framed reader steps over it.
fn frame_units(env: &[u8], bloom: bool) -> Vec<(usize, &[u8], &[u8])> {
    let mut out = Vec::new();
    let mut rest = env;
    while !rest.is_empty() {
        let len = u32::from_le_bytes(rest[..4].try_into().unwrap()) as usize;
        rest = &rest[4..];
        let stamp_end = 1 + 8 * rest[0] as usize;
        let stamp = &rest[..stamp_end];
        rest = &rest[stamp_end..];
        let blm = if bloom {
            let blen = u16::from_le_bytes(rest[..2].try_into().unwrap()) as usize;
            rest = &rest[2..];
            let b = &rest[..blen];
            rest = &rest[blen..];
            b
        } else {
            &[]
        };
        out.push((len, stamp, blm));
    }
    out
}

// ---- row mirrors of the msgpack captures (positional arrays) ----

#[derive(Deserialize)]
struct QeRow {
    #[allow(dead_code)]
    server_id: i64,
    #[allow(dead_code)]
    database_id: i64,
    #[allow(dead_code)]
    postgres_role_id: i64,
    #[allow(dead_code)]
    query_fingerprint: i64,
    #[allow(dead_code)]
    format: i32,
    #[allow(dead_code)]
    source: i32,
    #[allow(dead_code)]
    error: String,
    #[allow(dead_code)]
    seen_at: pco_pack::chrono::DateTime<pco_pack::chrono::Utc>,
    #[allow(dead_code)]
    runtime_ms: f32,
    #[allow(dead_code)]
    total_cost: Option<f32>,
    #[allow(dead_code)]
    total_shared_blks_hit: Option<i64>,
    #[allow(dead_code)]
    total_shared_blks_read: Option<i64>,
    #[allow(dead_code)]
    total_shared_blks_written: Option<i64>,
    #[allow(dead_code)]
    total_shared_blks_dirtied: Option<i64>,
    #[allow(dead_code)]
    total_blk_read_time: Option<f32>,
    #[allow(dead_code)]
    total_blk_write_time: Option<f32>,
    #[allow(dead_code)]
    plan_node_types: Vec<pco_pack::smol_str::SmolStr>,
    #[allow(dead_code)]
    plan_fingerprint: Option<i64>,
    output_text: Option<String>,
    #[allow(dead_code)]
    output_json: Option<serde_json::Value>,
    #[allow(dead_code)]
    node_insights: Option<serde_json::Value>,
}

// ---- masks ----

fn set_bit(mask: &mut [u64], row: usize) {
    mask[row >> 6] |= 1 << (row & 63);
}

fn bit_count(mask: &[u64]) -> usize {
    mask.iter().map(|w| w.count_ones() as usize).sum()
}

fn hits(mask: &[u64], start: usize, len: usize) -> bool {
    let end = (start + len).min(mask.len() * 64);
    let mut word = start >> 6;
    while word * 64 < end {
        let lo = start.max(word * 64) - word * 64;
        let hi = end.min(word * 64 + 64) - word * 64;
        let bits = if hi - lo == 64 { u64::MAX } else { (1u64 << hi) - (1u64 << lo) };
        if mask[word] & bits != 0 {
            return true;
        }
        word += 1;
    }
    false
}

/// Blob-local view of a global mask: bit `r` is global row `base + r`.
fn local_mask(global: &[u64], base: usize, len: usize) -> Vec<u64> {
    let mut m = vec![0u64; len.div_ceil(64)];
    for r in 0..len {
        if (global[(base + r) >> 6] >> ((base + r) & 63)) & 1 == 1 {
            set_bit(&mut m, r);
        }
    }
    m
}

// ---- msgpack framing primitives (mirrors of src/lazy_msgpack.rs) ----

fn chunk_bytes(chunk: &[String]) -> Vec<u8> {
    let mut buf = Vec::new();
    rmp_serde::encode::write(&mut buf, chunk).unwrap();
    buf
}

fn read_chunk(bytes: &[u8], pos: &mut usize) -> Vec<String> {
    let mut de = rmp_serde::Deserializer::new(Cursor::new(&bytes[*pos..]));
    let chunk: Vec<String> = Vec::deserialize(&mut de).unwrap();
    *pos += de.into_inner().position() as usize;
    chunk
}

/// Advance past one msgpack array without allocating, the production
/// `skip_chunk` cost: header bytes still get walked.
fn skip_chunk(bytes: &[u8], pos: &mut usize) {
    let mut de = rmp_serde::Deserializer::new(Cursor::new(&bytes[*pos..]));
    Vec::<IgnoredAny>::deserialize(&mut de).unwrap();
    *pos += de.into_inner().position() as usize;
}

fn inflate(bytes: &[u8], inflated_mb: &Cell<f64>) -> Vec<u8> {
    let out = zstd::decode_all(Cursor::new(bytes)).unwrap();
    inflated_mb.set(inflated_mb.get() + out.len() as f64 / 1e6);
    out
}

fn take_survivors(chunk: &mut [String], base: usize, mask: &[u64], out: &mut Vec<String>) {
    for (r, slot) in chunk.iter_mut().enumerate() {
        if (mask[(base + r) >> 6] >> ((base + r) & 63)) & 1 == 1 {
            out.push(std::mem::take(slot));
        }
    }
}

fn push_equal(chunk: &mut [String], target: &str, out: &mut Vec<String>) {
    for slot in chunk.iter_mut() {
        if slot == target {
            out.push(std::mem::take(slot));
        }
    }
}

// ---- approaches ----

enum Approach {
    /// Production lazy-msgpack: one zstd frame, 128-row chunks, no envelope.
    Current,
    /// Same payload, side envelope of chunk offsets + fingerprint stamps.
    WholePlusEnvelope,
    /// Each chunk of `N` rows is its own zstd frame; the envelope holds
    /// frame byte lengths + per-frame fingerprint stamps, offsets by
    /// prefix-sum. Guards skip frames without inflating them.
    Framed(usize),
    /// Framed at `FRAME_ROWS` with a fingerprint bloom appended to the
    /// envelope record of every frame whose stamp overflowed the cap:
    /// near-unique columns skip frames the stamp can never rule out.
    FramedPlusBloom,
    /// Write-time gate: frame fat blobs and guard-fitting blobs, keep
    /// everything else whole-frame plus envelope (`gate_framed`).
    Gated,
}

impl Approach {
    fn key(&self) -> String {
        match self {
            Approach::Current => "current: 1 frame".into(),
            Approach::WholePlusEnvelope => "1 frame + envelope".into(),
            Approach::Framed(n) => format!("framed {n}"),
            Approach::FramedPlusBloom => "framed 2048 + bloom".into(),
            Approach::Gated => "gated".into(),
        }
    }

    fn framing(&self) -> String {
        match self {
            Approach::Current => {
                "one zstd frame over msgpack chunks of 128 rows, production bytes; every read inflates the whole blob".into()
            }
            Approach::WholePlusEnvelope => {
                "same frame; envelope of (decompressed offset, fingerprint stamp) per 128-row chunk; eq skips chunk deserialize, inflate stays charged".into()
            }
            Approach::Framed(n) => format!(
                "chunk of {n} rows = own zstd frame; envelope of (frame len, fingerprint stamp); guards skip frames without inflating"
            ),
            Approach::FramedPlusBloom => {
                "framed 2048; frames whose stamp crossed the 64-value cap append a fingerprint bloom (10 bits per distinct value, k=7, about 1% false positives); near-unique columns skip frames without inflating for about 2 envelope bytes per row".into()
            }
            Approach::Gated => {
                "write-time rule: frame when the blob averages 32+ bytes per row or 90% of its probe windows fit the fingerprint guard; otherwise whole frame plus envelope".into()
            }
        }
    }

    fn encode(&self, rows: &[String]) -> Blob {
        match self {
            Approach::Current => Self::encode_whole(rows, false),
            Approach::WholePlusEnvelope => Self::encode_whole(rows, true),
            Approach::Framed(n) => Self::encode_framed(rows, *n),
            Approach::FramedPlusBloom => Self::encode_framed_bloom(rows, FRAME_ROWS),
            Approach::Gated => {
                if gate_framed(rows) {
                    Self::encode_framed(rows, FRAME_ROWS)
                } else {
                    Self::encode_whole(rows, true)
                }
            }
        }
    }

    fn encode_whole(rows: &[String], stamps: bool) -> Blob {
        let mut raw = Vec::with_capacity(1 + rows.len() * 32);
        raw.push(1u8); // format_version = chunked
        let mut envelope = Vec::new();
        let mut pos = 1usize;
        for chunk in rows.chunks(MSGPACK_CHUNK) {
            let bytes = chunk_bytes(chunk);
            if stamps {
                envelope.extend_from_slice(&(pos as u32).to_le_bytes());
                envelope.extend_from_slice(&fold_unit(chunk));
                pos += bytes.len();
            }
            raw.extend_from_slice(&bytes);
        }
        let compressed = zstd::encode_all(raw.as_slice(), 3).unwrap();
        let mut payload = Vec::with_capacity(16 + compressed.len());
        payload.extend_from_slice(&(rows.len() as u64).to_le_bytes());
        payload.extend_from_slice(&(compressed.len() as u64).to_le_bytes());
        payload.extend_from_slice(&compressed);
        Blob { payload, envelope, framed: false }
    }

    fn encode_framed(rows: &[String], chunk: usize) -> Blob {
        let mut payload = Vec::new();
        let mut envelope = Vec::new();
        for part in rows.chunks(chunk) {
            let frame = zstd::encode_all(chunk_bytes(part).as_slice(), 3).unwrap();
            envelope.extend_from_slice(&(frame.len() as u32).to_le_bytes());
            envelope.extend_from_slice(&fold_unit(part));
            payload.extend_from_slice(&frame);
        }
        Blob { payload, envelope, framed: true }
    }

    /// Framed with the bloom tail: stamp-first, bloom only where the stamp
    /// overflowed the cap (the frames whose guard otherwise never skips).
    fn encode_framed_bloom(rows: &[String], chunk: usize) -> Blob {
        let mut payload = Vec::new();
        let mut envelope = Vec::new();
        for part in rows.chunks(chunk) {
            let frame = zstd::encode_all(chunk_bytes(part).as_slice(), 3).unwrap();
            let stamp = fold_unit(part);
            let bloom = if stamp[0] != 0 { Vec::new() } else { build_bloom(part) };
            envelope.extend_from_slice(&(frame.len() as u32).to_le_bytes());
            envelope.extend_from_slice(&stamp);
            envelope.extend_from_slice(&(bloom.len() as u16).to_le_bytes());
            envelope.extend_from_slice(&bloom);
            payload.extend_from_slice(&frame);
        }
        Blob { payload, envelope, framed: true }
    }

    /// Read every row, inflating whatever the framing forces.
    fn read_all(&self, blob: &Blob, inflated_mb: &Cell<f64>) -> Vec<String> {
        let mut out = Vec::new();
        if blob.framed {
            let bloom = matches!(self, Approach::FramedPlusBloom);
            let mut off = 0usize;
            for (len, _, _) in frame_units(&blob.envelope, bloom) {
                out.append(&mut read_chunk(&inflate(&blob.payload[off..off + len], inflated_mb), &mut 0));
                off += len;
            }
        } else {
            let clen = u64::from_le_bytes(blob.payload[8..16].try_into().unwrap()) as usize;
            let bytes = inflate(&blob.payload[16..16 + clen], inflated_mb);
            let mut pos = 1usize;
            while pos < bytes.len() {
                out.append(&mut read_chunk(&bytes, &mut pos));
            }
        }
        out
    }

    /// Equality filter: guard every unit, decode only units that can hold
    /// the target, exact-compare survivors.
    fn eq_filter(&self, blob: &Blob, target: &str, inflated_mb: &Cell<f64>) -> Vec<String> {
        let tfp = fingerprint(target.as_bytes());
        let mut out = Vec::new();
        if matches!(self, Approach::Current) {
            let clen = u64::from_le_bytes(blob.payload[8..16].try_into().unwrap()) as usize;
            let bytes = inflate(&blob.payload[16..16 + clen], inflated_mb);
            let mut pos = 1usize;
            while pos < bytes.len() {
                push_equal(&mut read_chunk(&bytes, &mut pos), target, &mut out);
            }
        } else if blob.framed {
            let bloom = matches!(self, Approach::FramedPlusBloom);
            let mut off = 0usize;
            for (len, stamp, blm) in frame_units(&blob.envelope, bloom) {
                if excludes(stamp, tfp) || !bloom_may(blm, tfp) {
                    off += len;
                    continue;
                }
                push_equal(
                    &mut read_chunk(&inflate(&blob.payload[off..off + len], inflated_mb), &mut 0),
                    target,
                    &mut out,
                );
                off += len;
            }
        } else {
            let clen = u64::from_le_bytes(blob.payload[8..16].try_into().unwrap()) as usize;
            let bytes = inflate(&blob.payload[16..16 + clen], inflated_mb);
            for (offset, stamp) in chunk_units(&blob.envelope) {
                if excludes(stamp, tfp) {
                    continue;
                }
                let mut pos = offset;
                push_equal(&mut read_chunk(&bytes, &mut pos), target, &mut out);
            }
        }
        out
    }

    /// Gather survivors at the blob-local mask. Untouched chunks are walked
    /// past (`current`), jumped over via the envelope (`envelope`), or never
    /// inflated at all (`framed`).
    fn gather(&self, blob: &Blob, mask: &[u64], inflated_mb: &Cell<f64>) -> Vec<String> {
        let mut out = Vec::new();
        if blob.framed {
            let n = match self {
                Approach::Framed(n) => *n,
                _ => FRAME_ROWS,
            };
            let bloom = matches!(self, Approach::FramedPlusBloom);
            let mut off = 0usize;
            let mut base = 0usize;
            for (len, _, _) in frame_units(&blob.envelope, bloom) {
                let rows_here = (mask.len() * 64 - base).min(n);
                if rows_here == 0 || !hits(mask, base, rows_here) {
                    off += len;
                    base += n;
                    continue;
                }
                take_survivors(
                    &mut read_chunk(&inflate(&blob.payload[off..off + len], inflated_mb), &mut 0),
                    base,
                    mask,
                    &mut out,
                );
                off += len;
                base += n;
            }
        } else if matches!(self, Approach::Current) {
            let clen = u64::from_le_bytes(blob.payload[8..16].try_into().unwrap()) as usize;
            let bytes = inflate(&blob.payload[16..16 + clen], inflated_mb);
            let mut pos = 1usize;
            let mut start = 0usize;
            while pos < bytes.len() {
                if hits(mask, start, MSGPACK_CHUNK) {
                    take_survivors(&mut read_chunk(&bytes, &mut pos), start, mask, &mut out);
                } else {
                    skip_chunk(&bytes, &mut pos);
                }
                start += MSGPACK_CHUNK;
            }
        } else {
            let clen = u64::from_le_bytes(blob.payload[8..16].try_into().unwrap()) as usize;
            let bytes = inflate(&blob.payload[16..16 + clen], inflated_mb);
            for (ci, (offset, _)) in chunk_units(&blob.envelope).into_iter().enumerate() {
                if !hits(mask, ci * MSGPACK_CHUNK, MSGPACK_CHUNK) {
                    continue;
                }
                let mut pos = offset;
                take_survivors(&mut read_chunk(&bytes, &mut pos), ci * MSGPACK_CHUNK, mask, &mut out);
            }
        }
        out
    }
}

/// Write-time rule behind `gated`: frame a blob when the probed windows
/// average `FAT_AVG_BYTES` per row (the fat-text regime, where independent
/// frames cost about 2% of size) or when at least 90% of them fit the
/// fingerprint guard (the low-cardinality regime, where framing is free and
/// stamps actually skip). Lean mid-cardinality text stays whole-frame:
/// framing it doubled the measured column's size while its stamps could
/// never skip. Probe windows are `FRAME_ROWS` windows spread evenly across
/// the blob, so the gate only touches `PROBE_ROWS` rows.
fn gate_framed(blob: &[String]) -> bool {
    let windows = blob.len() / FRAME_ROWS;
    if windows == 0 {
        return false;
    }
    let step = (windows / (PROBE_ROWS / FRAME_ROWS)).max(1);
    let mut probed_rows = 0usize;
    let mut bytes = 0usize;
    let mut probed = 0usize;
    let mut fitting = 0usize;
    for k in (0..windows).step_by(step) {
        let window = &blob[k * FRAME_ROWS..(k + 1) * FRAME_ROWS];
        probed += 1;
        probed_rows += window.len();
        bytes += window.iter().map(|s| s.len() + 1).sum::<usize>();
        if fold_unit(window)[0] != 0 {
            fitting += 1;
        }
    }
    if bytes >= FAT_AVG_BYTES * probed_rows {
        return true;
    }
    fitting * 10 >= probed * 9
}

struct Blob {
    payload: Vec<u8>,
    envelope: Vec<u8>,
    /// Framed payload: concatenated self-contained zstd frames located by
    /// envelope lengths. False = whole-frame msgpack with chunk offsets in
    /// the envelope (or no envelope at all for `current`).
    framed: bool,
}

// ---- tables ----

struct ApproachRow {
    approach: String,
    size_mb: String,
    envelope_kb: String,
    ser_ms: String,
    full_read: String,
    eq_common: String,
    eq_rare: String,
    head_gather: String,
    tail_gather: String,
    head_inflate_mb: String,
    tail_inflate_mb: String,
}

impl MarkdownTableRow for ApproachRow {
    fn column_names() -> Vec<&'static str> {
        vec![
            "Approach",
            "Size MB",
            "Envelope KB",
            "ser ms",
            "full read",
            "eq common",
            "eq rare",
            "head 5% gather",
            "tail 1% gather",
            "head inflate MB",
            "tail inflate MB",
        ]
    }

    fn column_values(&self) -> Vec<String> {
        vec![
            self.approach.clone(),
            self.size_mb.clone(),
            self.envelope_kb.clone(),
            self.ser_ms.clone(),
            self.full_read.clone(),
            self.eq_common.clone(),
            self.eq_rare.clone(),
            self.head_gather.clone(),
            self.tail_gather.clone(),
            self.head_inflate_mb.clone(),
            self.tail_inflate_mb.clone(),
        ]
    }
}

struct FramingRow {
    approach: String,
    framing: String,
}

impl MarkdownTableRow for FramingRow {
    fn column_names() -> Vec<&'static str> {
        vec!["Approach", "Framing"]
    }

    fn column_values(&self) -> Vec<String> {
        vec![self.approach.clone(), self.framing.clone()]
    }
}

fn scenario(label: &str, data: Vec<String>) {
    let n = data.len();
    let mut hist: HashMap<&String, usize> = HashMap::new();
    for v in &data {
        *hist.entry(v).or_insert(0) += 1;
    }
    let mut by_count: Vec<(&String, usize)> = hist.iter().map(|(v, c)| (*v, *c)).collect();
    by_count.sort_by_key(|(v, c)| (*c, v.as_str()));
    let (rare, rare_n) = *by_count.first().unwrap();
    let (common, common_n) = *by_count.last().unwrap();

    let mut head = vec![0u64; n.div_ceil(64)];
    let mut tail = vec![0u64; n.div_ceil(64)];
    for r in 0..n / 20 {
        set_bit(&mut head, r);
    }
    for r in n - n / 100..n {
        set_bit(&mut tail, r);
    }
    let head_n = bit_count(&head);
    let tail_n = bit_count(&tail);
    let blobs: Vec<&[String]> = data.chunks(CHUNK_ROWS).collect();

    println!("\n## {label}");
    println!();
    println!(
        "{n} rows, {} distinct over {} blobs; common `{}` x{common_n}; rare `{}` x{rare_n}",
        hist.len(),
        blobs.len(),
        if common.chars().count() > 28 {
            format!("{}...", common.chars().take(28).collect::<String>())
        } else {
            common.clone()
        },
        if rare.chars().count() > 28 {
            format!("{}...", rare.chars().take(28).collect::<String>())
        } else {
            rare.clone()
        },
    );

    let approaches: Vec<Approach> = vec![
        Approach::Current,
        Approach::WholePlusEnvelope,
        Approach::Framed(1024),
        Approach::Framed(2048),
        Approach::FramedPlusBloom,
        Approach::Framed(128),
        Approach::Gated,
    ];

    let mut rows: Vec<ApproachRow> = Vec::new();
    let mut gate_picks: Vec<String> = Vec::new();
    for approach in &approaches {
        let t0 = Instant::now();
        let enc: Vec<Blob> = blobs.iter().map(|b| approach.encode(b)).collect();
        if matches!(approach, Approach::Gated) {
            gate_picks.extend(
                enc.iter().map(|b| if b.framed { format!("framed {FRAME_ROWS}") } else { "whole + envelope".into() }),
            );
        }
        let ser_ms = t0.elapsed().as_secs_f64() * 1000.0;
        let size: usize = enc.iter().map(|b| b.payload.len()).sum();
        let env: usize = enc.iter().map(|b| b.envelope.len()).sum();

        let inflated = Cell::new(0f64);
        let t0 = Instant::now();
        let mut total = 0usize;
        for b in &enc {
            total += approach.read_all(b, &inflated).len();
        }
        let full_ms = format_ms(t0.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(total, n, "{label} {}/full read", approach.key());

        let t0 = Instant::now();
        let mut total = 0usize;
        for b in &enc {
            total += approach.eq_filter(b, common, &inflated).len();
        }
        let eq_common = format!("{} x{total}", format_ms(t0.elapsed().as_secs_f64() * 1000.0));
        assert_eq!(total, common_n, "{label} {}/eq common", approach.key());

        let t0 = Instant::now();
        let mut total = 0usize;
        for b in &enc {
            total += approach.eq_filter(b, rare, &inflated).len();
        }
        let eq_rare = format!("{} x{total}", format_ms(t0.elapsed().as_secs_f64() * 1000.0));
        assert_eq!(total, rare_n, "{label} {}/eq rare", approach.key());

        inflated.set(0.0);
        let t0 = Instant::now();
        let mut total = 0usize;
        let mut base = 0usize;
        for (b, blob) in enc.iter().zip(&blobs) {
            total += approach.gather(b, &local_mask(&head, base, blob.len()), &inflated).len();
            base += blob.len();
        }
        let head_gather = format!("{} x{total}", format_ms(t0.elapsed().as_secs_f64() * 1000.0));
        assert_eq!(total, head_n, "{label} {}/head gather", approach.key());
        let head_inflated = inflated.get();

        inflated.set(0.0);
        let t0 = Instant::now();
        let mut total = 0usize;
        let mut base = 0usize;
        for (b, blob) in enc.iter().zip(&blobs) {
            total += approach.gather(b, &local_mask(&tail, base, blob.len()), &inflated).len();
            base += blob.len();
        }
        let tail_gather = format!("{} x{total}", format_ms(t0.elapsed().as_secs_f64() * 1000.0));
        assert_eq!(total, tail_n, "{label} {}/tail gather", approach.key());
        let tail_inflated = inflated.get();

        rows.push(ApproachRow {
            approach: approach.key(),
            size_mb: format!("{:.1}", size as f64 / 1e6),
            envelope_kb: format!("{:.1}", env as f64 / 1e3),
            ser_ms: format_ms(ser_ms),
            full_read: full_ms,
            eq_common,
            eq_rare,
            head_gather,
            tail_gather,
            head_inflate_mb: format!("{:.0}", head_inflated),
            tail_inflate_mb: format!("{:.0}", tail_inflated),
        });
    }
    println!();
    println!("{}", markdown_tables::as_table(&rows));
    if !gate_picks.is_empty() {
        println!();
        println!("Gated chose per blob: {}.", gate_picks.join(", "));
    }
}

fn main() -> anyhow::Result<()> {
    println!("# Text-column framing and filter bake-off");
    println!();
    println!(
        "Production text (`src/lazy_msgpack.rs`) packs msgpack chunks of \
         128 rows into one zstd frame per column blob, so every selective \
         read inflates the whole column. This run encodes real text from the \
         pganalyze captures whole-frame and framed (each frame its own zstd \
         frame behind a fingerprint-stamp envelope), adding one mechanism at \
         a time:"
    );
    println!();
    println!("- `Eq`: the guard path. Metadata check, decode only matching units, exact compare.");
    println!(
        "- `gather`: positional survivor masks over the first 5% and last 1% of rows. `head inflate MB` \
         and `tail inflate MB` count the decompressed bytes those gathers pay, which framing removes."
    );
    println!(
        "- `gated`: the proposed write-time rule. Frame fat or guard-fitting blobs, keep the rest \
         whole-frame; each table names the per-blob pick."
    );
    println!(
        "- `framed 2048 + bloom`: a fingerprint bloom behind stamps that overflow the 64-value cap, \
         letting near-unique columns skip frames too."
    );
    println!();

    let framing: Vec<FramingRow> = vec![
        Approach::Current,
        Approach::WholePlusEnvelope,
        Approach::Framed(1024),
        Approach::Framed(2048),
        Approach::FramedPlusBloom,
        Approach::Framed(128),
        Approach::Gated,
    ]
    .iter()
    .map(|a| FramingRow { approach: a.key(), framing: a.framing() })
    .collect();
    println!("Framing keys:");
    println!();
    println!("{}", markdown_tables::as_table(&framing));

    let works = datasets::openalex::load();
    scenario(
        &format!("openalex language ({} rows)", works.len()),
        works.iter().map(|w| w.language.as_deref().unwrap_or("").to_string()).collect(),
    );
    scenario(
        &format!("openalex source ({} rows)", works.len()),
        works.iter().map(|w| w.source.clone().unwrap_or_default()).collect(),
    );
    scenario(
        &format!("openalex title ({} rows)", works.len()),
        works.into_iter().map(|w| w.title.unwrap_or_default()).collect(),
    );
    if let Some(rows) = read_blob::<QeRow>(
        "query_explains.msgpack.zst",
        &private_data_dir(),
        "query_explains missing: clone pco_pack_bench_data beside this checkout and run `git lfs pull` there",
    )? {
        let n = rows.len();
        scenario(
            &format!("query_explains output_json as text ({n} rows, the text scenario's column)"),
            rows.into_iter().map(|r| r.output_json.map(|v| v.to_string()).unwrap_or_default()).collect(),
        );
    }
    Ok(())
}
