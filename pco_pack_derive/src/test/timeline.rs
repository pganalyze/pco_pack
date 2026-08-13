use pco_pack::Timeline;

#[derive(PcoPack)]
#[pco_pack(timestamp = collected_at, index = [database_id])]
pub struct TimeSeriesCompact {
    database_id: i64,
    collected_at: Timeline<11_000_000>,
    value: f64,
}
