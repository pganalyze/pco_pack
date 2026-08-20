use smol_str::SmolStr;

#[derive(PcoPack)]
#[pco_pack(index = [device_id])]
pub struct StringIndex {
    device_id: String,
    temperature: i64,
}

#[derive(PcoPack)]
#[pco_pack(index = [device_id])]
pub struct SmolStrIndex {
    device_id: SmolStr,
    temperature: i64,
}
