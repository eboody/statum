use statum::{machine, state};

#[state]
pub enum ToggleState {
    On,
    Off,
}

#[machine]
pub struct Switch<ToggleState>;

fn main() {
    let _: Switch<On> = Switch::<On>::builder().build();
}
