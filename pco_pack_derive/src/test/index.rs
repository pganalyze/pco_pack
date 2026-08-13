#[derive(PcoPack)]
#[pco_pack(index = [device_id])]
pub struct DeviceTelemetry {
    device_id: i64,
    temperature: i64,
    humidity: i64,
}
