#[derive(PcoPack)]
pub struct Outer {
    name: String,
    inner: Inner,
}

#[derive(Clone, Default, PcoPack)]
pub struct Inner {
    x: i32,
    label: String,
}
