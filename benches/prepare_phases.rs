//! Throwaway phase-attribution bench for the per-chunk fold pipeline.
//!
//! Measures where wall time goes: msgpack container copy, pure pco column
//! decode, the full fold path, the fold closure on a pre-warmed reader, thread
//! scaling of chunk folds, and the cost of moving a warmed chunk across a
//! thread boundary. Run with `cargo bench --bench prepare_phases`.

use pco_pack::{Consumer, FilterMask, LazyReader, PcoFilter, PcoPack, PcoSerde, fold};
use std::collections::HashMap;
use std::hint::black_box;
use std::time::Instant;

const CHUNK_ROWS: usize = 262_144; // lib default CHUNK_SIZE, so write() yields CHUNKS chunks.
const CHUNKS: usize = 18; // matches core count on the target machine.

#[derive(Clone, PcoPack)]
#[pco_pack(index = [device_id], timestamp = ts)]
struct Wide8 {
    device_id: i64,
    ts: i64,
    c0: i64,
    c1: i64,
    c2: i64,
    c3: i64,
    c4: i64,
    c5: i64,
    c6: i64,
    c7: i64,
}

#[derive(Clone, PcoPack)]
#[pco_pack(index = [device_id], timestamp = ts)]
struct Wide1 {
    device_id: i64,
    ts: i64,
    c0: i64,
}

type R8 = <Wide8 as PcoSerde>::Reader;
type C8 = <Wide8 as PcoPack>::Chunk;
type R1 = <Wide1 as PcoSerde>::Reader;

/// xorshift64* so the data is identical across reps and runs.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

fn gen_wide8() -> Vec<Wide8> {
    let mut rng = Rng(0x2545_F491_4F6C_DD1D);
    let mut v = Vec::with_capacity(CHUNKS * CHUNK_ROWS);
    for i in 0..(CHUNKS * CHUNK_ROWS) {
        v.push(Wide8 {
            device_id: 0,
            ts: i as i64,
            c0: (rng.next() & 0xFF_FFFF) as i64,
            c1: (rng.next() & 0xFF_FFFF) as i64,
            c2: (rng.next() & 0xFF_FFFF) as i64,
            c3: (rng.next() & 0xFF_FFFF) as i64,
            c4: (rng.next() & 0xFF_FFFF) as i64,
            c5: (rng.next() & 0xFF_FFFF) as i64,
            c6: (rng.next() & 0xFF_FFFF) as i64,
            c7: (rng.next() & 0xFF_FFFF) as i64,
        });
    }
    v
}

fn gen_wide1() -> Vec<Wide1> {
    let mut rng = Rng(0x2545_F491_4F6C_DD1D);
    let mut v = Vec::with_capacity(CHUNKS * CHUNK_ROWS);
    for i in 0..(CHUNKS * CHUNK_ROWS) {
        v.push(Wide1 { device_id: 0, ts: i as i64, c0: (rng.next() & 0xFF_FFFF) as i64 });
    }
    v
}

fn min_ms<R, F: FnMut() -> R>(reps: usize, mut f: F) -> f64 {
    let mut best = f64::INFINITY;
    for _ in 0..reps {
        let t = Instant::now();
        let out = f();
        let dt = t.elapsed().as_secs_f64() * 1e3;
        best = best.min(dt);
        black_box(out);
    }
    best
}

fn mbs(bytes: u64, ms: f64) -> f64 {
    (bytes as f64 / (1 << 20) as f64) / (ms / 1e3)
}

fn zero() -> pco_pack::chrono::Duration {
    pco_pack::chrono::Duration::zero()
}

/// Builds a chunk reader with every column's compressed block owned, then
/// decompresses ts + c0..c7 so the fold tail runs with zero decode left.
fn warm8(chunk: &C8) -> (R8, FilterMask, usize) {
    let mut reader = R8::default();
    reader.device_id = chunk.device_id;
    reader.start_at = chunk.start_at;
    reader.end_at = chunk.end_at;
    reader.ts = LazyReader::new(chunk.ts.to_vec(), 0u32, zero());
    reader.c0 = LazyReader::new(chunk.c0.to_vec(), 0u32, zero());
    reader.c1 = LazyReader::new(chunk.c1.to_vec(), 0u32, zero());
    reader.c2 = LazyReader::new(chunk.c2.to_vec(), 0u32, zero());
    reader.c3 = LazyReader::new(chunk.c3.to_vec(), 0u32, zero());
    reader.c4 = LazyReader::new(chunk.c4.to_vec(), 0u32, zero());
    reader.c5 = LazyReader::new(chunk.c5.to_vec(), 0u32, zero());
    reader.c6 = LazyReader::new(chunk.c6.to_vec(), 0u32, zero());
    reader.c7 = LazyReader::new(chunk.c7.to_vec(), 0u32, zero());
    let rc = reader.c0.row_count().unwrap();
    reader.ts.reader_mut().unwrap();
    reader.c0.reader_mut().unwrap();
    reader.c1.reader_mut().unwrap();
    reader.c2.reader_mut().unwrap();
    reader.c3.reader_mut().unwrap();
    reader.c4.reader_mut().unwrap();
    reader.c5.reader_mut().unwrap();
    reader.c6.reader_mut().unwrap();
    reader.c7.reader_mut().unwrap();
    (reader, FilterMask::ones(rc), rc)
}

/// Fold tail only: columns view + mask walk over a reader whose columns are already warm.
fn tail8<C: Consumer<Wide8>>(warmed: &mut (R8, FilterMask, usize), consumer: &mut C) {
    let (reader, mask, rc) = warmed;
    let mut columns = Wide8::columns(reader);
    mask.for_each_set_row(*rc, |r| consumer.consume(&mut columns, r)).unwrap();
}

macro_rules! fold8_sum {
    ($sum:expr) => {
        fold!(Wide8, |c0: i64, c1: i64, c2: i64, c3: i64, c4: i64, c5: i64, c6: i64, c7: i64| $sum +=
            c0 as u64 + c1 as u64 + c2 as u64 + c3 as u64 + c4 as u64 + c5 as u64 + c6 as u64 + c7 as u64)
    };
}

fn main() {
    println!("available_parallelism\t{}", std::thread::available_parallelism().unwrap());

    let t = Instant::now();
    let chunks: Vec<C8> = <Wide8 as PcoPack>::write(gen_wide8()).unwrap();
    let write8_ms = t.elapsed().as_secs_f64() * 1e3;
    println!("[untimed] Wide8 write {} chunks in {:.1} ms", chunks.len(), write8_ms);
    assert_eq!(chunks.len(), CHUNKS);

    let all_cols: [&str; 8] = ["c0", "c1", "c2", "c3", "c4", "c5", "c6", "c7"];
    let q_empty = serde_json::json!({});
    let plan_empty = <Wide8 as PcoFilter>::resolve_query(&q_empty).unwrap();
    let fields_all = Wide8::resolve_fields(&q_empty, &all_cols).unwrap();

    let q_filter = serde_json::json!({ "c0": { "start": 0x7F_FFFF_i64, "end": 0xFF_FFFF_i64 } });
    let plan_filter = <Wide8 as PcoFilter>::resolve_query(&q_filter).unwrap();
    let fields_filter = Wide8::resolve_fields(&q_filter, &all_cols).unwrap();

    let mut rows: Vec<(String, f64, String)> = Vec::new();
    let mut note = |name: &str, ms: f64, extra: String| rows.push((name.to_string(), ms, extra));

    // 1. msgpack container copy.
    let blob = Wide8::to_bytes(&chunks).unwrap();
    let to_bytes_ms = min_ms(3, || Wide8::to_bytes(&chunks).unwrap().len());
    let blob_bytes = blob.len() as u64;
    note("to_bytes", to_bytes_ms, format!("blob {} MB", blob_bytes as f64 / (1 << 20) as f64));
    let from_bytes_ms = min_ms(3, || Wide8::from_bytes(&blob).unwrap().len());
    note("from_bytes", from_bytes_ms, format!("{} MB/s blob", mbs(blob_bytes, from_bytes_ms)));
    drop(blob);

    // 2. pure pco decode, every payload column of every chunk, read + drop.
    let decode_ms = min_ms(3, || {
        let mut cols = 0usize;
        for chunk in &chunks {
            for col in [&chunk.c0, &chunk.c1, &chunk.c2, &chunk.c3, &chunk.c4, &chunk.c5, &chunk.c6, &chunk.c7] {
                let mut cur = std::io::Cursor::new(col.as_slice());
                let reader = <i64 as PcoSerde>::read(&mut cur, 0u32, zero()).unwrap();
                black_box(reader);
                cols += 1;
            }
        }
        cols
    });
    let decode_bytes = CHUNKS as u64 * 8 * CHUNK_ROWS as u64 * 8;
    note("pco_decode_per_column", decode_ms, format!("{} MB/s", mbs(decode_bytes, decode_ms)));

    // 3. full fold path, all 8 columns.
    let fold_all_ms = min_ms(3, || {
        let mut sum = 0u64;
        {
            let mut consumer = fold8_sum!(sum);
            for chunk in &chunks {
                chunk.fold_into(&plan_empty, &fields_all, &mut consumer).unwrap();
            }
        }
        sum
    });
    note("fold_into_all_cols", fold_all_ms, format!("{} MB/s", mbs(decode_bytes, fold_all_ms)));

    // 4. full fold path, one column.
    let fields_c0 = Wide8::resolve_fields(&q_empty, &["c0"]).unwrap();
    let fold_one_ms = min_ms(3, || {
        let mut sum = 0u64;
        {
            let mut consumer = fold!(Wide8, |c0: i64| sum += c0 as u64);
            for chunk in &chunks {
                chunk.fold_into(&plan_empty, &fields_c0, &mut consumer).unwrap();
            }
        }
        sum
    });
    let one_col_bytes = CHUNKS as u64 * CHUNK_ROWS as u64 * 8;
    note("fold_into_one_col", fold_one_ms, format!("{} MB/s", mbs(one_col_bytes, fold_one_ms)));

    // 6. full fold path with a ~50% range filter on c0.
    let fold_filter_ms = min_ms(3, || {
        let mut sum = 0u64;
        {
            let mut consumer = fold8_sum!(sum);
            for chunk in &chunks {
                chunk.fold_into(&plan_filter, &fields_filter, &mut consumer).unwrap();
            }
        }
        sum
    });
    note("fold_into_all_cols_filter", fold_filter_ms, String::new());

    // 7. fold closure only, all 8 cols, pre-warmed readers (A3, sum).
    // Scalar column reads carry no cursor state, so the prepared readers are
    // reused across reps without re-warming.
    let mut warmed: Vec<(R8, FilterMask, usize)> = chunks.iter().map(warm8).collect();
    let warm_sum_ms = min_ms(3, || {
        let mut sum = 0u64;
        {
            let mut consumer = fold8_sum!(sum);
            for w in warmed.iter_mut() {
                tail8(w, &mut consumer);
            }
        }
        sum
    });
    note("fold_tail_warm_sum", warm_sum_ms, format!("{} MB/s", mbs(decode_bytes, warm_sum_ms)));

    // 8. fold closure only, hashmap consumer (A3, hashmap). Map build is inside the timed region.
    let warm_hash_ms = min_ms(3, || {
        let mut map = HashMap::<i64, u32>::new();
        {
            let mut consumer = fold!(Wide8, |c0: i64| {
                *map.entry(c0).or_default() += 1;
            });
            for w in warmed.iter_mut() {
                tail8(w, &mut consumer);
            }
        }
        map.len()
    });
    note("fold_tail_warm_hashmap", warm_hash_ms, String::new());

    // 9. full materialization upper bound.
    let expand_ms = min_ms(2, || <Wide8 as PcoPack>::read(chunks.clone()).unwrap().len());
    note(
        "expand_upper",
        expand_ms,
        format!("{} rows/s", CHUNKS as u64 * CHUNK_ROWS as u64 * 1000 / expand_ms as u64),
    );

    // 10. parallel fold scaling (A4).
    let mut speedups = Vec::new();
    let mut base_w1 = 0f64;
    for &w in &[1usize, 2, 4, 8] {
        let ms = min_ms(3, || {
            std::thread::scope(|s| {
                let mut handles = Vec::new();
                for t in 0..w {
                    let idxs: Vec<usize> = (t..CHUNKS).step_by(w).collect();
                    let chunks = chunks.as_slice();
                    let plan = &plan_empty;
                    let fields = &fields_all;
                    handles.push(s.spawn(move || {
                        let mut sum = 0u64;
                        {
                            let mut consumer = fold8_sum!(sum);
                            for &i in &idxs {
                                chunks[i].fold_into(plan, fields, &mut consumer).unwrap();
                            }
                        }
                        sum
                    }));
                }
                handles.into_iter().map(|h| h.join().unwrap()).sum::<u64>()
            })
        });
        if w == 1 {
            base_w1 = ms;
        }
        speedups.push((w, ms, base_w1 / ms));
        note(&format!("parallel_fold_W{}", w), ms, String::new());
    }

    // 11. transfer probe (A5): serial prepare+fold vs pipelined across a channel.
    let serial_ms = min_ms(3, || {
        let mut sum = 0u64;
        {
            let mut consumer = fold8_sum!(sum);
            for chunk in &chunks {
                let mut w = warm8(chunk);
                tail8(&mut w, &mut consumer);
            }
        }
        sum
    });
    note("transfer_serial", serial_ms, String::new());
    let pipelined_ms = min_ms(3, || {
        std::thread::scope(|s| {
            let (tx, rx) = std::sync::mpsc::sync_channel::<(R8, FilterMask, usize)>(2);
            // tx must live on the producer thread and drop there, or recv never
            // sees channel close and scope deadlocks.
            let chunks_ref: &Vec<C8> = &chunks;
            s.spawn(move || {
                for chunk in chunks_ref.iter() {
                    if tx.send(warm8(chunk)).is_err() {
                        break;
                    }
                }
            });
            let mut sum = 0u64;
            {
                let mut consumer = fold8_sum!(sum);
                while let Ok(mut w) = rx.recv() {
                    tail8(&mut w, &mut consumer);
                }
            }
            sum
        })
    });
    note("transfer_pipelined", pipelined_ms, String::new());

    // Wide1 model, generated only after Wide8 is dropped.
    let wide1_chunks = <Wide1 as PcoPack>::write(gen_wide1()).unwrap();
    assert_eq!(wide1_chunks.len(), CHUNKS);
    let q1 = serde_json::json!({});
    let plan1 = <Wide1 as PcoFilter>::resolve_query(&q1).unwrap();
    let fields1 = Wide1::resolve_fields(&q1, &["c0"]).unwrap();
    let fold_w1_ms = min_ms(3, || {
        let mut sum = 0u64;
        {
            let mut consumer = fold!(Wide1, |c0: i64| sum += c0 as u64);
            for chunk in &wide1_chunks {
                chunk.fold_into(&plan1, &fields1, &mut consumer).unwrap();
            }
        }
        sum
    });
    note("fold_into_one_col_w1", fold_w1_ms, format!("{} MB/s", mbs(one_col_bytes, fold_w1_ms)));
    drop(wide1_chunks);

    println!("scenario\tmin_ms\tnote");
    for (name, ms, extra) in &rows {
        println!("{}\t{:.2}\t{}", name, ms, extra);
    }

    let pct = |a: f64, b: f64| (a / b) * 100.0;
    println!("\nderived\tvalue");
    println!("decode_share\t{:.3}\ts2/s3", decode_ms / fold_all_ms);
    println!(
        "container_share\t{:.3}\tfrom_bytes/(from_bytes+s3)",
        from_bytes_ms / (from_bytes_ms + fold_all_ms)
    );
    println!("filter_share\t{:.3}\t(s6-s3)/s6", (fold_filter_ms - fold_all_ms) / fold_filter_ms);
    println!("warm_fold_share_sum\t{:.3}\ts7/s3", warm_sum_ms / fold_all_ms);
    println!("warm_fold_share_hashmap\t{:.3}\ts8/s3", warm_hash_ms / fold_all_ms);
    println!("per_col_ms\t{:.2}\t(s3-s4)/7", (fold_all_ms - fold_one_ms) / 7.0);
    for (w, ms, speedup) in &speedups {
        if *w > 1 {
            println!("speedup_W{}\t{:.2} {:.2} ms", w, speedup, ms);
        }
    }
    println!(
        "transfer_overhead_pct\t{:.1}\t(pipelined-serial)/serial*100",
        pct(pipelined_ms - serial_ms, serial_ms)
    );
}
