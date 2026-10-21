//! Measures whether pco's built-in chunk metadata can serve as filter bounds.
//!
//! For each numeric column distribution used in the benchmarks, compresses a
//! column the way PcoPack does, then inspects the first chunk's metadata
//! without decoding its body, reporting: selected mode/delta encoding, the
//! ANS bin table (which bounds latent ranges), the original-value bounds
//! derivable from it, the cost of parsing metadata alone versus a full
//! decode, and whether the derived bounds would skip each benchmark's real
//! filter targets.
//!
//! Structural facts decide the answer (verified against pco 1.0.2 sources):
//! - `ChunkMeta` bins bound latent-space ranges. Only when delta encoding
//!   is NoOp do latents stand in for original values; with delta encoding
//!   they bound differences, hiding original bounds.
//! - The first chunk's metadata is reachable without decoding, but later
//!   chunk bodies carry no byte-length record, so their headers are only
//!   reachable by decoding earlier chunks.
use pco::standalone::{DecompressorItem, FileDecompressor, simple_compress};

fn config() -> pco::ChunkConfig {
    pco::ChunkConfig::default().with_enable_8_bit(true)
}

/// Parse the first chunk's metadata without touching its body.
fn first_chunk_meta(bytes: &[u8]) -> Option<(pco::metadata::ChunkMeta, usize)> {
    let (fd, src) = FileDecompressor::new(bytes).ok()?;
    match fd.chunk_decompressor::<i64, &[u8]>(src).ok()? {
        DecompressorItem::Chunk(cd) => Some((cd.meta().clone(), cd.n())),
        DecompressorItem::EndOfData(_) => None,
    }
}

fn probe_i64(name: &str, values: &[i64], filter_range: Option<(i64, i64)>) {
    let bytes = simple_compress(values, &config()).unwrap();
    let true_min = values.iter().copied().min().unwrap();
    let true_max = values.iter().copied().max().unwrap();

    // Metadata-only parse cost: same walk, but reading zero rows.
    let t_meta = std::time::Instant::now();
    let (meta, _) = first_chunk_meta(&bytes).expect("one chunk minimum");
    let meta_us = t_meta.elapsed().as_secs_f64() * 1e6;

    // Original-value bounds derivable from header metadata alone:
    // - NoOp delta: the ANS bin table (U64 latents centered at 2^63)
    //   exactly covers every value, so min/max invert losslessly.
    // - Dict: the dictionary of unique values is right there.
    // Everything else (delta, mult, quant) bounds latents, not values.
    let mut derived: Option<(i64, i64)> = None;
    if matches!(meta.delta_encoding, pco::metadata::DeltaEncoding::NoOp) {
        if let pco::metadata::DynBins::U64(bins) = &meta.per_latent_var.primary.bins {
            let lo = bins.iter().map(|b| b.lower).min().unwrap();
            let hi = bins
                .iter()
                .map(|b| {
                    let width = if b.offset_bits >= 64 { u64::MAX } else { (1u64 << b.offset_bits) - 1 };
                    b.lower.saturating_add(width)
                })
                .max()
                .unwrap();
            derived = Some((lo.wrapping_sub(1u64 << 63) as i64, hi.wrapping_sub(1u64 << 63) as i64));
        }
    }
    if let pco::metadata::Mode::Dict(dict) = &meta.mode {
        if let pco::metadata::DynLatents::U64(ds) = dict {
            let lo = ds.iter().copied().min().unwrap().wrapping_sub(1u64 << 63) as i64;
            let hi = ds.iter().copied().max().unwrap().wrapping_sub(1u64 << 63) as i64;
            derived = Some((lo, hi));
        }
    }

    // Full decode cost for comparison.
    let t_full = std::time::Instant::now();
    let (fd, mut src) = FileDecompressor::new(&bytes[..]).unwrap();
    let mut n_chunks = 0usize;
    let mut all_decoded: Vec<i64> = Vec::new();
    loop {
        match fd.chunk_decompressor::<i64, &[u8]>(src).unwrap() {
            DecompressorItem::Chunk(mut cd) => {
                n_chunks += 1;
                let mut buf = vec![0i64; cd.n()];
                cd.read(&mut buf).unwrap();
                all_decoded.extend(buf);
                src = cd.into_src();
            }
            DecompressorItem::EndOfData(_) => break,
        }
    }
    let full_ms = t_full.elapsed().as_secs_f64() * 1e3;
    assert_eq!(all_decoded.len(), values.len());

    let no_delta = matches!(meta.delta_encoding, pco::metadata::DeltaEncoding::NoOp);
    println!(
        "{:<22} rows={:>7} chunks={} bytes={:>8} meta={:>5.1}us full={:>7.1}ms no_delta={} true_range=[{},{}] mode={:?} delta={:?}",
        name,
        values.len(),
        n_chunks,
        bytes.len(),
        meta_us,
        full_ms,
        no_delta,
        true_min,
        true_max,
        meta.mode,
        meta.delta_encoding,
    );
    if !no_delta && !matches!(meta.mode, pco::metadata::Mode::Dict(_)) {
        println!("    primary bins: (delta/mult latents: original bounds hidden)");
    } else {
        println!("    primary bins: {:?}", meta.per_latent_var.primary.bins);
    }
    match derived {
        Some((lo, hi)) => println!("    derived bounds: [{}, {}] (tight={})", lo, hi, lo == true_min && hi == true_max),
        None => println!("    derived bounds: none"),
    }
    if let Some((f_start, f_end)) = filter_range {
        let has_match = values.iter().any(|v| f_start <= *v && *v <= f_end);
        let meta_skips = derived.map(|(lo, hi)| hi < f_start || lo > f_end).unwrap_or(false);
        println!(
            "    filter [{}, {}]: rows_match={} meta_skip={} {}",
            f_start,
            f_end,
            has_match,
            meta_skips,
            match (has_match, meta_skips) {
                (false, false) => "MISSED (exact bounds would skip)",
                (true, true) => "FALSE SKIP!!",
                _ => "ok",
            }
        );
    }
}

pub fn main() {
    println!("pco metadata probe (simple_compress, enable_8_bit)\n");
    let n = 100_000usize;

    let mut seed = 0xdeadbeef12345678u64;
    let mut rnd = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed as i64
    };

    // query_stats Loud filter shape: calls >= 10 over mostly-zero per-minute
    // counts. Note: in the real schema `calls` lives in the msgpack records,
    // not a pco column; this shows what pco metadata would do if it were.
    let calls: Vec<i64> = (0..n)
        .map(|_| {
            let r = rnd() as u64;
            match r % 100 {
                0 => ((r >> 7) % 100_000) as i64,
                _ => ((r >> 7) % 8) as i64,
            }
        })
        .collect();
    probe_i64("calls spiky", &calls, Some((10, i64::MAX)));

    // query_stats Db filter shape: database_id repeated within a chunk.
    // In the real schema db id is a chunk constant (index column), so it
    // never lands here; shown for completeness.
    probe_i64("db repeated", &vec![4242i64; n], Some((999, 999)));

    // filter.rs AllTypes id column: bucket*1000, 100 distinct values.
    probe_i64(
        "id buckets*1000",
        &(0..n).map(|i| ((i / 1000) % 100) as i64 * 1000).collect::<Vec<_>>(),
        Some((50_000, 50_000)),
    );
    // int32_val bucket pattern widened to i64.
    probe_i64("int32 buckets 0..99", &(0..n).map(|i| ((i / 1000) % 100) as i64).collect::<Vec<_>>(), Some((50, 50)));
    // Monotonic micros (timestamp / counter columns).
    probe_i64(
        "i64 monotonic +1",
        &(0..n as i64).map(|i| 1_700_000_000_000_000 + i).collect::<Vec<_>>(),
        Some((1_700_000_050_000_000, 1_700_000_099_999_999)),
    );
    // fingerprint-like random hash values.
    probe_i64(
        "i64 random 53-bit",
        &(0..n).map(|_| rnd() >> 11).collect::<Vec<_>>(),
        Some((-4_000_000_000_000_000, -4_000_000_000_000_000)),
    );
    // All-zero chunk (sparse calls chunks): tight bins expected.
    probe_i64("i64 all zeros", &vec![0i64; n], Some((10, i64::MAX)));
    // Mostly small values with one big value.
    let mut mostly_small: Vec<i64> = (0..n).map(|_| rnd().abs() % 8).collect();
    mostly_small[50_000] = 9_999_999;
    probe_i64("i64 small+1 spike", &mostly_small, Some((10_000_000, i64::MAX)));

    // backend_state-like low-cardinality small ints (backend_snapshots
    // Inclusion filter). Widen to i64 to keep the latent type stable.
    let states: Vec<i64> = (0..n).map(|_| rnd().abs() % 6).collect();
    probe_i64("state 0..5", &states, Some((1, 2)));
    // A chunk of states all 0 (idle pool): exact skip case.
    probe_i64("state all idle", &vec![6i64; n], Some((1, 2)));
    // wait_event-like: mostly 0 (no wait), small event ids.
    let waits: Vec<i64> = (0..n)
        .map(|_| {
            let r = rnd() as u64;
            if r % 4 == 0 { ((r >> 3) % 40) as i64 } else { 0 }
        })
        .collect();
    probe_i64("wait_event mostly0", &waits, Some((1, i64::from(i16::MAX))));
}
