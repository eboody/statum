use statum::{machine, state};

#[state]
pub enum LightState {
    Off,
    On,
}

#[machine]
pub struct Light<LightState> {
    name: String,
}

fn main() {
    let light: Light<Off> = Light::<Off>::builder().name("desk".to_string()).build();
    let _ = light.name;
}
