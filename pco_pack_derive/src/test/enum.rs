#[derive(PcoPack)]
pub struct Task {
    id: i64,
    value: f64,
    label: String,
    state: State,
}

#[derive(Clone, Default, PcoPack)]
pub enum State {
    #[default]
    Idle,
    Running(i32),
    Paused,
}
