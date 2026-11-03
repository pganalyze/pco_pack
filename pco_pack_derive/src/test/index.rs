#[derive(Clone, PcoPack)]
#[pco_pack(index = [device_id])]
pub struct DeviceTelemetry {
    device_id: i64,
    temperature: i64,
    humidity: i64,
}

pub fn fold_example() -> impl pco_pack::Consumer<DeviceTelemetry> {
    pco_pack::fold!(DeviceTelemetry, |device_id: i64, temperature: i64| {
        let _ = (device_id, temperature);
    })
}
