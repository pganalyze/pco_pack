#[derive(PcoPack)]
#[pco_pack(float_round = 2)]
pub struct MetricRecord {
    id: i64,
    metrics: HashMap<String, f64>,
}

#[derive(PcoPack)]
#[pco_pack(float_round = 2)]
pub struct MapTupleRecord {
    id: i64,
    metrics: HashMap<String, (f64, i32)>,
}

#[derive(PcoPack)]
#[pco_pack(float_round = 2)]
pub struct OptionMapTupleRecord {
    id: i64,
    metrics: Option<HashMap<String, (f32, i32)>>,
}

#[derive(PcoPack)]
#[pco_pack(float_round = 2)]
pub struct VecMapTupleRecord {
    id: i64,
    batches: Vec<HashMap<String, (half::f16, i32)>>,
}
