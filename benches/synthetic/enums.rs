use super::*;

use arrow_schema::FieldRef;
use columnar::{Borrow, Columnar, FromBytes, Index, Len};
use markdown_tables::{MarkdownTableRow, as_table};
use pco_pack::PcoPack;
use serde::Serialize;
use serde_arrow::schema::{SchemaLike, TracingOptions};
use serde_columnar::{columnar, from_bytes, to_vec};
use serde_json::json;
use vortex::array::expr::{col, eq, lit, or};
use vortex::array::stream::ArrayStreamExt;
use vortex::arrow::ArrowSessionExt as _;
use vortex::buffer::ByteBuffer;
use vortex::compressor::BtrBlocksCompressorBuilder;
use vortex::file::{OpenOptionsSessionExt, WriteOptionsSessionExt, WriteStrategyBuilder};
use vortex::session::VortexSession;

/// Flat-tagged column layouts only. The `columnar`, `serde_columnar`, and
/// Vortex codecs have no top-level tagged union, so each stores the same
/// event stream as a discriminator plus one column per payload field:
/// non-belonging fields take their default value.
const TICK: u8 = 1;
const NOTE: u8 = 2;

/// Row shares: Tick is ~10%, Note ~60%, Idle ~30%, so the filter and reduce
/// scenarios land on both the few-rows and many-rows points. `sensor` and
/// `author` are the reduce group keys; `blob` is a payload column no
/// reduce scenario reads.
pub fn bench() {
    let data = data(500_000);
    let tick_exp = data.iter().filter(|e| matches!(e, Event::Tick(_))).count();
    let note_exp = data.iter().filter(|e| matches!(e, Event::Note(_))).count();
    let inc_exp = tick_exp + note_exp;
    println!("## Enums\n");

    // ---- PcoPack benchmarks ----
    let (pco_serial_ms, pco_buf, pco_size_bytes, pco_deserial_ms) = if enabled("pco_pack") {
        let pco_serial_ms = min_ms(|| Event::write(data.clone()).unwrap());
        let pco_compressed = Event::write(data.clone()).unwrap();
        let pco_buf = Event::to_bytes(&pco_compressed).unwrap();
        let pco_deserial_ms = min_ms(|| {
            let compressed = Event::from_bytes(&pco_buf).unwrap();
            let results = Event::filter(&compressed, json!({}), &[]).unwrap();
            assert_eq!(results.len(), data.len());
        });
        let pco_size_bytes = pco_buf.len();
        (pco_serial_ms, pco_buf, pco_size_bytes, pco_deserial_ms)
    } else {
        (f64::NAN, Vec::new(), usize::MAX, f64::NAN)
    };

    // ---- Columnar benchmarks (flat-tagged) ----
    let (columnar_serial_ms, columnar_buf, columnar_size_bytes, columnar_deserial_ms) = if enabled("columnar") {
        let columnar_serial_ms = min_ms(|| build_columnar(&data));
        let columnar_buf = build_columnar(&data);
        let columnar_size_bytes = columnar_buf.len();
        let columnar_deserial_ms = min_ms(|| {
            let rows = columnar_rows(&columnar_buf);
            black_box(&rows[12345]);
            assert_eq!(rows.len(), data.len());
        });
        (columnar_serial_ms, columnar_buf, columnar_size_bytes, columnar_deserial_ms)
    } else {
        (f64::NAN, Vec::new(), usize::MAX, f64::NAN)
    };

    // ---- serde_columnar benchmarks (flat-tagged) ----
    let (sc_serial_ms, sc_buf, sc_size_bytes, sc_deserial_ms) = if enabled("serde_columnar") {
        let sc_serial_ms = min_ms(|| build_serde_columnar(&data));
        let sc_buf = build_serde_columnar(&data);
        let sc_size_bytes = sc_buf.len();
        let sc_deserial_ms = min_ms(|| {
            let rows = sc_rows(&sc_buf);
            black_box(&rows[12345].text);
            assert_eq!(rows.len(), data.len());
        });
        (sc_serial_ms, sc_buf, sc_size_bytes, sc_deserial_ms)
    } else {
        (f64::NAN, Vec::new(), usize::MAX, f64::NAN)
    };

    // ---- msgpack benchmarks (single-frame, native tagged enum) ----
    let (map_serial_ms, map_buf, map_size_bytes, map_deserial_ms) = if enabled("msgpack") {
        let map_serial_ms = min_ms(|| serialize_event_map(&data));
        let map_buf = serialize_event_map(&data);
        let map_deserial_ms = min_ms(|| {
            let rows = map_rows(&map_buf);
            assert_eq!(rows.len(), data.len());
        });
        let map_size_bytes = map_buf.len();
        (map_serial_ms, map_buf, map_size_bytes, map_deserial_ms)
    } else {
        (f64::NAN, Vec::new(), usize::MAX, f64::NAN)
    };

    // ---- Vortex benchmarks (flat-tagged) ----
    let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
    let (vortex_session, vortex_buf, vortex_size_bytes) = if enabled("vortex") {
        let (session, buf) = setup_vortex_file(&rt, &data);
        let len = buf.len();
        (session, buf, len)
    } else {
        let session = rt.block_on(async {
            use vortex::VortexSessionDefault as _;
            VortexSession::default()
        });
        (session, ByteBuffer::from(Vec::new()), usize::MAX)
    };
    let (vortex_serial_ms, vortex_deserial_ms, vortex_filter_tick_ms, vortex_filter_note_ms, vortex_filter_inc_ms) =
        if enabled("vortex") {
            let vortex_serial_ms = min_ms(|| {
                let bytes = rt.block_on(async { write_vortex_file(&vortex_session, &data).await });
                black_box(bytes.len())
            });
            let vortex_deserial_ms = min_ms(|| {
                let rows = rt.block_on(async { vortex_read_all(&vortex_session, vortex_buf.clone()).await });
                assert_eq!(rows.len(), data.len());
                black_box(rows.len())
            });
            let vortex_filter_tick_ms = min_ms(|| {
                rt.block_on(async {
                    let batch = vortex_scan(
                        &vortex_session,
                        vortex_buf.clone(),
                        Some(eq(col("kind"), lit(TICK))),
                        Some(&["kind"]),
                    )
                    .await;
                    assert_eq!(batch.num_rows(), tick_exp);
                    black_box(batch)
                })
            });
            let vortex_filter_note_ms = min_ms(|| {
                rt.block_on(async {
                    let batch = vortex_scan(
                        &vortex_session,
                        vortex_buf.clone(),
                        Some(eq(col("kind"), lit(NOTE))),
                        Some(&["kind"]),
                    )
                    .await;
                    assert_eq!(batch.num_rows(), note_exp);
                    black_box(batch)
                })
            });
            let vortex_filter_inc_ms = min_ms(|| {
                rt.block_on(async {
                    let in_expr = or(eq(col("kind"), lit(TICK)), eq(col("kind"), lit(NOTE)));
                    let batch = vortex_scan(&vortex_session, vortex_buf.clone(), Some(in_expr), Some(&["kind"])).await;
                    assert_eq!(batch.num_rows(), inc_exp);
                    black_box(batch)
                })
            });
            (vortex_serial_ms, vortex_deserial_ms, vortex_filter_tick_ms, vortex_filter_note_ms, vortex_filter_inc_ms)
        } else {
            (f64::NAN, f64::NAN, f64::NAN, f64::NAN, f64::NAN)
        };

    #[derive(Debug)]
    struct EnumRow {
        metric: String,
        pco_pack: String,
        vortex: String,
        columnar: String,
        serde_columnar: String,
        msgpack: String,
    }

    impl MarkdownTableRow for EnumRow {
        fn column_names() -> Vec<&'static str> {
            vec!["Metric", "PcoPack", "Vortex", "columnar", "serde_columnar", "msgpack"]
        }

        fn column_values(&self) -> Vec<String> {
            vec![
                self.metric.clone(),
                self.pco_pack.clone(),
                self.vortex.clone(),
                self.columnar.clone(),
                self.serde_columnar.clone(),
                self.msgpack.clone(),
            ]
        }
    }

    // --- Discriminant filters: PcoPack narrows the mask with the
    // discriminant column and never touches the other variants' blocks;
    // the baselines decode every row before testing the kind. ---
    let (pco_filter_tick_ms, pco_filter_note_ms, pco_filter_inc_ms) = if enabled("pco_pack") {
        let pco_filter_tick_ms = min_ms(|| {
            let compressed = Event::from_bytes(&pco_buf).unwrap();
            let results = Event::filter(&compressed, json!({"": 1}), &[]).unwrap();
            assert_eq!(results.len(), tick_exp);
        });
        let pco_filter_note_ms = min_ms(|| {
            let compressed = Event::from_bytes(&pco_buf).unwrap();
            let results = Event::filter(&compressed, json!({"": 2}), &[]).unwrap();
            assert_eq!(results.len(), note_exp);
        });
        let pco_filter_inc_ms = min_ms(|| {
            let compressed = Event::from_bytes(&pco_buf).unwrap();
            let results = Event::filter(&compressed, json!({"": [1, 2]}), &[]).unwrap();
            assert_eq!(results.len(), inc_exp);
        });
        (pco_filter_tick_ms, pco_filter_note_ms, pco_filter_inc_ms)
    } else {
        (f64::NAN, f64::NAN, f64::NAN)
    };

    let col_filter_tick_ms = if enabled("columnar") {
        min_ms(|| {
            let rows = columnar_rows(&columnar_buf);
            assert_eq!(rows.iter().filter(|r| r.kind == TICK).count(), tick_exp);
        })
    } else {
        f64::NAN
    };
    let sc_filter_tick_ms = if enabled("serde_columnar") {
        min_ms(|| {
            let rows = sc_rows(&sc_buf);
            assert_eq!(rows.iter().filter(|r| r.kind == TICK).count(), tick_exp);
        })
    } else {
        f64::NAN
    };
    let map_filter_tick_ms = if enabled("msgpack") {
        min_ms(|| {
            let rows = map_rows(&map_buf);
            assert_eq!(rows.iter().filter(|e| matches!(e, Event::Tick(_))).count(), tick_exp);
        })
    } else {
        f64::NAN
    };

    let col_filter_note_ms = if enabled("columnar") {
        min_ms(|| {
            let rows = columnar_rows(&columnar_buf);
            assert_eq!(rows.iter().filter(|r| r.kind == NOTE).count(), note_exp);
        })
    } else {
        f64::NAN
    };
    let sc_filter_note_ms = if enabled("serde_columnar") {
        min_ms(|| {
            let rows = sc_rows(&sc_buf);
            assert_eq!(rows.iter().filter(|r| r.kind == NOTE).count(), note_exp);
        })
    } else {
        f64::NAN
    };
    let map_filter_note_ms = if enabled("msgpack") {
        min_ms(|| {
            let rows = map_rows(&map_buf);
            assert_eq!(rows.iter().filter(|e| matches!(e, Event::Note(_))).count(), note_exp);
        })
    } else {
        f64::NAN
    };

    let col_filter_inc_ms = if enabled("columnar") {
        min_ms(|| {
            let rows = columnar_rows(&columnar_buf);
            assert_eq!(rows.iter().filter(|r| r.kind == TICK || r.kind == NOTE).count(), inc_exp);
        })
    } else {
        f64::NAN
    };
    let sc_filter_inc_ms = if enabled("serde_columnar") {
        min_ms(|| {
            let rows = sc_rows(&sc_buf);
            assert_eq!(rows.iter().filter(|r| r.kind == TICK || r.kind == NOTE).count(), inc_exp);
        })
    } else {
        f64::NAN
    };
    let map_filter_inc_ms = if enabled("msgpack") {
        min_ms(|| {
            let rows = map_rows(&map_buf);
            assert_eq!(rows.iter().filter(|e| matches!(e, Event::Tick(_) | Event::Note(_))).count(), inc_exp);
        })
    } else {
        f64::NAN
    };

    // --- Reduce: group the survivors of one variant by key and sum the
    // payload. The fold! variant pattern narrows the mask to its declared
    // variant and pops the named fields straight from that variant's
    // payload block; the blob column and the other variants' blocks stay
    // compressed. The baselines decode all flat columns for every row. ---
    let fold_inputs = FoldInputs {
        rt: &rt,
        vortex_session: &vortex_session,
        vortex_buf: &vortex_buf,
        pco: &pco_buf,
        columnar: &columnar_buf,
        serde_columnar: &sc_buf,
        msgpack: &map_buf,
        data: &data,
    };
    let (tick_pco_ms, col_tick_ms, sc_tick_ms, map_tick_ms, vortex_tick_ms) = reduce_tick(&fold_inputs);
    let (note_pco_ms, col_note_ms, sc_note_ms, map_note_ms, vortex_note_ms) = reduce_note(&fold_inputs);

    let pct = |exp: usize| -> String {
        let pct = exp as f64 * 100.0 / data.len() as f64;
        format!("{:.2}", pct).trim_end_matches('0').trim_end_matches('.').to_string()
    };
    let tick_pct = pct(tick_exp);
    let note_pct = pct(note_exp);
    let inc_pct = pct(inc_exp);

    let rows = vec![
        EnumRow {
            metric: "Size".to_string(),
            pco_pack: fmt_sz(pco_size_bytes),
            vortex: fmt_sz(vortex_size_bytes),
            columnar: fmt_sz(columnar_size_bytes),
            serde_columnar: fmt_sz(sc_size_bytes),
            msgpack: fmt_sz(map_size_bytes),
        },
        EnumRow {
            metric: "Serialize".to_string(),
            pco_pack: fmt_gated(pco_serial_ms),
            vortex: fmt_gated(vortex_serial_ms),
            columnar: fmt_gated(columnar_serial_ms),
            serde_columnar: fmt_gated(sc_serial_ms),
            msgpack: fmt_gated(map_serial_ms),
        },
        EnumRow {
            metric: "Deserialize".to_string(),
            pco_pack: fmt_gated(pco_deserial_ms),
            vortex: fmt_gated(vortex_deserial_ms),
            columnar: fmt_gated(columnar_deserial_ms),
            serde_columnar: fmt_gated(sc_deserial_ms),
            msgpack: fmt_gated(map_deserial_ms),
        },
        EnumRow {
            metric: format!("Filter Tick {tick_pct}%"),
            pco_pack: fmt_gated(pco_filter_tick_ms),
            vortex: fmt_gated(vortex_filter_tick_ms),
            columnar: fmt_gated(col_filter_tick_ms),
            serde_columnar: fmt_gated(sc_filter_tick_ms),
            msgpack: fmt_gated(map_filter_tick_ms),
        },
        EnumRow {
            metric: format!("Filter Note {note_pct}%"),
            pco_pack: fmt_gated(pco_filter_note_ms),
            vortex: fmt_gated(vortex_filter_note_ms),
            columnar: fmt_gated(col_filter_note_ms),
            serde_columnar: fmt_gated(sc_filter_note_ms),
            msgpack: fmt_gated(map_filter_note_ms),
        },
        EnumRow {
            metric: format!("Filter Tick or Note {inc_pct}%"),
            pco_pack: fmt_gated(pco_filter_inc_ms),
            vortex: fmt_gated(vortex_filter_inc_ms),
            columnar: fmt_gated(col_filter_inc_ms),
            serde_columnar: fmt_gated(sc_filter_inc_ms),
            msgpack: fmt_gated(map_filter_inc_ms),
        },
        EnumRow {
            metric: format!("Reduce Tick {tick_pct}%"),
            pco_pack: fmt_gated(tick_pco_ms),
            vortex: fmt_gated(vortex_tick_ms),
            columnar: fmt_gated(col_tick_ms),
            serde_columnar: fmt_gated(sc_tick_ms),
            msgpack: fmt_gated(map_tick_ms),
        },
        EnumRow {
            metric: format!("Reduce Note {note_pct}%"),
            pco_pack: fmt_gated(note_pco_ms),
            vortex: fmt_gated(vortex_note_ms),
            columnar: fmt_gated(col_note_ms),
            serde_columnar: fmt_gated(sc_note_ms),
            msgpack: fmt_gated(map_note_ms),
        },
    ];
    println!("{}", as_table(&rows));
    println!();
}

/// One encoded copy of the dataset per codec, plus the runtime and session
/// the Vortex path needs. Both reduce scenarios share a single value of
/// this; only the fold, the group key and the expected row count differ.
struct FoldInputs<'a> {
    rt: &'a tokio::runtime::Runtime,
    vortex_session: &'a VortexSession,
    vortex_buf: &'a ByteBuffer,
    pco: &'a [u8],
    columnar: &'a [u8],
    serde_columnar: &'a [u8],
    msgpack: &'a [u8],
    data: &'a [Event],
}

/// Fold Tick rows only: sensor => (sum value, count). Returns
/// `(pco_pack, columnar, serde_columnar, msgpack, vortex)` milliseconds;
/// `NaN` (a "-" cell) for codecs whose gate is off. The PcoPack fold pops
/// `sensor` and `value` straight from the Tick block and never assembles
/// `Reading`; the baselines decode every flat column for every row first.
fn reduce_tick(inputs: &FoldInputs) -> (f64, f64, f64, f64, f64) {
    let FoldInputs {
        rt,
        vortex_session,
        vortex_buf,
        pco: pco_buf,
        columnar: columnar_buf,
        serde_columnar: sc_buf,
        msgpack: map_buf,
        data,
    } = *inputs;
    let mut truth: std::collections::HashMap<u32, (f64, u64)> = Default::default();
    let mut expected = 0usize;
    for e in data.iter() {
        if let Event::Tick(r) = e {
            expected += 1;
            let entry = truth.entry(r.sensor).or_default();
            entry.0 += r.value;
            entry.1 += 1;
        }
    }

    let pco_ms = if enabled("pco_pack") {
        let (ms, map) = min_ms_pair(|| {
            let mut acc: std::collections::HashMap<u32, (f64, u64)> = Default::default();
            let mut n = 0usize;
            Event::for_each_bytes(
                pco_buf,
                json!({}),
                &[],
                pco_pack::fold!(Event, |Tick(Reading { sensor: u32, value: f64 })| {
                    n += 1;
                    let entry = acc.entry(sensor).or_default();
                    entry.0 += value;
                    entry.1 += 1;
                }),
            )
            .unwrap();
            assert_eq!(n, expected);
            acc
        });
        assert_eq!(map, truth, "projected tick reduce");
        ms
    } else {
        f64::NAN
    };

    let columnar_ms = if enabled("columnar") {
        min_ms(|| {
            let mut acc: std::collections::HashMap<u32, (f64, u64)> = Default::default();
            for r in columnar_rows(columnar_buf).iter().filter(|r| r.kind == TICK) {
                let entry = acc.entry(r.sensor).or_default();
                entry.0 += r.value;
                entry.1 += 1;
            }
            assert_eq!(acc, truth, "columnar tick reduce");
        })
    } else {
        f64::NAN
    };

    let sc_ms = if enabled("serde_columnar") {
        min_ms(|| {
            let mut acc: std::collections::HashMap<u32, (f64, u64)> = Default::default();
            for r in sc_rows(sc_buf).iter().filter(|r| r.kind == TICK) {
                let entry = acc.entry(r.sensor).or_default();
                entry.0 += r.value;
                entry.1 += 1;
            }
            assert_eq!(acc, truth, "serde_columnar tick reduce");
        })
    } else {
        f64::NAN
    };

    let map_ms = if enabled("msgpack") {
        min_ms(|| {
            let mut acc: std::collections::HashMap<u32, (f64, u64)> = Default::default();
            for row in map_rows(map_buf) {
                if let Event::Tick(r) = row {
                    let entry = acc.entry(r.sensor).or_default();
                    entry.0 += r.value;
                    entry.1 += 1;
                }
            }
            assert_eq!(acc, truth, "msgpack tick reduce");
        })
    } else {
        f64::NAN
    };

    let vortex_ms = if enabled("vortex") {
        min_ms(|| {
            rt.block_on(async {
                let batch = vortex_scan(
                    vortex_session,
                    vortex_buf.clone(),
                    Some(eq(col("kind"), lit(TICK))),
                    Some(&["sensor", "value"]),
                )
                .await;
                assert_eq!(batch.num_rows(), expected);
                let rows: Vec<(u32, f64)> = serde_arrow::from_record_batch(&batch).unwrap();
                let mut acc: std::collections::HashMap<u32, (f64, u64)> = Default::default();
                for (sensor, value) in rows.iter() {
                    let entry = acc.entry(*sensor).or_default();
                    entry.0 += *value;
                    entry.1 += 1;
                }
                assert_eq!(acc, truth, "vortex tick reduce");
                black_box(rows)
            })
        })
    } else {
        f64::NAN
    };

    (pco_ms, columnar_ms, sc_ms, map_ms, vortex_ms)
}

/// Fold Note rows only: author => (sum text length, count). Same codec
/// order and full-decode baselines as [`reduce_tick`]; the PcoPack fold
/// pops `author` and `text` from the Note block only.
fn reduce_note(inputs: &FoldInputs) -> (f64, f64, f64, f64, f64) {
    let FoldInputs {
        rt,
        vortex_session,
        vortex_buf,
        pco: pco_buf,
        columnar: columnar_buf,
        serde_columnar: sc_buf,
        msgpack: map_buf,
        data,
    } = *inputs;
    let mut truth: std::collections::HashMap<String, (f64, u64)> = Default::default();
    let mut expected = 0usize;
    for e in data.iter() {
        if let Event::Note(m) = e {
            expected += 1;
            let entry = truth.entry(m.author.clone()).or_default();
            entry.0 += m.text.len() as f64;
            entry.1 += 1;
        }
    }

    let pco_ms = if enabled("pco_pack") {
        let (ms, map) = min_ms_pair(|| {
            let mut acc: std::collections::HashMap<String, (f64, u64)> = Default::default();
            let mut n = 0usize;
            Event::for_each_bytes(
                pco_buf,
                json!({}),
                &[],
                pco_pack::fold!(Event, |Note(Message { author: String, text: String })| {
                    n += 1;
                    let entry = acc.entry(author).or_default();
                    entry.0 += text.len() as f64;
                    entry.1 += 1;
                }),
            )
            .unwrap();
            assert_eq!(n, expected);
            acc
        });
        assert_eq!(map, truth, "projected note reduce");
        ms
    } else {
        f64::NAN
    };

    let columnar_ms = if enabled("columnar") {
        min_ms(|| {
            let mut acc: std::collections::HashMap<String, (f64, u64)> = Default::default();
            for r in columnar_rows(columnar_buf).iter().filter(|r| r.kind == NOTE) {
                let entry = acc.entry(r.author.clone()).or_default();
                entry.0 += r.text.len() as f64;
                entry.1 += 1;
            }
            assert_eq!(acc, truth, "columnar note reduce");
        })
    } else {
        f64::NAN
    };

    let sc_ms = if enabled("serde_columnar") {
        min_ms(|| {
            let mut acc: std::collections::HashMap<String, (f64, u64)> = Default::default();
            for r in sc_rows(sc_buf).iter().filter(|r| r.kind == NOTE) {
                let entry = acc.entry(r.author.clone()).or_default();
                entry.0 += r.text.len() as f64;
                entry.1 += 1;
            }
            assert_eq!(acc, truth, "serde_columnar note reduce");
        })
    } else {
        f64::NAN
    };

    let map_ms = if enabled("msgpack") {
        min_ms(|| {
            let mut acc: std::collections::HashMap<String, (f64, u64)> = Default::default();
            for row in map_rows(map_buf) {
                if let Event::Note(m) = row {
                    let entry = acc.entry(m.author).or_default();
                    entry.0 += m.text.len() as f64;
                    entry.1 += 1;
                }
            }
            assert_eq!(acc, truth, "msgpack note reduce");
        })
    } else {
        f64::NAN
    };

    let vortex_ms = if enabled("vortex") {
        min_ms(|| {
            rt.block_on(async {
                let batch = vortex_scan(
                    vortex_session,
                    vortex_buf.clone(),
                    Some(eq(col("kind"), lit(NOTE))),
                    Some(&["author", "text"]),
                )
                .await;
                assert_eq!(batch.num_rows(), expected);
                let rows: Vec<(String, String)> = serde_arrow::from_record_batch(&batch).unwrap();
                let mut acc: std::collections::HashMap<String, (f64, u64)> = Default::default();
                for (author, text) in rows.iter() {
                    let entry = acc.entry(author.clone()).or_default();
                    entry.0 += text.len() as f64;
                    entry.1 += 1;
                }
                assert_eq!(acc, truth, "vortex note reduce");
                black_box(rows)
            })
        })
    } else {
        f64::NAN
    };

    (pco_ms, columnar_ms, sc_ms, map_ms, vortex_ms)
}

#[derive(Clone, Debug, PartialEq, Default, PcoPack, serde::Serialize, serde::Deserialize)]
struct Reading {
    sensor: u32,
    value: f64,
    blob: serde_json::Value,
}

#[derive(Clone, Debug, PartialEq, Default, PcoPack, serde::Serialize, serde::Deserialize)]
struct Message {
    author: String,
    text: String,
}

/// Top-level tagged union: a unit variant, a struct-payload variant, and a
/// message-payload variant.
#[derive(Clone, Debug, PartialEq, Default, PcoPack, serde::Serialize, serde::Deserialize)]
enum Event {
    #[default]
    Idle,
    Tick(Reading),
    Note(Message),
}

/// The same event stream as a discriminator plus every payload column, the
/// layout `columnar` and (via serde_arrow) Vortex require for rows that are
/// not all the same shape. `kind` carries the same value as the PcoPack
/// discriminant column; the other columns hold defaults for rows whose
/// variant does not own them.
#[derive(Clone, Debug, PartialEq, Default, Columnar, serde::Serialize, serde::Deserialize)]
struct FlatEvent {
    kind: u8,
    sensor: u32,
    value: f64,
    blob: String,
    author: String,
    text: String,
}

impl From<&Event> for FlatEvent {
    fn from(e: &Event) -> Self {
        match e {
            Event::Idle => FlatEvent { kind: 0, ..Default::default() },
            Event::Tick(r) => FlatEvent {
                kind: TICK,
                sensor: r.sensor,
                value: r.value,
                blob: r.blob.to_string(),
                ..Default::default()
            },
            Event::Note(m) => {
                FlatEvent { kind: NOTE, author: m.author.clone(), text: m.text.clone(), ..Default::default() }
            }
        }
    }
}

mod serde_columnar_event {
    use super::*;

    #[columnar(vec, ser, de)]
    #[derive(Debug, Clone, PartialEq)]
    pub struct SerdeColumnarEvent {
        pub kind: u8,
        pub sensor: u32,
        #[columnar(strategy = "Rle")]
        pub value: f64,
        #[columnar(strategy = "Rle")]
        pub blob: String,
        #[columnar(strategy = "Rle")]
        pub author: String,
        #[columnar(strategy = "Rle")]
        pub text: String,
    }

    #[columnar(ser, de)]
    #[derive(Debug, Clone, PartialEq)]
    pub struct SerdeColumnarStore {
        #[columnar(class = "vec")]
        pub data: Vec<SerdeColumnarEvent>,
    }

    impl From<&Event> for SerdeColumnarEvent {
        fn from(e: &Event) -> Self {
            let flat = FlatEvent::from(e);
            SerdeColumnarEvent {
                kind: flat.kind,
                sensor: flat.sensor,
                value: flat.value,
                blob: flat.blob,
                author: flat.author,
                text: flat.text,
            }
        }
    }
}

/// Skewed event mix with realistic payload shapes: Tick carries log-normal
/// readings, Note carries low-cardinality authors with random text, and the
/// `blob` column never leaves compression in the reduce scenarios.
fn data(n: usize) -> Vec<Event> {
    (0..n)
        .map(|i| match i % 10 {
            0 => Event::Tick(Reading {
                sensor: ((i / 10) % 5) as u32, // 5 unique sensor ids
                value: lognorm_small(i),
                blob: serde_json::json!({"never": "read"}),
            }),
            1..=6 => Event::Note(Message {
                author: format!("author_{}", i % 5), // 5 unique authors
                text: format!("note_{}", splitmix64(i.wrapping_add(0xBEEF)) % 1_000_000),
            }),
            _ => Event::Idle,
        })
        .collect()
}

// ---- Codec builders and decoders ----

fn build_columnar(data: &[Event]) -> Vec<u8> {
    let mut container = <FlatEvent as columnar::Columnar>::Container::default();
    for event in data.iter() {
        columnar::Push::push(&mut container, &event.into());
    }
    let mut words = vec![];
    columnar::bytes::indexed::encode(&mut words, &container.borrow());
    let bytes = bytemuck::cast_slice(&words);
    zstd::encode_all(bytes, 3).unwrap()
}

/// Decode the whole columnar buffer into owned rows. The borrow path could
/// filter without materializing, but the fold scenarios need the same owned
/// row shape the other baselines get, and struct benches already pay for
/// owned decompression in their timing.
fn columnar_rows(buf: &[u8]) -> Vec<FlatEvent> {
    let decompressed = zstd::decode_all(buf).unwrap();
    let words: &[u64] = bytemuck::cast_slice(&decompressed);
    type B<'a> = columnar::BorrowedOf<'a, FlatEvent>;
    let decoded = B::from_bytes(&mut columnar::bytes::indexed::decode(words));
    let count = Len::len(&decoded);
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let record = Index::get(&decoded, i);
        out.push(FlatEvent {
            kind: *record.kind,
            sensor: *record.sensor,
            value: *record.value,
            blob: String::from_utf8(record.blob.to_vec()).unwrap(),
            author: String::from_utf8(record.author.to_vec()).unwrap(),
            text: String::from_utf8(record.text.to_vec()).unwrap(),
        });
    }
    out
}

fn build_serde_columnar(data: &[Event]) -> Vec<u8> {
    let store = serde_columnar_event::SerdeColumnarStore { data: data.iter().map(|e| e.into()).collect() };
    let postcard_buf = to_vec(&store).unwrap();
    zstd::encode_all(postcard_buf.as_slice(), 3).unwrap()
}

fn sc_rows(buf: &[u8]) -> Vec<serde_columnar_event::SerdeColumnarEvent> {
    let decompressed = zstd::decode_all(buf).unwrap();
    let decoded: serde_columnar_event::SerdeColumnarStore = from_bytes(&decompressed).unwrap();
    decoded.data
}

fn serialize_event_map(data: &[Event]) -> Vec<u8> {
    let mut msgpack_buf = Vec::new();
    data.serialize(&mut rmp_serde::Serializer::new(&mut msgpack_buf).with_struct_map()).unwrap();

    zstd::encode_all(msgpack_buf.as_slice(), 3).unwrap()
}

fn map_rows(buf: &[u8]) -> Vec<Event> {
    let decompressed = zstd::decode_all(buf).unwrap();
    rmp_serde::from_slice(&decompressed).unwrap()
}

// ---- Vortex helpers ----

/// Construct the session inside the Tokio runtime and write one compressed vortex file.
fn setup_vortex_file(rt: &tokio::runtime::Runtime, data: &[Event]) -> (VortexSession, ByteBuffer) {
    rt.block_on(async {
        use vortex::VortexSessionDefault as _;
        let session = VortexSession::default();
        let bytes = write_vortex_file(&session, data).await;
        (session, ByteBuffer::from(bytes))
    })
}

/// Serialize the flat-tagged rows to Arrow via serde_arrow, then write
/// vortex bytes with the compact strategy.
async fn write_vortex_file(session: &VortexSession, data: &[Event]) -> Vec<u8> {
    let fields = <Vec<FieldRef>>::from_type::<FlatEvent>(TracingOptions::default()).unwrap();
    let flat: Vec<FlatEvent> = data.iter().map(FlatEvent::from).collect();
    let batch = serde_arrow::to_record_batch(&fields, &flat).unwrap();
    let schema_in = batch.schema();
    let array = session.arrow().from_arrow_record_batch(batch, &schema_in).unwrap();
    let mut bytes: Vec<u8> = vec![];
    let strategy = WriteStrategyBuilder::from_session(session)
        .with_btrblocks_builder(BtrBlocksCompressorBuilder::from_session(session).with_compact())
        .build();
    session.write_options().with_strategy(strategy).write(&mut bytes, array.to_array_stream()).await.unwrap();
    bytes
}

/// Open the file and scan all rows, materializing through Arrow back into Rust records.
async fn vortex_read_all(session: &VortexSession, buffer: ByteBuffer) -> Vec<FlatEvent> {
    let batch = vortex_scan(session, buffer, None, None).await;
    serde_arrow::from_record_batch(&batch).unwrap()
}

/// Open the file and run an (optionally filtered, optionally projected) scan,
/// materializing an Arrow RecordBatch of matches.
async fn vortex_scan(
    session: &VortexSession, buffer: ByteBuffer, filter: Option<vortex::array::expr::Expression>,
    projection: Option<&[&'static str]>,
) -> arrow_array::RecordBatch {
    let file = session.open_options().open_buffer(buffer).unwrap();
    let mut scan = file.scan().unwrap();
    if let Some(expr) = filter {
        let bound = expr.bind(file.dtype()).unwrap().optimize_recursive().unwrap();
        scan = scan.with_filter(bound);
    }
    if let Some(fields) = projection {
        let bound = vortex::array::expr::select(fields, vortex::array::expr::root())
            .bind(file.dtype())
            .unwrap()
            .optimize_recursive()
            .unwrap();
        scan = scan.with_projection(bound);
    }
    // Materialize the compressed scan result through Arrow (the session's stable export path).
    let array = scan.into_array_stream().unwrap().read_all().await.unwrap();
    let schema_out = session.arrow().to_arrow_schema(array.dtype()).unwrap();
    materialize_vortex_batch(session, array, &schema_out)
}

/// Materialize a vortex array into an Arrow RecordBatch via the session's executor.
#[allow(deprecated)] // execute_record_batch is deprecated in favor of the new Arrow exporter but still stable.
fn materialize_vortex_batch(
    session: &VortexSession, array: vortex::array::ArrayRef, schema: &arrow_schema::Schema,
) -> arrow_array::RecordBatch {
    use vortex::array::VortexSessionExecute;
    use vortex::arrow::ArrowArrayExecutor as _;
    let mut ctx = session.create_execution_ctx();
    array.execute_record_batch(schema, &mut ctx).unwrap()
}
