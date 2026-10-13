//! taxi: NYC TLC FHV trips, April 2023. 19.1M rows x 24 cols in one 492 MB
//! row group (the pcodec "taxi" file). The 9 f64 cols carry <= 3 decimals, so
//! `float_round = 3` is exactly lossless (asserted at load). `Y`/`N`/blank
//! flags are `Option<bool>` in every codec: null bitmap + 1-bit values. All
//! 4 timestamps are integer seconds (asserted at load).

use arrow_array::Array as _;
use chrono::{DateTime, Duration, Utc};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use pco_pack::PcoPack;
use smol_str::SmolStr;

use super::common::{self, CodecResults, Ds};

pub const DS: Ds = Ds {
    name: "fhvhv_tripdata_2023-04.parquet",
    url: "https://d37ci6vzurychx.cloudfront.net/trip-data/fhvhv_tripdata_2023-04.parquet",
    size: 491_826_475,
    md5: Some("7233505d557e0c4f7a2e64ba4bfe5c2d"),
};

#[derive(Clone, Debug, PartialEq, PcoPack, serde::Serialize, serde::Deserialize)]
#[pco_pack(timestamp = pickup_datetime)]
pub struct Trip {
    pub license_num: SmolStr,                  // 2 distinct values, <= 6 chars
    pub dispatching_base_num: SmolStr,         // 15 distinct
    pub originating_base_num: Option<SmolStr>, // 19 distinct; null when non-dispatched
    pub request_datetime: DateTime<Utc>,
    pub on_scene_datetime: Option<DateTime<Utc>>, // null when non-dispatched
    pub pickup_datetime: DateTime<Utc>,           // the `timestamp` field
    pub dropoff_datetime: DateTime<Utc>,
    pub pu_location_id: i32,      // 262 taxi zones
    pub do_location_id: i32,      // 261 taxi zones
    pub trip_miles: f64,          // <= 3 fractional digits
    pub trip_time: i64,           // seconds
    pub base_passenger_fare: f64, // <= 2 fractional digits
    pub tolls: f64,
    pub bcf: f64,
    pub sales_tax: f64,
    pub congestion_surcharge: f64,
    pub airport_fee: f64,
    pub tips: f64,
    pub driver_pay: f64,
    pub shared_request_flag: Option<bool>, // "Y"/"N"/" " -> true/false/None
    pub shared_match_flag: Option<bool>,
    pub access_a_ride_flag: Option<bool>,
    pub wav_request_flag: Option<bool>,
    pub wav_match_flag: Option<bool>,
}

pub fn bench() {
    let data = load();
    let n = data.len();

    // Two windows from the min pickup time (1 h, 24 h). All codecs store the
    // same precision (integer seconds), so one expected count serves them all.
    let tmin = data.iter().map(|r| &r.pickup_datetime).cloned().min().unwrap();
    let windows: [(Duration, &str); 2] = [(Duration::hours(1), "1 h"), (Duration::hours(24), "24 h")];
    let us_ranges: Vec<(i64, i64)> = windows
        .iter()
        .map(|(w, _)| {
            let tmax = tmin + *w;
            (tmin.timestamp_micros(), tmax.timestamp_micros())
        })
        .collect();
    let str_ranges: Vec<(String, String)> = windows
        .iter()
        .map(|(w, _)| {
            let tmax = tmin + *w;
            (tmin.format("%Y-%m-%dT%H:%M:%SZ").to_string(), tmax.format("%Y-%m-%dT%H:%M:%SZ").to_string())
        })
        .collect();
    let exp: Vec<usize> = us_ranges
        .iter()
        .map(|(a, b)| data.iter().filter(|r| (*a..=*b).contains(&r.pickup_datetime.timestamp_micros())).count())
        .collect();
    let labels: Vec<String> = exp.iter().map(|&e| common::filter_label(e, n)).collect();
    let label_refs: Vec<&str> = labels.iter().map(String::as_str).collect();

    println!("### taxi ({n} rows)\n");

    let queries: Vec<serde_json::Value> =
        us_ranges.iter().map(|(a, b)| serde_json::json!({ "pickup_datetime": { "start": a, "end": b } })).collect();
    let pco_filters: Vec<(&serde_json::Value, usize)> = queries.iter().zip(exp.iter()).map(|(q, e)| (q, *e)).collect();

    let pco = common::bench_pco_pack(&data, &pco_filters, |back| verify_identity(&data, back));

    let dataf: Vec<TripF> = data.iter().map(TripF::from).collect();
    let pcof = common::bench_pco_pack(&dataf, &pco_filters, |back| verify_lossy(&data, back));
    drop(dataf);

    let us_filters: Vec<((i64, i64), usize)> = us_ranges.iter().zip(exp.iter()).map(|(r, e)| (*r, *e)).collect();

    let col = common::bench_columnar(
        n,
        || data.iter().map(TripCol::from).collect(),
        &us_filters,
        |decoded: &columnar::BorrowedOf<TripCol>, k: &(i64, i64)| {
            let count = columnar::Len::len(decoded);
            (0..count)
                .filter(|&i| {
                    let r = columnar::Index::get(decoded, i);
                    k.0 <= *r.pickup_datetime && *r.pickup_datetime <= k.1
                })
                .count()
        },
    );

    let sc = common::bench_serde_columnar(
        n,
        || taxi_sc::TripScStore { data: data.iter().map(taxi_sc::TripScRow::from).collect() },
        &us_filters,
        |s: &taxi_sc::TripScStore, k: &(i64, i64)| {
            s.data.iter().filter(|r| k.0 <= r.pickup_datetime && r.pickup_datetime <= k.1).count()
        },
    );

    let mp = common::bench_msgpack(&data, &us_filters, |r: &Trip, k: &(i64, i64)| {
        let us = r.pickup_datetime.timestamp_micros();
        k.0 <= us && us <= k.1
    });

    // Vortex consumes Trip directly: no enums, flags are nullable bools;
    // `bench_vortex_ds`'s `from_samples` covers SmolStr/chrono.
    let vx = common::bench_vortex_ds(
        &data,
        &str_ranges.iter().zip(exp.iter()).map(|(r, e)| (r.clone(), *e)).collect::<Vec<_>>(),
        |k: &(String, String)| {
            vortex::array::expr::and(
                vortex::array::expr::gt_eq(
                    vortex::array::expr::col("pickup_datetime"),
                    vortex::array::expr::lit(k.0.as_str()),
                ),
                vortex::array::expr::lt_eq(
                    vortex::array::expr::col("pickup_datetime"),
                    vortex::array::expr::lit(k.1.as_str()),
                ),
            )
        },
    );

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

/// Lossy variant: `float_round = 3`, exactly lossless for this file (see
/// module docs).
#[derive(Clone, Debug, PartialEq, PcoPack)]
#[pco_pack(timestamp = pickup_datetime, float_round = 3)]
pub struct TripF {
    pub license_num: SmolStr,
    pub dispatching_base_num: SmolStr,
    pub originating_base_num: Option<SmolStr>,
    pub request_datetime: DateTime<Utc>,
    pub on_scene_datetime: Option<DateTime<Utc>>,
    pub pickup_datetime: DateTime<Utc>,
    pub dropoff_datetime: DateTime<Utc>,
    pub pu_location_id: i32,
    pub do_location_id: i32,
    pub trip_miles: f64,
    pub trip_time: i64,
    pub base_passenger_fare: f64,
    pub tolls: f64,
    pub bcf: f64,
    pub sales_tax: f64,
    pub congestion_surcharge: f64,
    pub airport_fee: f64,
    pub tips: f64,
    pub driver_pay: f64,
    pub shared_request_flag: Option<bool>,
    pub shared_match_flag: Option<bool>,
    pub access_a_ride_flag: Option<bool>,
    pub wav_request_flag: Option<bool>,
    pub wav_match_flag: Option<bool>,
}

impl From<&Trip> for TripF {
    fn from(r: &Trip) -> Self {
        TripF {
            license_num: r.license_num.clone(),
            dispatching_base_num: r.dispatching_base_num.clone(),
            originating_base_num: r.originating_base_num.clone(),
            request_datetime: r.request_datetime,
            on_scene_datetime: r.on_scene_datetime,
            pickup_datetime: r.pickup_datetime,
            dropoff_datetime: r.dropoff_datetime,
            pu_location_id: r.pu_location_id,
            do_location_id: r.do_location_id,
            trip_miles: r.trip_miles,
            trip_time: r.trip_time,
            base_passenger_fare: r.base_passenger_fare,
            tolls: r.tolls,
            bcf: r.bcf,
            sales_tax: r.sales_tax,
            congestion_surcharge: r.congestion_surcharge,
            airport_fee: r.airport_fee,
            tips: r.tips,
            driver_pay: r.driver_pay,
            shared_request_flag: r.shared_request_flag,
            shared_match_flag: r.shared_match_flag,
            access_a_ride_flag: r.access_a_ride_flag,
            wav_request_flag: r.wav_request_flag,
            wav_match_flag: r.wav_match_flag,
        }
    }
}

// ---- columnar mirror: SmolStr -> String (no SmolStr impl), DateTime -> i64
// us; Option<bool> flags stay (columnar has a native Option impl)

#[derive(Clone, Debug, PartialEq, columnar::Columnar, serde::Serialize, serde::Deserialize)]
pub struct TripCol {
    pub license_num: String,
    pub dispatching_base_num: String,
    pub originating_base_num: Option<String>,
    pub request_datetime: i64,
    pub on_scene_datetime: Option<i64>,
    pub pickup_datetime: i64,
    pub dropoff_datetime: i64,
    pub pu_location_id: i32,
    pub do_location_id: i32,
    pub trip_miles: f64,
    pub trip_time: i64,
    pub base_passenger_fare: f64,
    pub tolls: f64,
    pub bcf: f64,
    pub sales_tax: f64,
    pub congestion_surcharge: f64,
    pub airport_fee: f64,
    pub tips: f64,
    pub driver_pay: f64,
    pub shared_request_flag: Option<bool>,
    pub shared_match_flag: Option<bool>,
    pub access_a_ride_flag: Option<bool>,
    pub wav_request_flag: Option<bool>,
    pub wav_match_flag: Option<bool>,
}

impl From<&Trip> for TripCol {
    fn from(r: &Trip) -> Self {
        TripCol {
            license_num: r.license_num.to_string(),
            dispatching_base_num: r.dispatching_base_num.to_string(),
            originating_base_num: r.originating_base_num.as_deref().map(str::to_string),
            request_datetime: r.request_datetime.timestamp_micros(),
            on_scene_datetime: r.on_scene_datetime.map(|d| d.timestamp_micros()),
            pickup_datetime: r.pickup_datetime.timestamp_micros(),
            dropoff_datetime: r.dropoff_datetime.timestamp_micros(),
            pu_location_id: r.pu_location_id,
            do_location_id: r.do_location_id,
            trip_miles: r.trip_miles,
            trip_time: r.trip_time,
            base_passenger_fare: r.base_passenger_fare,
            tolls: r.tolls,
            bcf: r.bcf,
            sales_tax: r.sales_tax,
            congestion_surcharge: r.congestion_surcharge,
            airport_fee: r.airport_fee,
            tips: r.tips,
            driver_pay: r.driver_pay,
            shared_request_flag: r.shared_request_flag,
            shared_match_flag: r.shared_match_flag,
            access_a_ride_flag: r.access_a_ride_flag,
            wav_request_flag: r.wav_request_flag,
            wav_match_flag: r.wav_match_flag,
        }
    }
}

// ---- serde_columnar mirror: ints -> DeltaRle, String/f64 -> Rle ----

mod taxi_sc {
    use super::*;
    use serde_columnar::columnar;

    #[columnar(vec, ser, de)]
    #[derive(Debug, Clone, PartialEq)]
    pub struct TripScRow {
        #[columnar(strategy = "Rle")]
        pub license_num: String,
        #[columnar(strategy = "Rle")]
        pub dispatching_base_num: String,
        pub originating_base_num: Option<String>,
        #[columnar(strategy = "DeltaRle")]
        pub request_datetime: i64,
        pub on_scene_datetime: Option<i64>,
        #[columnar(strategy = "DeltaRle")]
        pub pickup_datetime: i64,
        #[columnar(strategy = "DeltaRle")]
        pub dropoff_datetime: i64,
        #[columnar(strategy = "DeltaRle")]
        pub pu_location_id: i32,
        #[columnar(strategy = "DeltaRle")]
        pub do_location_id: i32,
        #[columnar(strategy = "Rle")]
        pub trip_miles: f64,
        #[columnar(strategy = "DeltaRle")]
        pub trip_time: i64,
        #[columnar(strategy = "Rle")]
        pub base_passenger_fare: f64,
        #[columnar(strategy = "Rle")]
        pub tolls: f64,
        #[columnar(strategy = "Rle")]
        pub bcf: f64,
        #[columnar(strategy = "Rle")]
        pub sales_tax: f64,
        #[columnar(strategy = "Rle")]
        pub congestion_surcharge: f64,
        #[columnar(strategy = "Rle")]
        pub airport_fee: f64,
        #[columnar(strategy = "Rle")]
        pub tips: f64,
        #[columnar(strategy = "Rle")]
        pub driver_pay: f64,
        pub shared_request_flag: Option<bool>,
        pub shared_match_flag: Option<bool>,
        pub access_a_ride_flag: Option<bool>,
        pub wav_request_flag: Option<bool>,
        pub wav_match_flag: Option<bool>,
    }

    #[columnar(ser, de)]
    #[derive(Debug, Clone, PartialEq)]
    pub struct TripScStore {
        #[columnar(class = "vec")]
        pub data: Vec<TripScRow>,
    }

    impl From<&Trip> for TripScRow {
        fn from(r: &Trip) -> Self {
            TripScRow {
                license_num: r.license_num.to_string(),
                dispatching_base_num: r.dispatching_base_num.to_string(),
                originating_base_num: r.originating_base_num.as_deref().map(str::to_string),
                request_datetime: r.request_datetime.timestamp_micros(),
                on_scene_datetime: r.on_scene_datetime.map(|d| d.timestamp_micros()),
                pickup_datetime: r.pickup_datetime.timestamp_micros(),
                dropoff_datetime: r.dropoff_datetime.timestamp_micros(),
                pu_location_id: r.pu_location_id,
                do_location_id: r.do_location_id,
                trip_miles: r.trip_miles,
                trip_time: r.trip_time,
                base_passenger_fare: r.base_passenger_fare,
                tolls: r.tolls,
                bcf: r.bcf,
                sales_tax: r.sales_tax,
                congestion_surcharge: r.congestion_surcharge,
                airport_fee: r.airport_fee,
                tips: r.tips,
                driver_pay: r.driver_pay,
                shared_request_flag: r.shared_request_flag,
                shared_match_flag: r.shared_match_flag,
                access_a_ride_flag: r.access_a_ride_flag,
                wav_request_flag: r.wav_request_flag,
                wav_match_flag: r.wav_match_flag,
            }
        }
    }
}

impl common::ScStore for taxi_sc::TripScStore {
    fn row_count(&self) -> usize {
        self.data.len()
    }
}

// ---- load ----

fn lstr<'a>(batch: &'a arrow_array::RecordBatch, name: &str) -> &'a arrow_array::LargeStringArray {
    batch
        .column_by_name(name)
        .unwrap()
        .as_any()
        .downcast_ref::<arrow_array::LargeStringArray>()
        .unwrap_or_else(|| panic!("column {name} is not LargeString"))
}

fn tms<'a>(batch: &'a arrow_array::RecordBatch, name: &str) -> &'a arrow_array::TimestampMicrosecondArray {
    batch
        .column_by_name(name)
        .unwrap()
        .as_any()
        .downcast_ref::<arrow_array::TimestampMicrosecondArray>()
        .unwrap_or_else(|| panic!("column {name} is not TimestampMicrosecond"))
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

fn colf64<'a>(batch: &'a arrow_array::RecordBatch, name: &str) -> &'a arrow_array::Float64Array {
    batch
        .column_by_name(name)
        .unwrap()
        .as_any()
        .downcast_ref::<arrow_array::Float64Array>()
        .unwrap_or_else(|| panic!("column {name} is not Float64"))
}

/// Mostly-blank tri-state flag: blank ("no answer") -> `None`,
/// "N" -> `Some(false)`, "Y" -> `Some(true)`.
fn flag_value(s: &str) -> Option<bool> {
    match s {
        "Y" => Some(true),
        "N" => Some(false),
        " " => None,
        other => panic!("unexpected flag value {other:?}"),
    }
}

fn load() -> Vec<Trip> {
    let path = DS.ensure_downloaded();
    let file = std::fs::File::open(&path).unwrap();
    let builder = ParquetRecordBatchReaderBuilder::try_new(file).unwrap();
    let mut reader = builder.build().unwrap();
    let mut rows = Vec::new();
    while let Some(batch) = reader.next() {
        let batch = batch.unwrap();
        let license = lstr(&batch, "hvfhs_license_num");
        let dispatching = lstr(&batch, "dispatching_base_num");
        let originating = lstr(&batch, "originating_base_num");
        let request = tms(&batch, "request_datetime");
        let on_scene = tms(&batch, "on_scene_datetime");
        let pickup = tms(&batch, "pickup_datetime");
        let dropoff = tms(&batch, "dropoff_datetime");
        let pu = col32(&batch, "PULocationID");
        let do_ = col32(&batch, "DOLocationID");
        let miles = colf64(&batch, "trip_miles");
        let trip_time = col64(&batch, "trip_time");
        let base_fare = colf64(&batch, "base_passenger_fare");
        let tolls = colf64(&batch, "tolls");
        let bcf = colf64(&batch, "bcf");
        let sales_tax = colf64(&batch, "sales_tax");
        let congestion = colf64(&batch, "congestion_surcharge");
        let airport = colf64(&batch, "airport_fee");
        let tips = colf64(&batch, "tips");
        let driver_pay = colf64(&batch, "driver_pay");
        let shared_request = lstr(&batch, "shared_request_flag");
        let shared_match = lstr(&batch, "shared_match_flag");
        let access_a_ride = lstr(&batch, "access_a_ride_flag");
        let wav_request = lstr(&batch, "wav_request_flag");
        let wav_match = lstr(&batch, "wav_match_flag");
        for i in 0..batch.num_rows() {
            rows.push(Trip {
                license_num: SmolStr::new(license.value(i)),
                dispatching_base_num: SmolStr::new(dispatching.value(i)),
                originating_base_num: if originating.is_valid(i) {
                    Some(SmolStr::new(originating.value(i)))
                } else {
                    None
                },
                request_datetime: DateTime::from_timestamp_micros(request.value(i)).unwrap(),
                on_scene_datetime: if on_scene.is_valid(i) {
                    Some(DateTime::from_timestamp_micros(on_scene.value(i)).unwrap())
                } else {
                    None
                },
                pickup_datetime: DateTime::from_timestamp_micros(pickup.value(i)).unwrap(),
                dropoff_datetime: DateTime::from_timestamp_micros(dropoff.value(i)).unwrap(),
                pu_location_id: pu.value(i),
                do_location_id: do_.value(i),
                trip_miles: miles.value(i),
                trip_time: trip_time.value(i),
                base_passenger_fare: base_fare.value(i),
                tolls: tolls.value(i),
                bcf: bcf.value(i),
                sales_tax: sales_tax.value(i),
                congestion_surcharge: congestion.value(i),
                airport_fee: airport.value(i),
                tips: tips.value(i),
                driver_pay: driver_pay.value(i),
                shared_request_flag: flag_value(shared_request.value(i)),
                shared_match_flag: flag_value(shared_match.value(i)),
                access_a_ride_flag: flag_value(access_a_ride.value(i)),
                wav_request_flag: flag_value(wav_request.value(i)),
                wav_match_flag: flag_value(wav_match.value(i)),
            });
        }
    }
    // Load-time checks keep the pinned file honest; the Vortex string-range
    // filter depends on the integer-seconds one.
    for r in &rows {
        assert_eq!(r.request_datetime.timestamp_micros() % 1_000_000, 0, "request_datetime not an integer second");
        assert_eq!(r.pickup_datetime.timestamp_micros() % 1_000_000, 0, "pickup_datetime not an integer second");
        assert_eq!(r.dropoff_datetime.timestamp_micros() % 1_000_000, 0, "dropoff_datetime not an integer second");
        if let Some(d) = &r.on_scene_datetime {
            assert_eq!(d.timestamp_micros() % 1_000_000, 0, "on_scene_datetime not an integer second");
        }
        for v in [
            r.trip_miles,
            r.base_passenger_fare,
            r.tolls,
            r.bcf,
            r.sales_tax,
            r.congestion_surcharge,
            r.airport_fee,
            r.tips,
            r.driver_pay,
        ] {
            assert!(common::round_f64(v, 3) == v, "f64 value {v} carries more than 3 fractional digits");
        }
    }
    rows
}

// ---- roundtrip verify ----

// PcoPack returns rows in stored order (sorted by `pickup_datetime`) and the
// table has no unique key, so each field compares as a sorted multiset.

fn check_multiset<T>(field: &str, expected: Vec<T>, decoded: Vec<T>)
where
    T: Ord + PartialEq + std::fmt::Debug,
{
    let mut a = expected;
    let mut b = decoded;
    a.sort();
    b.sort();
    assert_eq!(a, b, "taxi roundtrip mismatch: {field}");
}

/// `f64` has no `Ord`; `total_cmp` gives the total order for the sorted
/// multiset compare (all fare values here are finite).
fn check_f64(field: &str, expected: Vec<f64>, decoded: Vec<f64>) {
    let mut a = expected;
    let mut b = decoded;
    a.sort_by(f64::total_cmp);
    b.sort_by(f64::total_cmp);
    assert_eq!(a, b, "taxi roundtrip mismatch: {field}");
}

fn verify_identity(data: &[Trip], back: &[Trip]) {
    macro_rules! check {
        ($f:ident) => {
            check_multiset(
                stringify!($f),
                data.iter().map(|r| r.$f.clone()).collect(),
                back.iter().map(|r| r.$f.clone()).collect(),
            );
        };
        ($f:ident, f64) => {
            check_f64(stringify!($f), data.iter().map(|r| r.$f).collect(), back.iter().map(|r| r.$f).collect());
        };
    }
    check!(license_num);
    check!(dispatching_base_num);
    check!(originating_base_num);
    check!(request_datetime);
    check!(on_scene_datetime);
    check!(pickup_datetime);
    check!(dropoff_datetime);
    check!(pu_location_id);
    check!(do_location_id);
    check!(trip_miles, f64);
    check!(trip_time);
    check!(base_passenger_fare, f64);
    check!(tolls, f64);
    check!(bcf, f64);
    check!(sales_tax, f64);
    check!(congestion_surcharge, f64);
    check!(airport_fee, f64);
    check!(tips, f64);
    check!(driver_pay, f64);
    check!(shared_request_flag);
    check!(shared_match_flag);
    check!(access_a_ride_flag);
    check!(wav_request_flag);
    check!(wav_match_flag);
}

/// The lossy variant's expected values: the nine f64s as `float_round = 3`
/// stores them, everything else identity.
fn verify_lossy(data: &[Trip], back: &[TripF]) {
    macro_rules! check {
        ($f:ident) => {
            check_multiset(
                stringify!($f),
                data.iter().map(|r| r.$f.clone()).collect(),
                back.iter().map(|r| r.$f.clone()).collect(),
            );
        };
        ($f:ident, rounded) => {
            check_f64(
                stringify!($f),
                data.iter().map(|r| common::round_f64(r.$f, 3)).collect(),
                back.iter().map(|r| r.$f).collect(),
            );
        };
    }
    check!(license_num);
    check!(dispatching_base_num);
    check!(originating_base_num);
    check!(request_datetime);
    check!(on_scene_datetime);
    check!(pickup_datetime);
    check!(dropoff_datetime);
    check!(pu_location_id);
    check!(do_location_id);
    check!(trip_miles, rounded);
    check!(trip_time);
    check!(base_passenger_fare, rounded);
    check!(tolls, rounded);
    check!(bcf, rounded);
    check!(sales_tax, rounded);
    check!(congestion_surcharge, rounded);
    check!(airport_fee, rounded);
    check!(tips, rounded);
    check!(driver_pay, rounded);
    check!(shared_request_flag);
    check!(shared_match_flag);
    check!(access_a_ride_flag);
    check!(wav_request_flag);
    check!(wav_match_flag);
}
