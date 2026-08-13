#[derive(PcoPack)]
#[pco_pack(timestamp = collected_at)]
pub struct TimeSeries {
    database_id: i64,
    collected_at: i64,
    value: f64,
}
