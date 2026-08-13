include!("bench_common.rs");

use markdown_tables::{MarkdownTableRow, as_table};
use pco_pack::{PcoPack, Timeline};

fn main() {
    let num_records = 100_000;
    let uniqueness_pcts = [10, 20, 50, 80, 90];

    println!("## Timeline vs DateTime\n");

    #[derive(Debug)]
    struct TimelineRow {
        uniqueness: String,
        timeline_ms: String,
        datetime_ms: String,
        time_ratio: String,
        timeline_size: String,
        datetime_size: String,
        size_ratio: String,
    }

    impl MarkdownTableRow for TimelineRow {
        fn column_names() -> Vec<&'static str> {
            vec!["Uniqueness", "Timeline", "DateTime", "Time ratio", "Timeline", "DateTime", "Size ratio"]
        }

        fn column_values(&self) -> Vec<String> {
            vec![
                self.uniqueness.clone(),
                self.timeline_ms.clone(),
                self.datetime_ms.clone(),
                self.time_ratio.clone(),
                self.timeline_size.clone(),
                self.datetime_size.clone(),
                self.size_ratio.clone(),
            ]
        }
    }

    let mut rows = Vec::new();

    for pct in &uniqueness_pcts {
        let datetime_data = datetime_data(num_records, *pct);
        let timeline_data = timeline_data_from_datetime(&datetime_data);

        for rec in &timeline_data {
            assert_eq!(rec.seen_at.len(), 1);
        }

        let tl_serial_ms = avg_ms(|| TimelineRecord::write(timeline_data.clone()).unwrap());
        let dt_serial_ms = avg_ms(|| DateTimeRecord::write(datetime_data.clone()).unwrap());
        let tl_compressed = TimelineRecord::write(timeline_data.clone()).unwrap();
        let dt_compressed = DateTimeRecord::write(datetime_data.clone()).unwrap();
        let tl_buf = TimelineRecord::to_bytes(&tl_compressed).unwrap();
        let dt_buf = DateTimeRecord::to_bytes(&dt_compressed).unwrap();

        let size_ratio = dt_buf.len() as f64 / tl_buf.len() as f64;
        let time_ratio = dt_serial_ms as f64 / tl_serial_ms as f64;

        rows.push(TimelineRow {
            uniqueness: format!("{}%", pct),
            timeline_ms: format_ms(tl_serial_ms as f64),
            datetime_ms: format_ms(dt_serial_ms as f64),
            time_ratio: format!("{:.1}x", time_ratio),
            timeline_size: format_bytes(tl_buf.len()),
            datetime_size: format_bytes(dt_buf.len()),
            size_ratio: format!("{:.1}x", size_ratio),
        });
    }

    println!("{}", as_table(&rows));
    println!();
}

#[derive(Clone, PartialEq, PcoPack)]
#[pco_pack(timestamp = seen_at)]
struct TimelineRecord {
    seen_at: Timeline<10_000_000>,
    sensor_id: u32,
    temperature: f32,
    humidity: f32,
    status: u8,
}

#[derive(Clone, PartialEq, PcoPack)]
struct DateTimeRecord {
    seen_at: chrono::DateTime<chrono::Utc>,
    sensor_id: u32,
    temperature: f32,
    humidity: f32,
    status: u8,
}

/// Generate `n` DateTimeRecord entries with timestamps clustered into groups.
/// `uniqueness_pct` controls the fraction of records that become separate
/// Timeline rows (i.e., don't merge). Higher % = more groups = more unique.
///
/// Each group has timestamps within 10 seconds of each other, so they will
/// merge when converted to Timeline<10_000_000>.
///
/// Always generates exactly `n` records; the group count determines clustering density.
fn datetime_data(n: usize, uniqueness_pct: usize) -> Vec<DateTimeRecord> {
    let mut records = Vec::with_capacity(n);
    // Number of groups = n * uniqueness_pct / 100
    // e.g., 10% -> 10k groups (10 records each), 95% -> 95k groups (~1 record each)
    let num_groups = n * uniqueness_pct / 100;
    let per_group = n / num_groups;
    let remainder = n % num_groups; // distribute leftover records across first `remainder` groups

    for g in 0..num_groups {
        let group_base: i64 = (g as i64) * 1_000_000_000; // 1 billion microsecond offset per group
        // First `remainder` groups get one extra record to reach exactly n total
        let group_size = per_group + if g < remainder { 1 } else { 0 };
        for i in 0..group_size {
            // Timestamps within 10 seconds of each other (will merge into same bucket with Timeline<10_000_000>)
            let ts_us = group_base + (i as i64) * 50_000; // 50ms apart, well within 10s
            let seen_at = chrono::DateTime::<chrono::Utc>::from_timestamp_micros(ts_us).unwrap_or_default();
            records.push(DateTimeRecord {
                seen_at,
                sensor_id: (g % 10) as u32,
                temperature: 20.0 + (i as f32) * 0.1,
                humidity: 50.0 + (g as f32) * 0.5,
                status: (i % 3) as u8,
            });
        }
    }

    records
}

/// Construct TimelineRecord entries from DateTimeRecord data by grouping timestamps
/// and calling Timeline.add() for each timestamp. Overlapping/close timestamps
/// within the same group get merged, so the actual range count is much lower.
/// Non-timestamp fields are aggregated from the source DateTimeRecord entries.
fn timeline_data_from_datetime(dt_data: &[DateTimeRecord]) -> Vec<TimelineRecord> {
    // Group DateTimeRecord entries by their time bucket (1 billion microsecond windows)
    // Each group's timestamps fall within the same bucket, so they merge together.
    // Also aggregate non-timestamp fields from the group.
    struct GroupData {
        ranges: Vec<(i64, i64)>,
        sensor_ids: Vec<u32>,
        temperatures: Vec<f32>,
        humidities: Vec<f32>,
        statuses: Vec<u8>,
    }
    let mut groups: std::collections::BTreeMap<i64, GroupData> = std::collections::BTreeMap::new();
    for dt in dt_data {
        let start = dt.seen_at.timestamp_micros();
        let end = start + 999_000; // 1ms duration per timestamp
        let group_key = start / 1_000_000_000; // bucket by billion-microsecond windows
        let g = groups.entry(group_key).or_insert_with(|| GroupData {
            ranges: Vec::new(),
            sensor_ids: Vec::new(),
            temperatures: Vec::new(),
            humidities: Vec::new(),
            statuses: Vec::new(),
        });
        g.ranges.push((start, end));
        g.sensor_ids.push(dt.sensor_id);
        g.temperatures.push(dt.temperature);
        g.humidities.push(dt.humidity);
        g.statuses.push(dt.status);
    }

    let mut records = Vec::with_capacity(groups.len());
    for (_group_key, g) in groups {
        let mut timeline = Timeline::<10_000_000>::new();
        for (s, e) in g.ranges {
            timeline.add(s, e);
        }
        // Aggregate non-timestamp fields: use median-like representative values
        let sensor_id = g.sensor_ids[0]; // all sensors in a group are the same
        let temperature = g.temperatures.iter().sum::<f32>() / g.temperatures.len() as f32;
        let humidity = g.humidities.iter().sum::<f32>() / g.humidities.len() as f32;
        let status = g.statuses.iter().max().copied().unwrap_or(0);
        records.push(TimelineRecord { seen_at: timeline, sensor_id, temperature, humidity, status });
    }

    records
}
