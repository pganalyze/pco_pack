include!("bench_common.rs");

use chrono::Duration;
use markdown_tables::{MarkdownTableRow, as_table};
use pco_pack::{PcoPack, Timeline};

fn main() {
    let num_records = 100_000;
    let uniqueness_pcts = [10, 20, 50, 80, 90];

    println!("## Timeline\n");

    #[derive(Debug)]
    struct TimeRow {
        uniqueness: String,
        timeline_ms: String,
        datetime_ms: String,
        round_ms: String,
        tl_vs_dt_ratio: String,
        tr_vs_dt_ratio: String,
    }

    impl MarkdownTableRow for TimeRow {
        fn column_names() -> Vec<&'static str> {
            vec!["Uniqueness", "Timeline", "DateTime", "time_round=1s", "Time ratio (TL/DT)", "Time ratio (TR/DT)"]
        }

        fn column_values(&self) -> Vec<String> {
            vec![
                self.uniqueness.clone(),
                self.timeline_ms.clone(),
                self.datetime_ms.clone(),
                self.round_ms.clone(),
                self.tl_vs_dt_ratio.clone(),
                self.tr_vs_dt_ratio.clone(),
            ]
        }
    }

    #[derive(Debug)]
    struct SizeRow {
        uniqueness: String,
        timeline_size: String,
        datetime_size: String,
        round_size: String,
        tl_vs_dt_size_ratio: String,
        tr_vs_dt_size_ratio: String,
    }

    impl MarkdownTableRow for SizeRow {
        fn column_names() -> Vec<&'static str> {
            vec!["Uniqueness", "Timeline", "DateTime", "time_round=1s", "Size ratio (TL/DT)", "Size ratio (TR/DT)"]
        }

        fn column_values(&self) -> Vec<String> {
            vec![
                self.uniqueness.clone(),
                self.timeline_size.clone(),
                self.datetime_size.clone(),
                self.round_size.clone(),
                self.tl_vs_dt_size_ratio.clone(),
                self.tr_vs_dt_size_ratio.clone(),
            ]
        }
    }

    let mut time_rows = Vec::new();
    let mut size_rows = Vec::new();

    for pct in &uniqueness_pcts {
        let total_seconds = num_records / 100; // 100 sensors reporting every second
        let (dt_data, timeline_data) = generate_scenario(total_seconds, *pct);

        let round_data: Vec<TimeRoundRecord> = dt_data
            .iter()
            .map(|r| TimeRoundRecord {
                seen_at: r.seen_at,
                sensor_id: r.sensor_id,
                temperature: r.temperature,
                humidity: r.humidity,
                status: r.status,
            })
            .collect();

        // Every input row's second must be covered by exactly one Timeline range.
        let coverage: usize = timeline_data
            .iter()
            .flat_map(|r| r.seen_at.ranges())
            .map(|&(s, e)| ((e - s + 1) / 1_000_000) as usize)
            .sum();
        assert_eq!(dt_data.len(), coverage);

        let tl_serial_ms = avg_ms(|| TimelineRecord::write(timeline_data.clone()).unwrap());
        let dt_serial_ms = avg_ms(|| DateTimeRecord::write(dt_data.clone()).unwrap());
        let round_serial_ms = avg_ms(|| TimeRoundRecord::write(round_data.clone()).unwrap());

        let tl_compressed = TimelineRecord::write(timeline_data.clone()).unwrap();
        let dt_compressed = DateTimeRecord::write(dt_data.clone()).unwrap();
        let round_compressed = TimeRoundRecord::write(round_data.clone()).unwrap();

        let tl_buf = TimelineRecord::to_bytes(&tl_compressed).unwrap();
        let dt_buf = DateTimeRecord::to_bytes(&dt_compressed).unwrap();
        let round_buf = TimeRoundRecord::to_bytes(&round_compressed).unwrap();

        time_rows.push(TimeRow {
            uniqueness: format!("{}%", pct),
            timeline_ms: format_ms(tl_serial_ms as f64),
            datetime_ms: format_ms(dt_serial_ms as f64),
            round_ms: format_ms(round_serial_ms as f64),
            tl_vs_dt_ratio: format!("{:.1}x", dt_serial_ms as f64 / tl_serial_ms as f64),
            tr_vs_dt_ratio: format!("{:.2}x", dt_serial_ms as f64 / round_serial_ms as f64),
        });

        size_rows.push(SizeRow {
            uniqueness: format!("{}%", pct),
            timeline_size: format_bytes(tl_buf.len()),
            datetime_size: format_bytes(dt_buf.len()),
            round_size: format_bytes(round_buf.len()),
            tl_vs_dt_size_ratio: format!("{:.1}x", dt_buf.len() as f64 / tl_buf.len() as f64),
            tr_vs_dt_size_ratio: format!("{:.2}x", dt_buf.len() as f64 / round_buf.len() as f64),
        });
    }

    println!("### Serialization time\n");
    println!("{}", as_table(&time_rows));
    println!();
    println!("### Compressed size\n");
    println!("{}", as_table(&size_rows));
    println!();
}

#[derive(Clone, PartialEq, PcoPack)]
#[pco_pack(timestamp = seen_at)]
struct TimelineRecord {
    seen_at: Timeline<1_000_000>,
    sensor_id: u32,
    temperature: f32,
    humidity: f32,
    status: u8,
}

#[derive(Clone, PartialEq, PcoPack)]
#[pco_pack(timestamp = seen_at)]
struct DateTimeRecord {
    seen_at: chrono::DateTime<chrono::Utc>,
    sensor_id: u32,
    temperature: f32,
    humidity: f32,
    status: u8,
}

#[derive(Clone, PartialEq, PcoPack)]
#[pco_pack(timestamp = seen_at, time_round = Duration::seconds(1))]
struct TimeRoundRecord {
    seen_at: chrono::DateTime<chrono::Utc>,
    sensor_id: u32,
    temperature: f32,
    humidity: f32,
    status: u8,
}

/// Simulate 100 sensors reporting every second, with persistent state across
/// each second except for the `uniqueness_pct` rows whose values change.
fn generate_scenario(total_seconds: usize, uniqueness_pct: usize) -> (Vec<DateTimeRecord>, Vec<TimelineRecord>) {
    const NUM_SENSORS: u32 = 100;
    const BASE_TS_S: i64 = 1_767_225_600; // 2026-01-01T00:00:00Z
    let mut dt_data = Vec::with_capacity(total_seconds * NUM_SENSORS as usize);
    struct SensorState {
        temperature: f32,
        humidity: f32,
        status: u8,
        timeline_row: Option<TimelineRecord>,
    }
    let mut sensors: Vec<SensorState> = (0..NUM_SENSORS)
        .map(|id| SensorState {
            temperature: random_temperature(id, 0),
            humidity: random_humidity(id, 0),
            status: random_status(id, 0),
            timeline_row: None,
        })
        .collect();
    let mut timeline_data = Vec::with_capacity(total_seconds * NUM_SENSORS as usize);
    for s in 0..total_seconds {
        let ts_s = BASE_TS_S + s as i64;
        let us = splitmix64(ts_s as u64) % 5_000; // 0-5ms jitter
        let seen_at = chrono::DateTime::<chrono::Utc>::from_timestamp(ts_s, us as u32).unwrap_or_default();
        for (id, sensor) in sensors.iter_mut().enumerate() {
            if splitmix64(id as u64 * 31 + s as u64) % 100 < uniqueness_pct as u64 {
                if let Some(row) = sensor.timeline_row.take() {
                    timeline_data.push(row);
                }
                let change_count = id * total_seconds + s;
                sensor.temperature = random_temperature(id as u32, change_count);
                sensor.humidity = random_humidity(id as u32, change_count);
                sensor.status = random_status(id as u32, change_count);
            }
            let row = sensor.timeline_row.get_or_insert_with(|| TimelineRecord {
                seen_at: Timeline::<1_000_000>::new(),
                sensor_id: id as u32,
                temperature: sensor.temperature,
                humidity: sensor.humidity,
                status: sensor.status,
            });
            row.seen_at.add(seen_at.timestamp_micros(), seen_at.timestamp_micros());
            dt_data.push(DateTimeRecord {
                seen_at,
                sensor_id: id as u32,
                temperature: sensor.temperature,
                humidity: sensor.humidity,
                status: sensor.status,
            });
        }
    }
    for sensor in &mut sensors {
        if let Some(row) = sensor.timeline_row.take() {
            timeline_data.push(row);
        }
    }
    (dt_data, timeline_data)
}

fn random_temperature(sensor_id: u32, change_count: usize) -> f32 {
    // [20, 25)
    20.0 + (splitmix64(sensor_id as u64 * 1_000_003 + change_count as u64) % 1_000_000) as f32 / 200_000.0
}

fn random_humidity(sensor_id: u32, change_count: usize) -> f32 {
    // [50, 100)
    50.0 + (splitmix64(sensor_id as u64 * 1_000_003 + change_count as u64 + 7919) % 1_000_000) as f32 / 20_000.0
}

fn random_status(sensor_id: u32, change_count: usize) -> u8 {
    (splitmix64(sensor_id as u64 * 1_000_003 + change_count as u64 + 104729) % 5) as u8
}

/// Generates pseudo-random values without sequential patterns
fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}
