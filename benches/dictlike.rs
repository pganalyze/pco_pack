//! Dictionary-like serialization and filter performance: measured storage
//! formats for real low-cardinality integer columns scraped from the
//! existing benchmarks.
//!
//! Columns are the actual low-cardinality integer columns of the openalex,
//! taxi, and backend_snapshots datasets, cut into 262,144-row blobs, the
//! production chunk envelope. Rows form a metadata ladder, each adding one
//! mechanism to the row above it: no metadata (upstream), a min/max bounds
//! prefix (`src/bounds.rs`), a bounds + dictionary tail; and within each
//! rung the payload is coded different ways. The framing key printed in the
//! output explains every row. The run's output is captured verbatim at
//! `benches/dictlike.md`; refresh it after changing the encodings.
//!
//! Filters per column: eq most-common value, eq rarest value, IN of the two
//! least-common values, and an absent value. Filter time = guard cost on
//! every blob plus payload decode on blobs the guard cannot exclude.

include!("bench_common.rs");

// The datasets/postgres modules carry scenario runners this bake-off never
// calls; their helpers would lint dead inside the bake-off crate alone.
#[path = "datasets/common.rs"]
#[allow(dead_code)]
pub mod ds_common;

#[allow(dead_code)]
mod datasets {
    pub use super::*;
    pub mod common;
    pub mod openalex;
    pub mod taxi;
}

#[allow(dead_code)]
mod postgres {
    pub mod backends;
    pub mod common;
}

use pco::metadata::{DynLatents, Mode};
use pco::standalone::{DecompressorItem, FileDecompressor, simple_compress};
use pco::{ChunkConfig, DeltaSpec, ModeSpec};
use std::collections::HashMap;

/// Mirrors the bounds prefix documented in `src/bounds.rs`:
/// `[BOUNDS_MAGIC][kind][exact][min i64 LE][max i64 LE][decimals][n u64 LE]`.
const BOUNDS_MAGIC: u8 = 0xFD;
const BOUNDS_KIND_I64: u8 = 3;
const BOUNDS_PREFIX_LEN: usize = 28;
/// Default `Chunk` envelope rows; production writes one blob per envelope.
const BLOB_N: usize = 262_144;

fn bounds_prefix(min: i64, max: i64, n: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(BOUNDS_PREFIX_LEN);
    out.extend_from_slice(&[BOUNDS_MAGIC, BOUNDS_KIND_I64, 1]);
    out.extend_from_slice(&min.to_le_bytes());
    out.extend_from_slice(&max.to_le_bytes());
    out.push(0);
    out.extend_from_slice(&(n as u64).to_le_bytes());
    out
}

fn parse_bounds(buf: &[u8]) -> Option<(i64, i64, usize)> {
    if buf.len() < BOUNDS_PREFIX_LEN || buf[0] != BOUNDS_MAGIC || buf[1] != BOUNDS_KIND_I64 {
        return None;
    }
    let min = i64::from_le_bytes(buf[3..11].try_into().unwrap());
    let max = i64::from_le_bytes(buf[11..19].try_into().unwrap());
    let n = u64::from_le_bytes(buf[20..28].try_into().unwrap()) as usize;
    Some((min, max, n))
}

fn auto_cfg() -> ChunkConfig {
    ChunkConfig::default().with_enable_8_bit(true)
}

fn trydict_cfg() -> ChunkConfig {
    auto_cfg().with_mode_spec(ModeSpec::TryDict).with_delta_spec(DeltaSpec::NoOp)
}

fn put_len(buf: &mut Vec<u8>, part: &[u8]) {
    buf.extend_from_slice(&(part.len() as u32).to_le_bytes());
    buf.extend_from_slice(part);
}

fn take_len(buf: &[u8]) -> (&[u8], &[u8]) {
    let n = u32::from_le_bytes(buf[..4].try_into().unwrap()) as usize;
    (&buf[4..4 + n], &buf[4 + n..])
}

fn put_varint(buf: &mut Vec<u8>, v: i64) {
    let mut x = ((v << 1) ^ (v >> 63)) as u64;
    loop {
        let b = (x as u8) & 0x7f;
        x >>= 7;
        if x == 0 {
            buf.push(b);
            return;
        }
        buf.push(b | 0x80);
    }
}

fn get_varint(buf: &[u8], pos: &mut usize) -> i64 {
    let mut x: u64 = 0;
    let mut shift = 0;
    loop {
        let b = buf[*pos];
        *pos += 1;
        x |= ((b & 0x7f) as u64) << shift;
        if b & 0x80 == 0 {
            return ((x >> 1) as i64) ^ -((x & 1) as i64);
        }
        shift += 7;
    }
}

fn zstd(bytes: &[u8]) -> Vec<u8> {
    pco_pack::zstd::encode_all(bytes, 3).unwrap()
}

fn unzip(bytes: &[u8]) -> Vec<u8> {
    pco_pack::zstd::decode_all(bytes).unwrap()
}

/// Exact dictionary: distinct values ascending (rank = index).
fn dict_of(chunk: &[i64]) -> Vec<i64> {
    let mut u: Vec<i64> = chunk.iter().copied().collect();
    u.sort_unstable();
    u.dedup();
    u
}

fn ranks_of(chunk: &[i64], dict: &[i64]) -> Vec<u32> {
    let rank: HashMap<i64, u32> = dict.iter().copied().enumerate().map(|(i, v)| (v, i as u32)).collect();
    chunk.iter().map(|v| rank[v]).collect()
}

/// Exact dictionary from pco's uncompressed chunk header, no body decode. Dispatches
/// on the stream's actual number type (enable_8_bit lets pco narrow it).
fn dict_header(payload: &[u8]) -> Option<Vec<i64>> {
    let (fd, mut src) = FileDecompressor::new(payload).ok()?;
    let ty = fd.peek_number_type_or_termination(&mut src).ok()??;
    macro_rules! dict_of_ty {
        ($t:ty) => {{
            match fd.chunk_decompressor::<$t, &[u8]>(src).ok()? {
                DecompressorItem::Chunk(cd) => match &cd.meta().mode {
                    Mode::Dict(d) => Some(match d {
                        DynLatents::U8(v) => v.iter().map(|&x| x as i64).collect(),
                        DynLatents::U16(v) => v.iter().map(|&x| x as i64).collect(),
                        DynLatents::U32(v) => v.iter().map(|&x| x as i64).collect(),
                        DynLatents::U64(v) => v.iter().map(|&x| x as i64).collect(),
                        _ => return None,
                    }),
                    _ => None,
                },
                DecompressorItem::EndOfData(_) => None,
            }
        }};
    }
    match ty {
        pco::data_types::NumberType::U8 => dict_of_ty!(u8),
        pco::data_types::NumberType::U16 => dict_of_ty!(u16),
        pco::data_types::NumberType::U32 => dict_of_ty!(u32),
        pco::data_types::NumberType::U64 => dict_of_ty!(u64),
        _ => None,
    }
}

const DICTIONARY_MAGIC: u8 = b'D';
/// Mirrors production's payload tags (`src/bounds.rs`): 0 = pco of values,
/// 1 = zstd over raw u8 indexes. Tags >= 2 describe payloads production
/// emits nowhere and whose readers reject, so those rows can never ship
/// as written; they exist to keep the rejected alternatives measurable.
const TAG_PCO_VALUES: u8 = 0;
const TAG_ZSTD_U8_INDEX: u8 = 1;
const TAG_MP_VALUES: u8 = 2;
const TAG_PCO_RANKS: u8 = 3;
const TAG_MP_RANKS: u8 = 4;
const TAG_RLE: u8 = 5;
const TAG_ZSTD_U16: u8 = 6;

/// Exact dictionary framing shared by every `bounds+dictionary+<codec>` kind, byte-identical
/// to production's tail (`push_dictionary` in `src/bounds.rs`):
/// `[b'D'][u32 k][zigzag varint * k ascending][tag u8][u32 payload len]`.
fn put_dictionary(blob: &mut Vec<u8>, dict: &[i64], tag: u8, payload_len: usize) {
    blob.push(DICTIONARY_MAGIC);
    blob.extend_from_slice(&(dict.len() as u32).to_le_bytes());
    for v in dict {
        put_varint(blob, *v);
    }
    blob.push(tag);
    blob.extend_from_slice(&(payload_len as u32).to_le_bytes());
}

/// Parses the dictionary at the head of `tail`; returns the dictionary, the
/// payload tag, and the payload byte offset (relative to `tail`). Mirrors
/// production `strip_dictionary`, including skipping the tag and its u32 length.
fn parse_dictionary(tail: &[u8]) -> (Vec<i64>, u8, usize) {
    assert_eq!(tail[0], DICTIONARY_MAGIC);
    let k = u32::from_le_bytes(tail[1..5].try_into().unwrap()) as usize;
    let mut pos = 5usize;
    let mut out = Vec::with_capacity(k);
    for _ in 0..k {
        out.push(get_varint(tail, &mut pos));
    }
    let tag = tail[pos];
    (out, tag, pos + 5)
}

/// Run-length pairs of `ranks` as `[varint index][varint run] * runs`.
fn put_rle(blob: &mut Vec<u8>, ranks: &[u32]) {
    let mut prev = ranks[0];
    let mut run = 1i64;
    for &r in &ranks[1..] {
        if r == prev {
            run += 1;
        } else {
            put_varint(blob, prev as i64);
            put_varint(blob, run);
            prev = r;
            run = 1;
        }
    }
    put_varint(blob, prev as i64);
    put_varint(blob, run);
}

/// Want-ranks: dictionary positions whose value matches one of `targets`
/// (ascending, since the dictionary is ascending).
fn want_ranks(dictionary: &[i64], targets: &[i64]) -> Vec<u32> {
    let mut ts = targets.to_vec();
    ts.sort_unstable();
    ts.dedup();
    (0..dictionary.len() as u32).filter(|i| ts.binary_search(&dictionary[*i as usize]).is_ok()).collect()
}

/// Decompresses a pco payload into i64 values, dispatching on the stream's
/// actual number type (enable_8_bit lets pco narrow it).
fn pco_vals_i64(payload: &[u8]) -> Vec<i64> {
    let (fd, mut src) = FileDecompressor::new(payload).unwrap();
    let ty = fd.peek_number_type_or_termination(&mut src).unwrap().unwrap();
    let mut out = Vec::new();
    macro_rules! vals_of_ty {
        ($t:ty) => {{
            while let DecompressorItem::Chunk(mut cd) = fd.chunk_decompressor::<$t, &[u8]>(src).unwrap() {
                let mut buf = vec![0 as $t; cd.n()];
                cd.read(&mut buf).unwrap();
                out.extend(buf.iter().map(|&x| x as i64));
                src = cd.into_src();
            }
        }};
    }
    match ty {
        pco::data_types::NumberType::I8 => vals_of_ty!(i8),
        pco::data_types::NumberType::I16 => vals_of_ty!(i16),
        pco::data_types::NumberType::I32 => vals_of_ty!(i32),
        pco::data_types::NumberType::I64 => vals_of_ty!(i64),
        pco::data_types::NumberType::U8 => vals_of_ty!(u8),
        pco::data_types::NumberType::U16 => vals_of_ty!(u16),
        pco::data_types::NumberType::U32 => vals_of_ty!(u32),
        pco::data_types::NumberType::U64 => vals_of_ty!(u64),
        _ => unreachable!("non-integer stream {ty:?}"),
    }
    out
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    PcoNoBounds,
    BoundsPco,
    BoundsDictPco,
    BoundsDictZstdVals,
    BoundsDictPcoRanks,
    BoundsDictZstdRanks,
    BoundsDictRle,
    BoundsDictZstdCodes,
    BoundsDictZstdIdx16,
    PcoUniqIdx,
    TryDict,
    MpUniqIdx,
    MpValues,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::PcoNoBounds => "upstream: pco, no bounds",
            Kind::BoundsPco => "bounds, pco",
            Kind::BoundsDictPco => "bounds+dictionary, pco values",
            Kind::BoundsDictZstdVals => "bounds+dictionary, msgpack+zstd values",
            Kind::BoundsDictPcoRanks => "bounds+dictionary, pco indexes",
            Kind::BoundsDictZstdRanks => "bounds+dictionary, msgpack+zstd indexes",
            Kind::BoundsDictRle => "bounds+dictionary, rle indexes",
            Kind::BoundsDictZstdCodes => "bounds+dictionary, zstd u8 indexes",
            Kind::BoundsDictZstdIdx16 => "bounds+dictionary, zstd u16 indexes",
            Kind::PcoUniqIdx => "bounds, pco dict + pco indexes",
            Kind::TryDict => "bounds, pco dict (TryDict)",
            Kind::MpUniqIdx => "bounds, mp+zstd dict + mp+zstd indexes",
            Kind::MpValues => "bounds, msgpack+zstd values",
        }
    }

    /// What each approach adds on top of the bounds prefix and what its payload
    /// is; printed as the framing key so rows in the tables are decidable.
    fn framing(self) -> &'static str {
        match self {
            Kind::PcoNoBounds => "no metadata at all; production upstream today, every filter decodes every blob",
            Kind::BoundsPco => {
                "uncompressed bounds prefix with min, max, and row count; skips only queries provably outside [min, max]"
            }
            Kind::BoundsDictPco => "bounds + sorted dictionary; pco payload of values",
            Kind::BoundsDictZstdVals => "bounds + dictionary; payload is zstd(msgpack(values))",
            Kind::BoundsDictPcoRanks => "bounds + dictionary; payload is pco of indexes",
            Kind::BoundsDictZstdRanks => "bounds + dictionary; payload is zstd(msgpack(indexes))",
            Kind::BoundsDictRle => "bounds + dictionary; payload is run-length coded (index, run) varint pairs",
            Kind::BoundsDictZstdCodes => {
                "bounds + dictionary; payload is zstd over one index byte per row; needs distinct count <= 256"
            }
            Kind::BoundsDictZstdIdx16 => {
                "bounds + dictionary; payload is zstd over two little-endian index bytes per row; covers distinct counts past the 256 gate up to 65536"
            }
            Kind::PcoUniqIdx => {
                "bounds; length-prefixed pco(dict) then pco(indexes); the dictionary lives in its own dict stream the reader must decompress"
            }
            Kind::TryDict => {
                "bounds; payload is the column cast to u64 encoded by pco in forced TryDict mode; the dictionary lives inside the pco chunk header"
            }
            Kind::MpUniqIdx => "bounds; length-prefixed zstd(msgpack(dict)) then zstd(msgpack(indexes))",
            Kind::MpValues => "bounds only, no dictionary; payload is zstd(msgpack(values))",
        }
    }

    fn dictionary_len(self) -> Option<usize> {
        match self {
            Kind::BoundsDictPco
            | Kind::BoundsDictZstdVals
            | Kind::BoundsDictPcoRanks
            | Kind::BoundsDictZstdRanks
            | Kind::BoundsDictRle
            | Kind::BoundsDictZstdCodes
            | Kind::BoundsDictZstdIdx16 => Some(BOUNDS_PREFIX_LEN),
            _ => None,
        }
    }

    /// Payload offset for the hand-framed kinds.
    fn payload_off(self, blob: &[u8]) -> usize {
        match self.dictionary_len() {
            Some(off) => off + parse_dictionary(&blob[off..]).2,
            None => BOUNDS_PREFIX_LEN,
        }
    }

    fn encode(self, chunk: &[i64]) -> Vec<u8> {
        if self == Kind::PcoNoBounds {
            return simple_compress(chunk, &auto_cfg()).unwrap();
        }
        let min = *chunk.iter().min().unwrap();
        let max = *chunk.iter().max().unwrap();
        if self == Kind::BoundsPco {
            let mut blob = bounds_prefix(min, max, chunk.len());
            blob.extend_from_slice(&simple_compress(chunk, &auto_cfg()).unwrap());
            return blob;
        }
        let n = chunk.len();
        let mut blob = bounds_prefix(min, max, n);
        match self {
            Kind::PcoNoBounds | Kind::BoundsPco => unreachable!(),
            Kind::BoundsDictPco => {
                let pay = simple_compress(chunk, &auto_cfg()).unwrap();
                put_dictionary(&mut blob, &dict_of(chunk), TAG_PCO_VALUES, pay.len());
                blob.extend_from_slice(&pay);
            }
            Kind::BoundsDictZstdVals => {
                let pay = zstd(&pco_pack::rmp_serde::to_vec(chunk).unwrap());
                put_dictionary(&mut blob, &dict_of(chunk), TAG_MP_VALUES, pay.len());
                blob.extend_from_slice(&pay);
            }
            Kind::BoundsDictPcoRanks => {
                let dict = dict_of(chunk);
                let pay = simple_compress(&ranks_of(chunk, &dict), &auto_cfg()).unwrap();
                put_dictionary(&mut blob, &dict, TAG_PCO_RANKS, pay.len());
                blob.extend_from_slice(&pay);
            }
            Kind::BoundsDictZstdRanks => {
                let dict = dict_of(chunk);
                let pay = zstd(&pco_pack::rmp_serde::to_vec(&ranks_of(chunk, &dict)).unwrap());
                put_dictionary(&mut blob, &dict, TAG_MP_RANKS, pay.len());
                blob.extend_from_slice(&pay);
            }
            Kind::BoundsDictRle => {
                let mut pay = Vec::new();
                put_rle(&mut pay, &ranks_of(chunk, &dict_of(chunk)));
                put_dictionary(&mut blob, &dict_of(chunk), TAG_RLE, pay.len());
                blob.extend_from_slice(&pay);
            }
            Kind::BoundsDictZstdCodes => {
                let dict = dict_of(chunk);
                assert!(dict.len() <= 256, "u8 indexes need d <= 256");
                let idx: Vec<u8> = ranks_of(chunk, &dict).into_iter().map(|r| r as u8).collect();
                let pay = zstd(&idx);
                put_dictionary(&mut blob, &dict, TAG_ZSTD_U8_INDEX, pay.len());
                blob.extend_from_slice(&pay);
            }
            Kind::BoundsDictZstdIdx16 => {
                let dict = dict_of(chunk);
                assert!(dict.len() <= 65_536, "u16 indexes need d <= 65536");
                let idx: Vec<u8> = ranks_of(chunk, &dict).into_iter().flat_map(|r| (r as u16).to_le_bytes()).collect();
                let pay = zstd(&idx);
                put_dictionary(&mut blob, &dict, TAG_ZSTD_U16, pay.len());
                blob.extend_from_slice(&pay);
            }
            Kind::TryDict => {
                // pco implements dict mode only for unsigned types; forcing it
                // on i64 silently falls back to Classic, so cast first.
                assert!(chunk.iter().all(|&v| v >= 0));
                let vals: Vec<u64> = chunk.iter().map(|&v| v as u64).collect();
                blob.extend_from_slice(&simple_compress(&vals, &trydict_cfg()).unwrap());
            }
            Kind::PcoUniqIdx => {
                let dict = dict_of(chunk);
                let idx = ranks_of(chunk, &dict);
                put_len(&mut blob, &simple_compress(&dict, &auto_cfg()).unwrap());
                put_len(&mut blob, &simple_compress(&idx, &auto_cfg()).unwrap());
            }
            Kind::MpUniqIdx => {
                let dict = dict_of(chunk);
                let idx = ranks_of(chunk, &dict);
                put_len(&mut blob, &zstd(&pco_pack::rmp_serde::to_vec(&dict).unwrap()));
                put_len(&mut blob, &zstd(&pco_pack::rmp_serde::to_vec(&idx).unwrap()));
            }
            Kind::MpValues => {
                put_len(&mut blob, &zstd(&pco_pack::rmp_serde::to_vec(chunk).unwrap()));
            }
        }
        blob
    }

    /// Proves no row in the blob matches `targets`, or says nothing. Never
    /// touches the value payload.
    fn guard_excludes(self, blob: &[u8], targets: &[i64]) -> bool {
        match self {
            Kind::PcoNoBounds => false, // upstream blob carries no metadata to consult
            Kind::MpValues => {
                // No dictionary tail, and the length-prefixed payload could alias
                // the dictionary magic by accident; the bounds prefix is all it offers.
                let (min, max, _) = parse_bounds(blob).unwrap();
                targets.iter().all(|t| *t < min || *t > max)
            }
            Kind::PcoUniqIdx => {
                let (uniq, _) = take_len(&blob[BOUNDS_PREFIX_LEN..]);
                let d = pco_vals_i64(uniq);
                targets.iter().all(|t| !d.contains(t))
            }
            Kind::TryDict => match dict_header(&blob[BOUNDS_PREFIX_LEN..]) {
                // Only a proven-absent dictionary excludes; an absent header
                // read (mode fell back to Classic) must fall through to decode.
                Some(d) if !d.is_empty() => targets.iter().all(|t| !d.contains(t)),
                _ => false,
            },
            Kind::MpUniqIdx => {
                let (frame, _) = take_len(&blob[BOUNDS_PREFIX_LEN..]);
                let d: Vec<i64> = pco_pack::rmp_serde::from_slice(&unzip(frame)).unwrap();
                targets.iter().all(|t| !d.contains(t))
            }
            // The guard the stack proposes, mirrored locally: trusted bounds
            // prove absence outside [min, max]; a fitted dictionary tail proves
            // absence by varint membership; inside both, the blob must decode.
            _ => {
                let (min, max, _) = parse_bounds(blob).unwrap();
                if targets.iter().all(|t| *t < min || *t > max) {
                    return true;
                }
                match self.dictionary_len() {
                    Some(off) => {
                        let (dict, _, _) = parse_dictionary(&blob[off..]);
                        targets.iter().all(|t| !dict.contains(t))
                    }
                    None => false,
                }
            }
        }
    }

    /// Decodes the payload and counts rows matching `targets`. Only called
    /// on blobs the guard could not exclude.
    fn decode_hits(self, blob: &[u8], targets: &[i64]) -> usize {
        match self {
            Kind::PcoNoBounds => pco_vals_i64(blob).into_iter().filter(|v| targets.contains(v)).count(),
            Kind::BoundsPco | Kind::BoundsDictPco => {
                pco_vals_i64(&blob[self.payload_off(blob)..]).into_iter().filter(|v| targets.contains(v)).count()
            }
            Kind::BoundsDictZstdVals => {
                let vals: Vec<i64> = pco_pack::rmp_serde::from_slice(&unzip(&blob[self.payload_off(blob)..])).unwrap();
                vals.iter().filter(|v| targets.contains(v)).count()
            }
            Kind::BoundsDictPcoRanks => {
                let (dictionary, _, _) = parse_dictionary(&blob[BOUNDS_PREFIX_LEN..]);
                let want = want_ranks(&dictionary, targets);
                pco_vals_i64(&blob[self.payload_off(blob)..])
                    .into_iter()
                    .filter(|r| want.contains(&(*r as u32)))
                    .count()
            }
            Kind::BoundsDictZstdRanks => {
                let (dictionary, _, _) = parse_dictionary(&blob[BOUNDS_PREFIX_LEN..]);
                let want = want_ranks(&dictionary, targets);
                let ranks: Vec<u32> = pco_pack::rmp_serde::from_slice(&unzip(&blob[self.payload_off(blob)..])).unwrap();
                ranks.iter().filter(|r| want.contains(r)).count()
            }
            Kind::BoundsDictRle => {
                // Consuming run pairs directly is the honest engine shape:
                // matching runs extend the selection, others are skipped
                // without touching any row.
                let (dictionary, _, _) = parse_dictionary(&blob[BOUNDS_PREFIX_LEN..]);
                let want = want_ranks(&dictionary, targets);
                let mut pos = self.payload_off(blob);
                let mut hits = 0usize;
                while pos < blob.len() {
                    let rank = get_varint(blob, &mut pos) as u32;
                    let run = get_varint(blob, &mut pos) as usize;
                    if want.binary_search(&rank).is_ok() {
                        hits += run;
                    }
                }
                hits
            }
            Kind::BoundsDictZstdCodes => {
                // Raw u8 indexes: the filter scans bytes (no row materialization).
                let (dictionary, _, _) = parse_dictionary(&blob[BOUNDS_PREFIX_LEN..]);
                let want: Vec<u8> = want_ranks(&dictionary, targets).into_iter().map(|r| r as u8).collect();
                let idx = unzip(&blob[self.payload_off(blob)..]);
                idx.iter().filter(|c| want.binary_search(c).is_ok()).count()
            }
            Kind::BoundsDictZstdIdx16 => {
                // Raw u16 indexes: inflate once, two-byte compare, no row
                // materialization; same engine shape as the u8 scan.
                let (dictionary, _, _) = parse_dictionary(&blob[BOUNDS_PREFIX_LEN..]);
                let want: Vec<u16> = want_ranks(&dictionary, targets).into_iter().map(|r| r as u16).collect();
                let idx = unzip(&blob[self.payload_off(blob)..]);
                idx.chunks_exact(2).filter(|b| want.binary_search(&u16::from_le_bytes([b[0], b[1]])).is_ok()).count()
            }
            Kind::TryDict => pco_vals_i64(&blob[BOUNDS_PREFIX_LEN..]).into_iter().filter(|v| targets.contains(v)).count(),
            Kind::PcoUniqIdx | Kind::MpUniqIdx => {
                let (dict_bytes, rest) = take_len(&blob[BOUNDS_PREFIX_LEN..]);
                let (idx_bytes, _) = take_len(rest);
                let dict: Vec<i64> = match self {
                    Kind::PcoUniqIdx => pco_vals_i64(dict_bytes),
                    _ => pco_pack::rmp_serde::from_slice(&unzip(dict_bytes)).unwrap(),
                };
                let idxs: Vec<u32> = match self {
                    Kind::PcoUniqIdx => pco_vals_i64(idx_bytes).into_iter().map(|v| v as u32).collect(),
                    _ => pco_pack::rmp_serde::from_slice(&unzip(idx_bytes)).unwrap(),
                };
                let want: Vec<u32> = (0..dict.len() as u32).filter(|i| targets.contains(&dict[*i as usize])).collect();
                idxs.iter().filter(|i| want.contains(i)).count()
            }
            Kind::MpValues => {
                let (frame, _) = take_len(&blob[BOUNDS_PREFIX_LEN..]);
                let vals: Vec<i64> = pco_pack::rmp_serde::from_slice(&unzip(frame)).unwrap();
                vals.iter().filter(|v| targets.contains(v)).count()
            }
        }
    }
}

struct Col {
    name: &'static str,
    data: Vec<i64>,
}

/// One row per approach: compressed size, serialize time, and filter time per
/// query. Filter cells are ms with `(n)` blobs skipped by the guard when
/// nonzero.
struct KindRow {
    approach: String,
    size_mb: String,
    ser_ms: String,
    eq_common: String,
    eq_rare: String,
    in2: String,
    absent: String,
}

impl markdown_tables::MarkdownTableRow for KindRow {
    fn column_names() -> Vec<&'static str> {
        vec!["Approach", "Size MB", "ser ms", "eq common", "eq rare", "IN 2", "absent"]
    }
    fn column_values(&self) -> Vec<String> {
        vec![
            self.approach.clone(),
            self.size_mb.clone(),
            self.ser_ms.clone(),
            self.eq_common.clone(),
            self.eq_rare.clone(),
            self.in2.clone(),
            self.absent.clone(),
        ]
    }
}

/// Framing key row explaining how two approaches differ.
struct FramingRow {
    approach: &'static str,
    framing: &'static str,
}

impl markdown_tables::MarkdownTableRow for FramingRow {
    fn column_names() -> Vec<&'static str> {
        vec!["Approach", "Framing"]
    }
    fn column_values(&self) -> Vec<String> {
        vec![self.approach.to_string(), self.framing.to_string()]
    }
}

fn run_col(col: &Col, kinds: &[Kind]) {
    let n = col.data.len();
    let mut hist: HashMap<i64, usize> = HashMap::new();
    for &v in &col.data {
        *hist.entry(v).or_insert(0) += 1;
    }
    let mut by_count: Vec<(i64, usize)> = hist.iter().map(|(v, c)| (*v, *c)).collect();
    by_count.sort_by_key(|&(v, _)| v);
    by_count.sort_by_key(|&(_, c)| c);
    let rarest = by_count[0].0;
    let common = by_count[by_count.len() - 1].0;
    let mut two: Vec<i64> = by_count.iter().take(2).map(|&(v, _)| v).collect();
    two.sort_unstable();
    two.dedup();
    let max = *col.data.iter().max().unwrap();
    let queries: Vec<(&str, Vec<i64>)> = vec![
        ("eq common", vec![common]),
        ("eq rare", vec![rarest]),
        ("IN 2", two.clone()),
        ("absent", vec![max + 1]),
    ];

    println!("\n## {} - {} rows, {} distinct, {} blobs\n", col.name, n, by_count.len(), n.div_ceil(BLOB_N));
    let mut rows: Vec<KindRow> = Vec::new();
    for kind in kinds {
        if matches!(kind, Kind::BoundsDictZstdCodes) && by_count.len() > 256 {
            continue; // u8 indexes need the distinct count to fit in a byte
        }
        // Callers offer u16 indexes only above the 256 gate, where u8 is
        // impossible; below the gate production always prefers the narrower
        // payload, so a u16 row there would measure a choice nobody makes.
        if matches!(kind, Kind::BoundsDictZstdIdx16) && by_count.len() <= 256 {
            continue;
        }
        let (ser_ms, blobs) =
            datasets::common::one_pass(|| col.data.chunks(BLOB_N).map(|c| kind.encode(c)).collect::<Vec<_>>());
        let bytes: usize = blobs.iter().map(|b| b.len()).sum();
        let mut cells: Vec<String> = Vec::new();
        for (label, targets) in queries.iter() {
            let (ms, (hits, skipped)) = datasets::common::one_pass(|| {
                let mut hits = 0usize;
                let mut skipped = 0usize;
                for blob in &blobs {
                    if kind.guard_excludes(blob, targets) {
                        skipped += 1;
                        continue;
                    }
                    hits += kind.decode_hits(blob, targets);
                }
                (hits, skipped)
            });
            // A Filter over a target dictionary counts each row once, so duplicate
            // targets must not double the expected hit count.
            let expected: usize = targets
                .iter()
                .copied()
                .collect::<std::collections::HashSet<i64>>()
                .iter()
                .map(|t| *hist.get(t).unwrap_or(&0))
                .sum();
            assert_eq!(hits, expected, "{}/{} {}", kind.name(), label, col.name);
            let cell = if skipped > 0 { format!("{:.1} ({})", ms, skipped) } else { format!("{:.1}", ms) };
            cells.push(cell);
        }
        let mut cells = cells.into_iter();
        rows.push(KindRow {
            approach: kind.name().to_string(),
            size_mb: format!("{:.1}", bytes as f64 / 1e6),
            ser_ms: format!("{:.1}", ser_ms),
            eq_common: cells.next().unwrap(),
            eq_rare: cells.next().unwrap(),
            in2: cells.next().unwrap(),
            absent: cells.next().unwrap(),
        });
        drop(blobs);
    }
    println!("{}", markdown_tables::as_table(&rows));
}

/// Test framing printed under the title; explains what is measured and why.
const WHAT_AND_WHY: &str = r#"## What this tests, and why

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

"#;

/// Outcome record printed under the framing; update it when evidence moves.
const SUMMARY: &str = r#"## Summary

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

"#;

/// Encoding-selection rules for production, distilled from the tables.
const RUNTIME_HEURISTICS: &str = r#"## Runtime heuristics

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

"#;

fn main() {
    println!("# Dictionary-like serialization and filter performance\n");
    println!("{WHAT_AND_WHY}");
    println!("{SUMMARY}");
    println!("{RUNTIME_HEURISTICS}");
    println!("## Framing key\n");
    let key: Vec<FramingRow> = [
        Kind::PcoNoBounds,
        Kind::BoundsPco,
        Kind::BoundsDictPco,
        Kind::BoundsDictZstdVals,
        Kind::BoundsDictPcoRanks,
        Kind::BoundsDictZstdRanks,
        Kind::BoundsDictRle,
        Kind::BoundsDictZstdCodes,
        Kind::BoundsDictZstdIdx16,
        Kind::PcoUniqIdx,
        Kind::TryDict,
        Kind::MpUniqIdx,
        Kind::MpValues,
    ]
    .into_iter()
    .map(|k| FramingRow { approach: k.name(), framing: k.framing() })
    .collect();
    println!("{}", markdown_tables::as_table(&key));
    let works = datasets::openalex::load();
    let cols = vec![
        Col { name: "openalex work_type", data: works.iter().map(|w| w.work_type as i64).collect() },
        Col { name: "openalex oa_status", data: works.iter().map(|w| w.oa_status as i64).collect() },
        Col { name: "openalex cited_by_count (ctrl)", data: works.iter().map(|w| w.cited_by_count as i64).collect() },
    ];
    drop(works);
    for c in &cols {
        run_col(
            c,
            &[
                Kind::PcoNoBounds,
                Kind::BoundsPco,
                Kind::BoundsDictPco,
                Kind::BoundsDictZstdVals,
                Kind::BoundsDictPcoRanks,
                Kind::BoundsDictZstdRanks,
                Kind::BoundsDictRle,
                Kind::BoundsDictZstdCodes,
                Kind::BoundsDictZstdIdx16,
                Kind::PcoUniqIdx,
                Kind::TryDict,
                Kind::MpUniqIdx,
                Kind::MpValues,
            ],
        );
    }
    drop(cols);

    let trips = datasets::taxi::load();
    let cols = vec![
        Col { name: "taxi pu_location_id", data: trips.iter().map(|t| t.pu_location_id as i64).collect() },
        Col { name: "taxi do_location_id", data: trips.iter().map(|t| t.do_location_id as i64).collect() },
    ];
    drop(trips);
    for c in &cols {
        run_col(
            c,
            &[
                Kind::PcoNoBounds,
                Kind::BoundsPco,
                Kind::BoundsDictPco,
                Kind::BoundsDictZstdVals,
                Kind::BoundsDictZstdRanks,
                Kind::BoundsDictRle,
                Kind::BoundsDictZstdCodes,
                Kind::BoundsDictZstdIdx16,
                Kind::TryDict,
                Kind::MpValues,
            ],
        );
    }
    drop(cols);

    let name = "backend_snapshots.msgpack.zst";
    let rows: Vec<postgres::backends::BackendSnapshot> =
        postgres::common::read_blob(name, &postgres::common::data_dir(), &postgres::common::lfs_warning(name))
            .unwrap()
            .unwrap();
    let cols = vec![
        Col { name: "backends backend_type", data: rows.iter().map(|r| r.backend_type as i64).collect() },
        Col { name: "backends backend_state", data: rows.iter().map(|r| r.backend_state as i64).collect() },
        Col { name: "backends wait_event_type", data: rows.iter().map(|r| r.wait_event_type as i64).collect() },
    ];
    drop(rows);
    for c in &cols {
        run_col(
            c,
            &[
                Kind::PcoNoBounds,
                Kind::BoundsPco,
                Kind::BoundsDictPco,
                Kind::BoundsDictZstdVals,
                Kind::BoundsDictPcoRanks,
                Kind::BoundsDictZstdRanks,
                Kind::BoundsDictRle,
                Kind::BoundsDictZstdCodes,
                Kind::BoundsDictZstdIdx16,
                Kind::TryDict,
                Kind::MpValues,
            ],
        );
    }
}
