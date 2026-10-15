//! air_quality: NYC AirData single-sensor stream (16 cols, 823,743 rows,
//! ~2.3 s cadence, 2022). `DateTime` is ns-precision; PcoPack stores us,
//! so default writes truncate. `time_round = 1 s` gives the only real
//! default-vs-lossy pair; default chunk size.

use chrono::{DateTime, Duration, Utc};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use pco_pack::PcoPack;

use super::common::{self, CodecResults, Ds};

pub const DS: Ds = Ds {
    name: "devinrsmith-air-quality.20220714.zstd.parquet",
    url: "https://pcodec-public.s3.amazonaws.com/devinrsmith-air-quality.20220714.zstd.parquet",
    size: 15_661_377,
    md5: Some("ed6699da78748bcfe9a418a1554597b6"),
};

#[derive(Clone, Debug, PartialEq, PcoPack, serde::Serialize, serde::Deserialize)]
#[pco_pack(timestamp = timestamp)]
pub struct AirQuality {
    pub kafka_partition: i32,
    pub kafka_offset: i64, // monotonic counter -> pco delta
    pub kafka_timestamp: DateTime<Utc>,
    pub timestamp: DateTime<Utc>, // device clock, ns precision
    pub pm_1: i16,
    pub pm_2_5: i16,
    pub pm_10: i16,
    pub pm_1_atmosphere: i16,
    pub pm_2_5_atmosphere: i16,
    pub pm_10_atmosphere: i16,
    pub p_0_3: i16,
    pub p_0_5: i16,
    pub p_1_0: i16,
    pub p_2_5: i16,
    pub p_5_0: i16,
    pub p_10: i16,
}

impl AirQuality {
    /// The row a default (lossless) PcoPack write stores: both `DateTime`
    /// fields floored to us.
    fn floored(&self) -> AirQuality {
        AirQuality {
            kafka_timestamp: common::us_floor(&self.kafka_timestamp),
            timestamp: common::us_floor(&self.timestamp),
            ..self.clone()
        }
    }
}

pub fn bench() {
    let data = load();
    let n = data.len();

    // Two windows from the min timestamp (1 h, 24 h); expected counts use
    // each codec's stored precision (us truncation, 1 s rounding, ns, exact).
    let tmin = data.iter().map(|r| &r.timestamp).cloned().min().unwrap();
    let windows: [(Duration, &str); 2] = [(Duration::hours(1), "1 h"), (Duration::hours(24), "24 h")];
    let us_ranges: Vec<(i64, i64)> = windows
        .iter()
        .map(|(w, _)| {
            let tmax = tmin + *w;
            (tmin.timestamp_micros(), tmax.timestamp_micros())
        })
        .collect();
    let ns_ranges: Vec<(i64, i64)> = windows
        .iter()
        .map(|(w, _)| {
            let tmax = tmin + *w;
            (tmin.timestamp_nanos_opt().unwrap(), tmax.timestamp_nanos_opt().unwrap())
        })
        .collect();
    let exp_pco: Vec<usize> = us_ranges
        .iter()
        .map(|(a, b)| data.iter().filter(|r| (*a..=*b).contains(&r.timestamp.timestamp_micros())).count())
        .collect();
    let exp_pcof: Vec<usize> = us_ranges
        .iter()
        .map(|(a, b)| {
            data.iter()
                .filter(|r| (*a..=*b).contains(&common::ts_rounded(&r.timestamp, 1_000_000).timestamp_micros()))
                .count()
        })
        .collect();
    let exp_ns: Vec<usize> = ns_ranges
        .iter()
        .map(|(a, b)| data.iter().filter(|r| (*a..=*b).contains(&r.timestamp.timestamp_nanos_opt().unwrap())).count())
        .collect();
    let labels: Vec<String> = exp_pco.iter().map(|&e| common::filter_label(e, n)).collect();
    let label_refs: Vec<&str> = labels.iter().map(String::as_str).collect();

    println!("### air_quality ({n} rows)\n");

    let queries: Vec<serde_json::Value> =
        us_ranges.iter().map(|(a, b)| serde_json::json!({ "timestamp": { "start": a, "end": b } })).collect();
    let pco_filters: Vec<(&serde_json::Value, usize)> =
        queries.iter().zip(exp_pco.iter()).map(|(q, e)| (q, *e)).collect();
    let pcof_filters: Vec<(&serde_json::Value, usize)> =
        queries.iter().zip(exp_pcof.iter()).map(|(q, e)| (q, *e)).collect();

    let pco = common::bench_pco_pack(data.to_vec(), &pco_filters, |back| {
        verify(data.iter().map(|r| r.floored()).collect(), back);
    });

    let dataf: Vec<AqF> = data.iter().map(AqF::from).collect();
    let pcof = common::bench_pco_pack(dataf, &pcof_filters, |back| {
        let mut a: Vec<AqF> = data.iter().map(AqF::rounded).collect();
        let mut b = back.to_vec();
        a.sort_by_key(|r| r.kafka_offset);
        b.sort_by_key(|r| r.kafka_offset);
        assert_eq!(a, b, "air_quality lossy roundtrip mismatch");
    });

    // Mirrors store ns, msgpack stores exact instants: one (i64, i64) ns
    // range serves all four codecs.
    let ns_filters: Vec<((i64, i64), usize)> = ns_ranges.iter().zip(exp_ns.iter()).map(|(r, e)| (*r, *e)).collect();

    let col = common::bench_columnar(
        n,
        || data.iter().map(AqCol::from).collect(),
        &ns_filters,
        |decoded: &columnar::BorrowedOf<AqCol>, k: &(i64, i64)| {
            let count = columnar::Len::len(decoded);
            (0..count)
                .filter(|&i| {
                    let r = columnar::Index::get(decoded, i);
                    k.0 <= *r.timestamp && *r.timestamp <= k.1
                })
                .count()
        },
    );

    let sc = common::bench_serde_columnar(
        n,
        || aq_sc::AqScStore { data: data.iter().map(aq_sc::AqScRow::from).collect() },
        &ns_filters,
        |s: &aq_sc::AqScStore, k: &(i64, i64)| {
            s.data.iter().filter(|r| k.0 <= r.timestamp && r.timestamp <= k.1).count()
        },
    );

    let mp = common::bench_msgpack(&data, &ns_filters, |r: &AirQuality, k: &(i64, i64)| {
        let ns = r.timestamp.timestamp_nanos_opt().unwrap();
        k.0 <= ns && ns <= k.1
    });

    // Vortex uses the i64-ns mirror + numeric ranges; chrono strings vary in
    // fractional digits, so lexicographic range != temporal range.
    let data_vx: Vec<AqVx> = data.iter().map(AqVx::from).collect();
    let vx = common::bench_vortex_ds(&data_vx, &ns_filters, |k: &(i64, i64)| {
        vortex::array::expr::and(
            vortex::array::expr::gt_eq(vortex::array::expr::col("timestamp"), vortex::array::expr::lit(k.0)),
            vortex::array::expr::lt_eq(vortex::array::expr::col("timestamp"), vortex::array::expr::lit(k.1)),
        )
    });

    CodecResults {
        pco_pack: pco,
        pco_pack_lossy: Some(pcof),
        columnar: col,
        serde_columnar: sc,
        msgpack: mp,
        vortex: vx,
    }
    .print(&label_refs);
}

/// Lossy variant: `time_round = 1 s` drops the sub-second noise of the two
/// ns-precision clocks.
#[derive(Clone, Debug, PartialEq, PcoPack)]
#[pco_pack(timestamp = timestamp, time_round = chrono::Duration::seconds(1))]
pub struct AqF {
    pub kafka_partition: i32,
    pub kafka_offset: i64,
    pub kafka_timestamp: DateTime<Utc>,
    pub timestamp: DateTime<Utc>,
    pub pm_1: i16,
    pub pm_2_5: i16,
    pub pm_10: i16,
    pub pm_1_atmosphere: i16,
    pub pm_2_5_atmosphere: i16,
    pub pm_10_atmosphere: i16,
    pub p_0_3: i16,
    pub p_0_5: i16,
    pub p_1_0: i16,
    pub p_2_5: i16,
    pub p_5_0: i16,
    pub p_10: i16,
}

impl From<&AirQuality> for AqF {
    fn from(r: &AirQuality) -> Self {
        AqF {
            kafka_partition: r.kafka_partition,
            kafka_offset: r.kafka_offset,
            kafka_timestamp: r.kafka_timestamp,
            timestamp: r.timestamp,
            pm_1: r.pm_1,
            pm_2_5: r.pm_2_5,
            pm_10: r.pm_10,
            pm_1_atmosphere: r.pm_1_atmosphere,
            pm_2_5_atmosphere: r.pm_2_5_atmosphere,
            pm_10_atmosphere: r.pm_10_atmosphere,
            p_0_3: r.p_0_3,
            p_0_5: r.p_0_5,
            p_1_0: r.p_1_0,
            p_2_5: r.p_2_5,
            p_5_0: r.p_5_0,
            p_10: r.p_10,
        }
    }
}

impl AqF {
    /// The row a `time_round = 1 s` write stores.
    fn rounded(r: &AirQuality) -> Self {
        AqF {
            kafka_timestamp: common::ts_rounded(&r.kafka_timestamp, 1_000_000),
            timestamp: common::ts_rounded(&r.timestamp, 1_000_000),
            ..AqF::from(r)
        }
    }
}

// ---- columnar mirror: DateTime -> i64 ns (no chrono in the crate) ----

#[derive(Clone, Debug, PartialEq, columnar::Columnar, serde::Serialize, serde::Deserialize)]
pub struct AqCol {
    pub kafka_partition: i32,
    pub kafka_offset: i64,
    pub kafka_timestamp: i64,
    pub timestamp: i64,
    pub pm_1: i16,
    pub pm_2_5: i16,
    pub pm_10: i16,
    pub pm_1_atmosphere: i16,
    pub pm_2_5_atmosphere: i16,
    pub pm_10_atmosphere: i16,
    pub p_0_3: i16,
    pub p_0_5: i16,
    pub p_1_0: i16,
    pub p_2_5: i16,
    pub p_5_0: i16,
    pub p_10: i16,
}

impl From<&AirQuality> for AqCol {
    fn from(r: &AirQuality) -> Self {
        AqCol {
            kafka_partition: r.kafka_partition,
            kafka_offset: r.kafka_offset,
            kafka_timestamp: r.kafka_timestamp.timestamp_nanos_opt().unwrap(),
            timestamp: r.timestamp.timestamp_nanos_opt().unwrap(),
            pm_1: r.pm_1,
            pm_2_5: r.pm_2_5,
            pm_10: r.pm_10,
            pm_1_atmosphere: r.pm_1_atmosphere,
            pm_2_5_atmosphere: r.pm_2_5_atmosphere,
            pm_10_atmosphere: r.pm_10_atmosphere,
            p_0_3: r.p_0_3,
            p_0_5: r.p_0_5,
            p_1_0: r.p_1_0,
            p_2_5: r.p_2_5,
            p_5_0: r.p_5_0,
            p_10: r.p_10,
        }
    }
}

// ---- vortex mirror: DateTime -> i64 ns (same shape as AqCol) ----

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AqVx {
    pub kafka_partition: i32,
    pub kafka_offset: i64,
    pub kafka_timestamp: i64,
    pub timestamp: i64,
    pub pm_1: i16,
    pub pm_2_5: i16,
    pub pm_10: i16,
    pub pm_1_atmosphere: i16,
    pub pm_2_5_atmosphere: i16,
    pub pm_10_atmosphere: i16,
    pub p_0_3: i16,
    pub p_0_5: i16,
    pub p_1_0: i16,
    pub p_2_5: i16,
    pub p_5_0: i16,
    pub p_10: i16,
}

impl From<&AirQuality> for AqVx {
    fn from(r: &AirQuality) -> Self {
        AqVx {
            kafka_partition: r.kafka_partition,
            kafka_offset: r.kafka_offset,
            kafka_timestamp: r.kafka_timestamp.timestamp_nanos_opt().unwrap(),
            timestamp: r.timestamp.timestamp_nanos_opt().unwrap(),
            pm_1: r.pm_1,
            pm_2_5: r.pm_2_5,
            pm_10: r.pm_10,
            pm_1_atmosphere: r.pm_1_atmosphere,
            pm_2_5_atmosphere: r.pm_2_5_atmosphere,
            pm_10_atmosphere: r.pm_10_atmosphere,
            p_0_3: r.p_0_3,
            p_0_5: r.p_0_5,
            p_1_0: r.p_1_0,
            p_2_5: r.p_2_5,
            p_5_0: r.p_5_0,
            p_10: r.p_10,
        }
    }
}

// ---- serde_columnar mirror: ints -> DeltaRle, timestamps as i64 ns ----

mod aq_sc {
    use super::*;
    use serde_columnar::columnar;

    #[columnar(vec, ser, de)]
    #[derive(Debug, Clone, PartialEq)]
    pub struct AqScRow {
        #[columnar(strategy = "DeltaRle")]
        pub kafka_partition: i32,
        #[columnar(strategy = "DeltaRle")]
        pub kafka_offset: i64,
        #[columnar(strategy = "DeltaRle")]
        pub kafka_timestamp: i64,
        #[columnar(strategy = "DeltaRle")]
        pub timestamp: i64,
        #[columnar(strategy = "DeltaRle")]
        pub pm_1: i16,
        #[columnar(strategy = "DeltaRle")]
        pub pm_2_5: i16,
        #[columnar(strategy = "DeltaRle")]
        pub pm_10: i16,
        #[columnar(strategy = "DeltaRle")]
        pub pm_1_atmosphere: i16,
        #[columnar(strategy = "DeltaRle")]
        pub pm_2_5_atmosphere: i16,
        #[columnar(strategy = "DeltaRle")]
        pub pm_10_atmosphere: i16,
        #[columnar(strategy = "DeltaRle")]
        pub p_0_3: i16,
        #[columnar(strategy = "DeltaRle")]
        pub p_0_5: i16,
        #[columnar(strategy = "DeltaRle")]
        pub p_1_0: i16,
        #[columnar(strategy = "DeltaRle")]
        pub p_2_5: i16,
        #[columnar(strategy = "DeltaRle")]
        pub p_5_0: i16,
        #[columnar(strategy = "DeltaRle")]
        pub p_10: i16,
    }

    #[columnar(ser, de)]
    #[derive(Debug, Clone, PartialEq)]
    pub struct AqScStore {
        #[columnar(class = "vec")]
        pub data: Vec<AqScRow>,
    }

    impl From<&AirQuality> for AqScRow {
        fn from(r: &AirQuality) -> Self {
            AqScRow {
                kafka_partition: r.kafka_partition,
                kafka_offset: r.kafka_offset,
                kafka_timestamp: r.kafka_timestamp.timestamp_nanos_opt().unwrap(),
                timestamp: r.timestamp.timestamp_nanos_opt().unwrap(),
                pm_1: r.pm_1,
                pm_2_5: r.pm_2_5,
                pm_10: r.pm_10,
                pm_1_atmosphere: r.pm_1_atmosphere,
                pm_2_5_atmosphere: r.pm_2_5_atmosphere,
                pm_10_atmosphere: r.pm_10_atmosphere,
                p_0_3: r.p_0_3,
                p_0_5: r.p_0_5,
                p_1_0: r.p_1_0,
                p_2_5: r.p_2_5,
                p_5_0: r.p_5_0,
                p_10: r.p_10,
            }
        }
    }
}

impl common::ScStore for aq_sc::AqScStore {
    fn row_count(&self) -> usize {
        self.data.len()
    }
}

// ---- load ----

fn col16<'a>(batch: &'a arrow_array::RecordBatch, name: &str) -> &'a arrow_array::Int16Array {
    batch
        .column_by_name(name)
        .unwrap()
        .as_any()
        .downcast_ref::<arrow_array::Int16Array>()
        .unwrap_or_else(|| panic!("column {name} is not Int16"))
}

fn col32<'a>(batch: &'a arrow_array::RecordBatch, name: &str) -> &'a arrow_array::Int32Array {
    batch
        .column_by_name(name)
        .unwrap()
        .as_any()
        .downcast_ref::<arrow_array::Int32Array>()
        .unwrap_or_else(|| panic!("column {name} is not Int32"))
}

fn col64<'a>(batch: &'a arrow_array::RecordBatch, name: &str) -> &'a arrow_array::Int64Array {
    batch
        .column_by_name(name)
        .unwrap()
        .as_any()
        .downcast_ref::<arrow_array::Int64Array>()
        .unwrap_or_else(|| panic!("column {name} is not Int64"))
}

fn colns<'a>(batch: &'a arrow_array::RecordBatch, name: &str) -> &'a arrow_array::TimestampNanosecondArray {
    batch
        .column_by_name(name)
        .unwrap()
        .as_any()
        .downcast_ref::<arrow_array::TimestampNanosecondArray>()
        .unwrap_or_else(|| panic!("column {name} is not TimestampNanosecond"))
}

fn load() -> Vec<AirQuality> {
    let path = DS.ensure_downloaded();
    let file = std::fs::File::open(&path).unwrap();
    let builder = ParquetRecordBatchReaderBuilder::try_new(file).unwrap();
    let mut reader = builder.build().unwrap();
    let mut rows = Vec::new();
    while let Some(batch) = reader.next() {
        let batch = batch.unwrap();
        let partition = col32(&batch, "KafkaPartition");
        let offset = col64(&batch, "KafkaOffset");
        let kts = colns(&batch, "KafkaTimestamp");
        let ts = colns(&batch, "timestamp");
        let pm_1 = col16(&batch, "pm_1");
        let pm_2_5 = col16(&batch, "pm_2_5");
        let pm_10 = col16(&batch, "pm_10");
        let pm_1_atmosphere = col16(&batch, "pm_1_atmosphere");
        let pm_2_5_atmosphere = col16(&batch, "pm_2_5_atmosphere");
        let pm_10_atmosphere = col16(&batch, "pm_10_atmosphere");
        let p_0_3 = col16(&batch, "p_0_3");
        let p_0_5 = col16(&batch, "p_0_5");
        let p_1_0 = col16(&batch, "p_1_0");
        let p_2_5 = col16(&batch, "p_2_5");
        let p_5_0 = col16(&batch, "p_5_0");
        let p_10 = col16(&batch, "p_10");
        for i in 0..batch.num_rows() {
            rows.push(AirQuality {
                kafka_partition: partition.value(i),
                kafka_offset: offset.value(i),
                kafka_timestamp: DateTime::from_timestamp_nanos(kts.value(i)),
                timestamp: DateTime::from_timestamp_nanos(ts.value(i)),
                pm_1: pm_1.value(i),
                pm_2_5: pm_2_5.value(i),
                pm_10: pm_10.value(i),
                pm_1_atmosphere: pm_1_atmosphere.value(i),
                pm_2_5_atmosphere: pm_2_5_atmosphere.value(i),
                pm_10_atmosphere: pm_10_atmosphere.value(i),
                p_0_3: p_0_3.value(i),
                p_0_5: p_0_5.value(i),
                p_1_0: p_1_0.value(i),
                p_2_5: p_2_5.value(i),
                p_5_0: p_5_0.value(i),
                p_10: p_10.value(i),
            });
        }
    }
    rows
}

/// PcoPack returns rows in stored order (sorted by `timestamp`); compare
/// as multisets keyed by the monotonic `kafka_offset`.
fn verify(a: Vec<AirQuality>, b: &[AirQuality]) {
    let mut a = a;
    let mut b = b.to_vec();
    a.sort_by_key(|r| r.kafka_offset);
    b.sort_by_key(|r| r.kafka_offset);
    assert_eq!(a, b, "air_quality roundtrip mismatch");
}
