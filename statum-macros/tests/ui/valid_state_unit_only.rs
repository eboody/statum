#![allow(unused_imports)]
extern crate self as statum;
pub use bon;
use statum_macros::{machine, state};
use bon::builder as _;

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